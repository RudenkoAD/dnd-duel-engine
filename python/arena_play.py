"""Play against the learned arena policy and record or view a duel."""
import argparse
import hashlib
from io import BytesIO
import json
from pathlib import Path
import re
import secrets
import sys

import numpy as np


DIRECTIONS = (
    ("север ↑", 0, -1), ("северо-восток ↗", 1, -1),
    ("восток →", 1, 0), ("юго-восток ↘", 1, 1),
    ("юг ↓", 0, 1), ("юго-запад ↙", -1, 1),
    ("запад ←", -1, 0), ("северо-запад ↖", -1, -1),
)
MOVE_LABELS = dict(zip((
    "north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west",
), DIRECTIONS))
MOVE_LABELS.update(zip(("n", "ne", "e", "se", "s", "sw", "w", "nw"), DIRECTIONS))
TILES = {0: "·", 1: "█", 2: "▒", 3: "♣"}
YES_NO = {True: "да", False: "нет"}
QUIT = {"q", "й", "выход"}
REPLAY_VERSION = 3


class QuitGame(Exception):
    """An intentional exit or closed terminal input."""


def read_choice(prompt, options, default=None):
    while True:
        print(prompt)
        for key, label in options.items():
            print(f"  {key:>2}  {label}")
        suffix = f" [{default}]" if default is not None else ""
        try:
            answer = input(f"Выбор{suffix} · q для выхода › ").strip().lower()
            if not sys.stdin.isatty():
                print()
        except (EOFError, KeyboardInterrupt):
            raise QuitGame from None
        if answer in QUIT:
            raise QuitGame
        if not answer and default is not None:
            return default
        if answer in options:
            return answer
        print("Выберите один из указанных пунктов.\n")


def coordinate(x, y):
    return f"{chr(65 + x)}{y + 1}"


def render_map(tiles, positions, human, caltrops=(), target_rows=(), target_cells=(), entities=(), objects=()):
    symbols = ("Я", "ИИ") if human == 0 else ("ИИ", "Я")
    occupants = {position: symbol for position, symbol in zip(positions, symbols) if position is not None}
    hazards = {tuple(position) for position in caltrops if position is not None}
    rows = ["     " + " ".join(f"{chr(65 + x):>2}" for x in range(len(tiles[0])))]
    overlays = dict.fromkeys(target_cells, "+")
    overlays.update({point: "▪" for obj in objects if (point := fighter_position(obj)) is not None})
    overlays.update(dict.fromkeys(hazards, "^"))
    displays = {}
    for entity in entities:
        if position := fighter_position(entity):
            label, marker = entity_display(entity)
            symbol = marker.upper() if entity.get("side") == human else marker
            overlays[position] = symbol
            displays[symbol] = label
    overlays.update(occupants)
    for y, row in enumerate(tiles):
        cells = [overlays.get((x, y), TILES[tile]) for x, tile in enumerate(row)]
        mark = "›" if y in target_rows else " "
        rows.append(f"{mark} {y + 1}  " + " ".join(f"{cell:>2}" for cell in cells))
    legend = ["· свободно", "█ стена", "▒ труднопроходимая клетка"]
    if any(3 in row for row in tiles):
        legend.append("♣ заросли · 4 фута движения за фут")
    if hazards:
        legend.append("^ шипы")
    if objects:
        legend.append("▪ предмет")
    if target_cells:
        legend.append("+ доступная клетка")
    if target_rows:
        legend.append("› выбранный ряд" if target_cells else "› доступный ряд")
    legend.extend(f"{symbol} {label}" for symbol, label in displays.items())
    rows.append("     " + "   ".join(legend))
    return "\n".join(rows)


def write_replay(path, data):
    path = Path(path)
    with path.open("x", encoding="utf-8") as handle:
        json.dump(data, handle, ensure_ascii=False, indent=2)
        handle.write("\n")
    print(f"Запись боя сохранена в {path}")


def replay(path):
    with Path(path).open(encoding="utf-8") as handle:
        record = json.load(handle)
    if record.get("version") not in (1, 2, REPLAY_VERSION):
        raise ValueError("Неподдерживаемая версия записи боя")
    print(record["intro"])
    for frame in record["frames"]:
        if frame["screen"]:
            print(frame["screen"])
        print(frame["message"])
    print(record["final_screen"])
    print(record["result"])


CLASS_NAMES = {
    "barbarian": "Варвар",
    "bard": "Бард",
    "cleric": "Жрец",
    "druid": "Друид",
    "fighter": "Воин",
    "monk": "Монах",
    "paladin": "Паладин",
    "ranger": "Следопыт",
    "rogue": "Плут",
    "sorcerer": "Чародей",
    "warlock": "Колдун",
    "wizard": "Волшебник",
}
SUBCLASS_NAMES = {
    "Berserker": "Берсерк", "Zealot": "Фанатик", "World Tree": "Мировое древо",
    "Lore": "Коллегия знаний", "Valor": "Коллегия доблести", "Glamour": "Коллегия очарования",
    "Life": "Домен жизни", "War": "Домен войны", "Trickery": "Домен обмана",
    "Land": "Круг земли", "Stars": "Круг звёзд", "Sea": "Круг моря",
    "Champion": "Чемпион", "Battle Master": "Мастер боевых искусств", "Psi Warrior": "Пси-воин",
    "Open Hand": "Открытая ладонь", "Mercy": "Милосердие", "Elements": "Стихии",
    "Devotion": "Клятва преданности", "Ancients": "Клятва древних", "Glory": "Клятва славы",
    "Hunter": "Охотник", "Fey Wanderer": "Странник фей", "Gloom Stalker": "Сумрачный охотник",
    "Thief": "Вор", "Assassin": "Убийца", "Soulknife": "Клинок души",
    "Draconic": "Драконья магия", "Aberrant": "Аберрантная магия", "Clockwork": "Заводная магия",
    "Fiend": "Исчадие", "Archfey": "Архифея", "Great Old One": "Великий Древний",
    "Evoker": "Школа воплощения", "Abjurer": "Школа ограждения", "Diviner": "Школа прорицания",
}


ROSTER_ACTIONS = {
    "Hunger of Hadar": "Голод Хадара", "Arms of Hadar": "Руки Хадара",
    "Mind Sliver": "Расщепление разума", "Faerie Fire": "Огонь фей",
    "Blink": "Мерцание", "Plant Growth": "Рост растений", "Phantasmal Force": "Воображаемая сила",
    "Tasha's Hideous Laughter": "Отвратительный смех Таши", "Hideous Laughter": "Отвратительный смех Таши",
    "Sleep": "Сон", "Detect Thoughts": "Обнаружение мыслей", "Calm Emotions": "Умиротворение",
    "See Invisibility": "Видение невидимого", "Charm Person": "Очарование личности",
    "Disguise Self": "Маскировка", "Mirror Image": "Отражения", "Rope Trick": "Верёвочный трюк",
    "Alarm": "Сигнал тревоги", "Nondetection": "Необнаружимость", "Fog Cloud": "Туманное облако",
    "Gust of Wind": "Порыв ветра", "Lightning Bolt": "Молния", "Invisibility": "Невидимость",
    "Pass without Trace": "Бесследное передвижение", "Crusader's Mantle": "Мантия крестоносца",
    "Enhance Ability": "Улучшение характеристики", "Water Breathing": "Подводное дыхание",
    "Speak with Animals": "Разговор с животными", "Elementalism": "Власть над стихиями",
    "Protective Field": "Защитное поле", "Psionic Strike": "Псионический удар",
    "Precision Attack": "Точная атака", "Trip Attack": "Сбивающая атака", "Parry": "Парирование",
    "Hand of Healing": "Длань исцеления", "Hand of Harm": "Длань вреда",
    "Mantle of Inspiration": "Мантия вдохновения", "Mantle of Majesty": "Мантия величия",
    "War Priest attack": "Атака жреца войны", "Blessing of the Trickster": "Благословение обманщика",
    "Invoke Duplicity": "Призвать двойника", "Move duplicate": "Переместить двойника",
    "Dismiss duplicate": "Отпустить двойника", "Peerless Athlete": "Непревзойдённый атлет",
    "Nature's Wrath": "Гнев природы", "Starry Form (Archer)": "Звёздная форма · Лучник",
    "Starry Form (Chalice)": "Звёздная форма · Чаша", "Starry Form (Dragon)": "Звёздная форма · Дракон",
    "Fire Archer arrow": "Выстрел Лучника", "End Starry Form": "Завершить звёздную форму",
    "Manifest Wrath of the Sea": "Призвать гнев моря", "Strike with Wrath of the Sea": "Удар гневом моря",
    "Dismiss Wrath of the Sea": "Унять гнев моря", "Taunting Step": "Провоцирующий шаг",
    "Refreshing Step": "Освежающий шаг",
}


