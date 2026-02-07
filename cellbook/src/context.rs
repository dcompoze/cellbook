//! Context handle for cells to access the host's store.
//!
//! The CellContext is passed to each cell by the host (cargo-cellbook).
//! Values are serialized with postcard for storage, allowing them to
//! survive hot-reloads where TypeIds change.

use std::any::type_name;

use serde::{de::DeserializeOwned, Serialize};

use crate::errors::{ContextError, Result};

/// Function pointer type for storing values (serialized bytes)
pub type StoreFn = fn(&str, Vec<u8>, &str);

/// Function pointer type for loading values (returns serialized bytes)
pub type LoadFn = fn(&str) -> Option<(Vec<u8>, String)>;

/// Function pointer type for removing values (returns serialized bytes)
pub type RemoveFn = fn(&str) -> Option<(Vec<u8>, String)>;

/// Function pointer type for listing values
pub type ListFn = fn() -> Vec<(String, String)>;

/// Handle to the host's context store.
///
/// This is passed to each cell and provides typed access to store/load operations.
/// Values are serialized with postcard, so stored types must implement Serialize
/// and DeserializeOwned.
#[derive(Clone, Copy)]
pub struct CellContext {
    store_fn: StoreFn,
    load_fn: LoadFn,
    remove_fn: RemoveFn,
    list_fn: ListFn,
}

impl CellContext {
    /// Create a new CellContext with the given function pointers.
    pub fn new(store_fn: StoreFn, load_fn: LoadFn, remove_fn: RemoveFn, list_fn: ListFn) -> Self {
        Self {
            store_fn,
            load_fn,
            remove_fn,
            list_fn,
        }
    }

    /// Store a value in the context with the given key.
    ///
    /// The value is serialized with postcard.
    pub fn store<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = postcard::to_stdvec(value).map_err(|e| ContextError::Serialization {
            key: key.to_string(),
            message: e.to_string(),
        })?;
        (self.store_fn)(key, bytes, type_name::<T>());
        Ok(())
    }

    /// Load a value from the context by key.
    ///
    /// The value is deserialized with postcard.
    pub fn load<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let (bytes, _type_name) = (self.load_fn)(key)
            .ok_or_else(|| ContextError::NotFound(key.to_string()))?;

        postcard::from_bytes(&bytes).map_err(|e| {
            ContextError::Deserialization {
                key: key.to_string(),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// Remove a value from the context by key.
    ///
    /// Returns true if the key existed.
    pub fn remove(&self, key: &str) -> bool {
        (self.remove_fn)(key).is_some()
    }

    /// Load and remove a value from the context in one operation.
    ///
    /// This is useful when you want to transfer ownership of a value.
    pub fn consume<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let (bytes, _type_name) = (self.remove_fn)(key)
            .ok_or_else(|| ContextError::NotFound(key.to_string()))?;

        postcard::from_bytes(&bytes).map_err(|e| {
            ContextError::Deserialization {
                key: key.to_string(),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// List all keys and their type names in the context.
    pub fn list(&self) -> Vec<(String, String)> {
        (self.list_fn)()
    }
}

// SAFETY: CellContext only contains function pointers which are Send + Sync
unsafe impl Send for CellContext {}
unsafe impl Sync for CellContext {}
