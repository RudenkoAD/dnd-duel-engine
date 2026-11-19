"""Human-facing, fixed-seat table. Bot observations never cross the HTTP boundary."""

from heapq import heappop, heappush
import re
import secrets

from arena.native import Batch, roster
import arena_play as ui


REASONS = {
    "star_map_not_held": "Возьмите в руку звёздную карту.",
    "social disguise": "Меняет внешность для социальных взаимодействий.",
    "no divination or magical scrying target": "В этой дуэли нет подходящей цели магического наблюдения.",
    "no underwater terrain": "На этой карте нет подводных клеток.",
    "no conversation with beasts": "На этой карте нет животных для разговора.",
    "reaction": "Открывается в ответ на подходящее действие соперника.",
    "on_hit": "Открывается в окне после подходящего попадания.",
    "form": "Текущее состояние бойца препятствует колдовству.",
    "components": "Не выполнены условия для компонентов, рук или снаряжения.",
    "bonus_spent": "Бонусное действие уже потрачено.",
    "action_spent": "Основное действие уже потрачено.",
    "slot_spent": "В этом индивидуальном ходе уже потрачена ячейка.",
    "slot": "Нет доступной ячейки подходящего ранга.",
    "conditions": "Сейчас условия применения не выполнены. Проверьте цель, дистанцию и снаряжение.",
}
TIMING = {
    "action": "Действие",
    "utility": "Вне боя",
    "bonus": "Бонусное действие",
    "reaction": "Реакция",
    "on_hit": "При попадании",
}
EXTRA = dict(
    line.split("|")
    for line in """Mage Armor|Доспехи мага
Misty Step|Туманный шаг
Aid|Подмога
Lesser Restoration|Малое восстановление
Mass Healing Word|Массовое лечащее слово
Revivify|Возрождение
Guidance|Указание
Resistance|Сопротивление
Light|Свет
Sanctuary|Убежище
Shield of Faith|Щит веры
Jump|Прыжок
Blur|Размытый образ
Longstrider|Скороход
Entangle|Опутывание
Protection from Energy|Защита от энергии
Dispel Magic|Рассеивание магии
Hold Person|Удержание личности
Counterspell|Контрзаклинание
Magic Weapon|Магическое оружие
Ensnaring Strike|Опутывающий удар
Alter Self|Смена обличья
Dragon's Breath|Дыхание дракона
Fear|Страх
Fly|Полёт
Stinking Cloud|Зловонное облако
Zone of Truth|Область истины
Heroism|Героизм
Bane|Проклятие
Expeditious Retreat|Поспешное отступление
Fiendish Vigor|Дьявольская живучесть
Ready|Подготовить действие
Unarmed|Безоружный удар
Help|Помощь
Study|Изучить
Stonecunning|Чутьё камня
Concentration options|Управление концентрацией
Opportunity weapon attack|Провоцированная атака оружием
Opportunity unarmed strike|Провоцированный безоружный удар
Find Familiar|Поиск фамильяра
Wild Companion (Wild Shape)|Дикий спутник за заряд облика
Wild Companion (spell slot)|Дикий спутник за ячейку
End companion turn|Завершить ход спутника
Companion ascend|Подняться на 5 футов
Companion descend|Спуститься на 5 футов
Companion Dash|Рывок спутника
Companion Dodge|Уклонение спутника
Companion Disengage|Отход спутника
Companion Help|Помощь спутника
Long Jump|Прыжок в длину
High Jump|Прыжок в высоту
Wild Shape|Дикий облик
Land's Aid|Помощь земли
Wild Resurgence|Восстановление дикого облика
Choose ability for Finesse or Martial Arts|Выбрать характеристику атаки
Throw dagger (Attack)|Метнуть кинжал
Use an object|Использовать предмет""".splitlines()
)
TARGET_PHASES = {14, 24, 55, 58, 59, 66, 77, 78, 80, 94, 95, 104}
PHASE_NAMES = {
    13: "Дальность прыжка",
    16: "Высота прыжка",
    17: "Клетка для лазания",
    18: "Смягчение падения",
    48: "Получатели заклинания",
    66: "Цель броска",
    67: "Точка броска",
    68: "Клетка броска",
    69: "Высота броска",
    109: "Точка подготовленного заклинания",
    110: "Клетка цели",
    111: "Высота цели",
}
DIRECTIONS = [d[1:] for d in ui.DIRECTIONS]


