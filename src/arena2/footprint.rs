//! A creature's occupied squares, including Large bodies and squeezing.
use super::battle::Battle;
use super::geometry::{Point, Pos};
use super::{Act, ACTS};

const LARGE: u8 = 3;
pub(super) const TRANSFORM_FOOTPRINT: i32 = 103;
const EXPANSIONS: [(i8, i8); 4] = [(0, 0), (-1, 0), (0, -1), (-1, -1)];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Footprint {
    pub anchor: Point,
    pub edge: i8,
}

impl Footprint {
    pub fn point(anchor: Point) -> Self {
        Self { anchor, edge: 1 }
    }

    pub fn squares(self) -> impl Iterator<Item = Point> {
        (0..self.edge).flat_map(move |dy| {
            (0..self.edge).map(move |dx| Point {
                cell: Pos {
                    x: self.anchor.cell.x + dx,
                    y: self.anchor.cell.y + dy,
                },
                altitude: self.anchor.altitude,
            })
        })
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.anchor.altitude == other.anchor.altitude
            && self.anchor.cell.x < other.anchor.cell.x + other.edge
            && other.anchor.cell.x < self.anchor.cell.x + self.edge
            && self.anchor.cell.y < other.anchor.cell.y + other.edge
            && other.anchor.cell.y < self.anchor.cell.y + self.edge
    }

    pub fn distance(self, other: Self) -> i32 {
        self.squares()
            .flat_map(|left| other.squares().map(move |right| left.dist(right)))
            .min()
            .expect("a footprint includes its anchor")
    }
}

impl Battle {
    pub fn footprint(&self, id: usize) -> Option<Footprint> {
        let anchor = self.creature_point(id)?;
        let large = self.combatant_size(id) >= LARGE;
        let edge = if large && !self.footprint_squeezed[id] {
            2
        } else {
            1
        };
        Some(Footprint { anchor, edge })
    }

    pub fn observed_footprint(&self, viewer: usize, id: usize) -> Option<Footprint> {
        if id >= 2 {
            return self
                .observed_entity(viewer, id)
                .and_then(|_| self.footprint(id));
        }
        let cell = self.observed_pos(viewer, id)?;
        if viewer == id || self.can_see(viewer, id) || self.familiar_sight(viewer, id) {
            self.footprint(id)
        } else {
            Some(Footprint::point(Point {
                cell,
                altitude: self.fighter_point(id).altitude,
            }))
        }
    }

    pub fn combatant_distance(&self, left: usize, right: usize) -> Option<i32> {
        self.footprint(left)
            .zip(self.footprint(right))
            .map(|(a, b)| a.distance(b))
    }

    pub(super) fn leaves_melee_reach(
        &self,
        mover: usize,
        destination: Point,
        reactor: usize,
    ) -> bool {
        let Some((before, enemy)) = self.footprint(mover).zip(self.footprint(reactor)) else {
            return false;
        };
        let size = self.combatant_size(mover);
        let Some(squeeze) = self.footprint_fit_kind(mover, destination, size, false, true) else {
            return false;
        };
        let after = Footprint {
            anchor: destination,
            edge: if size >= LARGE && !squeeze { 2 } else { 1 },
        };
        before.distance(enemy) <= 1 && after.distance(enemy) > 1
    }

    pub fn combatant_los(&self, left: usize, right: usize) -> bool {
        if !self.rope_same_space(left, right) {
            return false;
        }
        self.footprint(left)
            .zip(self.footprint(right))
            .is_some_and(|(a, b)| {
                a.squares()
                    .any(|from| b.squares().any(|to| self.map.los_at(from, to)))
            })
    }

    pub fn is_squeezing(&self, id: usize) -> bool {
        id < 4 && self.combatant_size(id) >= LARGE && self.footprint_squeezed[id]
    }

