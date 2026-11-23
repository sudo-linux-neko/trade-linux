# Backtest — BTCUSDT 15m (`e34-144_rsi14[55-78|x42]_atr14_sl2_tp5_h500_trail3_x5`)

- Bars: 303940 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=34 slow=144 rsi=14 entry=[55,78] exit<42 atr=14 stop=2x take=5x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$191.92** (-98.08% total, -36.60% CAGR)
- Sharpe -4.37 | Sortino -1.49 | Calmar -0.37 | ann.vol 10.3%
- Max drawdown -98.09% over 303514 bars
- Trades: 1210 | win 21.9% | PF 0.36 | expectancy $-8.11
- avg win $20.88 / avg loss $-16.23 | avg hold 14 bars | exposure 5.4% | turnover 457.2x | fees $4571.81

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 10 | -298.66 |
| stop_loss | 506 | -10629.43 |
| take_profit | 100 | +3367.28 |
| trailing_stop | 576 | -1969.75 |
| trend_flip | 18 | -277.51 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
