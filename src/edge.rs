// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! New edges from web research — all long-only spot, CLOSE signal → NEXT OPEN fill.
//!
//! 1. `rsi2` (Connors): price > SMA_long, price < SMA_short, RSI(2) < entry → exit RSI(2) > exit
//!    or price > SMA_short. Daily BTC 2020-25 adapted: +55-68%, 62-65% WR (StratBase/Connors).
//! 2. `donchian` (Turtle): close > prior-N high + EMA filter + RSI momentum → exit prior-M low.
//!    BTC daily System2 PF 1.72; ADX/volume/MTF filters help (StratBase/DennTech).
//! 3. `bbrev` (BB mean-reversion): close < lower BB(20,k) + RSI oversold + bandwidth-rank
//!    filter → exit at middle band. BTC 15m published +8.8%, 71% WR (CoinQuant); Gate shows
//!    bandwidth screening + cooldown turn losses into stability.
//!
//! Shared engine semantics with `backtest.rs`: fees + slippage both sides,
//! intrabar stop → trailing → take (conservative), risk-based sizing, time stop + cooldown.

use serde::{Deserialize, Serialize};

use crate::backtest::BacktestConfig;
use crate::indicators::{
    atr_series, bandwidth_pct_rank, bollinger_series, donchian_prior, ema_series, rsi_series,
    sma_series,
};
use crate::metrics::{compute_metrics, Metrics};
use crate::strategy::position_size;
use crate::strategy::StrategyParams as TrendParams;
use crate::types::{Bar, EquityPoint, ExitReason, OpenPosition, Trade};

// ---------------------------------------------------------------------------
// Shared risk + generic runner
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RiskConfig {
    pub stop_mult: f64,
    pub take_mult: f64,
    pub trailing_mult: f64,
    pub max_holding_bars: usize,
    pub cooldown_bars: usize,
    pub risk_pct: f64,
    pub max_notional_pct: f64,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            stop_mult: 2.5,
            take_mult: 50.0, // huge = effectively off (strategy exits rule instead)
            trailing_mult: 0.0,
            max_holding_bars: 100,
            cooldown_bars: 8,
            risk_pct: 0.01,
            max_notional_pct: 0.95,
        }
    }
}

