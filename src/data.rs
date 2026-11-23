// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Free Binance public market data (no API key, $0 budget).
//! REST klines with transparent resume + CSV cache. Tries multiple hosts
//! because api.binance.com is geo-restricted in some regions.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::Bar;

const HOSTS: &[&str] = &[
    "https://api.binance.com",
    "https://api1.binance.com",
    "https://api2.binance.com",
    "https://api3.binance.com",
    "https://api4.binance.com",
    "https://data-api.binance.vision",
];

pub fn interval_ms(interval: &str) -> anyhow::Result<i64> {
    let ms = match interval {
        "1m" => 60_000,
        "3m" => 3 * 60_000,
        "5m" => 5 * 60_000,
        "15m" => 15 * 60_000,
        "30m" => 30 * 60_000,
        "1h" => 3_600_000,
        "2h" => 2 * 3_600_000,
        "4h" => 4 * 3_600_000,
        "6h" => 6 * 3_600_000,
        "12h" => 12 * 3_600_000,
        "1d" => 24 * 3_600_000,
        "1w" => 7 * 24 * 3_600_000,
        _ => anyhow::bail!("unsupported interval {interval} (use 1m/5m/15m/1h/4h/1d)"),
    };
    Ok(ms)
}

/// Bars per year for annualization. Crypto trades 24/7/365.
pub fn bars_per_year(interval: &str) -> f64 {
    let year_ms = 365.0 * 24.0 * 3_600_000.0;
    match interval_ms(interval) {
        Ok(ms) => year_ms / ms as f64,
        Err(_) => 35_040.0,
    }
}

pub fn data_path(data_dir: &Path, symbol: &str, interval: &str) -> PathBuf {
    data_dir.join(format!("{symbol}_{interval}.csv"))
}

pub fn load_bars(path: &Path) -> anyhow::Result<Vec<Bar>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut rdr = csv::Reader::from_path(path)?;
    let mut out = Vec::new();
    for rec in rdr.deserialize() {
        let b: Bar = rec?;
        if b.close > 0.0 && b.high >= b.low {
            out.push(b);
        }
    }
    out.sort_by_key(|b| b.open_time);
    out.dedup_by_key(|b| b.open_time);
    Ok(out)
}

pub fn save_bars(path: &Path, bars: &[Bar]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut wtr = csv::Writer::from_path(path)?;
    for b in bars {
        wtr.serialize(b)?;
    }
    wtr.flush()?;
    Ok(())
}

async fn fetch_page(
    client: &reqwest::Client,
    symbol: &str,
    interval: &str,
    start_ms: i64,
    end_ms: Option<i64>,
) -> anyhow::Result<Vec<Bar>> {
    let mut last_err: Option<anyhow::Error> = None;
    for host in HOSTS {
        let mut url = format!(
            "{host}/api/v3/klines?symbol={symbol}&interval={interval}&startTime={start_ms}&limit=1000"
        );
        if let Some(end) = end_ms {
            url.push_str(&format!("&endTime={end}"));
        }
        match client.get(&url).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let st = resp.status();
                    let txt = resp.text().await.unwrap_or_default();
                    // 451 = restricted location — try next host silently.
                    last_err = Some(anyhow::anyhow!("{host} -> {st}: {txt}"));
                    continue;
                }
                let v: serde_json::Value = resp.json().await?;
                let arr = v.as_array().cloned().unwrap_or_default();
                let mut bars = Vec::with_capacity(arr.len());
                for row in arr {
                    let r = row.as_array().cloned().unwrap_or_default();
                    if r.len() < 6 {
                        continue;
                    }
                    let ot = r[0].as_i64().unwrap_or(0);
                    let o: f64 = r[1].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let h: f64 = r[2].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let l: f64 = r[3].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let c: f64 = r[4].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let vol: f64 = r[5].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    if ot > 0 && o > 0.0 && c > 0.0 {
                        bars.push(Bar { open_time: ot, open: o, high: h, low: l, close: c, volume: vol });
                    }
                }
                return Ok(bars);
            }
            Err(e) => {
                last_err = Some(anyhow::anyhow!("{host}: {e}"));
                continue;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("all Binance hosts failed")))
}

/// Download max history with resume. Returns full bar vec (cached + new).
pub async fn download_symbol(symbol: &str, interval: &str, data_dir: &Path) -> anyhow::Result<Vec<Bar>> {
    let step = interval_ms(interval)?;
    let path = data_path(data_dir, symbol, interval);
    let mut bars = load_bars(&path)?;
    let mut start_ms = bars.last().map(|b| b.open_time + step).unwrap_or_else(|| {
        // Binance spot history starts ~2017-08. Start 2018-01-01 for majors.
        1_514_764_800_000 // 2018-01-01T00:00:00Z
    });
    // If cache has an incomplete tail (last bar is still forming), step back one.
    if !bars.is_empty() {
        start_ms -= step; // re-fetch last bar to heal it, dedup handles overlap
    }

    let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
    println!("  [dl] {symbol} {interval}: cached {} bars, resuming from {}", bars.len(), start_ms);

    let mut total_new = 0usize;
    loop {
        let page = fetch_page(&client, symbol, interval, start_ms, None).await?;
        if page.is_empty() {
            break;
        }
        let first = page.first().unwrap().open_time;
        let last = page.last().unwrap().open_time;
        // Merge (dedup by open_time).
        for b in &page {
            if bars.last().map(|l| l.open_time < b.open_time).unwrap_or(true) {
                bars.push(*b);
                total_new += 1;
            } else if let Some(pos) = bars.iter().position(|x| x.open_time == b.open_time) {
                bars[pos] = *b; // heal forming candle
            }
        }
        println!("  [dl]   ... got {} bars {} -> {} (total {})", page.len(), first, last, bars.len());
        if page.len() < 1000 {
            break;
        }
        start_ms = last + step;
        // Be kind to the free tier.
        tokio::time::sleep(Duration::from_millis(250)).await;
        // Safety: never loop forever.
        if total_new > 600_000 {
            break;
        }
    }
    bars.sort_by_key(|b| b.open_time);
    bars.dedup_by_key(|b| b.open_time);
    save_bars(&path, &bars)?;
    println!("  [dl] {symbol} {interval}: DONE {} bars -> {}", bars.len(), path.display());
    Ok(bars)
}

/// Fetch the most recent `limit` klines (for paper mode). No cache write.
pub async fn fetch_recent(symbol: &str, interval: &str, limit: usize) -> anyhow::Result<Vec<Bar>> {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
    let mut last_err: Option<anyhow::Error> = None;
    for host in HOSTS {
        let url = format!("{host}/api/v3/klines?symbol={symbol}&interval={interval}&limit={limit}");
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let v: serde_json::Value = resp.json().await?;
                let arr = v.as_array().cloned().unwrap_or_default();
                let mut bars = Vec::new();
                for row in arr {
                    let r = row.as_array().cloned().unwrap_or_default();
                    if r.len() < 6 {
                        continue;
                    }
                    bars.push(Bar {
                        open_time: r[0].as_i64().unwrap_or(0),
                        open: r[1].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        high: r[2].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        low: r[3].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        close: r[4].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                        volume: r[5].as_str().unwrap_or("0").parse().unwrap_or(0.0),
                    });
                }
                return Ok(bars);
            }
            Ok(resp) => {
                last_err = Some(anyhow::anyhow!("{}: {}", host, resp.status()));
            }
            Err(e) => last_err = Some(e.into()),
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch_recent failed")))
}
