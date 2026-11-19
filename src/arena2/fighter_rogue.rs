//! Level-three Fighter and Rogue subclass features used by the fixed builds.
use super::{Battle, Build, Class, Hit};
use crate::arena2::{
    common::{Check, FailedCheck},
    equipment::DamageType,
    geometry::{Point, Pos, HEIGHT, MAX_ELEVATION, WIDTH},
    objects::{END_OBJECTS, FIRST_OBJECT},
    Act, ACTS,
};

pub(in crate::arena2) const PRECISION_ATTACK: i32 = 187;
pub(in crate::arena2) const TRIP_ATTACK: i32 = 188;
pub(in crate::arena2) const PSIONIC_STRIKE: i32 = 189;
pub(in crate::arena2) const PSIONIC_KNACK: i32 = 190;
pub(in crate::arena2) const PSYCHIC_BLADE_TARGET: i32 = 191;
pub(in crate::arena2) const TELEKINETIC_SUBJECT: i32 = 192;
pub(in crate::arena2) const TELEKINETIC_CELL: i32 = 193;
pub(in crate::arena2) const TELEKINETIC_HEIGHT: i32 = 194;
pub(in crate::arena2) const PROTECTIVE_FIELD: i32 = 195;
pub(in crate::arena2) const TELEKINETIC_CONSENT: i32 = 196;
const TO_HAND: Act = Act::Other7;
const BACK: Act = Act::Pass;
const TELEKINETIC_CANCEL: Act = ACTS[127];

#[derive(Clone, Copy, Debug)]
pub struct SubclassState {
    pub(in crate::arena2) dice: i32,
    strike_turn: i32,
    blade_turn: i32,
    blade_used_only_free_hand: bool,
    surge_turn: i32,
    pub(in crate::arena2) maneuver_spent: bool,
    pub(in crate::arena2) blade_action: Option<Act>,
    pub(in crate::arena2) strike: Option<(Hit, i32)>,
    pub(in crate::arena2) knack: Option<FailedCheck>,
    free_movement: bool,
    subject: usize,
    cell: Pos,
}

impl Default for SubclassState {
    fn default() -> Self {
        Self {
            dice: 0,
            strike_turn: -1,
            blade_turn: -1,
            blade_used_only_free_hand: false,
            surge_turn: -1,
            maneuver_spent: false,
            blade_action: None,
            strike: None,
            knack: None,
            free_movement: false,
            subject: 0,
            cell: Pos { x: 0, y: 0 },
        }
    }
}

impl SubclassState {
    pub(in crate::arena2) fn new(build: Build) -> Self {
        if build.level < 3 {
            return Self::default();
        }
        let psionics = build.is_subclass(Class::Fighter, 2) || build.is_subclass(Class::Rogue, 2);
        let dice = if psionics {
            2 * build.pb
        } else {
            4 * i32::from(build.is_subclass(Class::Fighter, 1))
        };
        Self {
            dice,
            free_movement: build.is_subclass(Class::Fighter, 2),
            ..Self::default()
        }
    }
}

impl Battle {
    fn fighter_rogue_subclass(&self, i: usize, class: Class, variant: u8) -> bool {
        self.is_subclass(i, class, variant) && self.f[i].build.level >= 3
    }

    fn psionic_die(&self, i: usize) -> i32 {
        if self.f[i].build.level >= 5 {
            8
        } else {
            6
        }
    }

    pub(in crate::arena2) fn fighter_precision_available(&self, hit: Hit) -> bool {
        hit.attacker < 2 && hit.d20 > 1 && self.fighter_maneuver_available(hit.attacker)
    }

    fn fighter_maneuver_available(&self, i: usize) -> bool {
        let state = self.f[i].martial.fighter_rogue;
        self.fighter_rogue_subclass(i, Class::Fighter, 1) && state.dice > 0 && !state.maneuver_spent
    }

    pub(in crate::arena2) fn fighter_spend_maneuver(&mut self, i: usize) -> i32 {
        let state = &mut self.f[i].martial.fighter_rogue;
        state.dice -= 1;
        state.maneuver_spent = true;
        self.roll(8)
    }

    pub(in crate::arena2) fn fighter_trip_available(&self, hit: Hit) -> bool {
        self.fighter_maneuver_available(hit.attacker) && hit.d20 > 0
    }

    pub(in crate::arena2) fn fighter_trip_attack(&mut self, hit: Hit) -> i32 {
        let i = hit.attacker;
        let build = self.f[i].build;
        let dc = 8 + build.pb + build.stats[0].max(build.stats[1]);
        let mut damage = self.fighter_spend_maneuver(i);
        if hit.crit {
            damage += self.roll(8);
        }
        if self.combatant_size(hit.defender) <= 3 && !self.martial_save(hit.defender, 0, dc) {
            if hit.defender < 2 {
                self.knock_prone(hit.defender);
            } else {
                self.entity_knock_prone(hit.defender);
            }
        }
        damage
    }

