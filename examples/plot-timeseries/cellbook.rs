//! Stock price analysis example.

use anyhow::Result;
use cellbook::{cell, init, load, open_image_bytes, store};
use plotters::prelude::*;
use polars::prelude::*;
use serde::{Deserialize, Serialize};

fn plot_err<E: std::fmt::Debug>(e: E) -> std::io::Error {
    std::io::Error::other(format!("{:?}", e))
}

#[init]
async fn setup() -> Result<()> {
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StockStats {
    symbol: String,
    mean_close: f64,
    min_close: f64,
    max_close: f64,
    volatility: f64,
    total_volume: i64,
    trading_days: usize,
    price_change_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StockPrices {
    symbol: String,
    dates: Vec<String>,
    closes: Vec<f64>,
    volumes: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DailyReturns {
    symbol: String,
    returns: Vec<f64>,
}

#[cell]
async fn load_data() -> Result<()> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/stock_prices.csv");

    let df = CsvReadOptions::default()
        .with_has_header(true)
        .try_into_reader_with_file_path(Some(path.into()))
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .finish()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    println!("Loaded {} rows from {}", df.height(), path);
    println!("\nSchema:");
    for field in df.schema().iter_fields() {
        println!("  {}: {:?}", field.name(), field.dtype());
    }

    println!("\nFirst 5 rows:");
    println!("{}", df.head(Some(5)));

    let symbols: Vec<String> = df
        .column("symbol")
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .unique()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .str()
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .into_no_null_iter()
        .map(|s| s.to_string())
        .collect();

    println!("\nSymbols: {:?}", symbols);

    let mut all_prices = Vec::new();

    for symbol in &symbols {
        let mask = df
            .column("symbol")
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .str()
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .equal(symbol.as_str());
        let stock_df = df
            .filter(&mask)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let dates: Vec<String> = stock_df
            .column("date")
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .str()
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .into_no_null_iter()
            .map(|s| s.to_string())
            .collect();

        let closes: Vec<f64> = stock_df
            .column("close")
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .f64()
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .into_no_null_iter()
            .collect();

        let volumes: Vec<i64> = stock_df
            .column("volume")
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .i64()
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .into_no_null_iter()
            .collect();

        all_prices.push(StockPrices {
            symbol: symbol.clone(),
            dates,
            closes,
            volumes,
        });
    }

    store!(symbols)?;
    store!(all_prices)?;

    Ok(())
}

#[cell]
async fn compute_stats() -> Result<()> {
    let all_prices: Vec<StockPrices> = load!(all_prices)?;

    let mut all_stats = Vec::new();

    for prices in &all_prices {
        let closes = &prices.closes;
        let volumes = &prices.volumes;

        let mean_close = closes.iter().sum::<f64>() / closes.len() as f64;
        let min_close = closes.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_close = closes.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Volatility: standard deviation of daily returns.
        let returns: Vec<f64> = closes.windows(2).map(|w| (w[1] - w[0]) / w[0] * 100.0).collect();
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / returns.len() as f64;
        let volatility = variance.sqrt();

        let price_change_pct =
            (closes.last().unwrap() - closes.first().unwrap()) / closes.first().unwrap() * 100.0;

        let stats = StockStats {
            symbol: prices.symbol.clone(),
            mean_close,
            min_close,
            max_close,
            volatility,
            total_volume: volumes.iter().sum(),
            trading_days: closes.len(),
            price_change_pct,
        };

        println!("=== {} ===", prices.symbol);
        println!("  Mean Close:     ${:.2}", stats.mean_close);
        println!(
            "  Range:          ${:.2} - ${:.2}",
            stats.min_close, stats.max_close
        );
        println!("  Volatility:     {:.2}%", stats.volatility);
        println!("  Total Volume:   {:>12}", stats.total_volume);
        println!("  Price Change:   {:+.2}%", stats.price_change_pct);
        println!();

        all_stats.push(stats);
    }

    store!(all_stats)?;

    Ok(())
}

#[cell]
async fn plot_prices() -> Result<()> {
    let all_prices: Vec<StockPrices> = load!(all_prices)?;

    let all_closes: Vec<f64> = all_prices.iter().flat_map(|p| p.closes.clone()).collect();
    let y_min = all_closes.iter().cloned().fold(f64::INFINITY, f64::min) * 0.95;
    let y_max = all_closes.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.05;

    let num_days = all_prices[0].closes.len();

    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (800, 500)).into_drawing_area();
        root.fill(&WHITE).map_err(plot_err)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Stock Price History", ("sans-serif", 24).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(0usize..num_days, y_min..y_max)
            .map_err(plot_err)?;

        let dates = all_prices[0].dates.clone();
        chart
            .configure_mesh()
            .x_labels(10)
            .y_labels(10)
            .x_label_formatter(&|x| {
                if *x < dates.len() {
                    dates[*x][5..10].to_string()
                } else {
                    String::new()
                }
            })
            .y_label_formatter(&|y| format!("${:.0}", y))
            .x_desc("Date")
            .y_desc("Close Price")
            .draw()
            .map_err(plot_err)?;

        let colors = [RED, BLUE, GREEN];

        for (i, prices) in all_prices.iter().enumerate() {
            let color = colors[i % colors.len()];
            let data: Vec<(usize, f64)> = prices.closes.iter().cloned().enumerate().collect();

            chart
                .draw_series(LineSeries::new(data.clone(), color.stroke_width(2)))
                .map_err(plot_err)?
                .label(&prices.symbol)
                .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color.stroke_width(2)));

            chart
                .draw_series(data.iter().map(|(x, y)| Circle::new((*x, *y), 3, color.filled())))
                .map_err(plot_err)?;
        }

        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.8))
            .border_style(BLACK)
            .position(SeriesLabelPosition::UpperLeft)
            .draw()
            .map_err(plot_err)?;

        root.present().map_err(plot_err)?;
    }

    open_image_bytes(svg.as_bytes(), "svg")?;

    Ok(())
}