SUBCLASS_DESCRIPTIONS = {
    "Berserker": "Безрассудная атака в ярости открывает дополнительный урон неистовства.",
    "Zealot": "В ярости добавляет излучение или некротический урон. Может лечить себя запасом костей.",
    "World Tree": "Получает временные хиты в начале ярости и может дать их другому существу.",
    "Lore": "Тратит вдохновение на «Острые слова», чтобы уменьшить результат броска противника.",
    "Valor": "Боевое вдохновение помогает союзнику усилить урон или защититься от попадания.",
    "Glamour": "Магия очарования дополняет заклинания. Мантия дарит временные хиты и перемещение реакцией.",
    "Life": "Усиливает лечение заклинаниями и распределяет хиты «Сохранением жизни».",
    "War": "Может атаковать бонусным действием и превратить промах в попадание «Направленным ударом».",
    "Trickery": "Создаёт двойника, может поменяться с ним местами или выбрать его источником заклинания.",
    "Land": "Круг засушливой земли даёт «Огненный шар». «Помощь земли» сочетает урон и лечение.",
    "Stars": "Выбирает звёздную форму для стрельбы, лечения или концентрации. Карта даёт «Направляющий снаряд».",
    "Sea": "Создаёт вокруг себя морские брызги, наносит урон холодом и может оттолкнуть цель.",
    "Champion": "Чаще наносит критические попадания и получает преимущество на инициативу.",
    "Battle Master": "Тратит кости превосходства на точную атаку, сбивание с ног и парирование.",
    "Psi Warrior": "Использует псионические кости для защиты, дополнительного урона и телекинеза.",
    "Open Hand": "Удары шквалом могут толкнуть, сбить с ног или лишить цель возможности провести провоцированную атаку.",
    "Mercy": "Один и тот же запас фокуса питает лечение касанием и некротический урон в ближнем бою.",
    "Elements": "В стихийной форме достаёт безоружным ударом на 15 футов и может перемещать цель.",
    "Devotion": "Священное оружие использует Харизму для усиления точности и может наносить урон излучением.",
    "Ancients": "«Гнев природы» сковывает выбранных существ вблизи. «Туманный шаг» помогает менять позицию.",
    "Glory": "После Божественной кары распределяет временные хиты. Усиливает прыжки и физические проверки.",
    "Hunter": "Получает сведения об отмеченной добыче и наносит дополнительный урон по раненой цели.",
    "Fey Wanderer": "Усиливает попадания психическим уроном и получает «Очарование личности» и «Туманный шаг».",
    "Gloom Stalker": "Быстрее действует в начале боя, скрывается от тёмного зрения и усиливает удары психическим уроном.",
    "Thief": "Быстрыми руками взаимодействует с предметами бонусным действием и лучше лазает.",
    "Assassin": "Преимущество при броске инициативы и усиленная скрытая атака в первом раунде.",
    "Soulknife": "Создаёт психические клинки для ближних и метательных атак и усиливает проверки навыков.",
    "Draconic": "Драконья стойкость повышает хиты и защиту без доспехов.",
    "Aberrant": "Владеет псионическими заклинаниями, включая «Расщепление разума» и «Голод Хадара».",
    "Clockwork": "Восстанавливает равновесие реакцией, отменяя преимущество или помеху перед броском.",
    "Fiend": "Получает временные хиты благодаря Благословению Темнейшего при подходящем нокауте.",
    "Archfey": "Получает дополнительные применения «Туманного шага», с защитным или провоцирующим эффектом.",
    "Great Old One": "Может заменить урон заклинания психическим и убрать часть компонентов у иллюзий и чар.",
    "Evoker": "Атакующие заговоры могут нанести часть урона даже после промаха или успешного спасброска.",
    "Abjurer": "Магическая защита поглощает урон. Заклинания ограждения и трата ячеек восстанавливают её.",
    "Diviner": "Заранее получает два результата d20 и может подставить их перед броском.",
}


def subclass_name(build):
    return SUBCLASS_NAMES.get(build["name"], build["name"])


COMMON_ACTIONS = {
    "end turn": ("Закончить ход", "оставшиеся перемещение и действия сгорают"),
    "melee attack": ("Атака в ближнем бою", "расходует доступную атаку"),
    "ranged attack": ("Дальнобойная атака", "расходует доступную атаку, нужна прямая видимость"),
    "dash": ("Рывок", "расходует действие, даёт дополнительное перемещение"),
    "disengage": ("Отход", "расходует действие, перемещение до конца хода не провоцирует атак"),
    "dodge": ("Уклонение", "расходует действие, видимые враги атакуют с помехой"),
    "opportunity attack": ("Провоцированная атака", "расходует реакцию, происходит до перемещения цели"),
    "pass": ("Пропустить", "продолжить без реакции или дополнительного эффекта"),
    "hide": ("Спрятаться", "действие · проверка Скрытности СЛ 15 за укрытием"),
    "search": ("Искать соперника", "действие · проверка Восприятия против Скрытности соперника"),
    "stand": ("Встать", "расходует половину скорости"),
    "drop prone": ("Лечь", "состояние «Сбит с ног»"),
    "grapple": ("Захват", "заменяет одну атаку"),
    "shove (push)": ("Оттолкнуть", "заменяет одну атаку"),
    "shove (knock prone)": ("Сбить с ног", "заменяет одну атаку"),
    "escape grapple (athletics)": ("Вырваться Атлетикой", "действие · проверка Силы (Атлетика)"),
    "escape grapple (acrobatics)": ("Вырваться Акробатикой", "действие · проверка Ловкости (Акробатика)"),
    "release grapple": ("Отпустить захват", "освобождает захваченную цель"),
    "drop concentration": ("Прекратить концентрацию", "снимает поддерживаемый эффект"),
}
SEATS = {"1": "Запад (A4)", "2": "Восток (I4)"}


def select_class(prompt, selected, default):
    if selected is not None:
        return selected
    classes = list(CLASS_NAMES)
    options = {str(i + 1): CLASS_NAMES[name] for i, name in enumerate(classes)}
    key = read_choice(prompt, options, str(classes.index(default) + 1))
    return classes[int(key) - 1]


def select_subclass(prompt, class_name, selected, roster):
    builds = [b for b in roster if b["class_name"].lower() == class_name]
    if selected is None:
        menu = {str(i): f"{subclass_name(b)} · {SUBCLASS_DESCRIPTIONS[b['name']]}" for i, b in enumerate(builds, 1)}
        selected = int(read_choice(prompt, menu, "1"))
    return builds[selected - 1]


def parse_seed(value):
    try:
        seed = int(value, 0)
    except ValueError:
        raise argparse.ArgumentTypeError("Введите целое число для зерна") from None
    if not 0 <= seed < 2**64:
        raise argparse.ArgumentTypeError("Зерно должно быть в диапазоне 0 … 2⁶⁴−1")
    return seed


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description="Пошаговая дуэль против обученного ИИ")
    parser.add_argument("--class", dest="player_class", choices=CLASS_NAMES, help="ваш класс")
    parser.add_argument("--level", type=int, choices=[5], help=argparse.SUPPRESS)
    parser.add_argument("--opponent", choices=CLASS_NAMES, help="класс соперника")
    parser.add_argument("--opponent-level", type=int, choices=[5], help=argparse.SUPPRESS)
    parser.add_argument("--subclass", type=int, choices=(1, 2, 3), help="номер вашего подкласса в меню класса")
    parser.add_argument("--opponent-subclass", type=int, choices=(1, 2, 3), help="номер подкласса соперника")
    parser.add_argument("--seat", choices=("west", "east"), help="ваша сторона арены")
    parser.add_argument("--seed", type=parse_seed, help="зерно карты, инициативы и бросков")
    parser.add_argument("--checkpoint", type=Path, help="путь к весам ИИ")
    parser.add_argument("--sample", action="store_true", help="выбирать действия ИИ случайно по вероятностям модели")
    records = parser.add_mutually_exclusive_group()
    records.add_argument("--save", type=Path, help="сохранить запись боя в новый JSON-файл")
    records.add_argument("--replay", type=Path, help="воспроизвести JSON-запись боя")
    return parser.parse_args(argv)