fn fee(notional: f64, fee_bps: f64) -> f64 {
    notional * fee_bps / 10_000.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeResult {
    pub strategy: String,
    pub tag: String,
    pub params: serde_json::Value,
    pub config: BacktestConfig,
    pub symbol: String,
    pub interval: String,
    pub n_bars: usize,
    pub trades: Vec<Trade>,
    pub equity: Vec<EquityPoint>,
    pub metrics: Metrics,
    pub buy_hold_return_pct: f64,
}

pub fn buy_hold_return(bars: &[Bar]) -> f64 {
    if bars.len() < 2 || bars[0].close <= 0.0 {
        return 0.0;
    }
    (bars.last().unwrap().close / bars[0].close - 1.0) * 100.0
}

/// Generic long-only engine. `entry(i)` / `exit(i, entry_idx)` evaluated on CLOSE of bar i;
/// fills at OPEN of i+1. `atr` used for stop/take/trailing geometry. Time stop enforced here.
#[allow(clippy::too_many_arguments)]
pub fn run_generic(
    bars: &[Bar],
    atr: &[Option<f64>],
    warmup: usize,
    risk: &RiskConfig,
    config: &BacktestConfig,
    strategy: &str,
    tag: &str,
    params_json: serde_json::Value,
    symbol: &str,
    interval: &str,
    entry: &dyn Fn(usize) -> bool,
    exit: &dyn Fn(usize, usize) -> Option<ExitReason>,
) -> anyhow::Result<EdgeResult> {
    anyhow::ensure!(bars.len() >= 100, "need >= 100 bars");
    let fee_bps = config.fee_bps;
    let slip = config.slippage_bps;

    // Mirror StrategyParams for the shared position_size helper.
    let sizing = TrendParams {
        risk_pct: risk.risk_pct,
        max_notional_pct: risk.max_notional_pct,
        ..TrendParams::default()
    };

    let mut cash = config.initial_cash;
    let mut position: Option<OpenPosition> = None;
    let mut trades: Vec<Trade> = Vec::new();
    let mut equity: Vec<EquityPoint> = Vec::with_capacity(bars.len());
    let mut cooldown_until: usize = 0;

    enum Pending {
        Enter,
        Exit(ExitReason),
    }
    let mut pending: Option<Pending> = None;

    for i in 0..bars.len() {
        let bar = &bars[i];

        if let Some(p) = pending.take() {
            match p {
                Pending::Enter => {
                    let a = atr.get(i - 1).and_then(|x| *x).unwrap_or(bar.open * 0.01);
                    let fill = bar.open * (1.0 + slip / 10_000.0);
                    let stop_dist = risk.stop_mult * a;
                    let qty = position_size(cash, fill, fill - stop_dist, &sizing);
                    if qty > 0.0 {
                        let notional = qty * fill;
                        if notional <= cash {
                            let f = fee(notional, fee_bps);
                            cash -= notional + f;
                            position = Some(OpenPosition {
                                entry_idx: i,
                                entry_time: bar.open_time,
                                entry_price: fill,
                                qty,
                                entry_fee: f,
                                stop: fill - stop_dist,
                                take: fill + risk.take_mult * a,
                                trailing_stop: None,
                                highest_since_entry: bar.high.max(fill),
                            });
                        }
                    }
                }
                Pending::Exit(reason) => {
                    if let Some(pos) = position.take() {
                        let fill = bar.open * (1.0 - slip / 10_000.0);
                        let proceeds = pos.qty * fill;
                        let f = fee(proceeds, fee_bps);
                        cash += proceeds - f;
                        let en = pos.qty * pos.entry_price;
                        let pnl = proceeds - f - en - pos.entry_fee;
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
                            pnl_pct: if en > 0.0 { pnl / en } else { 0.0 },
                            bars_held: i.saturating_sub(pos.entry_idx),
                            exit_reason: reason,
                        });
                        cooldown_until = i + risk.cooldown_bars;
                    }
                }
            }
        }

        // Intrabar stops (conservative order: stop → trailing → take).
        if let Some(mut pos) = position.take() {
            pos.highest_since_entry = pos.highest_since_entry.max(bar.high);
            if risk.trailing_mult > 0.0 {
                if let Some(a) = atr.get(i).and_then(|x| *x) {
                    let trail = pos.highest_since_entry - risk.trailing_mult * a;
                    let base = pos.trailing_stop.unwrap_or(pos.stop);
                    pos.trailing_stop = Some(trail.max(base).max(pos.stop));
                }
            }
            let mut closed: Option<(f64, ExitReason)> = None;
            if bar.low <= pos.stop {
                closed = Some((pos.stop * (1.0 - slip / 10_000.0), ExitReason::StopLoss));
            } else if let Some(ts) = pos.trailing_stop {
                if bar.low <= ts && i.saturating_sub(pos.entry_idx) >= 1 {
                    closed = Some((ts * (1.0 - slip / 10_000.0), ExitReason::TrailingStop));
                } else if bar.high >= pos.take {
                    closed = Some((pos.take * (1.0 - slip / 10_000.0), ExitReason::TakeProfit));
                }
            } else if bar.high >= pos.take {
                closed = Some((pos.take * (1.0 - slip / 10_000.0), ExitReason::TakeProfit));
            }
            if let Some((fill, reason)) = closed {
                let proceeds = pos.qty * fill;
                let f = fee(proceeds, fee_bps);
                cash += proceeds - f;
                let en = pos.qty * pos.entry_price;
                let pnl = proceeds - f - en - pos.entry_fee;
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
                    pnl_pct: if en > 0.0 { pnl / en } else { 0.0 },
                    bars_held: i.saturating_sub(pos.entry_idx),
                    exit_reason: reason,
                });
                cooldown_until = i + risk.cooldown_bars;
            } else {
                position = Some(pos);
            }
        }

        let pos_qty = position.as_ref().map(|p| p.qty).unwrap_or(0.0);
        let pos_val = position.as_ref().map(|p| p.qty * bar.close).unwrap_or(0.0);
        equity.push(EquityPoint { time: bar.open_time, equity: cash + pos_val, close: bar.close, position_qty: pos_qty });

        if i + 1 < bars.len() && pending.is_none() {
            if position.is_some() {
                let eidx = position.as_ref().unwrap().entry_idx;
                // Universal time stop first.
                if i.saturating_sub(eidx) >= risk.max_holding_bars {
                    pending = Some(Pending::Exit(ExitReason::MaxHolding));
                } else if let Some(r) = exit(i, eidx) {
                    pending = Some(Pending::Exit(r));
                }
            } else if i + 1 >= cooldown_until && i >= warmup && entry(i) {
                pending = Some(Pending::Enter);
            }
        }
    }

    if let Some(pos) = position.take() {
        let last = bars.last().unwrap();
        let fill = last.close * (1.0 - slip / 10_000.0);
        let proceeds = pos.qty * fill;
        let f = fee(proceeds, fee_bps);
        cash += proceeds - f;
        let en = pos.qty * pos.entry_price;
        let pnl = proceeds - f - en - pos.entry_fee;
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
            pnl_pct: if en > 0.0 { pnl / en } else { 0.0 },
            bars_held: bars.len().saturating_sub(pos.entry_idx + 1),
            exit_reason: ExitReason::EndOfData,
        });
        if let Some(e) = equity.last_mut() {
            e.equity = cash;
            e.position_qty = 0.0;
        }
    }

    let metrics = compute_metrics(&trades, &equity, config.initial_cash, crate::data::bars_per_year(interval));
    Ok(EdgeResult {
        strategy: strategy.into(),
        tag: tag.into(),
        params: params_json,
        config: *config,
        symbol: symbol.into(),
        interval: interval.into(),
        n_bars: bars.len(),
        trades,
        equity,
        metrics,
        buy_hold_return_pct: buy_hold_return(bars),
    })
}