#[cell]
async fn plot_volume() -> Result<()> {
    let all_stats: Vec<StockStats> = load!(all_stats)?;

    let avg_volumes: Vec<f64> = all_stats
        .iter()
        .map(|s| s.total_volume as f64 / s.trading_days as f64 / 1_000_000.0)
        .collect();

    let y_max = avg_volumes.iter().cloned().fold(0.0, f64::max) * 1.2;

    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (600, 400)).into_drawing_area();
        root.fill(&WHITE).map_err(plot_err)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Average Daily Volume (Millions)", ("sans-serif", 24).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d((0..all_stats.len()).into_segmented(), 0.0..y_max)
            .map_err(plot_err)?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .y_label_formatter(&|y| format!("{:.1}M", y))
            .x_label_formatter(&|x| {
                if let SegmentValue::CenterOf(idx) = x {
                    all_stats.get(*idx).map(|s| s.symbol.clone()).unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .draw()
            .map_err(plot_err)?;

        let colors = [RED, BLUE, GREEN];

        chart
            .draw_series(
                Histogram::vertical(&chart)
                    .style_func(|x, _| {
                        let idx = if let SegmentValue::CenterOf(i) = x { *i } else { 0 };
                        colors[idx % colors.len()].filled()
                    })
                    .margin(20)
                    .data(avg_volumes.iter().enumerate().map(|(i, v)| (i, *v))),
            )
            .map_err(plot_err)?;

        root.present().map_err(plot_err)?;
    }

    open_image_bytes(svg.as_bytes(), "svg")?;

    println!("Volume Summary:");
    for (i, s) in all_stats.iter().enumerate() {
        println!("  {}: {:.2}M shares/day avg", s.symbol, avg_volumes[i]);
    }

    Ok(())
}

#[cell]
async fn plot_performance() -> Result<()> {
    let all_stats: Vec<StockStats> = load!(all_stats)?;

    let changes: Vec<f64> = all_stats.iter().map(|s| s.price_change_pct).collect();
    let y_min = changes.iter().cloned().fold(f64::INFINITY, f64::min).min(0.0) * 1.2;
    let y_max = changes.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(0.0) * 1.2;

    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (600, 400)).into_drawing_area();
        root.fill(&WHITE).map_err(plot_err)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Price Change (%)", ("sans-serif", 24).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d((0..all_stats.len()).into_segmented(), y_min..y_max)
            .map_err(plot_err)?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .y_label_formatter(&|y| format!("{:+.1}%", y))
            .x_label_formatter(&|x| {
                if let SegmentValue::CenterOf(idx) = x {
                    all_stats.get(*idx).map(|s| s.symbol.clone()).unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .draw()
            .map_err(plot_err)?;

        chart
            .draw_series(
                Histogram::vertical(&chart)
                    .style_func(|_, y| if *y >= 0.0 { GREEN.filled() } else { RED.filled() })
                    .margin(20)
                    .data(changes.iter().enumerate().map(|(i, v)| (i, *v))),
            )
            .map_err(plot_err)?;

        chart
            .draw_series(LineSeries::new(
                vec![
                    (SegmentValue::Exact(0), 0.0),
                    (SegmentValue::Exact(all_stats.len()), 0.0),
                ],
                BLACK.stroke_width(1),
            ))
            .map_err(plot_err)?;

        root.present().map_err(plot_err)?;
    }

    open_image_bytes(svg.as_bytes(), "svg")?;

    Ok(())
}

#[cell]
async fn plot_risk_return() -> Result<()> {
    let all_stats: Vec<StockStats> = load!(all_stats)?;

    let volatilities: Vec<f64> = all_stats.iter().map(|s| s.volatility).collect();
    let returns: Vec<f64> = all_stats.iter().map(|s| s.price_change_pct).collect();

    let x_min = volatilities.iter().cloned().fold(f64::INFINITY, f64::min) * 0.8;
    let x_max = volatilities.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.2;
    let y_min = returns.iter().cloned().fold(f64::INFINITY, f64::min) * 0.8;
    let y_max = returns.iter().cloned().fold(f64::NEG_INFINITY, f64::max) * 1.2;

    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (600, 500)).into_drawing_area();
        root.fill(&WHITE).map_err(plot_err)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Risk vs Return", ("sans-serif", 24).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(60)
            .build_cartesian_2d(x_min..x_max, y_min..y_max)
            .map_err(plot_err)?;

        chart
            .configure_mesh()
            .x_desc("Volatility (%)")
            .y_desc("Return (%)")
            .x_label_formatter(&|x| format!("{:.1}%", x))
            .y_label_formatter(&|y| format!("{:+.1}%", y))
            .draw()
            .map_err(plot_err)?;

        let colors = [RED, BLUE, GREEN];

        for (i, stats) in all_stats.iter().enumerate() {
            let color = colors[i % colors.len()];

            chart
                .draw_series(std::iter::once(Circle::new(
                    (stats.volatility, stats.price_change_pct),
                    8,
                    color.filled(),
                )))
                .map_err(plot_err)?;

            chart
                .draw_series(std::iter::once(Text::new(
                    stats.symbol.clone(),
                    (stats.volatility + 0.05, stats.price_change_pct + 0.2),
                    ("sans-serif", 14).into_font(),
                )))
                .map_err(plot_err)?;
        }

        root.present().map_err(plot_err)?;
    }

    open_image_bytes(svg.as_bytes(), "svg")?;

    println!("Risk-Return Analysis:");
    for s in &all_stats {
        println!(
            "  {}: {:.2}% volatility, {:+.2}% return",
            s.symbol, s.volatility, s.price_change_pct
        );
    }

    Ok(())
}

