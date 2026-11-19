//! Ground targeting over legal squares in the arena's line-of-sight model.
use super::battle::Battle;
use super::class::Class;
use super::geometry::{Point, Pos, DIRECTIONS, HEIGHT, MAX_ELEVATION, WIDTH};
use super::Act;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Burst,
    LandAid,
    Moonbeam,
    Caltrops,
    SpiritualWeapon,
    Guess,
    OtherArea,
    Teleport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetSpec {
    pub kind: TargetKind,
    pub range: i32,
    pub radius: i32,
}

/// Spell volumes on the five-foot lattice. A square is affected when its
/// center falls within the volume and has a clear path to its origin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Sphere {
        radius: i32,
    },
    Cylinder {
        radius: i32,
        height: i32,
    },
    Cube {
        half_side: i32,
    },
    GroundSquare {
        half_side: i32,
    },
    Cone {
        length: i32,
        direction: usize,
    },
    ForwardCube {
        side: i32,
        direction: usize,
    },
    Line {
        length: i32,
        width: i32,
        direction: usize,
    },
    OffsetCube {
        side: i32,
        direction: usize,
        quarters: [i8; 2],
    },
}

pub const DIRECTION_NAMES: [&str; 26] = [
    "North",
    "Northeast",
    "East",
    "Southeast",
    "South",
    "Southwest",
    "West",
    "Northwest",
    "Up",
    "Down",
    "North and up",
    "Northeast and up",
    "East and up",
    "Southeast and up",
    "South and up",
    "Southwest and up",
    "West and up",
    "Northwest and up",
    "North and down",
    "Northeast and down",
    "East and down",
    "Southeast and down",
    "South and down",
    "Southwest and down",
    "West and down",
    "Northwest and down",
];

pub fn aim_vector(direction: usize) -> Option<[i32; 3]> {
    let (index, z) = match direction {
        0..=7 => (direction, 0),
        8 => return Some([0, 0, 1]),
        9 => return Some([0, 0, -1]),
        10..=17 => (direction - 10, 1),
        18..=25 => (direction - 18, -1),
        _ => return None,
    };
    let (x, y) = DIRECTIONS[index];
    Some([i32::from(x), i32::from(y), z])
}

fn dot(a: [i32; 3], b: [i32; 3]) -> i32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Area {
    pub fn contains(self, origin: Pos, target: Pos) -> bool {
        self.contains_at(Point::ground(origin), Point::ground(target))
    }

    pub fn contains_at(self, origin: Point, target: Point) -> bool {
        self.contains_delta(point_delta(origin, target), 1)
    }

    /// Tests a centered object's bounding box at quarter-square precision.
    /// Convex spell volumes contain the box iff they contain all its corners.
    pub fn contains_entire_object(self, origin: Point, target: Point, extent: [i32; 3]) -> bool {
        let [x, y, z] = point_delta(origin, target).map(|delta| 4 * delta);
        [-extent[0], extent[0]].into_iter().all(|dx| {
            [-extent[1], extent[1]].into_iter().all(|dy| {
                [-extent[2], extent[2]]
                    .into_iter()
                    .all(|dz| self.contains_delta([x + dx, y + dy, z + dz], 4))
            })
        })
    }

    fn contains_delta(self, delta: [i32; 3], scale: i32) -> bool {
        let [x, y, z] = delta;
        match self {
            Self::Sphere { radius } => dot(delta, delta) <= (radius * scale).pow(2),
            Self::Cylinder { radius, height } => {
                (0..=height * scale).contains(&z) && x * x + y * y <= (radius * scale).pow(2)
            }
            Self::Cube { half_side } => x.abs().max(y.abs()).max(z.abs()) <= half_side * scale,
            Self::GroundSquare { half_side } => z == 0 && x.abs().max(y.abs()) <= half_side * scale,
            Self::Cone { length, direction } => {
                let Some(axis) = aim_vector(direction) else {
                    return false;
                };
                let axial = dot(delta, axis);
                let axis_squared = dot(axis, axis);
                let lateral_squared = dot(delta, delta) * axis_squared - axial * axial;
                axial > 0
                    && axial * axial <= (length * scale).pow(2) * axis_squared
                    && 4 * lateral_squared <= axial * axial
            }
            Self::Line { length, width, direction } => {
                let Some(axis) = aim_vector(direction) else { return false };
                let axial = dot(delta, axis);
                let axis_squared = dot(axis, axis);
                let lateral_squared = dot(delta, delta) * axis_squared - axial * axial;
                axial > 0
                    && axial * axial <= (length * scale).pow(2) * axis_squared
                    && 4 * lateral_squared <= (width * scale).pow(2) * axis_squared
            }
            Self::ForwardCube { side, direction } => {
                forward_cube(delta, scale, side, direction, [0, 0])
            }
            Self::OffsetCube {
                side,
                direction,
                quarters,
            } => forward_cube(delta, scale, side, direction, quarters),
        }
    }
}

