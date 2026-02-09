//! Unit testing example for cellbook cells.

use anyhow::Result;
use cellbook::{cell, cellbook, load, store, Config};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stats {
    mean: f64,
    sum: f64,
    count: usize,
}

#[cell]
async fn load_data() -> Result<()> {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    store!(data)?;
    Ok(())
}

#[cell]
async fn compute_stats() -> Result<()> {
    let data: Vec<f64> = load!(data)?;

    let sum: f64 = data.iter().sum();
    let count = data.len();
    let mean = sum / count as f64;

    let stats = Stats { mean, sum, count };
    store!(stats)?;

    Ok(())
}

#[cell]
async fn print_stats() -> Result<()> {
    let stats: Stats = load!(stats)?;
    println!(
        "Mean: {:.2}, Sum: {:.2}, Count: {}",
        stats.mean, stats.sum, stats.count
    );
    Ok(())
}

cellbook!(Config::default());

#[cfg(test)]
mod tests {
    use super::*;
    use cellbook::test::TestContext;

    #[tokio::test]
    async fn test_load_data() {
        let ctx = TestContext::default();

        load_data(&ctx).await.unwrap();

        let data: Vec<f64> = ctx.load("data").unwrap();
        assert_eq!(data, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[tokio::test]
    async fn test_compute_stats() {
        let ctx = TestContext::default();
        ctx.store("data", &vec![10.0, 20.0, 30.0]).unwrap();

        compute_stats(&ctx).await.unwrap();

        let stats: Stats = ctx.load("stats").unwrap();
        assert_eq!(stats.count, 3);
        assert_eq!(stats.sum, 60.0);
        assert_eq!(stats.mean, 20.0);
    }

    #[tokio::test]
    async fn test_full_pipeline() {
        let ctx = TestContext::default();

        load_data(&ctx).await.unwrap();
        compute_stats(&ctx).await.unwrap();

        let stats: Stats = ctx.load("stats").unwrap();
        assert_eq!(stats.count, 5);
        assert_eq!(stats.sum, 15.0);
        assert_eq!(stats.mean, 3.0);
    }

    #[tokio::test]
    async fn test_missing_data_error() {
        let ctx = TestContext::default();

        let result = compute_stats(&ctx).await;
        assert!(result.is_err());
    }
}