#[cell]
async fn calculate_returns() -> Result<()> {
    let all_prices: Vec<StockPrices> = load!(all_prices)?;

    let mut all_returns = Vec::new();

    for prices in &all_prices {
        let returns: Vec<f64> = prices
            .closes
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0] * 100.0)
            .collect();

        println!(
            "{}: {} daily returns, range [{:.2}%, {:.2}%]",
            prices.symbol,
            returns.len(),
            returns.iter().cloned().fold(f64::INFINITY, f64::min),
            returns.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
        );

        all_returns.push(DailyReturns {
            symbol: prices.symbol.clone(),
            returns,
        });
    }

    store!(all_returns)?;

    Ok(())
}

#[cell]
async fn plot_returns() -> Result<()> {
    let all_returns: Vec<DailyReturns> = load!(all_returns)?;

    let bin_width = 0.5;
    let all_values: Vec<f64> = all_returns.iter().flat_map(|r| r.returns.clone()).collect();
    let min_val = (all_values.iter().cloned().fold(f64::INFINITY, f64::min) / bin_width).floor() * bin_width;
    let max_val =
        (all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max) / bin_width).ceil() * bin_width;

    let mut svg = String::new();
    {
        let root = SVGBackend::with_string(&mut svg, (800, 500)).into_drawing_area();
        root.fill(&WHITE).map_err(plot_err)?;

        let mut chart = ChartBuilder::on(&root)
            .caption("Daily Returns Distribution", ("sans-serif", 24).into_font())
            .margin(10)
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(min_val..max_val, 0u32..10u32)
            .map_err(plot_err)?;

        chart
            .configure_mesh()
            .x_desc("Daily Return (%)")
            .y_desc("Frequency")
            .x_label_formatter(&|x| format!("{:.1}%", x))
            .draw()
            .map_err(plot_err)?;

        let colors = [RED, BLUE, GREEN];

        for (i, dr) in all_returns.iter().enumerate() {
            let color = colors[i % colors.len()];

            let mut bins: std::collections::HashMap<i32, u32> = std::collections::HashMap::new();
            for r in &dr.returns {
                let bin = (r / bin_width).round() as i32;
                *bins.entry(bin).or_insert(0) += 1;
            }

            let data: Vec<(f64, u32)> = bins
                .into_iter()
                .map(|(bin, count)| (bin as f64 * bin_width, count))
                .collect();

            chart
                .draw_series(data.iter().map(|(x, y)| Circle::new((*x, *y), 5, color.filled())))
                .map_err(plot_err)?
                .label(&dr.symbol)
                .legend(move |(x, y)| Circle::new((x + 10, y), 5, color.filled()));

            let mut sorted_data = data.clone();
            sorted_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            chart
                .draw_series(LineSeries::new(sorted_data, color.stroke_width(1)))
                .map_err(plot_err)?;
        }

        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.8))
            .border_style(BLACK)
            .position(SeriesLabelPosition::UpperRight)
            .draw()
            .map_err(plot_err)?;

        root.present().map_err(plot_err)?;
    }

    open_image_bytes(svg.as_bytes(), "svg")?;

    Ok(())
}

