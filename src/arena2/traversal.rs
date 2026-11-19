//! Ground, climbing, flying and jump movement on the five-foot lattice.
use super::battle::Battle;
use super::class::Class;
use super::common::Skill;
use super::equipment::DamageType;
use super::geometry::{Point, DIRECTIONS, MAX_ELEVATION};
use super::targeting::DIRECTION_NAMES;
use super::Act;

const LONG_JUMP: i32 = 13;
const HIGH_JUMP: i32 = 16;
const CLIMB: i32 = 17;
const SLOW_FALL: i32 = 18;
const JUMP_RANKS: usize = 6;
const RUNUP: u8 = 2;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TraversalState {
    runup: u8,
    climbing: bool,
    spell_jump: bool,
    fall_damage: Option<i32>,
    pending: Option<PendingTraversal>,
}

#[derive(Clone, Copy, Debug)]
struct Movement {
    to: Point,
    cost: i32,
    assisted: bool,
}

#[derive(Clone, Copy, Debug)]
struct PendingTraversal {
    movement: Movement,
    hold_wall: bool,
    jump_landing: bool,
}

impl Battle {
    pub(super) fn traversal_begin_turn(&mut self, i: usize) {
        self.traversal[i].runup = 0;
        self.traversal[i].spell_jump = false;
    }

    pub(super) fn traversal_break_runup(&mut self, i: usize) {
        self.traversal[i].runup = 0;
    }

    pub(super) fn traversal_after_step(
        &mut self,
        i: usize,
        from: Point,
        to: Point,
        ordinary: bool,
    ) {
        let on_foot = ordinary
            && !self.entities.mounted[i]
            && !self.f[i].prone
            && from.altitude == 0
            && to.altitude == 0
            && from.cell != to.cell;
        let contact = self.climbing_contact(i, to);
        let state = &mut self.traversal[i];
        state.runup = if on_foot {
            state.runup.saturating_add(1).min(RUNUP)
        } else {
            0
        };
        if ordinary && state.climbing {
            // A climber can sidle along its wall. Walking off a ledge is a fall.
            state.climbing = !self.map.supported(from) && contact && !self.map.supported(to);
        }
        if !ordinary {
            let Some(plan) = state.pending.take() else {
                return;
            };
            state.spell_jump |= plan.movement.assisted;
            state.climbing = plan.hold_wall && contact && !self.map.supported(to);
            if plan.jump_landing && self.is_difficult(to) {
                let (bonus, advantage) = self.martial_skill_mod(i, Skill::Acrobatics);
                if self.d20_for(i, advantage, false) + bonus < 10 {
                    self.knock_prone(i);
                }
            }
        }
    }

    pub(super) fn traversal_fly_speed(&self, i: usize) -> i32 {
        let f = &self.f[i];
        if self.speed(i) == 0 || self.incap(i) || f.prone {
            return 0;
        }
        let mut speed = self.magic_fly_speed(i);
        if f.slow {
            speed -= 2;
        }
        if f.stun_slow {
            speed /= 2;
        }
        speed.max(0)
    }

    fn climbing_contact(&self, i: usize, point: Point) -> bool {
        if self.shape_spider_climb(i) {
            self.map.touches_structure(point)
        } else {
            self.map.touches_wall(point)
        }
    }

    pub(super) fn climb_speed(&self, i: usize) -> i32 {
        if self.is_subclass(i, Class::Rogue, 0) && self.f[i].build.level >= 3 {
            self.speed(i)
        } else {
            self.magic_climb_speed(i)
        }
    }

    fn climbing_extra(&self, i: usize, point: Point) -> i32 {
        if self.climb_speed(i) > 0 {
            0
        } else if self.is_difficult(point) {
            2
        } else {
            1
        }
    }

    fn is_difficult(&self, point: Point) -> bool {
        (point.altitude == 0 && self.map.tile(point.cell) == 2) || self.magic_difficult(point)
    }

    /// Extra movement cost for a horizontal movement already accepted by the
    /// terrain map. An elevated step off a ledge is legal and triggers falling.
    pub(super) fn traversal_walk_candidate(&self, i: usize, next: Point) -> Option<i32> {
        let from = self.fighter_point(i);
        if self.traversal_fly_speed(i) > 0 || self.map.supported(from) {
            return Some(0);
        }
        if self.traversal[i].climbing && self.climbing_contact(i, next) {
            return Some(self.climbing_extra(i, next));
        }
        None
    }

