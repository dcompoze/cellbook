use std::fs;
use std::path::Path;

use clap::{Args, Parser, Subcommand};

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
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        CargoSubcommand::Cellbook(args) => match args.command {
            Commands::Init { name } => {
                if let Err(e) = init_project(&name) {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        },
    }
}

fn init_project(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let project_path = Path::new(name);

    if project_path.exists() {
        return Err(format!("Directory '{}' already exists", name).into());
    }

    // Create project directory
    fs::create_dir_all(project_path)?;

    // Create Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{name}"
path = "cellbook.rs"

[dependencies]
cellbook = "0.1"
tokio = {{ version = "1", features = ["rt-multi-thread", "macros"] }}
"#
    );
    fs::write(project_path.join("Cargo.toml"), cargo_toml)?;

    // Create cellbook.rs
    let cellbook_rs = r#"use cellbook::{cell, cellbook, load, store, Result};

#[cell]
async fn hello_world() -> Result<()> {
    println!("Hello from cellbook!");

    let message = "Hello, World!".to_string();
    store!(message);

    Ok(())
}

#[cell]
async fn show_message() -> Result<()> {
    let message = load!(message as String)?;
    println!("Stored message: {}", message);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    cellbook!()?;
    Ok(())
}
"#;
    fs::write(project_path.join("cellbook.rs"), cellbook_rs)?;

    // Create Cellbook.toml (configuration)
    let cellbook_toml = r#"[cellbook]
# Configuration for cellbook project

[programs]
# image-viewer = "eog"
# editor = "vim"

[interface]
# table-format = "unicode"

[execution]
# compile-on-save = true
"#;
    fs::write(project_path.join("Cellbook.toml"), cellbook_toml)?;

    println!("Created cellbook project: {}", name);

    Ok(())
}
