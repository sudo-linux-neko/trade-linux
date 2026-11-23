# trade-bot — Long-only EMA + RSI + ATR crypto spot bot (Rust, $0 budget)

Built from scratch in Rust. No TA libs. Free Binance public data only. Paper mode never sends orders.

## What you got (your choices)

- Market: **Crypto spot (Binance)** — BTCUSDT / ETHUSDT / SOLUSDT
- Strategy: **Trend EMA + RSI + ATR, long-only**
- Timeframe: **15m intraday** (plus 1h/4h/1d for comparison — this matters, see results)
- Backtest: **max history bulk** — 820,749 x 15m bars total

```
BTC 15m: 303,940 bars (2018-01 → 2026-09, ~8.7y)
ETH 15m: 303,940 bars
SOL 15m: 212,866 bars (2020-08 → 2026-09)
+ BTC 1h/4h/1d for timeframe analysis
```

## Layout (from scratch)

```
src/
  main.rs        CLI: download / backtest / optimize / walkforward / paper
  types.rs       Bar, Trade, EquityPoint, ExitReason, OpenPosition
  indicators.rs  SMA/EMA/RSI-Wilder/ATR-Wilder + cross helpers (unit-tested)
  strategy.rs    TrendLong params + fresh-cross anti-churn gate + risk sizing
  backtest.rs    Event-driven engine: CLOSE signal → NEXT OPEN fill, stop→trail→take
  metrics.rs     Sharpe/Sortino/Calmar/CAGR/maxDD/WR/PF/expectancy/exposure/turnover
  data.rs        Binance free klines (6-host fallback) + resume-safe CSV cache
  walkforward.rs 72-combo grid + rolling IS→OOS validation
data/            CSV cache (resume-safe)
reports/         .md + trades.csv + equity.csv + result.json per run
```

Execution honesty (web-research standard: tesser / rustrade / hftbacktest / openquanter):

- Signals on bar CLOSE, fills next OPEN ± slippage. No lookahead.
- Fees 10 bps/side + 5 bps slippage (Binance VIP0 realism).
- Stops checked stop → trailing → take (conservative when one 15m bar hits both).
- Single long-only spot position. Risk = 1% equity per trade, capped 95% notional.
- v2 anti-churn: entry needs a **fresh EMA cross within 5 bars** (v1 without it did 6-8k trades → instant ruin).

## Run it

```bash
cargo test                                        # 7 unit tests (EMA/RSI/ATR/cross/sizing)
./target/release/trade-bot download --symbols BTCUSDT,ETHUSDT,SOLUSDT --interval 15m
./target/release/trade-bot backtest --symbol BTCUSDT --interval 15m
./target/release/trade-bot optimize --symbol BTCUSDT --interval 4h --top 10
./target/release/trade-bot walkforward --symbol BTCUSDT --interval 15m --windows 4
./target/release/trade-bot paper --symbol BTCUSDT --interval 15m --poll-secs 60 --iterations 0
```

## Results — read this before risking $1 (all with fees + slippage)

Default params `e21-55 RSI[52-78|x42] ATR14 SL2.5 TP4 trail3 x5`:

| run | bars | trades | result | Sharpe | maxDD | PF |
|---|---|---|---|---|---|---|
| BTC 15m | 303,940 | 2,644 | **-100%** | -7.1 | -100% | 0.37 |
| ETH 15m | 303,940 | 2,629 | **-100%** | -6.0 | -100% | 0.42 |
| SOL 15m | 212,866 | 1,813 | **-96.5%** | -3.2 | -96.6% | 0.70 |
| BTC 1h | 75,996 | 604 | -77% | -2.2 | -78% | 0.56 |
| BTC 4h | 19,014 | 153 | -6.2% | -0.16 | -13% | 0.90 |
| BTC 1d | 3,172 | 21 | +0.4% | +0.04 | -4.7% | 1.04 |

Best of 72-combo grid:

| run | best tag | result | Sharpe | trades | PF |
|---|---|---|---|---|---|
| BTC 15m | e34-144 SL2 TP5 | -98% | -4.4 | 1,210 | 0.36 |
| BTC 4h | e21-89 SL2 TP3 | **+12.6% total / +1.4% CAGR** | 0.37 | 106 | 1.26 |
| BTC 1d | e21-144 SL3 TP5 | +4.3% total | 0.53 | 11 | 2.92 |

Walk-forward (IS optimize → OOS confirm):

| run | avg OOS Sharpe | hit-rate | verdict |
|---|---|---|---|
| BTC 15m (4 win) | **-5.71** | 0% | no edge, all OOS deeply negative |
| BTC 4h (6 win) | +0.01 | 33% | IS 0.6-1.7 decays to ~0 OOS = overfit/marginal |
| BTC 1d (5 win) | -1.32 | 0% | too few trades to conclude |

Interpretation:

1. **Your requested 15m EMA+RSI long-only has no edge net of fees.** 1,200-2,600 trades × 15 bps round-trip + 22-30% win-rate vs 2.5:1 RR = bleed. Fresh-cross cut churn 3× but still -98%.
2. **Edge improves monotonically with timeframe** (15m → 1h → 4h → 1d). Classic noise + fee-drag signature.
3. **4h best is +12.6% over 8.7y vs BTC buy&hold ~+1,000%+.** It beats nothing. 1d best is 11 trades in 8.7y — noise.
4. **Walk-forward kills it.** If IS Sharpe doesn't survive OOS, don't trade it.

This is what "really good" means in quant: a bot that tells you **not** to trade a dead edge, with receipts (trades.csv / equity.csv / result.json per run).

## How to make it actually good (next steps, in order)

1. Stay 4h/1d, never 15m, for this family. Or switch 15m to mean-reversion (RSI<30 + BB) — different edge.
2. Add regime filter: only trade when BTC > 200d SMA or realized-vol percentile < 70.
3. Buy&hold benchmark in-report (currently missing — do it).
4. Deflated Sharpe / PBO (Lopez de Prado) on the 72-combo sweep.
5. Funding/slippage stress: re-run at 25 bps to test fragility.
6. Multi-symbol portfolio (BTC+ETH+SOL) instead of single-symbol all-in.
7. Paper-trade 4h winner 3 months before even thinking about size.

## $0 budget notes

- Data: Binance public `/api/v3/klines` only, 6-host fallback (`api.binance.com` is geo-blocked in places; `data-api.binance.vision` usually works). No keys.
- Compute: this repo. `cargo test` + release binary. 300k-bar backtest ≈ 0.2s. 72-combo grid ≈ 15s.
- Live: `paper` polls public klines and prints signals. It **cannot** place orders by design.

## Compliance / safety

- Educational software, **not investment advice**. No warranty. Crypto is volatile; most intraday trend systems lose net of fees (as demonstrated above).
- You mentioned Goldman Sachs: check your employer's personal-trading / outside-activity / IP rules before running anything live, even personal paper-trading code written on personal time.
- Never put keys in this repo. Live execution is intentionally not implemented.

## Web research used

- Finnhub/Polygon/Yahoo/Binance free-tier limits (2026) → chose Binance public klines for $0 + no-key.
- tesser (event-driven backtester, paper connector, tick vs candle fidelity), rustrade (engine/data/execution split), hftbacktest (latency/queue fill realism), openquanter (margin-aware, deflated Sharpe/PBO, gap attribution), GlowBack/AlphaField/manifold-bt (walk-forward, Monte Carlo, Arrow/Parquet).
- Applied: CLOSE-signal/NEXT-OPEN-fill, fee+slippage both sides, IS→OOS walk-forward, min-trades filter, exit attribution.