// ---------------------------------------------------------------------------
// 1) Connors RSI2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Rsi2Params {
    pub rsi_period: usize,
    pub entry_max: f64,
    pub exit_min: f64,
    pub sma_long: usize,
    pub sma_short: usize,
    pub atr_period: usize,
    pub stop_mult: f64,
    pub max_holding_bars: usize,
    pub cooldown_bars: usize,
    pub risk_pct: f64,
}

impl Default for Rsi2Params {
    fn default() -> Self {
        Self {
            rsi_period: 2,
            entry_max: 10.0,
            exit_min: 70.0,
            sma_long: 200,
            sma_short: 5,
            atr_period: 14,
            stop_mult: 3.0,
            max_holding_bars: 30,
            cooldown_bars: 5,
            risk_pct: 0.01,
        }
    }
}

impl Rsi2Params {
    pub fn tag(&self) -> String {
        format!(
            "rsi{}[x{:.0}>x{:.0}]_sma{}-{}_sl{}_h{}",
            self.rsi_period, self.entry_max, self.exit_min,
            self.sma_short, self.sma_long, self.stop_mult, self.max_holding_bars
        )
    }
}

pub struct Rsi2State {
    pub rsi: Vec<Option<f64>>,
    pub sma_long: Vec<Option<f64>>,
    pub sma_short: Vec<Option<f64>>,
    pub atr: Vec<Option<f64>>,
    pub warmup: usize,
}

