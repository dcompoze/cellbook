//! TUI application state.

#![allow(unused)]

use std::collections::HashMap;
use std::time::Duration;

use ratatui::widgets::ListState;

/// Execution status for a cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum CellStatus {
    #[default]
    Pending,
    Running,
    Success,
    Error(String),
}

/// Build and reload status.
#[derive(Clone, Debug, Default)]
pub enum BuildStatus {
    #[default]
    Idle,
    Building,
    Reloading,
    BuildError(String),
}

/// Captured output from a cell execution.
#[derive(Clone, Debug, Default)]
pub struct CellOutput {
    pub stdout: String,
    pub duration: Duration,
}

/// Main application state.
pub struct App {
    /// Cell names.
    pub cells: Vec<String>,

    /// Execution status for each cell.
    pub cell_statuses: Vec<CellStatus>,

    /// Execution count for each cell.
    pub cell_counts: HashMap<String, u32>,

    /// List selection state.
    pub list_state: ListState,

    /// Current build status.
    pub build_status: BuildStatus,

    /// Captured output for each cell.
    pub cell_outputs: HashMap<String, CellOutput>,

    /// Context store items.
    pub context_items: Vec<(String, String)>,

    /// Whether a cell is currently executing.
    pub executing: bool,

    pub show_timings: bool,
}

impl App {
    pub fn new(cells: Vec<String>, show_timings: bool) -> Self {
        let cell_count = cells.len();
        let mut list_state = ListState::default();
        if cell_count > 0 {
            list_state.select(Some(0));
        }

        Self {
            cells,
            cell_statuses: vec![CellStatus::Pending; cell_count],
            cell_counts: HashMap::new(),
            list_state,
            build_status: BuildStatus::Idle,
            cell_outputs: HashMap::new(),
            context_items: Vec::new(),
            executing: false,
            show_timings,
        }
    }

    pub fn get_count(&self, cell_name: &str) -> u32 {
        self.cell_counts.get(cell_name).copied().unwrap_or(0)
    }

    pub fn increment_count(&mut self, cell_name: &str) {
        let count = self.cell_counts.entry(cell_name.to_string()).or_insert(0);
        *count += 1;
    }

    pub fn selected_cell_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn selected_cell_name(&self) -> Option<&str> {
        self.list_state
            .selected()
            .and_then(|i| self.cells.get(i).map(|s| s.as_str()))
    }

    pub fn select_next(&mut self) {
        if self.cells.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.cells.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn select_previous(&mut self) {
        if self.cells.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.cells.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn store_output(&mut self, cell_name: &str, output: CellOutput) {
        if output.stdout.is_empty() {
            self.cell_outputs.remove(cell_name);
        } else {
            self.cell_outputs.insert(cell_name.to_string(), output);
        }
    }

    pub fn get_output(&self, cell_name: &str) -> Option<&CellOutput> {
        self.cell_outputs.get(cell_name)
    }

    pub fn has_output(&self, cell_name: &str) -> bool {
        self.cell_outputs.contains_key(cell_name)
    }

    pub fn get_error(&self, idx: usize) -> Option<&str> {
        match self.cell_statuses.get(idx) {
            Some(CellStatus::Error(msg)) => Some(msg.as_str()),
            _ => None,
        }
    }

    pub fn refresh_cells(&mut self, cells: Vec<String>) {
        let cell_count = cells.len();
        self.cells = cells;
        self.cell_statuses = vec![CellStatus::Pending; cell_count];
        self.cell_counts.clear();

        // Preserve selection if valid.
        if let Some(i) = self.list_state.selected() {
            if i >= cell_count && cell_count > 0 {
                self.list_state.select(Some(cell_count - 1));
            } else if cell_count == 0 {
                self.list_state.select(None);
            }
        } else if cell_count > 0 {
            self.list_state.select(Some(0));
        }
    }

    pub fn refresh_context(&mut self, items: Vec<(String, String)>) {
        self.context_items = items;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{App, CellOutput};

    #[test]
    fn empty_output_is_not_marked_as_output() {
        let mut app = App::new(vec!["init".to_string()], false);
        app.store_output(
            "init",
            CellOutput {
                stdout: String::new(),
                duration: Duration::from_millis(1),
            },
        );
        assert!(!app.has_output("init"));
    }

    #[test]
    fn non_empty_output_is_marked_as_output() {
        let mut app = App::new(vec!["init".to_string()], false);
        app.store_output(
            "init",
            CellOutput {
                stdout: "hello".to_string(),
                duration: Duration::from_millis(1),
            },
        );
        assert!(app.has_output("init"));
    }
}
