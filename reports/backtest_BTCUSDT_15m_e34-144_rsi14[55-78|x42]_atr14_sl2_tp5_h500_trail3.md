# Backtest — BTCUSDT 15m (`e34-144_rsi14[55-78|x42]_atr14_sl2_tp5_h500_trail3`)

- Bars: 303940 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=34 slow=144 rsi=14 entry=[55,78] exit<42 atr=14 stop=2x take=5x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$0.00** (-100.00% total, -90.78% CAGR)
- Sharpe -9.72 | Sortino -7.22 | Calmar -0.91 | ann.vol 24.2%
- Max drawdown -100.00% over 303675 bars
- Trades: 6397 | win 22.0% | PF 0.43 | expectancy $-1.56
- avg win $5.39 / avg loss $-3.52 | avg hold 13 bars | exposure 27.7% | turnover 460.9x | fees $4609.23

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 118 | -637.58 |
| stop_loss | 2575 | -12314.89 |
| take_profit | 601 | +5068.66 |
| trailing_stop | 3079 | -2077.15 |
| trend_flip | 24 | -39.05 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
