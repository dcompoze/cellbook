//! Ratatui-based TUI for cellbook.

mod events;
mod state;
mod ui;

use std::io::{Read, Write};
use std::process::Command;
use std::time::{Duration, Instant};

use gag::BufferRedirect;
use ratatui::crossterm::event::Event as CrosstermEvent;
use tokio::sync::mpsc;

use crate::errors::Result;
use crate::loader::LoadedLibrary;
use crate::runner::TuiEvent;
use crate::store;
use crate::watcher;

use events::{handle_key, Action, AppEvent, EventHandler};
use state::{App, BuildStatus, CellOutput, CellStatus};

/// Run the TUI.
pub async fn run(lib: &mut LoadedLibrary, event_rx: mpsc::Receiver<TuiEvent>) -> Result<()> {
    let mut terminal = ratatui::init();

    let cells: Vec<String> = lib.cells().iter().map(|c| c.name.clone()).collect();
    let mut app = App::new(cells, lib.config().show_timings);
    app.refresh_context(store::list());

    let mut events = EventHandler::new(event_rx, Duration::from_millis(100));

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Terminal(CrosstermEvent::Key(key)) => {
                    let action = handle_key(key, &mut app);
                    match action {
                        Action::Quit => break,
                        Action::RunCell(idx) => {
                            if !app.executing {
                                run_cell_with_capture(lib, &mut app, idx).await;
                            }
                        }
                        Action::ViewOutput => {
                            if let Some(name) = app.selected_cell_name()
                                && let Some(output) = app.get_output(name)
                            {
                                events.stop();
                                view_output_in_pager(&output.stdout);
                                terminal = ratatui::init();
                                events.resume();
                            }
                        }
                        Action::ClearContext => {
                            store::clear();
                            app.refresh_context(store::list());
                        }
                        Action::Reload => {
                            trigger_reload(&mut app, lib).await;
                        }
                        Action::Edit => {
                            events.stop();
                            edit_cellbook();
                            terminal = ratatui::init();
                            events.resume();
                        }
                        Action::None => {}
                    }
                }

                AppEvent::Terminal(CrosstermEvent::Resize(_, _)) => {
                    // Terminal handles resize automatically.
                }

                AppEvent::Tui(TuiEvent::BuildStarted) => {
                    app.build_status = BuildStatus::Building;
                }

                AppEvent::Tui(TuiEvent::BuildCompleted(None)) => {
                    app.build_status = BuildStatus::Idle;
                }

                AppEvent::Tui(TuiEvent::BuildCompleted(Some(err))) => {
                    app.build_status = BuildStatus::BuildError(err);
                }

                AppEvent::Tui(TuiEvent::Reloaded) => {
                    app.build_status = BuildStatus::Reloading;
                    match lib.reload() {
                        Ok(()) => {
                            let cells: Vec<String> =
                                lib.cells().iter().map(|c| c.name.clone()).collect();
                            app.refresh_cells(cells);
                            app.build_status = BuildStatus::Idle;
                        }
                        Err(e) => {
                            app.build_status = BuildStatus::BuildError(e.to_string());
                        }
                    }
                }

                AppEvent::Tick => {}

                _ => {}
            }
        }
    }

    ratatui::restore();

    Ok(())
}

/// Trigger a manual rebuild and reload.
async fn trigger_reload(app: &mut App, lib: &mut LoadedLibrary) {
    app.build_status = BuildStatus::Building;

    match watcher::rebuild().await {
        Ok(()) => {
            app.build_status = BuildStatus::Reloading;
            match lib.reload() {
                Ok(()) => {
                    let cells: Vec<String> =
                        lib.cells().iter().map(|c| c.name.clone()).collect();
                    app.refresh_cells(cells);
                    app.build_status = BuildStatus::Idle;
                }
                Err(e) => {
                    app.build_status = BuildStatus::BuildError(e.to_string());
                }
            }
        }
        Err(e) => {
            app.build_status = BuildStatus::BuildError(e.to_string());
        }
    }
}

/// Run a cell and capture its stdout.
async fn run_cell_with_capture(lib: &LoadedLibrary, app: &mut App, idx: usize) {
    if idx >= app.cells.len() {
        return;
    }

    let cell_name = app.cells[idx].clone();
    app.executing = true;
    app.cell_statuses[idx] = CellStatus::Running;

    let start = Instant::now();

    // Capture stdout during cell execution.
    let (captured, result) = capture_stdout(|| async {
        lib.run_cell(&cell_name).await
    })
    .await;

    let elapsed = start.elapsed();

    app.increment_count(&cell_name);

    match result {
        Ok(()) => {
            app.cell_statuses[idx] = CellStatus::Success;
        }
        Err(e) => {
            app.cell_statuses[idx] = CellStatus::Error(e.to_string());
        }
    }

    // Store the captured output.
    app.store_output(
        &cell_name,
        CellOutput {
            stdout: captured,
            duration: elapsed,
        },
    );

    app.refresh_context(store::list());
    app.executing = false;
}

/// Capture stdout during execution of an async closure.
async fn capture_stdout<F, Fut, T>(f: F) -> (String, T)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let mut buf = match BufferRedirect::stdout() {
        Ok(buf) => buf,
        Err(_) => return (String::new(), f().await),
    };

    let result = f().await;
    let _ = std::io::stdout().flush();

    let mut output = String::new();
    let _ = buf.read_to_string(&mut output);

    (output, result)
}

/// View output in an external pager.
fn view_output_in_pager(output: &str) {
    // Restore terminal before spawning pager.
    ratatui::restore();

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let mut child = match Command::new(&pager)
        .arg("-R") // Enable raw control chars for less.
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => {
            // Fallback: just print the output.
            print!("{}", output);
            let _ = std::io::stdout().flush();
            return;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(output.as_bytes());
    }

    let _ = child.wait();
}

/// Open cellbook.rs in the user's editor.
fn edit_cellbook() {
    ratatui::restore();

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let _ = Command::new(&editor)
        .arg("cellbook.rs")
        .status();
}