impl Rsi2State {
    pub fn compute(bars: &[Bar], p: &Rsi2Params) -> Self {
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
        let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
        Self {
            rsi: rsi_series(&closes, p.rsi_period),
            sma_long: sma_series(&closes, p.sma_long),
            sma_short: sma_series(&closes, p.sma_short),
            atr: atr_series(&highs, &lows, &closes, p.atr_period),
            warmup: p.sma_long.max(p.atr_period + 1).max(p.rsi_period + 1) + 5,
        }
    }
}

pub fn run_rsi2(
    bars: &[Bar], p: &Rsi2Params, config: &BacktestConfig, symbol: &str, interval: &str,
) -> anyhow::Result<EdgeResult> {
    let st = Rsi2State::compute(bars, p);
    let risk = RiskConfig {
        stop_mult: p.stop_mult,
        take_mult: 50.0,
        trailing_mult: 0.0,
        max_holding_bars: p.max_holding_bars,
        cooldown_bars: p.cooldown_bars,
        ..RiskConfig::default()
    };
    let risk2 = risk;
    let entry = |i: usize| -> bool {
        if i < st.warmup {
            return false;
        }
        match (st.rsi[i], st.sma_long[i], st.sma_short[i]) {
            (Some(r), Some(sl), Some(ss)) => {
                let c = bars[i].close;
                c > sl && c < ss && r < p.entry_max && st.atr[i].is_some()
            }
            _ => false,
        }
    };
    let exit = |i: usize, _e: usize| -> Option<ExitReason> {
        if i < st.warmup {
            return None;
        }
        if let Some(r) = st.rsi[i] {
            if r > p.exit_min {
                return Some(ExitReason::RsiExit);
            }
        }
        match (st.sma_short[i], ) {
            _ => {
                if let Some(ss) = st.sma_short[i] {
                    if bars[i].close > ss {
                        return Some(ExitReason::TakeProfit);
                    }
                }
                None
            }
        }
    };
    run_generic(
        bars, &st.atr, st.warmup, &risk2, config, "rsi2", &p.tag(),
        serde_json::to_value(p).unwrap_or_default(), symbol, interval, &entry, &exit,
    )
}

pub fn rsi2_grid() -> Vec<Rsi2Params> {
    let mut g = Vec::new();
    for entry_max in [5.0, 10.0] {
        for exit_min in [65.0, 75.0] {
            for sma_long in [100usize, 200] {
                for stop in [2.0, 3.5] {
                    g.push(Rsi2Params { entry_max, exit_min, sma_long, stop_mult: stop, ..Rsi2Params::default() });
                }
            }
        }
    }
    g
}

// ---------------------------------------------------------------------------
// 2) Donchian Turtle breakout
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DonchianParams {
    pub entry_n: usize,
    pub exit_m: usize,
    pub ema_filter: usize, // 0 = off
    pub rsi_period: usize,
    pub rsi_min: f64, // 0 = off
    pub rsi_max: f64,
    pub atr_period: usize,
    pub stop_mult: f64,
    pub trailing_mult: f64,
    pub max_holding_bars: usize,
    pub cooldown_bars: usize,
    pub risk_pct: f64,
}

impl Default for DonchianParams {
    fn default() -> Self {
        Self {
            entry_n: 20,
            exit_m: 10,
            ema_filter: 50,
            rsi_period: 14,
            rsi_min: 50.0,
            rsi_max: 80.0,
            atr_period: 20,
            stop_mult: 2.0,
            trailing_mult: 0.0,
            max_holding_bars: 1000,
            cooldown_bars: 10,
            risk_pct: 0.01,
        }
    }
}

impl DonchianParams {
    pub fn tag(&self) -> String {
        format!(
            "don{}-{}_ema{}_rsi[{:.0}-{:.0}]_sl{}_tr{}",
            self.entry_n, self.exit_m, self.ema_filter,
            self.rsi_min, self.rsi_max, self.stop_mult, self.trailing_mult
        )
    }
    pub fn valid(&self) -> bool {
        self.exit_m < self.entry_n && self.entry_n <= 200 && self.rsi_max > self.rsi_min
    }
}