def localized(label):
    if label.casefold() in ui.SPELLS:
        return ui.SPELLS[label.casefold()][0]
    if label in ui.ROSTER_ACTIONS:
        return ui.ROSTER_ACTIONS[label]
    if label in EXTRA:
        return EXTRA[label]
    for source, translation in (
        ("Stow ", "Убрать"),
        ("Drop ", "Бросить"),
        ("Draw ", "Достать"),
    ):
        if label.startswith(source) and "(" in label:
            item = label[len(source) :].split(" (")[0]
            return (
                translation
                + " "
                + {
                    "melee weapon": "оружие ближнего боя",
                    "bow": "лук",
                    "shield": "щит",
                }.get(item, item)
            )
    return label


def figures(state):
    result = []
    for seat, fighter in enumerate(state["fighters"]):
        result.append(
            dict(
                fighter,
                side=seat,
                kind="fighter",
                title=ui.name(fighter),
                conditions=[v for k, v in ui.CONDITIONS.items() if fighter.get(k)]
                + ui.special_status(fighter),
            )
        )
    for entity in state["entities"]:
        result.append(
            dict(entity, kind="companion", title=ui.entity_display(entity)[0])
        )
    for obj in state["objects"]:
        result.append(dict(obj, kind="object", title=ui.object_name(obj)))
    return result


def target_for(state, action, canon, pieces):
    phase = state["phase"]
    if phase == 55 and "jump" in canon.casefold():
        return []
    if phase in (92, 93):
        if action < 4:
            return [action]
        if not 32 <= action < 48:
            return []
        return [
            p["target_id"]
            for p in pieces
            if p["target_id"] < 4 and (action - 32) & (1 << p["target_id"])
        ]
    if phase == 108 and 32 <= action < 48 and "target" in canon.casefold():
        return [
            p["target_id"]
            for p in pieces
            if p["target_id"] < 4 and (action - 32) & (1 << p["target_id"])
        ]
    if phase == 80 and 16 <= action < 20:
        action -= 16
    elif (
        phase == 48
        and 32 <= action < 48
        and re.search("target|include|recipient", canon, re.I)
    ):
        return [
            p["target_id"]
            for p in pieces
            if p["target_id"] < 4 and (action - 32) & (1 << p["target_id"])
        ]
    elif phase not in TARGET_PHASES and not (
        phase in (48, 106, 108)
        and re.match(r"Target |Aim (?:one|all|at) |Attack |Include ", canon, re.I)
    ):
        return []
    return [action] if any(p["target_id"] == action for p in pieces) else []


def board_cell(state, action, canonical, pieces):
    if cell := re.search(r"\b([A-I])([1-7])\b", canonical):
        return [ord(cell[1]) - 65, int(cell[2]) - 1]
    if state["phase"] in (52, 57) and action < 9 and state["target_row"] is not None:
        return [action, state["target_row"]]
    if state["phase"] == 13 and (
        jump := re.match(r"Jump (\d+) ft ([A-Za-z]+)", canonical)
    ):
        dirs = [name.replace("-", "") for name in list(ui.MOVE_LABELS)[:8]]
        if jump[2].lower() in dirs:
            dx, dy = DIRECTIONS[dirs.index(jump[2].lower())]
            actor = pieces[state["viewer"]]
            return [
                actor["x"] + dx * int(jump[1]) // 5,
                actor["y"] + dy * int(jump[1]) // 5,
            ]
    if action >= 8:
        return None
    if state["phase"] in (0, 10, 34, 54):
        origin = pieces[state["viewer"]]
    elif state["phase"] == 53:
        origin = next(
            (p for p in pieces if p.get("active") and p["kind"] == "companion"), None
        )
    else:
        return None
    if origin:
        dx, dy = DIRECTIONS[action]
        return [origin["x"] + dx, origin["y"] + dy]


