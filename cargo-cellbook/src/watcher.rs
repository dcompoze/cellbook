//! File watching and automatic rebuild for hot-reloading.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

use crate::errors::{Error, Result};
use crate::loader::Config;
use crate::runner::TuiEvent;

type NotifyDebouncer = Debouncer<RecommendedWatcher>;

fn get_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
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
/// Returns `None` if `config.auto_reload` is false.
pub async fn start_watcher(
    event_tx: mpsc::Sender<TuiEvent>,
    config: &Config,
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
    let output = Command::new("cargo")
        .args(["build", "--lib"])
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
    println!("Building...");
    rebuild().await?;
    println!("Build complete.\n");
    Ok(())
}
