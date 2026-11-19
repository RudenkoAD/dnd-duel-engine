use std::sync::{
    atomic::{AtomicU8, Ordering},
    OnceLock,
};
use std::{fmt, fmt::Debug};

pub const WIDTH: i8 = 9;
pub const HEIGHT: i8 = 7;
pub const WALL_HEIGHT: i8 = 2;
pub const MAX_ELEVATION: i8 = 6;
const CELLS: usize = WIDTH as usize * HEIGHT as usize;
const POINTS: usize = CELLS * (MAX_ELEVATION as usize + 1);
const OPEN: u8 = 0;
const WALL: u8 = 1;
const DIFFICULT: u8 = 2;
const UNREACHABLE: u8 = u8::MAX;
const SIGHT_SAMPLES: [(i32, i32); 4] = [(1, 1), (3, 1), (1, 3), (3, 3)];
pub(super) const DIRECTIONS: [(i8, i8); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cover {
    None,
    Half,
    ThreeQuarters,
    Total,
}

impl Cover {
    /// Bonus to AC and Dexterity saves. Total cover prevents targeting.
    pub const fn bonus(self) -> i32 {
        match self {
            Self::Half => 2,
            Self::ThreeQuarters => 5,
            Self::None | Self::Total => 0,
        }
    }
}

// Each entry describes the western half and, optionally, the center column.
pub const LAYOUT_COUNT: usize = 5;
const BRIGHT_LAYOUT_COUNT: usize = 4;
pub const DARK_LAYOUT: usize = 4;

const LAYOUTS: [&[(i8, i8, u8)]; LAYOUT_COUNT] = [
    &[
        (2, 2, WALL),
        (2, 3, WALL),
        (2, 4, WALL),
        (3, 1, DIFFICULT),
        (3, 5, DIFFICULT),
    ],
    &[
        (2, 1, WALL),
        (3, 1, WALL),
        (3, 5, WALL),
        (4, 5, WALL),
        (2, 3, DIFFICULT),
        (3, 3, DIFFICULT),
        (4, 2, DIFFICULT),
    ],
    &[
        (2, 2, WALL),
        (3, 2, WALL),
        (3, 4, WALL),
        (4, 4, WALL),
        (1, 4, DIFFICULT),
        (2, 4, DIFFICULT),
        (4, 3, DIFFICULT),
    ],
    &[
        (2, 3, WALL),
        (2, 4, WALL),
        (4, 2, WALL),
        (4, 3, WALL),
        (1, 2, DIFFICULT),
        (3, 5, DIFFICULT),
        (4, 1, DIFFICULT),
    ],
    &[
        (2, 3, WALL),
        (2, 4, WALL),
        (4, 2, WALL),
        (4, 3, WALL),
        (1, 2, DIFFICULT),
        (3, 5, DIFFICULT),
        (4, 1, DIFFICULT),
    ],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub x: i8,
    pub y: i8,
}

/// A five-foot lattice position. Elevation is measured from the arena floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub cell: Pos,
    pub altitude: i8,
}

impl Point {
    pub const fn ground(cell: Pos) -> Self {
        Self { cell, altitude: 0 }
    }

    pub fn dist(self, other: Self) -> i32 {
        self.cell
            .dist(other.cell)
            .max(self.altitude.abs_diff(other.altitude) as i32)
    }
}

impl Pos {
    pub fn dist(self, other: Self) -> i32 {
        self.x.abs_diff(other.x).max(self.y.abs_diff(other.y)) as i32
    }

    fn index(self) -> Option<usize> {
        if (0..WIDTH).contains(&self.x) && (0..HEIGHT).contains(&self.y) {
            Some(self.y as usize * WIDTH as usize + self.x as usize)
        } else {
            None
        }
    }

