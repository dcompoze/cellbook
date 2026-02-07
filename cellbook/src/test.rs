//! Test utilities for cellbook.
//!
//! Provides an in-memory store for testing cells without cargo-cellbook.
//! Each test should create its own `TestContext` with a unique prefix to
//! ensure isolation when tests run in parallel or sequentially on the same thread.
//!
//! # Example
//!
//! ```ignore
//! #[cfg(test)]
//! mod tests {
//!     use super::*;
//!     use cellbook::test::TestContext;
//!
//!     #[tokio::test]
//!     async fn test_my_cell() {
//!         let ctx = TestContext::default();
//!         my_cell(&ctx).await.unwrap();
//!         // Store is automatically cleaned up when ctx is dropped
//!     }
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::CellContext;

type StoredValue = (Vec<u8>, String);

/// Global test store shared across all tests.
static TEST_STORE: LazyLock<Mutex<HashMap<String, StoredValue>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Counter for auto-generating unique prefixes.
static PREFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// Current prefix for this thread. Set by TestContext, read by store/load functions.
    static CURRENT_PREFIX: RefCell<String> = const { RefCell::new(String::new()) };
}

fn store(key: &str, bytes: Vec<u8>, type_name: &str) {
    let prefixed_key = CURRENT_PREFIX.with(|p| format!("{}:{}", p.borrow(), key));
    TEST_STORE
        .lock()
        .insert(prefixed_key, (bytes, type_name.to_string()));
}

fn load(key: &str) -> Option<(Vec<u8>, String)> {
    let prefixed_key = CURRENT_PREFIX.with(|p| format!("{}:{}", p.borrow(), key));
    TEST_STORE.lock().get(&prefixed_key).cloned()
}

fn remove(key: &str) -> Option<(Vec<u8>, String)> {
    let prefixed_key = CURRENT_PREFIX.with(|p| format!("{}:{}", p.borrow(), key));
    TEST_STORE.lock().remove(&prefixed_key)
}

fn list() -> Vec<(String, String)> {
    let prefix_with_sep = CURRENT_PREFIX.with(|p| format!("{}:", p.borrow()));
    TEST_STORE
        .lock()
        .iter()
        .filter_map(|(k, (_, t))| {
            k.strip_prefix(&prefix_with_sep)
                .map(|stripped| (stripped.to_string(), t.clone()))
        })
        .collect()
}

/// A test context that provides isolated storage for a single test.
///
/// When created, sets a prefix that is automatically prepended to all keys.
/// When dropped, cleans up all keys with that prefix from the global store.
///
/// # Example
///
/// ```ignore
/// #[tokio::test]
/// async fn test_load_data() {
///     let ctx = TestContext::default();
///     load_data(&ctx).await.unwrap();
///
///     // Can also access the underlying CellContext directly
///     let data: Vec<f64> = ctx.load("data").unwrap();
///     assert_eq!(data.len(), 5);
/// }
/// ```
pub struct TestContext {
    prefix: String,
    context: CellContext,
}

impl TestContext {
    /// Create a new test context with the given prefix.
    ///
    /// The prefix should be unique per test to ensure isolation.
    /// A common pattern is to use the test function name.
    pub fn new(prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();
        CURRENT_PREFIX.with(|p| *p.borrow_mut() = prefix.clone());
        Self {
            prefix,
            context: CellContext::new(store, load, remove, list),
        }
    }
}

impl Default for TestContext {
    /// Create a new test context with an auto-generated unique prefix.
    fn default() -> Self {
        let n = PREFIX_COUNTER.fetch_add(1, Ordering::SeqCst);
        Self::new(format!("_test_{n}"))
    }
}

impl std::ops::Deref for TestContext {
    type Target = CellContext;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // Clear only keys with this prefix
        let prefix_with_sep = format!("{}:", self.prefix);
        TEST_STORE.lock().retain(|k, _| !k.starts_with(&prefix_with_sep));

        // Reset the thread-local prefix
        CURRENT_PREFIX.with(|p| p.borrow_mut().clear());
    }
}
