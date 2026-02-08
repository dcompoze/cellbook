mod errors;
mod loader;
mod runner;
mod store;
mod tui;
mod watcher;

use std::fs;
use std::path::Path;

use clap::{Args, Parser, Subcommand};
use errors::Result;
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "cargo-cellbook")]
#[command(bin_name = "cargo")]
#[command(about = "A tool for managing cellbook projects")]
struct Cli {
    #[command(subcommand)]
    command: CargoSubcommand,
}

#[derive(Subcommand)]
enum CargoSubcommand {
    /// Cellbook commands
    Cellbook(CellbookArgs),
}

#[derive(Args)]
struct CellbookArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new cellbook project
    Init {
        /// Name of the project
        name: String,
    },
    /// Run the cellbook TUI with hot-reloading
    Run,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        CargoSubcommand::Cellbook(args) => match args.command {
            Commands::Init { name } => init_project(&name),
            Commands::Run => run_project().await,
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run_project() -> Result<()> {
    // Find the dylib path
    let lib_path = loader::find_dylib_path()?;

    // Initial build
    watcher::initial_build().await?;

    // Load the library
    let mut lib = loader::LoadedLibrary::load(&lib_path)?;

    // Set up event channel
    let (event_tx, event_rx) = mpsc::channel(32);

    // Start file watcher (uses config for auto_reload and debounce_ms)
    let watcher_handle = watcher::start_watcher(event_tx, lib.config()).await?;

    // Run the TUI
    tui::run(&mut lib, event_rx).await?;

    // Stop the watcher when TUI exits
    if let Some(handle) = watcher_handle {
        handle.stop();
    }

    Ok(())
}

fn init_project(name: &str) -> Result<()> {
    let project_path = Path::new(name);

    if project_path.exists() {
        return Err(errors::Error::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Directory '{}' already exists", name),
        )));
    }

    // Create project directory
    fs::create_dir_all(project_path)?;

    // Create Cargo.toml for a dylib crate
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]
path = "cellbook.rs"

[dependencies]
cellbook = "0.1"
"#
    );
    fs::write(project_path.join("Cargo.toml"), cargo_toml)?;

    // Create cellbook.rs with example cell
    let cellbook_rs = r#"use cellbook::{cell, cellbook, Config, Result};

#[cell]
async fn hello() -> Result<()> {
    println!("Hello");
    Ok(())
}

cellbook!(Config::default());
"#;
    fs::write(project_path.join("cellbook.rs"), cellbook_rs)?;

    println!("Created cellbook project: {}", name);

    Ok(())
}
