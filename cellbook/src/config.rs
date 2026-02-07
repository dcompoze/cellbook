//! Configuration for cellbook projects.

/// Configuration options for a cellbook.
///
/// Pass to the `cellbook!()` macro to customize behavior:
///
/// ```ignore
/// // Using struct literal
/// cellbook!(Config {
///     auto_reload: false,
///     ..Default::default()
/// });
///
/// // Using builder methods
/// cellbook!(Config::default()
///     .auto_reload(false)
///     .image_viewer("feh"));
///
/// // Using defaults
/// cellbook!();
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    /// Watch for file changes and rebuild automatically.
    /// Default: `true`
    pub auto_reload: bool,

    /// Debounce delay for file watcher in milliseconds.
    /// Default: `500`
    pub debounce_ms: u32,

    /// External command to view images (e.g., "feh", "open", "xdg-open").
    /// If `None`, uses platform default.
    pub image_viewer: Option<String>,

    /// External command to view plots.
    /// If `None`, uses `image_viewer` or platform default.
    pub plot_viewer: Option<String>,

    /// Show timing information for cell execution.
    /// Default: `false`
    pub show_timings: bool,

    /// Clear output between cell runs.
    /// Default: `false`
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
    /// Enable or disable auto-reload on file changes.
    pub fn auto_reload(mut self, enabled: bool) -> Self {
        self.auto_reload = enabled;
        self
    }

    /// Set the debounce delay for file watching.
    pub fn debounce_ms(mut self, ms: u32) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Set the external image viewer command.
    pub fn image_viewer(mut self, cmd: impl Into<String>) -> Self {
        self.image_viewer = Some(cmd.into());
        self
    }

    /// Set the external plot viewer command.
    pub fn plot_viewer(mut self, cmd: impl Into<String>) -> Self {
        self.plot_viewer = Some(cmd.into());
        self
    }

    /// Enable or disable timing display for cell execution.
    pub fn show_timings(mut self, enabled: bool) -> Self {
        self.show_timings = enabled;
        self
    }

    /// Enable or disable clearing output between runs.
    pub fn clear_on_run(mut self, enabled: bool) -> Self {
        self.clear_on_run = enabled;
        self
    }
}
