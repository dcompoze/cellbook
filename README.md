<br>
<div style="text-align: center"><img src="./cellbook.svg" width="20%"></div>


# Cellbook

Computational notebook experience in plain Rust.

Define cells as async functions, share data between them via a typed context store, and run them interactively from the command line.

## Features

- **Cell-based workflow** - Break analysis into discrete, rerunnable steps
- **Typed context store** - Share data between cells with compile-time type safety
- **Hot reloading** - Edit code and rerun cells without restarting (planned)
- **Pure Rust** - No notebooks, no Python, just `cargo run`

## Installation

```bash
cargo install cargo-cellbook
```

## Quick Start

```bash
cargo cellbook init my-analysis
cd my-analysis
cargo run
```

This creates a new cellbook project with a single `cellbook.rs` file.

## Usage

```rust
use cellbook::{cell, cellbook, load, store, Config, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Stats {
    mean: f64,
    count: usize,
}

#[cell]
async fn load_data() -> Result<()> {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    store!(data)?;
    println!("Loaded {} values", data.len());
    Ok(())
}

#[cell]
async fn compute_stats() -> Result<()> {
    let data: Vec<f64> = load!(data as Vec<f64>)?;
    let stats = Stats {
        mean: data.iter().sum::<f64>() / data.len() as f64,
        count: data.len(),
    };
    println!("Mean: {:.2}", stats.mean);
    store!(stats)?;
    Ok(())
}

cellbook!();
```

Run with `cargo run`, then select cells to execute by number.

## Context Store

Cells share data through a key-value store with typed access:

```rust
// Store a value (key is the variable name)
store!(data)?;

// Store with explicit key
store!(my_key = some_value)?;

// Load a value (must specify type)
let data: Vec<f64> = load!(data as Vec<f64>)?;

// Remove a value
remove!(data);

// Load and remove in one operation
let data: Vec<f64> = consume!(data as Vec<f64>)?;
```

Values are serialized with [postcard](https://crates.io/crates/postcard), so stored types must implement `Serialize` and `Deserialize`.

## Components

| Crate | Description |
|-------|-------------|
| `cellbook` | Core library with context store, cell registry, and macros |
| `cellbook-macros` | Proc macros (`#[cell]`, `cellbook!`) |
| `cargo-cellbook` | CLI for project scaffolding and runtime |