def choose_action(actions):
    menu = {str(i + 1): description for i, (_, description) in enumerate(actions)}
    index = read_choice("Ваше решение", menu)
    return actions[int(index) - 1][0]


def choose_setup(args, roster):
    print("═══ АРЕНА · Дуэль с ИИ ═══\n")
    player = select_class("Ваш класс", args.player_class, "fighter")
    build = select_subclass("Ваш подкласс · уровень 5", player, args.subclass, roster)
    opponent = select_class("Класс соперника", args.opponent, "wizard")
    enemy = select_subclass("Подкласс соперника · уровень 5", opponent, args.opponent_subclass, roster)
    human = {"west": 0, "east": 1}[args.seat] if args.seat else int(read_choice("Ваша сторона", SEATS, "1")) - 1
    return build, enemy, human


MARTIAL_ACTIONS = {
    "rage": ("Ярость", "бонусное действие и заряд · усиливает ближние атаки и даёт сопротивление", "self"),
    "reckless attack": ("Безрассудная атака", "первая ближняя атака с преимуществом · враг тоже получает преимущество", "enemy"),
    "extend rage": ("Продлить ярость", "бонусное действие", "self"),
    "second wind": ("Второе дыхание", "бонусное действие и заряд · восстанавливает хиты", "self"),
    "action surge": ("Всплеск действий", "заряд · даёт дополнительное действие", "self"),
    "deflect attacks": ("Отражение атак", "реакция · уменьшает входящий урон", "enemy"),
    "stunning strike": ("Ошеломляющий удар", "очко концентрации · спасбросок Телосложения цели", "enemy"),
    "bonus unarmed strike": ("Безоружный удар", "бонусное действие", "enemy"),
    "flurry of blows": ("Шквал ударов", "бонусное действие и очко концентрации · два удара", "enemy"),
    "patient defense": ("Терпеливая защита", "бонусное действие и очко концентрации · Уклонение и Отход", "self"),
    "step of the wind": ("Поступь ветра", "бонусное действие · Рывок", "self"),
    "patient defense: disengage": ("Терпеливая защита: Отход", "бонусное действие", "self"),
    "step of the wind: disengage": ("Поступь ветра: Рывок и Отход", "бонусное действие и очко концентрации", "self"),
    "divine smite": ("Божественная кара", "бонусное действие после попадания · бесплатное применение или ячейка", "enemy"),
    "lay on hands": ("Наложение рук", "бонусное действие · расходует запас исцеления", "self"),
    "sacred weapon": ("Священное оружие", "ближняя атака с расходом заряда Божественного канала", "enemy"),
    "uncanny dodge": ("Невероятное уклонение", "реакция · уменьшает входящий урон вдвое", "enemy"),
    "steady aim": ("Верный прицел", "бонусное действие · преимущество на следующую атаку, скорость становится нулевой", "self"),
    "cunning dash": ("Хитрый Рывок", "бонусное действие · добавляет перемещение", "self"),
    "cunning disengage": ("Хитрый Отход", "бонусное действие · выход из зоны досягаемости не провоцирует атак", "self"),
    "cunning action: hide": ("Хитрое действие — спрятаться", "бонусное действие · проверка Скрытности СЛ 15 за укрытием", "self"),
    "open hand technique: addle": ("Открытая ладонь — лишить реакции", "цель теряет возможность совершать провоцированные атаки", "enemy"),
    "open hand technique: topple": ("Открытая ладонь — сбить с ног", "спасбросок Ловкости цели", "enemy"),
    "sneak attack": ("Скрытая атака", "дополнительный урон после подходящего попадания", "enemy"),
    "sneak attack (full damage)": ("Скрытая атака — полный урон", "", "enemy"),
    "decline": ("Отказаться", "", None),
    "cunning strike: poison": ("Хитрый удар — яд", "часть урона Скрытой атаки · спасбросок Телосложения", "enemy"),
    "cunning strike: trip": ("Хитрый удар — подсечка", "часть урона Скрытой атаки · спасбросок Ловкости", "enemy"),
    "cunning strike: withdraw": ("Хитрый удар — отступить", "часть урона Скрытой атаки · перемещение без провоцирования", "self"),
}
RESOURCES = {
    "barbarian": ("Ярость", None),
    "fighter": ("Второе дыхание", "Всплеск действий"),
    "monk": ("Очки концентрации", None),
    "paladin": ("Запас исцеления", "Божественный канал"),
    "ranger": (None, "Бесплатные Метки охотника"),
    "rogue": (None, "Мешочки с шипами"),
    "bard": ("Бардовское вдохновение", None),
    "cleric": ("Божественный канал", None),
    "druid": ("Дикий облик", None),
    "sorcerer": ("Единицы чародейства", "Врождённое чародейство"),
}

CLASS_KEYS = tuple(CLASS_NAMES)


def class_key(fighter):
    class_id = (fighter or {}).get("class_id")
    return CLASS_KEYS[class_id] if class_id is not None and 0 <= class_id < len(CLASS_KEYS) else None


def name(fighter):
    return CLASS_NAMES.get(class_key(fighter))


def seat_name(seat, human):
    return "Вы" if seat == human else "ИИ"


def fighter_name(fighter, seat, human):
    role = seat_name(seat, human)
    class_name = name(fighter)
    return f"{role} · {class_name}" if class_name else role


def fighter_position(fighter):
    if not fighter:
        return None
    x, y = fighter.get("x"), fighter.get("y")
    if x is None or y is None or min(x, y) < 0:
        return None
    return x, y


ENTITY_NAMES = {
    "cat familiar": ("Кошка-фамильяр", "ф"),
    "owl familiar": ("Сова-фамильяр", "ф"),
    "spider familiar": ("Паук-фамильяр", "ф"),
    "celestial steed": ("Небесный скакун", "с"),
    "fey steed": ("Скакун фей", "с"),
    "fiend steed": ("Исчадийный скакун", "с"),
    "celestial presence": ("Обнаружен небожитель", "?"),
    "fiend presence": ("Обнаружено исчадие", "?"),
    "spiritual weapon": ("Духовное оружие", "о"),
    "moonbeam": ("Лунный луч", "л"),
}


def entity_display(entity):
    label = entity.get("name", str(entity.get("kind", "Существо")))
    return ENTITY_NAMES.get(label.casefold(), (label, "◦"))


def object_name(obj):
    canonical = obj.get("name") or "Предмет"
    return "Рассыпанные шипы" if canonical.casefold() == "deployed caltrops" else EQUIPMENT.get(canonical.casefold(), canonical)


