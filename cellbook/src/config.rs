//! Configuration for cellbook projects.

/// Configuration options for a cellbook.
///
/// ```ignore
/// cellbook!(Config::default().auto_reload(false));
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    /// Watch for file changes and rebuild automatically.
    pub auto_reload: bool,

    /// Debounce delay for file watcher in milliseconds.
    pub debounce_ms: u32,

    /// External command to view images.
    pub image_viewer: Option<String>,

    /// External command to view plots.
    pub plot_viewer: Option<String>,

    /// Show timing information for cell execution.
    pub show_timings: bool,

    /// Clear output between cell runs.
    pub clear_on_run: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_reload: true,
            debounce_ms: 500,
            image_viewer: None,
            plot_viewer: None,
            show_timings: false,
            clear_on_run: false,
        }
    }
}

impl Config {
    pub fn auto_reload(mut self, enabled: bool) -> Self {
        self.auto_reload = enabled;
        self
    }

    pub fn debounce_ms(mut self, ms: u32) -> Self {
        self.debounce_ms = ms;
        self
    }

    pub fn image_viewer(mut self, cmd: impl Into<String>) -> Self {
        self.image_viewer = Some(cmd.into());
        self
    }

    pub fn plot_viewer(mut self, cmd: impl Into<String>) -> Self {
        self.plot_viewer = Some(cmd.into());
        self
    }

    pub fn show_timings(mut self, enabled: bool) -> Self {
        self.show_timings = enabled;
        self
    }

    pub fn clear_on_run(mut self, enabled: bool) -> Self {
        self.clear_on_run = enabled;
        self
    }
}