def menu(state, table, pieces):
    options = []
    viewer = state["viewer"]
    phase = state["phase"]
    # Display every contextual grant even when several lack a playable action.
    entries = {
        int(row[0]) if int(row[0]) >= 0 else -(index + 1): row[1:]
        for index, row in enumerate(table["book"])
    }
    for action, canon in state["actions"].items():
        description = ui.describe_action(state, action, viewer)
        title = localized(description.split(" · ")[0].split(" → ")[0])
        kind = "choice"
        targets = target_for(state, action, canon, pieces)
        cell = board_cell(state, action, canon, pieces)
        if phase == 0:
            if action in entries:
                kind = "book"
            elif action < 8 or 48 <= action <= 50:
                kind = "move"
            elif 108 <= action < 120 or action in (43, 47, 124, 125):
                kind = "gear"
            elif action == 8:
                kind = "end"
            else:
                kind = "combat"
        if kind == "move" and action < 8:
            title = "Шаг в " + ui.coordinate(*cell)
            description = f"{5 * table['costs'][action]} фт перемещения · {ui.DIRECTIONS[action][0]}"
        if phase in (19, 69, 84, 111, 117) and action <= 6 and "ft" in canon:
            title = f"Высота {5*action} фт"
            description = title
        if rank := re.fullmatch(r"Cast (.+?) (?:with|using a) level (\d+) slot", canon):
            title = f"{localized(rank[1])} · ячейка {rank[2]} ур."
            description = title
        if phase == 53 and action < 8 and cell:
            title = "Шаг спутника в " + ui.coordinate(*cell)
            description = title
        if phase == 14 and targets:
            title = next(p["title"] for p in pieces if p["target_id"] == targets[0])
            description = f"Цель · {title}"
        if kind == "book":
            title = localized(canon)
            description = (
                ui.describe_magic_action(canon, pieces[viewer])[1]
                or TIMING[entries[action][2]]
            )
        options.append(
            dict(
                id=action,
                title=title,
                description=description,
                canonical=canon,
                kind=kind,
                cell=cell,
                targets=targets,
            )
        )
    book = []
    for action, (canon, tier, timing, unlock, reason) in entries.items():
        available = phase == 0 and action in state["actions"]
        why = (
            f"Открывается на уровне {unlock}." if reason == "level" else REASONS.get(reason, reason)
        )
        book.append(
            dict(
                id=action,
                title=localized(canon),
                tier=int(tier),
                timing=TIMING.get(timing, timing),
                available=available,
                reason="" if available else why,
            )
        )
    return options, book


def blocked_combat(s):
    if s["phase"] != 0 or s["actor"] != s["viewer"]:
        return []
    actor = s["fighters"][s["viewer"]]
    spent = not s["action_available"] and not s["attacks_left"]
    entries = []
    for index, title in [
        (9, "Ближняя атака"),
        (10, "Дальнобойная атака"),
        (12, "Отход"),
        (13, "Уклонение"),
    ]:
        if index in s["actions"]:
            continue
        if index <= 10 and spent:
            reason = "Не осталось действия или дополнительной атаки."
        elif index == 10 and not actor["ranged_die"]:
            reason = "Сборка без лука. Метание доступного оружия — во вкладке вещей."
        elif index == 10 and not actor["arrows"]:
            reason = "Закончились стрелы."
        elif index >= 12 and not s["action_available"]:
            reason = "Основное действие уже потрачено."
        elif index <= 10:
            reason = (
                "Требуются готовое оружие и допустимая цель в досягаемости."
                if index == 9
                else "Проверьте, взят ли лук и есть ли допустимая цель."
            )
        else:
            reason = "Текущее состояние бойца ограничивает действия."
        entries.append(dict(id=index, title=title, reason=reason))
    return entries