    fn from_index(index: usize) -> Self {
        Self {
            x: (index % WIDTH as usize) as i8,
            y: (index / WIDTH as usize) as i8,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Map(&'static Layout, usize);

impl Debug for Map {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Map")
            .field("layout_id", &self.1)
            .finish()
    }
}

struct Layout {
    tiles: [u8; CELLS],
    walls: [u64; CELLS],
    paths: [[u8; CELLS]; CELLS],
    paths_to_los: [[u8; CELLS]; CELLS],
    visibility: [u64; CELLS],
    cover: [[Cover; CELLS]; CELLS],
    elevated_cover: Box<[AtomicU8]>,
    point_cover: Box<[AtomicU8]>,
}

impl Map {
    pub fn new(seed: u64) -> Self {
        Self::seeded(seed, BRIGHT_LAYOUT_COUNT)
    }

    pub(super) fn seeded_with_darkness(seed: u64) -> Self {
        Self::seeded(seed, LAYOUT_COUNT)
    }

    fn seeded(seed: u64, count: usize) -> Self {
        // The SplitMix64 finalizer mixes high seed bits into the layout index.
        let seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let id = (seed ^ (seed >> 31)) as usize % count;
        Self::from_layout(id)
    }

    pub fn from_layout(id: usize) -> Self {
        static MAPS: OnceLock<[Layout; LAYOUT_COUNT]> = OnceLock::new();
        let layouts = MAPS.get_or_init(|| LAYOUTS.map(Layout::new));
        Self(&layouts[id], id)
    }

    pub fn has_ambient_darkness(self) -> bool {
        self.1 == DARK_LAYOUT
    }

    pub fn layout_id(self) -> usize {
        self.1
    }

    pub fn spawn(seat: usize) -> Pos {
        Pos {
            x: [0, WIDTH - 1][seat],
            y: HEIGHT / 2,
        }
    }

    pub fn step(self, pos: Pos, dir: usize) -> Option<(Pos, i32)> {
        self.0.step(pos, dir).map(|(pos, cost)| (pos, cost as i32))
    }

    pub fn los(self, a: Pos, b: Pos) -> bool {
        match (a.index(), b.index()) {
            (Some(a), Some(b)) => self.0.visibility[a] & (1 << b) != 0,
            _ => false,
        }
    }

    pub fn cover(self, from: Pos, target: Pos) -> Cover {
        match (from.index(), target.index()) {
            (Some(from), Some(target)) => self.0.cover[from][target],
            _ => Cover::Total,
        }
    }

    pub fn cover_at(self, from: Point, target: Point) -> Cover {
        if from.altitude == 0 && target.altitude == 0 {
            return self.cover(from.cell, target.cell);
        }
        if !self.can_occupy(from) || !self.can_occupy(target) {
            return Cover::Total;
        }
        if from.altitude >= WALL_HEIGHT && target.altitude >= WALL_HEIGHT {
            return Cover::None;
        }
        cache_cover(&self.0.elevated_cover, from, target, || {
            self.0.trace_cover_at(from, target)
        })
    }

    pub fn los_at(self, from: Point, target: Point) -> bool {
        self.cover_at(from, target) != Cover::Total
    }

    /// Cover against an effect originating at the center of the selected point.
    pub fn cover_from_point(self, from: Point, target: Point) -> Cover {
        if !self.can_occupy(from) || !self.can_occupy(target) {
            return Cover::Total;
        }
        if from.altitude >= WALL_HEIGHT && target.altitude >= WALL_HEIGHT {
            return Cover::None;
        }
        cache_cover(&self.0.point_cover, from, target, || {
            let rays = SIGHT_SAMPLES
                .into_iter()
                .filter(|&sample| {
                    if from.altitude == 0 && target.altitude == 0 {
                        self.0.trace_ray(from.cell, (2, 2), target.cell, sample)
                    } else {
                        self.0.trace_voxel_ray(
                            sight_sample(from, (2, 2)),
                            sight_sample(target, sample),
                        )
                    }
                })
                .count();
            classify_cover([rays as i32; 4])
        })
    }

    /// The exact center of a proposed origin must itself be reachable.
    pub fn can_aim_point(self, caster: Point, center: Point) -> bool {
        self.cover_from_point(center, caster) != Cover::Total
    }

    /// Other creatures grant at most half cover. Multiple sources do not stack.
    pub fn cover_with_creatures(
        self,
        from: Point,
        target: Point,
        blockers: impl IntoIterator<Item = Point>,
    ) -> Cover {
        let wall_cover = self.cover_at(from, target);
        self.cover_including_creatures(from, target, wall_cover, &SIGHT_SAMPLES, blockers)
    }

    pub fn cover_from_point_with_creatures(
        self,
        from: Point,
        target: Point,
        blockers: impl IntoIterator<Item = Point>,
    ) -> Cover {
        let wall_cover = self.cover_from_point(from, target);
        self.cover_including_creatures(from, target, wall_cover, &[(2, 2)], blockers)
    }

    fn cover_including_creatures(
        self,
        from: Point,
        target: Point,
        wall_cover: Cover,
        origins: &[(i32, i32)],
        blockers: impl IntoIterator<Item = Point>,
    ) -> Cover {
        if wall_cover != Cover::None {
            return wall_cover;
        }
        for body in blockers {
            if body == from || body == target || !self.can_occupy(body) {
                continue;
            }
            let body = [body.cell.x, body.cell.y, body.altitude].map(|n| i32::from(n) * 4);
            let obscures = origins.iter().all(|&origin| {
                SIGHT_SAMPLES.into_iter().any(|end| {
                    body_intersects(sight_sample(from, origin), sight_sample(target, end), body)
                })
            });
            if obscures {
                return Cover::Half;
            }
        }
        Cover::None
    }

    pub fn can_occupy(self, point: Point) -> bool {
        point.cell.index().is_some()
            && (0..=MAX_ELEVATION).contains(&point.altitude)
            && (point.altitude >= WALL_HEIGHT || self.tile(point.cell) != WALL)
    }

    pub fn supported(self, point: Point) -> bool {
        self.can_occupy(point)
            && (point.altitude == 0
                || point.altitude == WALL_HEIGHT && self.tile(point.cell) == WALL)
    }

    pub fn landing(self, point: Point) -> Point {
        Point {
            cell: point.cell,
            altitude: if self.tile(point.cell) == WALL {
                WALL_HEIGHT
            } else {
                0
            },
        }
    }

    pub fn touches_wall(self, point: Point) -> bool {
        self.stone_contacts(point) & !(1 << 63) != 0
    }

    pub fn touches_structure(self, point: Point) -> bool {
        self.can_occupy(point)
            && (self.touches_wall(point)
                || point.cell.x == 0
                || point.cell.y == 0
                || point.cell.x == WIDTH - 1
                || point.cell.y == HEIGHT - 1
                || point.altitude == MAX_ELEVATION)
    }

    pub fn touches_stone(self, point: Point) -> bool {
        self.stone_contacts(point) != 0
    }

    pub fn shares_stone_surface(self, a: Point, b: Point) -> bool {
        self.stone_contacts(a) & self.stone_contacts(b) != 0
    }

    fn stone_contacts(self, point: Point) -> u64 {
        if !self.can_occupy(point) || point.altitude > WALL_HEIGHT {
            return 0;
        }
        let mut contacts = if point.altitude == 0 { 1 << 63 } else { 0 };
        let wall = |cell: Pos| cell.index().map_or(0, |i| self.0.walls[i]);
        if point.altitude == WALL_HEIGHT {
            contacts |= wall(point.cell);
        }
        for &(dx, dy) in DIRECTIONS.iter().step_by(2) {
            contacts |= wall(Pos {
                x: point.cell.x + dx,
                y: point.cell.y + dy,
            });
        }
        contacts
    }

    pub fn step_at(self, point: Point, dir: usize) -> Option<(Point, i32)> {
        if point.altitude == 0 {
            return self
                .step(point.cell, dir)
                .map(|(cell, cost)| (Point::ground(cell), cost));
        }
        let &(dx, dy) = DIRECTIONS.get(dir)?;
        if !self.can_occupy(point) {
            return None;
        }
        let next = |x, y| Point {
            cell: Pos { x, y },
            altitude: point.altitude,
        };
        let dest = next(point.cell.x + dx, point.cell.y + dy);
        if !self.can_occupy(dest)
            || (dx != 0
                && dy != 0
                && (!self.can_occupy(next(dest.cell.x, point.cell.y))
                    || !self.can_occupy(next(point.cell.x, dest.cell.y))))
        {
            return None;
        }
        Some((dest, 1))
    }

    pub fn path(self, a: Pos, b: Pos) -> i32 {
        match (a.index(), b.index()) {
            (Some(a), Some(b)) => self.0.paths[a][b] as i32,
            _ => UNREACHABLE as i32,
        }
    }

    /// Minimum movement cost to see the target from an unoccupied square.
    pub fn path_to_los(self, from: Pos, target: Pos) -> i32 {
        match (from.index(), target.index()) {
            (Some(from), Some(target)) => self.0.paths_to_los[from][target] as i32,
            _ => UNREACHABLE as i32,
        }
    }

    pub fn tile(self, pos: Pos) -> u8 {
        self.0.tile(pos)
    }
}

impl Layout {
    fn new(half: &[(i8, i8, u8)]) -> Self {
        let mut layout = Self {
            tiles: [OPEN; CELLS],
            walls: [0; CELLS],
            paths: [[UNREACHABLE; CELLS]; CELLS],
            paths_to_los: [[UNREACHABLE; CELLS]; CELLS],
            visibility: [0; CELLS],
            cover: [[Cover::Total; CELLS]; CELLS],
            elevated_cover: (0..POINTS * POINTS).map(|_| AtomicU8::new(0)).collect(),
            point_cover: (0..POINTS * POINTS).map(|_| AtomicU8::new(0)).collect(),
        };
        for &(x, y, tile) in half {
            for x in [x, WIDTH - 1 - x] {
                layout.tiles[Pos { x, y }.index().unwrap()] = tile;
            }
        }
        layout.label_walls();
        for from in 0..CELLS {
            let origin = Pos::from_index(from);
            if layout.tile(origin) == WALL {
                continue;
            }
            layout.paths[from][from] = 0;
            for dir in 0..DIRECTIONS.len() {
                if let Some((dest, cost)) = layout.step(origin, dir) {
                    layout.paths[from][dest.index().unwrap()] = cost;
                }
            }
            for to in from..CELLS {
                let (outbound, inbound) = layout.trace_cover(origin, Pos::from_index(to));
                layout.cover[from][to] = outbound;
                layout.cover[to][from] = inbound;
                if outbound != Cover::Total {
                    layout.visibility[from] |= 1 << to;
                    layout.visibility[to] |= 1 << from;
                }
            }
        }
        // Floyd-Warshall over the directed move costs, paid on entry.
        for via in 0..CELLS {
            for from in 0..CELLS {
                let first_leg = layout.paths[from][via];
                if first_leg == UNREACHABLE {
                    continue;
                }
                for to in 0..CELLS {
                    let route = first_leg.saturating_add(layout.paths[via][to]);
                    layout.paths[from][to] = layout.paths[from][to].min(route);
                }
            }
        }
        for target in 0..CELLS {
            let mut firing_squares = layout.visibility[target] & !(1 << target);
            while firing_squares != 0 {
                let square = firing_squares.trailing_zeros() as usize;
                firing_squares &= firing_squares - 1;
                for from in 0..CELLS {
                    let cost = layout.paths[from][square];
                    layout.paths_to_los[from][target] = layout.paths_to_los[from][target].min(cost);
                }
            }
        }
        layout
    }

    fn label_walls(&mut self) {
        for cell in 0..CELLS {
            if self.tiles[cell] != WALL || self.walls[cell] != 0 {
                continue;
            }
            let label = 1 << cell;
            let mut frontier = label;
            self.walls[cell] = label;
            while frontier != 0 {
                let edge = frontier.trailing_zeros() as usize;
                frontier &= frontier - 1;
                let pos = Pos::from_index(edge);
                for &(dx, dy) in DIRECTIONS.iter().step_by(2) {
                    let adjacent = Pos {
                        x: pos.x + dx,
                        y: pos.y + dy,
                    };
                    if let Some(index) = adjacent.index() {
                        if self.tiles[index] == WALL && self.walls[index] == 0 {
                            self.walls[index] = label;
                            frontier |= 1 << index;
                        }
                    }
                }
            }
        }
    }

    fn tile(&self, pos: Pos) -> u8 {
        pos.index().map_or(WALL, |i| self.tiles[i])
    }

    fn step(&self, pos: Pos, dir: usize) -> Option<(Pos, u8)> {
        let &(dx, dy) = DIRECTIONS.get(dir)?;
        if self.tile(pos) == WALL {
            return None;
        }

        let dest = Pos {
            x: pos.x + dx,
            y: pos.y + dy,
        };
        if dx != 0
            && dy != 0
            && (self.tile(Pos {
                x: dest.x,
                y: pos.y,
            }) == WALL
                || self.tile(Pos {
                    x: pos.x,
                    y: dest.y,
                }) == WALL)
        {
            return None;
        }
        match self.tile(dest) {
            WALL => None,
            DIFFICULT => Some((dest, 2)),
            _ => Some((dest, 1)),
        }
    }

    fn trace_cover(&self, from: Pos, target: Pos) -> (Cover, Cover) {
        if self.tile(from) == WALL || self.tile(target) == WALL {
            return (Cover::Total, Cover::Total);
        }
        let mut outbound = [0; SIGHT_SAMPLES.len()];
        let mut inbound = [0; SIGHT_SAMPLES.len()];
        for (a, origin_sample) in SIGHT_SAMPLES.into_iter().enumerate() {
            for (b, target_sample) in SIGHT_SAMPLES.into_iter().enumerate() {
                if self.trace_ray(from, origin_sample, target, target_sample) {
                    outbound[a] += 1;
                    inbound[b] += 1;
                }
            }
        }
        // Use the most open vantage point in the attacker's square. Rays run
        // between inset corners so they do not lie along a wall boundary.
        (classify_cover(outbound), classify_cover(inbound))
    }

    fn trace_cover_at(&self, from: Point, target: Point) -> Cover {
        let mut rays = [0; SIGHT_SAMPLES.len()];
        for (index, (ox, oy)) in SIGHT_SAMPLES.into_iter().enumerate() {
            for (tx, ty) in SIGHT_SAMPLES {
                if self
                    .trace_voxel_ray(sight_sample(from, (ox, oy)), sight_sample(target, (tx, ty)))
                {
                    rays[index] += 1;
                }
            }
        }
        classify_cover(rays)
    }

    /// Exact integer traversal of the voxels between two inset samples.
    /// At an edge or corner, a ray is open if at least one infinitesimal
    /// offset through the adjacent voxels reaches the next voxel.
    fn trace_voxel_ray(&self, from: [i32; 3], target: [i32; 3]) -> bool {
        const ORDERS: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let blocked = |[x, y, z]: [i32; 3]| {
            z < i32::from(WALL_HEIGHT)
                && self.tile(Pos {
                    x: x as i8,
                    y: y as i8,
                }) == WALL
        };
        let mut cell = from.map(|coordinate| coordinate / 4);
        let end = target.map(|coordinate| coordinate / 4);
        let distance = std::array::from_fn::<_, 3, _>(|axis| (target[axis] - from[axis]).abs());
        let step = std::array::from_fn::<_, 3, _>(|axis| (target[axis] - from[axis]).signum());
        let mut boundary = std::array::from_fn::<_, 3, _>(|axis| {
            let offset = from[axis] % 4;
            if step[axis] > 0 {
                4 - offset
            } else {
                offset
            }
        });
        while cell != end {
            let time_cmp =
                |a: usize, b: usize| (boundary[a] * distance[b]).cmp(&(boundary[b] * distance[a]));
            let first = (0..3)
                .filter(|&axis| cell[axis] != end[axis])
                .min_by(|&a, &b| time_cmp(a, b))
                .expect("nonempty difference");
            let crossing = std::array::from_fn::<_, 3, _>(|axis| {
                cell[axis] != end[axis] && time_cmp(first, axis).is_eq()
            });
            let clear = ORDERS.into_iter().any(|order| {
                let mut adjacent = cell;
                order
                    .into_iter()
                    .filter(|&axis| crossing[axis])
                    .all(|axis| {
                        adjacent[axis] += step[axis];
                        !blocked(adjacent)
                    })
            });
            if !clear {
                return false;
            }
            for axis in (0..3).filter(|&axis| crossing[axis]) {
                cell[axis] += step[axis];
                boundary[axis] += 4;
            }
        }
        true
    }

    fn trace_ray(
        &self,
        mut pos: Pos,
        (from_x, from_y): (i32, i32),
        target: Pos,
        (to_x, to_y): (i32, i32),
    ) -> bool {
        let dx = 4 * i32::from(target.x - pos.x) + to_x - from_x;
        let dy = 4 * i32::from(target.y - pos.y) + to_y - from_y;
        let (sx, sy) = (dx.signum() as i8, dy.signum() as i8);
        let (dx, dy) = (dx.abs(), dy.abs());
        let mut x_elapsed = if sx > 0 { 4 - from_x } else { from_x };
        let mut y_elapsed = if sy > 0 { 4 - from_y } else { from_y };

        while self.tile(pos) != WALL {
            if pos == target {
                return true;
            }

            let next_x = x_elapsed * dy;
            let next_y = y_elapsed * dx;
            if next_x == next_y
                && self.tile(Pos {
                    x: pos.x + sx,
                    y: pos.y,
                }) == WALL
                && self.tile(Pos {
                    x: pos.x,
                    y: pos.y + sy,
                }) == WALL
            {
                return false;
            }
            if next_x <= next_y {
                pos.x += sx;
                x_elapsed += 4;
            }
            if next_y <= next_x {
                pos.y += sy;
                y_elapsed += 4;
            }
        }
        false
    }
}

fn classify_cover(rays: [i32; 4]) -> Cover {
    match rays.into_iter().max().unwrap() {
        4 => Cover::None,
        2 | 3 => Cover::Half,
        1 => Cover::ThreeQuarters,
        _ => Cover::Total,
    }
}

fn cache_cover(
    cache: &[AtomicU8],
    from: Point,
    target: Point,
    calculate: impl FnOnce() -> Cover,
) -> Cover {
    let index = |point: Point| {
        point.altitude as usize * CELLS + point.cell.index().expect("occupiable point")
    };
    let cached = &cache[index(from) * POINTS + index(target)];
    match cached.load(Ordering::Relaxed) {
        1 => Cover::None,
        2 => Cover::Half,
        3 => Cover::ThreeQuarters,
        4 => Cover::Total,
        _ => {
            let cover = calculate();
            // Races compute the same immutable geometry.
            cached.store(cover as u8 + 1, Ordering::Relaxed);
            cover
        }
    }
}

fn sight_sample(point: Point, (x, y): (i32, i32)) -> [i32; 3] {
    [
        i32::from(point.cell.x) * 4 + x,
        i32::from(point.cell.y) * 4 + y,
        i32::from(point.altitude) * 4 + 2,
    ]
}

/// Clip an inset sight ray against the creature's open cell volume.
fn body_intersects(from: [i32; 3], end: [i32; 3], body: [i32; 3]) -> bool {
    let less = |(a, b): (i32, i32), (c, d): (i32, i32)| a * d < c * b;
    let (mut lower, mut upper) = ((0, 1), (1, 1));
    for axis in 0..3 {
        let change = end[axis] - from[axis];
        let low = body[axis] - from[axis];
        let high = low + 4;
        if change == 0 {
            if low >= 0 || high <= 0 {
                return false;
            }
            continue;
        }
        let (enter, exit) = if change > 0 {
            ((low, change), (high, change))
        } else {
            ((-high, -change), (-low, -change))
        };
        if less(lower, enter) {
            lower = enter;
        }
        if less(exit, upper) {
            upper = exit;
        }
        if !less(lower, upper) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: i8, y: i8) -> Pos {
        Pos { x, y }
    }

    fn reflected(p: Pos) -> Pos {
        pos(WIDTH - 1 - p.x, p.y)
    }

    #[test]
    fn seed_selects_mirrored_connected_layouts() {
        let first = Map::new(12);
        assert!(std::ptr::eq(first.0, Map::new(12).0));
        for layout in LAYOUTS {
            assert!((0..128).any(|seed| Map::new(seed).0.tiles == Layout::new(layout).tiles));
        }

        for layout in LAYOUTS.map(Layout::new) {
            let spawn = Map::spawn(0);
            assert_eq!(reflected(spawn), Map::spawn(1));
            assert_eq!(layout.tile(spawn), OPEN);
            assert!(layout.tiles.contains(&WALL));
            assert!(layout.tiles.contains(&DIFFICULT));
            for a in 0..CELLS {
                let origin = Pos::from_index(a);
                let mirror_a = reflected(origin).index().unwrap();
                assert_eq!(layout.tiles[a], layout.tiles[mirror_a]);
                for b in 0..CELLS {
                    let dest = Pos::from_index(b);
                    let mirror_b = reflected(dest).index().unwrap();
                    let path = layout.paths[a][b];
                    if layout.tile(origin) != WALL && layout.tile(dest) != WALL {
                        assert_ne!(path, UNREACHABLE);
                        assert!(i32::from(path) >= origin.dist(dest));
                    }
                    assert_eq!(path, layout.paths[mirror_a][mirror_b]);
                    let visible = layout.visibility[a] >> b & 1;
                    assert_eq!(visible, layout.visibility[mirror_a] >> mirror_b & 1);
                    assert_eq!(visible, layout.visibility[b] >> a & 1);
                    assert_eq!(visible != 0, layout.cover[b][a] != Cover::Total);
                    assert_eq!(layout.cover[a][b], layout.cover[mirror_a][mirror_b]);
                    assert_eq!(
                        layout.paths_to_los[a][b],
                        layout.paths_to_los[mirror_a][mirror_b]
                    );
                }
            }
        }
    }

    #[test]
    fn path_to_los_stops_at_first_reachable_firing_square() {
        let map = Map::new(0);
        let (west, east) = (Map::spawn(0), Map::spawn(1));
        assert!(!map.los(west, east));
        let map_cells = (0..CELLS).map(Pos::from_index);
        let expected = map_cells
            .clone()
            .filter(|&square| square != east && map.los(square, east))
            .map(|square| map.path(west, square))
            .min()
            .unwrap();
        assert_eq!(map.path_to_los(west, east), expected);
        assert!(map.path_to_los(west, east) < map.path(west, east));
        assert_eq!(map.path_to_los(west, pos(0, 4)), 0);
        assert_eq!(map.path_to_los(west, pos(2, 3)), UNREACHABLE as i32);
    }

    #[test]
    fn walls_block_movements_and_targeting() {
        let layout = Layout::new(&[(1, 1, WALL), (0, 2, WALL)]);
        assert_eq!(layout.step(pos(0, 1), 2), None);
        assert_eq!(layout.step(pos(0, 1), 1), None);
        assert_eq!(layout.step(pos(0, 1), 3), None);
        assert_eq!(layout.step(pos(0, 0), 0), None);
        assert_eq!(layout.trace_cover(pos(0, 1), pos(2, 1)).0, Cover::Total);
        assert_eq!(layout.trace_cover(pos(0, 1), pos(1, 2)).0, Cover::Total);
        assert_ne!(layout.trace_cover(pos(0, 1), pos(1, 0)).0, Cover::Total);
    }

    #[test]
    fn walls_offer_both_partial_cover_bonuses_and_total_cover() {
        let map = Map::new(0);
        let from = pos(0, 0);
        let cases = [
            (pos(8, 0), Cover::None, 0, true),
            (pos(8, 2), Cover::Half, 2, true),
            (pos(3, 2), Cover::ThreeQuarters, 5, true),
            (pos(2, 2), Cover::Total, 0, false),
        ];
        for (target, cover, bonus, visible) in cases {
            assert_eq!(map.cover(from, target), cover);
            assert_eq!(cover.bonus(), bonus);
            assert_eq!(map.los(from, target), visible);
            assert_eq!(map.cover(reflected(from), reflected(target)), cover);
        }
    }

    #[test]
    fn a_precise_spell_origin_cannot_use_an_open_shooting_corner() {
        let map = Map::new(0);
        let origin = Point::ground(pos(0, 0));
        let target = Point::ground(pos(4, 3));
        assert_eq!(map.cover_at(origin, target), Cover::ThreeQuarters);
        assert!(map.los_at(origin, target));
        assert_eq!(map.cover_from_point(origin, target), Cover::Total);
        assert!(!map.can_aim_point(target, origin));
    }

    #[test]
    fn a_ten_foot_wall_blocks_low_rays_and_supports_its_top() {
        let map = Map::new(0);
        let at = |x, altitude| Point {
            cell: pos(x, 3),
            altitude,
        };
        let ground = at(1, 0);
        assert_eq!(map.cover_at(ground, at(3, 1)), Cover::Total);
        assert_ne!(map.cover_at(ground, at(1, 4)), Cover::Total);
        assert_eq!(
            map.cover_at(at(1, WALL_HEIGHT), at(3, WALL_HEIGHT)),
            Cover::None
        );
        assert!(!map.can_occupy(at(2, 1)));
        assert!(!map.can_occupy(at(2, MAX_ELEVATION + 1)));
        assert!(map.can_occupy(at(2, MAX_ELEVATION)));
        assert!(map.touches_wall(ground));
        assert!(!map.touches_wall(at(1, MAX_ELEVATION)));
        assert_eq!(map.step_at(ground, 2), None);
        assert_eq!(
            map.step_at(at(1, WALL_HEIGHT), 2),
            Some((at(2, WALL_HEIGHT), 1))
        );
        assert!(map.supported(at(2, WALL_HEIGHT)));
        assert!(!map.supported(at(1, WALL_HEIGHT)));
        assert_eq!(map.landing(at(2, MAX_ELEVATION)), at(2, WALL_HEIGHT));
        assert_eq!(map.landing(at(1, MAX_ELEVATION)), ground);
    }

    #[test]
    fn creatures_offer_half_cover_without_stacking_and_only_at_intervening_height() {
        let map = Map::new(0);
        let at = |x, y, altitude| Point {
            cell: pos(x, y),
            altitude,
        };
        let caster = at(0, 0, 0);
        let target = at(8, 0, 0);
        assert_eq!(
            map.cover_with_creatures(caster, target, [at(4, 0, 0)]),
            Cover::Half
        );
        assert_eq!(
            map.cover_with_creatures(caster, target, [at(3, 0, 0), at(4, 0, 0)]),
            Cover::Half
        );
        assert_eq!(
            map.cover_with_creatures(caster, target, [at(4, 0, 2)]),
            Cover::None
        );
        assert_eq!(
            map.cover_with_creatures(caster, target, [caster, target]),
            Cover::None
        );

        let mostly_blocked = at(3, 2, 0);
        assert_eq!(
            map.cover_with_creatures(caster, mostly_blocked, [at(1, 1, 0)]),
            Cover::ThreeQuarters
        );
    }

    #[test]
    fn stone_contact_requires_the_floor_or_a_shared_wall_section() {
        let map = Map::new(0);
        let at = |x, y, altitude| Point {
            cell: pos(x, y),
            altitude,
        };
        let ground = at(0, 0, 0);
        assert!(map.touches_stone(ground));
        assert!(map.shares_stone_surface(ground, at(8, 6, 0)));
        assert!(!map.touches_stone(at(0, 0, 1)));
        assert!(!map.touches_stone(at(2, 3, 3)));
        assert!(map.shares_stone_surface(at(1, 2, 1), at(3, 4, 1)));
        assert!(map.shares_stone_surface(at(1, 2, 1), at(2, 3, WALL_HEIGHT)));
        assert!(!map.shares_stone_surface(at(1, 2, 1), at(5, 3, 1)));
    }

    #[test]
    fn difficult_terrain_costs_two_and_can_be_avoided() {
        let layout = Layout::new(&[(1, 1, DIFFICULT)]);
        let start = pos(0, 1);
        let difficult = pos(1, 1);
        let end = pos(2, 1);
        assert_eq!(layout.step(start, 2), Some((difficult, 2)));
        assert_eq!(layout.step(difficult, 6), Some((start, 1)));
        assert_eq!(
            layout.paths[start.index().unwrap()][end.index().unwrap()],
            2
        );
        assert_eq!(layout.trace_cover(start, end).0, Cover::None);
    }
}
