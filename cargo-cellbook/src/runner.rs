//! TUI runner for cellbook.

use std::io::{BufRead, Write};

use tokio::sync::mpsc;

use crate::errors::Result;
use crate::loader::LoadedLibrary;
use crate::store;

pub enum TuiEvent {
    Reloaded,
    BuildStarted,
    BuildCompleted(Option<String>),
}

pub async fn run_tui(
    lib: &mut LoadedLibrary,
    mut event_rx: mpsc::Receiver<TuiEvent>,
) -> Result<()> {
    print_header(lib);

    let (input_tx, mut input_rx) = mpsc::channel::<String>(32);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let reader = stdin.lock();
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if input_tx.blocking_send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    print!("> ");
    std::io::stdout().flush()?;

    loop {
        tokio::select! {
            biased;

            event = event_rx.recv() => {
                match event {
                    Some(TuiEvent::Reloaded) => {
                        match lib.reload() {
                            Ok(()) => {
                                println!("\n✓ Reloaded\n");
                                print_cells(lib);
                            }
                            Err(e) => {
                                println!("\nReload error: {}\n", e);
                            }
                        }
                        print!("> ");
                        std::io::stdout().flush()?;
                    }
                    Some(TuiEvent::BuildStarted) => {
                        print!("\rBuilding...");
                        std::io::stdout().flush()?;
                    }
                    Some(TuiEvent::BuildCompleted(None)) => {
                        println!(" done");
                        print!("> ");
                        std::io::stdout().flush()?;
                    }
                    Some(TuiEvent::BuildCompleted(Some(err))) => {
                        println!("\nBuild error:\n{}", err);
                        print!("> ");
                        std::io::stdout().flush()?;
                    }
                    None => {
                        break;
                    }
                }
            }

            input = input_rx.recv() => {
                match input {
                    Some(line) => {
                        let input = line.trim();
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
                        print!("> ");
                        std::io::stdout().flush()?;
                    }
                    None => {
                        break;
                    }
                }
            }
        }
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
