# Независимая проверка лиги со специалистами

Домен `X1F`. На одну пару политик — 5 760 боёв. У политик совпадают сиды, классы, уровни, карты и рассадки. Баллы за победу, ничью и поражение равны 1, ½ и 0 соответственно.

Группа historical поровну учитывает три замороженных исторических чекпойнта. Имя champion в этой группе обозначает d819 из прежнего теста, а champion-1300 — стартовую политику нового эксперимента.

## Итоги по очкам

- treatment против champion-1300. historical — интервал включает ноль, -0.03 [-1.23, +1.17] п. п. expert — интервал включает ноль, +0.69 [-0.50, +1.89] п. п.
- treatment против ordinary. historical — интервал включает ноль, -0.68 [-1.88, +0.52] п. п. expert — интервал включает ноль, -0.26 [-1.46, +0.94] п. п.
- ordinary против champion-1300. historical — интервал включает ноль, +0.65 [-0.55, +1.85] п. п. expert — интервал включает ноль, +0.95 [-0.24, +2.15] п. п.

Квадратные скобки показывают совместные 95% интервалы.

| Политика | Соперник | В/Н/П | Победы | Очки за бой |
|---|---|---:|---:|---:|
| champion-1300 | historical | 8879/918/7483 | 51.38% | 54.04% |
| ordinary | historical | 8848/1204/7228 | 51.20% | 54.69% |
| treatment | historical | 8882/902/7496 | 51.40% | 54.01% |
| ordinary-endpoint | historical | 9026/1015/7239 | 52.23% | 55.17% |
| treatment-endpoint | historical | 8963/918/7399 | 51.87% | 54.53% |
| champion-1300 | champion | 2950/322/2488 | 51.22% | 54.01% |
| champion-1300 | abi2002-control | 2933/278/2549 | 50.92% | 53.33% |
| champion-1300 | relative-update-90 | 2996/318/2446 | 52.01% | 54.77% |
| champion-1300 | expert | 3945/171/1644 | 68.49% | 69.97% |
| champion-1300 | champion-1300 | 2744/272/2744 | 47.64% | 50.00% |
| champion-1300 | specialist-401 | 2724/259/2777 | 47.29% | 49.54% |
| champion-1300 | specialist-402 | 2868/244/2648 | 49.79% | 51.91% |
| ordinary | champion | 2900/394/2466 | 50.35% | 53.77% |
| ordinary | abi2002-control | 2957/397/2406 | 51.34% | 54.78% |
| ordinary | relative-update-90 | 2991/413/2356 | 51.93% | 55.51% |
| ordinary | expert | 3959/253/1548 | 68.73% | 70.93% |
| ordinary | champion-1300 | 2683/419/2658 | 46.58% | 50.22% |
| ordinary | specialist-401 | 2718/340/2702 | 47.19% | 50.14% |
| ordinary | specialist-402 | 2888/339/2533 | 50.14% | 53.08% |
| treatment | champion | 2937/313/2510 | 50.99% | 53.71% |
| treatment | abi2002-control | 2976/277/2507 | 51.67% | 54.07% |
| treatment | relative-update-90 | 2969/312/2479 | 51.55% | 54.25% |
| treatment | expert | 4000/141/1619 | 69.44% | 70.67% |
| treatment | champion-1300 | 2711/269/2780 | 47.07% | 49.40% |
| treatment | specialist-401 | 2765/242/2753 | 48.00% | 50.10% |
| treatment | specialist-402 | 2895/242/2623 | 50.26% | 52.36% |
| treatment | ordinary | 2681/396/2683 | 46.55% | 49.98% |
| ordinary-endpoint | champion | 2959/355/2446 | 51.37% | 54.45% |
| ordinary-endpoint | abi2002-control | 3069/314/2377 | 53.28% | 56.01% |
| ordinary-endpoint | relative-update-90 | 2998/346/2416 | 52.05% | 55.05% |
| ordinary-endpoint | expert | 3986/167/1607 | 69.20% | 70.65% |
| treatment-endpoint | champion | 2953/302/2505 | 51.27% | 53.89% |
| treatment-endpoint | abi2002-control | 2984/305/2471 | 51.81% | 54.45% |
| treatment-endpoint | relative-update-90 | 3026/311/2423 | 52.53% | 55.23% |
| treatment-endpoint | expert | 3958/211/1591 | 68.72% | 70.55% |
| treatment-endpoint | ordinary-endpoint | 2727/333/2700 | 47.34% | 50.23% |