pub(super) fn cube_face_offset(index: usize) -> [i8; 2] {
    const OFFSETS: [i8; 5] = [-6, -4, 0, 4, 6];
    let position = match index {
        0 => 12,
        1..=12 => index - 1,
        _ => index,
    };
    [OFFSETS[position % 5], OFFSETS[position / 5]]
}

fn forward_cube(delta: [i32; 3], scale: i32, side: i32, direction: usize, offset: [i8; 2]) -> bool {
    let Some(axis) = aim_vector(direction) else {
        return false;
    };
    let [dx, dy, dz] = axis;
    let axial = dot(delta, axis);
    if axial <= 0 || axial * axial > (side * scale).pow(2) * dot(axis, axis) {
        return false;
    }
    // An apex on the rear face may lie at its center or offset along either
    // transverse axis. Orient right and up relative to the direction of cast.
    let (right, up) = if dx == 0 && dy == 0 {
        ([1, 0, 0], [0, 1, 0])
    } else {
        ([-dy, dx, 0], [-dx * dz, -dy * dz, dx * dx + dy * dy])
    };
    let spans = |basis, offset: i8| {
        let projection = f64::from(dot(delta, basis)) / f64::from(dot(basis, basis)).sqrt();
        let shifted = projection - f64::from(i32::from(offset) * scale) / 4.0;
        2.0 * shifted.abs() <= f64::from(side * scale)
    };
    spans(right, offset[0]) && spans(up, offset[1])
}

fn point_delta(origin: Point, target: Point) -> [i32; 3] {
    [
        i32::from(target.cell.x) - i32::from(origin.cell.x),
        i32::from(target.cell.y) - i32::from(origin.cell.y),
        i32::from(target.altitude) - i32::from(origin.altitude),
    ]
}

impl Battle {
    /// Tests the footprint and the line from its point of origin. Partial
    /// cover does not exclude a square, but may affect an ensuing DEX save.
    pub fn area_hits(&self, origin: Pos, creature: usize, area: Area) -> bool {
        self.area_hits_at(Point::ground(origin), self.fighter_point(creature), area)
    }

