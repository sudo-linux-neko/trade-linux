# Backtest — BTCUSDT 1d (`e21-144_rsi14[50-78|x42]_atr14_sl3_tp5_h500_trail3_x5`)

- Bars: 3172 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=144 rsi=14 entry=[50,78] exit<42 atr=14 stop=3x take=5x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$10431.24** (+4.31% total, +0.49% CAGR)
- Sharpe 0.53 | Sortino 0.18 | Calmar 0.30 | ann.vol 0.9%
- Max drawdown -1.62% over 1259 bars
- Trades: 11 | win 45.5% | PF 2.92 | expectancy $39.20
- avg win $131.23 / avg loss $-37.49 | avg hold 13 bars | exposure 4.6% | turnover 2.2x | fees $21.55

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| end_of_data | 1 | +18.50 |
| take_profit | 3 | +500.36 |
| trailing_stop | 5 | +36.16 |
| trend_flip | 2 | -123.78 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
