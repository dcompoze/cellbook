//! Dynamic library loading for cellbook.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use futures::future::BoxFuture;
use libloading::{Library, Symbol};

use crate::errors::{Error, Result};
use crate::store;

static RELOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct CellInfo {
    pub name: String,
    pub line: u32,
}

type CellFn = fn(
    store::StoreFn,
    store::LoadFn,
    store::RemoveFn,
    store::ListFn,
) -> BoxFuture<'static, std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>>;
type InitFn = fn() -> BoxFuture<'static, std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>>;

type GetCellsFn = unsafe extern "Rust" fn() -> Vec<(String, u32, CellFn)>;
type GetInitFn = unsafe extern "Rust" fn() -> (String, u32, InitFn);

type CellResult = std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>;

type LoadedSymbols = (Vec<CellInfo>, Vec<CellFn>, String, u32, InitFn);

/// SAFETY: The caller must ensure the library exports valid `__cellbook_get_cells`
/// and `__cellbook_get_init` symbols with the expected signatures.
unsafe fn load_symbols(library: &Library) -> Result<LoadedSymbols> {
    let get_cells: Symbol<GetCellsFn> = unsafe {
        library
            .get(b"__cellbook_get_cells")
            .map_err(|e| Error::LibLoad(format!("Symbol not found: {}", e)))?
    };
    let get_init: Symbol<GetInitFn> = unsafe {
        library
            .get(b"__cellbook_get_init")
            .map_err(|e| Error::LibLoad(format!("Symbol not found: {}", e)))?
    };

    let raw_cells = unsafe { get_cells() };
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

    let (init_name, init_line, init_fn) = unsafe { get_init() };
    Ok((sorted_cells, sorted_fns, init_name, init_line, init_fn))
}

pub struct LoadedLibrary {
    _library: Library,
    cells: Vec<CellInfo>,
    cell_fns: Vec<CellFn>,
    init_name: String,
    init_line: u32,
    init_fn: InitFn,
    lib_path: PathBuf,
    temp_paths: Vec<PathBuf>,
}

impl Drop for LoadedLibrary {
    fn drop(&mut self) {
        for path in &self.temp_paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl LoadedLibrary {
    pub fn load(lib_path: &Path) -> Result<Self> {
        // SAFETY: We trust the user's cellbook code to be safe (dylib).
        let library = unsafe { Library::new(lib_path) }
            .map_err(|e| Error::LibLoad(format!("Failed to load {}: {}", lib_path.display(), e)))?;

        let (cells, cell_fns, init_name, init_line, init_fn) = unsafe { load_symbols(&library) }?;

        Ok(LoadedLibrary {
            _library: library,
            cells,
            cell_fns,
            init_name,
            init_line,
            init_fn,
            lib_path: lib_path.to_path_buf(),
            temp_paths: Vec::new(),
        })
    }

    pub fn reload(&mut self) -> Result<()> {
        // Copy to a unique path to bypass dlopen caching.
        let counter = RELOAD_COUNTER.fetch_add(1, Ordering::SeqCst);
        let unique_path = PathBuf::from(format!("{}.reload.{}", self.lib_path.display(), counter));

        std::fs::copy(&self.lib_path, &unique_path)
            .map_err(|e| Error::LibLoad(format!("Failed to copy library for reload: {}", e)))?;

        let library = unsafe { Library::new(&unique_path) }.map_err(|e| {
            let _ = std::fs::remove_file(&unique_path);
            Error::LibLoad(format!("Failed to load {}: {}", unique_path.display(), e))
        })?;

        let (cells, cell_fns, init_name, init_line, init_fn) = unsafe { load_symbols(&library) }?;

        self.temp_paths.push(unique_path);
        self._library = library;
        self.cells = cells;
        self.cell_fns = cell_fns;
        self.init_name = init_name;
        self.init_line = init_line;
        self.init_fn = init_fn;

        Ok(())
    }

    pub fn cells(&self) -> &[CellInfo] {
        &self.cells
    }

    /// Create a future for running a cell without awaiting it.
    pub fn cell_future(&self, name: &str) -> Result<BoxFuture<'static, CellResult>> {
        let idx = self
            .cells
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| Error::LibLoad(format!("Cell '{}' not found", name)))?;

        let cell_fn = self.cell_fns[idx];
        Ok(cell_fn(
            store::get_store_fn(),
            store::get_load_fn(),
            store::get_remove_fn(),
            store::get_list_fn(),
        ))
    }

    /// Create a future for running the init function without awaiting it.
    pub fn init_future(&self) -> BoxFuture<'static, CellResult> {
        (self.init_fn)()
    }

    pub fn init_name(&self) -> &str {
        &self.init_name
    }

    pub fn init_line(&self) -> u32 {
        self.init_line
    }
}

pub fn find_dylib_path() -> Result<PathBuf> {
    let cargo_toml = Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(Error::NoCargoToml);
    }

    let content = std::fs::read_to_string(cargo_toml)?;
    let name = extract_package_name(&content)?;
    let lib_name = name.replace('-', "_");

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

    let local_path = Path::new("target/debug").join(&lib_filename);
    if local_path.exists() {
        return Ok(local_path);
    }

    // Check for workspace root.
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
            return Ok(workspace_path);
        }
        current = parent.to_path_buf();
    }

    Ok(local_path)
}

fn extract_package_name(cargo_toml: &str) -> Result<String> {
    let parsed: toml::Value =
        toml::from_str(cargo_toml).map_err(|e| Error::LibLoad(format!("Invalid Cargo.toml: {}", e)))?;

    parsed
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
        .ok_or_else(|| Error::LibLoad("Could not find package name in Cargo.toml".to_string()))
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