    pub(in crate::arena2) fn fighter_can_parry(&self, i: usize, hit: Hit) -> bool {
        self.fighter_rogue_subclass(i, Class::Fighter, 1)
            && self.f[i].martial.fighter_rogue.dice > 0
            && hit.attacker != i
            && hit.d20 != 0
            && !hit.ranged
    }

    pub(in crate::arena2) fn fighter_reduce_damage(&mut self, i: usize, damage: i32) -> i32 {
        self.f[i].martial.fighter_rogue.dice -= 1;
        (damage - (self.roll(8) + self.f[i].build.stats[0].max(self.f[i].build.stats[1])).max(0))
            .max(0)
    }

    pub(in crate::arena2) fn fighter_protects(&self, i: usize, target: usize) -> bool {
        if !self.fighter_rogue_subclass(i, Class::Fighter, 2)
            || self.f[i].martial.fighter_rogue.dice == 0
            || !self.can_react(i)
            || self.magic_no_reactions(i)
            || self.f[i].addled
        {
            return false;
        }
        i == target
            || (self.entity_visible(i, target)
                && self
                    .combatant_distance(i, target)
                    .is_some_and(|distance| distance <= 6))
    }

    pub(in crate::arena2) fn fighter_has_field(&self, i: usize) -> bool {
        self.fighter_rogue_subclass(i, Class::Fighter, 2)
            && self.f[i].martial.fighter_rogue.dice > 0
    }

    pub(in crate::arena2) fn fighter_resolve_fields(
        &mut self,
        target: usize,
        mut damage: i32,
    ) -> i32 {
        for caster in [target % 2, 1 - target % 2] {
            if damage > 0
                && self.fighter_protects(caster, target)
                && self.fighter_field_choice(caster, target, damage)
            {
                damage = self.fighter_spend_field(caster, damage);
            }
        }
        damage
    }

    fn fighter_spend_field(&mut self, i: usize, damage: i32) -> i32 {
        self.f[i].reaction = false;
        self.f[i].martial.fighter_rogue.dice -= 1;
        (damage - (self.roll(self.psionic_die(i)) + self.f[i].build.stats[3]).max(1)).max(0)
    }

    pub(in crate::arena2) fn fighter_psionic_strike_request(
        &mut self,
        hit: Hit,
        damage: i32,
    ) -> bool {
        let i = hit.attacker;
        if i >= 2
            || i != self.owner
            || !self.fighter_rogue_subclass(i, Class::Fighter, 2)
            || !hit.weapon
            || hit.unarmed
            || damage <= 0
        {
            return false;
        }
        let distance = if hit.defender < FIRST_OBJECT {
            self.combatant_distance(i, hit.defender)
        } else {
            self.object(hit.defender)
                .map(|object| self.fighter_point(i).dist(object.point))
        };
        if !distance.is_some_and(|distance| distance <= 6) {
            return false;
        }
        let state = &mut self.f[i].martial.fighter_rogue;
        if state.dice == 0 || state.strike_turn == self.turns {
            return false;
        }
        state.strike = Some((hit, damage));
        self.request(i, PSIONIC_STRIKE);
        true
    }

    pub(in crate::arena2) fn fighter_finish_psionic_strike(&mut self, i: usize, use_die: bool) {
        let state = &mut self.f[i].martial.fighter_rogue;
        let (hit, dealt) = state.strike.take().expect("psionic strike has an attack");
        if use_die {
            state.dice -= 1;
            state.strike_turn = self.turns;
            let before = self.footprint(hit.defender);
            let damage = (self.roll(self.psionic_die(i)) + self.f[i].build.stats[3]).max(0);
            if hit.defender >= FIRST_OBJECT {
                self.damage_object(hit.defender, damage, DamageType::Force);
            } else {
                self.damage_typed(hit.defender, damage, DamageType::Force);
                self.magic_blessing_at_zero(Some(i), hit.defender, before);
            }
        }
        if hit.defender >= FIRST_OBJECT || !self.martial_after_hit_request(hit, dealt, false) {
            self.complete_attack(i);
        }
    }

    pub(in crate::arena2) fn rogue_knack_request(&mut self, i: usize, result: FailedCheck) -> bool {
        let build = self.f[i].build;
        if !self.fighter_rogue_subclass(i, Class::Rogue, 2)
            || self.f[i].martial.fighter_rogue.dice == 0
        {
            return false;
        }
        let Some((skill, _)) = self.check_requirements(i, result.check) else {
            return false;
        };
        let stat = skill.ability();
        if super::super::origins::skill_bonus(build, skill, stat) - build.stats[stat] < build.pb {
            return false;
        }
        self.f[i].martial.fighter_rogue.knack = Some(result);
        self.request(i, PSIONIC_KNACK);
        true
    }

