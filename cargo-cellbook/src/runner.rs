//! TUI events shared between watcher and TUI.

/// Events sent from the watcher to the TUI.
pub enum TuiEvent {
    Reloaded,
    BuildStarted,
    BuildCompleted(Option<String>),
}