def state_screen(state, human):
    fighters = state["fighters"]
    entities = state.get("entities", ())
    objects = state.get("objects", ())
    owner = state["owner"]
    turn_name = fighter_name(fighters[owner], owner, human)
    if companion := next((entity for entity in entities if entity.get("active")), None):
        turn_name = f"{seat_name(companion['side'], human)} · {entity_display(companion)[0]}"
    lines = [f"\n═══ Ход {state['turns']} · {turn_name} ═══"]
    positions = [fighter_position(fighter) for fighter in fighters]
    target_rows, target_cells = [], []
    if state["actor"] == human and state["phase"] in (5, 51, 56, 75, 82):
        target_rows = [row for row in state["actions"] if row < len(state["terrain"])]
    elif state["actor"] == human and state["phase"] in (6, 52, 57, 76, 83) and state.get("target_row") is not None:
        row = state["target_row"]
        target_rows = [row]
        target_cells = [(column, row) for column in state["actions"] if column < len(state["terrain"][row])]
    elif state["actor"] == human and state["phase"] == 19 and state.get("target_cell"):
        target_cells = [tuple(state["target_cell"])]
    lines.append(render_map(state["terrain"], positions, human, state.get("caltrops", ()),
                            target_rows, target_cells, entities, objects))
    for position in state.get("caltrops", ()):
        if position is not None and tuple(position) in positions:
            lines.append(f"     ^ Шипы под бойцом на {coordinate(*position)}")
    for seat in (human, 1 - human):
        fighter = fighters[seat]
        prefix = f"  {fighter_name(fighter, seat, human)}"
        if positions[seat] is None:
            lines.append(f"{prefix} · скрыт")
            continue
        prefix += f" {fighter['level']} ур."
        loc = coordinate(*positions[seat])
        if fighter.get("altitude"):
            loc += f" · высота {5 * fighter['altitude']} фт"
        details = [prefix, loc]
        if fighter.get("visible") is False:
            details.append("вне прямой видимости")
        if (fighter.get("footprint") or 0) > 1:
            details.append(f"место {fighter['footprint']} × {fighter['footprint']} клетки")
        if fighter.get("squeezing"):
            details.append("протискивается")
        if fighter.get("hp") is not None:
            hp = f"{fighter['hp']}/{fighter['max_hp']} хитов"
            if fighter.get("temp"):
                hp += f" + {fighter['temp']} врем."
            details.append(hp)
        if fighter.get("ac") is not None:
            details.append(f"КД {fighter['ac']}")
        if fighter.get("reaction") is not None:
            details.append(f"реакция {YES_NO[bool(fighter['reaction'])]}")
        lines.append(" · ".join(details))
        equipment = fighter_equipment(fighter)
        if equipment:
            lines.append("    " + " / ".join(equipment))
        resources = []
        if fighter.get("arrows") is not None and fighter.get("ranged_die"):
            resources.append(f"Стрелы {fighter['arrows']}")
        for field, label in zip(("resource", "secondary"), RESOURCES.get(class_key(fighter), (None, None))):
            if label and fighter.get(field) is not None:
                resources.append(f"{label} {fighter[field]}")
        if fighter.get("slots") is not None and (any(fighter["slots"]) or class_key(fighter) in SPELLCASTERS):
            slots = ", ".join(f"{i + 1} ур. — {count}" for i, count in enumerate(fighter["slots"]))
            resources.append(f"Ячейки ({slots})")
        if resources:
            lines.append("    " + " · ".join(resources))
        conditions = [label for key, label in CONDITIONS.items() if fighter.get(key)]
        if fighter.get("frost_slow_feet"):
            conditions.append(f"Ледяной луч · скорость −{fighter['frost_slow_feet']} фт")
        conditions.extend(special_status(fighter))
        if conditions:
            lines.append("    " + ", ".join(conditions))
    for entity in entities:
        if position := fighter_position(entity):
            label = entity_display(entity)[0]
            side = entity.get("side")
            owner = f" · {seat_name(side, human)}" if side in (0, 1) else ""
            details = [f"{label}{owner}", coordinate(*position)]
            if entity.get("altitude"):
                details.append(f"высота {5 * entity['altitude']} фт")
            if (entity.get("footprint") or 0) > 1:
                details.append(f"место {entity['footprint']} × {entity['footprint']} клетки")
            if entity.get("squeezing"):
                details.append("протискивается")
            if entity.get("hp") is not None:
                hp = f"{entity['hp']}/{entity['max_hp']} хитов"
                if entity.get("temp_hp"):
                    hp += f" + {entity['temp_hp']} врем."
                details.append(hp)
            if entity.get("mounted"):
                details.append("с всадником")
            if entity.get("starry_wisp"):
                details.append("свечение Звёздного огонька")
            if entity.get("guiding_bolt"):
                details.append("атаки с преимуществом от Направляющего снаряда")
            if entity.get("exhaustion"):
                details.append(f"истощение {entity['exhaustion']}")
            if entity.get("active"):
                details.append("ходит")
            if entity.get("movement_left") is not None:
                details.append(f"перемещение {5 * entity['movement_left']} фт")
            lines.append("  " + " · ".join(details))
    for obj in objects:
        if position := fighter_position(obj):
            details = [f"Предмет · {object_name(obj)}", coordinate(*position)]
            if obj.get("altitude"):
                details.append(f"высота {5 * obj['altitude']} фт")
            if obj.get("hp") is not None:
                details.append(f"{obj['hp']} хитов · КД {obj['ac']}")
            if obj.get("burning"):
                details.append("горит")
            elif obj.get("flammable"):
                details.append("может загореться")
            lines.append("  " + " · ".join(details))
    for field in state.get("spell_fields", ()):
        parts = [f"Видимый эффект · {coordinate(field['x'], field['y'])}",
                 f"высота точки {5 * field['altitude']} фт"]
        if field["extent_feet"]:
            parts.append(f"от центра {field['extent_feet']} фт")
        if field["height_feet"]:
            parts.append(f"высота области {field['height_feet']} фт")
        lines.append("  " + " · ".join(parts))
    if identified := state.get("identified_spells"):
        lines.append(f"  Опознано при изучении · {identified}")
    if all(position is not None for position in positions):
        heights = [fighter.get("altitude") or 0 for fighter in fighters]
        distance = max(abs(heights[0] - heights[1]), *(abs(a - b) for a, b in zip(*positions)))
        lines.append(f"  До соперника {5 * distance} фт")
    if state["winner"] < 0 and state["owner"] == human:
        budgets = []
        for field, label in (("action_available", "действие"), ("bonus_available", "бонусное действие")):
            if state.get(field) is not None:
                budgets.append(f"{label} {YES_NO[bool(state[field])]}")
        if state.get("movement_left") is not None:
            budgets.append(f"перемещение {5 * state['movement_left']} фт")
        if state.get("attacks_left"):
            budgets.append(f"атак в серии {state['attacks_left']}")
        if state.get("disengaged"):
            budgets.append("действует Отход")
        if budgets:
            lines.append("  В текущем ходе · " + " · ".join(budgets))
    prompt = phase_prompt(state, human)
    if prompt:
        lines.append(f"  {prompt} · {seat_name(state['actor'], human)}")
        if pending := pending_description(state):
            lines.append(f"  {pending}")
    return "\n".join(lines)


EQUIPMENT = {
    "greataxe": "секира", "dagger": "кинжал", "shortbow": "короткий лук",
    "mace": "булава", "shield": "щит", "scale mail": "чешуйчатый доспех",
    "quarterstaff": "боевой посох", "leather": "кожаный доспех",
    "longsword": "длинный меч", "chain mail": "кольчуга",
    "unarmed strikes": "безоружные удары", "shortsword": "короткий меч",
    "longbow": "длинный лук", "studded leather": "проклёпанный кожаный доспех",
    "rapier": "рапира",
}
PHASES = {
    1: "Выход из досягаемости", 2: "Защитная реакция", 3: "Эффект после попадания",
    5: "Выбор ряда цели", 6: "Выбор клетки цели",
    7: "Условие подготовки", 8: "Подготовить действие", 9: "Подготовленная реакция",
    10: "Подготовленное перемещение", 11: "Тактический разум", 12: "Спасбросок от приёма",
    14: "Выбор существа", 19: "Высота цели",
    24: "Цель Наложения рук", 25: "Исцеление Наложением рук",
    34: "Направление заклинания", 35: "Тип урона", 36: "Форма Превращения",
    37: "Слово Приказа", 38: "Ранг ячейки", 39: "Восстановление", 40: "Кость Острого словца",
    50: "Форма спутника", 51: "Ряд появления спутника", 52: "Клетка появления спутника",
    53: "Ход спутника", 54: "Клетка спешивания", 55: "Цель способности скакуна",
    56: "Ряд цели способности", 57: "Клетка цели способности", 58: "Цель удара",
    59: "Цель Помощи", 60: "Расход на призыв",
    73: "Перемещение под действием заклинания", 74: "Содержание Внушения",
    75: "Ряд назначения Внушения", 76: "Клетка назначения Внушения",
    77: "Предмет для Внушения", 80: "Цель снаряда", 81: "Защита от Волшебной стрелы",
    117: "Высота назначения Внушения", 121: "Выбор места прыжка",
    82: "Ряд предполагаемой цели", 83: "Клетка предполагаемой цели",
    84: "Высота предполагаемой цели", 85: "Защита от урона снаряда",
}


def phase_prompt(state, human):
    phase = state["phase"]
    if state["actor"] != human or phase in (0, 4):
        return None
    return PHASES.get(phase, "Выбор параметра действия")


