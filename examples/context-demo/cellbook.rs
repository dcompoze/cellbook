use std::sync::Arc;

use cellbook::{cell, cellbook, load, store, Result};

#[derive(Debug, Clone)]
struct Config {
    threshold: f64,
    name: String,
}

#[derive(Debug, Clone)]
struct AnalysisResult {
    mean: f64,
    count: usize,
}

#[cell]
async fn setup() -> Result<()> {
    let config = Config {
        threshold: 2.5,
        name: "demo".to_string(),
    };
    store!(config);

    let raw_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 100.0];
    store!(raw_data);

    println!("Setup complete - stored config and raw_data");
    Ok(())
}

#[cell]
async fn analyze() -> Result<()> {
    let config: Arc<Config> = load!(config as Config)?;
    let raw_data: Arc<Vec<f64>> = load!(raw_data as Vec<f64>)?;

    let mean = raw_data.iter().sum::<f64>() / raw_data.len() as f64;
    let count = raw_data
        .iter()
        .filter(|x| (**x - mean).abs() <= config.threshold * mean)
        .count();

    let result = AnalysisResult { mean, count };
    println!("Analysis: mean={:.2}, valid_count={}", result.mean, result.count);

    store!(result);
    Ok(())
}

#[cell]
async fn report() -> Result<()> {
    let config: Arc<Config> = load!(config as Config)?;
    let result: Arc<AnalysisResult> = load!(result as AnalysisResult)?;

    println!("=== Report for '{}' ===", config.name);
    println!("Threshold: {}", config.threshold);
    println!("Mean: {:.4}", result.mean);
    println!("Valid count: {}", result.count);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    cellbook!()?;
    Ok(())
}
