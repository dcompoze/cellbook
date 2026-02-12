//! Versioned store example for cellbook.
//!
//! Demonstrates:
//! - derive-based schema versions via `StoreSchema`
//! - explicit version at call-site for foreign-like types
//! - version mismatch handling
//! - consume semantics (failed consume keeps data, successful consume removes it)

use anyhow::Result;
use cellbook::{Error as CellbookError, StoreSchema, cell, consumev, init, loadv, storev};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, StoreSchema)]
#[store_schema(version = 1)]
struct FeatureFlags {
    enable_plots: bool,
    sample_size: usize,
}

#[init]
async fn setup() -> Result<()> {
    println!("no setup");
    Ok(())
}

#[cell]
async fn store_derive_backed_type() -> Result<()> {
    let flags = FeatureFlags {
        enable_plots: true,
        sample_size: 128,
    };
    storev!(flags)?;
    println!("Stored versioned FeatureFlags");
    Ok(())
}

#[cell]
async fn load_derive_backed_type() -> Result<()> {
    let flags: FeatureFlags = loadv!(flags)?;
    println!(
        "Loaded FeatureFlags: enable_plots={}, sample_size={}",
        flags.enable_plots, flags.sample_size
    );
    Ok(())
}

#[cell]
async fn explicit_version_for_foreign_like_type() -> Result<()> {
    let bytes = vec![1_u8, 2, 3, 4];
    storev!(bytes, version = 7)?;

    let loaded: Vec<u8> = loadv!(bytes, version = 7)?;
    println!(
        "Stored/loaded Vec<u8> using explicit version override: {:?}",
        loaded
    );
    Ok(())
}

#[cell]
async fn show_version_mismatch_error() -> Result<()> {
    match loadv!(bytes as Vec<u8>, version = 8) {
        Ok(_) => {
            println!("Unexpectedly loaded bytes with wrong version");
        }
        Err(err) => match err {
            CellbookError::Context(ctx_err) => {
                println!("Expected loadv mismatch: {ctx_err}");
            }
            other => {
                println!("Unexpected non-context error: {other}");
            }
        },
    }
    Ok(())
}

#[cell]
async fn failed_consume_keeps_value() -> Result<()> {
    let queue = vec![10_u8, 20, 30];
    storev!(queue, version = 42)?;

    match consumev!(queue as Vec<u8>, version = 43) {
        Ok(_) => {
            println!("Unexpectedly consumed queue with wrong version");
        }
        Err(err) => {
            println!("Expected consumev mismatch: {err}");
        }
    }

    let still_there: Vec<u8> = loadv!(queue, version = 42)?;
    println!("Value still present after failed consume: {:?}", still_there);
    Ok(())
}

#[cell]
async fn successful_consume_removes_value() -> Result<()> {
    let taken: Vec<u8> = consumev!(queue, version = 42)?;
    println!("Successfully consumed queue: {:?}", taken);

    match loadv!(queue as Vec<u8>, version = 42) {
        Ok(_) => {
            println!("Unexpectedly loaded queue after successful consume");
        }
        Err(err) => {
            println!("Expected not-found after consume: {err}");
        }
    }
    Ok(())
}
