// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Performance + risk metrics from scratch.
//! Annualization uses bars_per_year (15m crypto 24/7 = 35,040).

use serde::{Deserialize, Serialize};

use crate::types::{EquityPoint, Trade};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metrics {
    pub initial: f64,
    pub final_equity: f64,
    pub total_return_pct: f64,
    pub cagr_pct: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub max_drawdown_pct: f64,
    pub max_dd_bars: usize,
    pub volatility_ann_pct: f64,
    pub n_trades: usize,
    pub win_rate_pct: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub profit_factor: f64,
    pub expectancy: f64,
    pub avg_bars_held: f64,
    pub exposure_pct: f64,
    pub turnover: f64,
    pub calmar: f64,
    pub total_fees: f64,
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn stddev(xs: &[f64], m: f64) -> f64 {
    if xs.len() < 2 {
        return 0.0;
    }
    let v: f64 = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64;
    v.sqrt()
}

pub fn compute_metrics(
    trades: &[Trade],
    equity: &[EquityPoint],
    initial: f64,
    bars_per_year: f64,
) -> Metrics {
    let final_equity = equity.last().map(|e| e.equity).unwrap_or(initial);
    let total_return_pct = if initial > 0.0 { (final_equity / initial - 1.0) * 100.0 } else { 0.0 };

    // CAGR from bar count.
    let years = if bars_per_year > 0.0 { equity.len() as f64 / bars_per_year } else { 0.0 };
    let cagr_pct = if years > 0.0 && initial > 0.0 && final_equity > 0.0 {
        ((final_equity / initial).powf(1.0 / years) - 1.0) * 100.0
    } else {
        0.0
    };

    // Per-bar simple returns on equity.
    let mut rets: Vec<f64> = Vec::with_capacity(equity.len().saturating_sub(1));
    for w in equity.windows(2) {
        let prev = w[0].equity;
        let cur = w[1].equity;
        if prev > 0.0 {
            rets.push(cur / prev - 1.0);
        }
    }
    let m = mean(&rets);
    let sd = stddev(&rets, m);
    let sharpe = if sd > 1e-12 { m / sd * bars_per_year.sqrt() } else { 0.0 };
    // Sortino: downside deviation only.
    let downside: Vec<f64> = rets.iter().cloned().filter(|r| *r < 0.0).collect();
    let dd_sd = if downside.len() >= 2 {
        let dm = mean(&downside);
        stddev(&downside, dm)
    } else {
        0.0
    };
    let sortino = if dd_sd > 1e-12 { m / dd_sd * bars_per_year.sqrt() } else if m > 0.0 { 10.0 } else { 0.0 };
    let volatility_ann_pct = sd * bars_per_year.sqrt() * 100.0;

    // Max drawdown.
    let mut peak = f64::NEG_INFINITY;
    let mut peak_idx = 0usize;
    let mut max_dd = 0.0f64;
    let mut max_dd_bars = 0usize;
    for (i, e) in equity.iter().enumerate() {
        if e.equity > peak {
            peak = e.equity;
            peak_idx = i;
        }
        let dd = if peak > 0.0 { (e.equity / peak - 1.0) * 100.0 } else { 0.0 };
        if dd < max_dd {
            max_dd = dd;
            max_dd_bars = i.saturating_sub(peak_idx);
        }
    }

    let n = trades.len();
    let wins = trades.iter().filter(|t| t.pnl > 0.0).count();
    let win_rate_pct = if n > 0 { wins as f64 / n as f64 * 100.0 } else { 0.0 };
    let gross_win: f64 = trades.iter().filter(|t| t.pnl > 0.0).map(|t| t.pnl).sum();
    let gross_loss: f64 = trades.iter().filter(|t| t.pnl <= 0.0).map(|t| -t.pnl).sum();
    let profit_factor = if gross_loss > 1e-12 { gross_win / gross_loss } else if gross_win > 0.0 { 99.0 } else { 0.0 };
    let avg_win = if wins > 0 { gross_win / wins as f64 } else { 0.0 };
    let n_loss = n.saturating_sub(wins);
    let avg_loss = if n_loss > 0 { -gross_loss / n_loss as f64 } else { 0.0 };
    let expectancy = if n > 0 { trades.iter().map(|t| t.pnl).sum::<f64>() / n as f64 } else { 0.0 };
    let avg_bars_held = if n > 0 {
        trades.iter().map(|t| t.bars_held as f64).sum::<f64>() / n as f64
    } else {
        0.0
    };
    let exposure_pct = if !equity.is_empty() {
        equity.iter().filter(|e| e.position_qty > 0.0).count() as f64 / equity.len() as f64 * 100.0
    } else {
        0.0
    };
    let notional_traded: f64 = trades
        .iter()
        .map(|t| t.qty * t.entry_price + t.qty * t.exit_price)
        .sum();
    let turnover = if initial > 0.0 { notional_traded / initial } else { 0.0 };
    let calmar = if max_dd.abs() > 1e-9 { cagr_pct / max_dd.abs() } else { 0.0 };
    let total_fees: f64 = trades.iter().map(|t| t.entry_fee + t.exit_fee).sum();

    Metrics {
        initial,
        final_equity,
        total_return_pct,
        cagr_pct,
        sharpe,
        sortino,
        max_drawdown_pct: max_dd,
        max_dd_bars,
        volatility_ann_pct,
        n_trades: n,
        win_rate_pct,
        avg_win,
        avg_loss,
        profit_factor,
        expectancy,
        avg_bars_held,
        exposure_pct,
        turnover,
        calmar,
        total_fees,
    }
}

/// One-line human summary.
pub fn summarize(m: &Metrics) -> String {
    format!(
        "ret {:+.1}% | CAGR {:+.1}% | Sharpe {:.2} Sortino {:.2} | maxDD {:.1}% ({} bars) | {} trades WR {:.1}% PF {:.2} | exp {:.0}% fees ${:.0}",
        m.total_return_pct,
        m.cagr_pct,
        m.sharpe,
        m.sortino,
        m.max_drawdown_pct,
        m.max_dd_bars,
        m.n_trades,
        m.win_rate_pct,
        m.profit_factor,
        m.exposure_pct,
        m.total_fees
    )
}