    fn movement_ready(&self, i: usize) -> bool {
        self.speed(i) > 0 && !self.f[i].prone && !self.incap(i) && !self.entities.mounted[i]
    }

    fn landing_is_known_clear(&self, i: usize, at: Point) -> bool {
        (self.observed_pos(i, 1 - i).is_none() || at != self.fighter_point(1 - i))
            && (2..4).all(|id| {
                self.observed_entity(i, id)
                    .is_none_or(|ally| ally.point() != at)
            })
    }

    fn has_air_space(&self, i: usize, at: Point) -> bool {
        self.map.can_occupy(at)
            && self.landing_is_known_clear(i, at)
            && self.entity_fear_allows_move(i, at.cell)
    }

    fn climb_candidate(&self, i: usize, choice: usize) -> Option<Movement> {
        let ordinary_budget = self.speed(i).max(self.climb_speed(i)) + self.dash - self.moved;
        let budget = ordinary_budget.max(self.tactical_steps);
        self.climb_with_budget(i, choice, budget)
    }

    fn climb_with_budget(&self, i: usize, choice: usize, budget: i32) -> Option<Movement> {
        let from = self.fighter_point(i);
        if !self.movement_ready(i) || !self.climbing_contact(i, from) {
            return None;
        }
        let to = match choice {
            0 => Point {
                altitude: from.altitude + 1,
                ..from
            },
            1 => Point {
                altitude: from.altitude - 1,
                ..from
            },
            2..=9 if from.altitude > 0 => {
                let (edge, _) = self.map.step_at(from, choice - 2)?;
                Point {
                    altitude: edge.altitude - 1,
                    ..edge
                }
            }
            _ => return None,
        };
        if !self.has_air_space(i, to) || !(self.climbing_contact(i, to) || self.map.supported(to)) {
            return None;
        }
        let terrain = if self.is_difficult(to) { 2 } else { 1 };
        let cost = terrain + self.climbing_extra(i, to);
        (cost <= budget).then_some(Movement {
            to,
            cost,
            assisted: false,
        })
    }

    fn flight_candidate(&self, i: usize, direction: i8) -> Option<Movement> {
        let ordinary_budget = self.traversal_fly_speed(i) + self.dash - self.moved;
        let budget = ordinary_budget.max(self.tactical_steps);
        self.flight_with_budget(i, direction, budget)
    }

    fn flight_with_budget(&self, i: usize, direction: i8, budget: i32) -> Option<Movement> {
        let fly_speed = self.traversal_fly_speed(i);
        let from = self.fighter_point(i);
        let to = Point {
            altitude: from.altitude + direction,
            ..from
        };
        (self.movement_ready(i) && fly_speed > 0 && budget > 0 && self.has_air_space(i, to))
            .then_some(Movement {
                to,
                cost: 1,
                assisted: false,
            })
    }

    pub(in crate::arena2) fn traversal_safe_vertical(
        &self,
        i: usize,
        choice: usize,
        budget: i32,
    ) -> Option<(Point, i32, bool)> {
        let (movement, climbing) = match choice {
            0..=9 => (self.climb_with_budget(i, choice, budget), true),
            10 => (self.flight_with_budget(i, 1, budget), false),
            11 => (self.flight_with_budget(i, -1, budget), false),
            _ => return None,
        };
        let movement = movement?;
        Some((movement.to, movement.cost, climbing))
    }

    pub(in crate::arena2) fn traversal_safe_finish(
        &mut self,
        i: usize,
        from: Point,
        to: Point,
        vertical: Option<bool>,
    ) {
        if let Some(climbing) = vertical {
            self.traversal_break_runup(i);
            self.traversal[i].climbing =
                climbing && self.climbing_contact(i, to) && !self.map.supported(to);
        } else {
            self.traversal_after_step(i, from, to, true);
        }
    }

    fn jump_ability(&self, i: usize) -> (i32, i32) {
        let build = self.f[i].build;
        let stat = if build.is_subclass(Class::Rogue, 0) && build.level >= 3 {
            1
        } else {
            0
        };
        let score = self.body_score(i, stat);
        (score, (score - 10).div_euclid(2))
    }

