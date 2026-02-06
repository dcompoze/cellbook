//! Async-safe context store for sharing data between cells.
//!
//! Uses `Arc<dyn Any>` internally to allow storing arbitrary types
//! and returning cheap `Arc<T>` handles without cloning the underlying data.

use std::any::{Any, type_name};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use parking_lot::Mutex;

use crate::errors::{ContextError, Result};

struct StoredValue {
    value: Arc<dyn Any + Send + Sync>,
    type_name: &'static str,
}

static STORE: LazyLock<Mutex<HashMap<&'static str, StoredValue>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Store a value in the context with the given key.
///
/// The value is wrapped in an `Arc` internally, so subsequent loads
/// are cheap (just an Arc clone / refcount increment).
pub fn store<T: Any + Send + Sync>(key: &'static str, value: T) {
    let mut store = STORE.lock();
    store.insert(
        key,
        StoredValue {
            value: Arc::new(value),
            type_name: type_name::<T>(),
        },
    );
}

/// Load a value from the context by key.
///
/// Returns an `Arc<T>` which can be used like a reference via `Deref`.
/// The lock is released immediately, making this safe for async contexts.
pub fn load<T: Any + Send + Sync>(key: &'static str) -> Result<Arc<T>> {
    let (arc_any, actual_type) = {
        let store = STORE.lock();
        let stored = store
            .get(key)
            .ok_or_else(|| ContextError::NotFound(key.to_string()))?;
        (Arc::clone(&stored.value), stored.type_name)
    };

    arc_any.downcast::<T>().map_err(|_| {
        ContextError::TypeMismatch {
            key: key.to_string(),
            expected: type_name::<T>(),
            actual: actual_type,
        }
        .into()
    })
}

/// List all keys and their type names currently in the context.
pub fn list() -> Vec<(&'static str, &'static str)> {
    let store = STORE.lock();
    store.iter().map(|(k, v)| (*k, v.type_name)).collect()
}

/// Clear all values from the context.
pub fn clear() {
    let mut store = STORE.lock();
    store.clear();
}

/// Remove a specific key from the context.
pub fn remove(key: &'static str) -> bool {
    let mut store = STORE.lock();
    store.remove(key).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_load() {
        store("test_store_and_load_value", 42i32);
        let loaded = load::<i32>("test_store_and_load_value").unwrap();
        assert_eq!(*loaded, 42);
    }

    #[test]
    fn test_type_mismatch() {
        store("test_type_mismatch_string", "hello".to_string());
        let result = load::<i32>("test_type_mismatch_string");
        assert!(result.is_err());
    }

    #[test]
    fn test_not_found() {
        let result = load::<i32>("test_not_found_nonexistent");
        assert!(result.is_err());
    }
}