    pub(in crate::arena2) fn rogue_finish_knack(&mut self, i: usize, use_die: bool) {
        let result = self.f[i]
            .martial
            .fighter_rogue
            .knack
            .take()
            .expect("failed check");
        if use_die {
            let total = result.total + self.roll(self.psionic_die(i));
            if total >= result.dc {
                self.f[i].martial.fighter_rogue.dice -= 1;
                self.resolve_successful_check(i, FailedCheck { total, ..result }, false);
            }
        }
    }

    pub(in crate::arena2) fn rogue_blade_available(&self, i: usize) -> bool {
        self.fighter_rogue_subclass(i, Class::Rogue, 2) && self.gear_free_hand(i)
    }

    pub(in crate::arena2) fn note_rogue_blade(&mut self, i: usize) {
        if i == self.owner {
            self.f[i].martial.fighter_rogue.blade_turn = self.turns;
            self.f[i].martial.fighter_rogue.blade_used_only_free_hand =
                !self.gear_two_free_hands(i);
        }
    }

    pub(in crate::arena2) fn rogue_blade_targets(&self, i: usize, action: Act) -> u128 {
        let range = if matches!(action, Act::Special4 | Act::Special6) {
            24
        } else {
            1
        };
        (0..4)
            .filter(|&target| {
                target % 2 != i
                    && !self.suggestion_charmed_by(i, target)
                    && self.martial_observed_in_range(i, target, range)
            })
            .fold(self.object_mask(i, range), |mask, target| {
                mask | ACTS[target].bit()
            })
    }

    pub(in crate::arena2) fn fighter_surge_used(&mut self, i: usize) {
        self.f[i].martial.fighter_rogue.surge_turn = self.turns;
    }

    pub(in crate::arena2) fn fighter_rogue_mask(&self, i: usize) -> u128 {
        let mut mask = 0;
        if self.rogue_blade_available(i) {
            for action in [Act::Special3, Act::Special4, Act::Special5, Act::Special6] {
                let allowed = if matches!(action, Act::Special5 | Act::Special6) {
                    self.bonus
                        && (!self.f[i].martial.fighter_rogue.blade_used_only_free_hand
                            || self.gear_two_free_hands(i))
                        && self.f[i].martial.fighter_rogue.blade_turn == self.turns
                } else {
                    self.action || self.attacks > 0
                };
                if allowed && self.rogue_blade_targets(i, action) != 0 {
                    mask |= action.bit();
                }
            }
        }
        let state = self.f[i].martial.fighter_rogue;
        if self.fighter_rogue_subclass(i, Class::Fighter, 2)
            && self.action
            && state.surge_turn != self.turns
            && (state.free_movement || state.dice > 0)
            && self.telekinetic_subjects(i) != 0
        {
            mask |= Act::Special3.bit();
        }
        mask
    }

    pub(in crate::arena2) fn fighter_rogue_main_step(&mut self, i: usize, action: Act) {
        if self.fighter_rogue_subclass(i, Class::Rogue, 2) {
            self.f[i].martial.fighter_rogue.blade_action = Some(action);
            self.request(i, PSYCHIC_BLADE_TARGET);
        } else {
            self.request(i, TELEKINETIC_SUBJECT);
        }
    }

    pub(in crate::arena2) fn fighter_rogue_resource_summary(&self, i: usize) -> Option<String> {
        let state = self.f[i].martial.fighter_rogue;
        if self.fighter_rogue_subclass(i, Class::Fighter, 1) {
            return Some(format!("Superiority Dice {}d8", state.dice));
        }
        if self.fighter_rogue_subclass(i, Class::Fighter, 2)
            || self.fighter_rogue_subclass(i, Class::Rogue, 2)
        {
            let free = if state.free_movement {
                " | Free Telekinetic Movement"
            } else {
                ""
            };
            return Some(format!(
                "Psionic Energy Dice {}d{}{free}",
                state.dice,
                self.psionic_die(i)
            ));
        }
        None
    }

    pub(in crate::arena2) fn fighter_rogue_label(&self, i: usize, action: Act) -> Option<String> {
        if self.fighter_rogue_subclass(i, Class::Rogue, 2) {
            let name = match action {
                Act::Special3 => "Psychic Blade (melee)",
                Act::Special4 => "Psychic Blade (throw)",
                Act::Special5 => "Second Psychic Blade (melee)",
                Act::Special6 => "Second Psychic Blade (throw)",
                _ => return None,
            };
            return Some(name.into());
        }
        if self.fighter_rogue_subclass(i, Class::Fighter, 2) && action == Act::Special3 {
            return Some("Telekinetic Movement".into());
        }
        if self.fighter_rogue_subclass(i, Class::Fighter, 1) && action == Act::Defend {
            return Some("Parry".into());
        }
        None
    }

