// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Long-only trend strategy: EMA(fast) > EMA(slow) + RSI momentum filter + ATR stops.
//! All logic is on CLOSED bars only (no lookahead). Fills happen on the NEXT bar open.

use serde::{Deserialize, Serialize};

use crate::indicators::{crossed_above, crossed_below, ema_series, rsi_series, atr_series};
use crate::types::{Bar, ExitReason};

/// Tunable parameters. Every field is grid-searchable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StrategyParams {
    pub fast: usize,           // fast EMA, e.g. 21
    pub slow: usize,           // slow EMA, e.g. 55
    pub rsi_period: usize,     // e.g. 14
    pub rsi_entry_min: f64,    // momentum gate, e.g. 52 (avoid buying dead trends)
    pub rsi_entry_max: f64,    // chase guard, e.g. 78 (don't buy parabolic top)
    pub rsi_exit: f64,         // momentum died, e.g. 42
    pub atr_period: usize,     // e.g. 14
    pub stop_mult: f64,        // stop = entry - stop_mult * ATR
    pub take_mult: f64,        // take = entry + take_mult * ATR
    pub max_holding_bars: usize, // time stop, e.g. 500 bars on 15m ≈ 5.2 days
    pub cooldown_bars: usize,  // bars to wait after an exit before re-entry
    pub risk_pct: f64,         // % of equity risked per trade (0.01 = 1%)
    pub max_notional_pct: f64, // max % of equity in one position (0.95)
    pub trailing_mult: f64,    // 0 = off; e.g. 3.0 => trail = highest - 3*ATR
    pub cross_lookback: usize, // entry only if fresh cross within N bars (0 = off). 5 kills churn.
}

impl Default for StrategyParams {
    fn default() -> Self {
        Self {
            fast: 21,
            slow: 55,
            rsi_period: 14,
            rsi_entry_min: 52.0,
            rsi_entry_max: 78.0,
            rsi_exit: 42.0,
            atr_period: 14,
            stop_mult: 2.5,
            take_mult: 4.0,
            max_holding_bars: 500,
            cooldown_bars: 10,
            risk_pct: 0.01,
            max_notional_pct: 0.95,
            trailing_mult: 3.0,
            cross_lookback: 5,
        }
    }
}

impl StrategyParams {
    /// Basic sanity: fast < slow, thresholds ordered, etc.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.fast >= 2 && self.slow >= 3, "EMA periods too small");
        anyhow::ensure!(self.fast < self.slow, "need fast < slow");
        anyhow::ensure!(self.slow <= 400, "slow too large for intraday");
        anyhow::ensure!(self.rsi_period >= 2 && self.rsi_period <= 50, "bad rsi_period");
        anyhow::ensure!(
            self.rsi_exit < self.rsi_entry_min && self.rsi_entry_min < self.rsi_entry_max,
            "need rsi_exit < rsi_entry_min < rsi_entry_max"
        );
        anyhow::ensure!(self.stop_mult > 0.3 && self.stop_mult < 10.0, "bad stop_mult");
        anyhow::ensure!(self.take_mult > 0.5 && self.take_mult < 20.0, "bad take_mult");
        anyhow::ensure!(self.risk_pct > 0.0 && self.risk_pct <= 0.05, "risk_pct 0..5%");
        Ok(())
    }

    pub fn tag(&self) -> String {
        format!(
            "e{}-{}_rsi{}[{:.0}-{:.0}|x{:.0}]_atr{}_sl{}_tp{}_h{}_trail{}_x{}",
            self.fast,
            self.slow,
            self.rsi_period,
            self.rsi_entry_min,
            self.rsi_entry_max,
            self.rsi_exit,
            self.atr_period,
            self.stop_mult,
            self.take_mult,
            self.max_holding_bars,
            self.trailing_mult,
            self.cross_lookback
        )
    }
}

/// Precomputed indicator state aligned with bars.
pub struct IndicatorState {
    pub ema_fast: Vec<Option<f64>>,
    pub ema_slow: Vec<Option<f64>>,
    pub rsi: Vec<Option<f64>>,
    pub atr: Vec<Option<f64>>,
    pub warmup: usize,
}

impl IndicatorState {
    pub fn compute(bars: &[Bar], p: &StrategyParams) -> Self {
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
        let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
        let ema_fast = ema_series(&closes, p.fast);
        let ema_slow = ema_series(&closes, p.slow);
        let rsi = rsi_series(&closes, p.rsi_period);
        let atr = atr_series(&highs, &lows, &closes, p.atr_period);
        let warmup = p.slow.max(p.rsi_period + 1).max(p.atr_period + 1) + 5;
        Self { ema_fast, ema_slow, rsi, atr, warmup }
    }
}