SPELLCASTERS = {"bard", "cleric", "druid", "paladin", "ranger", "sorcerer", "warlock", "wizard"}
CONDITIONS = {
    "raging": "Ярость", "dodge": "Уклонение",
    "sap": "Помеха на следующую атаку", "slow": "Замедление",
    "stunned": "Ошеломление", "stun_slow": "Скорость снижена вдвое",
    "stun_exposed": "Атаки по бойцу с преимуществом", "reckless": "Безрассудная атака",
    "advantage": "Преимущество на атаку", "disadvantage": "Помеха на атаку",
    "speed_zero": "Скорость 0", "shield": "Магический щит",
    "prone": "Сбит с ног", "hidden": "Скрыт",
    "poisoned": "Отравлен", "grappled": "Захвачен", "restrained": "Опутан",
    "frightened": "Испуган", "invisible": "Невидим",
    "starry_wisp": "Свечение Звёздного огонька",
    "guiding_bolt": "Преимущество от Направляющего снаряда",
}


def target_request(state):
    """The pending ability and, once chosen, its zero-based target row."""
    text = state.get("pending", "")
    prompt = re.fullmatch(r"Choose a target (?:row|column in row \d+) for (.+)", text)
    label = prompt[1] if prompt else text
    return label, state["target_row"]

def pending_description(state):
    if state["phase"] in (5, 6):
        canonical, row = target_request(state)
        fighter = state["fighters"][state["actor"]]
        label = describe_magic_action(canonical, fighter)[0]
        return label if row is None else f"{label} · ряд {row + 1}"
    text = state.get("pending", "")
    if elevation := re.fullmatch(r"Choose a height at ([A-I][1-7]) · (.+)", text):
        label = SPELLS.get(elevation[2].casefold(), (elevation[2],))[0]
        return f"{label} · клетка {elevation[1]}"
    if compelled := re.fullmatch(r"(.+): (?:([^·]+) · )?compelled movement, (\d+) ft remaining", text):
        spell = {"Command": "Приказ", "Suggestion": "Внушение",
                 "Dissonant Whispers": "Диссонирующий шёпот"}.get(compelled[1], compelled[1])
        entity = compelled[2]
        role = f"{entity_display({'name': entity})[0]} · " if entity else ""
        return f"{spell} · {role}осталось {compelled[3]} фт перемещения"
    if state["phase"] in (74, 75, 76, 77):
        return ""
    if projectile := re.fullmatch(r"(.+): select a target \((\d+) remaining\)", text):
        spell = SPELLS.get(projectile[1].casefold(), (projectile[1],))[0]
        return f"{spell} · осталось снарядов {projectile[2]}"
    if attack := re.fullmatch(r"(.+): select the attack target", text):
        return f"{SPELLS.get(attack[1].casefold(), (attack[1],))[0]} · выберите цель атаки"
    if darts := re.fullmatch(r"Magic Missile: (\d+) incoming darts\. Choose a Shield slot or pass", text):
        return f"Волшебная стрела · летит снарядов {darts[1]} · выберите ячейку Щита или пропустите реакцию"
    if damage := re.fullmatch(r"Magic Missile: dart damage (\d+)\. Use Cutting Words or pass", text):
        return f"Волшебная стрела · урон снаряда {damage[1]} · Острое словцо или пропуск реакции"
    if text == "Select a guessed projectile location":
        return "Укажите предполагаемое положение цели."
    if height := re.fullmatch(r"Select height of guessed cell ([A-I][1-7])", text):
        return f"Предполагаемая цель · клетка {height[1]} · выберите высоту"
    move = re.fullmatch(r"Opportunity attack before movement to \((\d+), (\d+)\)", text)
    if move:
        return f"Провоцированная атака перед шагом в {coordinate(*map(int, move.groups()))}"
    for source, replacement in (
        ("Critical hit", "Критическое попадание"), ("Hit", "Попадание"),
        ("Magic Missile incoming", "В цель летит Волшебная стрела"),
        ("total", "итог"), ("AC", "КД"), (" for ", " · "),
        ("disadvantage", "помеха"), ("advantage", "преимущество"),
        (" damage before additional effects", " урона до дополнительных эффектов"),
    ):
        text = text.replace(source, replacement)
    return text


