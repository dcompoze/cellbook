//! Test utilities for cellbook.
//!
//! Provides an in-memory store for testing cells without `cargo-cellbook`.
//!
//! ```ignore
//! #[tokio::test]
//! async fn test_my_cell() {
//!     let ctx = TestContext::default();
//!     my_cell(&ctx).await.unwrap();
//! }
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

use parking_lot::Mutex;

use crate::CellContext;

type StoredValue = (Vec<u8>, String);

static TEST_STORE: LazyLock<Mutex<HashMap<String, StoredValue>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static PREFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

thread_local! {
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

/// Test context providing isolated storage for a single test.
///
/// Keys are automatically prefixed for isolation.
/// Storage is cleaned up when the context is dropped.
///
/// ```ignore
/// let ctx = TestContext::default();
/// my_cell(&ctx).await.unwrap();
/// let data: Vec<f64> = ctx.load("data").unwrap();
/// ```
pub struct TestContext {
    prefix: String,
    context: CellContext,
}

impl TestContext {
    /// Create a test context with a custom prefix.
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
        let prefix_with_sep = format!("{}:", self.prefix);
        TEST_STORE.lock().retain(|k, _| !k.starts_with(&prefix_with_sep));
        CURRENT_PREFIX.with(|p| p.borrow_mut().clear());
    }
}
