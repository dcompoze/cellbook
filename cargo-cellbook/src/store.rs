//! Context store for sharing data between cells.
//!
//! Values are stored as serialized bytes, which allows them to survive
//! hot-reloads where TypeIds change across recompilation.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

/// A stored value as serialized bytes
struct StoredValue {
    bytes: Vec<u8>,
    type_name: String,
}

static STORE: LazyLock<Mutex<HashMap<String, StoredValue>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Store a serialized value in the context.
pub fn store_value(key: &str, bytes: Vec<u8>, type_name: &str) {
    let mut store = STORE.lock();
    store.insert(
        key.to_string(),
        StoredValue {
            bytes,
            type_name: type_name.to_string(),
        },
    );
}

/// Load a serialized value from the context.
/// Returns the bytes and type name, or None if not found.
pub fn load_value(key: &str) -> Option<(Vec<u8>, String)> {
    let store = STORE.lock();
    store.get(key).map(|v| (v.bytes.clone(), v.type_name.clone()))
}

/// Remove a value from the context.
/// Returns the bytes and type name if the key existed.
pub fn remove_value(key: &str) -> Option<(Vec<u8>, String)> {
    let mut store = STORE.lock();
    store.remove(key).map(|v| (v.bytes, v.type_name))
}

/// List all keys and their type names in the context.
pub fn list() -> Vec<(String, String)> {
    let store = STORE.lock();
    store
        .iter()
        .map(|(k, v)| (k.clone(), v.type_name.clone()))
        .collect()
}

/// Clear all values from the context.
pub fn clear() {
    let mut store = STORE.lock();
    store.clear();
}

// FFI-compatible function pointers for CellContext

/// Store function pointer type for FFI
pub type StoreFn = fn(&str, Vec<u8>, &str);

/// Load function pointer type for FFI
pub type LoadFn = fn(&str) -> Option<(Vec<u8>, String)>;

/// Remove function pointer type for FFI
pub type RemoveFn = fn(&str) -> Option<(Vec<u8>, String)>;

/// List function pointer type for FFI
pub type ListFn = fn() -> Vec<(String, String)>;

/// Get the store function pointer for FFI
pub fn get_store_fn() -> StoreFn {
    store_value
}

/// Get the load function pointer for FFI
pub fn get_load_fn() -> LoadFn {
    load_value
}

/// Get the remove function pointer for FFI
pub fn get_remove_fn() -> RemoveFn {
    remove_value
}

/// Get the list function pointer for FFI
pub fn get_list_fn() -> ListFn {
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_load() {
        store_value("test_bytes", vec![1, 2, 3, 4], "test");
        let loaded = load_value("test_bytes").unwrap();
        assert_eq!(loaded.0, vec![1, 2, 3, 4]);
        assert_eq!(loaded.1, "test");
    }

    #[test]
    fn test_remove() {
        store_value("test_remove", vec![5, 6], "test");
        let removed = remove_value("test_remove").unwrap();
        assert_eq!(removed.0, vec![5, 6]);
        assert!(load_value("test_remove").is_none());
    }

    #[test]
    fn test_not_found() {
        let result = load_value("nonexistent_key");
        assert!(result.is_none());
    }
}
