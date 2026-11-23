// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! Core domain types — no third-party TA libs, everything from scratch.
//! Long-only spot: no shorts, no leverage, no liquidation (spot). Fees + slippage modeled.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One OHLCV bar. Prices as f64 for research speed; execution rounds to tick.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Bar {
    /// Candle open time, unix millis (Binance convention).
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Bar {
    pub fn datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.open_time)
            .unwrap_or_default()
            .with_timezone(&Utc)
    }
}

/// Why a long position was exited. Used for trade attribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    TrendFlip,    // fast EMA crossed below slow EMA
    RsiExit,      // RSI fell below exit threshold
    StopLoss,     // ATR stop hit (intraday low <= stop)
    TakeProfit,   // ATR target hit
    TrailingStop, // ATR trailing stop hit
    MaxHolding,   // held longer than max_holding_bars
    EndOfData,    // forced flat at end of backtest
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ExitReason::TrendFlip => "trend_flip",
            ExitReason::RsiExit => "rsi_exit",
            ExitReason::StopLoss => "stop_loss",
            ExitReason::TakeProfit => "take_profit",
            ExitReason::TrailingStop => "trailing_stop",
            ExitReason::MaxHolding => "max_holding",
            ExitReason::EndOfData => "end_of_data",
        };
        write!(f, "{}", s)
    }
}

/// A completed round-trip long trade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub entry_idx: usize,
    pub entry_time: i64,
    pub entry_price: f64,
    pub exit_idx: usize,
    pub exit_time: i64,
    pub exit_price: f64,
    pub qty: f64,
    pub entry_fee: f64,
    pub exit_fee: f64,
    pub pnl: f64,     // net of fees
    pub pnl_pct: f64, // pnl / entry_notional
    pub bars_held: usize,
    pub exit_reason: ExitReason,
}

/// Equity curve point (one per bar, marked to close).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityPoint {
    pub time: i64,
    pub equity: f64,
    pub close: f64,
    pub position_qty: f64,
}

/// Open long position state held by the engine.
#[derive(Debug, Clone)]
pub struct OpenPosition {
    pub entry_idx: usize,
    pub entry_time: i64,
    pub entry_price: f64,
    pub qty: f64,
    pub entry_fee: f64,
    pub stop: f64,
    pub take: f64,
    pub trailing_stop: Option<f64>,
    pub highest_since_entry: f64,
}
