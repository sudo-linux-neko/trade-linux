// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Event-driven long-only backtest engine.
//! Execution semantics (no lookahead, institutional standard):
//! - Signals computed on bar `i` CLOSE.
//! - Market fills on bar `i+1` OPEN ± slippage.
//! - ATR stop / take / trailing checked against bar `i+1` HIGH/LOW first
//!   (conservative: stop before take when both hit in one bar).
//! - Fees in bps per side on notional. Slippage in bps against the trader.

use serde::{Deserialize, Serialize};

use crate::metrics::{compute_metrics, Metrics};
use crate::strategy::{entry_signal, exit_signal, position_size, IndicatorState, StrategyParams};
use crate::types::{Bar, EquityPoint, ExitReason, OpenPosition, Trade};

/// Backtest cost + capital config.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub initial_cash: f64,
    /// Binance spot VIP0 ≈ 10 bps taker. Use 10 for realism.
    pub fee_bps: f64,
    /// Extra adverse move on fill, e.g. 5 bps.
    pub slippage_bps: f64,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self { initial_cash: 10_000.0, fee_bps: 10.0, slippage_bps: 5.0 }
    }
}

/// Full result of one backtest run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub params: StrategyParams,
    pub config: BacktestConfig,
    pub symbol: String,
    pub interval: String,
    pub n_bars: usize,
    pub trades: Vec<Trade>,
    pub equity: Vec<EquityPoint>,
    pub metrics: Metrics,
}

fn fee(notional: f64, fee_bps: f64) -> f64 {
    notional * fee_bps / 10_000.0
}

fn buy_fill_price(next_open: f64, slippage_bps: f64) -> f64 {
    next_open * (1.0 + slippage_bps / 10_000.0)
}

fn sell_fill_price(next_open: f64, slippage_bps: f64) -> f64 {
    next_open * (1.0 - slippage_bps / 10_000.0)
}

