# Backtest — BTCUSDT 15m (`e21-55_rsi14[52-78|x42]_atr14_sl2.5_tp4_h500_trail3`)

- Bars: 303940 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=55 rsi=14 entry=[52,78] exit<42 atr=14 stop=2.5x take=4x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$0.00** (-100.00% total, -94.36% CAGR)
- Sharpe -11.80 | Sortino -9.94 | Calmar -0.94 | ann.vol 24.1%
- Max drawdown -100.00% over 303676 bars
- Trades: 8184 | win 23.4% | PF 0.45 | expectancy $-1.22
- avg win $4.21 / avg loss $-2.88 | avg hold 14 bars | exposure 36.6% | turnover 456.0x | fees $4560.01

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| rsi_exit | 515 | -1800.72 |
| stop_loss | 1822 | -7244.90 |
| take_profit | 1157 | +6202.46 |
| trailing_stop | 4343 | -6333.52 |
| trend_flip | 347 | -823.32 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