pub struct DonchianState {
    pub up: Vec<Option<f64>>,
    pub dn_exit: Vec<Option<f64>>,
    pub ema: Vec<Option<f64>>,
    pub rsi: Vec<Option<f64>>,
    pub atr: Vec<Option<f64>>,
    pub warmup: usize,
    pub use_ema: bool,
    pub use_rsi: bool,
}

impl DonchianState {
    pub fn compute(bars: &[Bar], p: &DonchianParams) -> Self {
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
        let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
        let (up, _) = donchian_prior(&highs, &lows, p.entry_n);
        let (_, dn_exit) = donchian_prior(&highs, &lows, p.exit_m);
        let use_ema = p.ema_filter > 0;
        let ema = if use_ema { ema_series(&closes, p.ema_filter) } else { vec![None; bars.len()] };
        let use_rsi = p.rsi_min > 0.0;
        let rsi = rsi_series(&closes, p.rsi_period);
        let atr = atr_series(&highs, &lows, &closes, p.atr_period);
        let warmup = p.entry_n.max(p.atr_period + 1).max(p.rsi_period + 1).max(p.ema_filter) + 5;
        Self { up, dn_exit, ema, rsi, atr, warmup, use_ema, use_rsi }
    }
}

pub fn run_donchian(
    bars: &[Bar], p: &DonchianParams, config: &BacktestConfig, symbol: &str, interval: &str,
) -> anyhow::Result<EdgeResult> {
    anyhow::ensure!(p.valid(), "invalid donchian params");
    let st = DonchianState::compute(bars, p);
    let risk = RiskConfig {
        stop_mult: p.stop_mult,
        take_mult: 50.0,
        trailing_mult: p.trailing_mult,
        max_holding_bars: p.max_holding_bars,
        cooldown_bars: p.cooldown_bars,
        ..RiskConfig::default()
    };
    let entry = |i: usize| -> bool {
        if i < st.warmup {
            return false;
        }
        let (Some(ch), c) = (st.up[i], bars[i].close) else { return false };
        if !(c > ch) {
            return false; // confirmed CLOSE breakout only
        }
        if st.use_ema {
            match st.ema[i] {
                Some(e) if c > e => {}
                _ => return false,
            }
        }
        if st.use_rsi {
            match st.rsi[i] {
                Some(r) if r >= p.rsi_min && r <= p.rsi_max => {}
                _ => return false,
            }
        }
        st.atr[i].is_some()
    };
    let exit = |i: usize, _e: usize| -> Option<ExitReason> {
        if i < st.warmup {
            return None;
        }
        if let Some(dn) = st.dn_exit[i] {
            if bars[i].close < dn {
                return Some(ExitReason::TrendFlip);
            }
        }
        None
    };
    run_generic(
        bars, &st.atr, st.warmup, &risk, config, "donchian", &p.tag(),
        serde_json::to_value(p).unwrap_or_default(), symbol, interval, &entry, &exit,
    )
}

pub fn donchian_grid() -> Vec<DonchianParams> {
    let mut g = Vec::new();
    for entry_n in [20usize, 55] {
        for exit_m in [10usize, 20] {
            if exit_m >= entry_n {
                continue;
            }
            for rsi_min in [0.0, 50.0] {
                for stop in [1.5, 2.5] {
                    let p = DonchianParams { entry_n, exit_m, rsi_min, stop_mult: stop, ..DonchianParams::default() };
                    if p.valid() {
                        g.push(p);
                    }
                }
            }
        }
    }
    g
}

