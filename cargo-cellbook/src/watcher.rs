//! File watching and automatic rebuild for hot-reloading.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::errors::{Error, Result};
use crate::loader::Config;
use crate::runner::TuiEvent;

/// Start watching source files and trigger rebuilds on changes.
///
/// Uses the provided config to determine debounce delay and whether to watch at all.
/// If `config.auto_reload` is false, this function returns immediately without
/// starting a watcher.
pub async fn start_watcher(
    event_tx: mpsc::Sender<TuiEvent>,
    config: &Config,
) -> Result<()> {
    if !config.auto_reload {
        return Ok(());
    }

    let (tx, rx) = std::sync::mpsc::channel();

    let debounce_duration = Duration::from_millis(config.debounce_ms as u64);
    let mut debouncer = new_debouncer(debounce_duration, tx)
        .map_err(|e| Error::Watch(e.to_string()))?;

    // Watch cellbook.rs (flat structure) or src directory (traditional structure)
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

    // Spawn a task to handle file events
    let event_tx_clone = event_tx.clone();
    tokio::task::spawn_blocking(move || {
        // Keep debouncer alive
        let _debouncer = debouncer;

        loop {
            match rx.recv() {
                Ok(Ok(events)) => {
                    // Check if any event is a meaningful file change
                    let has_changes = events.iter().any(|e| {
                        if !matches!(e.kind, DebouncedEventKind::Any) {
                            return false;
                        }
                        // Check for .rs extension or cellbook.rs specifically
                        e.path.extension().map(|ext| ext == "rs").unwrap_or(false)
                            || e.path.file_name().map(|n| n == "cellbook.rs").unwrap_or(false)
                    });

                    if has_changes {
                        // Trigger rebuild
                        let tx = event_tx_clone.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(TuiEvent::BuildStarted).await;
                            match rebuild().await {
                                Ok(()) => {
                                    let _ = tx.send(TuiEvent::BuildCompleted(None)).await;
                                    let _ = tx.send(TuiEvent::Reloaded).await;
                                }
                                Err(e) => {
                                    let _ = tx
                                        .send(TuiEvent::BuildCompleted(Some(e.to_string())))
                                        .await;
                                }
                            }
                        });
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("Watch error: {:?}", e);
                }
                Err(_) => {
                    // Channel closed, exit
                    break;
                }
            }
        }
    });

    Ok(())
}

/// Run cargo build --lib
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

/// Run initial build
pub async fn initial_build() -> Result<()> {
    println!("Building...");
    rebuild().await?;
    println!("Build complete.\n");
    Ok(())
}
