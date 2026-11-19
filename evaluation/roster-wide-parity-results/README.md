# Level-five roster matchup parity

Both seats used **Greedy policy update-2901.npz**. Each fixture was played twice, swapping seats.
The schedule covers 66 cross-class pairings across four shared arenas, with 4 seeds per pair and arena.
The legacy arm assigns different levels in a quarter of fixtures and cycles levels 1–5 in the remaining fixtures. Each unequal-level arrangement also occurs with the level assignments reversed. The level-5 arms use one canonical build or all nine build combinations for each class pair.
Each seed and arena assignment is reused across arms and all nine build combinations.

This checkpoint predates the roster. The benchmark zeroed its weights on the new roster input features, matching the PPO warm start.

A win counts as one point and a draw as half a point. Class-pair scores give equal weight to each build combination. Gaps are absolute deviations from a 50% score.

| Arm | Bouts | Mean class gap | 90th-percentile class gap | ≥70–30 | ≥80–20 | Mean build gap | Draws |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Legacy levels | 2,112 | 14.9% | 31.2% | 19/66 | 8/66 | 14.9% | 10.3% |
| Level 5, canonical | 2,112 | 26.9% | 44.5% | 46/66 | 29/66 | 26.9% | 6.9% |
| Level 5, full roster | 19,008 | 25.9% | 40.1% | 45/66 | 27/66 | 27.7% | 9.9% |

### One-sided finishes

The winner must have at least 75% of their maximum hit points at knockout. Temporary hit points do not count. The all-bout denominator includes draws.

| Arm | KOs at ≥75% HP | Share of KOs | Share of all bouts |
| --- | ---: | ---: | ---: |
| Legacy levels | 1,018/1,894 | 53.7% | 48.2% |
| Level 5, canonical | 732/1,967 | 37.2% | 34.7% |
| Level 5, full roster | 7,218/17,121 | 42.2% | 38.0% |

| Change in one-sided finishes among all bouts | Percentage points | Paired 95% interval |
| --- | ---: | ---: |
| Level 5, canonical vs. Legacy levels | -13.54 | [-15.39, -11.65] |
| Level 5, full roster vs. Level 5, canonical | +3.31 | [+2.30, +4.42] |
| Level 5, full roster vs. Legacy levels | -10.23 | [-11.83, -8.50] |

| Change in mean class gap | Percentage points | Paired 95% interval |
| --- | ---: | ---: |
| Level 5, canonical vs. Legacy levels | +11.96 | [+9.80, +13.07] |
| Level 5, full roster vs. Level 5, canonical | -0.96 | [-2.37, -0.24] |
| Level 5, full roster vs. Legacy levels | +11.00 | [+8.78, +11.46] |

Intervals resample matched seed blocks within each arena, class pair, and level-mixture stratum. The lopsided-pair counts use observed scores and are sensitive to sampling noise near the thresholds. The average absolute gaps can also increase from finite-sample noise, especially in arms with fewer bouts per class pair.

The four-seed run has one mixed-level repeat per class pair and arena. Its bootstrap intervals hold those repeats fixed.

### Largest observed class-pair gaps in the full roster

- Fighter scored 95.8% against Ranger.
- Fighter scored 94.6% against Rogue.
- Monk scored 93.9% against Rogue.

The roster's canonical combination differed from the legacy level-5 preset in 0 matched bouts across outcomes, terminal reasons, and winner hit points.

These results describe the fixed controller under these arena rules. The experiment did not train a new policy.

[Fixture statistics and provenance](summary.json) · [Per-bout outcomes and seeds](bouts.npz)

Rebuild with `make release` and run:

    PYTHONPATH=python .venv/bin/python evaluation/roster_parity.py --checkpoint training/runs/long-width/checkpoints/update-2901.npz --neutralize-roster-inputs --repeats 4 --output evaluation/roster-wide-parity-results
