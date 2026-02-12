//! Ratatui-based TUI for cellbook.

pub(crate) mod config;
pub(crate) mod events;
mod state;
mod ui;

use std::io::{Read, Write};
use std::process::Command;
use std::time::{Duration, Instant};

pub use events::TuiEvent;
use events::{Action, AppEvent, EventHandler, handle_key};
use gag::BufferRedirect;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::MoveTo;
use ratatui::crossterm::event::Event as CrosstermEvent;
use ratatui::crossterm::terminal::{
    Clear,
    ClearType,
    EnterAlternateScreen,
    LeaveAlternateScreen,
    disable_raw_mode,
    enable_raw_mode,
};
use ratatui::crossterm::{ExecutableCommand, execute};
use state::{App, BuildStatus, CellOutput, CellStatus};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::errors::Result;
use crate::loader::LoadedLibrary;
use crate::{store, watcher};

type AppTerminal = Terminal<CrosstermBackend<std::io::Stderr>>;

/// Run the TUI.
pub async fn run(
    lib: &mut LoadedLibrary,
    event_tx: mpsc::Sender<TuiEvent>,
    event_rx: mpsc::Receiver<TuiEvent>,
    app_config: config::AppConfig,
) -> Result<()> {
    let mut terminal = init_terminal()?;

    // Set image viewer env var for cells to use.
    if let Some(viewer) = app_config.general.image_viewer.as_ref() {
        // SAFETY: Called once at startup before cells run.
        unsafe { std::env::set_var("CELLBOOK_IMAGE_VIEWER", viewer) };
    }

    let mut app = App::new(visible_cells(lib), app_config.general.show_timings);
    app.refresh_context(store::list());
    let mut cell_task: Option<JoinHandle<()>> = spawn_cell(lib, &mut app, 0, &event_tx);

    let mut events = EventHandler::new(event_rx, Duration::from_millis(100));

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if let Some(event) = events.next().await {
            match event {
                AppEvent::Terminal(CrosstermEvent::Key(key)) => {
                    let action = handle_key(key, &mut app, &app_config);
                    match action {
                        Action::Quit => break,
                        Action::RunCell(idx) => {
                            if !app.executing {
                                cell_task = spawn_cell(lib, &mut app, idx, &event_tx);
                            }
                        }
                        Action::ViewOutput => {
                            if let Some(name) = app.selected_cell_name()
                                && let Some(output) = app.get_output(name)
                            {
                                events.stop();
                                view_output_in_pager(&output.stdout);
                                terminal = init_terminal()?;
                                events.resume();
                            }
                        }
                        Action::ViewError => {
                            if let Some(idx) = app.selected_cell_index()
                                && let Some(error) = app.get_error(idx)
                            {
                                events.stop();
                                view_output_in_pager(error);
                                terminal = init_terminal()?;
                                events.resume();
                            }
                        }
                        Action::ViewBuildError => {
                            if let BuildStatus::BuildError(error) = &app.build_status {
                                events.stop();
                                view_output_in_pager(error);
                                terminal = init_terminal()?;
                                events.resume();
                            }
                        }
                        Action::ClearContext => {
                            store::clear();
                            app.refresh_context(store::list());
                        }
                        Action::Reload => {
                            cell_task = trigger_reload(&mut app, lib, &event_tx, cell_task.take()).await;
                        }
                        Action::Edit => {
                            let line = app.selected_cell_index().and_then(|i| {
                                if i == 0 {
                                    Some(lib.init_line())
                                } else {
                                    lib.cells().get(i - 1).map(|c| c.line)
                                }
                            });
                            events.stop();
                            edit_cellbook(line);
                            terminal = init_terminal()?;
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
                    // Abort any running cell task before reloading the library.
                    // The spawned future holds code from the current dylib, so it
                    // must be dropped before the library is unmapped.
                    if let Some(handle) = cell_task.take() {
                        handle.abort();
                        let _ = handle.await;
                    }
                    app.executing = false;
                    app.build_status = BuildStatus::Reloading;
                    match lib.reload() {
                        Ok(()) => {
                            app.refresh_cells(visible_cells(lib));
                            cell_task = spawn_cell(lib, &mut app, 0, &event_tx);
                            app.build_status = BuildStatus::Idle;
                        }
                        Err(e) => {
                            app.build_status = BuildStatus::BuildError(e.to_string());
                        }
                    }
                }

                AppEvent::Tui(TuiEvent::CellCompleted {
                    idx,
                    name,
                    stdout,
                    duration,
                    result,
                }) => {
                    app.increment_count(&name);
                    match result {
                        Ok(()) => {
                            app.cell_statuses[idx] = CellStatus::Success;
                        }
                        Err(e) => {
                            app.cell_statuses[idx] = CellStatus::Error(e);
                        }
                    }
                    app.store_output(&name, CellOutput { stdout, duration });
                    app.refresh_context(store::list());
                    app.executing = false;
                    cell_task = None;
                }

                AppEvent::Tick => {}

                _ => {}
            }
        }
    }

    // Abort any running cell task before exiting.
    if let Some(handle) = cell_task.take() {
        handle.abort();
        let _ = handle.await;
    }

    restore_terminal();

    Ok(())
}

fn init_terminal() -> Result<AppTerminal> {
    enable_raw_mode()?;
    execute!(std::io::stderr(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(std::io::stderr());
    Ok(Terminal::new(backend)?)
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stderr(), LeaveAlternateScreen);
}

/// Trigger a manual rebuild and reload.
/// Aborts any running cell task before reloading the library to prevent UB.
async fn trigger_reload(
    app: &mut App,
    lib: &mut LoadedLibrary,
    event_tx: &mpsc::Sender<TuiEvent>,
    cell_task: Option<JoinHandle<()>>,
) -> Option<JoinHandle<()>> {
    app.build_status = BuildStatus::Building;

    match watcher::rebuild().await {
        Ok(()) => {
            if let Some(handle) = cell_task {
                handle.abort();
                let _ = handle.await;
            }
            app.executing = false;
            app.build_status = BuildStatus::Reloading;
            match lib.reload() {
                Ok(()) => {
                    app.refresh_cells(visible_cells(lib));
                    let handle = spawn_cell(lib, app, 0, event_tx);
                    app.build_status = BuildStatus::Idle;
                    handle
                }
                Err(e) => {
                    app.build_status = BuildStatus::BuildError(e.to_string());
                    None
                }
            }
        }
        Err(e) => {
            app.build_status = BuildStatus::BuildError(e.to_string());
            cell_task
        }
    }
}

/// Spawn a cell as a background task, sending the result via `event_tx`.
/// Returns the `JoinHandle` so it can be aborted before a library reload.
fn spawn_cell(
    lib: &LoadedLibrary,
    app: &mut App,
    idx: usize,
    event_tx: &mpsc::Sender<TuiEvent>,
) -> Option<JoinHandle<()>> {
    if idx >= app.cells.len() {
        return None;
    }

    let cell_name = app.cells[idx].clone();
    app.executing = true;
    app.cell_statuses[idx] = CellStatus::Running;

    let future = if idx == 0 {
        lib.init_future()
    } else {
        match lib.cell_future(&cell_name) {
            Ok(f) => f,
            Err(e) => {
                app.cell_statuses[idx] = CellStatus::Error(e.to_string());
                app.executing = false;
                return None;
            }
        }
    };

    let tx = event_tx.clone();
    let name = cell_name.clone();
    let handle = tokio::spawn(async move {
        let start = Instant::now();
        let (stdout, result) = capture_stdout(|| async { future.await.map_err(|e| e.to_string()) }).await;
        let duration = start.elapsed();

        let _ = tx
            .send(TuiEvent::CellCompleted {
                idx,
                name,
                stdout,
                duration,
                result,
            })
            .await;
    });
    Some(handle)
}

fn visible_cells(lib: &LoadedLibrary) -> Vec<String> {
    let mut cells = Vec::with_capacity(lib.cells().len() + 1);
    cells.push(lib.init_name().to_string());
    cells.extend(lib.cells().iter().map(|c| c.name.clone()));
    cells
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
    restore_terminal();

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
    restore_terminal();

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
