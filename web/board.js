// Orthographic miniature table. Every model is built in board coordinates;
// picking uses the same projected polygons that were painted on the canvas.
const TAU = Math.PI * 2;
const palettes = [
  ["#c8ac72", "#91aa83", "#637e6b", "#768b74"],
  ["#b8b49a", "#9fae8e", "#789185", "#859882"],
  ["#c3ac8e", "#a8a389", "#728882", "#839187"],
  ["#a8b1ac", "#96a797", "#718f87", "#84998e"],
];
const sideColors = ["#61e4d9", "#ff956c"];
const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
export const coord = (x, y) => String.fromCharCode(65 + x) + (y + 1);
function noise(n) {
  const v = Math.sin(n * 78.233 + 13.13) * 43758.5453;
  return v - Math.floor(v);
}
function tint(color, value) {
  const n = parseInt(color.slice(1), 16);
  return (
    "rgb(" +
    [n >> 16, (n >> 8) & 255, n & 255].map((v) => clamp(Math.round(v * value), 0, 255)).join(",") +
    ")"
  );
}
function inside(x, y, poly) {
  let hit = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i],
      b = poly[j];
    if (a[1] > y !== b[1] > y && x < ((b[0] - a[0]) * (y - a[1])) / (b[1] - a[1]) + a[0]) hit = !hit;
  }
  return hit;
}
export class Board {
  constructor(canvas, inspect, choose) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.angle = -0.44;
    this.top = false;
    this.cut = false;
    this.hover = null;
    this.selected = null;
    this.targets = [];
    this.inspect = inspect;
    this.choose = choose;
    this.observer = new ResizeObserver(() => this.draw());
    this.observer.observe(canvas);
    const point = (e) => {
      const r = canvas.getBoundingClientRect();
      return this.pick(e.clientX - r.left, e.clientY - r.top);
    };
    canvas.addEventListener("pointermove", (e) => {
      const hit = point(e);
      if (hit?.join() !== this.hover?.join()) {
        this.hover = hit;
        this.inspect(hit);
        this.draw();
      }
    });
    canvas.addEventListener("pointerleave", () => {
      this.hover = null;
      this.inspect(null);
      this.draw();
    });
    canvas.addEventListener("click", (e) => {
      const hit = point(e);
      if (hit) this.choose(hit);
    });
    canvas.addEventListener("keydown", (e) => {
      const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
      if (arrows[e.key]) {
        e.preventDefault();
        const [x, y] = this.hover || this.selected || [4, 3],
          [dx, dy] = arrows[e.key];
        this.hover = [clamp(x + dx, 0, 8), clamp(y + dy, 0, 6)];
        this.inspect(this.hover);
        this.draw();
      } else if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        if (this.hover) this.choose(this.hover);
      }
    });
  }
  turn(n) {
    this.angle += (n * Math.PI) / 2;
    this.draw();
  }
  pick(x, y) {
    return [...this.hits].reverse().find((h) => inside(x, y, h.poly))?.cell || null;
  }
  update(state, targets, selected, area) {
    const old = this.s;
    if (
      old &&
      old.rev !== state.rev &&
      old.seed === state.seed &&
      !matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      this.motion = new Map();
      for (const p of state.pieces) {
        const prev = old.pieces.find((o) => o.target_id === p.target_id);
        if (!prev || p.x < 0 || prev.x < 0 || p.kind === "object") continue;
        const hp = p.hp != null && prev.hp != null ? p.hp - prev.hp : 0;
        if (hp || p.x !== prev.x || p.y !== prev.y)
          this.motion.set(p.target_id, { x: prev.x, y: prev.y, hp });
      }
      this.started = performance.now();
    } else if (!old || old.seed !== state.seed || state.rev < old.rev) this.motion = null;
    this.s = state;
    this.targets = targets;
    this.selected = selected;
    this.area = area;
    this.draw();
  }
  project(x, y, z = 0) {
    const dx = x - 4,
      dy = y - 3;
    return [
      this.cx + this.k * (dx * this.cos - dy * this.sin),
      this.cy + this.k * ((dx * this.sin + dy * this.cos) * this.pitch - z * this.lift),
    ];
  }
  depth(x, y) {
    return x * this.sin + y * this.cos;
  }
  poly(points, fill, stroke, width = 1) {
    const c = this.ctx;
    c.beginPath();
    points.forEach(([x, y], i) => (i ? c.lineTo(x, y) : c.moveTo(x, y)));
    c.closePath();
    if (fill) {
      c.fillStyle = fill;
      c.fill();
    }
    if (stroke) {
      c.lineWidth = width;
      c.strokeStyle = stroke;
      c.stroke();
    }
  }
  ground(x, y, z, rx, ry, fill, edge, w) {
    const p = [
      [-rx, -ry],
      [rx, -ry],
      [rx, ry],
      [-rx, ry],
    ].map(([dx, dy]) => this.project(x + dx, y + dy, z));
    this.poly(p, fill, edge, w);
    return p;
  }
  // Tapered polygonal solids, lit in world space. Rings allow stone courses,
  // carved bases, cloth, faces, weapons and crystals to share one projection.
  solid(x, y, z, rx, ry, h, topX, topY, color, n = 8, rotation = 0, lean = [0, 0]) {
    const rim = (sx, sy, alt, shift) =>
      Array.from({ length: n }, (_, i) => {
        const a = rotation + (i * TAU) / n;
        return [x + sx * Math.cos(a) + shift[0], y + sy * Math.sin(a) + shift[1], alt];
      });
    const bottom = rim(rx, ry, z, [0, 0]),
      top = rim(topX, topY, z + h, lean);
    const p = (a) => this.project(...a);
    const sides = [];
    for (let i = 0; i < n; i++) {
      const j = (i + 1) % n,
        angle = rotation + ((i + 0.5) * TAU) / n;
      const depth = Math.cos(angle) * this.sin + Math.sin(angle) * this.cos;
      sides.push([
        depth,
        [bottom[i], bottom[j], top[j], top[i]].map(p),
        0.77 + 0.22 * Math.cos(angle + 2.2),
      ]);
    }
    sides.sort((a, b) => a[0] - b[0]);
    for (const [, pts, light] of sides) this.poly(pts, tint(color, light));
    if (topX > 0 && topY > 0) this.poly(top.map(p), tint(color, 1.1));
  }
  block(x, y, z, w, d, h, color, cap = 1) {
    this.solid(
      x,
      y,
      z,
      w * Math.SQRT1_2,
      d * Math.SQRT1_2,
      h,
      w * Math.SQRT1_2 * cap,
      d * Math.SQRT1_2 * cap,
      color,
      4,
      Math.PI / 4,
    );
  }
  ellipse(x, y, z, rx, ry, color) {
    const pts = Array.from({ length: 24 }, (_, i) =>
      this.project(x + rx * Math.cos((i * TAU) / 24), y + ry * Math.sin((i * TAU) / 24), z),
    );
    this.poly(pts, color);
  }
  text(x, y, text, color, size = 11) {
    const c = this.ctx;
    c.textAlign = "center";
    c.textBaseline = "middle";
    c.font = "600 " + size + "px system-ui";
    c.fillStyle = color;
    c.fillText(text, x, y);
  }
  sky(w, h) {
    const c = this.ctx,
      g = c.createLinearGradient(0, 0, 0, h);
    g.addColorStop(0, "#515d7c");
    g.addColorStop(0.24, "#af9392");
    g.addColorStop(0.46, "#e3b68c");
    g.addColorStop(0.63, "#93969a");
    g.addColorStop(1, "#273e47");
    c.fillStyle = g;
    c.fillRect(0, 0, w, h);
    const sx = w * 0.76,
      sy = h * 0.23,
      r = Math.min(w, h) * 0.068;
    const sun = c.createRadialGradient(sx, sy, r * 0.2, sx, sy, r * 3.6);
    sun.addColorStop(0, "#fff2bfaa");
    sun.addColorStop(1, "#fbd5a000");
    c.fillStyle = sun;
    c.fillRect(sx - r * 4, sy - r * 4, r * 8, r * 8);
    c.fillStyle = "#f5d9af";
    c.beginPath();
    c.arc(sx, sy, r, 0, TAU);
    c.fill();
    for (let i = 0; i < 22; i++) {
      const x = noise(i + 200) * w,
        y = noise(i + 450) * h * 0.38;
      c.fillStyle = "rgba(247,220,193,.20)";
      c.fillRect(x, y, (0.025 + noise(i) * 0.12) * w, 1 + noise(i) * 1.5);
    }
    for (let layer = 0; layer < 4; layer++) {
      const base = h * (0.32 + layer * 0.093),
        color = ["#a299a2", "#8a8b99", "#687c85", "#506c74"][layer];
      const pts = [[0, h]];
      for (let i = 0; i <= 50; i++) {
        const x = (i * w) / 50;
        const y =
          base -
          (0.52 + 0.34 * Math.sin(i * 0.23 + layer * 9) + noise(i + layer * 81) * 0.19) *
            h *
            (0.14 + layer * 0.012);
        pts.push([x, y]);
      }
      pts.push([w, h]);
      this.poly(pts, color);
    }
    // Distant aqueduct and cypresses frame the playable stone platform.
    const vx = w * 0.02,
      vy = h * 0.41,
      unit = w * 0.014;
    c.save();
    c.strokeStyle = "#5a7279";
    c.lineWidth = unit * 0.48;
    for (let i = 0; i < 10; i++) {
      c.beginPath();
      c.arc(vx + (i * 2 + 1) * unit, vy, unit, Math.PI, TAU);
      c.stroke();
      c.fillStyle = "#5a7279";
      c.fillRect(vx + i * 2 * unit - unit * 0.24, vy, unit * 0.48, h * 0.09);
    }
    c.fillRect(vx - unit * 0.24, vy - unit * 1.25, 20 * unit + unit * 0.48, unit * 0.35);
    c.restore();
    for (let i = 0; i < 100; i++) {
      const x = noise(i + 811) * w,
        y = h * (0.49 + noise(i + 615) * 0.16),
        s = h * (0.011 + noise(i + 44) * 0.042);
      this.poly(
        [
          [x - s * 0.21, y],
          [x, y - s],
          [x + s * 0.21, y],
        ],
        "#4a666c",
      );
    }
    const mist = c.createLinearGradient(0, h * 0.51, 0, h);
    mist.addColorStop(0, "#344d5500");
    mist.addColorStop(1, "#1b303be8");
    c.fillStyle = mist;
    c.fillRect(0, 0, w, h);
  }
  tree(x, y, z, size, seed) {
    this.solid(x, y, z, 0.09 * size, 0.1 * size, 0.65 * size, 0.08 * size, 0.08 * size, "#715742", 5);
    for (let j = 0; j < 3; j++) {
      const r = (0.43 - j * 0.085) * size;
      this.solid(
        x,
        y,
        z + (0.33 + j * 0.35) * size,
        r,
        r * 0.91,
        0.73 * size,
        0.015,
        0.015,
        ["#345c59", "#477467", "#588772"][j],
        7,
        seed + j * 0.6,
      );
    }
  }
  rocks(x, y, z, n, seed, s = 1) {
    for (let i = 0; i < n; i++) {
      const dx = (noise(seed + i * 4) - 0.5) * 0.76,
        dy = (noise(seed + 1 + i * 4) - 0.5) * 0.76;
      const r = (0.065 + noise(seed + 2 + i * 4) * 0.13) * s;
      this.solid(
        x + dx,
        y + dy,
        z,
        r,
        r * 0.83,
        r * 0.8,
        r * 0.7,
        r * 0.5,
        i % 2 ? "#999b82" : "#817f71",
        5,
        i * 0.7,
        [r * 0.1, r * 0.1],
      );
    }
  }
  wall(x, y, seed) {
    const h = this.cut ? 0.36 : 1.62;
    this.block(x, y, 0, 0.99, 0.99, h * 0.16, "#787e72");
    for (let i = 0; i < 4; i++) {
      const z = h * (0.16 + i * 0.185);
      if (i % 2) {
        for (const dx of [-0.235, 0.235])
          this.block(x + dx, y, z, 0.458, 0.9, h * 0.177, i % 3 ? "#a7a891" : "#acac93");
      } else {
        for (const dy of [-0.234, 0.234]) this.block(x, y + dy, z, 0.89, 0.456, h * 0.177, "#a3a28c");
      }
    }
    this.block(x, y, h * 0.91, 0.99, 0.99, h * 0.09, "#babc9d");
    const moss = (dx, dy, r) => this.ellipse(x + dx, y + dy, h + 0.003, r, r * 0.55, "#849675");
    moss(0.22, -0.2, 0.19);
    moss(-0.15, 0.19, 0.15);
    if (!this.cut && seed % 3 === 0) this.rocks(x, y, h, 3, seed, 0.7);
    return h;
  }
  standard(x, y, color, d) {
    this.block(x, y, 0, 0.29, 0.29, 0.12, "#8a8771");
    this.solid(x, y, 0.12, 0.037, 0.037, 1.31, 0.029, 0.029, "#a38c67", 7);
    this.solid(x, y, 1.43, 0.07, 0.07, 0.13, 0.006, 0.006, "#e4c383", 6);
    const pt = (dx, dy, h) => this.project(x + dx * d, y + dy, h);
    const top = [pt(0, 0, 1.4), pt(0.26, 0.055, 1.35), pt(0.54, 0.02, 1.34)];
    const bottom = [pt(0, 0, 0.9), pt(0.26, 0.055, 0.74), pt(0.54, 0.02, 0.88)];
    this.poly([top[0], top[1], bottom[1], bottom[0]], color);
    this.poly([top[1], top[2], bottom[2], bottom[1]], tint(color, 0.82));
    this.poly(
      [pt(0.24, 0.06, 1.26), pt(0.34, 0.05, 1.09), pt(0.24, 0.06, 0.97), pt(0.15, 0.04, 1.09)],
      "#e8d1a3",
    );
  }
  torch(x, y, z) {
    this.block(x, y, z, 0.55, 0.55, 0.13, "#9eaa94");
    this.solid(x, y, z + 0.13, 0.23, 0.23, 0.54, 0.16, 0.16, "#858d80", 8);
    this.solid(x, y, z + 0.67, 0.17, 0.17, 0.2, 0.29, 0.29, "#a88252", 8);
    this.solid(x, y, z + 0.87, 0.17, 0.17, 0.47, 0.02, 0.02, "#ffb85d", 6, 0, [0.06, 0.02]);
    this.solid(x, y, z + 0.88, 0.09, 0.09, 0.3, 0, 0, "#ffe99b", 5);
    const [px, py] = this.project(x, y, z + 1.02),
      radius = this.k * 0.56,
      c = this.ctx;
    const glow = c.createRadialGradient(px, py, 0, px, py, radius);
    glow.addColorStop(0, "#ffd26640");
    glow.addColorStop(1, "#ffc64300");
    c.fillStyle = glow;
    c.fillRect(px - radius, py - radius, radius * 2, radius * 2);
  }
  hero(p, x, y) {
    const z = p.altitude || 0,
      own = p.side === this.s.seat,
      base = sideColors[own ? 0 : 1];
    if ((!p.visible && p.kind === "fighter") || p.detected) {
      const [sx, sy] = this.project(x, y, z + 0.23);
      this.ellipse(x, y, z + 0.02, 0.33, 0.33, "#ffd39c65");
      this.text(sx, sy, "?", "#fff1ce", this.k * 0.44);
      return;
    }
    const radius = p.footprint === 2 ? 0.52 : 0.34;
    this.ellipse(x + 0.05, y + 0.1, z + 0.006, radius * 1.28, radius * 0.9, "#192d3666");
    this.solid(x, y, z + 0.015, radius, radius, 0.09, radius, radius, "#354848", 14);
    this.solid(x, y, z + 0.105, radius, radius, 0.045, radius * 0.94, radius * 0.94, base, 14);
    const cloth = own ? "#367f88" : "#a74d46",
      light = own ? "#76cfbd" : "#de986a",
      metal = own ? "#b9c9c0" : "#cdbfa7";
    if (p.kind === "companion" || p.conditions?.includes("Дикий облик")) {
      this.solid(x, y, z + 0.15, 0.26, 0.15, 0.28, 0.23, 0.14, cloth, 7);
      this.solid(x + 0.14, y, z + 0.42, 0.13, 0.12, 0.18, 0.11, 0.1, light, 6);
      this.solid(x - 0.15, y, z + 0.38, 0.11, 0.08, 0.25, 0.02, 0.02, light, 5, 0, [-0.13, 0]);
      return;
    }
    if (p.prone) {
      this.solid(x, y, z + 0.15, 0.14, 0.27, 0.16, 0.13, 0.24, cloth, 7);
      this.solid(x, y + 0.21, z + 0.27, 0.11, 0.11, 0.08, 0.09, 0.09, metal);
      return;
    }
    const mage = [1, 2, 3, 9, 10, 11].includes(p.class_id);
    for (const dx of [-0.11, 0.11]) this.block(x + dx, y + 0.02, z + 0.15, 0.13, 0.19, 0.19, "#544e4a");
    this.solid(x, y, z + 0.27, 0.26, 0.2, 0.39, 0.16, 0.13, cloth, 8);
    this.solid(x, y, z + 0.56, 0.18, 0.15, 0.18, 0.22, 0.15, mage ? light : metal, 7);
    this.solid(x, y, z + 0.52, 0.19, 0.15, 0.055, 0.19, 0.15, "#d9ac60", 8);
    this.solid(x, y, z + 0.71, 0.14, 0.13, 0.19, 0.13, 0.12, "#deb78d", 8);
    this.solid(x, y + 0.05, z + 0.7, 0.126, 0.115, 0.12, 0.08, 0.1, "#bba88c", 6);
    if (mage) {
      this.solid(x, y, z + 0.89, 0.22, 0.2, 0.04, 0.17, 0.16, cloth, 9);
      this.solid(x, y, z + 0.92, 0.16, 0.15, 0.29, 0.015, 0.014, cloth, 8, 0, [-0.06, 0.02]);
      this.solid(x + 0.26, y + 0.07, z + 0.15, 0.029, 0.029, 0.83, 0.032, 0.032, "#ae8757", 6);
      this.solid(x + 0.26, y + 0.07, z + 0.98, 0.085, 0.085, 0.16, 0.015, 0.015, base, 5);
    } else {
      this.solid(x, y, z + 0.85, 0.165, 0.15, 0.16, 0.07, 0.06, metal, 8);
      this.solid(x, y - 0.02, z + 0.995, 0.047, 0.14, 0.13, 0.018, 0.08, light, 6);
      if (p.class_id === 0) {
        this.solid(x + 0.24, y, z + 0.2, 0.032, 0.032, 0.82, 0.03, 0.03, "#a0805c", 6);
        this.solid(x + 0.27, y, z + 0.83, 0.19, 0.055, 0.14, 0.145, 0.04, metal, 5);
      } else if (p.class_id === 7) {
        const points = [
          [0.2, 0.15, 0.37],
          [0.34, 0.13, 0.62],
          [0.36, 0.11, 0.84],
          [0.24, 0.1, 1.08],
        ].map(([dx, dy, h]) => this.project(x + dx, y + dy, z + h));
        this.poly(points, null, "#e6b477", 2);
      } else
        this.solid(x + 0.24, y, z + 0.48, 0.045, 0.045, 0.51, 0.016, 0.016, "#d9dbcb", 4, 0, [0.09, 0]);
      this.block(x + 0.24, y, z + 0.55, 0.19, 0.065, 0.052, "#c99c65");
      this.solid(x - 0.2, y + 0.09, z + 0.36, 0.14, 0.07, 0.31, 0.13, 0.06, cloth, 6);
    }
  }
  draw() {
    const w = this.canvas.clientWidth,
      h = this.canvas.clientHeight;
    if (!w || !h) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    if (this.canvas.width !== Math.round(w * dpr) || this.canvas.height !== Math.round(h * dpr)) {
      this.canvas.width = Math.round(w * dpr);
      this.canvas.height = Math.round(h * dpr);
    }
    this.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    this.sky(w, h);
    this.hits = [];
    if (!this.s) return;
    this.cos = Math.cos(this.angle);
    this.sin = Math.sin(this.angle);
    this.pitch = this.top ? 0.91 : 0.54;
    this.lift = this.top ? 0.26 : 0.78;
    const zw = Math.max(2, ...this.s.pieces.filter((p) => p.x >= 0).map((p) => (p.altitude || 0) + 1.3));
    const extentX = 10.95 * Math.abs(this.cos) + 8.9 * Math.abs(this.sin);
    const extentY = (10.95 * Math.abs(this.sin) + 8.9 * Math.abs(this.cos)) * this.pitch;
    this.k = Math.min(w / (extentX + 0.3), h / (extentY + 2.1 + Math.max(0, zw - 2) * this.lift));
    this.cx = w / 2;
    this.cy = h * 0.53 + Math.max(0, zw - 2) * this.k * this.lift * 0.45;
    const s = this.s,
      tileColors = palettes[s.layout] || palettes[0];
    this.block(4, 3, -0.8, 10.44, 8.38, 0.38, "#485e60", 1.03);
    this.block(4, 3, -0.44, 10.66, 8.56, 0.12, "#a79871");
    this.block(4, 3, -0.32, 10.61, 8.51, 0.25, "#737d70");
    this.block(4, 3, -0.075, 10.62, 8.52, 0.07, "#a4a78b");
    this.ground(4, 3, 0, 4.53, 3.53, "#66776e");
    const jobs = [],
      add = (x, y, run) => jobs.push({ depth: this.depth(x, y), run });
    const hit = (poly, x, y) => this.hits.push({ poly, cell: [x, y] });
    const same = (q, x, y) => q && q[0] === x && q[1] === y;
    const highlight = new Set(this.targets.map((q) => q.join(",")));
    for (let y = 0; y < 7; y++)
      for (let x = 0; x < 9; x++) {
        const n = y * 9 + x,
          t = s.terrain[y][x],
          v = noise(n + 31 * s.layout);
        const col = tileColors[(x + y) % 4 === 0 ? 0 : n % 4 === 2 ? 3 : 1];
        const poly = this.ground(
          x,
          y,
          0.002,
          0.491,
          0.491,
          tint(t === 3 ? "#527645" : t === 2 ? "#a18e6b" : col, 0.93 + v * 0.12),
        );
        hit(poly, x, y);
        if (v > 0.41) {
          const a = this.project(x - 0.44 + v * 0.19, y + 0.15, 0.004),
            b = this.project(x - 0.1 + v * 0.19, y + 0.07, 0.004),
            e = this.project(x + 0.03, y + 0.19, 0.004);
          this.poly([a, b, e], null, "#7f84773a", 0.6);
        }
        if (t === 2) this.rocks(x, y, 0.01, 5, n);
        if (this.cover && s.cover && s.cover[n] !== "0") {
          this.ground(
            x,
            y,
            0.024,
            0.46,
            0.46,
            [null, "#d5ca654a", "#7ccbb581", "#65baca9c"][Number(s.cover[n])],
          );
        }
        if (this.reachable?.includes(n)) this.ellipse(x, y, 0.032, 0.08, 0.08, "#ccf2c2bb");
        if (this.route?.includes(n)) this.ground(x, y, 0.036, 0.42, 0.42, "#f0c66888", "#f0d894", 1.6);
        if (this.area?.has(n)) this.ground(x, y, 0.03, 0.46, 0.46, "#ed9b5f88", "#e7af71", 1);
        if (highlight.has(x + "," + y))
          this.ground(x, y, 0.037, 0.445, 0.445, "#5ce8cd2e", "#c0f0c3b0", 1.2);
        if (same(this.selected, x, y) || same(this.hover, x, y))
          this.ground(x, y, 0.045, 0.46, 0.46, "#f9dd8552", "#fff1bc", 2);
        if (t === 1) {
          const shadow = [
            [0.48, -0.47],
            [0.69, -0.2],
            [0.72, 0.64],
            [-0.36, 0.66],
            [-0.48, 0.47],
          ].map(([dx, dy]) => this.project(x + dx, y + dy, 0.02));
          add(x - 0.001, y - 0.002, () => this.poly(shadow, "#35453939"));
        }
        if (t === 1)
          add(x, y, () => {
            const height = this.wall(x, y, n);
            const corners = [
              [-0.48, -0.48],
              [0.48, -0.48],
              [0.48, 0.48],
              [-0.48, 0.48],
            ];
            for (let i = 0; i < 4; i++) {
              const a = corners[i],
                b = corners[(i + 1) % 4];
              hit(
                [
                  this.project(x + a[0], y + a[1], 0),
                  this.project(x + b[0], y + b[1], 0),
                  this.project(x + b[0], y + b[1], height),
                  this.project(x + a[0], y + a[1], height),
                ],
                x,
                y,
              );
            }
            hit(
              this.ground(
                x,
                y,
                height + 0.01,
                0.48,
                0.48,
                null,
                same(this.hover, x, y) ? "#fff1bc" : null,
                2,
              ),
              x,
              y,
            );
          });
      }
    for (let x = 0; x < 9; x++) {
      const y = this.cos > 0 ? 6.65 : -0.65;
      this.text(...this.project(x, y, 0.006), coord(x, 0)[0], "#344a45", Math.max(8, this.k * 0.19));
    }
    for (let y = 0; y < 7; y++) {
      const x = this.sin > 0 ? 8.68 : -0.68;
      this.text(...this.project(x, y, 0.006), String(y + 1), "#344a45", Math.max(8, this.k * 0.19));
    }
    for (const [x, y, size] of [
      [-0.84, -0.6, 1.35],
      [-0.8, 0.2, 0.84],
      [8.78, -0.58, 1.4],
      [8.79, 0.2, 0.94],
      [8.75, 6.45, 0.88],
      [0.28, 6.72, 0.73],
      [5.8, -0.73, 0.59],
      [-0.7, 6.5, 0.98],
    ]) {
      add(x, y, () => this.tree(x, y, 0, size, x * 6));
    }
    for (const [x, y] of [
      [1.2, -0.72],
      [7.1, 6.72],
      [-0.75, 2.12],
      [8.73, 4.22],
    ])
      add(x, y, () => this.torch(x, y, 0));
    add(-0.72, 4.9, () => this.standard(-0.72, 4.9, s.seat === 0 ? "#318d87" : "#bc6352", 1));
    add(8.7, 1.35, () => this.standard(8.7, 1.35, s.seat === 1 ? "#318d87" : "#bc6352", -1));
    for (const q of s.hazards.filter(Boolean)) add(...q, () => this.rocks(...q, 0.08, 6, 31, 0.6));
    for (const f of s.fields)
      add(f.x, f.y, () => {
        this.ellipse(f.x, f.y, (f.altitude || 0) + 0.06, 0.48, 0.48, "#c3a3ff99");
        this.solid(f.x, f.y, (f.altitude || 0) + 0.08, 0.12, 0.12, 0.45, 0.01, 0.01, "#c6abef", 5);
      });
    const visible = s.pieces.filter((p) => p.x >= 0 && p.y >= 0);
    const elapsed = performance.now() - this.started,
      t = Math.min(1, elapsed / 290);
    const position = (p) => {
      const m = this.motion?.get(p.target_id);
      return m && t < 1 ? [m.x + (p.x - m.x) * t, m.y + (p.y - m.y) * t] : [p.x, p.y];
    };
    for (const p of visible) {
      const [x, y] = position(p),
        z = p.altitude || 0;
      add(x, y + 0.04, () => {
        const stacked = visible.some((o) => o !== p && o.x === x && o.y === y && o.kind === "fighter");
        const offset = stacked && p.kind !== "fighter" ? 0.25 : 0;
        if (p.footprint > 1)
          this.ground(
            x + 0.5,
            y + 0.5,
            z + 0.02,
            0.94,
            0.94,
            null,
            sideColors[p.side === s.seat ? 0 : 1],
            2,
          );
        if (p.kind === "object")
          this.block(x + offset, y, z + 0.06, 0.25, 0.16, 0.13, p.burning ? "#f0985e" : "#b18c58");
        else this.hero(p, x + offset, y);
        const [px, py] = this.project(x + offset, y, z + 0.1),
          r = this.k * 0.32;
        hit(
          [
            [px - r, py + r / 2],
            [px - r, py - this.k * 0.96],
            [px + r, py - this.k * 0.96],
            [px + r, py + r / 2],
          ],
          p.x,
          p.y,
        );
      });
    }
    jobs.sort((a, b) => a.depth - b.depth).forEach((j) => j.run());
    // Floating pennants stay legible even when a wall obscures the miniature.
    for (const p of visible.filter((p) => p.kind !== "object")) {
      const [x, y] = this.project(
        ...position(p),
        (p.altitude || 0) + (p.kind === "fighter" ? 1.38 : 0.88),
      );
      const hidden = (p.kind === "fighter" && !p.visible) || p.detected;
      const tag =
        (hidden ? "? " : p.side === s.seat ? "◆ " : "◇ ") +
        (p.kind === "fighter" ? (p.side === s.seat ? "ВЫ" : "ИИ") : p.title) +
        (p.altitude ? " ↑" + 5 * p.altitude : "");
      const c = this.ctx;
      c.font = "bold 10px system-ui";
      const width = c.measureText(tag).width + 14;
      const rect = [
        [x - width / 2, y - 8],
        [x + width / 2, y - 8],
        [x + width / 2, y + 8],
        [x - width / 2, y + 8],
      ];
      this.poly(rect, "#20353bed", sideColors[p.side === s.seat ? 0 : 1], 1);
      this.text(x, y, tag, "#feefd5", 10);
      hit(rect, p.x, p.y);
      const damage = this.motion?.get(p.target_id)?.hp;
      if (damage && elapsed < 850)
        this.text(
          x,
          y - 16 - elapsed / 75,
          (damage > 0 ? "+" : "") + damage + " ОЗ",
          damage > 0 ? "#c4ffd3" : "#ffe0ac",
          15,
        );
    }
    if (this.motion?.size && elapsed < 850) {
      cancelAnimationFrame(this.frame);
      this.frame = requestAnimationFrame(() => this.draw());
    }
  }
}