SPELLS = {
    # Canonical label: localized label, minimum slot level, economy, target, effect.
    "vicious mockery": ("Злая насмешка", 0, "action", "enemy", "спасбросок Мудрости, помеха на атаку при провале"),
    "starry wisp": ("Звёздный огонёк", 0, "action", "enemy", "дальнобойная атака заклинанием"),
    "sacred flame": ("Священное пламя", 0, "action", "enemy", "спасбросок Ловкости"),
    "fire bolt": ("Огненный снаряд", 0, "action", "enemy", "дальнобойная атака заклинанием"),
    "ray of frost": ("Луч холода", 0, "action", "enemy", "дальнобойная атака заклинанием, замедляет при попадании"),
    "shocking grasp": ("Электрошок", 0, "action", None, "цель в досягаемости касания · при попадании цель не может совершать провоцированные атаки до своего следующего хода"),
    "poison spray": ("Ядовитые брызги", 0, "action", None, "дальнобойная атака заклинанием по существу в пределах 30 фт"),
    "true strike": ("Верный удар", 0, "action", None, "выберите оружие, тип урона и цель · атака использует заклинательную характеристику"),
    "eldritch blast": ("Мистический заряд", 0, "action", "enemy", "дальнобойная атака заклинанием"),
    "shillelagh": ("Дубинка", 0, "bonus", "self", "усиливает атаки посохом"),
    "command": ("Приказ", 1, "action", None, "выберите слово и цель · спасбросок Мудрости"),
    "suggestion": ("Внушение", 2, "action", None, "концентрация · выберите цель и выполнимое поручение"),
    "dissonant whispers": ("Диссонирующий шёпот", 1, "action", "enemy", "спасбросок Мудрости, при провале цель может отступить"),
    "healing word": ("Лечащее слово", 1, "bonus", "self", "восстанавливает хиты"),
    "cure wounds": ("Лечение ран", 1, "action", "self", "восстанавливает хиты"),
    "guiding bolt": ("Направляющий снаряд", 1, "action", "enemy", "дальнобойная атака заклинанием, следующая атака по цели с преимуществом при попадании"),
    "bless": ("Благословение", 1, "action", "self", "концентрация, +1к4 к атакам и спасброскам"),
    "thunderwave": ("Громовая волна", 1, "action", "enemy", "спасбросок Телосложения, отталкивает при провале"),
    "burning hands": ("Огненные ладони", 1, "action", "enemy", "спасбросок Ловкости"),
    "chromatic orb": ("Цветной шар", 1, "action", "enemy", "дальнобойная атака заклинанием"),
    "magic missile": ("Волшебная стрела", 1, "action", "enemy", "попадает без броска атаки, блокируется заклинанием Щит"),
    "shield": ("Щит", 1, "reaction", "self", "до начала следующего хода +5 к КД и защита от Волшебной стрелы"),
    "hex": ("Порча", 1, "bonus", "enemy", "концентрация, дополнительный урон при попаданиях"),
    "divine favor": ("Божественное благоволение", 1, "bonus", "self", "добавляет урон атакам оружием"),
    "hunter's mark": ("Метка охотника", 1, "bonus", "enemy", "концентрация, дополнительный урон при попаданиях"),
    "shatter": ("Дребезги", 2, "action", "ground", "радиус 10 фт, спасбросок Телосложения, может задеть заклинателя"),
    "spiritual weapon": ("Духовное оружие", 2, "bonus", "enemy", "концентрация, создаёт оружие рядом с целью и атакует"),
    "moonbeam": ("Лунный луч", 2, "action", "ground", "концентрация, радиус 5 фт, может задеть заклинателя"),
    "scorching ray": ("Палящий луч", 2, "action", "enemy", "несколько отдельных дальнобойных атак"),
    "hypnotic pattern": ("Гипнотический узор", 3, "action", "ground", "концентрация, радиус 15 фт, обездвиживает и лишает действий при провале спасброска Мудрости"),
    "spirit guardians": ("Духовные стражи", 3, "action", "self", "концентрация, область урона вокруг заклинателя"),
    "fireball": ("Огненный шар", 3, "action", "ground", "радиус 20 фт, спасбросок Ловкости, может задеть заклинателя"),
}
NAMED_ACTIONS = {
    "fast hands: caltrops": ("Ловкие руки — шипы", "бонусное действие и мешочек шипов · при входе в клетку спасбросок Ловкости СЛ 15, при провале урон и скорость 0 до следующего хода", "ground"),
    "cutting words": ("Острое словцо", "реакция и заряд вдохновения · уменьшает бросок атаки врага", "enemy"),
    "font of inspiration": ("Источник вдохновения", "ячейка в обмен на заряд вдохновения", "self"),
    "divine spark": ("Божественная искра", "действие и заряд Божественного канала · спасбросок Телосложения", "enemy"),
    "preserve life": ("Сохранение жизни", "действие и заряд Божественного канала · исцеление", "self"),
    "strike with spiritual weapon": ("Удар духовным оружием", "бонусное действие · передвигает оружие к цели и атакует", "enemy"),
    "wild shape (wolf)": ("Дикий облик — волк", "бонусное действие и заряд Дикого облика", "self"),
    "leave wild shape": ("Выйти из Дикого облика", "бонусное действие", "self"),
    "land's aid": ("Помощь земли", "действие и заряд Дикого облика · радиус 10 фт, наносит урон сопернику в области и исцеляет друида", "ground"),
    "move moonbeam": ("Переместить Лунный луч", "действие · переносит область на выбранную клетку", "ground"),
    "wild resurgence": ("Дикое возрождение", "ячейка в обмен на заряд Дикого облика", "self"),
    "innate sorcery": ("Врождённое чародейство", "бонусное действие и заряд · усиливает заклинания", "self"),
    "heightened fireball": ("Усиленный Огненный шар", "действие, ячейка 3 ур. и 2 единицы чародейства · радиус 20 фт, соперник спасается с помехой", "ground"),
    "quickened fire bolt": ("Ускоренный Огненный снаряд", "бонусное действие и 2 единицы чародейства", "enemy"),
    "create level 1 spell slot": ("Создать ячейку 1 ур.", "бонусное действие и 2 единицы чародейства", "self"),
    "convert slot to sorcery points": ("Преобразовать ячейку", "ячейка в обмен на единицы чародейства", "self"),
    "shield": ("Щит", "реакция и ячейка от 1 ур. · +5 к КД, блокирует Волшебную стрелу", "self"),
    "fiendish vigor": ("Дьявольская стойкость", "действие · даёт временные хиты", "self"),
    "select higher spell slot": ("Выбрать ранг ячейки", "ограничивает ранг ячейки для следующего заклинания", None),
    "wild companion (wild shape)": ("Призвать дикого спутника", "действие и применение Дикого облика", "ground"),
    "wild companion (spell slot)": ("Призвать дикого спутника из ячейки", "действие и выбранная ячейка", "ground"),
    "find steed (faithful steed)": ("Призвать верного скакуна", "действие и бесплатное применение", "ground"),
    "find steed (spell slot)": ("Призвать скакуна из ячейки", "действие и выбранная ячейка", "ground"),
    "mount steed": ("Сесть в седло", "расходует половину скорости", None),
    "dismount": ("Спешиться", "расходует половину скорости", None),
    "end companion turn": ("Закончить ход спутника", "", None),
    "companion dash": ("Рывок спутника", "добавляет перемещение спутника", None),
    "companion disengage": ("Отход спутника", "предотвращает провоцированные атаки от перемещения спутника", None),
    "companion dodge": ("Уклонение спутника", "помеха на атаки видимых врагов по спутнику", None),
    "companion help": ("Помощь спутника", "выбрать цель", None),
    "companion ascend": ("Поднять спутника на 5 фт", "", None),
    "companion descend": ("Опустить спутника на 5 фт", "", None),
    "suggest moving to a safe cell": ("Внушить перейти на безопасную клетку", "выбрать место", None),
    "suggest setting held items aside": ("Внушить отложить предметы из рук", "", None),
    "suggest taking the dodge action": ("Внушить Уклонение", "", None),
    "suggest stowing the melee weapon": ("Внушить убрать оружие ближнего боя", "", None),
    "suggest stowing the bow": ("Внушить убрать лук", "", None),
    "suggest picking up a weapon from the ground": ("Внушить поднять оружие", "выбрать доступный предмет", None),
    "suggest dropping prone": ("Внушить лечь", "", None),
    "suggest standing up": ("Внушить встать", "", None),
    "suggest refraining from the attack action this turn": ("Внушить воздержаться от действия Атака в этом ходе", "", None),
    "suggest ending the turn": ("Внушить закончить ход", "", None),
    "cancel suggestion": ("Отменить Внушение", "ячейка сохраняется", None),
    "finish compelled movement": ("Завершить перемещение", "", None),
    "compelled dash (action)": ("Рывок по Приказу", "действие", None),
    "compelled dash (bonus action)": ("Рывок по Приказу", "бонусное действие", None),
    "aim one projectile at the opposing adventurer": ("Один снаряд в соперника", "", None),
    "aim one projectile at yourself": ("Один снаряд в себя", "", None),
    "attack yourself": ("Атаковать себя", "", None),
    "attack the opposing adventurer": ("Атаковать соперника", "", None),
    "aim at a guessed location": ("Указать предполагаемую клетку цели", "", None),
    "strike with melee weapon": ("Ударить оружием ближнего боя", "", None),
    "strike with ranged weapon": ("Выстрелить из дальнобойного оружия", "", None),
    "throw held weapon": ("Метнуть оружие из руки", "", None),
    "compelled long jump": ("Прыгнуть в длину", "выбрать клетку приземления", None),
    "compelled high jump": ("Прыгнуть в высоту", "выбрать высоту", None),
    "weapon's normal damage": ("Обычный тип урона оружия", "", None),
    "radiant damage": ("Урон излучением", "", None),
    "aim all remaining darts at the opposing adventurer": ("Все оставшиеся снаряды в соперника", "", None),
    "aim all remaining darts at yourself": ("Все оставшиеся снаряды в себя", "", None),
    "let the darts strike": ("Пропустить защиту", "снаряды попадут в цель", None),
    "approach": ("Приблизиться", "цель подходит по кратчайшему пути", None),
    "drop": ("Бросить", "цель выпускает предметы из рук и заканчивает ход", None),
    "flee": ("Бежать", "цель удаляется всеми доступными средствами", None),
    "grovel": ("Лечь", "цель падает ничком и заканчивает ход", None),
    "halt": ("Замереть", "цель пропускает перемещение и действия", None),
}


def fighter_equipment(fighter):
    if "Wild Shape (Wolf)" in (fighter.get("summary") or ""):
        return ("волчий укус",)
    equipment = fighter.get("equipment", "")
    return tuple(EQUIPMENT.get(part.casefold(), part) for part in equipment.split(" / ")) if equipment else ()


def special_status(fighter):
    summary = fighter.get("summary") or ""
    statuses = []
    concentration = re.search(r"Concentrating on ([A-Za-z' ]+?)(?: at \((\d+), (\d+)\)|,|$)", summary)
    if concentration:
        canonical, x, y = concentration.groups()
        label = SPELLS.get(canonical.casefold(), (canonical,))[0]
        location = f" ({coordinate(int(x), int(y))})" if x is not None else ""
        statuses.append(f"Концентрация · {label}{location}")
    for source, label in (
        ("Innate Sorcery", "Врождённое чародейство активно"),
        ("Divine Favor", "Божественное благоволение"),
        ("Shillelagh", "Дубинка"),
        ("Wild Shape (Wolf)", "Дикий облик"),
        ("Hypnotized", "Гипнотизирован"),
        ("Guiding Bolt advantage", "Преимущество от Направляющего снаряда"),
        ("Free Smite 1", "Бесплатная Божественная кара"),
        ("Sacred Weapon active", "Священное оружие"),
        ("Vex ready", "Свойство «Изнурение» готово"),
    ):
        if source in summary:
            statuses.append(label)
    if shift := re.search(r"Tactical Shift (\d+) ft", summary):
        statuses.append(f"Тактическое перемещение {shift[1]} фт")
    if slot := re.search(r"Selected spell slot level (\d+)", summary):
        statuses.append(f"Выбрана ячейка от {slot[1]} ур.")
    return statuses


