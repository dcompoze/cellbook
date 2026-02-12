//! Context store for sharing data between cells.
//!
//! Values are stored as serialized bytes to survive hot-reloads.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;

struct StoredValue {
    bytes: Vec<u8>,
    type_name: String,
}

static STORE: LazyLock<Mutex<HashMap<String, StoredValue>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

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

pub fn load_value(key: &str) -> Option<(Vec<u8>, String)> {
    let store = STORE.lock();
    store.get(key).map(|v| (v.bytes.clone(), v.type_name.clone()))
}

pub fn remove_value(key: &str) -> Option<(Vec<u8>, String)> {
    let mut store = STORE.lock();
    store.remove(key).map(|v| (v.bytes, v.type_name))
}

pub fn list() -> Vec<(String, String)> {
    let store = STORE.lock();
    store
        .iter()
        .map(|(k, v)| (k.clone(), v.type_name.clone()))
        .collect()
}

pub fn clear() {
    let mut store = STORE.lock();
    store.clear();
}

pub type StoreFn = fn(&str, Vec<u8>, &str);
pub type LoadFn = fn(&str) -> Option<(Vec<u8>, String)>;
pub type RemoveFn = fn(&str) -> Option<(Vec<u8>, String)>;
pub type ListFn = fn() -> Vec<(String, String)>;

pub fn get_store_fn() -> StoreFn {
    store_value
}

pub fn get_load_fn() -> LoadFn {
    load_value
}

pub fn get_remove_fn() -> RemoveFn {
    remove_value
}

pub fn get_list_fn() -> ListFn {
    list
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_KEY_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_key(base: &str) -> String {
        let n = TEST_KEY_COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{base}_{n}")
    }

    #[test]
    fn test_store_and_load() {
        let key = unique_key("bytes");
        store_value(&key, vec![1, 2, 3, 4], "test");
        let loaded = load_value(&key).unwrap();
        assert_eq!(loaded.0, vec![1, 2, 3, 4]);
        assert_eq!(loaded.1, "test");
    }

    #[test]
    fn test_remove() {
        let key = unique_key("remove");
        store_value(&key, vec![5, 6], "test");
        let removed = remove_value(&key).unwrap();
        assert_eq!(removed.0, vec![5, 6]);
        assert!(load_value(&key).is_none());
    }

    #[test]
    fn test_not_found() {
        let key = unique_key("nonexistent");
        assert!(load_value(&key).is_none());
    }
}
