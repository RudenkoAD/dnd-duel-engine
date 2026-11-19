"""Render the frozen S2F evaluation artifacts as a Russian Markdown report."""

import argparse
from collections import Counter
import json
from pathlib import Path


MATCHES = {
    "learned_vs_heuristic": "Модель против стандартной эвристики",
    "learned_vs_expert": "Модель против усиленной эвристики",
    "learned_vs_prior": "Модель против прежней политики",
    "learned_vs_random": "Модель против случайной политики",
    "heuristic_vs_heuristic": "Стандартная эвристика против себя",
    "expert_vs_expert": "Усиленная эвристика против себя",
    "prior_vs_heuristic": "Прежняя политика против стандартной эвристики",
    "random_vs_heuristic": "Случайная политика против стандартной эвристики",
}
CLASSES = dict(zip(
    ("barbarian", "bard", "cleric", "druid", "fighter", "monk",
     "paladin", "ranger", "rogue", "sorcerer", "warlock", "wizard"),
    ("Варвар", "Бард", "Жрец", "Друид", "Воин", "Монах",
     "Паладин", "Следопыт", "Плут", "Чародей", "Колдун", "Волшебник"),
))
TACTICS = {
    "opportunity": "Атака по возможности",
    "defend": "Защитная реакция в боевой фазе",
    "rider": "Дополнительное свойство атаки",
    "ready_release": "Выполнение подготовленного действия",
    "counterspell": "Контрзаклинание",
    "cast_when_available": "Заклинание из базового ряда Spell0–Spell5",
    "weapon_with_spell_available": "Оружие при доступном заклинании из базового ряда",
    "move_with_weapon_or_spell": "Шаг при доступной атаке оружием или базовом заклинании",
    "end_with_weapon_or_spell": "Конец хода при доступной атаке оружием или базовом заклинании",
}
POSITION_NAMES = ("запад", "восток")
TARGET_NAMES = {"burst": "Взрывная область", "moonbeam": "Лунный луч",
                "land_aid": "Помощь земли", "caltrops": "Кальтропы"}


def percent(value, digits=1):
    return f"{value:.{digits}%}".replace(".", ",")


def number(value):
    return f"{value:,}".replace(",", "\u202f")


def point_delta(value):
    return f"{100 * value:+.2f}".replace(".", ",")


def wdl(data):
    return " / ".join(number(data[field]) for field in ("wins", "draws", "losses"))


def interval(values, translate):
    return f"[{translate(values[0])} … {translate(values[1])}]"


def table(columns, rows):
    header = "| " + " | ".join(columns) + " |"
    separator = "| " + " | ".join(["---", *["---:"] * (len(columns) - 1)]) + " |"
    return [header, separator, *("| " + " | ".join(row) + " |" for row in rows)]


def breakdown(report, name):
    for cls in report["class_ids"]:
        cells = []
        for level in range(1, 6):
            entry = report["class_levels"][cls][str(level)][name]
            cells.append(f"{wdl(entry)} · {percent(entry['score'])}")
        yield [CLASSES[cls], *cells]


def reaction_counts(report, names):
    total = {key: Counter() for key in TACTICS}
    for name in names:
        for stats in report["tactical"][name].values():
            for key in TACTICS:
                total[key].update(stats["conditional_decisions"].get(key, {}))
    return total


def reaction_summary(report, name):
    for key, counts in reaction_counts(report, [name]).items():
        offered = counts["eligible"]
        chosen = counts["chosen"]
        rate = f" · {percent(chosen / offered)}" if offered else ""
        yield TACTICS[key], f"{number(chosen)} / {number(offered)}{rate}"


def target_summary(report, name):
    total = {key: Counter() for key in TARGET_NAMES}
    for stats in report["tactical"][name].values():
        for key in TARGET_NAMES:
            total[key].update(stats["ground_targets"].get(key, {}))
    for key, counts in total.items():
        if counts["chosen_cells"] == 0:
            continue
        available = counts["foe_only_available_on_board"]
        success = counts["foe_only_when_available"]
        use = f"{number(success)} / {number(available)}"
        if available:
            use += f" · {percent(success / available)}"
        yield (TARGET_NAMES[key], number(counts["chosen_cells"]), use,
               *(number(counts[tag]) for tag in ("self_only", "both", "empty")))


