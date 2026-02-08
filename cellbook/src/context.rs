//! Context handle for cells to access the host's store.
//!
//! Values are serialized with postcard, allowing them to survive hot-reloads.

use std::any::type_name;

use serde::{de::DeserializeOwned, Serialize};

use crate::errors::{ContextError, Result};

pub type StoreFn = fn(&str, Vec<u8>, &str);
pub type LoadFn = fn(&str) -> Option<(Vec<u8>, String)>;
pub type RemoveFn = fn(&str) -> Option<(Vec<u8>, String)>;
pub type ListFn = fn() -> Vec<(String, String)>;

/// Handle to the host's context store.
///
/// Passed to each cell to provide typed access to store/load operations.
/// Types must implement `Serialize` for storing and `DeserializeOwned` for loading.
#[derive(Clone, Copy)]
pub struct CellContext {
    store_fn: StoreFn,
    load_fn: LoadFn,
    remove_fn: RemoveFn,
    list_fn: ListFn,
}

impl CellContext {
    pub fn new(store_fn: StoreFn, load_fn: LoadFn, remove_fn: RemoveFn, list_fn: ListFn) -> Self {
        Self {
            store_fn,
            load_fn,
            remove_fn,
            list_fn,
        }
    }

    /// Store a value with the given key.
    pub fn store<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = postcard::to_stdvec(value).map_err(|e| ContextError::Serialization {
            key: key.to_string(),
            message: e.to_string(),
        })?;
        (self.store_fn)(key, bytes, type_name::<T>());
        Ok(())
    }

    /// Load a value by key.
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

    /// Remove a value by key.
    /// Returns true if the key existed.
    pub fn remove(&self, key: &str) -> bool {
        (self.remove_fn)(key).is_some()
    }

    /// Load and remove a value in one operation.
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

    /// List all keys and their type names.
    pub fn list(&self) -> Vec<(String, String)> {
        (self.list_fn)()
    }
}

// SAFETY: CellContext only contains function pointers which are Send + Sync.
unsafe impl Send for CellContext {}
unsafe impl Sync for CellContext {}