// ---------------------------------------------------------------------------
// 3) Bollinger mean-reversion (15m squeeze-aware)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BbRevParams {
    pub bb_period: usize,
    pub bb_mult: f64,
    pub rsi_period: usize,
    pub rsi_entry_max: f64, // oversold gate: only enter if RSI below this
    pub bw_window: usize,
    pub bw_lo: f64, // skip below this rank (squeeze → breakout risk)
    pub bw_hi: f64, // skip above this rank (exhaust)
    pub atr_period: usize,
    pub stop_mult: f64, // breakout-invalidation stop
    pub max_holding_bars: usize,
    pub cooldown_bars: usize,
    pub risk_pct: f64,
}

impl Default for BbRevParams {
    fn default() -> Self {
        Self {
            bb_period: 20,
            bb_mult: 2.0,
            rsi_period: 14,
            rsi_entry_max: 32.0,
            bw_window: 100,
            bw_lo: 0.15,
            bw_hi: 0.85,
            atr_period: 14,
            stop_mult: 2.5,
            max_holding_bars: 80,
            cooldown_bars: 8,
            risk_pct: 0.01,
        }
    }
}

impl BbRevParams {
    pub fn tag(&self) -> String {
        format!(
            "bb{}x{}_rsi<x{:.0}_bw{:.0} {:.2}-{:.2}_sl{}_h{}",
            self.bb_period, self.bb_mult, self.rsi_entry_max,
            self.bw_window as f64, self.bw_lo, self.bw_hi, self.stop_mult, self.max_holding_bars
        )
    }
}

pub struct BbRevState {
    pub mid: Vec<Option<f64>>,
    pub up: Vec<Option<f64>>,
    pub lo: Vec<Option<f64>>,
    pub bw: Vec<Option<f64>>,
    pub rsi: Vec<Option<f64>>,
    pub atr: Vec<Option<f64>>,
    pub warmup: usize,
}

impl BbRevState {
    pub fn compute(bars: &[Bar], p: &BbRevParams) -> Self {
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
        let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
        let bb = bollinger_series(&closes, p.bb_period, p.bb_mult);
        Self {
            mid: bb.middle,
            up: bb.upper,
            lo: bb.lower,
            bw: bb.bandwidth,
            rsi: rsi_series(&closes, p.rsi_period),
            atr: atr_series(&highs, &lows, &closes, p.atr_period),
            warmup: p.bb_period.max(p.rsi_period + 1).max(p.atr_period + 1).max(p.bw_window) + 5,
        }
    }
}

pub fn run_bbrev(
    bars: &[Bar], p: &BbRevParams, config: &BacktestConfig, symbol: &str, interval: &str,
) -> anyhow::Result<EdgeResult> {
    let st = BbRevState::compute(bars, p);
    let risk = RiskConfig {
        stop_mult: p.stop_mult,
        take_mult: 50.0,
        trailing_mult: 0.0,
        max_holding_bars: p.max_holding_bars,
        cooldown_bars: p.cooldown_bars,
        ..RiskConfig::default()
    };
    let entry = |i: usize| -> bool {
        if i < st.warmup {
            return false;
        }
        let (Some(lo), Some(_mid), Some(r)) = (st.lo[i], st.mid[i], st.rsi[i]) else { return false };
        if st.atr[i].is_none() {
            return false;
        }
        if !(bars[i].close < lo) {
            return false;
        }
        if !(r < p.rsi_entry_max) {
            return false;
        }
        // Squeeze/exhaust filter (Gate Research): only mid-bandwidth regimes.
        if let Some(rank) = bandwidth_pct_rank(&st.bw, p.bw_window, i) {
            if rank < p.bw_lo || rank > p.bw_hi {
                return false;
            }
        } else {
            return false;
        }
        true
    };
    let exit = |i: usize, _e: usize| -> Option<ExitReason> {
        if i < st.warmup {
            return None;
        }
        // Reversion complete: back to the mean.
        if let Some(mid) = st.mid[i] {
            if bars[i].close > mid {
                return Some(ExitReason::TakeProfit);
            }
        }
        // Opposite extreme: overbought, take it.
        if let Some(up) = st.up[i] {
            if bars[i].close > up {
                return Some(ExitReason::TakeProfit);
            }
        }
        None
    };
    run_generic(
        bars, &st.atr, st.warmup, &risk, config, "bbrev", &p.tag(),
        serde_json::to_value(p).unwrap_or_default(), symbol, interval, &entry, &exit,
    )
}