def walking_routes(state, table):
    """Estimates use the visible board only. Interrupts stop an issued route."""
    viewer = state["viewer"]
    me = state["fighters"][viewer]
    if (
        state["phase"] != 0
        or state["actor"] != viewer
        or not table["edges"]
        or me.get("altitude")
        or me.get("prone")
        or me.get("footprint", 1) != 1
        or any(e.get("mounted") and e["side"] == viewer for e in state["entities"])
    ):
        return {}
    start = me["y"] * 9 + me["x"]
    occupied = set()
    for other in [state["fighters"][1 - viewer], *state["entities"]]:
        if other["x"] < 0 or other.get("altitude", 0):
            continue
        for dx in range(other.get("footprint", 1)):
            for dy in range(other.get("footprint", 1)):
                occupied.add((other["x"] + dx, other["y"] + dy))
    best = {start: (0, [])}
    queue = [(0, start)]
    while queue:
        cost, idx = heappop(queue)
        if cost != best[idx][0]:
            continue
        for direction, (dx, dy) in enumerate(DIRECTIONS):
            if not table["edges"][idx] & (1 << direction):
                continue
            x, y = idx % 9 + dx, idx // 9 + dy
            if (x, y) in occupied or idx == start and direction not in state["actions"]:
                continue
            step = (
                table["costs"][direction]
                if idx == start
                else {2: 2, 3: 4}.get(state["terrain"][y][x], 1)
            )
            price = cost + step
            dest = y * 9 + x
            if price <= state["movement_left"] and (
                dest not in best or price < best[dest][0]
            ):
                best[dest] = (price, best[idx][1] + [dest])
                heappush(queue, (price, dest))
    return {
        dest: dict(feet=price * 5, cells=cells)
        for dest, (price, cells) in best.items()
        if len(cells) > 1
    }


