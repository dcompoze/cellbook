use cellbook::{cell, cellbook, load, store, Result};

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