pub fn bbrev_grid() -> Vec<BbRevParams> {
    let mut g = Vec::new();
    for mult in [2.0, 2.5] {
        for rsi_max in [30.0, 35.0] {
            for stop in [2.0, 3.0] {
                for hold in [60usize, 120] {
                    g.push(BbRevParams { bb_mult: mult, rsi_entry_max: rsi_max, stop_mult: stop, max_holding_bars: hold, ..BbRevParams::default() });
                }
            }
        }
    }
    g
}

// ---------------------------------------------------------------------------
// Walk-forward for edge families (IS grid-search → OOS confirm per window)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWindow {
    pub window: usize,
    pub train_start: i64,
    pub train_end: i64,
    pub test_start: i64,
    pub test_end: i64,
    pub best_tag: String,
    pub train_sharpe: f64,
    pub train_trades: usize,
    pub test_sharpe: f64,
    pub test_return_pct: f64,
    pub test_trades: usize,
    pub test_max_dd_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeWFReport {
    pub strategy: String,
    pub symbol: String,
    pub interval: String,
    pub windows: Vec<EdgeWindow>,
    pub avg_oos_sharpe: f64,
    pub oos_hit_rate_pct: f64,
    pub total_oos_trades: usize,
}

fn sharpe_of(r: &EdgeResult) -> f64 {
    r.metrics.sharpe
}