/// Entry signal on closed bar `i`. Returns false during warmup or missing data.
pub fn entry_signal(i: usize, ind: &IndicatorState, p: &StrategyParams) -> bool {
    if i < ind.warmup || i >= ind.ema_fast.len() {
        return false;
    }
    let (Some(f), Some(s), Some(r)) = (ind.ema_fast[i], ind.ema_slow[i], ind.rsi[i]) else {
        return false;
    };
    if ind.atr[i].is_none() {
        return false;
    }
    // Trend gate: fast strictly above slow.
    if !(f > s) {
        return false;
    }
    // Fresh-cross gate: require a recent fast-over-slow cross within lookback.
    // Without this, 15m chops you into 6000+ fees-driven trades. This is the
    // single most important anti-churn filter (found via web research + our
    // own -100% backtest diagnosing overtrading).
    if p.cross_lookback > 0 {
        let mut fresh = false;
        for k in 0..=p.cross_lookback.min(i) {
            if crossed_above(&ind.ema_fast, &ind.ema_slow, i - k) {
                fresh = true;
                break;
            }
        }
        if !fresh {
            return false;
        }
    }
    // Momentum gate: RSI in [min, max].
    if !(r >= p.rsi_entry_min && r <= p.rsi_entry_max) {
        return false;
    }
    true
}

/// Exit signal on closed bar `i` (trend flip / RSI death / time stop).
/// Stop-loss / take-profit / trailing are checked intrabar by the engine.
pub fn exit_signal(
    i: usize,
    entry_idx: usize,
    ind: &IndicatorState,
    p: &StrategyParams,
) -> Option<ExitReason> {
    if i < ind.warmup {
        return None;
    }
    // Time stop first (deterministic).
    if i.saturating_sub(entry_idx) >= p.max_holding_bars {
        return Some(ExitReason::MaxHolding);
    }
    // Trend flip: fast crossed below slow.
    if crossed_below(&ind.ema_fast, &ind.ema_slow, i) {
        return Some(ExitReason::TrendFlip);
    }
    // Also exit if firmly below (not just the cross bar) — catches gaps.
    if let (Some(f), Some(s)) = (ind.ema_fast[i], ind.ema_slow[i]) {
        // require a small hysteresis? No — simple: f < s means trend dead.
        if f < s {
            return Some(ExitReason::TrendFlip);
        }
    }
    // RSI death.
    if let Some(r) = ind.rsi[i] {
        if r < p.rsi_exit {
            return Some(ExitReason::RsiExit);
        }
    }
    None
}

/// Risk-based position sizing from scratch:
/// risk_amount = equity * risk_pct; per-unit risk = entry - stop;
/// qty_risk = risk_amount / per_unit_risk; qty_cap = equity*max_notional/entry.
/// qty = min(qty_risk, qty_cap). Returns 0 if stop invalid.
pub fn position_size(equity: f64, entry: f64, stop: f64, p: &StrategyParams) -> f64 {
    if equity <= 0.0 || entry <= 0.0 || stop <= 0.0 || stop >= entry {
        return 0.0;
    }
    let risk_amount = equity * p.risk_pct;
    let per_unit = entry - stop;
    if per_unit <= 0.0 {
        return 0.0;
    }
    let qty_risk = risk_amount / per_unit;
    let qty_cap = equity * p.max_notional_pct / entry;
    qty_risk.min(qty_cap).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Bar;

    fn bars_from_closes(closes: &[f64]) -> Vec<Bar> {
        closes
            .iter()
            .enumerate()
            .map(|(i, c)| Bar {
                open_time: i as i64 * 900_000,
                open: *c,
                high: *c * 1.001,
                low: *c * 0.999,
                close: *c,
                volume: 100.0,
            })
            .collect()
    }

    #[test]
    fn uptrend_with_good_rsi_enters() {
        // Steady grind up: EMA fast > slow, RSI high but < 78 eventually?
        // Build 200 bars rising 0.1% per bar.
        let closes: Vec<f64> = (0..200).map(|i| 100.0 * (1.001f64).powi(i)).collect();
        let bars = bars_from_closes(&closes);
        let p = StrategyParams::default();
        let ind = IndicatorState::compute(&bars, &p);
        // Late in a smooth grind RSI -> ~100 which is > entry_max 78, so NO entry.
        // That's the chase-guard working. Loosen max but keep fresh-cross gate:
        // a 200-bar-old cross is stale, so still NO entry (anti-churn working).
        assert!(!entry_signal(199, &ind, &p));
        let mut p2 = p;
        p2.rsi_entry_max = 100.0;
        assert!(!entry_signal(199, &ind, &p2));
        // Disable the fresh-cross gate -> should enter (old behavior).
        p2.cross_lookback = 0;
        assert!(entry_signal(199, &ind, &p2));
    }

    #[test]
    fn sizing_respects_risk_and_cap() {
        let p = StrategyParams::default(); // 1% risk, 95% cap
        // equity 10k, entry 100, stop 98 (risk 2/unit): risk 100 -> 50 units = 5000 notional < cap 9500.
        assert!((position_size(10_000.0, 100.0, 98.0, &p) - 50.0).abs() < 1e-9);
        // Tight stop -> huge qty_risk, capped at 95%: 9500/100=95.
        assert!((position_size(10_000.0, 100.0, 99.9, &p) - 95.0).abs() < 1e-9);
    }
}
