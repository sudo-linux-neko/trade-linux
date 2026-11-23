# Backtest — BTCUSDT 4h (`e21-89_rsi14[55-78|x42]_atr14_sl2_tp3_h500_trail3_x5`)

- Bars: 19014 | Range: 2018-01-01 → 2026-09-07
- Costs: fee 10 bps/side, slippage 5 bps | initial $10000
- Params: fast=21 slow=89 rsi=14 entry=[55,78] exit<42 atr=14 stop=2x take=3x hold<=500 trail=3x risk=1.0%

## Metrics

- Final equity: **$11258.82** (+12.59% total, +1.38% CAGR)
- Sharpe 0.37 | Sortino 0.15 | Calmar 0.20 | ann.vol 3.9%
- Max drawdown -6.74% over 8511 bars
- Trades: 106 | win 44.3% | PF 1.26 | expectancy $11.88
- avg win $129.67 / avg loss $-81.96 | avg hold 13 bars | exposure 7.2% | turnover 81.7x | fees $816.79

## Exit attribution

| reason | n | total pnl |
|---|---|---|
| stop_loss | 26 | -3010.65 |
| take_profit | 32 | +4713.44 |
| trailing_stop | 47 | -331.43 |
| trend_flip | 1 | -112.54 |

## Honesty notes (read before risking money)

- Signals on CLOSE, fills next OPEN ± slippage. No lookahead.
- Stops checked stop→trail→take (conservative when a 15m bar hits both).
- Single long-only spot position; fees on both sides; forming candle re-fetched.
- Past OOS Sharpe ≠ future Sharpe. See `walkforward` for IS→OOS decay.
- $0 data = Binance public klines (spot). No survivorship bias handling for delisted symbols.