    fn footprint_fits(
        &self,
        id: usize,
        shape: Footprint,
        public: bool,
        linked_rider: bool,
    ) -> bool {
        let side = id % 2;
        shape.squares().all(|point| self.map.can_occupy(point))
            && (0..4).all(|other| {
                if other == id || !self.rope_same_space(id, other) || linked_rider && other % 2 == side && self.entities.mounted[side] {
                    return true;
                }
                let existing = if public {
                    self.observed_footprint(side, other)
                } else {
                    self.footprint(other)
                };
                existing.is_none_or(|existing| !shape.overlaps(existing))
            })
    }

    /// Returns whether a legal placement has to use a squeezed footprint.
    pub(super) fn footprint_fit_kind(
        &self,
        id: usize,
        destination: Point,
        size: u8,
        public: bool,
        linked_rider: bool,
    ) -> Option<bool> {
        let full = Footprint {
            anchor: destination,
            edge: if size >= LARGE { 2 } else { 1 },
        };
        if self.footprint_fits(id, full, public, linked_rider) {
            Some(false)
        } else if size >= LARGE
            && self.footprint_fits(id, Footprint::point(destination), public, linked_rider)
        {
            Some(true)
        } else {
            None
        }
    }

    /// Plans voluntary movement from the mover's perspective. The actual fit
    /// is checked again at commitment against the complete board.
    pub(super) fn footprint_movement(
        &self,
        id: usize,
        destination: Point,
        base_cost: i32,
    ) -> Option<(i32, bool)> {
        let size = self.combatant_size(id);
        let squeeze = self.footprint_fit_kind(id, destination, size, true, true)?;
        let shape = Footprint {
            anchor: destination,
            edge: if size >= LARGE && !squeeze { 2 } else { 1 },
        };
        let difficult = destination.altitude == 0
            && self.map.tile(destination.cell) != 2
            && shape.squares().any(|point| self.map.tile(point.cell) == 2);
        let squeeze_at_corner = self.footprint(id).is_some_and(|origin| {
            if origin.edge < 2
                || shape.edge < 2
                || origin.anchor.altitude != destination.altitude
                || origin.anchor.cell.x == destination.cell.x
                || origin.anchor.cell.y == destination.cell.y
            {
                return false;
            }
            [
                Pos {
                    x: origin.anchor.cell.x,
                    y: destination.cell.y,
                },
                Pos {
                    x: destination.cell.x,
                    y: origin.anchor.cell.y,
                },
            ]
            .into_iter()
            .any(|cell| {
                !self.footprint_fits(
                    id,
                    Footprint {
                        anchor: Point {
                            cell,
                            altitude: destination.altitude,
                        },
                        edge: 2,
                    },
                    true,
                    true,
                )
            })
        });
        // Difficult terrain and squeezing each add one foot per foot moved.
        let cost = base_cost
            + i32::from(difficult)
            + i32::from(squeeze || squeeze_at_corner || self.is_squeezing(id));
        Some((cost, squeeze))
    }

    /// Updates the footprint shape for an actual destination. Position and
    /// hazard callbacks remain the movement/action caller's responsibility.
    pub(super) fn commit_footprint(
        &mut self,
        id: usize,
        destination: Point,
        strict_placement: bool,
    ) -> bool {
        if self.rope_space(id).is_some() && self.creature_point(id) != Some(destination) {
            return false;
        }
        let size = self.combatant_size(id);
        let Some(squeeze) =
            self.footprint_fit_kind(id, destination, size, false, !strict_placement)
        else {
            return false;
        };
        self.footprint_squeezed[id] = squeeze;
        true
    }

    fn transform_anchor(&self, actor: usize, choice: Act) -> Option<Point> {
        let (dx, dy) = *EXPANSIONS.get(choice as usize)?;
        let mut anchor = self.fighter_point(actor);
        anchor.cell.x += dx;
        anchor.cell.y += dy;
        let footprint = Footprint { anchor, edge: 2 };
        self.footprint_fits(actor, footprint, true, false)
            .then_some(anchor)
    }