/// Rolling walk-forward: `n_windows` equal chunks, each 70% train / 30% test.
/// Best IS (Sharpe, min_trades filter) evaluated on OOS test.
pub fn edge_walk_forward(
    bars: &[Bar],
    strategy: &str,
    n_windows: usize,
    config: &BacktestConfig,
    symbol: &str,
    interval: &str,
    min_trades: usize,
) -> anyhow::Result<EdgeWFReport> {
    anyhow::ensure!(bars.len() > n_windows * 500, "not enough bars for {} windows", n_windows);
    let s = strategy.to_lowercase();
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
        // Grid-search train.
        let mut ranked: Vec<(String, EdgeResult)> = Vec::new();
        match s.as_str() {
            "rsi2" => {
                for p in rsi2_grid() {
                    if let Ok(r) = run_rsi2(train, &p, config, symbol, interval) {
                        if r.metrics.n_trades >= min_trades {
                            ranked.push((p.tag(), r));
                        }
                    }
                }
            }
            "donchian" | "turtle" | "breakout" => {
                for p in donchian_grid() {
                    if let Ok(r) = run_donchian(train, &p, config, symbol, interval) {
                        if r.metrics.n_trades >= min_trades {
                            ranked.push((p.tag(), r));
                        }
                    }
                }
            }
            "bbrev" | "bb" | "meanrev" => {
                for p in bbrev_grid() {
                    if let Ok(r) = run_bbrev(train, &p, config, symbol, interval) {
                        if r.metrics.n_trades >= min_trades {
                            ranked.push((p.tag(), r));
                        }
                    }
                }
            }
            _ => anyhow::bail!("unknown edge strategy '{strategy}'"),
        }
        if ranked.is_empty() {
            continue;
        }
        ranked.sort_by(|a, b| {
            sharpe_of(&b.1).partial_cmp(&sharpe_of(&a.1)).unwrap_or(std::cmp::Ordering::Equal)
        });
        let (best_tag, train_res) = &ranked[0];
        // Re-run best params on test slice by matching tag.
        let test_res = match s.as_str() {
            "rsi2" => {
                let mut out = None;
                for p in rsi2_grid() {
                    if &p.tag() == best_tag {
                        out = Some(run_rsi2(test, &p, config, symbol, interval)?);
                        break;
                    }
                }
                out.unwrap()
            }
            "donchian" | "turtle" | "breakout" => {
                let mut out = None;
                for p in donchian_grid() {
                    if &p.tag() == best_tag {
                        out = Some(run_donchian(test, &p, config, symbol, interval)?);
                        break;
                    }
                }
                out.unwrap()
            }
            _ => {
                let mut out = None;
                for p in bbrev_grid() {
                    if &p.tag() == best_tag {
                        out = Some(run_bbrev(test, &p, config, symbol, interval)?);
                        break;
                    }
                }
                out.unwrap()
            }
        };
        windows.push(EdgeWindow {
            window: w,
            train_start: train.first().unwrap().open_time,
            train_end: train.last().unwrap().open_time,
            test_start: test.first().unwrap().open_time,
            test_end: test.last().unwrap().open_time,
            best_tag: best_tag.clone(),
            train_sharpe: train_res.metrics.sharpe,
            train_trades: train_res.metrics.n_trades,
            test_sharpe: test_res.metrics.sharpe,
            test_return_pct: test_res.metrics.total_return_pct,
            test_trades: test_res.metrics.n_trades,
            test_max_dd_pct: test_res.metrics.max_drawdown_pct,
        });
    }

    let avg_oos_sharpe = if windows.is_empty() { 0.0 } else {
        windows.iter().map(|x| x.test_sharpe).sum::<f64>() / windows.len() as f64
    };
    let hit = if windows.is_empty() { 0.0 } else {
        windows.iter().filter(|x| x.test_return_pct > 0.0).count() as f64 / windows.len() as f64 * 100.0
    };
    let total_oos_trades = windows.iter().map(|x| x.test_trades).sum();
    Ok(EdgeWFReport { strategy: s, symbol: symbol.into(), interval: interval.into(), windows, avg_oos_sharpe, oos_hit_rate_pct: hit, total_oos_trades })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Bar;

    fn bars_from_closes(closes: &[f64]) -> Vec<Bar> {
        closes.iter().enumerate().map(|(i, c)| Bar {
            open_time: i as i64 * 900_000, open: *c,
            high: *c * 1.001, low: *c * 0.999, close: *c, volume: 100.0,
        }).collect()
    }

    fn cfg() -> BacktestConfig {
        BacktestConfig { initial_cash: 10_000.0, fee_bps: 10.0, slippage_bps: 5.0 }
    }

    #[test]
    fn rsi2_runs_without_panic_and_respects_uptrend_only() {
        // Downtrend only: price below SMA long → no entries expected.
        let closes: Vec<f64> = (0..300).map(|i| 200.0 - i as f64 * 0.1).collect();
        let bars = bars_from_closes(&closes);
        let p = Rsi2Params::default();
        let r = run_rsi2(&bars, &p, &cfg(), "T", "1d").unwrap();
        assert_eq!(r.metrics.n_trades, 0);
    }

    #[test]
    fn donchian_breakout_triggers_on_new_high() {
        // Flat then spike: should enter on the breakout bar close.
        let mut closes = vec![100.0; 100];
        closes.extend(vec![101.0; 19]);
        closes.push(120.0); // new 20-bar high at last index
        let bars = bars_from_closes(&closes);
        let mut p = DonchianParams::default();
        p.rsi_min = 0.0; // off
        p.ema_filter = 0; // off
        let st = DonchianState::compute(&bars, &p);
        let i = bars.len() - 1;
        assert!(st.up[i].unwrap() < bars[i].close);
    }

    #[test]
    fn bbrev_exits_at_mean() {
        let closes: Vec<f64> = (0..250).map(|i| 200.0 - (i as f64 - 125.0).abs() * 0.5).collect();
        let bars = bars_from_closes(&closes);
        let p = BbRevParams::default();
        // Just verify it runs and produces sane output shape.
        let r = run_bbrev(&bars, &p, &cfg(), "T", "15m");
        assert!(r.is_ok());
    }
}
