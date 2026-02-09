//! File watching and automatic rebuild for hot-reloading.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use ratatui::crossterm::cursor::{MoveToColumn, MoveUp};
use ratatui::crossterm::execute;
use ratatui::crossterm::style::Print;
use ratatui::crossterm::terminal::{Clear, ClearType};
#[cfg(windows)]
use ratatui::crossterm::QueueableCommand;
use serde::Deserialize;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebouncedEventKind, Debouncer, new_debouncer};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

use crate::errors::{Error, Result};
use crate::runner::TuiEvent;
use crate::tui::config::GeneralConfig;

type NotifyDebouncer = Debouncer<RecommendedWatcher>;

struct DeleteLines(pub u16);

impl ratatui::crossterm::Command for DeleteLines {
    fn write_ansi(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        write!(f, "\x1b[{}M", self.0)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        // Best-effort fallback for older Windows consoles without ANSI delete-line support.
        let mut stdout = std::io::stdout();
        stdout.queue(MoveUp(self.0))?;
        stdout.queue(ratatui::crossterm::terminal::ScrollUp(self.0))?;
        Ok(())
    }
}

fn get_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

#[derive(Debug, Deserialize)]
struct CargoMetadata {
    workspace_root: PathBuf,
}

fn workspace_root_from_metadata() -> Option<PathBuf> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let metadata = serde_json::from_slice::<CargoMetadata>(&output.stdout).ok()?;
    Some(metadata.workspace_root)
}

fn has_lockfile() -> bool {
    if Path::new("Cargo.lock").exists() {
        return true;
    }

    workspace_root_from_metadata()
        .map(|root| root.join("Cargo.lock").exists())
        .unwrap_or(false)
}

fn cargo_build_args() -> Vec<&'static str> {
    let mut args = vec!["build", "--lib"];
    if has_lockfile() {
        args.push("--locked");
    }
    args
}

fn cargo_build_display_cmd() -> String {
    format!("cargo {}", cargo_build_args().join(" "))
}

/// Check if any paths have changed since last recorded.
/// First-time observations are recorded but do not count as changes.
fn has_actual_changes(paths: &[PathBuf], mtimes: &mut HashMap<PathBuf, SystemTime>) -> bool {
    let mut changed = false;
    for path in paths {
        if let Some(current_mtime) = get_mtime(path) {
            match mtimes.get(path) {
                Some(previous_mtime) if *previous_mtime != current_mtime => {
                    mtimes.insert(path.clone(), current_mtime);
                    changed = true;
                }
                Some(_) => {}
                None => {
                    mtimes.insert(path.clone(), current_mtime);
                }
            }
        }
    }
    changed
}

pub struct WatcherHandle {
    shutdown_tx: oneshot::Sender<()>,
    _debouncer: NotifyDebouncer,
}