    pub(super) fn footprint_transform(&mut self, actor: usize) {
        self.footprint_squeezed[actor] = false;
        if self.combatant_size(actor) < LARGE {
            return;
        }
        // Preserve the square occupied before transformation until the
        // controller chooses one of the full-size placements around it.
        self.footprint_squeezed[actor] = true;
        let mask = self.footprint_transform_mask(actor);
        if mask.count_ones() > 1 {
            self.request(actor, TRANSFORM_FOOTPRINT);
        } else if mask != 0 {
            self.footprint_transform_step(actor, ACTS[mask.trailing_zeros() as usize]);
        }
    }

    pub(super) fn footprint_transform_mask(&self, actor: usize) -> u128 {
        ACTS[..4]
            .iter()
            .copied()
            .filter(|&choice| self.transform_anchor(actor, choice).is_some())
            .fold(0, |mask, choice| mask | choice.bit())
    }

    pub(super) fn footprint_transform_step(&mut self, actor: usize, choice: Act) {
        let anchor = self
            .transform_anchor(actor, choice)
            .expect("verified footprint choice");
        let footprint = Footprint { anchor, edge: 2 };
        if self.footprint_fits(actor, footprint, false, false) {
            self.footprint_squeezed[actor] = false;
            self.f[actor].pos = anchor.cell;
        }
    }

    pub(super) fn footprint_transform_label(&self, actor: usize, choice: Act) -> Option<String> {
        let anchor = self.transform_anchor(actor, choice)?;
        Some(format!(
            "Occupy {}{}–{}{}",
            char::from(b'A' + anchor.cell.x as u8),
            anchor.cell.y + 1,
            char::from(b'A' + anchor.cell.x as u8 + 1),
            anchor.cell.y + 2
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{
        class::Class,
        entities::{Entity, EntityKind, Spirit},
    };

    #[test]
    fn large_transformation_keeps_old_square_and_offers_distinct_reanchors() {
        let mut duel = Battle::new(0, [Class::Druid, Class::Fighter], [5, 5]);
        duel.owner = 0;
        let original = Point::ground(Pos { x: 4, y: 0 });
        duel.f[0].pos = original.cell;
        duel.f[1].pos = Pos { x: 6, y: 0 };
        duel.step(Act::Special1).unwrap();
        duel.step(ACTS[2]).unwrap(); // Riding Horse
        assert_eq!(duel.phase_id(), 100);
        duel.step(ACTS[0]).unwrap(); // Merge equipment
        assert_eq!(duel.phase_id(), TRANSFORM_FOOTPRINT);
        assert!(duel.mask().count_ones() > 1);
        duel.step(ACTS[0]).unwrap();
        let body = duel.footprint(0).unwrap();
        assert_eq!(body.squares().count(), 4);
        assert!(body.squares().any(|square| square == original));
        assert_eq!(duel.combatant_distance(0, 1), Some(1));
        assert!(!duel.is_squeezing(0));
    }

    #[test]
    fn large_steed_squeezes_at_an_edge_and_pays_additional_movement() {
        let mut duel = Battle::new(0, [Class::Paladin, Class::Fighter], [5, 5]);
        duel.f[0].pos = Pos { x: 0, y: 0 };
        duel.f[1].pos = Pos { x: 0, y: 6 };
        let position = Pos { x: 8, y: 0 };
        duel.fixture_companion(
            0,
            Entity::new(
                EntityKind::Steed {
                    spirit: Spirit::Celestial,
                    spell_level: 2,
                },
                0,
                position,
                1,
            ),
        );
        let body = duel.footprint(2).unwrap();
        assert_eq!(body.edge, 1);
        assert!(duel.is_squeezing(2));
        let when_still_squeezed = Point::ground(Pos { x: 8, y: 1 });
        assert_eq!(
            duel.footprint_movement(2, when_still_squeezed, 1),
            Some((2, true))
        );
    }
}
