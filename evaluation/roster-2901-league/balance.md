# Level-5 roster balance under greedy play

The frozen warm start and final checkpoint each played itself on the same 36-build, five-arena fixture manifest.
There were eight seeds per build pair and arena, with two seat assignments for distinct builds.
Exact mirrors have one physical bout per seed. Including both focal labels would count the same fight twice.
A win scores one, a draw scores one half, and a loss scores zero.
An off-diagonal class score weights its nine build combinations equally.
Same-class aggregate and exact-mirror build scores are 50% by symmetry.
They remain in the CSVs and physical-bout totals. The gap and 80–20 summaries use 66 cross-class and 630 distinct-build cells.

| Metric | Initial | Final | Final − initial | Paired 95% interval |
| --- | ---: | ---: | ---: | ---: |
| Class-pair mean gap (pp) | 25.37 | 26.53 | 1.16 | [0.76, 1.56] |
| Class-pair 90th-percentile gap (pp) | 39.55 | 42.85 | 3.30 | [1.77, 3.99] |
| Build-pair mean gap (pp) | 26.64 | 28.12 | 1.48 | [1.16, 1.91] |
| Build-pair 90th-percentile gap (pp) | 45.00 | 46.25 | 1.25 | [0.00, 1.88] |
| KO with ≥75% HP, all bouts (%) | 35.70 | 35.77 | 0.07 | [-0.40, 0.50] |
| KO with ≥75% HP, among KOs (%) | 39.87 | 37.40 | -2.47 | [-2.96, -2.01] |

Winner HP uses actual hit points and maximum hit points at termination.

| Checkpoint | Physical bouts | Seat 0 wins | Seat 1 wins | Draws | KO | Double KO | Turn cap | Decision cap | KO with ≥75% HP |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Initial | 51,840 | 23,653 | 22,768 | 5,419 | 46,421 | 36 | 579 | 4,804 | 18,506 |
| Final | 51,840 | 24,971 | 24,609 | 2,260 | 49,580 | 17 | 535 | 1,708 | 18,543 |

| Cells | Bouts per cell | Initial observed ≥80–20 | Final observed ≥80–20 | Initial simultaneous 95% count bounds | Final simultaneous 95% count bounds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Class pairs | 720 | 23/66 | 27/66 | 4–49 | 8–44 |
| Build pairs | 80 | 293/630 | 322/630 | 0–630 | 0–630 |

The paired bootstrap resamples seeds separately within each build pair and arena while keeping both seat outcomes and both checkpoints together.
Cells close to 80–20 can cross the threshold from sampling noise. Absolute gaps have finite-sample upward bias, especially for build pairs with 40 seed blocks.
The summary includes a bootstrap bias-correction sensitivity estimate for mean gaps and conservative simultaneous Hoeffding bounds for the threshold counts.
Threshold-count bootstrap ranges in the summary are descriptive sensitivity ranges for a discontinuous statistic.
These intervals quantify combat-seed sampling under the fixed controller, roster, and arena set.

## Expert bot mirror

On the same 51,840 bouts, this controller scored mean gaps of 30.77 pp by class and 32.29 pp by build.
The observed 80–20 counts were 36/66 class cells and 406/630 build cells.
Winners retained at least 75% HP in 27,555 KOs, or 53.15% of all bouts and 54.68% of KOs.

## Original unmodified checkpoint mirror

On the same 51,840 bouts, this controller scored mean gaps of 25.35 pp by class and 26.77 pp by build.
The observed 80–20 counts were 23/66 class cells and 295/630 build cells.
Winners retained at least 75% HP in 19,204 KOs, or 37.04% of all bouts and 41.53% of KOs.

## Largest class-pair score shifts

Positive changes favor the first named side.
The approximate simultaneous bootstrap bands use the largest centered error across all distinct cells of this type.
The CSVs also give conservative simultaneous Hoeffding bounds.

| First side | Second side | Initial score (%) | Final score (%) | Change (pp) | Simultaneous bootstrap 95% (pp) |
| --- | --- | ---: | ---: | ---: | ---: |
| Bard | Warlock | 34.65 | 8.12 | -26.53 | [-32.57, -20.49] |
| Druid | Wizard | 77.43 | 93.89 | 16.46 | [10.42, 22.50] |
| Cleric | Sorcerer | 66.25 | 50.14 | -16.11 | [-22.15, -10.07] |
| Bard | Wizard | 25.97 | 39.79 | 13.82 | [7.78, 19.86] |
| Druid | Rogue | 56.25 | 68.96 | 12.71 | [6.67, 18.75] |

## Largest build-pair score shifts

Positive changes favor the first named side.
The approximate simultaneous bootstrap bands use the largest centered error across all distinct cells of this type.
The CSVs also give conservative simultaneous Hoeffding bounds.

| First side | Second side | Initial score (%) | Final score (%) | Change (pp) | Simultaneous bootstrap 95% (pp) |
| --- | --- | ---: | ---: | ---: | ---: |
| World Tree | Soulknife | 96.25 | 38.75 | -57.50 | [-81.25, -33.75] |
| Glamour | Abjurer | 7.50 | 60.00 | 52.50 | [28.75, 76.25] |
| Glamour | Draconic | 11.88 | 62.50 | 50.62 | [26.88, 74.37] |
| Glamour | Evoker | 20.00 | 68.75 | 48.75 | [25.00, 72.50] |
| Berserker | Glamour | 70.62 | 22.50 | -48.13 | [-71.88, -24.38] |

[Class-pair CSV](balance-class-pairs.csv) · [Build-pair CSV](balance-build-pairs.csv) · [Summary JSON](balance-summary.json)