def describe_magic_action(canonical, actor):
    key = canonical.casefold()
    if key in SPELLS:
        label, tier, economy, target, effect = SPELLS[key]
        cost = {"bonus": "бонусное действие", "reaction": "реакция"}.get(economy, "действие")
        if key == "hunter's mark" and actor.get("secondary", 0):
            cost += " и бесплатное применение Метки"
        elif tier:
            cost += f" и ячейка от {tier} ур."
        else:
            cost += " (заговор)"
        return label, f"{cost} · {effect}", target
    return NAMED_ACTIONS.get(key, (ROSTER_ACTIONS.get(canonical, canonical), "", None))


def describe_action(state, action, human):
    seat = state["actor"]
    actor = state["fighters"][seat]
    other = state["fighters"][1 - seat]
    if state["phase"] in (5, 6):
        canonical, row = target_request(state)
        label = describe_magic_action(canonical, actor)[0]
        coverage = state.get("target_effects", {}).get(action)
        caltrops = canonical.casefold() == "fast hands: caltrops"
        if row is None:
            line = f"Ряд {action + 1} · {label}"
            if coverage is not None and not caltrops:
                if coverage["foe_only"]:
                    line += " · есть клетки, накрывающие только соперника"
                elif coverage["both"]:
                    line += " · при попадании в соперника область накроет обоих"
                else:
                    line += " · соперник вне области"
            return line
        line = f"{label} → клетка {coordinate(action, row)}"
        if coverage is not None and not caltrops:
            if coverage["both"]:
                line += " · область накрывает обоих"
            elif coverage["foe"]:
                line += " · область накрывает соперника"
            elif coverage["self"]:
                line += " · область накрывает заклинателя"
            else:
                line += " · бойцы вне области"
        return line
    canonical = state["actions"][action]
    if state["phase"] == 73 and (step := re.fullmatch(r"Compelled move ([a-z-]+) to ([A-I][1-7])", canonical)):
        return f"Шаг на {MOVE_LABELS[step[1]][0]} {step[2]}"
    if state["phase"] == 73 and (height := re.fullmatch(r"Compelled move to elevation (\d+) ft", canonical)):
        return f"Переместиться на высоту {height[1]} фт"
    if state["phase"] == 121 and (jump := re.fullmatch(r"Compelled jump to ([A-I][1-7]) at (\d+) ft \((\d+) ft movement(; Jump spell)?\)", canonical)):
        source = " · заклинание Прыжок" if jump[4] else ""
        return f"Прыжок в {jump[1]} · высота {jump[2]} фт · расход {jump[3]} фт{source}"
    if state["phase"] == 19:
        cell = state.get("target_cell")
        return f"{coordinate(*cell)} · высота {5 * action} фт" if cell else f"Высота {5 * action} фт"
    if state["phase"] in (51, 56, 75, 82) and action < len(state["terrain"]):
        return f"Ряд {action + 1}"
    if state["phase"] in (52, 57, 76, 83) and state.get("target_row") is not None and action < len(state["terrain"][0]):
        return f"Клетка {coordinate(action, state['target_row'])}"
    if state["phase"] in (84, 117) and canonical != "Cancel Suggestion":
        return f"Высота {5 * action} фт"
    if state["phase"] == 80 and 2 <= action and canonical.startswith("Attack "):
        target_label = canonical.removeprefix("Attack ")
        if action >= 4:
            return f"Атаковать предмет · {object_name({'name': target_label})}"
        return f"Атаковать существо · {entity_display({'name': target_label})[0]}"
    if state["phase"] == 77 and canonical.startswith("Pick up "):
        label = canonical.removeprefix("Pick up ")
        return f"Поднять {object_name({'name': label})}"
    if state["phase"] == 50 and canonical.casefold() in ENTITY_NAMES:
        return ENTITY_NAMES[canonical.casefold()][0]
    if slot := re.fullmatch(r"(Use|Select) level (\d+) spell slot", canonical):
        verb = "Потратить" if slot[1] == "Use" else "Выбрать"
        return f"{verb} ячейку {slot[2]} ур."
    if cast := re.fullmatch(r"Cast (.+?) with level (\d+) slot", canonical):
        spell = SPELLS.get(cast[1].casefold(), (cast[1],))[0]
        return f"{spell} · ячейка {cast[2]} ур."
    if shield := re.fullmatch(r"Cast Shield using a level (\d+) slot", canonical):
        return f"Щит · реакция и ячейка {shield[1]} ур."
    if restore := re.fullmatch(r"Restore (\d+) HP", canonical):
        return f"Восстановить {restore[1]} хитов"
    if push := re.fullmatch(r"Open Hand Technique: Push (\d+) ft", canonical):
        return f"Открытая ладонь — оттолкнуть на {push[1]} фт · спасбросок Силы цели"
    movement = MOVE_LABELS.get(canonical.casefold())
    if movement is not None and state["phase"] == 0:
        direction, dx, dy = movement
        x, y = actor["x"] + dx, actor["y"] + dy
        return f"Шаг на {direction} {coordinate(x, y)}"
    if movement is not None and state["phase"] == 34:
        return f"Направить на {movement[0]}"
    key = canonical.casefold()
    special = MARTIAL_ACTIONS.get(key)
    if special:
        label, cost, target = special
        if key == "second wind" and actor["level"] >= 5:
            cost += " и даёт тактическое перемещение без провоцирования атак"
    elif key in COMMON_ACTIONS:
        label, cost = COMMON_ACTIONS[key]
        target = "enemy" if key == "opportunity attack" else None
        if key in ("opportunity attack", "melee attack", "ranged attack"):
            equipment = fighter_equipment(actor)
            index = int(key == "ranged attack")
            if index < len(equipment):
                label += f" ({equipment[index]})"
            if key == "ranged attack" and "arrows" in actor:
                cost += " · расходует стрелу"
    else:
        label, cost, target = describe_magic_action(canonical, actor)
    if target == "enemy":
        label += f" → {fighter_name(other, 1 - seat, human)}"
        if position := fighter_position(other):
            label += f" ({coordinate(*position)})"
    elif target == "self":
        label += f" → себя ({coordinate(actor['x'], actor['y'])})"
    elif target == "ground":
        label += " → выбрать клетку"
    return f"{label} · {cost}" if cost else label