## Парные разности на общих соперниках

Интервалы совместно покрывают двенадцать сравнений при номинальном уровне 95%. Стратифицированный бутстрэп сохраняет вместе оба порядка классов, обе рассадки и результаты всех политик и соперников на одном сиде. Страты заданы уровнем, ареной и совпадением классов.

| Политика | База сравнения | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |
|---|---|---|---|---:|
| treatment | champion-1300 | historical | score | -0.03 [-1.23, +1.17] |
| treatment | champion-1300 | historical | raw_win | +0.02 [-1.18, +1.22] |
| treatment | ordinary | historical | score | -0.68 [-1.88, +0.52] |
| treatment | ordinary | historical | raw_win | +0.20 [-1.00, +1.39] |
| ordinary | champion-1300 | historical | score | +0.65 [-0.55, +1.85] |
| ordinary | champion-1300 | historical | raw_win | -0.18 [-1.38, +1.02] |
| treatment | champion-1300 | expert | score | +0.69 [-0.50, +1.89] |
| treatment | champion-1300 | expert | raw_win | +0.95 [-0.24, +2.15] |
| treatment | ordinary | expert | score | -0.26 [-1.46, +0.94] |
| treatment | ordinary | expert | raw_win | +0.71 [-0.49, +1.91] |
| ordinary | champion-1300 | expert | score | +0.95 [-0.24, +2.15] |
| ordinary | champion-1300 | expert | raw_win | +0.24 [-0.95, +1.44] |

## Вторичная проверка при общем лимите CPU

Совместный интервал покрывает четыре разности в этой таблице.

| База сравнения | Соперник | Метрика | Δ, п. п. [совместный 95% ДИ] |
|---|---|---|---:|
| ordinary-endpoint | historical | score | -0.65 [-1.69, +0.40] |
| ordinary-endpoint | historical | raw_win | -0.36 [-1.41, +0.68] |
| ordinary-endpoint | expert | score | -0.10 [-1.15, +0.94] |
| ordinary-endpoint | expert | raw_win | -0.49 [-1.53, +0.56] |

## Очные матчи финалистов

Отклонение очков за бой от 50%, с отдельным парным 95% интервалом.

- ordinary против champion-1300 — +0.22 [-0.65, +1.09] п. п.
- treatment против champion-1300 — -0.60 [-1.43, +0.25] п. п.
- treatment против ordinary — -0.02 [-0.86, +0.82] п. п.
- treatment-endpoint против ordinary-endpoint — +0.23 [-0.59, +1.02] п. п.

## Специалисты против champion-1300

| Политика | Соперник | В/Н/П | Победы | Очки за бой |
|---|---|---:|---:|---:|
| specialist-401 | champion-1300 | 2777/259/2724 | 48.21% | 50.46% |
| specialist-402 | champion-1300 | 2648/244/2868 | 45.97% | 48.09% |

### specialist-401 по классу специалиста

