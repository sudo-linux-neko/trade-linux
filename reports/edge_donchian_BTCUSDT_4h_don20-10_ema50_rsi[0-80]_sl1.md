# Edge backtest — donchian BTCUSDT 4h (`don20-10_ema50_rsi[0-80]_sl1.5_tr0`)

- Bars: 19014 | Range: 2018-01-01 → 2026-09-07 | costs 10/5 bps | initial $10000
- Final **$63823.66** (+538.24% total, +23.80% CAGR) vs buy&hold +492.0% (excess +46.2pp)
- Sharpe 1.41 Sortino 1.16 Calmar 1.21 vol 16.0% | maxDD -19.60% (1892 bars)
- Trades 259 WR 31.7% PF 1.81 exp $207.81 | avgW $1465.98 avgL $-375.07 hold 23b | expos 32.0% turn 980.2x fees $9801.77

## Exit attribution

| reason | n | pnl |
|---|---|---|
| stop_loss | 132 | -56858.83 |
| take_profit | 2 | +12308.24 |
| trend_flip | 125 | +98374.25 |

## Honesty

- CLOSE signal, NEXT OPEN fill, no lookahead. Stop→trail→take conservative.
- Long-only spot. Rank ≠ future edge: run `walkforward`-style OOS before sizing.