    fn jump_distance(&self, i: usize, high: bool) -> usize {
        let (score, modifier) = self.jump_ability(i);
        let feet = if high { (3 + modifier).max(0) } else { score };
        let running = self.traversal[i].runup >= RUNUP;
        let multiplier = self.martial_jump_multiplier(i);
        let feet = feet * multiplier / if running { 1 } else { 2 };
        let extra = if self.divine_athlete(i) { 10 } else { 0 };
        ((feet + extra) / 5) as usize
    }

    fn jump_candidate(&self, i: usize, high: bool, choice: usize) -> Option<Movement> {
        let ordinary_budget = self.speed(i) + self.dash - self.moved;
        let budget = ordinary_budget.max(self.tactical_steps);
        self.jump_with_budget(i, high, choice, budget)
    }

    fn jump_with_budget(
        &self,
        i: usize,
        high: bool,
        choice: usize,
        budget: i32,
    ) -> Option<Movement> {
        if !self.movement_ready(i) || self.f[1 - i].grappled.is_some() {
            return None;
        }
        let from = self.fighter_point(i);
        if !self.map.supported(from) && !self.traversal[i].climbing {
            return None;
        }
        let variants = if high {
            JUMP_RANKS
        } else {
            DIRECTIONS.len() * JUMP_RANKS
        };
        let assisted = choice >= variants;
        if choice >= 2 * variants
            || assisted && (!self.magic_jump_active(i) || self.traversal[i].spell_jump)
        {
            return None;
        }
        let choice = choice % variants;
        let distance = choice % JUMP_RANKS + 1;
        if !assisted && distance > self.jump_distance(i, high) {
            return None;
        }
        let cost = if assisted { 2 } else { distance as i32 };
        if cost > budget {
            return None;
        }
        let mut to = from;
        if high {
            to.altitude += distance as i8;
        } else {
            let direction = choice / JUMP_RANKS;
            for _ in 0..distance {
                to = self.map.step_at(to, direction)?.0;
            }
        }
        self.has_air_space(i, to)
            .then_some(Movement { to, cost, assisted })
    }

    pub(in crate::arena2) fn traversal_safe_jump(
        &self,
        i: usize,
        high: bool,
        choice: usize,
        budget: i32,
    ) -> Option<(Point, i32, bool)> {
        let jump = self.jump_with_budget(i, high, choice, budget)?;
        Some((jump.to, jump.cost, jump.assisted))
    }

    pub(in crate::arena2) fn traversal_safe_jump_finish(
        &mut self,
        i: usize,
        from: Point,
        to: Point,
        high: bool,
        assisted: bool,
    ) {
        self.traversal[i].pending = Some(PendingTraversal {
            movement: Movement {
                to,
                cost: 0,
                assisted,
            },
            hold_wall: high,
            jump_landing: !high,
        });
        self.traversal_after_step(i, from, to, false);
    }

    pub(super) fn traversal_mask(&self, i: usize) -> u128 {
        let mut mask = 0;
        for (action, decision) in [
            (Act::JumpLong, LONG_JUMP),
            (Act::JumpHigh, HIGH_JUMP),
            (Act::Climb, CLIMB),
        ] {
            if self.first_traversal_choice(i, decision) {
                mask |= action.bit();
            }
        }
        if self.flight_candidate(i, 1).is_some() {
            mask |= Act::OtherCommon1.bit();
        }
        if self.flight_candidate(i, -1).is_some() {
            mask |= Act::OtherCommon2.bit();
        }
        mask
    }

    pub(super) fn traversal_step(&mut self, i: usize, a: Act) -> bool {
        match a {
            Act::JumpLong => self.request(i, LONG_JUMP),
            Act::JumpHigh => self.request(i, HIGH_JUMP),
            Act::Climb => self.request(i, CLIMB),
            Act::OtherCommon1 | Act::OtherCommon2 => {
                let direction = if a == Act::OtherCommon1 { 1 } else { -1 };
                let movement = self.flight_candidate(i, direction).expect("legal flight");
                self.finish_traversal(i, movement, false, false);
            }
            _ => return false,
        }
        true
    }

