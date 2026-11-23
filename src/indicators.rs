// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Technical indicators from scratch (no TA crates).
//! All functions are pure, deterministic, and unit-tested.
//! Conventions:
//! - EMA: k = 2/(n+1), seeded with SMA of first `period` values.
//! - RSI: Wilder's SMMA of gains/losses, seeded with SMA.
//! - ATR: Wilder's SMMA of True Range, seeded with SMA.
//! - Outputs are `Vec<Option<f64>>` aligned with input; `None` = warmup.

/// Simple moving average, seeded helper. Returns None for i < period-1.
pub fn sma_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period >= 1, "period must be >= 1");
    let mut out = vec![None; values.len()];
    if values.len() < period || period == 0 {
        return out;
    }
    let mut sum = 0.0;
    for (i, v) in values.iter().enumerate() {
        sum += *v;
        if i >= period {
            sum -= values[i - period];
        }
        if i + 1 >= period {
            out[i] = Some(sum / period as f64);
        }
    }
    out
}

/// Exponential moving average from scratch.
pub fn ema_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period >= 1, "period must be >= 1");
    let mut out = vec![None; values.len()];
    if values.len() < period {
        return out;
    }
    if period == 1 {
        return values.iter().map(|v| Some(*v)).collect();
    }
    let k = 2.0 / (period as f64 + 1.0);
    // Seed with SMA of first `period` values.
    let seed: f64 = values[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = Some(seed);
    let mut prev = seed;
    for i in period..values.len() {
        let e = values[i] * k + prev * (1.0 - k);
        out[i] = Some(e);
        prev = e;
    }
    out
}

/// True Range for bar i (needs previous close). First TR = high - low.
pub fn true_range(high: f64, low: f64, prev_close: Option<f64>) -> f64 {
    match prev_close {
        None => high - low,
        Some(pc) => {
            let a = high - low;
            let b = (high - pc).abs();
            let c = (low - pc).abs();
            a.max(b).max(c)
        }
    }
}

/// Average True Range (Wilder). `highs/lows/closes` must be same length.
pub fn atr_series(highs: &[f64], lows: &[f64], closes: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period >= 1);
    assert_eq!(highs.len(), lows.len());
    assert_eq!(lows.len(), closes.len());
    let n = closes.len();
    let mut out = vec![None; n];
    if n <= period {
        return out;
    }
    // Build TR series.
    let mut tr = vec![0.0; n];
    for i in 0..n {
        let pc = if i == 0 { None } else { Some(closes[i - 1]) };
        tr[i] = true_range(highs[i], lows[i], pc);
    }
    // Seed: SMA of first `period` TRs -> placed at index `period-1`... but
    // standard Wilder seeds at index `period` using closes[1..=period].
    // We use SMA(tr[1..=period]) at index `period` so the very first bar
    // (which has no prev close) doesn't bias the seed. Simpler + robust.
    if n < period + 1 {
        return out;
    }
    let seed: f64 = tr[1..=period].iter().sum::<f64>() / period as f64;
    out[period] = Some(seed);
    let mut prev = seed;
    for i in (period + 1)..n {
        let a = (prev * (period as f64 - 1.0) + tr[i]) / period as f64;
        out[i] = Some(a);
        prev = a;
    }
    out
}

/// RSI (Wilder, 0..100). Needs period+1 closes minimum.
pub fn rsi_series(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period >= 1);
    let n = closes.len();
    let mut out = vec![None; n];
    if n <= period {
        return out;
    }
    // Gains / losses per bar (index 0 = 0).
    let mut gains = vec![0.0; n];
    let mut losses = vec![0.0; n];
    for i in 1..n {
        let d = closes[i] - closes[i - 1];
        if d > 0.0 {
            gains[i] = d;
        } else {
            losses[i] = -d;
        }
    }
    // Seed: SMA of first `period` deltas (bars 1..=period).
    let mut avg_gain: f64 = gains[1..=period].iter().sum::<f64>() / period as f64;
    let mut avg_loss: f64 = losses[1..=period].iter().sum::<f64>() / period as f64;
    out[period] = Some(rsi_from_avgs(avg_gain, avg_loss));
    for i in (period + 1)..n {
        avg_gain = (avg_gain * (period as f64 - 1.0) + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period as f64 - 1.0) + losses[i]) / period as f64;
        out[i] = Some(rsi_from_avgs(avg_gain, avg_loss));
    }
    out
}

