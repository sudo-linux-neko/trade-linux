# Edge backtest — donchian ETHUSDT 4h (`don20-10_ema50_rsi[50-80]_sl2_tr0`)

- Bars: 19014 | Range: 2018-01-01 → 2026-09-07 | costs 25/10 bps | initial $10000
- Final **$16611.26** (+66.11% total, +6.02% CAGR) vs buy&hold +238.2% (excess -172.1pp)
- Sharpe 0.53 Sortino 0.44 Calmar 0.27 vol 12.6% | maxDD -22.11% (1892 bars)
- Trades 273 WR 24.9% PF 1.24 exp $24.22 | avgW $498.82 avgL $-133.21 hold 23b | expos 33.5% turn 209.7x fees $5242.17

## Exit attribution

| reason | n | pnl |
|---|---|---|
| stop_loss | 113 | -19403.28 |
| take_profit | 1 | +3562.83 |
| trend_flip | 159 | +22451.71 |

## Honesty

- CLOSE signal, NEXT OPEN fill, no lookahead. Stop→trail→take conservative.
- Long-only spot. Rank ≠ future edge: run `walkforward`-style OOS before sizing.
