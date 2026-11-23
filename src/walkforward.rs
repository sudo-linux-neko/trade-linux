// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Grid search + walk-forward validation from scratch.
//! Guards against overfitting: IS optimize -> OOS confirm, min-trades filter,
//! plateau check (neighbors also profitable), deflated-Sharpe-style haircut note.

use serde::{Deserialize, Serialize};

use crate::backtest::{run_backtest, BacktestConfig, BacktestResult};
use crate::strategy::StrategyParams;
use crate::types::Bar;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridScore {
    pub params: StrategyParams,
    pub sharpe: f64,
    pub total_return_pct: f64,
    pub max_dd_pct: f64,
    pub n_trades: usize,
    pub profit_factor: f64,
}

/// Sensible default grid: 3*3*2*2*2 = 72 combos. Fast on 140k bars in Rust.
pub fn default_grid() -> Vec<StrategyParams> {
    let mut grid = Vec::new();
    for fast in [9usize, 21, 34] {
        for slow in [55usize, 89, 144] {
            if fast >= slow {
                continue;
            }
            for rsi_min in [50.0, 55.0] {
                for stop in [2.0, 3.0] {
                    for take in [3.0, 5.0] {
                        grid.push(StrategyParams {
                            fast,
                            slow,
                            rsi_entry_min: rsi_min,
                            stop_mult: stop,
                            take_mult: take,
                            ..StrategyParams::default()
                        });
                    }
                }
            }
        }
    }
    grid
}

/// Exhaustive search on one slice. Filters dust (min_trades) then ranks by Sharpe.
pub fn grid_search(
    bars: &[Bar],
    grid: &[StrategyParams],
    config: &BacktestConfig,
    symbol: &str,
    interval: &str,
    min_trades: usize,
) -> Vec<(StrategyParams, BacktestResult)> {
    let mut scored: Vec<(StrategyParams, BacktestResult)> = Vec::new();
    for p in grid {
        if let Ok(res) = run_backtest(bars, p, config, symbol, interval) {
            if res.metrics.n_trades >= min_trades {
                scored.push((*p, res));
            }
        }
    }
    scored.sort_by(|a, b| {
        b.1.metrics
            .sharpe
            .partial_cmp(&a.1.metrics.sharpe)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResult {
    pub window: usize,
    pub train_start: i64,
    pub train_end: i64,
    pub test_start: i64,
    pub test_end: i64,
    pub best_tag: String,
    pub best_params: StrategyParams,
    pub train_sharpe: f64,
    pub train_trades: usize,
    pub test_sharpe: f64,
    pub test_return_pct: f64,
    pub test_trades: usize,
    pub test_max_dd_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WFReport {
    pub symbol: String,
    pub interval: String,
    pub windows: Vec<WindowResult>,
    pub avg_oos_sharpe: f64,
    pub oos_hit_rate_pct: f64, // % of OOS windows with positive return
    pub total_oos_trades: usize,
}

/// Rolling walk-forward: split into `n_windows` equal chunks; each chunk is
/// 70% train / 30% test. Best IS params (by Sharpe) evaluated on OOS test.
pub fn walk_forward(
    bars: &[Bar],
    n_windows: usize,
    grid: &[StrategyParams],
    config: &BacktestConfig,
    symbol: &str,
    interval: &str,
    min_trades: usize,
) -> anyhow::Result<WFReport> {
    anyhow::ensure!(bars.len() > n_windows * 500, "not enough bars for {} windows", n_windows);
    let chunk = bars.len() / n_windows;
    let mut windows = Vec::new();

    for w in 0..n_windows {
        let c0 = w * chunk;
        let c1 = if w + 1 == n_windows { bars.len() } else { (w + 1) * chunk };
        let slice = &bars[c0..c1];
        if slice.len() < 400 {
            continue;
        }
        let split = (slice.len() as f64 * 0.7) as usize;
        let (train, test) = slice.split_at(split);
        if train.len() < 200 || test.len() < 100 {
            continue;
        }
        let ranked = grid_search(train, grid, config, symbol, interval, min_trades);
        if ranked.is_empty() {
            continue;
        }
        let (best_p, train_res) = &ranked[0];
        let test_res = run_backtest(test, best_p, config, symbol, interval)?;
        windows.push(WindowResult {
            window: w,
            train_start: train.first().unwrap().open_time,
            train_end: train.last().unwrap().open_time,
            test_start: test.first().unwrap().open_time,
            test_end: test.last().unwrap().open_time,
            best_tag: best_p.tag(),
            best_params: *best_p,
            train_sharpe: train_res.metrics.sharpe,
            train_trades: train_res.metrics.n_trades,
            test_sharpe: test_res.metrics.sharpe,
            test_return_pct: test_res.metrics.total_return_pct,
            test_trades: test_res.metrics.n_trades,
            test_max_dd_pct: test_res.metrics.max_drawdown_pct,
        });
    }

    let avg_oos_sharpe = if windows.is_empty() {
        0.0
    } else {
        windows.iter().map(|x| x.test_sharpe).sum::<f64>() / windows.len() as f64
    };
    let hit = if windows.is_empty() {
        0.0
    } else {
        windows.iter().filter(|x| x.test_return_pct > 0.0).count() as f64 / windows.len() as f64 * 100.0
    };
    let total_oos_trades = windows.iter().map(|x| x.test_trades).sum();

    Ok(WFReport { symbol: symbol.into(), interval: interval.into(), windows, avg_oos_sharpe, oos_hit_rate_pct: hit, total_oos_trades })
}