fn rsi_from_avgs(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        return 100.0;
    }
    if avg_gain == 0.0 {
        return 0.0;
    }
    let rs = avg_gain / avg_loss;
    100.0 - 100.0 / (1.0 + rs)
}

/// Cross detection helpers (strictly on closed-bar series).
pub fn crossed_above(a: &[Option<f64>], b: &[Option<f64>], i: usize) -> bool {
    if i == 0 || i >= a.len() || i >= b.len() {
        return false;
    }
    match (a[i - 1], b[i - 1], a[i], b[i]) {
        (Some(pa), Some(pb), Some(ca), Some(cb)) => pa <= pb && ca > cb,
        _ => false,
    }
}

pub fn crossed_below(a: &[Option<f64>], b: &[Option<f64>], i: usize) -> bool {
    if i == 0 || i >= a.len() || i >= b.len() {
        return false;
    }
    match (a[i - 1], b[i - 1], a[i], b[i]) {
        (Some(pa), Some(pb), Some(ca), Some(cb)) => pa >= pb && ca < cb,
        _ => false,
    }
}

/// Rolling population standard deviation (for Bollinger Bands).
/// `None` for i < period-1.
pub fn stdev_series(values: &[f64], period: usize) -> Vec<Option<f64>> {
    assert!(period >= 1);
    let mut out = vec![None; values.len()];
    if values.len() < period {
        return out;
    }
    let sma = sma_series(values, period);
    for i in (period - 1)..values.len() {
        let m = sma[i].unwrap_or(0.0);
        let mut sum_sq = 0.0;
        for k in (i + 1 - period)..=i {
            let d = values[k] - m;
            sum_sq += d * d;
        }
        out[i] = Some((sum_sq / period as f64).sqrt());
    }
    out
}

/// Bollinger Bands from scratch: middle = SMA(period),
/// upper/lower = middle ± mult * stdev(period). Population stdev (TA standard).
pub struct Bollinger {
    pub middle: Vec<Option<f64>>,
    pub upper: Vec<Option<f64>>,
    pub lower: Vec<Option<f64>>,
    pub bandwidth: Vec<Option<f64>>, // (upper-lower)/middle, for squeeze filters
}

pub fn bollinger_series(values: &[f64], period: usize, mult: f64) -> Bollinger {
    let middle = sma_series(values, period);
    let sd = stdev_series(values, period);
    let n = values.len();
    let mut upper = vec![None; n];
    let mut lower = vec![None; n];
    let mut bandwidth = vec![None; n];
    for i in 0..n {
        if let (Some(m), Some(s)) = (middle[i], sd[i]) {
            let u = m + mult * s;
            let l = m - mult * s;
            upper[i] = Some(u);
            lower[i] = Some(l);
            if m != 0.0 {
                bandwidth[i] = Some((u - l) / m.abs());
            }
        }
    }
    Bollinger { middle, upper, lower, bandwidth }
}

/// Donchian channel of the PRIOR N bars (excludes bar i — no lookahead).
/// `highs/lows` typically bar high/low; breakout entry compares close[i] vs channel[i].
/// `None` for i < n.
pub fn donchian_prior(highs: &[f64], lows: &[f64], n: usize) -> (Vec<Option<f64>>, Vec<Option<f64>>) {
    assert_eq!(highs.len(), lows.len());
    let len = highs.len();
    let mut up = vec![None; len];
    let mut dn = vec![None; len];
    if n == 0 || len <= n {
        return (up, dn);
    }
    for i in n..len {
        let mut h = f64::NEG_INFINITY;
        let mut l = f64::INFINITY;
        for k in (i - n)..i {
            if highs[k] > h {
                h = highs[k];
            }
            if lows[k] < l {
                l = lows[k];
            }
        }
        up[i] = Some(h);
        dn[i] = Some(l);
    }
    (up, dn)
}

/// ATR as % of close (volatility-regime filter). `None` where ATR is None or close==0.
pub fn atr_pct_series(atr: &[Option<f64>], closes: &[f64]) -> Vec<Option<f64>> {
    atr.iter()
        .zip(closes.iter())
        .map(|(a, c)| match (a, c) {
            (Some(v), cc) if *cc != 0.0 => Some(v / cc.abs() * 100.0),
            _ => None,
        })
        .collect()
}

