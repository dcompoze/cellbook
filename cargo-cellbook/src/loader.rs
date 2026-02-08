//! Dynamic library loading for cellbook.
//!
//! Loads user's compiled dylib and discovers cells via __cellbook_get_cells().

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use futures::future::BoxFuture;
use libloading::{Library, Symbol};

use crate::errors::{Error, Result};
use crate::store;

/// Counter for generating unique library paths on reload
static RELOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Information about a registered cell
#[derive(Clone)]
pub struct CellInfo {
    pub name: String,
    pub line: u32,
}

/// Configuration for a cellbook project.
///
/// This struct must match the layout of `cellbook::Config` exactly.
#[derive(Clone, Debug)]
#[allow(unused)]
pub struct Config {
    pub auto_reload: bool,
    pub debounce_ms: u32,
    pub image_viewer: Option<String>,
    pub plot_viewer: Option<String>,
    pub show_timings: bool,
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

/// Cell function type - receives context functions and returns a future
type CellFn = fn(
    store::StoreFn,
    store::LoadFn,
    store::RemoveFn,
    store::ListFn,
) -> BoxFuture<'static, std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>>;

/// Type returned by __cellbook_get_cells
type GetCellsFn = unsafe extern "Rust" fn() -> Vec<(String, u32, CellFn)>;

/// Type returned by __cellbook_get_config
type GetConfigFn = unsafe extern "Rust" fn() -> Config;

/// A loaded library with its cells
pub struct LoadedLibrary {
    _library: Library,
    /// Old libraries kept alive (no longer needed for vtables with serialization, but kept for safety)
    _old_libraries: Vec<Library>,
    cells: Vec<CellInfo>,
    cell_fns: Vec<CellFn>,
    /// Path to the original library (source of truth after rebuilds)
    lib_path: PathBuf,
    /// Path to the currently loaded copy (may be a temp file)
    loaded_path: PathBuf,
    /// Temporary paths to clean up on drop
    temp_paths: Vec<PathBuf>,
    config: Config,
}

impl Drop for LoadedLibrary {
    fn drop(&mut self) {
        // Clean up temporary library copies
        for path in &self.temp_paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl LoadedLibrary {
    /// Load a library from the given path
    pub fn load(lib_path: &Path) -> Result<Self> {
        // SAFETY: We trust the user's cellbook dylib
        let library = unsafe { Library::new(lib_path) }
            .map_err(|e| Error::LibLoad(format!("Failed to load {}: {}", lib_path.display(), e)))?;

        let (cells, cell_fns, config) = unsafe {
            let get_cells: Symbol<GetCellsFn> = library
                .get(b"__cellbook_get_cells")
                .map_err(|e| Error::LibLoad(format!("Symbol not found: {}", e)))?;

            let raw_cells = get_cells();
            let mut cells = Vec::new();
            let mut cell_fns = Vec::new();

            for (name, line, func) in raw_cells {
                cells.push(CellInfo { name, line });
                cell_fns.push(func);
            }

            // Sort by line number (if all are 0, preserve registration order)
            let mut indices: Vec<usize> = (0..cells.len()).collect();
            indices.sort_by_key(|&i| cells[i].line);

            let sorted_cells: Vec<_> = indices.iter().map(|&i| cells[i].clone()).collect();
            let sorted_fns: Vec<_> = indices.iter().map(|&i| cell_fns[i]).collect();

            // Load config (optional, for backwards compatibility)
            let config = match library.get::<GetConfigFn>(b"__cellbook_get_config") {
                Ok(get_config) => get_config(),
                Err(_) => Config::default(),
            };

            (sorted_cells, sorted_fns, config)
        };

        Ok(LoadedLibrary {
            _library: library,
            _old_libraries: Vec::new(),
            cells,
            cell_fns,
            lib_path: lib_path.to_path_buf(),
            loaded_path: lib_path.to_path_buf(),
            temp_paths: Vec::new(),
            config,
        })
    }

    /// Reload the library
    pub fn reload(&mut self) -> Result<()> {
        // Copy library to a unique path to bypass dlopen's caching
        // dlopen caches by path, so loading the same path returns the old library
        let counter = RELOAD_COUNTER.fetch_add(1, Ordering::SeqCst);
        let unique_path = PathBuf::from(format!("{}.reload.{}", self.lib_path.display(), counter));

        std::fs::copy(&self.lib_path, &unique_path).map_err(|e| {
            Error::LibLoad(format!(
                "Failed to copy library for reload: {}",
                e
            ))
        })?;

        // Load the new library from the unique path
        let library = unsafe { Library::new(&unique_path) }.map_err(|e| {
            // Clean up on failure
            let _ = std::fs::remove_file(&unique_path);
            Error::LibLoad(format!("Failed to load {}: {}", unique_path.display(), e))
        })?;

        let (cells, cell_fns, config) = unsafe {
            let get_cells: Symbol<GetCellsFn> = library
                .get(b"__cellbook_get_cells")
                .map_err(|e| Error::LibLoad(format!("Symbol not found: {}", e)))?;

            let raw_cells = get_cells();
            let mut cells = Vec::new();
            let mut cell_fns = Vec::new();

            for (name, line, func) in raw_cells {
                cells.push(CellInfo { name, line });
                cell_fns.push(func);
            }

            let mut indices: Vec<usize> = (0..cells.len()).collect();
            indices.sort_by_key(|&i| cells[i].line);

            let sorted_cells: Vec<_> = indices.iter().map(|&i| cells[i].clone()).collect();
            let sorted_fns: Vec<_> = indices.iter().map(|&i| cell_fns[i]).collect();

            // Load config (optional, for backwards compatibility)
            let config = match library.get::<GetConfigFn>(b"__cellbook_get_config") {
                Ok(get_config) => get_config(),
                Err(_) => Config::default(),
            };

            (sorted_cells, sorted_fns, config)
        };

        // Track the temp path for cleanup
        self.temp_paths.push(unique_path.clone());

        // Replace old library
        self._library = library;
        self.loaded_path = unique_path;
        self.cells = cells;
        self.cell_fns = cell_fns;
        self.config = config;

        Ok(())
    }

    /// Get the list of cells
    pub fn cells(&self) -> &[CellInfo] {
        &self.cells
    }

    /// Run a cell by name
    pub async fn run_cell(&self, name: &str) -> Result<()> {
        let idx = self
            .cells
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| Error::LibLoad(format!("Cell '{}' not found", name)))?;

        let cell_fn = self.cell_fns[idx];
        let future = cell_fn(
            store::get_store_fn(),
            store::get_load_fn(),
            store::get_remove_fn(),
            store::get_list_fn(),
        );

        future.await.map_err(|e| Error::LibLoad(e.to_string()))
    }

    /// Get the library path
    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.lib_path
    }