class Duel:
    def __init__(
        self,
        policy,
        *,
        player=11,
        opponent=4,
        player_variant=0,
        opponent_variant=0,
        level=5,
        enemy_level=5,
        seat=0,
        seed=None,
    ):
        for v, lo, hi in (
            (player, 0, 11),
            (opponent, 0, 11),
            (level, 5, 5),
            (enemy_level, 5, 5),
            (seat, 0, 1),
            (player_variant, 0, 2),
            (opponent_variant, 0, 2),
        ):
            if type(v) is not int or not lo <= v <= hi:
                raise ValueError("Проверьте класс, подкласс и сторону. Оба бойца играют на уровне 5.")
        self.seed = secrets.randbits(64) if seed is None or seed == "" else int(seed)
        if not 0 <= self.seed < 2**64:
            raise ValueError("Зерно должно помещаться в 64 бита.")
        self.policy, self.seat = policy, seat
        self.builds = [player * 3 + player_variant, opponent * 3 + opponent_variant]
        if seat:
            self.builds.reverse()
        self.batch = Batch(1)
        self.batch.reset_builds([self.seed], [self.builds])
        self.presets = roster()
        self.state = self.batch.state(viewer=seat)
        self.rev = 0
        self.events = []
        self.event(
            "Начало дуэли",
            [
                f"Инициатива · {ui.fighter_name(self.state['fighters'][self.state['owner']], self.state['owner'], seat)}"
            ],
        )

    def event(self, title, lines, turn=None):
        self.events.append(
            dict(
                turn=self.state["turns"] if turn is None else turn,
                title=title,
                lines=[s.strip() for s in lines],
            )
        )

    def step(self, action, *, bot_main=False):
        old = self.state
        human = old["actor"] == self.seat
        title = ""
        if human:
            title = ui.describe_action(old, action, self.seat)
            if old["phase"] == 0:
                title = title.split(" · ")[0]
            elif old["phase"] == 19 and old["target_cell"] is not None and action < 7:
                title = (
                    f"Центр {ui.coordinate(*old['target_cell'])} · высота {5*action} фт"
                )
        self.batch.step([action])
        self.rev += 1
        self.state = new = self.batch.state(viewer=self.seat)
        lines = ui.state_delta(old, new, self.seat)
        if not human and old["fighters"][1 - self.seat]["visible"]:
            # Only a few outward physical actions can be named without disclosing
            # the enemy's private target, prepared trigger, resources or spell.
            if bot_main:
                title = {
                    8: "Соперник завершил ход",
                    9: "Соперник атакует оружием",
                    10: "Соперник стреляет",
                    13: "Соперник уклоняется",
                }.get(action, "")
        if not human and not title and lines:
            title = "Действие соперника"
        if title or lines:
            self.event(("Вы · " if human else "") + title, lines, old["turns"])
        if old["turns"] != new["turns"] and new["winner"] == -1:
            self.event("Ваш ход" if new["owner"] == self.seat else "Ход соперника", [])

    def bot(self):
        while self.state["winner"] == -1 and self.state["actor"] != self.seat:
            before = len(self.events)
            x, mask, _, _ = self.batch.observe()
            self.step(self.policy.act(x[0], mask[0]), bot_main=bool(x[0, 256]))
            if len(self.events) > before:
                break

    def walk(self, destination):
        routes = walking_routes(self.state, self.batch.tabletop(self.seat))
        if type(destination) is not int or destination not in routes:
            raise ValueError("Маршрут уже недоступен.")
        for cell in routes[destination]["cells"]:
            old = self.state
            if old["actor"] != self.seat or old["phase"] != 0:
                break
            me = old["fighters"][self.seat]
            dx, dy = cell % 9 - me["x"], cell // 9 - me["y"]
            if (dx, dy) not in DIRECTIONS:
                break
            direction = DIRECTIONS.index((dx, dy))
            if direction not in old["actions"]:
                break
            self.step(direction)
            now = self.state["fighters"][self.seat]
            foe = 1 - self.seat
            if (
                now["y"] * 9 + now["x"] != cell
                or now["hp"] != me["hp"]
                or now.get("prone") != me.get("prone")
                or self.state["fighters"][foe]["visible"]
                != old["fighters"][foe]["visible"]
            ):
                break

    def play(self, action, cell=None):
        s = self.state
        if (
            s["actor"] != self.seat
            or type(action) is not int
            or action not in s["actions"]
        ):
            raise ValueError("Это решение сейчас недоступно.")
        if cell is not None:
            cols = self.batch.tabletop(self.seat)["columns"]
            if (
                type(cell) is not int
                or not 0 <= cell < 9
                or not 0 <= action < len(cols)
                or not cols[action] & (1 << cell)
            ):
                raise ValueError("Эта клетка недоступна.")
        self.step(action)
        if cell is not None:
            self.step(cell)

    def view(self):
        s = self.state
        table = self.batch.tabletop(self.seat)
        pieces = figures(s)
        for piece, build in zip(pieces[:2], self.builds):
            piece["subclass"] = ui.subclass_name(self.presets[build])
        choices, book = menu(s, table, pieces)
        active = s["actor"] == self.seat
        return dict(
            rev=self.rev,
            seat=self.seat,
            seed=str(self.seed),
            actor=s["actor"],
            owner=s["owner"],
            turn=s["turns"],
            phase=s["phase"],
            winner=s["winner"],
            terrain=s["terrain"],
            layout=s["layout"],
            pieces=pieces,
            fields=s["spell_fields"],
            hazards=s["caltrops"],
            routes=walking_routes(s, table),
            cover=table["cover"],
            columns=table["columns"],
            slices=table["slices"],
            effects=s["target_effects"] if active else {},
            center=s["target_cell"],
            prompt=(PHASE_NAMES.get(s["phase"]) or ui.phase_prompt(s, self.seat) or "")
            if active
            else "",
            pending=(
                f"Центр {ui.coordinate(*s['target_cell'])} · выберите высоту"
                if s["phase"] == 19 and s["target_cell"]
                else ui.pending_description(s)
                if active and s["phase"] != 0
                else ""
            ),
            choices=choices,
            blocked=blocked_combat(s),
            book=book if active and s["phase"] == 0 else [],
            budget=dict(
                move=5 * s["movement_left"],
                action=s["action_available"],
                bonus=s["bonus_available"],
                attacks=s["attacks_left"],
            ),
            events=self.events,
            result=s["limit_reason"] or s["result_reason"],
        )