    fn telekinetic_subjects(&self, i: usize) -> u128 {
        let origin = self.fighter_point(i);
        let objects = (FIRST_OBJECT..END_OBJECTS)
            .filter(|&id| {
                self.observed_object(i, id)
                    .is_some_and(|item| origin.dist(item.point) <= 6)
            })
            .fold(0, |mask, id| mask | ACTS[id].bit());
        let creatures = (0..4)
            .filter(|&target| {
                target != i
                    && self.entity_visible(i, target)
                    && self
                        .combatant_distance(i, target)
                        .is_some_and(|distance| distance <= 6)
            })
            .fold(0, |mask, target| mask | ACTS[target].bit());
        let daggers = (4..6)
            .filter(|&id| self.gear_telekinetic_held_dagger(i, id))
            .fold(0, |mask, id| mask | ACTS[id].bit());
        objects | creatures | daggers
    }

    fn telekinetic_destination(&self, i: usize, destination: Point) -> bool {
        let subject = self.f[i].martial.fighter_rogue.subject;
        let from = if subject < FIRST_OBJECT {
            self.creature_point(subject)
        } else if self.gear_telekinetic_held_dagger(i, subject) {
            Some(self.fighter_point(i))
        } else {
            self.object(subject).map(|object| object.point)
        };
        from.is_some_and(|from| from != destination && from.dist(destination) <= 6)
            && self.map.can_occupy(destination)
            && self.map.los_at(self.fighter_point(i), destination)
            && !self.magic_cloud_blocks(self.fighter_point(i), destination)
            && (!(8..10).contains(&subject) || {
                let landing = self.map.landing(destination);
                landing.altitude == 0 && self.caltrops(1 - subject % 2) != Some(landing.cell)
            })
            && (0..4).all(|other| {
                other == subject
                    || self
                        .observed_footprint(i, other)
                        .is_none_or(|body| !body.squares().any(|p| p == destination))
            })
            && (subject >= FIRST_OBJECT
                || self
                    .footprint_fit_kind(
                        subject,
                        destination,
                        self.combatant_size(subject),
                        true,
                        true,
                    )
                    .is_some())
    }

    fn telekinetic_cell(index: usize) -> Pos {
        Pos {
            x: (index % WIDTH as usize) as i8,
            y: (index / WIDTH as usize) as i8,
        }
    }

    pub(in crate::arena2) fn fighter_rogue_phase_mask(&self, i: usize, phase: i32) -> u128 {
        let choices = match phase {
            TRIP_ATTACK => self.hit_decision_mask(i, phase),
            PRECISION_ATTACK | PSIONIC_STRIKE => Act::Rider.bit(),
            PSIONIC_KNACK => Act::Defend.bit(),
            PSYCHIC_BLADE_TARGET => self.f[i]
                .martial
                .fighter_rogue
                .blade_action
                .map_or(0, |action| self.rogue_blade_targets(i, action)),
            TELEKINETIC_SUBJECT => self.telekinetic_subjects(i),
            TELEKINETIC_CONSENT => Act::Defend.bit(),
            TELEKINETIC_CELL => (0..(WIDTH as usize * HEIGHT as usize))
                .filter(|&index| {
                    (0..=MAX_ELEVATION).any(|altitude| {
                        self.telekinetic_destination(
                            i,
                            Point {
                                cell: Self::telekinetic_cell(index),
                                altitude,
                            },
                        )
                    })
                })
                .fold(self.telekinetic_pickup(i), |mask, id| mask | ACTS[id].bit()),
            TELEKINETIC_HEIGHT => (0..=MAX_ELEVATION)
                .filter(|&altitude| {
                    self.telekinetic_destination(
                        i,
                        Point {
                            cell: self.f[i].martial.fighter_rogue.cell,
                            altitude,
                        },
                    )
                })
                .fold(0, |mask, altitude| mask | ACTS[altitude as usize].bit()),
            _ => 0,
        };
        choices
            | if phase == TELEKINETIC_CELL {
                TELEKINETIC_CANCEL.bit()
            } else {
                BACK.bit()
            }
    }

    fn telekinetic_pickup(&self, i: usize) -> u128 {
        let id = self.f[i].martial.fighter_rogue.subject;
        if self.gear_can_telekinetic_pickup(i, id) {
            TO_HAND.bit()
        } else {
            0
        }
    }

