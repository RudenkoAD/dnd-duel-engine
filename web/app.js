import { Board, coord } from "./board.js";

const $ = (id) => document.getElementById(id);
const paths = {
  die: "M12 2 22 8 20 19 12 23 3 18 2 8Z M2 8l10-4 10 4-10 13Z M2 8l10 3 10-3 M12 11v10",
  guide: "M3 4q5-2 9 1 4-3 9-1v15q-5-2-9 1-4-3-9-1z M12 5v15",
  plus: "M12 5v14 M5 12h14",
  hour: "M6 3h12 M6 21h12 M7 4c0 5 2 5 5 8-3 3-5 3-5 8 M17 4c0 5-2 5-5 8 3 3 5 3 5 8",
  swords: "M3 3l5 2 12 14-2 2L5 7z M21 3l-5 2-4 5 M9 13l-5 6 2 2 5-6 M3 15l6 5 M15 20l6-5",
  sword: "M4 20l4-4m-3-3 6 6m-4-5L18 3l3-1-1 4L10 17",
  wand: "m4 21 12-13 M13 3l2 3 4 1-4 2-2 4-1-4-4-2 4-1z M20 13v4m-2-2h4 M4 3v4M2 5h4",
  flame: "M12 2c2 6-3 7-1 10 1-1 3-2 4-6 5 5 7 10 3 14-2 2-8 3-12-1-4-5 1-9 3-12-1 5 1 5 2 5",
  ice: "M12 2v20 M3 7l18 10 M3 17 21 7 M9 4l3 3 3-3 M9 20l3-3 3 3 M3 11l4-2-1-4 M21 13l-4 2 1 4",
  spark: "m12 2 2.6 6.8L21 12l-6.4 2.8L12 22l-2.5-7.2L3 12l6.5-3.2z M19 2v4m-2-2h4",
  bolt: "m14 2-10 12h7l-2 8L21 9h-8z",
  shield: "m12 2 8 3v7c0 4-3 7-8 10-5-3-8-6-8-10V5z M8 12l3 3 5-6",
  move: "M3 18h5c5 0 1-12 7-12h5 M16 2l4 4-4 4 M4 16l2 2-2 2",
  pin: "M19 9c0 5-7 12-7 12S5 14 5 9a7 7 0 1 1 14 0z M14 9a2 2 0 1 1-4 0 2 2 0 1 1 4 0",
  map: "m2 6 7-3 6 3 7-3v15l-7 3-6-3-7 3z M9 3v15 M15 6v15",
  layers: "m2 8 10-5 10 5-10 5z M2 12l10 5 10-5 M2 16l10 5 10-5",
  left: "M5 6v6h6 M5 12c2-8 15-8 15 2a6 6 0 0 1-6 6",
  right: "M19 6v6h-6 M19 12C17 4 4 4 4 14a6 6 0 0 0 6 6",
  quill: "M3 21 16 8 M6 16C6 9 13 2 21 3c0 8-7 14-15 13z M13 11h5",
  bag: "M7 3h10l-2 5c5 3 7 7 5 11-3 3-13 3-16 0-2-4 1-9 5-11z M8 8h8",
  lock: "M5 10h14v11H5z M8 10V7a4 4 0 1 1 8 0v3",
  arrow: "M4 12h16 M14 6l6 6-6 6",
  bow: "M5 20C8 10 9 6 20 3L5 20z M5 20 20 3 M3 17l4 4",
};
function icon(name) {
  return '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="' + paths[name] + '"/></svg>';
}
document.querySelectorAll("[data-icon]").forEach((el) => (el.innerHTML = icon(el.dataset.icon)));
const esc = (value) =>
  String(value ?? "").replace(
    /[&<>"']/g,
    (k) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[k],
  );
const classes = [
  "Варвар",
  "Бард",
  "Жрец",
  "Друид",
  "Воин",
  "Монах",
  "Паладин",
  "Следопыт",
  "Плут",
  "Чародей",
  "Колдун",
  "Волшебник",
];
const arenas = ["Мшистая цитадель", "Янтарная застава", "Серебряный бастион", "Туманные врата", "Тёмный бастион"];
const titles = {
  book: "Книга заклинаний",
  combat: "Боевые действия",
  move: "Перемещение",
  gear: "Снаряжение",
};
const tabLabels = { book: "Книга", combat: "Бой", move: "Шаг", gear: "Вещи" };
const cover = ["без укрытия", "половина укрытия (+2)", "три четверти укрытия (+5)", "полное укрытие"];
let rosterCatalog;
let spellTier = null;
let game = sessionStorage.getItem("duel-table"),
  s = null,
  model = "",
  tab = "book",
  selected = null,
  cell = null,
  picked = [],
  busy = false,
  botTimer = 0,
  bookExpanded = false,
  manualRows = false,
  lastSetup = null;
const board = new Board($("board"), inspect, selectCell);
const human = () => s && s.winner === -1 && s.actor === s.seat;
const choices = () => s.choices;
const classIcon = (c) => ([0, 4, 5, 6, 8].includes(c) ? "sword" : c === 7 ? "bow" : "wand");
function styleFor(a) {
  const name = a.canonical || a.title;
  if (/fire|burn|scorch|огнен|палящ|ладони/i.test(name)) return ["fire", "flame"];
  if (/frost|ice|холод/i.test(name)) return ["ice", "ice"];
  if (/shock|lightning|гром|электро/i.test(name)) return ["arcane", "bolt"];
  if (/shield|armor|щит|доспех|уклон/i.test(name)) return ["", "shield"];
  if (a.kind === "move" || a.cell) return ["", "move"];
  if (a.kind === "gear") return ["fire", "bag"];
  if (a.kind === "combat") return ["", "swords"];
  return ["arcane", "spark"];
}
function error(msg) {
  $("error").textContent = msg;
  $("error").hidden = !msg;
}
async function api(path, body) {
  const response = await fetch("/api/" + path, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await response.json();
  if (!response.ok) {
    if (response.status === 409 && data.view) accept(data.view);
    const e = new Error(data.error || "Не удалось получить позицию.");
    e.status = response.status;
    throw e;
  }
  return data;
}
function accept(view) {
  const old = s;
  s = view;
  selected = null;
  cell = null;
  picked = [];
  manualRows = false;
  bookExpanded = false;
  if (!old || old.seat !== s.seat || old.pieces[s.seat].class_id !== s.pieces[s.seat].class_id) {
    tab = s.book.length ? "book" : "combat";
    spellTier = null;
  }
  render();
  if (
    old &&
    human() &&
    s.phase !== old.phase &&
    matchMedia("(max-width:700px)").matches &&
    (s.columns.length || s.choices.some((a) => a.targets.length))
  ) {
    $("board").scrollIntoView({ block: "center" });
  }
}
function schedule() {
  clearTimeout(botTimer);
  if (s && !human() && s.winner === -1 && !$("modal").open)
    botTimer = setTimeout(() => perform("bot"), 420);
}
async function perform(path, extra = {}) {
  if (busy) return;
  busy = true;
  clearTimeout(botTimer);
  error("");
  controls();
  let ok = false;
  try {
    const data = await api(path, path === "new" ? extra : { game, rev: s?.rev, ...extra });
    game = data.game;
    model = data.model;
    sessionStorage.setItem("duel-table", game);
    if (path === "new") s = null;
    accept(data.view);
    ok = true;
  } catch (e) {
    error(e.message);
    if (e.status === 404 && path === "state") {
      game = null;
      sessionStorage.removeItem("duel-table");
    }
  } finally {
    busy = false;
    controls();
    if (ok) schedule();
    else if (s && !human() && s.winner === -1) render();
  }
  return ok;
}
function inspect(q) {
  if (!s) return;
  q = q || cell;
  if (!q) {
    $("inspection").textContent = board.cover
      ? s.cover
        ? "Укрытия · жёлтый ½ · зелёный ¾ · голубой полное."
        : "Чтобы оценить укрытия, нужно знать положение соперника."
      : "Наведите на клетку, чтобы оценить местность и укрытие.";
    return;
  }
  const [x, y] = q,
    n = y * 9 + x,
    terrain = s.terrain[y][x];
  const items = [coord(x, y), ["каменная плита", "стена · верх 10 фт", "трудная местность · низкие растения", "заросли · 4 фута движения за фут"][terrain]];
  if (s.cover) items.push("от соперника по рельефу · " + cover[Number(s.cover[n])]);
  else items.push("положение соперника неизвестно");
  for (const p of s.pieces.filter((p) => p.x === x && p.y === y))
    items.push(
      p.title +
        (p.visible === false ? " · известно положение" : "") +
        (p.altitude ? " ↑" + p.altitude * 5 + " фт" : ""),
    );
  if (s.hazards.some((h) => h && h[0] === x && h[1] === y)) items.push("рассыпанные шипы");
  for (const f of s.fields.filter((f) => f.x === x && f.y === y))
    items.push("магическая область · " + f.extent_feet + " фт, высота центра " + 5 * f.altitude + " фт");
  $("inspection").textContent = items.join("  ·  ");
}
function anchors(a) {
  if (a.cell) return [a.cell];
  return s.pieces
    .filter((p) => a.targets.includes(p.target_id) && p.x >= 0)
    .flatMap((p) => {
      const edge = p.footprint || 1;
      return Array.from({ length: edge * edge }, (_, i) => [
        p.x + (i % edge),
        p.y + Math.floor(i / edge),
      ]);
    });
}
function rowCell(x, y) {
  return s.columns.length && s.columns[y] & (1 << x);
}
function selectCell(q) {
  if (!s) return;
  const [x, y] = q;
  cell = q;
  picked = [];
  selected = null;
  if (human() && !busy) {
    if (rowCell(x, y)) {
      selected = {
        ...choices().find((a) => a.id === y),
        column: x,
        title: "Точка " + coord(x, y),
        description: "Указать клетку " + coord(x, y) + ". Следующий выбор задаёт движок.",
      };
    } else {
      picked = choices().filter((a) => anchors(a).some((p) => p[0] === x && p[1] === y));
      if (picked.length === 1) {
        selected = picked[0];
        if (s.phase === 0) tab = selected.kind;
      }
      if (!picked.length && s.phase === 0 && s.routes?.[y * 9 + x]) {
        const route = s.routes[y * 9 + x];
        tab = "move";
        selected = {
          kind: "route",
          destination: y * 9 + x,
          title: "Маршрут в " + coord(x, y),
          cells: route.cells,
          description:
            route.cells.map((n) => coord(n % 9, Math.floor(n / 9))).join(" → ") +
            " · около " +
            route.feet +
            " фт. При реакции или изменении видимости движение остановится.",
        };
      }
    }
  }
  inspect(q);
  renderPanel();
  draw();
}
function choose(a) {
  if (!human() || busy) return;
  selected = a;
  picked = [];
  cell = anchors(a)[0] || null;
  renderPanel();
  draw();
  inspect(cell);
}
function draw() {
  const targets = s.columns.length
    ? Array.from({ length: 63 }, (_, i) => [i % 9, Math.floor(i / 9)]).filter(([x, y]) => rowCell(x, y))
    : choices().flatMap(anchors);
  let area = null;
  if (s.slices.length && selected && s.slices[selected.id]) {
    const bits = BigInt("0x" + s.slices[selected.id]);
    area = new Set();
    for (let i = 0; i < 63; i++) if (bits & (1n << BigInt(i))) area.add(i);
  }
  board.route = selected?.cells || [];
  board.reachable = s.phase === 0 && tab === "move" ? Object.keys(s.routes || {}).map(Number) : [];
  board.update(s, targets, cell || s.center, area);
}
function fighterCard(p, enemy) {
  const fraction = p.hp == null ? 0 : (100 * p.hp) / p.max_hp;
  return (
    '<button class="duelist ' +
    (enemy ? "enemy" : "") +
    '" data-sheet="' +
    p.side +
    '" aria-label="Лист бойца ' +
    p.title +
    '"><span class="sigil">' +
    icon(classIcon(p.class_id)) +
    '</span><span class="duel-info"><small>' +
    (enemy ? "Соперник · ИИ" : "Ваш герой") +
    " · ур. " +
    p.level +
    " · " + esc(p.subclass) +
    "</small><strong>" +
    p.title +
    '</strong><span class="hp"><b><i style="width:' +
    fraction +
    '%"></i></b>' +
    (p.hp == null ? "скрыт" : p.hp + " / " + p.max_hp + " ОЗ") +
    "</span></span></button>"
  );
}
function logs(events) {
  return events
    .map(
      (e) =>
        '<article class="log-entry"><small>Ход ' +
        e.turn +
        "</small><div><strong>" +
        esc(e.title) +
        "</strong>" +
        e.lines.map((l) => "<p>" + esc(l) + "</p>").join("") +
        "</div></article>",
    )
    .join("");
}
function render() {
  if (!s) return;
  $("arena-name").textContent = arenas[s.layout];
  $("round").textContent = "Раунд " + Math.ceil(s.turn / 2) + " · ход " + s.turn;
  $("whose").textContent =
    s.winner !== -1
      ? "Дуэль завершена"
      : human()
        ? s.owner !== s.seat
          ? "Ваш ответ"
          : "Ваш ход"
        : "Ход сети";
  $("fighters").innerHTML =
    fighterCard(s.pieces[s.seat], false) +
    '<span class="versus">' +
    icon("swords") +
    "</span>" +
    fighterCard(s.pieces[1 - s.seat], true);
  $("recent").innerHTML = logs(s.events.slice(-2).reverse());
  renderPanel();
  inspect(null);
  draw();
}
function option(a, sub, locked = false) {
  const [tone, glyph] = styleFor(a);
  return (
    '<button class="choice ' +
    (locked ? "locked" : "") +
    '" data-' +
    (locked ? "blocked" : "action") +
    '="' +
    a.id +
    '" aria-pressed="' +
    (!locked && selected?.id === a.id && selected?.column == null) +
    '"><span class="glyph ' +
    tone +
    '">' +
    icon(locked ? "lock" : glyph) +
    '</span><span class="choice-text"><strong>' +
    esc(a.title) +
    "</strong>" +
    (sub ? "<em>" + esc(sub) + "</em>" : "") +
    '</span><span class="mark">' +
    (locked ? "?" : "›") +
    "</span></button>"
  );
}
function budget() {
  const p = s.pieces[s.seat],
    comp = s.pieces.find((p) => p.kind === "companion" && p.active && p.side === s.seat);
  if (s.phase === 53 && comp) return "Спутник · <b>" + 5 * comp.movement_left + " фт</b>";
  if (s.phase === 0 && tab === "book" && s.book.length) {
    const levels = [
      [-1, "Все"],
      [0, "Заговоры"],
      ...p.slots.map((n, i) => [i + 1, ["I", "II", "III"][i] + " ◆ " + n]),
    ];
    return levels
      .map(
        ([tier, text]) =>
          '<button class="tier" data-tier="' +
          tier +
          '" aria-pressed="' +
          ((spellTier ?? -1) === tier) +
          '" title="' +
          (tier > 0 ? "Круг " + tier + " · осталось ячеек " + p.slots[tier - 1] : text) +
          '">' +
          text +
          "</button>",
      )
      .join("");
  }
  if (s.owner !== s.seat) return "Реакция · <b>" + (p.reaction ? "готова" : "потрачена") + "</b>";
  return (
    "<span>Ходьба <b>" +
    s.budget.move +
    " фт</b></span><span>Действие <b>" +
    (s.budget.action ? "◆" : "—") +
    "</b></span><span>Бонус <b>" +
    (s.budget.bonus ? "◆" : "—") +
    "</b></span>"
  );
}
function renderPanel() {
  const own = human(),
    main = own && s.phase === 0,
    otherTurn = s.owner !== s.seat,
    finish = s.winner !== -1;
  const comp = s.phase === 53;
  if (main && tab === "book" && !s.book.length) tab = "combat";
  $("panel-kicker").textContent = finish
    ? "Исход боя"
    : !own
      ? "Обученная сеть"
      : otherTurn
        ? "Окно ответа"
        : comp
          ? "Ваш спутник"
          : main
            ? "Ваш арсенал"
            : "Решение игрока";
  $("panel-title").textContent = finish
    ? s.winner === 2
      ? "Ничья"
      : s.winner === s.seat
        ? "Победа"
        : "Поражение"
    : !own
      ? "Ход соперника"
      : main
        ? titles[tab]
        : s.columns.length
          ? "Выберите точку"
          : s.prompt || "Выберите вариант";
  $("instruction").textContent = finish
    ? {
        defeat: "Один из бойцов потерял последние хиты.",
        double_ko: "Оба бойца выбыли.",
        turn_limit: "Достигнут предел ходов.",
        decision_limit: "Достигнут предел решений.",
      }[s.result] || "Дуэль завершена."
    : !own
      ? "Следите за полем и хроникой."
      : main
        ? {
            book: "Выберите заклинание, затем укажите цель.",
            combat: "Выберите приём и проверьте его условия.",
            move: "Нажмите клетку для шага или выберите дальнюю точку маршрута.",
            gear: "Выберите предмет или способ его применения.",
          }[tab]
        : s.pending || "Выберите один из доступных вариантов.";
  $("tabs").hidden = !main;
  $("tabs").innerHTML = main
    ? Object.keys(titles)
        .filter((k) => k !== "book" || s.book.length)
        .map(
          (k) =>
            '<button data-tab="' +
            k +
            '" aria-pressed="' +
            (k === tab) +
            '">' +
            tabLabels[k] +
            "</button>",
        )
        .join("")
    : "";
  $("resources").hidden = !own;
  $("resources").innerHTML = own ? budget() : "";
  if (main && tab === "book")
    $("instruction").textContent = "Выберите заклинание и подтвердите его объявление.";
  const el = $("options"),
    prevScroll = el.scrollTop;
  if (finish)
    el.innerHTML =
      '<div class="board-pick"><strong>' +
      s.turn +
      " ходов</strong><p>Все видимые решения и результаты сохранены в хронике этой дуэли.</p></div>";
  else if (!own)
    el.innerHTML =
      '<div class="board-pick"><strong>' +
      s.pieces[1 - s.seat].title +
      "</strong><p>Если появится возможность ответить, игра остановится на вашем решении.</p></div>";
  else if (picked.length > 1)
    el.innerHTML =
      '<p class="group-heading">Варианты в ' +
      coord(...cell) +
      "</p>" +
      picked.map((a) => option(a)).join("");
  else if (main && tab === "book") {
    const page = s.book.filter((b) => spellTier == null || b.tier === spellTier);
    const available = page.filter((b) => b.available),
      blocked = page.filter((b) => !b.available);
    let html = "";
    for (const tier of [0, 1, 2, 3]) {
      const list = available.filter((b) => b.tier === tier);
      if (list.length)
        html +=
          '<div class="group-heading">' +
          (tier ? "Круг " + ["", "I", "II", "III"][tier] : "Заговоры") +
          "</div>" +
          list
            .map((b) => {
              const a = choices().find((a) => a.id === b.id);
              return option(a, b.timing);
            })
            .join("");
    }
    if (!available.length)
      html =
        '<p class="empty">' +
        (page.length
          ? "Сейчас нет доступных заклинаний."
          : "У этой сборки нет заклинаний выбранного круга.") +
        "</p>";
    if (blocked.length)
      html +=
        '<details id="blocked" ' +
        (bookExpanded ? "open" : "") +
        "><summary>Недоступные заклинания · " +
        blocked.length +
        "</summary>" +
        blocked.map((b) => option(b, b.reason, true)).join("") +
        "</details>";
    el.innerHTML = html;
  } else if (s.columns.length && !manualRows) {
    el.innerHTML =
      '<div class="board-pick"><strong>Прицельтесь на поле</strong><p>Светлая рамка отмечает доступные клетки. Для выбора нажмите клетку или используйте стрелки и Enter.</p></div><button class="link" id="rows">Выбрать ряд из списка ›</button>' +
      choices()
        .filter((a) => a.id >= 7)
        .map((a) => option(a))
        .join("");
  } else {
    const list = main ? choices().filter((a) => a.kind === tab) : choices();
    el.innerHTML =
      list
        .map((a) =>
          option(
            a,
            a.kind === "move" && a.id < 8
              ? a.description
              : a.title !== a.description
                ? a.description.split(" · ").slice(1).join(" · ")
                : "",
          ),
        )
        .join("") || '<p class="empty">В этой группе сейчас нет доступных действий.</p>';
  }
  if (main && tab === "combat" && s.blocked?.length && !picked.length)
    el.innerHTML +=
      '<details id="blocked" ' +
      (bookExpanded ? "open" : "") +
      "><summary>Недоступные действия · " +
      s.blocked.length +
      "</summary>" +
      s.blocked.map((a) => option(a, a.reason, true)).join("") +
      "</details>";
  el.scrollTop = prevScroll;
  const eff = selected && s.phase === 19 ? s.effects[selected.id] : null;
  let hint = selected?.description || "";
  if (eff)
    hint +=
      " · " +
      (eff.self ? "заклинатель в области" : "заклинатель вне области") +
      (s.pieces[1 - s.seat].known ? (eff.foe ? " · соперник в области" : " · соперник вне области") : "");
  $("preview").innerHTML = selected
    ? "<strong>" + esc(selected.title) + "</strong>" + esc(hint)
    : finish
      ? ""
      : !own
        ? "Сеть выбирает действие…"
        : picked.length > 1
          ? "В этой клетке несколько вариантов. Выберите нужный в списке."
          : cell
            ? "Клетка " + coord(...cell) + " выбрана для осмотра."
            : main
              ? "Выберите действие для предпросмотра."
              : "Подтвердите выбор, чтобы продолжить.";
  $("history").disabled = !s;
  $("end").hidden = !main || !choices().some((a) => a.kind === "end");
  $("board-order").hidden = !selected || !human();
  $("board-order").textContent = selected ? "Подтвердить · " + selected.title : "";
  $("confirm").textContent = finish
    ? "Новая дуэль"
    : !own
      ? "Продолжить ход сети"
      : selected?.kind === "end"
        ? "Завершить ход"
        : selected?.column != null
          ? "Указать " + coord(selected.column, selected.id)
          : "Подтвердить";
  controls();
}
function controls() {
  $("confirm").disabled =
    busy || !s || (human() && !selected) || (!human() && s?.winner === -1 && !$("error").textContent);
  $("board-order").disabled = busy;
  $("end").disabled = busy;
  $("new").disabled = busy;
}
function show(title, html) {
  clearTimeout(botTimer);
  $("modal-title").textContent = title;
  $("modal-body").innerHTML = html;
  if (!$("modal").open) $("modal").showModal();
}
function showHelp() {
  const blocks = [
    [
      "Первый бой",
      "Вы играете бирюзовой фигуркой. Цель дуэли — лишить соперника последних хитов. Кликните имя бойца, чтобы посмотреть его лист.",
    ],
    [
      "Шаг и укрытие",
      "Нажмите соседнюю клетку со светлой рамкой или откройте «Шаг» и выберите точку дальше. Маршрут проходит по видимой местности. Движок проверяет каждый шаг и останавливает движение при реакции. Укрытие показано для атаки с известной позиции противника по рельефу, на высоте вашего бойца или верха стены. Каждая клетка — 5 футов.",
    ],
    [
      "Книга и цели",
      "Выберите заклинание и подтвердите объявление. Движок предложит ранг, цель, высоту или направление, когда это требуется. На поле можно выбирать существ и точки. Совпадающие цели и группы доступны в списке.",
    ],
    [
      "Высота и видимость",
      "Стены высотой 10 футов. Кнопки под полем поворачивают камеру, включают вид сверху и срез стен. Знак вопроса обозначает известное положение невидимого существа.",
    ],
    [
      "Ответ на чужом ходу",
      "Игра ждёт вас на каждой реакции, спасброске и ходе спутника. Осмотрите варианты и подтвердите решение. Завершение основного хода требует отдельного подтверждения.",
    ],
    [
      "Клавиатура",
      "Tab и Enter управляют кнопками. На поле стрелки перемещают указатель, Enter выбирает клетку. Esc закрывает окно.",
    ],
  ];
  show(
    "За игровым столом",
    blocks.map(([h, p]) => '<div class="help-block"><h3>' + h + "</h3><p>" + p + "</p></div>").join("") +
      '<p class="meta">Локальная политика · ' +
      esc(model) +
      "</p>",
  );
}
async function setup() {
  if (!rosterCatalog) {
    try {
      const response = await fetch("/api/roster");
      if (!response.ok) throw new Error();
      rosterCatalog = await response.json();
    } catch {
      show("Новая дуэль", "<p>Не удалось загрузить подклассы. Откройте меню ещё раз.</p>");
      return;
    }
  }
  const sel = (vals, n, labels) =>
    vals
      .map(
        (v, i) =>
          '<option value="' +
          v +
          '" ' +
          (v === n ? "selected" : "") +
          ">" +
          esc(labels?.[i] ?? v) +
          "</option>",
      )
      .join("");
  const side = s?.seat ?? 0,
    start = lastSetup || {
      player: s?.pieces[side].class_id ?? 11,
      opponent: s?.pieces[1 - side].class_id ?? 4,
      player_variant: s?.pieces[side].variant ?? 0,
      opponent_variant: s?.pieces[1 - side].variant ?? 0,
      seat: side,
    };
  const variants = (classId, current) => {
    const entries = rosterCatalog.filter((build) => build.class_id === Number(classId));
    return sel(entries.map((b) => b.variant), current, entries.map((b) => b.title));
  };
  const blurb = (classId, variant) => rosterCatalog.find((b) => b.class_id === Number(classId) && b.variant === Number(variant))?.detail ?? "";
  const fighter = (name, key) =>
    '<div class="formrow"><label>' +
    name +
    '<select name="' +
    key +
    '">' +
    sel(
      classes.map((_, i) => i),
      start[key],
      classes,
    ) +
    '</select></label><label>Подкласс<select name="' +
    key + "_variant" +
    '">' +
    variants(start[key], start[key + "_variant"]) +
    '</select></label></div><p class="meta" id="detail-' + key + '">' + esc(blurb(start[key], start[key + "_variant"])) + "</p>";
  show(
    "Новая дуэль",
    '<form id="setup">' +
      fighter("Ваш герой", "player") +
      fighter("Соперник", "opponent") +
      '<label>Ваша сторона<select name="seat">' +
      sel([0, 1], start.seat, ["Запад · A4", "Восток · I4"]) +
      '</select></label><label>Зерно карты и бросков<input name="seed" inputmode="numeric" pattern="[0-9]*" placeholder="Случайное"></label><p class="meta">Оба бойца пятого уровня и начинают с полными ресурсами. Инициативу определяет движок.</p><button class="primary">Начать дуэль</button></form>',
  );
  for (const key of ["player", "opponent"]) {
    const form = $("setup");
    const detail = () => { $("detail-" + key).textContent = blurb(form.elements[key].value, form.elements[key + "_variant"].value); };
    form.elements[key + "_variant"].onchange = detail;
    form.elements[key].onchange = () => {
      form.elements[key + "_variant"].innerHTML = variants(form.elements[key].value, 0);
      detail();
    };
  }
  $("setup").onsubmit = async (e) => {
    e.preventDefault();
    const data = Object.fromEntries(new FormData(e.target));
    for (const key of ["player", "opponent", "player_variant", "opponent_variant", "seat"])
      data[key] = Number(data[key]);
    if (busy) return;
    lastSetup = data;
    $("modal").close();
    await perform("new", data);
  };
}
function sheet(side) {
  const p = s.pieces[side],
    mine = side === s.seat;
  const lines = [
    (mine ? "Ваш герой" : "Обученная сеть") + " · " + p.subclass + " · уровень " + p.level,
    (p.hp == null
      ? "Хиты скрыты"
      : "Хиты " + p.hp + " / " + p.max_hp + (p.temp ? " + " + p.temp + " временных" : "")) +
      (p.ac != null ? " · КД " + p.ac : ""),
    p.known ? coord(p.x, p.y) + " · высота " + p.altitude * 5 + " фт" : "Положение неизвестно",
    p.equipment,
    ...p.conditions,
  ];
  if (mine) {
    const rnames = {
      0: ["Ярость"],
      1: ["Вдохновение"],
      2: ["Божественный канал"],
      3: ["Дикий облик"],
      4: ["Второе дыхание", "Всплеск действий"],
      5: ["Очки концентрации"],
      6: ["Запас исцеления", "Божественный канал"],
      7: [null, "Бесплатные метки"],
      8: [null, "Мешочки шипов"],
      9: ["Единицы чародейства", "Врождённое чародейство"],
    };
    lines.push("Реакция · " + (p.reaction ? "готова" : "потрачена"), "Скорость " + p.speed * 5 + " фт");
    (rnames[p.class_id] || []).forEach((n, i) => {
      if (n) lines.push(n + " · " + p[i ? "secondary" : "resource"]);
    });
    if (p.slots.some(Boolean)) lines.push("Ячейки I / II / III · " + p.slots.join(" / "));
  }
  for (const piece of s.pieces.filter((o) => o.kind === "companion" && o.side === side))
    lines.push(
      piece.title +
        " · " +
        (piece.x >= 0 ? coord(piece.x, piece.y) : "место неизвестно") +
        (piece.hp != null ? " · " + piece.hp + " ОЗ" : ""),
    );
  show(
    p.title,
    lines
      .filter(Boolean)
      .map((l) => '<p class="sheet-line">' + esc(l) + "</p>")
      .join("") +
      '<p class="meta">Зерно ' +
      s.seed +
      " · " +
      esc(model) +
      "</p>",
  );
}
$("modal").addEventListener("close", schedule);
$("history").onclick = () => show("Хроника боя", logs(s.events.slice().reverse()));
$("help").onclick = showHelp;
$("new").onclick = setup;
$("fighters").onclick = (e) => {
  const b = e.target.closest("[data-sheet]");
  if (b) sheet(+b.dataset.sheet);
};
$("left").onclick = () => board.turn(-1);
$("right").onclick = () => board.turn(1);
for (const [id, prop] of [
  ["top", "top"],
  ["cut", "cut"],
  ["cover", "cover"],
])
  $(id).onclick = () => {
    board[prop] = !board[prop];
    $(id).setAttribute("aria-pressed", board[prop]);
    inspect(cell);
    board.draw();
  };
$("resources").onclick = (e) => {
  const b = e.target.closest("[data-tier]");
  if (!b) return;
  spellTier = +b.dataset.tier < 0 ? null : +b.dataset.tier;
  bookExpanded = false;
  picked = [];
  selected = null;
  cell = null;
  renderPanel();
  $("options").scrollTop = 0;
  draw();
};
$("tabs").onclick = (e) => {
  if (e.target.dataset.tab) {
    tab = e.target.dataset.tab;
    selected = null;
    cell = null;
    picked = [];
    renderPanel();
    $("options").scrollTop = 0;
    draw();
  }
};
$("options").onclick = (e) => {
  if (e.target.closest("summary")) {
    bookExpanded = !$("blocked").open;
    return;
  }
  if (e.target.closest("#rows")) {
    manualRows = true;
    renderPanel();
    return;
  }
  const blocked = e.target.closest("[data-blocked]");
  if (blocked) {
    const a = (tab === "combat" ? s.blocked : s.book).find((b) => b.id === +blocked.dataset.blocked);
    show(a.title, "<p>" + esc(a.reason) + "</p>");
    return;
  }
  const b = e.target.closest("[data-action]");
  if (b) choose(choices().find((a) => a.id === +b.dataset.action));
};
$("end").onclick = () => choose(choices().find((a) => a.kind === "end"));
$("confirm").onclick = () => {
  if (s.winner !== -1) return setup();
  if (!human()) return perform("bot");
  if (selected?.kind === "route") return perform("walk", { destination: selected.destination });
  if (selected) return perform("play", { action: selected.id, column: selected.column });
};
$("board-order").onclick = () => $("confirm").click();
if (!game || (!(await perform("state")) && !game))
  await perform("new", { player: 11, opponent: 4, level: 5, enemy_level: 5, seat: 0, seed: "11" });