    fn choice_count(&self, i: usize, id: i32) -> usize {
        let multiplier = if self.magic_jump_active(i) && !self.traversal[i].spell_jump {
            2
        } else {
            1
        };
        match id {
            LONG_JUMP => multiplier * DIRECTIONS.len() * JUMP_RANKS,
            HIGH_JUMP => multiplier * JUMP_RANKS,
            CLIMB => 10,
            _ => 0,
        }
    }

    fn movement_choice(&self, i: usize, id: i32, choice: usize) -> Option<Movement> {
        match id {
            LONG_JUMP => self.jump_candidate(i, false, choice),
            HIGH_JUMP => self.jump_candidate(i, true, choice),
            CLIMB => self.climb_candidate(i, choice),
            _ => None,
        }
    }

    fn first_traversal_choice(&self, i: usize, id: i32) -> bool {
        self.movement_ready(i)
            && (0..self.choice_count(i, id))
                .any(|choice| self.movement_choice(i, id, choice).is_some())
    }

    pub(super) fn traversal_decision_mask(&self, i: usize, id: i32) -> u128 {
        if id == SLOW_FALL {
            return Act::Defend.bit() | Act::Pass.bit();
        }
        (0..self.choice_count(i, id))
            .filter(|&choice| self.movement_choice(i, id, choice).is_some())
            .fold(0, |mask, choice| mask | 1 << choice)
    }

    pub(super) fn traversal_decision_step(&mut self, i: usize, id: i32, choice: Act) {
        if id == SLOW_FALL {
            let damage = self.traversal[i].fall_damage.take().expect("pending fall");
            let reduction = if choice == Act::Defend {
                self.f[i].reaction = false;
                5 * self.f[i].build.level
            } else {
                0
            };
            self.land_fall(i, (damage - reduction).max(0));
            return;
        }
        let movement = self
            .movement_choice(i, id, choice as usize)
            .expect("legal traversal");
        self.finish_traversal(i, movement, id == CLIMB || id == HIGH_JUMP, id == LONG_JUMP);
    }

    fn finish_traversal(
        &mut self,
        i: usize,
        movement: Movement,
        hold_wall: bool,
        jump_landing: bool,
    ) {
        self.traversal_break_runup(i);
        self.traversal[i].pending = Some(PendingTraversal {
            movement,
            hold_wall,
            jump_landing,
        });
        self.queue_traversal_move(i, movement.to, movement.cost);
    }

    pub(super) fn traversal_can_resume(&self, i: usize) -> bool {
        self.traversal[i].pending.is_some_and(|plan| {
            self.movement_ready(i)
                && (plan.hold_wall || plan.jump_landing || self.traversal_fly_speed(i) > 0)
                && plan.movement.to != self.fighter_point(1 - i)
                && (2..4).all(|id| self.creature_point(id) != Some(plan.movement.to))
                && self.has_air_space(i, plan.movement.to)
        })
    }

    pub(super) fn traversal_cancel_pending(&mut self, i: usize) {
        self.traversal[i].pending = None;
    }

    /// Check at a stable main-phase boundary, after effects have changed speed
    /// or imposed a condition. Fall reactions must not displace other prompts.
    pub(super) fn traversal_falls(&mut self) {
        for i in [self.owner, 1 - self.owner] {
            let point = self.fighter_point(i);
            let hanging = self.traversal[i].climbing
                && self.climbing_contact(i, point)
                && !self.incap(i)
                && !self.f[i].prone;
            if self.f[i].hp == 0
                || self.entities.mounted[i]
                || self.map.supported(point)
                || hanging
                || self.traversal_fly_speed(i) > 0
            {
                continue;
            }
            let (_, cells) = self.fall_destination(i);
            let damage = (0..(i32::from(cells) / 2).min(20))
                .map(|_| self.roll(6))
                .sum();
            if damage > 0
                && self.f[i].build.class == Class::Monk
                && self.f[i].build.level >= 4
                && self.can_react(i)
            {
                self.traversal[i].fall_damage = Some(damage);
                self.request(i, SLOW_FALL);
                return;
            }
            self.land_fall(i, damage);
            if self.phase_id() != 0 {
                return;
            }
        }
    }

