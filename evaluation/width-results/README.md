# Независимая проверка ширины сети

Домен `W1F`, 5 760 боёв на пару политик.
В основном тесте 12×12 упорядоченных пар классов, уровни 1–5, четыре арены и обе рассадки.
В historical поровну учитываются d819 (`champion`), исходный ABI2002 control и target-aware-0090. Expert — встроенный усиленный бот. Победа, ничья и поражение дают соответственно 1, ½ и 0 очков.

## Широкая сеть против узкой

- historical. Очки -0.12 [-1.30, +1.06] п. п. Победы -1.11 [-2.29, +0.07] п. п. Для очков порог +0,5 п. п. остаётся неопределённым.
- expert. Очки -0.13 [-1.31, +1.05] п. п. Победы -0.61 [-1.79, +0.57] п. п. Для очков порог +0,5 п. п. остаётся неопределённым.

## Сравнения на общих соперниках

Совместные 95% интервалы покрывают все 12 разностей в таблице. Оба направления пары классов, обе рассадки и все политики и соперники на одном сиде входят в один кластер. Бутстрэп стратифицирован по уровню, арене и совпадению классов.

| Политика | База | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] | Порог 0,5 п. п. |
|---|---|---|---|---:|---|
| wide | narrow | historical | score | -0.12 [-1.30, +1.06] | порог +0,5 п. п. остаётся неопределённым |
| wide | narrow | historical | raw_win | -1.11 [-2.29, +0.07] | прирост свыше +0,5 п. п. исключён |
| wide | champion-1300 | historical | score | +0.95 [-0.23, +2.13] | порог +0,5 п. п. остаётся неопределённым |
| wide | champion-1300 | historical | raw_win | -0.04 [-1.22, +1.14] | порог +0,5 п. п. остаётся неопределённым |
| narrow | champion-1300 | historical | score | +1.07 [-0.11, +2.25] | порог +0,5 п. п. остаётся неопределённым |
| narrow | champion-1300 | historical | raw_win | +1.07 [-0.11, +2.25] | порог +0,5 п. п. остаётся неопределённым |
| wide | narrow | expert | score | -0.13 [-1.31, +1.05] | порог +0,5 п. п. остаётся неопределённым |
| wide | narrow | expert | raw_win | -0.61 [-1.79, +0.57] | порог +0,5 п. п. остаётся неопределённым |
| wide | champion-1300 | expert | score | +0.13 [-1.05, +1.31] | порог +0,5 п. п. остаётся неопределённым |
| wide | champion-1300 | expert | raw_win | -0.05 [-1.23, +1.13] | порог +0,5 п. п. остаётся неопределённым |
| narrow | champion-1300 | expert | score | +0.26 [-0.92, +1.44] | порог +0,5 п. п. остаётся неопределённым |
| narrow | champion-1300 | expert | raw_win | +0.56 [-0.62, +1.74] | порог +0,5 п. п. остаётся неопределённым |

## Очные встречи

Совместные 95% интервалы образуют отдельное семейство. Доля побед сравнивается с долей поражений, а средние очки — с 50%.

| Политика | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |
|---|---|---|---:|
| wide | narrow | score_minus_half | +0.02 [-1.00, +1.03] |
| wide | champion-1300 | score_minus_half | +0.92 [-0.10, +1.94] |
| narrow | champion-1300 | score_minus_half | +1.23 [+0.22, +2.25] |
| wide | narrow | raw_win_minus_raw_loss | +0.03 [-2.00, +2.07] |
| wide | champion-1300 | raw_win_minus_raw_loss | +1.84 [-0.19, +3.87] |
| narrow | champion-1300 | raw_win_minus_raw_loss | +2.47 [+0.43, +4.50] |

## Проверка при сопоставимой стоимости обучения

Для четырёх разностей срезов по бюджету рассчитано отдельное семейство интервалов.

| Политика | База | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] | Порог 0,5 п. п. |
|---|---|---|---|---:|---|
| wide-matched | narrow-matched | historical | score | -0.88 [-1.92, +0.16] | прирост свыше +0,5 п. п. исключён |
| wide-matched | narrow-matched | historical | raw_win | -1.59 [-2.63, -0.55] | ниже −0,5 п. п. |
| wide-matched | narrow-matched | expert | score | +0.10 [-0.95, +1.14] | порог +0,5 п. п. остаётся неопределённым |
| wide-matched | narrow-matched | expert | raw_win | -0.62 [-1.67, +0.42] | прирост свыше +0,5 п. п. исключён |