/// Percentile rank of `bandwidth[i]` within trailing `window` values (0..1).
/// Used for squeeze filters: skip extremely narrow (breakout coming) and
/// extremely wide (trend exhaust) regimes. `None` during warmup.
pub fn bandwidth_pct_rank(bandwidth: &[Option<f64>], window: usize, i: usize) -> Option<f64> {
    if window == 0 || i >= bandwidth.len() {
        return None;
    }
    let cur = bandwidth[i]?;
    let start = i.saturating_sub(window);
    let mut vals: Vec<f64> = bandwidth[start..i].iter().filter_map(|x| *x).collect();
    if vals.len() < window / 2 {
        return None;
    }
    vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let below = vals.iter().filter(|v| **v <= cur).count();
    Some(below as f64 / vals.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_converges_and_seeds_with_sma() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let e = ema_series(&v, 3);
        assert!(e[0].is_none() && e[1].is_none());
        assert!((e[2].unwrap() - 2.0).abs() < 1e-12); // SMA(1,2,3)=2
        // k=0.5: e3 = 4*0.5+2*0.5=3, e4=5*0.5+3*0.5=4
        assert!((e[3].unwrap() - 3.0).abs() < 1e-12);
        assert!((e[4].unwrap() - 4.0).abs() < 1e-12);
    }

    #[test]
    fn rsi_all_up_is_100_all_down_is_0() {
        let up: Vec<f64> = (0..20).map(|i| i as f64).collect();
        let r = rsi_series(&up, 14);
        assert!((r[19].unwrap() - 100.0).abs() < 1e-9);
        let dn: Vec<f64> = (0..20).map(|i| 20.0 - i as f64).collect();
        let r2 = rsi_series(&dn, 14);
        assert!(r2[19].unwrap() < 1e-9);
    }

    #[test]
    fn rsi_flat_is_100_no_loss() {
        let v = vec![10.0; 30];
        // gains=0 losses=0 -> avg_loss==0 -> 100 by convention
        let r = rsi_series(&v, 14);
        assert!((r[29].unwrap() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn atr_constant_range_equals_range() {
        // Every bar H-L = 2, closes flat -> TR=2 -> ATR=2.
        let n = 30;
        let highs = vec![11.0; n];
        let lows = vec![9.0; n];
        let closes = vec![10.0; n];
        let a = atr_series(&highs, &lows, &closes, 14);
        assert!((a[29].unwrap() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn cross_helpers_work() {
        let a = vec![Some(1.0), Some(2.0), Some(1.0)];
        let b = vec![Some(1.5), Some(1.5), Some(1.5)];
        assert!(crossed_above(&a, &b, 1));
        assert!(crossed_below(&a, &b, 2));
        assert!(!crossed_above(&a, &b, 2));
    }

    #[test]
    fn bollinger_flat_has_zero_width() {
        let v = vec![10.0; 30];
        let bb = bollinger_series(&v, 20, 2.0);
        let m = bb.middle[29].unwrap();
        let u = bb.upper[29].unwrap();
        let l = bb.lower[29].unwrap();
        assert!((m - 10.0).abs() < 1e-9);
        assert!((u - 10.0).abs() < 1e-9);
        assert!((l - 10.0).abs() < 1e-9);
        assert!(bb.bandwidth[29].unwrap() < 1e-12);
    }

    #[test]
    fn donchian_prior_excludes_current_bar() {
        // Highs rising 1..6. Prior-3 channel at i=5 covers bars 2,3,4 -> high 5.
        let h = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let l = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let (up, dn) = donchian_prior(&h, &l, 3);
        assert!(up[2].is_none()); // warmup
        assert!((up[5].unwrap() - 5.0).abs() < 1e-12);
        assert!((dn[5].unwrap() - 3.0).abs() < 1e-12);
        // No lookahead: channel at i does not include bar i itself.
        assert!(up[5].unwrap() < h[5]);
    }

    #[test]
    fn stdev_known_values() {
        // population stdev of 1..5 = sqrt(2) ≈ 1.4142
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let s = stdev_series(&v, 5);
        assert!((s[4].unwrap() - 2f64.sqrt()).abs() < 1e-9);
    }
}