    pub fn area_hits_at(&self, origin: Point, target: Point, area: Area) -> bool {
        area.contains_at(origin, target)
            && self.map.cover_from_point(origin, target) != super::geometry::Cover::Total
    }
    /// Ranges and footprints are in five-foot cells. Action availability (slots,
    /// class level, and so forth) is checked by the main battle mask.
    pub fn target_spec(&self, action: Act) -> Option<TargetSpec> {
        use Act::*;
        use Class::*;
        use TargetKind::*;

        use super::magic::Spell;
        if self.f[self.owner].build.variant != 0 {
            if let Some(spell) = self.prepared_spell(self.owner, action) {
                let (kind, range, radius) = match spell {
                    Spell::PlantGrowth => (OtherArea, 30, 20),
                    Spell::FaerieFire => (Burst, 12, 2),
                    Spell::CalmEmotions => (Burst, 12, 4),
                    Spell::Sleep => (Burst, 12, 1),
                    Spell::HungerOfHadar | Spell::Fireball => (Burst, 30, 4),
                    Spell::Shatter => (Burst, 12, 2),
                    Spell::HypnoticPattern => (Burst, 24, 3),
                    Spell::Moonbeam => (Moonbeam, 24, 1),
                    Spell::SpiritualWeapon => (SpiritualWeapon, 12, 1),
                    Spell::Entangle => (OtherArea, 18, 2),
                    Spell::FogCloud => (OtherArea, 24, 4),
                    Spell::Alarm if !self.alarm_cast_in_progress(self.owner) => (OtherArea, 6, 2),
                    Spell::StinkingCloud => (OtherArea, 18, 4),
                    Spell::ZoneOfTruth => (OtherArea, 12, 3),
                    Spell::MistyStep => (Teleport, 6, 0),
                    Spell::Elementalism => (OtherArea, 6, 0),
                    _ => return None,
                };
                return Some(TargetSpec {kind, range, radius});
            }
        }
        let (kind, range, radius) = match (self.f[self.owner].build.class, action) {
            (Bard, Spell3) | (Wizard, Magic5) | (Sorcerer, Magic13) => (Burst, 12, 2), // Shatter
            (Bard, Spell4) => (Burst, 24, 3), // Hypnotic Pattern
            (Druid | Sorcerer | Warlock | Wizard, Spell4) | (Sorcerer, Special2) => {
                (Burst, 30, 4) // Fireball / Heightened Fireball
            }
            (Druid, Special3) => (LandAid, 12, 2),
            (Druid, Spell3) => (Moonbeam, 24, 1),
            (Druid | Paladin, Special5) => (Moonbeam, 12, 1),
            (Cleric, Spell3) => (SpiritualWeapon, 12, 1),
            (Cleric, Special3) => (SpiritualWeapon, 4, 1),
            (Rogue, Special4 | Utilize) => (Caltrops, 1, 0),
            (Warlock, Magic14) => (OtherArea, 18, 4), // Stinking Cloud
            (Paladin, Magic17) => (OtherArea, 12, 3), // Zone of Truth
            (Druid | Ranger, Magic2) => (OtherArea, 18, 2), // Entangle
            (Wizard | Warlock, Magic6) => (Teleport, 6, 0), // Misty Step
            (_, Melee | Unarmed | Ranged) | (Barbarian, Special2) | (Monk, Special1 | Special2)
                if self.observed_pos(self.owner, 1 - self.owner).is_none() =>
            {
                let reach = match (self.f[self.owner].build.class, action) {
                    (_, Ranged) => 24,
                    (_, Unarmed) | (Monk, Special1 | Special2) => self.unarmed_reach(self.owner),
                    _ => 1,
                };
                (Guess, reach, 0)
            }
            _ => return None,
        };
        Some(TargetSpec {
            kind,
            range,
            radius,
        })
    }

    pub fn target_uses_altitude(&self, action: Act) -> bool {
        self.target_spec(action)
            .is_some_and(|spec| !self.floor_only(action, spec))
    }

    fn is_relocation(&self, action: Act) -> bool {
        matches!(
            (self.f[self.owner].build.class, action),
            (Class::Druid | Class::Paladin, Act::Special5) | (Class::Cleric, Act::Special3)
        )
    }

    fn floor_only(&self, action: Act, spec: TargetSpec) -> bool {
        if self.f[self.owner].build.variant != 0 {
            if let Some(spell) = self.prepared_spell(self.owner, action) {
                return spell == super::magic::Spell::Entangle;
            }
        }
        spec.kind == TargetKind::Caltrops
            || matches!(
                (self.f[self.owner].build.class, action),
                (Class::Druid | Class::Ranger, Act::Magic2)
            )
    }

    fn targeting_origin(&self, action: Act) -> Point {
        if self.prepared_spell(self.owner, action).is_some() {
            self.divine_spell_origin(self.owner)
        } else {
            self.fighter_point(self.owner)
        }
    }

