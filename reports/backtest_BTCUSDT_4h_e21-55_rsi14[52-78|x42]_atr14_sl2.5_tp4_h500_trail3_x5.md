# Backtest — BTCUSDT 4h (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3_x5`)

- Bars: 19014 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$9383.69** (-6.16% total, -0.73% CAGR)
- Sharpe -0.16 | Sortino -0.08 | Calmar -0.06 | ann.vol 4.0%
- Max drawdown -13.25% over 15231 bars
- Trades: 153 | win 39.9% | PF 0.90 | expectancy $-4.03
- avg win $87.81 / avg loss $-64.92 | avg hold 14 bars | exposure 11.2% | turnover 85.4x | fees $853.63

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| stop_loss | 26 | -2756.15 |
| take_profit | 25 | +3709.12 |
| trailing_stop | 86 | -566.88 |
| trend_flip | 16 | -1002.40 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