    fn fall_destination(&self, i: usize) -> (Point, i8) {
        let from = self.fighter_point(i);
        let directly_below = self.map.landing(from);
        let free = |point: Point| {
            point != self.fighter_point(1 - i)
                && (2..4).all(|id| self.creature_point(id) != Some(point))
        };
        let landing = if free(directly_below) {
            directly_below
        } else {
            // A featureless arena has no stacked ground spaces. Resolve an
            // occupied landing in the first adjacent supported square.
            (0..DIRECTIONS.len())
                .find_map(|dir| {
                    let (edge, _) = self.map.step_at(from, dir)?;
                    let destination = self.map.landing(edge);
                    (destination.altitude < from.altitude && free(destination))
                        .then_some(destination)
                })
                .unwrap_or(directly_below)
        };
        (landing, (from.altitude - landing.altitude).max(0))
    }

    fn land_fall(&mut self, i: usize, damage: i32) {
        let (landing, _) = self.fall_destination(i);
        self.traversal_break_runup(i);
        self.traversal[i].climbing = false;
        self.commit_move_point(i, landing);
        if damage > 0 {
            let before = self.f[i].hp + self.f[i].temp;
            self.damage_typed(i, damage, DamageType::Bludgeoning);
            if self.f[i].hp + self.f[i].temp < before {
                self.knock_prone(i);
            }
        }
    }

    pub(super) fn traversal_label(&self, _i: usize, action: Act) -> Option<String> {
        Some(
            match action {
                Act::JumpLong => "Long jump",
                Act::JumpHigh => "High jump",
                Act::Climb => "Climb stone wall",
                Act::OtherCommon1 => "Fly up 5 feet",
                Act::OtherCommon2 => "Fly down 5 feet",
                _ => return None,
            }
            .into(),
        )
    }

    pub(super) fn traversal_decision_label(
        &self,
        i: usize,
        id: i32,
        choice: Act,
    ) -> Option<String> {
        if id == SLOW_FALL {
            return Some(if choice == Act::Defend {
                format!("Slow Fall (reduce damage by {})", 5 * self.f[i].build.level)
            } else {
                "Fall without using reaction".into()
            });
        }
        let movement = self.movement_choice(i, id, choice as usize)?;
        let assisted = if movement.assisted {
            " with Jump spell"
        } else {
            ""
        };
        let cost = 5 * movement.cost;
        Some(match id {
            LONG_JUMP => format!(
                "Jump {} ft {}{assisted} ({cost} ft movement)",
                (choice as usize % JUMP_RANKS + 1) * 5,
                DIRECTION_NAMES[choice as usize / JUMP_RANKS % DIRECTIONS.len()]
            ),
            HIGH_JUMP => format!(
                "Jump {} ft upward{assisted} ({cost} ft movement)",
                (choice as usize % JUMP_RANKS + 1) * 5
            ),
            CLIMB => format!(
                "Climb to {}{}, elevation {} ft ({cost} ft movement)",
                char::from(b'A' + movement.to.cell.x as u8),
                movement.to.cell.y + 1,
                movement.to.altitude * 5
            ),
            _ => return None,
        })
    }

    pub fn traversal_features(&self, i: usize) -> [f32; 8] {
        [
            f32::from(self.f[i].elevation) / f32::from(MAX_ELEVATION),
            f32::from(self.traversal[i].runup) / f32::from(RUNUP),
            f32::from(u8::from(self.traversal[i].climbing)),
            self.traversal_fly_speed(i) as f32 / 12.0,
            self.climb_speed(i) as f32 / 12.0,
            f32::from(u8::from(
                self.magic_jump_active(i) && !self.traversal[i].spell_jump,
            )),
            self.jump_distance(i, false) as f32 / 6.0,
            self.jump_distance(i, true) as f32 / 6.0,
        ]
    }

