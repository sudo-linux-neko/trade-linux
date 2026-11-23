# Backtest — ETHUSDT 15m (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3_x5`)

- Bars: 303940 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$4.33** (-99.96% total, -59.06% CAGR)
- Sharpe -6.01 | Sortino -2.98 | Calmar -0.59 | ann.vol 14.7%
- Max drawdown -99.96% over 302989 bars
- Trades: 2629 | win 25.3% | PF 0.42 | expectancy $-3.80
- avg win $10.68 / avg loss $-8.70 | avg hold 13 bars | exposure 11.3% | turnover 419.7x | fees $4196.57

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 30 | -564.38 |
| stop_loss | 570 | -6986.04 |
| take_profit | 395 | +5480.39 |
| trailing_stop | 1392 | -5813.24 |
| trend_flip | 242 | -2112.41 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