    pub(in crate::arena2) fn fighter_rogue_phase_label(
        &self,
        i: usize,
        phase: i32,
        action: Act,
    ) -> Option<String> {
        if phase == TELEKINETIC_CELL && action == TELEKINETIC_CANCEL {
            return Some("Cancel Telekinetic Movement".into());
        }
        if phase != TELEKINETIC_CELL && action == BACK {
            return Some("Pass".into());
        }
        let id = action as usize;
        Some(match phase {
            PRECISION_ATTACK => "Precision Attack (d8)".into(),
            TRIP_ATTACK => "Trip Attack (d8)".into(),
            PSIONIC_STRIKE => format!("Psionic Strike (d{})", self.psionic_die(i)),
            PSIONIC_KNACK => format!("Psi-Bolstered Knack (d{})", self.psionic_die(i)),
            PSYCHIC_BLADE_TARGET => self.creature_label(id),
            TELEKINETIC_SUBJECT if id < FIRST_OBJECT => self.creature_label(id),
            TELEKINETIC_SUBJECT if self.gear_telekinetic_held_dagger(i, id) => "Held dagger".into(),
            TELEKINETIC_SUBJECT => self.object(id)?.label().into(),
            TELEKINETIC_CONSENT => "Allow Telekinetic Movement".into(),
            TELEKINETIC_CELL if action == TO_HAND => "Move dagger into free hand".into(),
            TELEKINETIC_CELL => {
                let cell = Self::telekinetic_cell(id);
                format!("Move to {}{}", char::from(b'A' + cell.x as u8), cell.y + 1)
            }
            TELEKINETIC_HEIGHT => format!("Move to {} ft elevation", id * 5),
            _ => return None,
        })
    }

    pub(in crate::arena2) fn telekinetic_step(&mut self, i: usize, phase: i32, choice: Act) {
        if (phase == TELEKINETIC_CELL && choice == TELEKINETIC_CANCEL)
            || (phase != TELEKINETIC_CELL && choice == BACK)
        {
            return;
        }
        if phase == TELEKINETIC_CONSENT {
            self.request(self.owner, TELEKINETIC_CELL);
            return;
        }
        let id = choice as usize;
        let state = &mut self.f[i].martial.fighter_rogue;
        match phase {
            TELEKINETIC_SUBJECT => {
                state.subject = id;
                if id < FIRST_OBJECT && id % 2 != i {
                    self.request(id % 2, TELEKINETIC_CONSENT);
                } else {
                    self.request(i, TELEKINETIC_CELL);
                }
            }
            TELEKINETIC_CELL if choice == TO_HAND => {
                let subject = state.subject;
                if self.gear_telekinetic_dagger_to_hand(i, subject) {
                    self.pay_telekinetic_movement(i);
                }
            }
            TELEKINETIC_CELL => {
                state.cell = Self::telekinetic_cell(id);
                self.request(i, TELEKINETIC_HEIGHT);
            }
            TELEKINETIC_HEIGHT => {
                let point = Point {
                    cell: state.cell,
                    altitude: id as i8,
                };
                let subject = state.subject;
                self.pay_telekinetic_movement(i);
                match subject {
                    0..=1 => self.commit_forced_move(subject, point),
                    2..=3 => {
                        self.entity_force_move(subject, point);
                    }
                    _ if self.gear_telekinetic_held_dagger(i, subject) => {
                        self.gear_telekinetic_dagger_from_hand(i, subject, point);
                    }
                    _ => {
                        self.move_ground_object(subject, point);
                    }
                }
            }
            _ => unreachable!("not a telekinetic movement phase"),
        }
    }

    fn pay_telekinetic_movement(&mut self, i: usize) {
        self.action = false;
        let state = &mut self.f[i].martial.fighter_rogue;
        if state.free_movement {
            state.free_movement = false;
        } else {
            state.dice -= 1;
        }
    }
}

impl Battle {
    pub(in crate::arena2) fn fighter_rogue_subclass_features(&self, viewer: usize) -> [f32; 8] {
        let state = self.f[viewer].martial.fighter_rogue;
        let marker = |yes: bool| f32::from(u8::from(yes));
        [
            state.dice as f32 / 6.,
            marker(state.free_movement),
            marker(state.strike_turn == self.turns),
            marker(state.blade_turn == self.turns && self.owner == viewer),
            marker(state.maneuver_spent),
            state
                .knack
                .filter(|result| result.check != Check::Search)
                .map_or(0., |check| (check.dc - check.total) as f32 / 20.),
            marker(self.fighter_rogue_subclass(viewer, Class::Rogue, 1) && self.turns <= 2),
            state.strike.map_or(0., |(_, damage)| damage as f32 / 30.),
        ]
    }