def replay_section(directory):
    heading = {
        "replay.json": "Боевой выбор",
        "replay-opportunity.json": "Атака по возможности",
        "replay-target.json": "Выбор точки",
    }
    for filename, title in heading.items():
        path = directory / filename
        if not path.is_file():
            continue
        data = json.loads(path.read_text())
        pair, seat = data["pair"], data["focal_seat"]
        mine = [event for event in data["decisions"] if event["actor"] == seat]
        fields = ((lambda e: e["state"]["phase"] == 0 and e["action"] < 8),
                  (lambda e: e["state"]["phase"] == 0 and e["action_name"] in ("Melee", "Ranged", "Unarmed")),
                  (lambda e: e["state"]["phase"] == 0 and e["action_name"].startswith(("Spell", "Magic"))),
                  (lambda e: e["state"]["phase"] in (1, 2, 3, 9, 15, 47)),
                  (lambda e: "target" in e and "column" in e["target"]),
                  (lambda e: "target" in e and "altitude" in e["target"]),
                  (lambda e: len(e["legal"]) == 1))
        moments = sorted({i for query in fields
                          for i in [next((j for j, event in enumerate(mine) if query(event)), -1)] if i >= 0})
        outcome = "победа модели" if data["result"] == seat else (
            "поражение модели" if data["result"] == 1 - seat else "ничья"
        )
        yield (f"### {title}\n\n[{filename}]({filename}) · {CLASSES[data['focal_class']]} / "
               f"{CLASSES[data['opponent_class']]} · уровень {pair['level']} · карта {pair['arena']} · "
               f"{POSITION_NAMES[seat]} · seed `{pair['seed']}` · {outcome}.")
        yield ""
        for index in moments:
            event = mine[index]
            target = event.get("target")
            ability = f"`{event['action_label']}`"
            if target and "column" in target:
                ability += f" (строка {1 + target['row']}, столбец {1 + target['column']}"
                if "altitude" in target:
                    ability += f", высота {5 * target['altitude']} фт."
                ability += ")"
            alternatives = len(event["legal"])
            choice = ("единственный допустимый шаг" if alternatives == 1
                      else f"{number(alternatives)} допустимых вариантов")
            observation = ""
            if event["state"]["phase"] == 1 and "15" in event["state"]["actions"]:
                observation = " Модель могла пропустить реакцию."
            if target and "altitude" in target and target["covers_foe"] and not target["covers_self"]:
                observation = " Область накрывала противника и оставляла заклинателя вне её."
            yield f"- Ход {1 + event['state']['turns']}. {ability}, {choice}.{observation}"
        yield ""


