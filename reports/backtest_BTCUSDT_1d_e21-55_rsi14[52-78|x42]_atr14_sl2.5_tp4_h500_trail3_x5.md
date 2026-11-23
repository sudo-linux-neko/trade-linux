# Backtest — BTCUSDT 1d (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3_x5`)

- Bars: 3172 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$10036.56** (+0.37% total, +0.04% CAGR)
- Sharpe 0.04 | Sortino 0.02 | Calmar 0.01 | ann.vol 1.4%
- Max drawdown -4.71% over 1482 bars
- Trades: 21 | win 28.6% | PF 1.04 | expectancy $1.74
- avg win $152.15 / avg loss $-58.42 | avg hold 13 bars | exposure 8.4% | turnover 4.9x | fees $48.53

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| stop_loss | 3 | -314.68 |
| take_profit | 4 | +635.50 |
| trailing_stop | 11 | -48.64 |
| trend_flip | 3 | -235.62 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