    fn valid_target_point(&self, action: Act, spec: TargetSpec, point: Point) -> bool {
        let caster = self.targeting_origin(action);
        if self.is_relocation(action) {
            return self
                .magic_field(self.owner)
                .is_some_and(|field| field.dist(point) <= spec.range)
                && self.map.can_occupy(point);
        }
        caster.dist(point) <= spec.range
            && (spec.kind != TargetKind::Guess || caster != point)
            && (!self.floor_only(action, spec) || point.altitude == 0)
            && self.map.can_aim_point(caster, point)
            && (spec.kind != TargetKind::Caltrops
                || ![0, 1]
                    .into_iter()
                    .any(|seat| self.caltrops(seat) == Some(point.cell)))
            && (spec.kind != TargetKind::Teleport || self.publicly_unoccupied(point))
    }

    /// Check whether the action has at least one legal point. In the main
    /// phase, most areas have the caster's square as a legal origin.
    pub fn target_available(&self, action: Act) -> bool {
        let Some(spec) = self.target_spec(action) else {
            return false;
        };
        if self.is_relocation(action) {
            return self
                .magic_field(self.owner)
                .is_some_and(|point| self.map.can_occupy(point));
        }
        let caster = self.targeting_origin(action);
        if self.floor_only(action, spec) && i32::from(caster.altitude) > spec.range {
            return false;
        }
        self.target_points(action, spec, caster.cell)
            .next()
            .is_some()
            || self.target_mask(action, None) != 0
    }

    fn publicly_unoccupied(&self, point: Point) -> bool {
        !self.magic_cloud_blocks(self.fighter_point(self.owner), point)
            && (0..4).all(|id| {
                self.observed_footprint(self.owner, id)
                    .is_none_or(|body| !body.squares().any(|square| square == point))
            })
    }

