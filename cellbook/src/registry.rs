use futures::future::BoxFuture;

use crate::Result;

pub struct CellInfo {
    pub name: &'static str,
    pub func: fn() -> BoxFuture<'static, Result<()>>,
    pub line: u32,
}

inventory::collect!(CellInfo);

/// Returns all registered cells, sorted by source line number.
pub fn cells() -> Vec<&'static CellInfo> {
    let mut cells: Vec<_> = inventory::iter::<CellInfo>.into_iter().collect();
    cells.sort_by_key(|c| c.line);
    cells
}

/// Find a cell by name.
pub fn get(name: &str) -> Option<&'static CellInfo> {
    cells().into_iter().find(|c| c.name == name)
}

/// Run a cell by name.
pub async fn run(name: &str) -> Result<()> {
    let cell = get(name).ok_or_else(|| {
        crate::Error::Context(crate::ContextError::NotFound(format!("cell '{}'", name)))
    })?;
    (cell.func)().await
}

/// Run all cells in registration order.
pub async fn run_all() -> Result<()> {
    for cell in cells() {
        (cell.func)().await?;
    }
    Ok(())
}
