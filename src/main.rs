// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Vaibhav Agarwal

//! trade-bot: long-only crypto spot trend bot (EMA + RSI + ATR).
//! $0 budget: Binance public klines only, no keys. Paper mode never sends orders.
//!
//! Usage:
//!   trade-bot download --symbols BTCUSDT,ETHUSDT,SOLUSDT --interval 15m
//!   trade-bot backtest --symbol BTCUSDT --interval 15m
//!   trade-bot optimize --symbol BTCUSDT --interval 15m --top 10
//!   trade-bot walkforward --symbol BTCUSDT --interval 15m --windows 6
//!   trade-bot paper --symbol BTCUSDT --interval 15m

mod backtest;
mod data;
mod edge;
mod indicators;
mod metrics;
mod strategy;
mod types;
mod walkforward;

use std::path::PathBuf;

use backtest::{run_backtest, BacktestConfig};
use clap::{Parser, Subcommand};
use strategy::StrategyParams;

#[derive(Parser)]
#[command(name = "trade-bot", version, about = "Long-only EMA+RSI+ATR crypto spot bot + backtester")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Download max-history free klines (resume-safe CSV cache).
    Download {
        #[arg(long, default_value = "BTCUSDT,ETHUSDT,SOLUSDT")]
        symbols: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
    },
    /// Run one backtest with given params and write a report.
    Backtest {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "reports")]
        report_dir: PathBuf,
        #[arg(long, default_value_t = 10_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
        #[arg(long, default_value_t = 21)]
        fast: usize,
        #[arg(long, default_value_t = 55)]
        slow: usize,
        #[arg(long, default_value_t = 14)]
        rsi_period: usize,
        #[arg(long, default_value_t = 52.0)]
        rsi_min: f64,
        #[arg(long, default_value_t = 78.0)]
        rsi_max: f64,
        #[arg(long, default_value_t = 42.0)]
        rsi_exit: f64,
        #[arg(long, default_value_t = 2.5)]
        stop_mult: f64,
        #[arg(long, default_value_t = 4.0)]
        take_mult: f64,
        #[arg(long, default_value_t = 500)]
        max_holding: usize,
        #[arg(long, default_value_t = 3.0)]
        trailing_mult: f64,
        #[arg(long, default_value_t = 0.01)]
        risk_pct: f64,
    },
    /// Grid-search 72 param combos, rank by Sharpe.
    Optimize {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "reports")]
        report_dir: PathBuf,
        #[arg(long, default_value_t = 10_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
        #[arg(long, default_value_t = 30)]
        min_trades: usize,
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
    /// Walk-forward validation (IS optimize -> OOS confirm per window).
    Walkforward {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "reports")]
        report_dir: PathBuf,
        #[arg(long, default_value_t = 6)]
        windows: usize,
        #[arg(long, default_value_t = 20)]
        min_trades: usize,
        #[arg(long, default_value_t = 10_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
    },
    /// Live signal loop (dry-run: prints signals, NEVER trades).
    Paper {
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "15m")]
        interval: String,
        #[arg(long, default_value_t = 60)]
        poll_secs: u64,
        #[arg(long, default_value_t = 1)]
        iterations: u64, // 0 = infinite
    },
    /// Single backtest of a new edge: rsi2 | donchian | bbrev (defaults + overrides).
    EdgeBacktest {
        #[arg(long, default_value = "donchian")]
        strategy: String,
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "4h")]
        interval: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "reports")]
        report_dir: PathBuf,
        #[arg(long, default_value_t = 10_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
        #[arg(long)]
        stop_mult: Option<f64>,
        #[arg(long)]
        max_holding: Option<usize>,
    },
    /// Forever loop: grid-search rsi2+donchian+bbrev+trend across symbols×intervals,
    /// rank by OOS-proxy (Sharpe, min-trades, PF, DD), write leaderboard. Pass
    /// --iterations 0 --sleep-secs 3600 --refresh-data to run forever.
    Discover {
        #[arg(long, default_value = "BTCUSDT,ETHUSDT")]
        symbols: String,
        #[arg(long, default_value = "15m,1h,4h,1d")]
        intervals: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "reports")]
        report_dir: PathBuf,
        #[arg(long, default_value_t = 10_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
        #[arg(long, default_value_t = 15)]
        min_trades: usize,
        #[arg(long, default_value_t = 20)]
        top: usize,
        #[arg(long, default_value_t = false)]
        refresh_data: bool,
        /// 0 = infinite loop.
        #[arg(long, default_value_t = 1)]
        iterations: u64,
        #[arg(long, default_value_t = 0)]
        sleep_secs: u64,
    },
    /// Walk-forward for edge families (IS grid → OOS confirm per window).
    EdgeWalkforward {
        #[arg(long, default_value = "donchian")]
        strategy: String,
        #[arg(long, default_value = "BTCUSDT")]
        symbol: String,
        #[arg(long, default_value = "4h")]
        interval: String,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "reports")]
        report_dir: PathBuf,
        #[arg(long, default_value_t = 6)]
        windows: usize,
        #[arg(long, default_value_t = 5)]
        min_trades: usize,
        #[arg(long, default_value_t = 10_000.0)]
        cash: f64,
        #[arg(long, default_value_t = 10.0)]
        fee_bps: f64,
        #[arg(long, default_value_t = 5.0)]
        slippage_bps: f64,
    },
}

