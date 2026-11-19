//! Conditions applied to companions by martial features.
use super::Battle;
use crate::arena2::{
    equipment::DamageType,
    geometry::{Point, DIRECTIONS},
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EntityMartial {
    pub prone: bool,
    pub addled: bool,
    pub caltrops_stopped: bool,
    pub grappled: Option<(usize, i32)>,
    stunned_until: i32,
    slowed_until: i32,
    stun_slow_until: i32,
    exposed_until: i32,
    sapped_until: i32,
}

impl EntityMartial {
    pub fn stunned(&self, turn: i32) -> bool {
        turn < self.stunned_until
    }

    pub fn slowed(&self, turn: i32) -> bool {
        turn < self.slowed_until
    }

    pub fn stun_slowed(&self, turn: i32) -> bool {
        turn < self.stun_slow_until
    }

    pub fn take_exposed(&mut self, turn: i32) -> bool {
        turn < std::mem::take(&mut self.exposed_until)
    }

    pub fn take_sap(&mut self, turn: i32) -> bool {
        turn < std::mem::take(&mut self.sapped_until)
    }
}

impl Battle {
    pub(in crate::arena2) fn entity_enter_grapple(
        &mut self,
        target: usize,
        source: usize,
        dc: i32,
    ) {
        if let Some(entity) = self.entity_mut(target) {
            entity.martial.grappled = Some((source, dc));
            self.martial_entity_fall(target);
        }
    }

    pub(in crate::arena2) fn entity_release_grapple(&mut self, target: usize) {
        if let Some(entity) = self.entity_mut(target) {
            entity.martial.grappled = None;
        }
    }

    pub(in crate::arena2) fn martial_entity_grapple_count(&self, source: usize) -> usize {
        (2..4)
            .filter(|&target| {
                self.entity(target).is_some_and(|entity| {
                    entity.martial.grappled.is_some_and(|(by, _)| by == source)
                })
            })
            .count()
    }

    pub(in crate::arena2) fn martial_break_entity_grapples(&mut self) {
        for target in 2..4 {
            let Some(entity) = self.entity(target) else {
                continue;
            };
            let Some((source, _)) = entity.martial.grappled else {
                continue;
            };
            if self.f[source].hp == 0
                || self.incap(source)
                || self
                    .combatant_distance(source, target)
                    .is_none_or(|distance| distance > self.unarmed_reach(source))
            {
                self.entity_release_grapple(target);
            }
        }
    }

    pub(in crate::arena2) fn martial_drag_grappled_entity(
        &mut self,
        source: usize,
        previous: Point,
    ) {
        for target in 2..4 {
            let Some(entity) = self.entity(target) else {
                continue;
            };
            if entity.martial.grappled.is_some_and(|(by, _)| by == source)
                && self
                    .combatant_distance(source, target)
                    .is_none_or(|distance| distance > 1)
            {
                self.entity_force_move(target, previous);
            }
        }
    }

    fn source_next_turn(&self, source: usize) -> i32 {
        self.turns + if source == self.owner { 2 } else { 1 }
    }

    pub(in crate::arena2) fn entity_sap(&mut self, source: usize, target: usize) {
        let until = self.source_next_turn(source);
        if let Some(entity) = self.entity_mut(target) {
            entity.martial.sapped_until = until;
        }
    }

    pub(in crate::arena2) fn entity_slow(&mut self, source: usize, target: usize) {
        let until = self.source_next_turn(source);
        if let Some(entity) = self.entity_mut(target) {
            entity.martial.slowed_until = until;
        }
    }

    pub(in crate::arena2) fn entity_addle(&mut self, target: usize) {
        if let Some(entity) = self.entity_mut(target) {
            entity.martial.addled = true;
        }
    }

    pub(in crate::arena2) fn entity_knock_prone(&mut self, target: usize) {
        if let Some(entity) = self.entity_mut(target) {
            entity.martial.prone = true;
            self.martial_entity_fall(target);
        }
    }

    pub(in crate::arena2) fn martial_entity_fall(&mut self, id: usize) {
        let Some(point) = self.entity(id).map(|entity| entity.point()) else {
            return;
        };
        if self.map.supported(point) {
            return;
        }
        let direct = self.map.landing(point);
        let nearby: Vec<_> = (0..8)
            .filter_map(|dir| {
                self.map
                    .step_at(point, dir)
                    .map(|(adjacent, _)| self.map.landing(adjacent))
            })
            .collect();
        let Some(landing) = std::iter::once(direct).chain(nearby).find(|&candidate| {
            candidate.altitude < point.altitude && self.entity_force_move(id, candidate)
        }) else {
            return;
        };
        let dice = (i32::from(point.altitude - landing.altitude) / 2).min(20);
        if dice > 0 {
            let damage = (0..dice).map(|_| self.roll(6)).sum::<i32>();
            self.entity_damage_typed(id, damage, DamageType::Bludgeoning);
            if let Some(entity) = self.entity_mut(id) {
                entity.martial.prone = true;
            }
        }
    }

    pub(in crate::arena2) fn martial_stun_entity(&mut self, source: usize, target: usize, dc: i32) {
        let until = self.source_next_turn(source);
        let saved = self.martial_save(target, 2, dc);
        if let Some(entity) = self.entity_mut(target) {
            if saved {
                entity.martial.stun_slow_until = until;
                entity.martial.exposed_until = until;
            } else {
                entity.martial.stunned_until = until;
                self.martial_entity_fall(target);
            }
        }
    }

    pub(in crate::arena2) fn entity_forced_push(
        &mut self,
        source: usize,
        target: usize,
        distance: i32,
    ) {
        let Some(from) = self.martial_pos(source) else {
            return;
        };
        let Some(entity) = self.entity(target) else {
            return;
        };
        let toward = entity.position;
        let direction = ((toward.x - from.x).signum(), (toward.y - from.y).signum());
        let Some(dir) = DIRECTIONS.iter().position(|&value| value == direction) else {
            return;
        };
        let mut point = Point {
            cell: toward,
            altitude: entity.altitude,
        };
        for _ in 0..distance {
            let Some((next, _)) = self.map.step_at(point, dir) else {
                break;
            };
            if !self.entity_force_move(target, next) {
                break;
            }
            point = next;
        }
    }
}