impl WatcherHandle {
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Start watching source files and trigger rebuilds on changes.
///
/// Returns `None` if auto-reload is disabled.
pub async fn start_watcher(
    event_tx: mpsc::Sender<TuiEvent>,
    config: &GeneralConfig,
) -> Result<Option<WatcherHandle>> {
    if !config.auto_reload {
        return Ok(None);
    }

    let (tx, rx) = std::sync::mpsc::channel();

    let debounce_duration = Duration::from_millis(config.debounce_ms as u64);
    let mut debouncer = new_debouncer(debounce_duration, tx).map_err(|e| Error::Watch(e.to_string()))?;

    let cellbook_rs = Path::new("cellbook.rs");
    let src_path = Path::new("src");

    if cellbook_rs.exists() {
        debouncer
            .watcher()
            .watch(cellbook_rs, RecursiveMode::NonRecursive)
            .map_err(|e| Error::Watch(e.to_string()))?;
    }
    if src_path.exists() {
        debouncer
            .watcher()
            .watch(src_path, RecursiveMode::Recursive)
            .map_err(|e| Error::Watch(e.to_string()))?;
    }

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

    let (file_event_tx, mut file_event_rx) = mpsc::channel(32);
    tokio::task::spawn_blocking(move || {
        while let Ok(event) = rx.recv() {
            if file_event_tx.blocking_send(event).is_err() {
                break;
            }
        }
    });

    let mut mtimes: HashMap<PathBuf, SystemTime> = HashMap::new();

    if cellbook_rs.exists()
        && let Ok(canonical) = cellbook_rs.canonicalize()
        && let Some(mtime) = get_mtime(&canonical)
    {
        mtimes.insert(canonical, mtime);
    }

    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;

                _ = &mut shutdown_rx => {
                    break;
                }

                event = file_event_rx.recv() => {
                    match event {
                        Some(Ok(events)) => {
                            let rs_paths: Vec<PathBuf> = events
                                .iter()
                                .filter(|e| matches!(e.kind, DebouncedEventKind::Any))
                                .filter(|e| {
                                    e.path.extension().map(|ext| ext == "rs").unwrap_or(false)
                                        || e.path.file_name().map(|n| n == "cellbook.rs").unwrap_or(false)
                                })
                                .filter_map(|e| e.path.canonicalize().ok())
                                .collect();

                            if !rs_paths.is_empty() && has_actual_changes(&rs_paths, &mut mtimes) {
                                let _ = event_tx.send(TuiEvent::BuildStarted).await;
                                match rebuild().await {
                                    Ok(()) => {
                                        let _ = event_tx.send(TuiEvent::BuildCompleted(None)).await;
                                        let _ = event_tx.send(TuiEvent::Reloaded).await;
                                    }
                                    Err(e) => {
                                        let _ = event_tx
                                            .send(TuiEvent::BuildCompleted(Some(e.to_string())))
                                            .await;
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            eprintln!("Watch error: {:?}", e);
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok(Some(WatcherHandle {
        shutdown_tx,
        _debouncer: debouncer,
    }))
}

pub async fn rebuild() -> Result<()> {
    let args = cargo_build_args();
    let output = Command::new("cargo")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Build(stderr.to_string()));
    }

    Ok(())
}

pub async fn initial_build() -> Result<()> {
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let build_cmd = cargo_build_display_cmd();
    let latest_output = Arc::new(Mutex::new(String::new()));

    // Reserve two terminal lines that we redraw in-place:
    // line 1: spinner + command
    // line 2: latest output line from the build stream
    let _ = execute!(
        std::io::stdout(),
        Print(format!("{} Building notebook: {}\n\n", spinner_chars[0], build_cmd))
    );

    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();

    let output_for_spinner = Arc::clone(&latest_output);
    let spinner_handle = tokio::spawn(async move {
        let mut idx = 0;
        loop {
            let output_line = output_for_spinner
                .lock()
                .map(|s| s.clone())
                .unwrap_or_default();

            let _ = execute!(
                std::io::stdout(),
                MoveUp(2),
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(format!("{} Building notebook: {}", spinner_chars[idx], build_cmd)),
                Print("\n"),
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(output_line),
                Print("\n")
            );
            idx = (idx + 1) % spinner_chars.len();

            tokio::select! {
                biased;
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_millis(80)) => {}
            }
        }
    });

    let output_for_reader = Arc::clone(&latest_output);
    let build_result = tokio::task::spawn_blocking(move || -> Result<()> {
        let args = cargo_build_args();
        let mut child = std::process::Command::new("cargo")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stderr_log = String::new();
        if let Some(stderr) = child.stderr.take() {
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                let line = line?;
                if let Ok(mut latest) = output_for_reader.lock() {
                    *latest = line.clone();
                }
                stderr_log.push_str(&line);
                stderr_log.push('\n');
            }
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(Error::Build(stderr_log));
        }

        Ok(())
    })
    .await
    .map_err(|e| Error::Watch(e.to_string()))?;

    let _ = stop_tx.send(());
    let _ = spinner_handle.await;
    // Remove the two reserved lines entirely so follow-up output (including
    // errors) is printed normally without empty spacer lines.
    let _ = execute!(std::io::stdout(), MoveUp(2), DeleteLines(2));

    build_result
}