    pub fn traversal_choice_features(&self, i: usize, choice: Act) -> [f32; 8] {
        let id = self.phase_id();
        if id == SLOW_FALL && matches!(choice, Act::Defend | Act::Pass) {
            let damage = self.traversal[i].fall_damage.unwrap_or(0);
            let reduction = if choice == Act::Defend {
                5 * self.f[i].build.level
            } else {
                0
            };
            return [
                1.0,
                0.0,
                0.0,
                0.0,
                0.0,
                damage as f32 / 18.0,
                reduction as f32 / 25.0,
                (damage - reduction).max(0) as f32 / 18.0,
            ];
        }
        let Some(movement) = self.movement_choice(i, id, choice as usize) else {
            return [0.0; 8];
        };
        let from = self.fighter_point(i);
        [
            1.0,
            f32::from(movement.to.cell.x - from.cell.x) / 6.0,
            f32::from(movement.to.cell.y - from.cell.y) / 6.0,
            f32::from(movement.to.altitude - from.altitude) / 6.0,
            movement.cost as f32 / 12.0,
            f32::from(u8::from(self.is_difficult(movement.to))),
            f32::from(u8::from(movement.assisted)),
            f32::from(u8::from(self.map.supported(movement.to))),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::super::geometry::{Map, Pos};
    use super::super::magic::Concentration;
    use super::super::ACTS;
    use super::*;

    fn at(x: i8, y: i8, altitude: i8) -> Point {
        Point {
            cell: Pos { x, y },
            altitude,
        }
    }

    fn battle(seed: u64, class: Class, level: i32, start: Point) -> Battle {
        let mut b = Battle::new(seed, [class, Class::Wizard], [level, 1]);
        b.map = Map::new(0);
        b.owner = 0;
        b.f[0].pos = start.cell;
        b.f[0].elevation = start.altitude;
        b.f[1].pos = Pos { x: 8, y: 6 };
        b
    }

    fn long_choice(direction: usize, distance: usize, assisted: bool) -> Act {
        ACTS[direction * JUMP_RANKS + distance - 1
            + if assisted {
                DIRECTIONS.len() * JUMP_RANKS
            } else {
                0
            }]
    }

    #[test]
    fn natural_long_jump_requires_ten_feet_of_actual_runup_for_full_distance() {
        let mut b = battle(0, Class::Fighter, 5, at(0, 0, 0));
        let running_jump = long_choice(2, 3, false);
        assert_eq!(
            b.traversal_decision_mask(0, LONG_JUMP) & running_jump.bit(),
            0
        );
        b.step(Act::E).unwrap();
        assert_eq!(
            b.traversal_decision_mask(0, LONG_JUMP) & running_jump.bit(),
            0
        );
        b.step(Act::E).unwrap();
        assert_ne!(
            b.traversal_decision_mask(0, LONG_JUMP) & running_jump.bit(),
            0
        );
        b.step(Act::JumpLong).unwrap();
        assert_eq!(b.phase_id(), LONG_JUMP);
        b.step(running_jump).unwrap();
        assert_eq!(b.fighter_point(0), at(5, 0, 0));
        assert_eq!(b.moved, 5);
        assert_eq!(b.traversal[0].runup, 0);
    }

    #[test]
    fn second_story_work_uses_dex_and_a_climb_speed() {
        let start = at(1, 3, 0);
        let mut rogue = battle(0, Class::Rogue, 3, start);
        rogue.traversal[0].runup = RUNUP;
        assert_eq!(rogue.jump_distance(0, false), 3);
        rogue.step(Act::Climb).unwrap();
        rogue.step(Act::N).unwrap();
        assert_eq!(rogue.fighter_point(0), at(1, 3, 1));
        assert_eq!(rogue.moved, 1);

        let mut fighter = battle(0, Class::Fighter, 5, start);
        fighter.step(Act::Climb).unwrap();
        fighter.step(Act::N).unwrap();
        assert_eq!(fighter.fighter_point(0), at(1, 3, 1));
        assert_eq!(fighter.moved, 2);
        assert_ne!(fighter.mask() & Act::Climb.bit(), 0);
    }

    #[test]
    fn flying_can_cross_the_wall_and_land_without_fall_damage() {
        let mut b = battle(0, Class::Wizard, 5, at(1, 3, 0));
        let hp = b.f[0].hp;
        b.f[0].concentration = Some(Concentration::Fly);
        b.step(Act::OtherCommon1).unwrap();
        b.step(Act::OtherCommon1).unwrap();
        b.step(Act::E).unwrap();
        assert_eq!(b.fighter_point(0), at(2, 3, 2));
        assert_eq!(b.mask() & Act::OtherCommon2.bit(), 0);
        b.step(Act::E).unwrap();
        b.step(Act::OtherCommon2).unwrap();
        b.step(Act::OtherCommon2).unwrap();
        assert_eq!(b.fighter_point(0), at(3, 3, 0));
        assert_eq!(b.moved, 6);
        assert_eq!(b.f[0].hp, hp);
        assert!(!b.f[0].prone);
    }

    #[test]
    fn ending_the_fly_spell_while_airborne_causes_a_fall() {
        let mut b = battle(0, Class::Wizard, 5, at(1, 0, 4));
        b.f[0].concentration = Some(Concentration::Fly);
        let hp = b.f[0].hp;
        b.step(Act::DropConcentration).unwrap();
        assert_eq!(b.fighter_point(0), at(1, 0, 0));
        assert!(b.f[0].prone);
        assert!(b.f[0].hp < hp);
    }

    #[test]
    fn slow_fall_offers_reaction_and_prevents_prone_when_damage_is_zero() {
        for choice in [Act::Defend, Act::Pass] {
            let mut b = battle(0, Class::Monk, 4, at(1, 0, 4));
            let hp = b.f[0].hp;
            b.traversal_falls();
            assert_eq!(b.phase_id(), SLOW_FALL);
            assert_eq!(b.mask(), Act::Pass.bit() | Act::Defend.bit());
            b.step(choice).unwrap();
            assert_eq!(b.fighter_point(0), at(1, 0, 0));
            assert_eq!(b.f[0].reaction, choice == Act::Pass);
            assert_eq!(b.f[0].prone, choice == Act::Pass);
            assert_eq!(b.f[0].hp == hp, choice == Act::Defend);
        }
    }

    #[test]
    fn difficult_landing_uses_acrobatics_without_doubling_jump_cost() {
        let landing = at(3, 1, 0);
        let mut outcomes = [false; 2];
        for seed in 0..32 {
            let mut b = battle(seed, Class::Fighter, 5, at(1, 1, 0));
            b.traversal[0].runup = RUNUP;
            b.step(Act::JumpLong).unwrap();
            b.step(long_choice(2, 2, false)).unwrap();
            assert_eq!(b.fighter_point(0), landing);
            assert_eq!(b.moved, 2);
            outcomes[usize::from(b.f[0].prone)] = true;
        }
        assert_eq!(outcomes, [true, true]);
    }

    #[test]
    fn a_running_high_jump_reaches_a_wall_hold() {
        let mut b = battle(0, Class::Fighter, 5, at(1, 3, 0));
        assert_eq!(b.mask() & Act::JumpHigh.bit(), 0);
        b.traversal[0].runup = RUNUP;
        b.step(Act::JumpHigh).unwrap();
        assert_eq!(b.phase_id(), HIGH_JUMP);
        b.step(Act::N).unwrap();
        assert_eq!(b.fighter_point(0), at(1, 3, 1));
        assert!(b.traversal[0].climbing);
        assert_eq!(b.moved, 1);
    }

    #[test]
    fn the_jump_spell_can_pay_two_movement_once_per_turn() {
        let mut b = battle(0, Class::Ranger, 5, at(0, 0, 0));
        b.step(Act::Magic3).unwrap();
        if b.phase_id() == 38 {
            b.step(ACTS[b.mask().trailing_zeros() as usize]).unwrap();
        }
        assert!(b.magic_jump_active(0));
        b.step(Act::JumpLong).unwrap();
        b.step(long_choice(2, 6, true)).unwrap();
        assert_eq!(b.fighter_point(0), at(6, 0, 0));
        assert_eq!(b.moved, 2);
        assert_eq!(
            b.traversal_decision_mask(0, LONG_JUMP) & long_choice(6, 6, true).bit(),
            0
        );
    }

    #[test]
    fn a_jump_out_of_reach_waits_for_the_opportunity_decision() {
        let mut b = battle(0, Class::Fighter, 5, at(1, 0, 0));
        b.f[1].pos = Pos { x: 0, y: 0 };
        b.traversal[0].runup = RUNUP;
        b.step(Act::JumpLong).unwrap();
        b.step(long_choice(2, 2, false)).unwrap();
        assert_eq!(b.phase_id(), 1);
        assert_eq!(b.actor(), Some(1));
        assert_eq!(b.fighter_point(0), at(1, 0, 0));
        assert_eq!(b.moved, 0);
        b.step(Act::Pass).unwrap();
        assert_eq!(b.fighter_point(0), at(3, 0, 0));
        assert_eq!(b.moved, 2);
    }
}