fn split_symbols(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_uppercase()).filter(|x| !x.is_empty()).collect()
}

fn write_backtest_report(
    res: &backtest::BacktestResult,
    report_dir: &std::path::Path,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(report_dir)?;
    let tag = format!("{}_{}_{}", res.symbol, res.interval, res.params.tag());
    let md_path = report_dir.join(format!("backtest_{tag}.md"));

    // Exit-reason attribution.
    use std::collections::BTreeMap;
    let mut by_reason: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for t in &res.trades {
        let e = by_reason.entry(t.exit_reason.to_string()).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += t.pnl;
    }

    let first = res.equity.first().map(|e| e.time).unwrap_or(0);
    let last = res.equity.last().map(|e| e.time).unwrap_or(0);
    let md = format!(
        "# Backtest — {} {} (`{}`)\n\n\
        - Bars: {} | Range: {} → {}\n\
        - Costs: fee {:.0} bps/side, slippage {:.0} bps | initial ${:.0}\n\
        - Params: fast={} slow={} rsi={} entry=[{:.0},{:.0}] exit<{:.0} atr={} stop={}x take={}x hold<={} trail={}x risk={:.1}%\n\n\
        ## Metrics\n\n\
        - Final equity: **${:.2}** ({:+.2}% total, {:+.2}% CAGR)\n\
        - Sharpe {:.2} | Sortino {:.2} | Calmar {:.2} | ann.vol {:.1}%\n\
        - Max drawdown {:.2}% over {} bars\n\
        - Trades: {} | win {:.1}% | PF {:.2} | expectancy ${:.2}\n\
        - avg win ${:.2} / avg loss ${:.2} | avg hold {:.0} bars | exposure {:.1}% | turnover {:.1}x | fees ${:.2}\n\n\
        ## Exit attribution\n\n\
        | reason | n | total pnl |\n|---|---|---|\n{}\n\n\
        ## Honesty notes (read before risking money)\n\n\
        - Signals on CLOSE, fills next OPEN ± slippage. No lookahead.\n\
        - Stops checked stop→trail→take (conservative when a 15m bar hits both).\n\
        - Single long-only spot position; fees on both sides; forming candle re-fetched.\n\
        - Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.\n\
        - $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.\n",
        res.symbol,
        res.interval,
        res.params.tag(),
        res.n_bars,
        fmt_ts(first),
        fmt_ts(last),
        res.config.fee_bps,
        res.config.slippage_bps,
        res.config.initial_cash,
        res.params.fast,
        res.params.slow,
        res.params.rsi_period,
        res.params.rsi_entry_min,
        res.params.rsi_entry_max,
        res.params.rsi_exit,
        res.params.atr_period,
        res.params.stop_mult,
        res.params.take_mult,
        res.params.max_holding_bars,
        res.params.trailing_mult,
        res.params.risk_pct * 100.0,
        res.metrics.final_equity,
        res.metrics.total_return_pct,
        res.metrics.cagr_pct,
        res.metrics.sharpe,
        res.metrics.sortino,
        res.metrics.calmar,
        res.metrics.volatility_ann_pct,
        res.metrics.max_drawdown_pct,
        res.metrics.max_dd_bars,
        res.metrics.n_trades,
        res.metrics.win_rate_pct,
        res.metrics.profit_factor,
        res.metrics.expectancy,
        res.metrics.avg_win,
        res.metrics.avg_loss,
        res.metrics.avg_bars_held,
        res.metrics.exposure_pct,
        res.metrics.turnover,
        res.metrics.total_fees,
        by_reason
            .iter()
            .map(|(k, (n, p))| format!("| {k} | {n} | {p:+.2} |"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    std::fs::write(&md_path, md)?;

    // Machine-readable artifacts.
    let stem = report_dir.join(format!("backtest_{tag}"));
    let mut wtr = csv::Writer::from_path(stem.with_extension("trades.csv"))?;
    for t in &res.trades {
        wtr.serialize(t)?;
    }
    wtr.flush()?;
    let mut wtr = csv::Writer::from_path(stem.with_extension("equity.csv"))?;
    for e in &res.equity {
        wtr.serialize(e)?;
    }
    wtr.flush()?;
    std::fs::write(
        stem.with_extension("result.json"),
        serde_json::to_string_pretty(res)?,
    )?;
    Ok(md_path)
}

fn fmt_ts(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "?".into())
}

fn write_edge_report(res: &edge::EdgeResult, report_dir: &std::path::Path) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(report_dir)?;
    let safe_tag: String = res.tag.chars().map(|c| if c == '/' || c == ' ' { '_' } else { c }).collect();
    let stem = report_dir.join(format!("edge_{}_{}_{}_{}", res.strategy, res.symbol, res.interval, safe_tag));
    use std::collections::BTreeMap;
    let mut by_reason: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for t in &res.trades {
        let e = by_reason.entry(t.exit_reason.to_string()).or_insert((0, 0.0));
        e.0 += 1;
        e.1 += t.pnl;
    }
    let first = res.equity.first().map(|e| e.time).unwrap_or(0);
    let last = res.equity.last().map(|e| e.time).unwrap_or(0);
    let m = &res.metrics;
    let md = format!(
        "# Edge backtest — {} {} {} (`{}`)\n\n\
        - Bars: {} | Range: {} → {} | costs {:.0}/{:.0} bps | initial ${:.0}\n\
        - Final **${:.2}** ({:+.2}% total, {:+.2}% CAGR) vs buy&hold {:+.1}% (excess {:+.1}pp)\n\
        - Sharpe {:.2} Sortino {:.2} Calmar {:.2} vol {:.1}% | maxDD {:.2}% ({} bars)\n\
        - Trades {} WR {:.1}% PF {:.2} exp ${:.2} | avgW ${:.2} avgL ${:.2} hold {:.0}b | expos {:.1}% turn {:.1}x fees ${:.2}\n\n\
        ## Exit attribution\n\n| reason | n | pnl |\n|---|---|---|\n{}\n\n\
        ## Honesty\n\n- CLOSE signal, NEXT OPEN fill, no lookahead. Stop→trail→take conservative.\n\
        - Long-only spot. Rank ≠ future edge: run `walkforward`-style OOS before sizing.\n",
        res.strategy, res.symbol, res.interval, res.tag,
        res.n_bars, fmt_ts(first), fmt_ts(last), res.config.fee_bps, res.config.slippage_bps,
        res.config.initial_cash, m.final_equity, m.total_return_pct, m.cagr_pct,
        res.buy_hold_return_pct, m.total_return_pct - res.buy_hold_return_pct,
        m.sharpe, m.sortino, m.calmar, m.volatility_ann_pct, m.max_drawdown_pct, m.max_dd_bars,
        m.n_trades, m.win_rate_pct, m.profit_factor, m.expectancy,
        m.avg_win, m.avg_loss, m.avg_bars_held, m.exposure_pct, m.turnover, m.total_fees,
        by_reason.iter().map(|(k, (n, p))| format!("| {k} | {n} | {p:+.2} |")).collect::<Vec<_>>().join("\n"),
    );
    std::fs::write(stem.with_extension("md"), &md)?;
    let mut wtr = csv::Writer::from_path(stem.with_extension("trades.csv"))?;
    for t in &res.trades {
        wtr.serialize(t)?;
    }
    wtr.flush()?;
    let mut wtr = csv::Writer::from_path(stem.with_extension("equity.csv"))?;
    for e in &res.equity {
        wtr.serialize(e)?;
    }
    wtr.flush()?;
    std::fs::write(stem.with_extension("result.json"), serde_json::to_string_pretty(res)?)?;
    Ok(stem.with_extension("md"))
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Download { symbols, interval, data_dir } => {
            data::interval_ms(&interval)?;
            for sym in split_symbols(&symbols) {
                println!("[dl] {sym} {interval} ...");
                match data::download_symbol(&sym, &interval, &data_dir).await {
                    Ok(bars) => println!("[dl] OK {sym}: {} bars", bars.len()),
                    Err(e) => eprintln!("[dl] FAIL {sym}: {e:#}"),
                }
            }
        }
        Cmd::Backtest {
            symbol, interval, data_dir, report_dir, cash, fee_bps, slippage_bps,
            fast, slow, rsi_period, rsi_min, rsi_max, rsi_exit, stop_mult, take_mult,
            max_holding, trailing_mult, risk_pct,
        } => {
            let p = StrategyParams {
                fast, slow, rsi_period,
                rsi_entry_min: rsi_min, rsi_entry_max: rsi_max, rsi_exit,
                stop_mult, take_mult, max_holding_bars: max_holding,
                trailing_mult, risk_pct, ..StrategyParams::default()
            };
            let cfg = BacktestConfig { initial_cash: cash, fee_bps, slippage_bps };
            let path = data::data_path(&data_dir, &symbol, &interval);
            let bars = if path.exists() {
                println!("[bt] loading {}", path.display());
                data::load_bars(&path)?
            } else {
                println!("[bt] no cache, downloading {symbol} {interval} ...");
                data::download_symbol(&symbol, &interval, &data_dir).await?
            };
            println!("[bt] {} bars, params {}", bars.len(), p.tag());
            let res = run_backtest(&bars, &p, &cfg, &symbol, &interval)?;
            println!("[bt] {}", metrics::summarize(&res.metrics));
            let md = write_backtest_report(&res, &report_dir)?;
            println!("[bt] report -> {}", md.display());
        }
        Cmd::Optimize { symbol, interval, data_dir, report_dir, cash, fee_bps, slippage_bps, min_trades, top } => {
            let cfg = BacktestConfig { initial_cash: cash, fee_bps, slippage_bps };
            let path = data::data_path(&data_dir, &symbol, &interval);
            let bars = if path.exists() { data::load_bars(&path)? } else {
                data::download_symbol(&symbol, &interval, &data_dir).await?
            };
            println!("[opt] {} bars, grid {} combos, min_trades {}", bars.len(), walkforward::default_grid().len(), min_trades);
            let grid = walkforward::default_grid();
            let ranked = walkforward::grid_search(&bars, &grid, &cfg, &symbol, &interval, min_trades);
            println!("[opt] {} passed filter", ranked.len());
            for (i, (p, r)) in ranked.iter().take(top).enumerate() {
                println!("  #{:>2} {} | {}", i + 1, p.tag(), metrics::summarize(&r.metrics));
            }
            // Save table.
            std::fs::create_dir_all(&report_dir)?;
            let out = report_dir.join(format!("optimize_{symbol}_{interval}.csv"));
            let mut wtr = csv::Writer::from_path(&out)?;
            wtr.write_record(["rank", "tag", "fast", "slow", "rsi_min", "stop", "take", "sharpe", "ret_pct", "maxdd_pct", "trades", "win_pct", "pf"])?;
            for (i, (p, r)) in ranked.iter().enumerate() {
                wtr.write_record([
                    (i + 1).to_string(), p.tag(),
                    p.fast.to_string(), p.slow.to_string(),
                    p.rsi_entry_min.to_string(), p.stop_mult.to_string(), p.take_mult.to_string(),
                    format!("{:.3}", r.metrics.sharpe),
                    format!("{:.2}", r.metrics.total_return_pct),
                    format!("{:.2}", r.metrics.max_drawdown_pct),
                    r.metrics.n_trades.to_string(),
                    format!("{:.1}", r.metrics.win_rate_pct),
                    format!("{:.2}", r.metrics.profit_factor),
                ])?;
            }
            wtr.flush()?;
            println!("[opt] table -> {}", out.display());
            // Auto-write full report for the winner so `optimize` doubles as backtest.
            if let Some((p, _)) = ranked.first() {
                let res = run_backtest(&bars, p, &cfg, &symbol, &interval)?;
                let md = write_backtest_report(&res, &report_dir)?;
                println!("[opt] winner report -> {}", md.display());
            }
        }
        Cmd::Walkforward { symbol, interval, data_dir, report_dir, windows, min_trades, cash, fee_bps, slippage_bps } => {
            let cfg = BacktestConfig { initial_cash: cash, fee_bps, slippage_bps };
            let path = data::data_path(&data_dir, &symbol, &interval);
            let bars = if path.exists() { data::load_bars(&path)? } else {
                data::download_symbol(&symbol, &interval, &data_dir).await?
            };
            println!("[wf] {} bars, {} windows", bars.len(), windows);
            let grid = walkforward::default_grid();
            let rep = walkforward::walk_forward(&bars, windows, &grid, &cfg, &symbol, &interval, min_trades)?;
            for w in &rep.windows {
                println!(
                    "  w{} IS Sharpe {:+.2} ({} tr) -> OOS {:+.2} / {:+.1}% ({} tr, dd {:.1}%) [{}]",
                    w.window, w.train_sharpe, w.train_trades, w.test_sharpe,
                    w.test_return_pct, w.test_trades, w.test_max_dd_pct, w.best_tag
                );
            }
            println!(
                "[wf] avg OOS Sharpe {:+.2} | OOS hit-rate {:.0}% | total OOS trades {}",
                rep.avg_oos_sharpe, rep.oos_hit_rate_pct, rep.total_oos_trades
            );
            std::fs::create_dir_all(&report_dir)?;
            let out = report_dir.join(format!("walkforward_{symbol}_{interval}.json"));
            std::fs::write(&out, serde_json::to_string_pretty(&rep)?)?;
            println!("[wf] -> {}", out.display());
        }
        Cmd::Paper { symbol, interval, poll_secs, iterations } => {
            println!("[paper] DRY-RUN {symbol} {interval} (never places orders). Ctrl-C to stop.");
            let p = StrategyParams::default();
            let mut it = 0u64;
            loop {
                match data::fetch_recent(&symbol, &interval, 300).await {
                    Ok(bars) if bars.len() > 100 => {
                        // Last bar is forming -> decide on second-to-last CLOSE.
                        let i = bars.len() - 2;
                        let ind = strategy::IndicatorState::compute(&bars, &p);
                        let sig = strategy::entry_signal(i, &ind, &p);
                        let close = bars[i].close;
                        let atr = ind.atr[i].unwrap_or(close * 0.01);
                        let rsi = ind.rsi[i].unwrap_or(50.0);
                        let ef = ind.ema_fast[i].unwrap_or(0.0);
                        let es = ind.ema_slow[i].unwrap_or(0.0);
                        let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
                        if sig {
                            let stop = close - p.stop_mult * atr;
                            let take = close + p.take_mult * atr;
                            println!(
                                "[{ts}] LONG SIGNAL close={:.2} rsi={:.1} emaF={:.2}>emaS={:.2} atr={:.2} stop={:.2} take={:.2} (paper only)",
                                close, rsi, ef, es, atr, stop, take
                            );
                        } else {
                            println!(
                                "[{ts}] flat/hold close={:.2} rsi={:.1} emaF={:.2} vs emaS={:.2} (paper only)",
                                close, rsi, ef, es
                            );
                        }
                    }
                    Ok(_) => eprintln!("[paper] not enough bars yet"),
                    Err(e) => eprintln!("[paper] fetch failed: {e:#}"),
                }
                it += 1;
                if iterations != 0 && it >= iterations {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_secs(poll_secs)).await;
            }
        }
        Cmd::EdgeBacktest { strategy, symbol, interval, data_dir, report_dir, cash, fee_bps, slippage_bps, stop_mult, max_holding } => {
            let cfg = BacktestConfig { initial_cash: cash, fee_bps, slippage_bps };
            let path = data::data_path(&data_dir, &symbol, &interval);
            let bars = if path.exists() { data::load_bars(&path)? } else {
                data::download_symbol(&symbol, &interval, &data_dir).await?
            };
            let s = strategy.to_lowercase();
            let res = match s.as_str() {
                "rsi2" => {
                    let mut p = edge::Rsi2Params::default();
                    if let Some(v) = stop_mult { p.stop_mult = v; }
                    if let Some(v) = max_holding { p.max_holding_bars = v; }
                    edge::run_rsi2(&bars, &p, &cfg, &symbol, &interval)?
                }
                "donchian" | "turtle" | "breakout" => {
                    let mut p = edge::DonchianParams::default();
                    if let Some(v) = stop_mult { p.stop_mult = v; }
                    if let Some(v) = max_holding { p.max_holding_bars = v; }
                    edge::run_donchian(&bars, &p, &cfg, &symbol, &interval)?
                }
                "bbrev" | "bb" | "meanrev" | "mean-reversion" => {
                    let mut p = edge::BbRevParams::default();
                    if let Some(v) = stop_mult { p.stop_mult = v; }
                    if let Some(v) = max_holding { p.max_holding_bars = v; }
                    edge::run_bbrev(&bars, &p, &cfg, &symbol, &interval)?
                }
                _ => anyhow::bail!("unknown strategy '{strategy}' (use rsi2|donchian|bbrev)"),
            };
            println!(
                "[edge:{}] {} {} n={} ret {:+.2}% (BH {:+.1}%) Sharpe {:.2} DD {:.1}% trades {} WR {:.1}% PF {:.2}",
                res.strategy, res.symbol, res.interval, res.n_bars,
                res.metrics.total_return_pct, res.buy_hold_return_pct, res.metrics.sharpe,
                res.metrics.max_drawdown_pct, res.metrics.n_trades,
                res.metrics.win_rate_pct, res.metrics.profit_factor
            );
            let md = write_edge_report(&res, &report_dir)?;
            println!("[edge] report -> {}", md.display());
        }
        Cmd::Discover { symbols, intervals, data_dir, report_dir, cash, fee_bps, slippage_bps, min_trades, top, refresh_data, iterations, sleep_secs } => {
            let syms = split_symbols(&symbols);
            let ivs = split_list(&intervals);
            let cfg = BacktestConfig { initial_cash: cash, fee_bps, slippage_bps };
            let mut iter: u64 = 0;
            loop {
                iter += 1;
                println!("[discover] iteration {iter} (symbols={syms:?} intervals={ivs:?})");
                // (strategy, tag, symbol, interval, sharpe, ret, bh, maxdd, trades, wr, pf)
                let mut rows: Vec<(String, String, String, String, f64, f64, f64, f64, usize, f64, f64)> = Vec::new();
                // Keep full results in memory only for top rerun; store best per combo.
                let mut best_map: std::collections::HashMap<String, edge::EdgeResult> = std::collections::HashMap::new();
                for sym in &syms {
                    for iv in &ivs {
                        let path = data::data_path(&data_dir, sym, iv);
                        let bars = if refresh_data || !path.exists() {
                            match data::download_symbol(sym, iv, &data_dir).await {
                                Ok(b) => b,
                                Err(e) => { eprintln!("[discover] SKIP {sym} {iv}: download failed: {e:#}"); continue; }
                            }
                        } else {
                            data::load_bars(&path)?
                        };
                        if bars.len() < 300 {
                            eprintln!("[discover] SKIP {sym} {iv}: only {} bars", bars.len());
                            continue;
                        }
                        // --- trend family (existing 72-grid) ---
                        for p in walkforward::default_grid() {
                            if let Ok(r) = run_backtest(&bars, &p, &cfg, sym, iv) {
                                if r.metrics.n_trades >= min_trades {
                                    let key = format!("trend|{}|{sym}|{iv}", p.tag());
                                    rows.push(("trend".into(), p.tag(), sym.clone(), iv.clone(),
                                        r.metrics.sharpe, r.metrics.total_return_pct,
                                        edge::buy_hold_return(&bars), r.metrics.max_drawdown_pct,
                                        r.metrics.n_trades, r.metrics.win_rate_pct, r.metrics.profit_factor));
                                    // store lightweight? trend results are BacktestResult, not EdgeResult — skip storing.
                                    let _ = key;
                                }
                            }
                        }
                        // --- rsi2 ---
                        for p in edge::rsi2_grid() {
                            if let Ok(r) = edge::run_rsi2(&bars, &p, &cfg, sym, iv) {
                                if r.metrics.n_trades >= min_trades {
                                    rows.push(("rsi2".into(), p.tag(), sym.clone(), iv.clone(),
                                        r.metrics.sharpe, r.metrics.total_return_pct,
                                        r.buy_hold_return_pct, r.metrics.max_drawdown_pct,
                                        r.metrics.n_trades, r.metrics.win_rate_pct, r.metrics.profit_factor));
                                    let key = format!("rsi2|{}|{sym}|{iv}", p.tag());
                                    best_map.insert(key, r);
                                }
                            }
                        }
                        // --- donchian ---
                        for p in edge::donchian_grid() {
                            if let Ok(r) = edge::run_donchian(&bars, &p, &cfg, sym, iv) {
                                if r.metrics.n_trades >= min_trades {
                                    rows.push(("donchian".into(), p.tag(), sym.clone(), iv.clone(),
                                        r.metrics.sharpe, r.metrics.total_return_pct,
                                        r.buy_hold_return_pct, r.metrics.max_drawdown_pct,
                                        r.metrics.n_trades, r.metrics.win_rate_pct, r.metrics.profit_factor));
                                    let key = format!("donchian|{}|{sym}|{iv}", p.tag());
                                    best_map.insert(key, r);
                                }
                            }
                        }
                        // --- bbrev ---
                        for p in edge::bbrev_grid() {
                            if let Ok(r) = edge::run_bbrev(&bars, &p, &cfg, sym, iv) {
                                if r.metrics.n_trades >= min_trades {
                                    rows.push(("bbrev".into(), p.tag(), sym.clone(), iv.clone(),
                                        r.metrics.sharpe, r.metrics.total_return_pct,
                                        r.buy_hold_return_pct, r.metrics.max_drawdown_pct,
                                        r.metrics.n_trades, r.metrics.win_rate_pct, r.metrics.profit_factor));
                                    let key = format!("bbrev|{}|{sym}|{iv}", p.tag());
                                    best_map.insert(key, r);
                                }
                            }
                        }
                        println!("[discover] {sym} {iv}: {} ranked rows so far", rows.len());
                    }
                }
                rows.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));
                println!("[discover] TOP {} of {}:", top.min(rows.len()), rows.len());
                for (i, r) in rows.iter().take(top).enumerate() {
                    println!("  #{:>2} {:8} {} {} {} | Sharpe {:+.2} ret {:+.1}% (BH {:+.0}%) DD {:.1}% n={} WR {:.0}% PF {:.2}",
                        i + 1, r.0, r.2, r.3, r.1, r.4, r.5, r.6, r.7, r.8, r.9, r.10);
                }
                std::fs::create_dir_all(&report_dir)?;
                let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let out = report_dir.join(format!("leaderboard_iter{iter}_{ts}.csv"));
                let mut wtr = csv::Writer::from_path(&out)?;
                wtr.write_record(["rank", "strategy", "symbol", "interval", "tag", "sharpe", "ret_pct", "buyhold_pct", "excess_pp", "maxdd_pct", "trades", "win_pct", "pf"])?;
                for (i, r) in rows.iter().enumerate() {
                    wtr.write_record([
                        (i + 1).to_string(), r.0.clone(), r.2.clone(), r.3.clone(), r.1.clone(),
                        format!("{:.3}", r.4), format!("{:.2}", r.5), format!("{:.1}", r.6),
                        format!("{:.1}", r.5 - r.6), format!("{:.2}", r.7),
                        r.8.to_string(), format!("{:.1}", r.9), format!("{:.2}", r.10),
                    ])?;
                }
                wtr.flush()?;
                println!("[discover] leaderboard -> {}", out.display());
                // Full report for the #1 edge result (if it's an edge family; trend winner re-run separately).
                if let Some(r) = rows.first() {
                    let key = format!("{}|{}|{}|{}", r.0, r.1, r.2, r.3);
                    if let Some(full) = best_map.get(&key) {
                        let md = write_edge_report(full, &report_dir)?;
                        println!("[discover] winner report -> {}", md.display());
                    } else if r.0 == "trend" {
                        // Re-run trend winner to emit its standard report.
                        let path = data::data_path(&data_dir, &r.2, &r.3);
                        if let Ok(bars) = data::load_bars(&path) {
                            for p in walkforward::default_grid() {
                                if p.tag() == r.1 {
                                    if let Ok(res) = run_backtest(&bars, &p, &cfg, &r.2, &r.3) {
                                        let md = write_backtest_report(&res, &report_dir)?;
                                        println!("[discover] trend-winner report -> {}", md.display());
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                if iterations != 0 && iter >= iterations {
                    break;
                }
                if sleep_secs > 0 {
                    println!("[discover] sleeping {sleep_secs}s (Ctrl-C to stop) ...");
                    tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
                }
            }
        }
        Cmd::EdgeWalkforward { strategy, symbol, interval, data_dir, report_dir, windows, min_trades, cash, fee_bps, slippage_bps } => {
            let cfg = BacktestConfig { initial_cash: cash, fee_bps, slippage_bps };
            let path = data::data_path(&data_dir, &symbol, &interval);
            let bars = if path.exists() { data::load_bars(&path)? } else {
                data::download_symbol(&symbol, &interval, &data_dir).await?
            };
            println!("[edge-wf:{}] {} bars, {} windows", strategy, bars.len(), windows);
            let rep = edge::edge_walk_forward(&bars, &strategy, windows, &cfg, &symbol, &interval, min_trades)?;
            for w in &rep.windows {
                println!(
                    "  w{} IS {:+.2} ({} tr) -> OOS {:+.2} / {:+.1}% ({} tr, dd {:.1}%) [{}]",
                    w.window, w.train_sharpe, w.train_trades, w.test_sharpe,
                    w.test_return_pct, w.test_trades, w.test_max_dd_pct, w.best_tag
                );
            }
            println!(
                "[edge-wf] avg OOS Sharpe {:+.2} | hit-rate {:.0}% | total OOS trades {}",
                rep.avg_oos_sharpe, rep.oos_hit_rate_pct, rep.total_oos_trades
            );
            std::fs::create_dir_all(&report_dir)?;
            let out = report_dir.join(format!("edge_walkforward_{}_{}_{}.json", rep.strategy, symbol, interval));
            std::fs::write(&out, serde_json::to_string_pretty(&rep)?)?;
            println!("[edge-wf] -> {}", out.display());
        }
    }
    Ok(())
}
