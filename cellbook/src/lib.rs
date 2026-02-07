//! Cellbook - A computational notebook environment for Rust.
//!
//! This crate provides the user-facing API for cellbook projects.
//! The actual runtime lives in cargo-cellbook.
//!
//! # Serialization
//!
//! Values stored in the context are serialized with postcard. This means:
//! - Stored types must implement `Serialize`
//! - Loaded types must implement `DeserializeOwned`
//! - Values survive hot-reloads because they're stored as bytes, not as Rust types

pub mod config;
pub mod context;
pub mod errors;
mod macros;
pub mod registry;
pub mod test;

pub use cellbook_macros::{cell, cellbook};
pub use config::Config;
pub use context::CellContext;
pub use errors::{ContextError, Error, Result};
pub use futures;
pub use inventory;
pub use registry::CellInfo;
pub use serde;
