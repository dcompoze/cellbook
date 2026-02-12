//! Cellbook core library.
//!
//! Provides the user-facing API for cellbook projects.
//! The runtime lives in `cargo-cellbook`.
//!
//! Values in the context store are serialized with postcard.
//! Stored types must implement `Serialize` and loaded types must implement `DeserializeOwned`.

pub mod context;
pub mod errors;
pub mod image;
mod macros;
pub mod registry;
pub mod test;

pub use cellbook_macros::{StoreSchema, cell, init};
pub use context::CellContext;
pub use errors::{ContextError, Error, Result};
pub use image::{open_image, open_image_bytes};
pub use registry::CellInfo;
pub use {futures, inventory, serde};

/// Opt-in schema version metadata for versioned shared-store operations.
///
/// Use `#[derive(StoreSchema)]` with `#[store_schema(version = N)]` on your type.
pub trait StoreSchema {
    const VERSION: u32;
}