    fn target_points(
        &self,
        action: Act,
        spec: TargetSpec,
        cell: Pos,
    ) -> impl Iterator<Item = Point> + '_ {
        let height = if self.floor_only(action, spec) {
            0
        } else {
            MAX_ELEVATION
        };
        (0..=height)
            .map(move |altitude| Point { cell, altitude })
            .filter(move |&point| self.valid_target_point(action, spec, point))
    }

    /// `None` selects a row (bits 0..=6). A selected row enables columns (bits 0..=8).
    pub fn target_mask(&self, action: Act, row: Option<i8>) -> u128 {
        let Some(spec) = self.target_spec(action) else {
            return 0;
        };
        let valid = |x, y| {
            self.target_points(action, spec, Pos { x, y })
                .next()
                .is_some()
        };
        match row {
            Some(y) if (0..HEIGHT).contains(&y) => (0..WIDTH)
                .filter(|&x| valid(x, y))
                .fold(0, |mask, x| mask | 1 << x),
            Some(_) => 0,
            None => (0..HEIGHT)
                .filter(|&y| (0..WIDTH).any(|x| valid(x, y)))
                .fold(0, |mask, y| mask | 1 << y),
        }
    }

    pub fn target_altitude_mask(&self, action: Act, cell: Pos) -> u128 {
        let Some(spec) = self.target_spec(action) else {
            return 0;
        };
        self.target_points(action, spec, cell)
            .fold(0, |mask, point| mask | 1 << point.altitude)
    }

    /// A solid wall gives total cover from the point of origin.
    pub fn target_hits(&self, center: Pos, creature: usize, radius: i32) -> bool {
        let target = self.f[creature].pos;
        center.dist(target) <= radius && self.map.los(center, target)
    }

    /// Shape of an aimable effect. Direction is used only for effects
    /// originating at the caster and aimed along one of the eight axes.
    pub fn target_area(&self, action: Act, _direction: usize) -> Option<Area> {
        let spec = self.target_spec(action)?;
        let class = self.f[self.owner].build.class;
        if self.f[self.owner].build.variant != 0 {
            if let Some(spell) = self.prepared_spell(self.owner, action) {
                use super::magic::Spell;
                return Some(match spell {
                    Spell::Elementalism | Spell::FaerieFire | Spell::HypnoticPattern | Spell::Alarm => Area::Cube { half_side: spec.radius },
                    Spell::Entangle => Area::GroundSquare { half_side: spec.radius },
                    Spell::Moonbeam => Area::Cylinder { radius: 1, height: 8 },
                    _ => Area::Sphere { radius: spec.radius },
                });
            }
        }
        Some(match (class, action) {
            (Class::Bard, Act::Spell4) | (Class::Cleric, Act::Magic15) => Area::Cube {
                half_side: spec.radius,
            },
            (Class::Druid | Class::Ranger, Act::Magic2) => Area::GroundSquare {
                half_side: spec.radius,
            },
            (_, _) if spec.kind == TargetKind::Moonbeam => Area::Cylinder {
                radius: spec.radius,
                height: 8,
            },
            _ => Area::Sphere {
                radius: spec.radius,
            },
        })
    }

    /// Features for an axis choice: legal, normalized coordinate, can affect
    /// foe, can affect self, can affect only foe, can affect both, minimum
    /// center distance to foe and self divided by the board's maximum distance.
    /// Early choices aggregate over all subsequently legal coordinates.
    /// Land's Aid heals self.
    pub fn target_choice_features(&self, row: Option<i8>, action: Act, choice: usize) -> [f32; 8] {
        let Some(spec) = self.target_spec(action) else {
            return [0.0; 8];
        };
        let (y, columns, axis_max) = match row {
            None if choice < HEIGHT as usize => (choice as i8, 0..WIDTH, HEIGHT - 1),
            Some(y) if (0..HEIGHT).contains(&y) && choice < WIDTH as usize => {
                let x = choice as i8;
                (y, x..x + 1, WIDTH - 1)
            }
            _ => return [0.0; 8],
        };
        let points = columns.flat_map(|x| self.target_points(action, spec, Pos { x, y }));
        self.target_features(action, spec, choice as f32 / f32::from(axis_max), points)
    }

    pub fn target_altitude_features(&self, action: Act, cell: Pos, choice: usize) -> [f32; 8] {
        let Some(spec) = self.target_spec(action) else {
            return [0.0; 8];
        };
        if choice > MAX_ELEVATION as usize {
            return [0.0; 8];
        }
        let point = Point {
            cell,
            altitude: choice as i8,
        };
        let points = self
            .valid_target_point(action, spec, point)
            .then_some(point);
        self.target_features(
            action,
            spec,
            choice as f32 / f32::from(MAX_ELEVATION),
            points.into_iter(),
        )
    }

    fn target_features(
        &self,
        action: Act,
        spec: TargetSpec,
        axis: f32,
        points: impl Iterator<Item = Point>,
    ) -> [f32; 8] {
        let mut values: [f32; 8] = [0.0; 8];
        let mut dist_foe = WIDTH as i32;
        let mut dist_self = WIDTH as i32;
        let area = self.target_area(action, 0).expect("point area");
        let foe_seat = 1 - self.owner;
        let foe = self
            .observed_pos(self.owner, foe_seat)
            .map(|_| self.fighter_point(foe_seat));
        let me = self.fighter_point(self.owner);
        for point in points {
            let hits_foe = foe.is_some_and(|foe| self.area_hits_at(point, foe, area));
            let hits_self = !matches!(
                spec.kind,
                TargetKind::SpiritualWeapon | TargetKind::Guess | TargetKind::Teleport
            ) && !matches!(area, Area::GroundSquare { .. })
                && self.area_hits_at(point, me, area);
            values[0] = 1.0;
            values[2] = values[2].max(u8::from(hits_foe).into());
            values[3] = values[3].max(u8::from(hits_self).into());
            values[4] = values[4].max(u8::from(hits_foe && !hits_self).into());
            values[5] = values[5].max(u8::from(hits_foe && hits_self).into());
            if let Some(foe) = foe {
                dist_foe = dist_foe.min(point.dist(foe));
            }
            dist_self = dist_self.min(point.dist(me));
        }
        if values[0] > 0.0 {
            values[1] = axis;
            values[6] = u8::from(foe.is_some()) as f32 * dist_foe as f32 / f32::from(WIDTH - 1);
            values[7] = dist_self as f32 / f32::from(WIDTH - 1);
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::super::ACTS;
    use super::*;

    fn battle(class: Class, from: Pos, foe: Pos) -> Battle {
        let mut battle = Battle::new(0, [class, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = from;
        battle.f[1].pos = foe;
        battle
    }

    #[test]
    fn ground_cast_can_choose_empty_visible_cell_when_foe_has_total_cover() {
        let battle = battle(Class::Wizard, Pos { x: 0, y: 3 }, Pos { x: 4, y: 3 });
        let fireball = Act::Spell4;
        let columns = battle.target_mask(fireball, Some(3));
        assert_eq!(columns & 0b11, 0b11);
        assert_eq!(
            battle.target_altitude_mask(fireball, Pos { x: 3, y: 3 }) & 1,
            0
        );
        let wall_column = battle.target_altitude_mask(fireball, Pos { x: 2, y: 3 });
        assert_eq!(wall_column & 0b11, 0);
        assert_ne!(wall_column & (1 << MAX_ELEVATION), 0);
        assert_ne!(battle.target_mask(fireball, None) & (1 << 3), 0);
        assert_eq!(battle.target_mask(fireball, Some(-1)), 0);
        assert_eq!(battle.target_mask(Act::Spell1, None), 0);
        assert!(!battle.target_hits(Pos { x: 1, y: 3 }, 1, 4));
        assert!(battle.target_hits(Pos { x: 4, y: 0 }, 1, 4));
        assert!(!battle.target_hits(Pos { x: 4, y: 0 }, 1, 2));
    }

    #[test]
    fn row_features_distinguish_both_from_foe_only() {
        let battle = battle(Class::Druid, Pos { x: 0, y: 0 }, Pos { x: 4, y: 0 });
        let land_aid = Act::Special3;
        let row = battle.target_choice_features(None, land_aid, 0);
        assert_eq!(row, [1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        let empty_center = battle.target_choice_features(Some(0), land_aid, 2);
        assert_eq!(empty_center, [1.0, 0.25, 1.0, 1.0, 0.0, 1.0, 0.25, 0.25]);
        let foe_only = battle.target_choice_features(Some(0), land_aid, 3);
        assert_eq!(foe_only, [1.0, 0.375, 1.0, 0.0, 1.0, 0.0, 0.125, 0.375]);
    }

    #[test]
    fn caltrops_are_restricted_to_self_and_reachable_adjacent_squares() {
        let battle = battle(Class::Rogue, Pos { x: 0, y: 3 }, Pos { x: 4, y: 3 });
        assert_eq!(battle.target_mask(Act::Special4, None), 0b11100);
        assert_eq!(battle.target_mask(Act::Special4, Some(3)), 0b11);
        assert_eq!(battle.target_mask(Act::Special4, Some(5)), 0);
        assert_eq!(
            battle.target_choice_features(Some(3), Act::Special4, 2),
            [0.0; 8]
        );
    }

    #[test]
    fn spheres_and_cylinders_do_not_include_their_bounding_box_corners() {
        let origin = Pos { x: 4, y: 3 };
        for area in [
            Area::Sphere { radius: 2 },
            Area::Cylinder {
                radius: 2,
                height: 8,
            },
        ] {
            assert!(area.contains(origin, Pos { x: 6, y: 3 }));
            assert!(!area.contains(origin, Pos { x: 6, y: 5 }));
        }
        assert!(Area::Cube { half_side: 2 }.contains(origin, Pos { x: 6, y: 5 }));
    }

    #[test]
    fn directional_cone_and_forward_cube_have_distinct_footprints() {
        let origin = Pos { x: 4, y: 3 };
        let cone = Area::Cone {
            length: 3,
            direction: 2,
        };
        let cube = Area::ForwardCube {
            side: 3,
            direction: 2,
        };
        let narrow_end = Pos { x: 5, y: 4 };
        assert!(!cone.contains(origin, narrow_end));
        assert!(cube.contains(origin, narrow_end));
        assert!(cone.contains(origin, Pos { x: 7, y: 4 }));
        assert!(!cone.contains(origin, Pos { x: 8, y: 3 }));
        assert!(!cube.contains(origin, Pos { x: 3, y: 3 }));
        assert!(!cone.contains(origin, origin));

        let diagonal = Area::Cone {
            length: 3,
            direction: 7,
        };
        assert!(diagonal.contains(origin, Pos { x: 2, y: 1 }));
        assert!(!diagonal.contains(origin, Pos { x: 1, y: 0 }));
    }

    #[test]
    fn elevation_changes_membership_and_sloped_cones_can_aim_up() {
        let p = |x, altitude| Point {
            cell: Pos { x, y: 3 },
            altitude,
        };
        let origin = p(4, 0);
        let above = p(4, 3);
        let above_and_east = p(6, 2);
        assert!(!Area::Sphere { radius: 2 }.contains_at(origin, above));
        assert!(Area::Cylinder {
            radius: 2,
            height: 8
        }
        .contains_at(origin, above));
        assert!(Area::Cone {
            length: 3,
            direction: 8
        }
        .contains_at(origin, above));
        assert!(!Area::Cone {
            length: 3,
            direction: 2
        }
        .contains_at(origin, above));
        assert!(Area::Cone {
            length: 3,
            direction: 12
        }
        .contains_at(origin, above_and_east));
        assert!(!Area::Cone {
            length: 3,
            direction: 20
        }
        .contains_at(origin, above_and_east));
        assert!(Area::ForwardCube {
            side: 3,
            direction: 8
        }
        .contains_at(origin, above));
        assert!(!Area::ForwardCube {
            side: 3,
            direction: 2
        }
        .contains_at(origin, above));
        assert_eq!(
            DIRECTION_NAMES.len(),
            (0..32).filter(|&i| aim_vector(i).is_some()).count()
        );
    }

    #[test]
    fn aiming_masks_and_features_do_not_reveal_a_hidden_foe() {
        let mut battle = battle(Class::Wizard, Pos { x: 0, y: 0 }, Pos { x: 4, y: 4 });
        battle.f[1].hidden_dc = Some(20);
        for action in [Act::Spell4, Act::Melee, Act::Ranged] {
            let cell = Pos { x: 1, y: 0 };
            let snapshot = |b: &Battle| {
                (
                    b.target_mask(action, None),
                    b.target_mask(action, Some(0)),
                    b.target_altitude_mask(action, cell),
                    b.target_choice_features(Some(0), action, 1),
                    b.target_altitude_features(action, cell, 0),
                )
            };
            let before = snapshot(&battle);
            battle.f[1].pos = Pos { x: 1, y: 0 };
            assert_eq!(before, snapshot(&battle));
            assert_eq!(before.3[2], 0.0);
            assert_eq!(before.3[6], 0.0);
            battle.f[1].pos = Pos { x: 4, y: 4 };
        }
    }

    #[test]
    fn movable_field_range_is_measured_from_its_existing_origin() {
        let mut b = battle(Class::Cleric, Pos { x: 0, y: 0 }, Pos { x: 8, y: 6 });
        b.step(Act::Gear2).unwrap();
        b.step(Act::Spell3).unwrap();
        assert_eq!(b.phase_id(), 5);
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[0]).unwrap();
        if b.phase_id() == 38 {
            b.step(ACTS[b.mask().trailing_zeros() as usize]).unwrap();
        }
        assert_eq!(b.magic_field(0), Some(Point::ground(Pos { x: 0, y: 0 })));
        b.f[0].pos = Pos { x: 8, y: 0 };
        assert!(b.target_available(Act::Special3));
        assert_ne!(
            b.target_altitude_mask(Act::Special3, Pos { x: 4, y: 0 }) & 1,
            0
        );
        assert_eq!(b.target_altitude_mask(Act::Special3, Pos { x: 5, y: 0 }), 0);
        assert_ne!(
            b.target_altitude_mask(Act::Special3, Pos { x: 2, y: 3 }) & (1 << 2),
            0
        );
        assert_eq!(
            b.target_altitude_mask(Act::Special3, Pos { x: 2, y: 3 }) & 0b11,
            0
        );
    }

    #[test]
    fn entangle_is_grounded_and_misty_step_hides_unobserved_occupancy() {
        let ground = Pos { x: 0, y: 0 };
        let enemy = Pos { x: 1, y: 0 };
        let druid = battle(Class::Druid, ground, enemy);
        assert!(!druid.target_uses_altitude(Act::Magic2));
        assert_eq!(druid.target_altitude_mask(Act::Magic2, ground), 1);
        let square = druid.target_area(Act::Magic2, 0).unwrap();
        assert!(square.contains(ground, enemy));
        assert!(!square.contains_at(
            Point::ground(ground),
            Point {
                cell: enemy,
                altitude: 1
            }
        ));

        let mut wizard = battle(Class::Wizard, ground, enemy);
        assert!(wizard.target_uses_altitude(Act::Magic6));
        assert_eq!(wizard.target_altitude_mask(Act::Magic6, enemy) & 1, 0);
        wizard.f[1].hidden_dc = Some(20);
        assert_ne!(wizard.target_altitude_mask(Act::Magic6, enemy) & 1, 0);
        assert_eq!(wizard.target_altitude_mask(Act::Magic6, ground) & 1, 0);
    }

    #[test]
    fn three_quarters_cover_changes_dexterity_saves_but_not_constitution_saves() {
        let origin = Point::ground(Pos { x: 0, y: 0 });
        let mut covered = battle(Class::Wizard, origin.cell, Pos { x: 3, y: 2 });
        assert_eq!(
            covered
                .map
                .cover_from_point(origin, covered.fighter_point(1))
                .bonus(),
            5
        );
        let mut exposed = covered.clone();
        exposed.f[1].pos = Pos { x: 3, y: 0 };
        let dexterity = 1;
        let constitution = 2;
        let die = exposed.clone().roll(20);
        let dex_dc = die + exposed.f[1].build.stats[dexterity] + 5;
        let con_dc = die + exposed.f[1].build.stats[constitution] + exposed.f[1].build.pb + 5;
        assert!(!exposed
            .clone()
            .save_from_point(1, dexterity, dex_dc, false, origin));
        assert!(covered
            .clone()
            .save_from_point(1, dexterity, dex_dc, false, origin));
        assert!(!covered.save_from_point(1, constitution, con_dc, false, origin));
        assert!(!exposed.save_from_point(1, constitution, con_dc, false, origin));
    }

    #[test]
    fn three_quarters_cover_can_defeat_the_same_attack_roll() {
        let covered = Pos { x: 3, y: 2 };
        let exposed = Pos { x: 3, y: 0 };
        let mut cover_saved = false;
        for seed in 0..40 {
            let mut original = Battle::new(seed, [Class::Wizard, Class::Fighter], [1, 1]);
            original.map = super::super::geometry::Map::new(0);
            original.owner = 0;
            original.f[0].pos = Pos { x: 0, y: 0 };
            let damage = |mut battle: Battle, position| {
                battle.f[1].pos = position;
                let hp = battle.f[1].hp;
                battle.step(Act::Spell5).expect("Fire Bolt");
                if battle.phase_id() == 80 {
                    battle.step(ACTS[1]).expect("creature choice");
                }
                hp - battle.f[1].hp
            };
            let no_cover = damage(original.clone(), exposed);
            let with_cover = damage(original, covered);
            assert!(with_cover <= no_cover);
            cover_saved |= with_cover < no_cover;
        }
        assert!(cover_saved);
    }
}