/// Run the engine. Pure + deterministic: same bars+params => same result.
pub fn run_backtest(
    bars: &[Bar],
    params: &StrategyParams,
    config: &BacktestConfig,
    symbol: &str,
    interval: &str,
) -> anyhow::Result<BacktestResult> {
    anyhow::ensure!(bars.len() >= 100, "need >= 100 bars, got {}", bars.len());
    params.validate()?;

    let ind = IndicatorState::compute(bars, params);
    let fee_bps = config.fee_bps;
    let slip = config.slippage_bps;

    let mut cash = config.initial_cash;
    let mut position: Option<OpenPosition> = None;
    let mut trades: Vec<Trade> = Vec::new();
    let mut equity: Vec<EquityPoint> = Vec::with_capacity(bars.len());
    let mut cooldown_until: usize = 0;

    // Pending market orders decided on close of bar i, filled at open of i+1.
    // Only one pending at a time (single-position long-only).
    enum Pending {
        Enter,
        Exit(ExitReason),
    }
    let mut pending: Option<Pending> = None;

    for i in 0..bars.len() {
        let bar = &bars[i];

        // ---- 1) process pending fill at this bar's OPEN ----
        if let Some(p) = pending.take() {
            match p {
                Pending::Enter => {
                    // Recompute sizing at fill time using ATR known at decision bar (i-1).
                    let atr = ind.atr[i - 1].unwrap_or(bar.open * 0.01);
                    let ref_price = buy_fill_price(bar.open, slip);
                    // equity for sizing = cash (flat) — we only enter when flat.
                    let eq_now = cash;
                    let qty = position_size(eq_now, ref_price, ref_price - params.stop_mult * atr, params);
                    if qty > 0.0 {
                        let notional = qty * ref_price;
                        if notional <= cash {
                            let f = fee(notional, fee_bps);
                            cash -= notional + f;
                            // Align stop/take to actual fill (keeps risk geometry).
                            let stop = ref_price - params.stop_mult * atr;
                            let take = ref_price + params.take_mult * atr;
                            position = Some(OpenPosition {
                                entry_idx: i,
                                entry_time: bar.open_time,
                                entry_price: ref_price,
                                qty,
                                entry_fee: f,
                                stop,
                                take,
                                trailing_stop: None,
                                highest_since_entry: bar.high.max(ref_price),
                            });
                        }
                    }
                }
                Pending::Exit(reason) => {
                    if let Some(pos) = position.take() {
                        let fill = sell_fill_price(bar.open, slip);
                        let proceeds = pos.qty * fill;
                        let f = fee(proceeds, fee_bps);
                        cash += proceeds - f;
                        let entry_notional = pos.qty * pos.entry_price;
                        let pnl = proceeds - f - entry_notional - pos.entry_fee;
                        trades.push(Trade {
                            entry_idx: pos.entry_idx,
                            entry_time: pos.entry_time,
                            entry_price: pos.entry_price,
                            exit_idx: i,
                            exit_time: bar.open_time,
                            exit_price: fill,
                            qty: pos.qty,
                            entry_fee: pos.entry_fee,
                            exit_fee: f,
                            pnl,
                            pnl_pct: if entry_notional > 0.0 { pnl / entry_notional } else { 0.0 },
                            bars_held: i.saturating_sub(pos.entry_idx),
                            exit_reason: reason,
                        });
                        cooldown_until = i + params.cooldown_bars;
                    }
                }
            }
        }

        // ---- 2) intrabar stop / take / trailing check (only if holding entering THIS bar) ----
        // Uses this bar's high/low. Conservative order: stop -> trailing -> take.
        // Fills at the stop/take price with adverse slippage.
        if let Some(mut pos) = position.take() {
            // Update highest + trailing level from this bar so far.
            pos.highest_since_entry = pos.highest_since_entry.max(bar.high);
            if params.trailing_mult > 0.0 {
                if let Some(atr_now) = ind.atr.get(i).and_then(|x| *x) {
                    let trail = pos.highest_since_entry - params.trailing_mult * atr_now;
                    // Only ratchet up, never below initial stop.
                    let base = pos.trailing_stop.unwrap_or(pos.stop);
                    pos.trailing_stop = Some(trail.max(base).max(pos.stop));
                }
            }

            // Stop-loss hit?
            if bar.low <= pos.stop {
                let fill = pos.stop * (1.0 - slip / 10_000.0);
                let proceeds = pos.qty * fill;
                let f = fee(proceeds, fee_bps);
                cash += proceeds - f;
                let entry_notional = pos.qty * pos.entry_price;
                let pnl = proceeds - f - entry_notional - pos.entry_fee;
                trades.push(Trade {
                    entry_idx: pos.entry_idx,
                    entry_time: pos.entry_time,
                    entry_price: pos.entry_price,
                    exit_idx: i,
                    exit_time: bar.open_time,
                    exit_price: fill,
                    qty: pos.qty,
                    entry_fee: pos.entry_fee,
                    exit_fee: f,
                    pnl,
                    pnl_pct: if entry_notional > 0.0 { pnl / entry_notional } else { 0.0 },
                    bars_held: i.saturating_sub(pos.entry_idx),
                    exit_reason: ExitReason::StopLoss,
                });
                cooldown_until = i + params.cooldown_bars;
                position = None;
            } else if let Some(ts) = pos.trailing_stop {
                if bar.low <= ts && i.saturating_sub(pos.entry_idx) >= 1 {
                    let fill = ts * (1.0 - slip / 10_000.0);
                    let proceeds = pos.qty * fill;
                    let f = fee(proceeds, fee_bps);
                    cash += proceeds - f;
                    let entry_notional = pos.qty * pos.entry_price;
                    let pnl = proceeds - f - entry_notional - pos.entry_fee;
                    trades.push(Trade {
                        entry_idx: pos.entry_idx,
                        entry_time: pos.entry_time,
                        entry_price: pos.entry_price,
                        exit_idx: i,
                        exit_time: bar.open_time,
                        exit_price: fill,
                        qty: pos.qty,
                        entry_fee: pos.entry_fee,
                        exit_fee: f,
                        pnl,
                        pnl_pct: if entry_notional > 0.0 { pnl / entry_notional } else { 0.0 },
                        bars_held: i.saturating_sub(pos.entry_idx),
                        exit_reason: ExitReason::TrailingStop,
                    });
                    cooldown_until = i + params.cooldown_bars;
                    position = None;
                } else if bar.high >= pos.take {
                    let fill = pos.take * (1.0 - slip / 10_000.0);
                    let proceeds = pos.qty * fill;
                    let f = fee(proceeds, fee_bps);
                    cash += proceeds - f;
                    let entry_notional = pos.qty * pos.entry_price;
                    let pnl = proceeds - f - entry_notional - pos.entry_fee;
                    trades.push(Trade {
                        entry_idx: pos.entry_idx,
                        entry_time: pos.entry_time,
                        entry_price: pos.entry_price,
                        exit_idx: i,
                        exit_time: bar.open_time,
                        exit_price: fill,
                        qty: pos.qty,
                        entry_fee: pos.entry_fee,
                        exit_fee: f,
                        pnl,
                        pnl_pct: if entry_notional > 0.0 { pnl / entry_notional } else { 0.0 },
                        bars_held: i.saturating_sub(pos.entry_idx),
                        exit_reason: ExitReason::TakeProfit,
                    });
                    cooldown_until = i + params.cooldown_bars;
                    position = None;
                } else {
                    position = Some(pos);
                }
            } else if bar.high >= pos.take {
                let fill = pos.take * (1.0 - slip / 10_000.0);
                let proceeds = pos.qty * fill;
                let f = fee(proceeds, fee_bps);
                cash += proceeds - f;
                let entry_notional = pos.qty * pos.entry_price;
                let pnl = proceeds - f - entry_notional - pos.entry_fee;
                trades.push(Trade {
                    entry_idx: pos.entry_idx,
                    entry_time: pos.entry_time,
                    entry_price: pos.entry_price,
                    exit_idx: i,
                    exit_time: bar.open_time,
                    exit_price: fill,
                    qty: pos.qty,
                    entry_fee: pos.entry_fee,
                    exit_fee: f,
                    pnl,
                    pnl_pct: if entry_notional > 0.0 { pnl / entry_notional } else { 0.0 },
                    bars_held: i.saturating_sub(pos.entry_idx),
                    exit_reason: ExitReason::TakeProfit,
                });
                cooldown_until = i + params.cooldown_bars;
                position = None;
            } else {
                position = Some(pos);
            }
        }

        // ---- 3) mark to market + record equity ----
        let pos_qty = position.as_ref().map(|p| p.qty).unwrap_or(0.0);
        let pos_val = position.as_ref().map(|p| p.qty * bar.close).unwrap_or(0.0);
        let eq = cash + pos_val;
        equity.push(EquityPoint { time: bar.open_time, equity: eq, close: bar.close, position_qty: pos_qty });

        // ---- 4) compute signal on CLOSE for next bar (no lookahead) ----
        if i + 1 < bars.len() && pending.is_none() {
            if position.is_some() {
                let pos_entry = position.as_ref().unwrap().entry_idx;
                if let Some(reason) = exit_signal(i, pos_entry, &ind, params) {
                    pending = Some(Pending::Exit(reason));
                }
            } else if i + 1 >= cooldown_until && i >= ind.warmup && entry_signal(i, &ind, params) {
                pending = Some(Pending::Enter);
            }
        }
    }

    // ---- 5) force flat at end ----
    if let Some(pos) = position.take() {
        let last = bars.last().unwrap();
        let fill = last.close * (1.0 - slip / 10_000.0);
        let proceeds = pos.qty * fill;
        let f = fee(proceeds, fee_bps);
        cash += proceeds - f;
        let entry_notional = pos.qty * pos.entry_price;
        let pnl = proceeds - f - entry_notional - pos.entry_fee;
        trades.push(Trade {
            entry_idx: pos.entry_idx,
            entry_time: pos.entry_time,
            entry_price: pos.entry_price,
            exit_idx: bars.len() - 1,
            exit_time: last.open_time,
            exit_price: fill,
            qty: pos.qty,
            entry_fee: pos.entry_fee,
            exit_fee: f,
            pnl,
            pnl_pct: if entry_notional > 0.0 { pnl / entry_notional } else { 0.0 },
            bars_held: bars.len().saturating_sub(pos.entry_idx + 1),
            exit_reason: ExitReason::EndOfData,
        });
        if let Some(last_eq) = equity.last_mut() {
            last_eq.equity = cash;
            last_eq.position_qty = 0.0;
        }
    }

    let bars_per_year = crate::data::bars_per_year(interval);
    let metrics = compute_metrics(&trades, &equity, config.initial_cash, bars_per_year);

    Ok(BacktestResult {
        params: *params,
        config: *config,
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        n_bars: bars.len(),
        trades,
        equity,
        metrics,
    })
}
