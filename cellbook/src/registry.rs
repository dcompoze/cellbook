//! Cell registry using inventory for automatic collection.

use futures::future::BoxFuture;

use crate::context::{ListFn, LoadFn, RemoveFn, StoreFn};

/// Function pointer type matching the cell wrapper signature
pub type CellFn = fn(StoreFn, LoadFn, RemoveFn, ListFn)
    -> BoxFuture<'static, std::result::Result<(), Box<dyn std::error::Error + Send + Sync>>>;

/// Information about a registered cell
pub struct CellInfo {
    pub name: &'static str,
    pub func: CellFn,
    pub line: u32,
}

inventory::collect!(CellInfo);

/// Returns all registered cells, sorted by source line number.
pub fn cells() -> Vec<&'static CellInfo> {
    let mut cells: Vec<_> = inventory::iter::<CellInfo>.into_iter().collect();
    cells.sort_by_key(|c| c.line);
    cells
}
