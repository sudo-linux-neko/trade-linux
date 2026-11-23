# Backtest — BTCUSDT 1h (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3_x5`)

- Bars: 75996 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$2260.90** (-77.39% total, -15.75% CAGR)
- Sharpe -2.19 | Sortino -1.03 | Calmar -0.20 | ann.vol 7.7%
- Max drawdown -78.51% over 73560 bars
- Trades: 604 | win 29.0% | PF 0.56 | expectancy $-12.81
- avg win $56.38 / avg loss $-41.04 | avg hold 14 bars | exposure 10.9% | turnover 384.0x | fees $3839.66

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 15 | -817.37 |
| stop_loss | 110 | -6805.70 |
| take_profit | 86 | +7255.72 |
| trailing_stop | 350 | -5572.19 |
| trend_flip | 43 | -1799.55 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
