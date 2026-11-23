# Edge backtest — rsi2 BTCUSDT 1d (`rsi2[x10>x70]_sma5-200_sl3_h30`)

- Bars: 3172 | Range: 2018-01-01 → 2026-09-07 | costs 10/5 bps | initial $10000
- Final **$10031.35** (+0.31% total, +0.04% CAGR) vs buy&hold +493.3% (excess -493.0pp)
- Sharpe 0.04 Sortino 0.01 Calmar 0.01 vol 1.2% | maxDD -2.97% (689 bars)
- Trades 60 WR 71.7% PF 1.04 exp $0.52 | avgW $19.22 avgL $-46.78 hold 3b | expos 6.3% turn 10.0x fees $100.22

## Exit attribution

| reason | n | pnl |
|---|---|---|
| rsi_exit | 27 | +363.34 |
| stop_loss | 6 | -620.56 |
| take_profit | 27 | +288.57 |

## Honesty

- CLOSE signal, NEXT OPEN fill, no lookahead. Stop→trail→take conservative.
- Long-only spot. Rank ≠ future edge: run `walkforward`-style OOS before sizing.