def render(report, directory):
    pairs, bouts = report["pairs"], report["bouts_per_matchup"]
    lines = [
        "# Отложенная проверка политики после self-play",
        "",
        f"{number(pairs)} пар и {number(bouts)} боёв на сравнение. Все 12 классов на уровнях 1–5 "
        "встречаются с каждым классом своего уровня, включая зеркальные бои. Для каждого "
        "сочетания используются четыре карты. Пара повторяет тот же seed в обеих позициях. "
        "Домены обучения и выбора контрольной точки отделены от финального домена `S2F`.",
        "",
        f"SHA-256 модели `{report['checkpoint_sha256']}`. "
        f"Запуск через пакетный Python CLI на нативном Rust ABI {report['arena_abi']} "
        f"({report['arena_features']} признаков, {report['arena_actions']} действий).",
        "",
    ]
    if report["prior_checkpoint_sha256"]:
        mode = ("выборку из распределения по легальным действиям"
                if report["prior_action_mode"] == "sampled" else "выбор максимума по легальным действиям")
        lines += [f"Мигрированная прежняя политика использует {mode}. SHA-256 "
                  f"`{report['prior_checkpoint_sha256']}`.", ""]
    overall = report["overall"]
    lines += [
        "## Общий результат",
        "",
        "Баллы равны доле побед плюс половина доли ничьих. Интервалы — 95%-й "
        "парный бутстреп по seed с сохранением обеих позиций и стратификацией по классу, уровню и карте.",
        "",
    ]
    rows = []
    for name, label in MATCHES.items():
        if name not in overall:
            continue
        result, costs = overall[name], report["costs"][name]
        rows.append([label, wdl(result), percent(result["score"], 2), interval(result["score_ci95"], percent),
                     *(number(costs[c]) for c in ("arena_turn_cap", "arena_decision_cap", "evaluator_decision_cap"))])
    lines += table(("Политики", "П / Н / Пор", "Баллы", "95% ДИ", "Лимит ходов", "Лимит решений", "Лимит CLI"), rows)
    paired = (
        ("learned_vs_heuristic", "vs_heuristic_self_control", "Прирост над стандартной эвристикой при том же сопернике"),
        ("learned_vs_expert", "vs_expert_self_control", "Прирост над усиленной эвристикой при том же сопернике"),
        ("learned_vs_heuristic", "vs_prior_control", "Прирост над прежней моделью при той же стандартной эвристике"),
    )
    lines.append("")
    for name, control, label in paired:
        if (entry := overall.get(name, {}).get(control)) is not None:
            lines.append(f"- {label} **{point_delta(entry['paired_score_gain'])} п.п.** "
                         f"{interval(entry['paired_score_gain_ci95'], point_delta)}.")
    lines += ["", "## Парный прирост по классам", "",
              "Дельта в процентных пунктах. В каждой колонке модель заменяет соответствующего "
              "эвристического игрока в тех же боях и против того же соперника.", ""]
    comparison_columns = [
        ("learned_vs_heuristic", "vs_heuristic_self_control", "Над стандартной"),
        ("learned_vs_expert", "vs_expert_self_control", "Над усиленной"),
    ]
    comparison_columns = [(name, field, label) for name, field, label in comparison_columns if name in overall]
    class_rows = []
    for name in report["class_ids"]:
        row = [CLASSES[name]]
        for matchup, field, _ in comparison_columns:
            comparison = report["classes"][name][matchup][field]
            gain = point_delta(comparison["paired_score_gain"])
            ci = interval(comparison["paired_score_gain_ci95"], point_delta)
            row.append(f"{gain} {ci}")
        class_rows.append(row)
    lines += table(("Класс", *(label for _, _, label in comparison_columns)), class_rows)
    lines += ["", "## Классы и уровни", "", "В каждой ячейке победы / ничьи / поражения и доля баллов.", ""]
    for name, label in MATCHES.items():
        if name not in overall:
            continue
        lines += [f"### {label}", "", *table(
            ("Класс", "Уровень 1", "Уровень 2", "Уровень 3", "Уровень 4", "Уровень 5"),
            breakdown(report, name),
        ), ""]
    learned_matches = [name for name in overall if name.startswith("learned_vs_")]
    reactions = reaction_counts(report, learned_matches)
    lines += ["## Тактические решения", "",
              "Учитываются только окна с выбором из нескольких легальных решений. "
              "Счётчики показывают поведение на посещённых состояниях.", ""]
    def uptake(key):
        stats = reactions[key]
        return f"{number(stats['chosen'])} из {number(stats['eligible'])}"
    lines += [
        f"Во всех четырёх сравнениях модель использовала атаку по возможности в {uptake('opportunity')} окон, "
        f"защитную реакцию в {uptake('defend')}, контрзаклинание в {uptake('counterspell')}. "
        f"Дополнительный эффект атаки она выбирала в {uptake('rider')} окон.",
        "",
        "Против стандартной эвристики",
        "",
              *table(("Решение", "Выбрано / доступно"), reaction_summary(report, "learned_vs_heuristic")), "",
              "Геометрия учитывает известные позиции основного противника и самого заклинателя до бросков спасения.", "",
              *table(("Область", "Выбрано точек", "Только враг при наличии такой точки", "Только боец модели", "Оба", "Никого"),
                     target_summary(report, "learned_vs_heuristic")), "",
              "## Записи боёв", "",
              "Пакетный CLI воспроизвёл записанные команды и сверил состояние и маску легальных решений на каждом шаге.", ""]
    lines.extend(replay_section(directory))
    lines += ["[Сводка по картам, позициям и классам](summary.json) · [Парные результаты](paired-outcomes.jsonl.gz)", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    report = json.loads((args.directory / "summary.json").read_text())
    if report.get("seed_domain") != "s2-final":
        parser.error("the report requires an S2F summary")
    (args.directory / "REPORT.md").write_text(render(report, args.directory), encoding="utf-8")


if __name__ == "__main__":
    main()
