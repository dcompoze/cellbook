//! Ratatui-based TUI for cellbook.

mod config;
mod events;
mod state;
mod ui;

use std::io::{Read, Write};
use std::process::Command;
use std::time::{Duration, Instant};

use gag::BufferRedirect;
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::event::Event as CrosstermEvent;
use ratatui::crossterm::terminal::{Clear, ClearType};
use ratatui::crossterm::ExecutableCommand;
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

    // Load TUI configuration and ensure default config file exists.
    config::ensure_config_exists();
    let tui_config = config::load();

    // Set image viewer env var for cells to use.
    // Global config takes precedence over notebook config.
    let image_viewer = tui_config
        .general
        .image_viewer
        .as_ref()
        .or(lib.config().image_viewer.as_ref());
    if let Some(viewer) = image_viewer {
        // SAFETY: Called once at startup before cells run.
        unsafe { std::env::set_var("CELLBOOK_IMAGE_VIEWER", viewer) };
    }

    let cells: Vec<String> = lib.cells().iter().map(|c| c.name.clone()).collect();
    let mut app = App::new(cells, lib.config().show_timings);
    app.refresh_context(store::list());

    let mut events = EventHandler::new(event_rx, Duration::from_millis(100));

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Terminal(CrosstermEvent::Key(key)) => {
                    let action = handle_key(key, &mut app, &tui_config);
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
                        Action::ViewError => {
                            if let Some(idx) = app.selected_cell_index()
                                && let Some(error) = app.get_error(idx)
                            {
                                events.stop();
                                view_output_in_pager(error);
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
                            let line = app
                                .selected_cell_index()
                                .and_then(|i| lib.cells().get(i))
                                .map(|c| c.line);
                            events.stop();
                            edit_cellbook(line);
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
    ratatui::restore();

    // Clear screen to minimize flash of terminal history.
    let _ = std::io::stdout()
        .execute(Clear(ClearType::All))
        .and_then(|s| s.execute(MoveTo(0, 0)));

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
/// If a line number is provided, attempts to open at that line.
fn edit_cellbook(line: Option<u32>) {
    ratatui::restore();

    // Clear screen to minimize flash of terminal history.
    let _ = std::io::stdout()
        .execute(Clear(ClearType::All))
        .and_then(|s| s.execute(MoveTo(0, 0)));

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let mut cmd = Command::new(&editor);

    // Most editors support +LINE syntax (vim, nvim, nano, emacs, etc).
    if let Some(n) = line {
        cmd.arg(format!("+{}", n));
    }

    cmd.arg("cellbook.rs");
    let _ = cmd.status();
}