#[cell]
async fn summary() -> Result<()> {
    let all_stats: Vec<StockStats> = load!(all_stats)?;

    println!("+--------------------------------------------------------------------+");
    println!("|                    STOCK ANALYSIS SUMMARY                          |");
    println!("+--------------------------------------------------------------------+");

    let best = all_stats
        .iter()
        .max_by(|a, b| a.price_change_pct.partial_cmp(&b.price_change_pct).unwrap())
        .unwrap();
    let worst = all_stats
        .iter()
        .min_by(|a, b| a.price_change_pct.partial_cmp(&b.price_change_pct).unwrap())
        .unwrap();
    let lowest_vol = all_stats
        .iter()
        .min_by(|a, b| a.volatility.partial_cmp(&b.volatility).unwrap())
        .unwrap();

    println!(
        "|  Best Performer:    {:5} ({:+.2}%)                              |",
        best.symbol, best.price_change_pct
    );
    println!(
        "|  Worst Performer:   {:5} ({:+.2}%)                              |",
        worst.symbol, worst.price_change_pct
    );
    println!(
        "|  Lowest Volatility: {:5} ({:.2}%)                               |",
        lowest_vol.symbol, lowest_vol.volatility
    );
    println!("+--------------------------------------------------------------------+");
    println!("|  Symbol  |  Close Avg  |  Change  |  Volatility  |   Volume       |");
    println!("+--------------------------------------------------------------------+");

    for s in &all_stats {
        println!(
            "|  {:5}   |  ${:>8.2} | {:>+6.2}%  |    {:>5.2}%    | {:>13}  |",
            s.symbol, s.mean_close, s.price_change_pct, s.volatility, s.total_volume
        );
    }

    println!("+--------------------------------------------------------------------+");

    Ok(())
}