## Исходы боёв

| Политика | Соперник | В/Н/П | Победы | Очки | Лимит ходов / решений |
|---|---|---:|---:|---:|---:|
| champion-1300 | historical | 8953/834/7493 | 51.81% | 54.22% | 717/113 |
| narrow | historical | 9138/834/7308 | 52.88% | 55.30% | 647/184 |
| wide | historical | 8946/1177/7157 | 51.77% | 55.18% | 845/327 |
| narrow-matched | historical | 9138/834/7308 | 52.88% | 55.30% | 647/184 |
| wide-matched | historical | 8863/1079/7338 | 51.29% | 54.41% | 658/417 |
| champion-1300 | champion | 2970/277/2513 | 51.56% | 53.97% | 230/45 |
| champion-1300 | abi2002-control | 3038/260/2462 | 52.74% | 55.00% | 227/33 |
| champion-1300 | relative-update-90 | 2945/297/2518 | 51.13% | 53.71% | 260/35 |
| champion-1300 | expert | 4006/171/1583 | 69.55% | 71.03% | 146/25 |
| narrow | champion | 3032/278/2450 | 52.64% | 55.05% | 205/73 |
| narrow | abi2002-control | 3102/271/2387 | 53.85% | 56.21% | 221/48 |
| narrow | relative-update-90 | 3004/285/2471 | 52.15% | 54.63% | 221/63 |
| narrow | expert | 4038/137/1585 | 70.10% | 71.29% | 93/44 |
| wide | champion | 2957/402/2401 | 51.34% | 54.83% | 273/126 |
| wide | abi2002-control | 2999/359/2402 | 52.07% | 55.18% | 269/89 |
| wide | relative-update-90 | 2990/416/2354 | 51.91% | 55.52% | 303/112 |
| wide | expert | 4003/192/1565 | 69.50% | 71.16% | 125/67 |
| narrow-matched | champion | 3032/278/2450 | 52.64% | 55.05% | 205/73 |
| narrow-matched | abi2002-control | 3102/271/2387 | 53.85% | 56.21% | 221/48 |
| narrow-matched | relative-update-90 | 3004/285/2471 | 52.15% | 54.63% | 221/63 |
| narrow-matched | expert | 4038/137/1585 | 70.10% | 71.29% | 93/44 |
| wide-matched | champion | 2922/360/2478 | 50.73% | 53.85% | 211/146 |
| wide-matched | abi2002-control | 2961/352/2447 | 51.41% | 54.46% | 217/135 |
| wide-matched | relative-update-90 | 2980/367/2413 | 51.74% | 54.92% | 230/136 |
| wide-matched | expert | 4002/220/1538 | 69.48% | 71.39% | 115/105 |
| wide | narrow | 2680/402/2678 | 46.53% | 50.02% | 258/143 |
| wide | champion-1300 | 2746/374/2640 | 47.67% | 50.92% | 252/121 |
| narrow | champion-1300 | 2815/272/2673 | 48.87% | 51.23% | 201/71 |
| champion-1300 | champion-1300 | 2763/234/2763 | 47.97% | 50.00% | 194/40 |

`manifest.json` хранит расписание и SHA-256 входов. Сжатые файлы содержат исходы отдельных боёв и причины окончания.

## Воспроизведение

```bash
OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 PYTHONPATH=/private/tmp/dnd-spec-frozen-621964e7/python .venv/bin/python evaluation/width_holdout.py \
  --policy champion-1300 training/runs/league/checkpoints/update-1300.npz 96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31 \
  --policy narrow training/runs/width-league/narrow/checkpoints/cpu-100.npz 4f4bb5c76eb175e1028b18761df01b5206fb379930a9fbca9b19c82336edc346 \
  --policy wide training/runs/width-league/wide/checkpoints/cpu-100.npz 1c8cfe14f7a18f5455e022dfcc27376ecb273ae276a1a493914680c4d7fc51b5 \
  --policy narrow-matched training/runs/width-league/narrow/checkpoints/cpu-100.npz 4f4bb5c76eb175e1028b18761df01b5206fb379930a9fbca9b19c82336edc346 \
  --policy wide-matched training/runs/width-league/wide/cpu-matched.npz 1a40b9430dee0fcd35e400682fd1e2de3afa5bbc367a46481009a2d3ffb909af \
  --batch-size 512 --jobs 6 --output evaluation/width-results-reproduced
```
