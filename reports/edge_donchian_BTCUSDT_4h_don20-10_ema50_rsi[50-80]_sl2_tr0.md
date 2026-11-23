# Edge backtest — donchian BTCUSDT 4h (`don20-10_ema50_rsi[50-80]_sl2_tr0`)

- Bars: 19014 | Range: 2018-01-01 → 2026-09-07 | costs 4/1 bps | initial $10000
- Final **$45383.87** (+353.84% total, +19.03% CAGR) vs buy&hold +492.0% (excess -138.2pp)
- Sharpe 1.43 Sortino 1.20 Calmar 1.28 vol 12.7% | maxDD -14.84% (1892 bars)
- Trades 254 WR 32.7% PF 1.89 exp $139.31 | avgW $905.50 avgL $-232.59 hold 25b | expos 33.5% turn 537.3x fees $2149.37

## Exit attribution

| reason | n | pnl |
|---|---|---|
| stop_loss | 103 | -30657.70 |
| take_profit | 2 | +9408.81 |
| trend_flip | 149 | +56632.76 |

## Honesty

- CLOSE signal, NEXT OPEN fill, no lookahead. Stop→trail→take conservative.
- Long-only spot. Rank ≠ future edge: run `walkforward`-style OOS before sizing.
