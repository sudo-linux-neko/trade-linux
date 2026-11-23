# Backtest — BTCUSDT 15m (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3_x5`)

- Bars: 303940 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$2.62** (-99.97% total, -61.36% CAGR)
- Sharpe -7.09 | Sortino -3.47 | Calmar -0.61 | ann.vol 13.3%
- Max drawdown -99.97% over 303725 bars
- Trades: 2644 | win 22.2% | PF 0.37 | expectancy $-3.78
- avg win $10.03 / avg loss $-7.72 | avg hold 13 bars | exposure 11.6% | turnover 465.7x | fees $4657.22

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 44 | -434.02 |
| stop_loss | 587 | -5951.26 |
| take_profit | 356 | +4179.81 |
| trailing_stop | 1404 | -5644.94 |
| trend_flip | 253 | -2146.97 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