def state_delta(before, after, human):
    changes = []
    for seat, (old, new) in enumerate(zip(before["fighters"], after["fighters"])):
        prefix = fighter_name(new, seat, human)
        deltas = []
        old_position, new_position = fighter_position(old), fighter_position(new)
        if new_position is None:
            if old_position is not None:
                changes.append(f"  {prefix} · скрылся из виду")
            continue
        if old_position is None:
            changes.append(f"  {prefix} · обнаружен на {coordinate(*new_position)}")
            continue
        if old_position != new_position:
            deltas.append(f"{coordinate(*old_position)} → {coordinate(*new_position)}")
        for key, label in (("hp", "хиты"), ("temp", "временные хиты"), ("ac", "КД"), ("arrows", "стрелы"),
                           ("altitude", "высота")):
            previous, current = old.get(key), new.get(key)
            if previous is None or current is None or previous == current:
                continue
            if key == "altitude":
                deltas.append(f"{label} {5 * previous} → {5 * current} фт")
            elif key == "hp":
                deltas.append(f"{label} {previous} → {current} ({current - previous:+d})")
            else:
                deltas.append(f"{label} {previous} → {current}")
        if old.get("reaction") and new.get("reaction") is not None and not new["reaction"]:
            deltas.append("реакция потрачена")
        for i, (old_slots, new_slots) in enumerate(zip(old.get("slots") or (), new.get("slots") or ()), 1):
            if old_slots != new_slots:
                deltas.append(f"ячейки {i} ур. {old_slots} → {new_slots}")
        for field, label in zip(("resource", "secondary"), RESOURCES.get(class_key(new), ("Ресурс", "Доп. ресурс"))):
            previous, current = old.get(field), new.get(field)
            if label and previous is not None and current is not None and previous != current:
                deltas.append(f"{label} {previous} → {current}")
        for key, label in CONDITIONS.items():
            if old.get(key) is not None and new.get(key) is not None and bool(old[key]) != bool(new[key]):
                deltas.append(f"{label} {'+' if new.get(key) else '−'}")
        if old.get("summary") is not None and new.get("summary") is not None:
            old_status, new_status = special_status(old), special_status(new)
            deltas.extend(f"{item} +" for item in new_status if item not in old_status)
            deltas.extend(f"{item} −" for item in old_status if item not in new_status)
        if deltas:
            changes.append(f"  {prefix} · " + " · ".join(deltas))
    viewed_before = {entity["id"]: entity for entity in before.get("entities", ())}
    viewed_after = {entity["id"]: entity for entity in after.get("entities", ())}
    for identifier, entity in viewed_after.items():
        previous = viewed_before.get(identifier, {})
        label = entity_display(entity)[0]
        position = fighter_position(entity)
        old_position = fighter_position(previous)
        if position is not None and old_position is None:
            changes.append(f"  {label} · появился на {coordinate(*position)}")
        elif position is not None and old_position != position:
            changes.append(f"  {label} · {coordinate(*old_position)} → {coordinate(*position)}")
        old_hp, hp = previous.get("hp"), entity.get("hp")
        if old_hp is not None and hp is not None and old_hp != hp:
            changes.append(f"  {label} · хиты {old_hp} → {hp}")
        old_temp, temp = previous.get("temp_hp"), entity.get("temp_hp")
        if old_temp is not None and temp is not None and old_temp != temp:
            changes.append(f"  {label} · временные хиты {old_temp} → {temp}")
        old_height, height = previous.get("altitude"), entity.get("altitude")
        if old_height is not None and height is not None and old_height != height:
            changes.append(f"  {label} · высота {5 * old_height} → {5 * height} фт")
    for identifier, old in viewed_before.items():
        if identifier not in viewed_after:
            changes.append(f"  {entity_display(old)[0]} · исчез из виду")
    objects_before = {obj["id"]: obj for obj in before.get("objects", ())}
    objects_after = {obj["id"]: obj for obj in after.get("objects", ())}
    for identifier, obj in objects_after.items():
        label = object_name(obj)
        location = fighter_position(obj)
        old = objects_before.get(identifier)
        if old is None:
            if location is not None:
                changes.append(f"  Предмет · {label} · замечен на {coordinate(*location)}")
            continue
        previous = fighter_position(old)
        if previous is not None and location is not None and location != previous:
            changes.append(f"  Предмет · {label} · {coordinate(*previous)} → {coordinate(*location)}")
        if old.get("hp") is not None and obj.get("hp") is not None and old["hp"] != obj["hp"]:
            changes.append(f"  Предмет · {label} · хиты {old['hp']} → {obj['hp']}")
        if old.get("burning") is not None and obj.get("burning") is not None and old["burning"] != obj["burning"]:
            changes.append(f"  Предмет · {label} · {'загорелся' if obj['burning'] else 'погас'}")
    for identifier, old in objects_before.items():
        if identifier not in objects_after:
            changes.append(f"  Предмет · {object_name(old)} · исчез из виду")
    return changes


def result_message(state, human):
    winner = state["winner"]
    if winner == -1:
        return "\nБой прерван."
    if winner == 2:
        reasons = {
            "double_ko": "Оба бойца выбыли.",
            "turn_limit": "Достигнут предел в 60 индивидуальных ходов.",
            "decision_limit": "Достигнут предел в 800 решений.",
            "timeout": "Достигнут предел боя.",
        }
        detail = reasons.get(state.get("limit_reason") or state.get("result_reason"))
        return "\n═══ Ничья ═══" + (f"\n{detail}" if detail else "")
    result = "Победа" if winner == human else "Поражение"
    return f"\n═══ {result} ═══\nПобедитель · {fighter_name(state['fighters'][winner], winner, human)}"


def run_duel(batch, policy, human, intro, *, choose=choose_action, rng=None):
    """Drive both seats with one Rust battle, optionally with a scripted human seat."""
    print(intro)
    frames = []
    shown_turn = None
    interrupted = False
    state = batch.state(viewer=human)
    while state["winner"] == -1:
        features, masks, actors, _ = batch.observe()
        actor = int(actors[0])
        turn_key = state["turns"], state["owner"]
        seat_label = fighter_name(state["fighters"][actor], actor, human)
        screen = state_screen(state, human) if actor == human or shown_turn != turn_key else ""
        if screen:
            print(screen)
        shown_turn = turn_key
        legal = np.flatnonzero(masks[0]).tolist()
        if actor == human:
            if set(state["actions"]) != set(legal):
                raise RuntimeError("Список действий зрителя расходится с маской арены")
            descriptions = [(action, describe_action(state, action, human)) for action in legal]
            try:
                action = choose(descriptions)
            except QuitGame:
                interrupted = True
                break
        else:
            action = policy.act(features[0], masks[0], rng)
        if action not in legal:
            raise ValueError(f"Действие {action} отсутствует в маске допустимых действий")
        description = describe_action(state, action, human) if actor == human else None
        batch.step([action])
        current = batch.state(viewer=human)
        changes = state_delta(state, current, human)
        current_pending = pending_description(current) if current["actor"] == human else ""
        previous_pending = pending_description(state) if actor == human else ""
        if (prompt := phase_prompt(current, human)) and (
            current["phase"] != state["phase"] or current_pending != previous_pending
        ):
            phase_desc = f"  {prompt} · {seat_name(current['actor'], human)}"
            changes.append(phase_desc + f" · {current_pending}" if current_pending else phase_desc)
        if actor != human:
            if changes or current["winner"] != -1:
                description = "действие соперника"
            elif current["turns"] != state["turns"]:
                description = "заканчивает ход"
        if description or screen or current["turns"] != state["turns"]:
            message = "\n".join([f"› {seat_label} · {description}", *changes]) if description else ""
            if message:
                print(message)
            frame = {"screen": screen, "message": message, "actor": actor, "before": state, "after": current}
            if actor == human:
                frame.update(action=action, legal=legal, canonical=state["actions"][action])
            frames.append(frame)
        state = current
    screen = state_screen(state, human)
    result = result_message(state, human)
    print(screen)
    print(result)
    return {"frames": frames, "final_screen": screen, "result": result, "winner": state["winner"],
            "interrupted": interrupted}


def main(argv=None):
    args = parse_args(argv)
    try:
        if args.replay:
            replay(args.replay)
            return 0
        from arena.native import ACTIONS, ARENA_ABI, FEATURES, Batch, roster
        from arena.policy import DEFAULT_CHECKPOINT, Policy

        if args.save and args.save.exists():
            raise FileExistsError(f"Файл записи уже существует: {args.save}")
        checkpoint = args.checkpoint or DEFAULT_CHECKPOINT
        if not checkpoint.is_file():
            raise FileNotFoundError(f"Файл весов ИИ не найден: {checkpoint}")
        checkpoint_bytes = checkpoint.read_bytes()
        policy = Policy.load(BytesIO(checkpoint_bytes)).require_arena(ARENA_ABI, FEATURES, ACTIONS)
        if policy.metadata.get("stage") == "migrated_warm_start":
            raise ValueError("Эти веса подготовлены для обучения. Для игры нужны обученные веса этой версии арены.")
        checkpoint_hash = hashlib.sha256(checkpoint_bytes).hexdigest()
        player, opponent, human = choose_setup(args, roster())
        builds = [player["id"], opponent["id"]]
        if human == 1:
            builds.reverse()
        seed = args.seed if args.seed is not None else secrets.randbits(64)
        rng = np.random.default_rng(seed ^ 0xb9ff837a2161dc0d) if args.sample else None
        with Batch(1) as batch:
            batch.reset_builds([seed], [builds])
            intro = (f"Вы · {CLASS_NAMES[player['class_name'].lower()]} · {subclass_name(player)} · 5 ур.   │   "
                     f"ИИ · {CLASS_NAMES[opponent['class_name'].lower()]} · {subclass_name(opponent)} · 5 ур.\n"
                     f"Зерно {seed}   │   Веса {checkpoint.name} [{checkpoint_hash[:12]}]")
            result = run_duel(batch, policy, human, intro, rng=rng)
        if args.save:
            write_replay(args.save, {
                "version": REPLAY_VERSION,
                "arena_abi": ARENA_ABI,
                "seed": seed,
                "checkpoint": {"name": checkpoint.name, "sha256": checkpoint_hash, "arena_abi": ARENA_ABI},
                "selection": {"builds": builds, "levels": [5, 5], "human": human, "sampled": args.sample},
                "intro": intro,
                **result,
            })
        return 0
    except QuitGame:
        print("\nВыход.")
        return 0
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        print(f"Ошибка · {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