    /// Get the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Find the dylib path for the current project
pub fn find_dylib_path() -> Result<PathBuf> {
    let cargo_toml = Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(Error::NoCargoToml);
    }

    // Read Cargo.toml to get package name
    let content = std::fs::read_to_string(cargo_toml)?;
    let name = extract_package_name(&content)?;

    // Convert package name to lib name (replace - with _)
    let lib_name = name.replace('-', "_");

    // Determine the dylib extension based on platform
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };

    let lib_filename = if cfg!(target_os = "windows") {
        format!("{}.{}", lib_name, ext)
    } else {
        format!("lib{}.{}", lib_name, ext)
    };

    // Check local target directory first
    let local_path = Path::new("target/debug").join(&lib_filename);
    if local_path.exists() {
        return Ok(local_path);
    }

    // Check for workspace root by looking for parent Cargo.toml with [workspace]
    let mut current = std::env::current_dir()?;
    loop {
        let parent = current.parent();
        if parent.is_none() {
            break;
        }
        let parent = parent.unwrap();
        let parent_cargo = parent.join("Cargo.toml");
        if parent_cargo.exists()
            && let Ok(content) = std::fs::read_to_string(&parent_cargo)
            && content.contains("[workspace]")
        {
            let workspace_path = parent.join("target/debug").join(&lib_filename);
            // Return workspace path whether it exists or not (will be created by build)
            return Ok(workspace_path);
        }
        current = parent.to_path_buf();
    }

    // Default to local path (will be created by build)
    Ok(local_path)
}

fn extract_package_name(cargo_toml: &str) -> Result<String> {
    // Simple TOML parsing - look for name = "..." in [package]
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let line = line.trim();
        if line == "[package]" {
            in_package = true;
        } else if line.starts_with('[') {
            in_package = false;
        } else if in_package
            && line.starts_with("name")
            && let Some(value) = line.split('=').nth(1)
        {
            let name = value.trim().trim_matches('"').trim_matches('\'');
            return Ok(name.to_string());
        }
    }
    Err(Error::LibLoad(
        "Could not find package name in Cargo.toml".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_package_name() {
        let toml = r#"
[package]
name = "my-project"
version = "0.1.0"
"#;
        assert_eq!(extract_package_name(toml).unwrap(), "my-project");
    }
}
