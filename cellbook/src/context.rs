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
        let (bytes, stored_type_name) = (self.load_fn)(key)
            .ok_or_else(|| ContextError::NotFound(key.to_string()))?;
        let requested_type_name = type_name::<T>();
        if stored_type_name != requested_type_name {
            return Err(ContextError::TypeMismatch {
                key: key.to_string(),
                expected: requested_type_name.to_string(),
                found: stored_type_name,
            }
            .into());
        }

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
        let (bytes, stored_type_name) = (self.load_fn)(key)
            .ok_or_else(|| ContextError::NotFound(key.to_string()))?;
        let requested_type_name = type_name::<T>();
        if stored_type_name != requested_type_name {
            return Err(ContextError::TypeMismatch {
                key: key.to_string(),
                expected: requested_type_name.to_string(),
                found: stored_type_name,
            }
            .into());
        }

        let value = postcard::from_bytes(&bytes).map_err(|e| {
            ContextError::Deserialization {
                key: key.to_string(),
                message: e.to_string(),
            }
        })?;

        let _ = (self.remove_fn)(key);
        Ok(value)
    }

    /// List all keys and their type names.
    pub fn list(&self) -> Vec<(String, String)> {
        (self.list_fn)()
    }
}

// SAFETY: CellContext only contains function pointers which are Send + Sync.
unsafe impl Send for CellContext {}
unsafe impl Sync for CellContext {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::LazyLock;

    use parking_lot::Mutex;

    use super::*;
    use crate::Error;

    type StoredValue = (Vec<u8>, String);

    static STORE: LazyLock<Mutex<HashMap<String, StoredValue>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    fn store(key: &str, bytes: Vec<u8>, type_name: &str) {
        STORE
            .lock()
            .insert(key.to_string(), (bytes, type_name.to_string()));
    }

    fn load(key: &str) -> Option<(Vec<u8>, String)> {
        STORE.lock().get(key).cloned()
    }

    fn remove(key: &str) -> Option<(Vec<u8>, String)> {
        STORE.lock().remove(key)
    }

    fn list() -> Vec<(String, String)> {
        STORE
            .lock()
            .iter()
            .map(|(k, (_, ty))| (k.clone(), ty.clone()))
            .collect()
    }

    #[test]
    fn load_rejects_type_mismatch() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = vec![1u8, 2, 3];
        ctx.store("data", &value).expect("store should succeed");

        let err = ctx.load::<Vec<u16>>("data").expect_err("load should fail");
        let Error::Context(ContextError::TypeMismatch {
            key,
            expected,
            found,
        }) = err
        else {
            panic!("expected type mismatch error");
        };

        assert_eq!(key, "data");
        assert_eq!(expected, std::any::type_name::<Vec<u16>>());
        assert_eq!(found, std::any::type_name::<Vec<u8>>());
    }

    #[test]
    fn consume_rejects_type_mismatch() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = vec![1u8, 2, 3];
        ctx.store("data", &value).expect("store should succeed");

        let err = ctx
            .consume::<Vec<u16>>("data")
            .expect_err("consume should fail");
        let Error::Context(ContextError::TypeMismatch {
            key,
            expected,
            found,
        }) = err
        else {
            panic!("expected type mismatch error");
        };

        assert_eq!(key, "data");
        assert_eq!(expected, std::any::type_name::<Vec<u16>>());
        assert_eq!(found, std::any::type_name::<Vec<u8>>());

        let still_present = ctx
            .load::<Vec<u8>>("data")
            .expect("value should still be present after failed consume");
        assert_eq!(still_present, value);
    }
}
