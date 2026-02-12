//! Context handle for cells to access the host's store.
//!
//! Values are serialized with postcard, allowing them to survive hot-reloads.

use std::any::type_name;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::StoreSchema;
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

    /// Store a versioned value with the given key.
    pub fn store_versioned<T: Serialize + StoreSchema>(&self, key: &str, value: &T) -> Result<()> {
        self.store_versioned_with(key, value, T::VERSION)
    }

    /// Store a value with an explicit schema version.
    pub fn store_versioned_with<T: Serialize>(&self, key: &str, value: &T, version: u32) -> Result<()> {
        let bytes = postcard::to_stdvec(value).map_err(|e| ContextError::Serialization {
            key: key.to_string(),
            message: e.to_string(),
        })?;
        let tagged_type_name = format!("{}#v{}", type_name::<T>(), version);
        (self.store_fn)(key, bytes, &tagged_type_name);
        Ok(())
    }

    /// Load a value by key.
    pub fn load<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let (bytes, stored_type_name) =
            (self.load_fn)(key).ok_or_else(|| ContextError::NotFound(key.to_string()))?;
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
        let (bytes, stored_type_name) =
            (self.load_fn)(key).ok_or_else(|| ContextError::NotFound(key.to_string()))?;
        let requested_type_name = type_name::<T>();
        if stored_type_name != requested_type_name {
            return Err(ContextError::TypeMismatch {
                key: key.to_string(),
                expected: requested_type_name.to_string(),
                found: stored_type_name,
            }
            .into());
        }

        let value = postcard::from_bytes(&bytes).map_err(|e| ContextError::Deserialization {
            key: key.to_string(),
            message: e.to_string(),
        })?;

        let _ = (self.remove_fn)(key);
        Ok(value)
    }

    /// Load a versioned value by key.
    pub fn load_versioned<T: DeserializeOwned + StoreSchema>(&self, key: &str) -> Result<T> {
        self.load_versioned_with(key, T::VERSION)
    }

    /// Load a value by key with an explicit expected schema version.
    pub fn load_versioned_with<T: DeserializeOwned>(&self, key: &str, version: u32) -> Result<T> {
        let (bytes, stored_type_name) =
            (self.load_fn)(key).ok_or_else(|| ContextError::NotFound(key.to_string()))?;
        Self::validate_versioned_type(key, &stored_type_name, type_name::<T>(), version)?;

        postcard::from_bytes(&bytes).map_err(|e| {
            ContextError::Deserialization {
                key: key.to_string(),
                message: e.to_string(),
            }
            .into()
        })
    }

    /// Load and remove a versioned value in one operation.
    pub fn consume_versioned<T: DeserializeOwned + StoreSchema>(&self, key: &str) -> Result<T> {
        self.consume_versioned_with(key, T::VERSION)
    }

    /// Load and remove a value with an explicit expected schema version.
    pub fn consume_versioned_with<T: DeserializeOwned>(&self, key: &str, version: u32) -> Result<T> {
        let (bytes, stored_type_name) =
            (self.load_fn)(key).ok_or_else(|| ContextError::NotFound(key.to_string()))?;
        Self::validate_versioned_type(key, &stored_type_name, type_name::<T>(), version)?;

        let value = postcard::from_bytes(&bytes).map_err(|e| ContextError::Deserialization {
            key: key.to_string(),
            message: e.to_string(),
        })?;
        let _ = (self.remove_fn)(key);
        Ok(value)
    }

    /// List all keys and their type names.
    pub fn list(&self) -> Vec<(String, String)> {
        (self.list_fn)()
    }

    fn validate_versioned_type(
        key: &str,
        stored_type_name: &str,
        expected_type_name: &str,
        expected_version: u32,
    ) -> Result<()> {
        match Self::split_versioned_type_name(stored_type_name) {
            Some((stored_type_name_only, stored_version)) => {
                if stored_type_name_only != expected_type_name {
                    return Err(ContextError::TypeMismatch {
                        key: key.to_string(),
                        expected: expected_type_name.to_string(),
                        found: stored_type_name_only.to_string(),
                    }
                    .into());
                }
                if stored_version != expected_version {
                    return Err(ContextError::SchemaVersionMismatch {
                        key: key.to_string(),
                        expected: expected_version,
                        found: stored_version,
                    }
                    .into());
                }
                Ok(())
            }
            None => {
                if stored_type_name == expected_type_name {
                    return Err(ContextError::SchemaVersionMismatch {
                        key: key.to_string(),
                        expected: expected_version,
                        found: 0,
                    }
                    .into());
                }
                Err(ContextError::TypeMismatch {
                    key: key.to_string(),
                    expected: expected_type_name.to_string(),
                    found: stored_type_name.to_string(),
                }
                .into())
            }
        }
    }

    fn split_versioned_type_name(type_name_with_version: &str) -> Option<(&str, u32)> {
        let (type_name, version_part) = type_name_with_version.rsplit_once("#v")?;
        let version = version_part.parse().ok()?;
        Some((type_name, version))
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
    use serde::{Deserialize, Serialize};

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
        let Error::Context(ContextError::TypeMismatch { key, expected, found }) = err else {
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

        let err = ctx.consume::<Vec<u16>>("data").expect_err("consume should fail");
        let Error::Context(ContextError::TypeMismatch { key, expected, found }) = err else {
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

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct VersionedData {
        value: u32,
    }

    impl crate::StoreSchema for VersionedData {
        const VERSION: u32 = 1;
    }

    #[test]
    fn load_versioned_round_trip() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = VersionedData { value: 42 };
        ctx.store_versioned("versioned_data", &value)
            .expect("store_versioned should succeed");

        let loaded: VersionedData = ctx
            .load_versioned("versioned_data")
            .expect("load_versioned should succeed");
        assert_eq!(loaded, value);
    }

    #[test]
    fn load_versioned_rejects_schema_mismatch() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = VersionedData { value: 7 };
        let bytes = postcard::to_stdvec(&value).expect("serialization should succeed");
        let tagged_type_name = format!("{}#v99", std::any::type_name::<VersionedData>());
        store("versioned_data", bytes, &tagged_type_name);

        let err = ctx
            .load_versioned::<VersionedData>("versioned_data")
            .expect_err("load_versioned should fail");
        let Error::Context(ContextError::SchemaVersionMismatch { key, expected, found }) = err else {
            panic!("expected schema version mismatch error");
        };

        assert_eq!(key, "versioned_data");
        assert_eq!(expected, 1);
        assert_eq!(found, 99);
    }

    #[test]
    fn consume_versioned_rejects_schema_mismatch_without_removal() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = VersionedData { value: 9 };
        let bytes = postcard::to_stdvec(&value).expect("serialization should succeed");
        let tagged_type_name = format!("{}#v3", std::any::type_name::<VersionedData>());
        store("versioned_data", bytes, &tagged_type_name);

        let err = ctx
            .consume_versioned::<VersionedData>("versioned_data")
            .expect_err("consume_versioned should fail");
        let Error::Context(ContextError::SchemaVersionMismatch { key, expected, found }) = err else {
            panic!("expected schema version mismatch error");
        };

        assert_eq!(key, "versioned_data");
        assert_eq!(expected, 1);
        assert_eq!(found, 3);

        assert!(
            load("versioned_data").is_some(),
            "value should still be present after failed consume_versioned"
        );
    }

    #[test]
    fn load_versioned_with_round_trip_without_store_schema_trait() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = vec![10u8, 20, 30];
        ctx.store_versioned_with("bytes", &value, 5)
            .expect("store_versioned_with should succeed");

        let loaded: Vec<u8> = ctx
            .load_versioned_with("bytes", 5)
            .expect("load_versioned_with should succeed");
        assert_eq!(loaded, value);
    }

    #[test]
    fn load_versioned_with_rejects_schema_mismatch() {
        let ctx = CellContext::new(store, load, remove, list);
        let value = vec![10u8, 20, 30];
        ctx.store_versioned_with("bytes", &value, 5)
            .expect("store_versioned_with should succeed");

        let err = ctx
            .load_versioned_with::<Vec<u8>>("bytes", 6)
            .expect_err("load_versioned_with should fail");
        let Error::Context(ContextError::SchemaVersionMismatch { key, expected, found }) = err else {
            panic!("expected schema version mismatch error");
        };

        assert_eq!(key, "bytes");
        assert_eq!(expected, 6);
        assert_eq!(found, 5);
    }
}
