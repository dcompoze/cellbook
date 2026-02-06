pub mod context;
pub mod errors;
mod macros;
pub mod registry;

pub use cellbook_macros::cell;
pub use errors::{ContextError, Error, Result};
pub use registry::CellInfo;
pub use {futures, inventory};

/// Main entry point macro. Runs the cellbook TUI.
#[macro_export]
macro_rules! cellbook {
    () => {{
        $crate::run_tui().await
    }};
}

/// Runs the cellbook TUI loop.
pub async fn run_tui() -> Result<()> {
    let cells = registry::cells();

    if cells.is_empty() {
        println!("No cells registered.");
        return Ok(());
    }

    println!("Cellbook - {} cells registered:\n", cells.len());
    for (i, cell) in cells.iter().enumerate() {
        println!("  [{}] {}", i + 1, cell.name);
    }
    println!("\n  [a] Run all");
    println!("  [c] Show context");
    println!("  [q] Quit\n");

    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "q" | "quit" => break,
            "a" | "all" => {
                for cell in &cells {
                    println!("Running {}...", cell.name);
                    if let Err(e) = (cell.func)().await {
                        println!("Error in {}: {}", cell.name, e);
                    }
                }
            }
            "c" | "context" => {
                println!("Context:");
                for (key, type_name) in context::list() {
                    println!("  {}: {}", key, type_name);
                }
            }
            _ => {
                if let Ok(n) = input.parse::<usize>() {
                    if n >= 1 && n <= cells.len() {
                        let cell = &cells[n - 1];
                        println!("Running {}...", cell.name);
                        if let Err(e) = (cell.func)().await {
                            println!("Error: {}", e);
                        }
                    } else {
                        println!("Invalid cell number");
                    }
                } else {
                    println!("Unknown command: {}", input);
                }
            }
        }
        println!();
    }

    Ok(())
}