| Политика | Соперник | В/Н/П | Победы | Очки за бой |
|---|---|---:|---:|---:|
| Barbarian | champion-1300 | 299/3/178 | 62.29% | 62.60% |
| Bard | champion-1300 | 161/4/315 | 33.54% | 33.96% |
| Cleric | champion-1300 | 203/48/229 | 42.29% | 47.29% |
| Druid | champion-1300 | 249/14/217 | 51.88% | 53.33% |
| Fighter | champion-1300 | 241/58/181 | 50.21% | 56.25% |
| Monk | champion-1300 | 275/11/194 | 57.29% | 58.44% |
| Paladin | champion-1300 | 335/44/101 | 69.79% | 74.38% |
| Ranger | champion-1300 | 196/3/281 | 40.83% | 41.15% |
| Rogue | champion-1300 | 183/1/296 | 38.12% | 38.23% |
| Sorcerer | champion-1300 | 301/7/172 | 62.71% | 63.44% |
| Warlock | champion-1300 | 154/64/262 | 32.08% | 38.75% |
| Wizard | champion-1300 | 180/2/298 | 37.50% | 37.71% |

### specialist-402 по классу специалиста

| Политика | Соперник | В/Н/П | Победы | Очки за бой |
|---|---|---:|---:|---:|
| Barbarian | champion-1300 | 302/3/175 | 62.92% | 63.23% |
| Bard | champion-1300 | 143/1/336 | 29.79% | 29.90% |
| Cleric | champion-1300 | 193/46/241 | 40.21% | 45.00% |
| Druid | champion-1300 | 220/17/243 | 45.83% | 47.60% |
| Fighter | champion-1300 | 234/64/182 | 48.75% | 55.42% |
| Monk | champion-1300 | 260/9/211 | 54.17% | 55.10% |
| Paladin | champion-1300 | 342/29/109 | 71.25% | 74.27% |
| Ranger | champion-1300 | 194/1/285 | 40.42% | 40.52% |
| Rogue | champion-1300 | 174/2/304 | 36.25% | 36.46% |
| Sorcerer | champion-1300 | 279/10/191 | 58.13% | 59.17% |
| Warlock | champion-1300 | 144/58/278 | 30.00% | 36.04% |
| Wizard | champion-1300 | 163/4/313 | 33.96% | 34.38% |

Манифест хранит хэши и расписание, а сжатые файлы содержат результаты отдельных боёв.

## Воспроизведение

Команда ниже использует зафиксированную локальную сборку. Для проверки на другой машине соберите совместимую библиотеку, замените `PYTHONPATH` и передайте SHA-256 своей сборки в `--expected-native-sha`.

```bash
OPENBLAS_NUM_THREADS=1 VECLIB_MAXIMUM_THREADS=1 PYTHONPATH=/private/tmp/dnd-spec-frozen-621964e7/python .venv/bin/python evaluation/specialist_holdout.py \
  --policy champion-1300 training/runs/league/checkpoints/update-1300.npz 96f9bddabc0eeff4a70c111db443dea064490c6e576ff8bb569966024cb26e31 \
  --policy ordinary training/runs/specialist-leagues/control/checkpoints/update-1000.npz 426795d4ee218115c2a95a570dcecf70d6a2c5fa11ed6de0ca93b6525f955100 \
  --policy treatment training/runs/specialist-leagues/treatment/checkpoints/update-0200.npz cfaa5f39fe56ac4e3a8095a5cbef3df93965ed13640029c7f4063a317d0e081f \
  --policy specialist-401 training/runs/specialists/champion-start/update-0150.npz 50833cbb06ea875b724c008eb0d255b1a4da93a3313c78e5520b918efe000923 \
  --policy specialist-402 training/runs/specialists/control-start/update-0200.npz 6f78550df56d6c030d0624373ddeb72e85d60f11fdcfabb99c7e473675bb77da \
  --policy ordinary-endpoint training/runs/specialist-leagues/control/checkpoints/update-1200.npz 393da9cf45ee76971e9ec1369df8623f358f39dd42d768e28e90a2962c492876 \
  --policy treatment-endpoint training/runs/specialist-leagues/treatment/compute-audit/update-0670.npz 232509c79af1156d8432c3ef390c801b7b6d65c58d10f3e505878578b098823b \
  --expected-native-sha 621964e798659ba534fab6c8494664c8e7f9d91abb9a4e28ccb0ed9f40888e3f \
  --batch-size 512 --jobs 3 --output evaluation/specialist-results-reproduced
```
