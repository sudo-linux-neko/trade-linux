# Edge backtest — bbrev BTCUSDT 15m (`bb20x2_rsi<x32_bw100 0.15-0.85_sl2.5_h80`)

- Bars: 303940 | Range: 2018-01-01 → 2026-09-07 | costs 10/5 bps | initial $10000
- Final **$84.52** (-99.15% total, -42.32% CAGR) vs buy&hold +485.4% (excess -584.5pp)
- Sharpe -4.21 Sortino -1.40 Calmar -0.43 vol 12.9% | maxDD -99.17% (303400 bars)
- Trades 2195 WR 47.8% PF 0.58 exp $-4.52 | avgW $12.97 avgL $-20.52 hold 8b | expos 5.9% turn 777.0x fees $7770.21

## Exit attribution

| reason | n | pnl |
|---|---|---|
| stop_loss | 906 | -22811.16 |
| take_profit | 1289 | +12895.68 |

## Honesty

- CLOSE signal, NEXT OPEN fill, no lookahead. Stop→trail→take conservative.
- Long-only spot. Rank ≠ future edge: run `walkforward`-style OOS before sizing.
