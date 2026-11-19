# Level-five roster matchup parity

Both seats used **Expert bot**. Each fixture was played twice, swapping seats.
The schedule covers 66 cross-class pairings across four shared arenas, with 8 seeds per pair and arena.
The legacy arm assigns different levels in a quarter of fixtures and cycles levels 1–5 in the remaining fixtures. Each unequal-level arrangement also occurs with the level assignments reversed. The level-5 arms use one canonical build or all nine build combinations for each class pair.
Each seed and arena assignment is reused across arms and all nine build combinations.

A win counts as one point and a draw as half a point. Class-pair scores give equal weight to each build combination. Gaps are absolute deviations from a 50% score.

| Arm | Bouts | Mean class gap | 90th-percentile class gap | ≥70–30 | ≥80–20 | Mean build gap | Draws |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Legacy levels | 4,224 | 25.2% | 40.6% | 43/66 | 24/66 | 25.2% | 3.3% |
| Level 5, canonical | 4,224 | 31.4% | 49.2% | 49/66 | 41/66 | 31.4% | 2.8% |
| Level 5, full roster | 38,016 | 31.2% | 47.8% | 48/66 | 41/66 | 33.8% | 2.0% |

### One-sided finishes

The winner must have at least 75% of their maximum hit points at knockout. Temporary hit points do not count. The all-bout denominator includes draws.

| Arm | KOs at ≥75% HP | Share of KOs | Share of all bouts |
| --- | ---: | ---: | ---: |
| Legacy levels | 2,612/4,084 | 64.0% | 61.8% |
| Level 5, canonical | 2,099/4,105 | 51.1% | 49.7% |
| Level 5, full roster | 21,019/37,243 | 56.4% | 55.3% |

| Change in one-sided finishes among all bouts | Percentage points | Paired 95% interval |
| --- | ---: | ---: |
| Level 5, canonical vs. Legacy levels | -12.14 | [-13.71, -10.58] |
| Level 5, full roster vs. Level 5, canonical | +5.60 | [+4.78, +6.41] |
| Level 5, full roster vs. Legacy levels | -6.55 | [-7.93, -5.20] |

| Change in mean class gap | Percentage points | Paired 95% interval |
| --- | ---: | ---: |
| Level 5, canonical vs. Legacy levels | +6.17 | [+4.71, +7.59] |
| Level 5, full roster vs. Level 5, canonical | -0.13 | [-1.10, +0.47] |
| Level 5, full roster vs. Legacy levels | +6.04 | [+4.50, +7.12] |

Intervals resample matched seed blocks within each arena, class pair, and level-mixture stratum. The lopsided-pair counts use observed scores and are sensitive to sampling noise near the thresholds. The average absolute gaps can also increase from finite-sample noise, especially in arms with fewer bouts per class pair.

### Largest observed class-pair gaps in the full roster

- Barbarian scored 100.0% against Bard.
- Fighter scored 100.0% against Bard.
- Monk scored 100.0% against Bard.

The roster's canonical combination differed from the legacy level-5 preset in 0 matched bouts across outcomes, terminal reasons, and winner hit points.

These results describe the fixed controller under these arena rules. The experiment did not train a new policy.

[Fixture statistics and provenance](summary.json) · [Per-bout outcomes and seeds](bouts.npz)

Rebuild with `make release` and run:

    PYTHONPATH=python .venv/bin/python evaluation/roster_parity.py --output evaluation/roster-parity-results
