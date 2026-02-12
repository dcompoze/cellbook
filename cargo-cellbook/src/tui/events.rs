//! Event handling for the TUI.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use tokio::sync::mpsc;

use super::config::AppConfig;
use super::state::App;

/// Events sent from the watcher or spawned tasks to the TUI.
pub enum TuiEvent {
    Reloaded,
    BuildStarted,
    BuildCompleted(Option<String>),
    CellCompleted {
        idx: usize,
        name: String,
        stdout: String,
        duration: Duration,
        result: std::result::Result<(), String>,
    },
}

/// Unified event type for the TUI.
pub enum AppEvent {
    /// Terminal event from crossterm.
    Terminal(CrosstermEvent),

    /// Event from the file watcher or spawned tasks.
    Tui(TuiEvent),

    /// Periodic tick for animations.
    Tick,
}

/// Event handler that bridges crossterm with tokio.
pub struct EventHandler {
    terminal_rx: mpsc::UnboundedReceiver<CrosstermEvent>,
    tui_rx: mpsc::Receiver<TuiEvent>,
    tick_rate: Duration,
    stop_flag: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

fn spawn_poll_thread(
    stop_flag: Arc<AtomicBool>,
    terminal_tx: mpsc::UnboundedSender<CrosstermEvent>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            if event::poll(Duration::from_millis(50)).unwrap_or(false)
                && let Ok(evt) = event::read()
                && terminal_tx.send(evt).is_err()
            {
                break;
            }
        }
    })
}

impl EventHandler {
    pub fn new(tui_rx: mpsc::Receiver<TuiEvent>, tick_rate: Duration) -> Self {
        let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let handle = spawn_poll_thread(stop_flag.clone(), terminal_tx);

        Self {
            terminal_rx,
            tui_rx,
            tick_rate,
            stop_flag,
            thread_handle: Some(handle),
        }
    }

    /// Stop the event polling thread and wait for it to finish.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Resume event polling with a new thread.
    pub fn resume(&mut self) {
        self.stop_flag.store(false, Ordering::Relaxed);

        let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
        self.terminal_rx = terminal_rx;
        self.thread_handle = Some(spawn_poll_thread(self.stop_flag.clone(), terminal_tx));
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        let tick = tokio::time::sleep(self.tick_rate);

        tokio::select! {
            biased;

            event = self.tui_rx.recv() => {
                event.map(AppEvent::Tui)
            }

            event = self.terminal_rx.recv() => {
                event.map(AppEvent::Terminal)
            }

            _ = tick => {
                Some(AppEvent::Tick)
            }
        }
    }
}

/// Actions the app can take in response to events.
pub enum Action {
    None,
    Quit,
    RunCell(usize),
    ViewOutput,
    ViewError,
    ViewBuildError,
    ClearContext,
    Reload,
    Edit,
}

/// Process a key event and return the action.
pub fn handle_key(key: KeyEvent, app: &mut App, config: &AppConfig) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::None;
    }

    let kb = &config.keybindings;

    if kb.quit.matches(key.code, key.modifiers) {
        return Action::Quit;
    }
    if kb.clear_context.matches(key.code, key.modifiers) {
        return Action::ClearContext;
    }
    if kb.view_output.matches(key.code, key.modifiers) {
        return Action::ViewOutput;
    }
    if kb.view_error.matches(key.code, key.modifiers) {
        return Action::ViewError;
    }
    if kb.view_build_error.matches(key.code, key.modifiers) {
        return Action::ViewBuildError;
    }
    if kb.reload.matches(key.code, key.modifiers) {
        return Action::Reload;
    }
    if kb.edit.matches(key.code, key.modifiers) {
        return Action::Edit;
    }
    if kb.navigate_down.matches(key.code, key.modifiers) {
        app.select_next();
        return Action::None;
    }
    if kb.navigate_up.matches(key.code, key.modifiers) {
        app.select_previous();
        return Action::None;
    }
    if kb.run_cell.matches(key.code, key.modifiers)
        && let Some(idx) = app.selected_cell_index()
        && idx > 0
    {
        return Action::RunCell(idx);
    }

    Action::None
}
