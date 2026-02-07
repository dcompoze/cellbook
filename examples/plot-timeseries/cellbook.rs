use cellbook::{cell, cellbook, Config, Result};

#[cell]
async fn hello() -> Result<()> {
    println!("Hello");
    Ok(())
}

cellbook!(Config::default());
