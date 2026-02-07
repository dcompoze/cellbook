//! TUI runner for cellbook.
//!
//! Provides an interactive loop for running cells and viewing context.

use std::io::Write;

use tokio::sync::mpsc;

use crate::errors::Result;
use crate::loader::LoadedLibrary;
use crate::store;

/// Events that can occur during the TUI loop
pub enum TuiEvent {
    /// Library was reloaded
    Reloaded,
    /// Build started
    BuildStarted,
    /// Build completed (with optional error message)
    BuildCompleted(Option<String>),
}

/// Run the interactive TUI loop.
pub async fn run_tui(
    lib: &mut LoadedLibrary,
    mut event_rx: mpsc::Receiver<TuiEvent>,
) -> Result<()> {
    print_header(lib);

    loop {
        // Check for events without blocking
        while let Ok(event) = event_rx.try_recv() {
            match event {
                TuiEvent::Reloaded => {
                    match lib.reload() {
                        Ok(()) => {
                            println!("\n✓ Reloaded\n");
                            print_cells(lib);
                        }
                        Err(e) => {
                            println!("\nReload error: {}\n", e);
                        }
                    }
                }
                TuiEvent::BuildStarted => {
                    print!("Building...");
                    std::io::stdout().flush()?;
                }
                TuiEvent::BuildCompleted(None) => {
                    println!(" done");
                }
                TuiEvent::BuildCompleted(Some(err)) => {
                    println!("\nBuild error:\n{}", err);
                }
            }
        }

        print!("> ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "q" | "quit" => break,
            "a" | "all" => {
                run_all_cells(lib).await;
            }
            "c" | "context" => {
                print_context();
            }
            "r" | "reload" => {
                println!("Use file save to trigger reload");
            }
            "x" | "clear" => {
                store::clear();
                println!("Context cleared");
            }
            "?" | "h" | "help" => {
                print_help();
            }
            "" => {}
            _ => {
                if let Ok(n) = input.parse::<usize>() {
                    run_cell_by_number(lib, n).await;
                } else {
                    println!("Unknown command: {} (type ? for help)", input);
                }
            }
        }
        println!();
    }

    Ok(())
}

fn print_header(lib: &LoadedLibrary) {
    let cells = lib.cells();
    println!("Cellbook - {} cells registered:\n", cells.len());
    print_cells(lib);
    println!("\n  [a] Run all  [c] Context  [x] Clear  [q] Quit  [?] Help\n");
}

fn print_cells(lib: &LoadedLibrary) {
    for (i, cell) in lib.cells().iter().enumerate() {
        println!("  [{}] {}", i + 1, cell.name);
    }
}

fn print_context() {
    let items = store::list();
    if items.is_empty() {
        println!("Context is empty");
    } else {
        println!("Context:");
        for (key, type_name) in items {
            println!("  {}: {}", key, type_name);
        }
    }
}

fn print_help() {
    println!("Commands:");
    println!("  [n]    Run cell n");
    println!("  [a]    Run all cells");
    println!("  [c]    Show context");
    println!("  [x]    Clear context");
    println!("  [q]    Quit");
    println!();
    println!("Hot reload is automatic on file save.");
}

async fn run_all_cells(lib: &LoadedLibrary) {
    for cell in lib.cells() {
        println!("Running {}...", cell.name);
        if let Err(e) = lib.run_cell(&cell.name).await {
            println!("Error in {}: {}", cell.name, e);
        }
    }
}

async fn run_cell_by_number(lib: &LoadedLibrary, n: usize) {
    let cells = lib.cells();
    if n >= 1 && n <= cells.len() {
        let cell = &cells[n - 1];
        println!("Running {}...", cell.name);
        if let Err(e) = lib.run_cell(&cell.name).await {
            println!("Error: {}", e);
        }
    } else {
        println!("Invalid cell number");
    }
}
