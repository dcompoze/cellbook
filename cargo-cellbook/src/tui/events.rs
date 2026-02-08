//! Event handling for the TUI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use tokio::sync::mpsc;

use crate::runner::TuiEvent;

use super::config::TuiConfig;
use super::state::App;

/// Unified event type for the TUI.
pub enum AppEvent {
    /// Terminal event from crossterm.
    Terminal(CrosstermEvent),

    /// Event from the file watcher.
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
}

impl EventHandler {
    pub fn new(tui_rx: mpsc::Receiver<TuiEvent>, tick_rate: Duration) -> Self {
        let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let thread_stop_flag = stop_flag.clone();

        // Spawn thread to poll crossterm events.
        std::thread::spawn(move || {
            loop {
                if thread_stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                if event::poll(Duration::from_millis(50)).unwrap_or(false)
                    && let Ok(evt) = event::read()
                    && terminal_tx.send(evt).is_err()
                {
                    break;
                }
            }
        });

        Self {
            terminal_rx,
            tui_rx,
            tick_rate,
            stop_flag,
        }
    }

    /// Stop the event polling thread.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // Wait for the polling thread to finish its current cycle.
        std::thread::sleep(Duration::from_millis(100));
    }

    /// Resume event polling with a new thread.
    pub fn resume(&mut self) {
        self.stop_flag.store(false, Ordering::Relaxed);

        let (terminal_tx, terminal_rx) = mpsc::unbounded_channel();
        self.terminal_rx = terminal_rx;

        let thread_stop_flag = self.stop_flag.clone();
        std::thread::spawn(move || {
            loop {
                if thread_stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                if event::poll(Duration::from_millis(50)).unwrap_or(false)
                    && let Ok(evt) = event::read()
                    && terminal_tx.send(evt).is_err()
                {
                    break;
                }
            }
        });
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
    ClearContext,
    Reload,
    Edit,
}

/// Process a key event and return the action.
pub fn handle_key(key: KeyEvent, app: &mut App, config: &TuiConfig) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::None;
    }

    let kb = &config.keybindings;

    if kb.quit.matches(key.code) {
        return Action::Quit;
    }
    if kb.clear_context.matches(key.code) {
        return Action::ClearContext;
    }
    if kb.view_output.matches(key.code) {
        return Action::ViewOutput;
    }
    if kb.reload.matches(key.code) {
        return Action::Reload;
    }
    if kb.edit.matches(key.code) {
        return Action::Edit;
    }
    if kb.navigate_down.matches(key.code) {
        app.select_next();
        return Action::None;
    }
    if kb.navigate_up.matches(key.code) {
        app.select_previous();
        return Action::None;
    }
    if kb.run_cell.matches(key.code)
        && let Some(idx) = app.selected_cell_index()
    {
        return Action::RunCell(idx);
    }

    Action::None
}