    pub(in crate::arena2) fn fighter_rogue_subclass_policy(
        &self,
        actor: usize,
        mask: u128,
        expert: bool,
    ) -> Option<Act> {
        let legal = |action: Act| mask & action.bit() != 0;
        let first = || ACTS[mask.trailing_zeros() as usize];
        let pick = match self.phase_id() {
            PROTECTIVE_FIELD => {
                let offer = self.save_choice_features().unwrap_or_default();
                if offer[2] > 0. && (!expert || offer[0] >= 0.075 || self.f[actor].hp <= 10) {
                    Act::Defend
                } else {
                    Act::Pass
                }
            }
            PRECISION_ATTACK if self.pending_features()[0] < -8. / 20. => Act::Pass,
            PRECISION_ATTACK | TRIP_ATTACK | PSIONIC_STRIKE => Act::Rider,
            PSIONIC_KNACK => Act::Defend,
            TELEKINETIC_CONSENT => Act::Pass,
            PSYCHIC_BLADE_TARGET | TELEKINETIC_SUBJECT => {
                [ACTS[(1 - actor) as usize], ACTS[3 - actor]]
                    .into_iter()
                    .find(|&action| legal(action))
                    .unwrap_or_else(first)
            }
            TELEKINETIC_CELL | TELEKINETIC_HEIGHT => {
                let cancel = if self.phase_id() == TELEKINETIC_CELL {
                    TELEKINETIC_CANCEL
                } else {
                    Act::Pass
                };
                let destinations = mask & !cancel.bit();
                if destinations != 0 {
                    ACTS[destinations.trailing_zeros() as usize]
                } else {
                    cancel
                }
            }
            0 if self.fighter_rogue_subclass(actor, Class::Rogue, 2) => [
                Act::Special1,
                Act::Special5,
                Act::Special6,
                Act::Special3,
                Act::Special4,
            ]
            .into_iter()
            .find(|&action| legal(action))?,
            1 | 61
                if self.fighter_rogue_subclass(actor, Class::Rogue, 2) && legal(Act::Special3) =>
            {
                Act::Special3
            }
            _ => return None,
        };
        Some(if legal(pick) { pick } else { first() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{bot, common::Check, equipment::Carried, martial::critical_threshold};

    fn adjacent(seed: u64, builds: [u8; 2]) -> Battle {
        let mut battle = Battle::new_roster_in(seed, builds, 0);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 1, y: 0 };
        battle.f[1].pos = Pos { x: 2, y: 0 };
        battle
    }

    #[test]
    fn fighter_presets_gate_critical_threshold_and_dice_by_subclass_and_level() {
        for (roster, threshold, dice) in [(12, 19, 0), (13, 20, 4), (14, 20, 6)] {
            let battle = adjacent(0, [roster, 24]);
            let mut build = battle.f[0].build;
            assert_eq!(critical_threshold(build), threshold);
            assert_eq!(battle.f[0].martial.fighter_rogue.dice, dice);
            build.level = 2;
            assert_eq!(critical_threshold(build), 20);
            assert_eq!(SubclassState::new(build).dice, 0);
        }
        for preset in [25, 26] {
            let battle = adjacent(0, [preset, 12]);
            assert_eq!(battle.mask() & Act::Martial4.bit(), 0);
            assert_eq!(battle.mask() & Act::Martial5.bit(), 0);
            assert_ne!(battle.mask() & Act::Utilize.bit(), 0);
        }
    }

    #[test]
    fn precision_converts_a_miss_without_granting_trip_on_the_same_attack() {
        let (mut battle, hp) = (0..1000)
            .find_map(|seed| {
                let mut battle = adjacent(seed, [13, 12]);
                let hp = battle.f[1].hp;
                battle.weapon_attack_against(0, 1, false);
                if battle.phase_id() != PRECISION_ATTACK {
                    return None;
                }
                let mut accepted = battle.clone();
                accepted.step(Act::Rider).unwrap();
                (accepted.f[1].hp < hp).then_some((battle, hp))
            })
            .expect("a missed attack that Precision can turn into a hit");
        assert_eq!(battle.mask(), Act::Rider.bit() | Act::Pass.bit());
        battle.step(Act::Rider).unwrap();
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, 3);
        assert!(battle.f[1].hp < hp);
        assert_ne!(battle.phase_id(), TRIP_ATTACK);
    }

    #[test]
    fn precision_attack_can_turn_a_miss_against_unattended_equipment_into_a_hit() {
        let fixture = (0..500).find_map(|seed| {
            let mut battle = adjacent(seed, [13, 24]);
            battle.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 1 });
            let hp = battle.object(5).unwrap().hp;
            battle.weapon_attack_against(0, 5, false);
            if battle.phase_id() != PRECISION_ATTACK {
                return None;
            }
            let mut repaired = battle.clone();
            repaired.step(Act::Rider).unwrap();
            (repaired.object(5).map_or(0, |item| item.hp) < hp).then_some((battle, repaired, hp))
        });
        let (mut ordinary, repaired, hp) = fixture.expect("Precision can rescue an object hit");
        ordinary.step(Act::Pass).unwrap();
        assert_eq!(ordinary.object(5).unwrap().hp, hp);
        assert_eq!(repaired.f[0].martial.fighter_rogue.dice, 3);
    }

    #[test]
    fn psionic_strike_can_deal_force_damage_to_an_object_damaged_by_the_weapon() {
        let fixture = (0..500).find_map(|seed| {
            let mut battle = adjacent(seed, [14, 24]);
            battle.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 1 });
            battle.weapon_attack_against(0, 5, false);
            if battle.phase_id() != PSIONIC_STRIKE {
                return None;
            }
            let hp = battle.object(5)?.hp;
            let mut empowered = battle.clone();
            empowered.step(Act::Rider).unwrap();
            (empowered.object(5).map_or(0, |item| item.hp) < hp).then_some((battle, empowered, hp))
        });
        let (mut ordinary, empowered, hp) = fixture.expect("Psionic Strike damages an object");
        ordinary.step(Act::Pass).unwrap();
        assert_eq!(ordinary.object(5).unwrap().hp, hp);
        assert_eq!(empowered.f[0].martial.fighter_rogue.dice, 5);
    }

    #[test]
    fn trip_and_parry_actually_change_damage_and_charge_the_shared_pool() {
        let trip = (0..500).any(|seed| {
            let mut battle = adjacent(seed, [13, 12]);
            battle.weapon_attack_against(0, 1, false);
            if battle.phase_id() != TRIP_ATTACK {
                return false;
            }
            let mut plain = battle.clone();
            plain.step(Act::Pass).unwrap();
            battle.step(Act::Rider).unwrap();
            assert_eq!(battle.f[0].martial.fighter_rogue.dice, 3);
            battle.f[1].prone && battle.f[1].hp < plain.f[1].hp
        });
        assert!(trip);

        let parry = (0..500).any(|seed| {
            let mut battle = adjacent(seed, [24, 13]);
            battle.weapon_attack_against(0, 1, false);
            if battle.phase_id() != 2 {
                return false;
            }
            let mut plain = battle.clone();
            plain.step(Act::Pass).unwrap();
            battle.step(Act::Defend).unwrap();
            assert_eq!(battle.f[1].martial.fighter_rogue.dice, 3);
            !battle.f[1].reaction && battle.f[1].hp > plain.f[1].hp
        });
        assert!(parry);
    }

    #[test]
    fn psi_warrior_strike_costs_a_die_and_occurs_once_per_turn() {
        let mut battle = (0..500)
            .find_map(|seed| {
                let mut battle = adjacent(seed, [14, 12]);
                battle.weapon_attack_against(0, 1, false);
                (battle.phase_id() == PSIONIC_STRIKE).then_some(battle)
            })
            .expect("a damaging weapon hit opens Psionic Strike");
        let hp = battle.f[1].hp;
        battle.step(Act::Rider).unwrap();
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, 5);
        assert!(battle.f[1].hp <= hp);
        assert_eq!(battle.f[0].martial.fighter_rogue.strike_turn, battle.turns);
        battle.weapon_attack_against(0, 1, false);
        assert_ne!(battle.phase_id(), PSIONIC_STRIKE);
    }

    #[test]
    fn telekinetic_movement_pays_on_resolution_and_uses_the_free_cast_first() {
        let mut battle = adjacent(0, [14, 24]);
        let start = Pos { x: 3, y: 0 };
        battle.f[0].gear.melee = Carried::Ground(start);
        battle.step(Act::Special3).unwrap();
        battle.step(ACTS[4]).unwrap();
        assert_eq!(battle.phase_id(), TELEKINETIC_CELL);
        battle.step(TELEKINETIC_CANCEL).unwrap();
        assert!(battle.action);
        assert!(battle.f[0].martial.fighter_rogue.free_movement);
        for expected_dice in [6, 5] {
            battle.action = true;
            battle.step(Act::Special3).unwrap();
            battle.step(ACTS[4]).unwrap();
            let cell = (battle.mask() & !TELEKINETIC_CANCEL.bit()).trailing_zeros() as usize;
            battle.step(ACTS[cell]).unwrap();
            assert_eq!(battle.phase_id(), TELEKINETIC_HEIGHT);
            let height = (battle.mask() & !Act::Pass.bit()).trailing_zeros() as usize;
            battle.step(ACTS[height]).unwrap();
            assert!(!battle.action);
            assert_eq!(battle.f[0].martial.fighter_rogue.dice, expected_dice);
            assert!(!battle.f[0].martial.fighter_rogue.free_movement);
        }
        assert!(battle.object(4).is_some());
    }

    #[test]
    fn telekinetic_movement_picks_up_and_releases_a_tiny_dagger() {
        let mut battle = adjacent(0, [14, 33]);
        battle.f[1].gear.melee = Carried::Ground(Pos { x: 3, y: 0 });
        battle.step(Act::Gear2).unwrap();
        battle.step(Act::Special3).unwrap();
        battle.step(ACTS[5]).unwrap();
        assert_ne!(battle.mask() & TO_HAND.bit(), 0);
        battle.step(TO_HAND).unwrap();
        assert_eq!(battle.f[1].gear.melee, Carried::ForeignHeld);
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, 6);

        battle.action = true;
        battle.step(Act::Special3).unwrap();
        battle.step(ACTS[5]).unwrap();
        let cell = (battle.mask() & !TELEKINETIC_CANCEL.bit()).trailing_zeros() as usize;
        battle.step(ACTS[cell]).unwrap();
        let height = (battle.mask() & !Act::Pass.bit()).trailing_zeros() as usize;
        battle.step(ACTS[height]).unwrap();
        assert!(matches!(battle.f[1].gear.melee, Carried::Ground(_)));
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, 5);
    }

    #[test]
    fn telekinetic_movement_waits_for_an_opponents_consent_and_cannot_use_action_surge() {
        let mut battle = adjacent(0, [14, 24]);
        let start = battle.f[1].pos;
        for answer in [Act::Pass, Act::Defend] {
            battle.step(Act::Special3).unwrap();
            battle.step(Act::NE).unwrap();
            assert_eq!(battle.phase_id(), TELEKINETIC_CONSENT);
            assert_eq!(battle.actor(), Some(1));
            battle.step(answer).unwrap();
        }
        let cell = (battle.mask() & !TELEKINETIC_CANCEL.bit()).trailing_zeros() as usize;
        battle.step(ACTS[cell]).unwrap();
        let height = (battle.mask() & !Act::Pass.bit()).trailing_zeros() as usize;
        battle.step(ACTS[height]).unwrap();
        assert!(!battle.action);
        assert_ne!(battle.f[1].pos, start);
        battle.step(Act::Special2).unwrap();
        assert!(battle.action);
        assert_eq!(battle.mask() & Act::Special3.bit(), 0);
    }

    #[test]
    fn soulknife_blade_does_not_spend_a_die_and_the_second_requires_another_hand() {
        let mut battle = adjacent(321, [26, 12]);
        let charge = battle.f[0].martial.fighter_rogue.dice;
        battle.step(Act::Special3).unwrap();
        assert_eq!(battle.phase_id(), PSYCHIC_BLADE_TARGET);
        battle.step(Act::NE).unwrap();
        while battle.phase_id() != 0 {
            battle.step(Act::Pass).unwrap();
        }
        assert_eq!(battle.mask() & Act::Special5.bit(), 0);
        assert!(battle.bonus);
        battle.step(Act::Gear2).unwrap();
        assert_ne!(battle.mask() & Act::Special5.bit(), 0);
        battle.step(Act::Special5).unwrap();
        battle.step(Act::NE).unwrap();
        assert!(!battle.bonus);
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, charge);
    }

    #[test]
    fn soulknife_can_manifest_a_blade_in_an_opportunity_attack() {
        let mut battle = adjacent(133, [26, 12]);
        battle.owner = 1;
        let dice = battle.f[0].martial.fighter_rogue.dice;
        battle.step(Act::E).unwrap();
        assert_eq!(battle.phase_id(), 1);
        assert_eq!(battle.actor(), Some(0));
        assert_ne!(battle.mask() & Act::Special3.bit(), 0);
        battle.step(Act::Special3).unwrap();
        assert!(!battle.f[0].reaction);
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, dice);
        assert_ne!(battle.f[0].martial.fighter_rogue.blade_turn, battle.turns);
    }

    #[test]
    fn soulknife_knack_spends_only_on_a_successful_proficient_check() {
        for (total, expected) in [(-100, 6), (14, 5)] {
            let mut battle = adjacent(5, [26, 12]);
            assert!(battle.rogue_knack_request(
                0,
                FailedCheck {
                    check: Check::Disarm(1),
                    total,
                    dc: 15,
                }
            ));
            battle.step(Act::Defend).unwrap();
            assert_eq!(battle.f[0].martial.fighter_rogue.dice, expected);
        }
    }

    #[test]
    fn assassin_first_round_sneak_attack_adds_the_level_without_doubling_it() {
        let mut battle = (0..500)
            .find_map(|seed| {
                let mut battle = adjacent(seed, [25, 12]);
                battle.weapon_attack_against(0, 1, false);
                (battle.phase_id() == 22).then_some(battle)
            })
            .expect("Assassinate grants opening attack advantage");
        let mut ordinary = battle.clone();
        ordinary.f[0].build.variant = 0;
        battle.step(Act::N).unwrap();
        ordinary.step(Act::N).unwrap();
        assert_eq!(ordinary.f[1].hp - battle.f[1].hp, battle.f[0].build.level);
    }

    #[test]
    fn baseline_and_expert_choose_legal_responses_to_replayed_protective_field() {
        for (seed, builds, layout) in [(201615, [20, 14], 0), (5923729589667448554, [18, 14], 2)] {
            let mut battle = Battle::new_roster_in(seed, builds, layout);
            let mut offered = false;
            for decision in 0..500 {
                if battle.finished() {
                    break;
                }
                for expert in [false, true] {
                    let action = bot::choose(&battle, expert);
                    assert!(
                        battle.mask() & action.bit() != 0,
                        "illegal {action:?} for seed {seed} at phase {}",
                        battle.phase_id()
                    );
                }
                offered |= battle.phase_id() == PROTECTIVE_FIELD;
                let action = bot::choose(&battle, decision % 2 != 0);
                battle.step(action).unwrap();
            }
            assert!(
                offered,
                "fixture never offered Protective Field for seed {seed}"
            );
        }
    }
}
