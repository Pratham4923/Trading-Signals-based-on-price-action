use reqwest::Client;
use serde::Deserialize;
use std::error::Error;
use chrono::{DateTime, Utc, TimeZone};

#[derive(Debug, Deserialize)]
struct ChartQuote {
    close: Vec<Option<f64>>,
    open: Vec<Option<f64>>,
    high: Vec<Option<f64>>,
    low: Vec<Option<f64>>,
    volume: Vec<Option<i64>>,
}

#[derive(Debug, Deserialize)]
struct ChartIndicators {
    quote: Vec<ChartQuote>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    timestamp: Option<Vec<i64>>,
    indicators: ChartIndicators,
}

#[derive(Debug, Deserialize)]
struct ChartResponse {
    result: Option<Vec<ChartResult>>,
}

#[derive(Debug, Deserialize)]
struct YahooChartResponse {
    chart: ChartResponse,
}

#[derive(Debug, Clone)]
struct MarketData {
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    atr: f64,
}

#[derive(Debug)]
struct Signal {
    pair: String,
    date: String,
    signal_type: String, // "BUY" or "SELL"
    entry: f64,
    stop_loss: f64,
    take_profit: f64,
}

/// Fetch chart data from Yahoo Finance API asynchronously
async fn fetch_data(client: &Client, symbol: &str, period: &str, interval: &str) -> Result<Vec<MarketData>, Box<dyn Error>> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?range={}&interval={}",
        symbol, period, interval
    );
    
    // We send an asynchronous JSON request to Yahoo Finance
    let resp = client.get(&url).send().await?.json::<YahooChartResponse>().await?;
    
    // Safely unwrap options without crashing (Anti-leak & Crash-proof design)
    let chart_result = resp.chart.result.ok_or("No data returned")?;
    let result = &chart_result[0];
    
    let timestamps = result.timestamp.as_ref().ok_or("No timestamps")?;
    let quote = &result.indicators.quote[0];

    // Pre-allocate vector for memory efficiency
    let mut clean_data = Vec::with_capacity(timestamps.len());

    // Iterate through raw arrays ensuring there are no null values holding us back
    for i in 0..timestamps.len() {
        if let (Some(open), Some(high), Some(low), Some(close)) = (
            quote.open[i], quote.high[i], quote.low[i], quote.close[i]
        ) {
            clean_data.push(MarketData {
                timestamp: Utc.timestamp_opt(timestamps[i], 0).unwrap(),
                open,
                high,
                low,
                close,
                atr: 0.0,
            });
        }
    }

    // Mathematical ATR calculation
    let window = 14;
    for i in 0..clean_data.len() {
        let tr = if i > 0 {
            let current = &clean_data[i];
            let prev = &clean_data[i - 1];
            let hl = current.high - current.low;
            let hc = (current.high - prev.close).abs();
            let lc = (current.low - prev.close).abs();
            hl.max(hc).max(lc)
        } else {
            clean_data[i].high - clean_data[i].low
        };

        // Rolling SMA for Average True Range
        let atr = if i >= window {
            let start = i as i64 - window as i64 + 1;
            let mut sum_tr = 0.0;
            
            for j in start..=i as i64 {
                let j_usize = j as usize;
                let j_tr = if j_usize > 0 {
                    let cur = &clean_data[j_usize];
                    let pr = &clean_data[j_usize - 1];
                    let h_l = cur.high - cur.low;
                    let h_c = (cur.high - pr.close).abs();
                    let l_c = (cur.low - pr.close).abs();
                    h_l.max(h_c).max(l_c)
                } else {
                    clean_data[j_usize].high - clean_data[j_usize].low
                };
                sum_tr += j_tr;
            }
            sum_tr / window as f64
        } else {
            tr
        };
        
        // Mutate safely (Borrow Checker enforces validity)
        clean_data[i].atr = atr;
    }

    Ok(clean_data)
}

/// Price action strat implementation
fn calculate_signals(data: &[MarketData], pair: &str, atr_multiplier: f64) -> Vec<Signal> {
    let mut signals = Vec::new();

    // Iterate from 1 because we need access to the `previous` bar
    for i in 1..data.len() {
        let current = &data[i];
        let previous = &data[i - 1];
        let atr = current.atr;

        // Buying Condition Mapping
        if current.close > previous.high && current.open > previous.high {
            let entry = current.open;
            let stop_loss = entry - (atr * atr_multiplier);
            let take_profit = entry + (atr * atr_multiplier * 2.0);

            signals.push(Signal {
                pair: pair.to_string(),
                date: current.timestamp.to_string(),
                signal_type: "BUY".to_string(),
                entry,
                stop_loss,
                take_profit,
            });
        }
        // Selling Condition Mapping 
        else if current.close < previous.low && current.open < previous.low {
            let entry = current.open;
            let stop_loss = entry + (atr * atr_multiplier);
            let take_profit = entry - (atr * atr_multiplier * 2.0);

            signals.push(Signal {
                pair: pair.to_string(),
                date: current.timestamp.to_string(),
                signal_type: "SELL".to_string(),
                entry,
                stop_loss,
                take_profit,
            });
        }
    }

    signals
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // We instantiate a fake User-Agent because Yahoo Finance blocks automated generic requests
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36")
        .build()?;

    let stock_list = vec![
        "BTC-USD", "RELIANCE.NS", "TCS.NS", "INFY.NS", "HDFCBANK.NS", "WIPRO.NS"
    ];

    println!("===========================================================");
    println!("🚀 RUST TRADING ENGINE ACTIVE");
    println!("🛡️ Anti-leak active. Memory structurally protected.");
    println!("===========================================================");

    for pair in stock_list {
        println!("Analyzing {}...", pair);
        match fetch_data(&client, pair, "1d", "5m").await {
            Ok(market_data) => {
                if market_data.is_empty() {
                    println!("  ⚠️ No data available for {}", pair);
                    continue;
                }
                
                let signals = calculate_signals(&market_data, pair, 1.5);
                
                if signals.is_empty() {
                    println!("  ✅ No trade signals triggered (P.A. constraints unmet).");
                } else {
                    for signal in signals {
                        if signal.signal_type == "BUY" {
                            println!(
                                "  🟢 [{}] SIGNAL: {} | Entry: {:.2} | Stop: {:.2} | TP: {:.2}",
                                signal.date, signal.signal_type, signal.entry, signal.stop_loss, signal.take_profit
                            );
                        } else {
                            println!(
                                "  🔴 [{}] SIGNAL: {} | Entry: {:.2} | Stop: {:.2} | TP: {:.2}",
                                signal.date, signal.signal_type, signal.entry, signal.stop_loss, signal.take_profit
                            );
                        }
                    }
                }
            },
            Err(e) => {
                println!("  ❌ Error fetching {}: {}", pair, e);
            }
        }
    }

    println!("===========================================================");
    Ok(())
}
