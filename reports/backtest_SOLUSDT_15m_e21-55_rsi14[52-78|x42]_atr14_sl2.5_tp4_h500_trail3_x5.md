# Backtest — SOLUSDT 15m (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3_x5`)

- Bars: 212866 | Range: 2020-08-11 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$351.62** (-96.48% total, -42.37% CAGR)
- Sharpe -3.24 | Sortino -1.80 | Calmar -0.44 | ann.vol 16.6%
- Max drawdown -96.56% over 211277 bars
- Trades: 1813 | win 30.0% | PF 0.70 | expectancy $-5.32
- avg win $41.79 / avg loss $-25.52 | avg hold 13 bars | exposure 11.4% | turnover 701.2x | fees $7011.64

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 25 | -671.23 |
| stop_loss | 314 | -12297.41 |
| take_profit | 356 | +19669.25 |
| trailing_stop | 946 | -11847.85 |
| trend_flip | 172 | -4501.14 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
