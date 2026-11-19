//! Choices chained to successful attacks and their resolution.
use super::{Battle, Class, Hit};
use crate::arena2::{
    entities::{EntityKind, Spirit},
    footprint::Footprint,
    geometry::Point,
    origins::MasteryWeapon,
    Act, ACTS,
};

pub(super) const OPEN_HAND: i32 = 20;
pub(super) const REDIRECT: i32 = 21;
pub(super) const SNEAK: i32 = 22;
pub(super) const SMITE: i32 = 23;
pub(super) const CLEAVE: i32 = 26;
pub(super) const SACRED: i32 = 27;
pub(super) const COLOSSUS: i32 = 28;
pub(super) const SLOW: i32 = 29;
pub(super) const STUN: i32 = 30;
pub(super) const SAFE_MOVE: i32 = 31;
pub(super) const RECKLESS: i32 = 32;
pub(super) const SAFE_JUMP: i32 = 114;
pub(super) const DREADFUL_STRIKES: i32 = 180;
pub(super) const DREAD_AMBUSHER: i32 = 181;
const JUMP_BACK: usize = 127;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Cunning {
    #[default]
    None,
    Poison,
    Trip,
    Withdraw,
}

#[derive(Clone, Copy, Debug)]
pub struct MartialHit {
    hit: Hit,
    contact: Option<Footprint>,
    stage: i32,
    weapon_extra: i32,
    radiant_extra: i32,
    psychic_extra: i32,
    cunning: Cunning,
    movement: i32,
    dealt: i32,
    jump_high: bool,
}

impl MartialHit {
    fn new(hit: Hit, contact: Option<Footprint>) -> Self {
        Self {
            hit,
            contact,
            stage: 0,
            weapon_extra: 0,
            radiant_extra: 0,
            psychic_extra: 0,
            cunning: Cunning::None,
            movement: 0,
            dealt: 0,
            jump_high: false,
        }
    }
}

pub(in crate::arena2) fn is_hit_phase(phase: i32) -> bool {
    matches!(
        phase,
        OPEN_HAND
            | REDIRECT
            | SNEAK
            | SMITE
            | CLEAVE
            | SACRED
            | COLOSSUS
            | SLOW
            | STUN
            | SAFE_MOVE
            | RECKLESS
            | SAFE_JUMP
            | DREADFUL_STRIKES
            | DREAD_AMBUSHER
    )
}

fn choices(indices: impl IntoIterator<Item = usize>) -> u128 {
    indices
        .into_iter()
        .fold(Act::Pass.bit(), |mask, i| mask | ACTS[i].bit())
}

impl Battle {
    pub(in crate::arena2) fn martial_can_offer_reckless(&self, i: usize) -> bool {
        let f = &self.f[i];
        i == self.owner
            && f.build.class == Class::Barbarian
            && f.build.level >= 2
            && !f.martial.attack_rolled
            && !f.reckless
    }

    pub(in crate::arena2) fn martial_sacred_request(&mut self, hit: Hit) -> bool {
        let i = hit.attacker;
        if i >= 2
            || self.f[i].build.class != Class::Paladin
            || !hit.weapon
            || hit.unarmed
            || hit.ranged
            || !self.martial_sacred_strike(i)
        {
            return false;
        }
        self.f[i].martial.hit = Some(MartialHit::new(hit, self.footprint(hit.defender)));
        self.request(i, SACRED);
        true
    }

    pub(in crate::arena2) fn martial_hit_request(&mut self, hit: Hit, _damage: i32) -> bool {
        let i = hit.attacker;
        if i >= 2 || !hit.weapon {
            return false;
        }
        self.f[i].martial.hit = Some(MartialHit::new(hit, self.footprint(hit.defender)));
        self.martial_next_hit_choice(i)
    }

    fn martial_next_hit_choice(&mut self, i: usize) -> bool {
        let context = self.f[i].martial.hit.unwrap();
        let hit = context.hit;
        let class = self.f[i].build.class;
        let level = self.f[i].build.level;
        let phases = [super::fighter_rogue::TRIP_ATTACK, SNEAK, COLOSSUS, DREADFUL_STRIKES, DREAD_AMBUSHER, SMITE, STUN, OPEN_HAND];
        let next = phases
            .iter()
            .position(|&phase| phase == context.stage)
            .map_or(0, |index| index + 1);
        for &phase in &phases[next..] {
            let legal = match phase {
                super::fighter_rogue::TRIP_ATTACK => self.fighter_trip_available(hit),
                SNEAK => {
                    class == Class::Rogue
                        && !hit.unarmed
                        && hit
                            .weapon_kind
                            .is_some_and(|kind| kind.finesse() || kind.is_bow())
                        && self.f[i].martial.sneak_turn != self.turns
                        && (hit.advantage
                            || (!hit.disadvantage && self.martial_adjacent_ally(i, hit.defender)))
                }
                COLOSSUS => {
                    class == Class::Ranger
                        && self.f[i].build.variant == 0
                        && !hit.unarmed
                        && level >= 3
                        && self.f[i].martial.colossus_turn != self.turns
                        && self.martial_missing_hp(hit.defender) > 0
                }
                DREADFUL_STRIKES => {
                    crate::arena2::bard_ranger::is_fey_wanderer(self.f[i].build)
                        && !hit.unarmed
                        && self.f[i].bard_ranger.dreadful_turns[hit.defender] != self.turns
                }
                DREAD_AMBUSHER => {
                    crate::arena2::bard_ranger::is_gloom_stalker(self.f[i].build)
                        && !hit.unarmed
                        && self.f[i].bard_ranger.dread_turn != self.turns
                        && self.f[i].bard_ranger.dread_uses > 0
                }
                SMITE => class == Class::Paladin && self.martial_rider(i, hit),
                STUN => class == Class::Monk && self.martial_rider(i, hit),
                OPEN_HAND => self.has_martial_subclass(i, Class::Monk, 0) && hit.flurry,
                _ => false,
            };
            if legal {
                self.f[i].martial.hit.as_mut().unwrap().stage = phase;
                self.request(i, phase);
                return true;
            }
        }
        false
    }

    fn martial_adjacent_ally(&self, i: usize, target: usize) -> bool {
        let id = 2 + i;
        id != target
            && self.entity(id).is_some_and(|ally| {
                !ally.martial.stunned(self.turns)
                    && !self.magic_area_incapacitated(id)
                    && self
                        .combatant_distance(id, target)
                        .is_some_and(|distance| distance <= 1)
            })
    }

    fn redirect_targets(&self, i: usize, hit: Hit) -> impl Iterator<Item = usize> + '_ {
        let range = if hit.ranged { 12 } else { 1 };
        (0..4).filter(move |&target| {
            target != i
                && self.martial_pos(target).is_some()
                && self.martial_observed_in_range(i, target, range)
                && (target >= 2 || self.can_see(i, target))
        })
    }

    fn cleave_targets(&self, i: usize, context: MartialHit) -> impl Iterator<Item = usize> + '_ {
        (0..4).filter(move |&target| {
            target != context.hit.defender
                && target != i
                && self.martial_pos(target).is_some()
                && self.martial_observed_in_range(i, target, 1)
                && context
                    .contact
                    .zip(self.observed_footprint(i, target))
                    .is_some_and(|(contact, body)| contact.distance(body) <= 1)
        })
    }

    pub(in crate::arena2) fn hit_decision_mask(&self, i: usize, phase: i32) -> u128 {
        if phase == RECKLESS {
            return choices([Act::Rider as usize]);
        }
        let Some(context) = self.f[i].martial.hit else {
            return 0;
        };
        match phase {
            super::fighter_rogue::TRIP_ATTACK => choices([Act::Rider as usize]),
            SACRED | COLOSSUS | STUN | SLOW | DREADFUL_STRIKES | DREAD_AMBUSHER => choices([Act::Rider as usize]),
            OPEN_HAND => choices(0..5),
            SNEAK if self.f[i].build.level >= 5 => choices((0..4).filter(|&mode| {
                (mode != 1 || self.f[i].gear.poisoners_kit)
                    && (mode != 2 || self.combatant_size(context.hit.defender) <= 3)
            })),
            SNEAK => choices([0]),
            SMITE => choices((0..=2).filter(|&level| {
                if level == 0 {
                    self.f[i].martial.free_smite
                } else {
                    self.can_slot(i, level)
                }
            })),
            REDIRECT => choices(self.redirect_targets(i, context.hit)),
            CLEAVE => choices(self.cleave_targets(i, context)),
            SAFE_MOVE => {
                let mut mask = choices((0..=20).filter(|&dir| {
                    self.martial_safe_destination(i, dir, context.movement)
                        .is_some()
                }));
                for (action, high) in [(Act::JumpLong, false), (Act::JumpHigh, true)] {
                    if self.martial_safe_jump_mask(i, high, context.movement)
                        != ACTS[JUMP_BACK].bit()
                    {
                        mask |= action.bit();
                    }
                }
                if self.f[i].prone
                    && self.speed(i) > 0
                    && (self.speed(i) + 1) / 2 <= context.movement
                {
                    mask |= Act::Stand.bit();
                }
                mask
            }
            SAFE_JUMP => self.martial_safe_jump_mask(i, context.jump_high, context.movement),
            _ => 0,
        }
    }

    fn martial_safe_jump_mask(&self, i: usize, high: bool, budget: i32) -> u128 {
        (0..if high { 12 } else { 96 })
            .filter(|&choice| self.traversal_safe_jump(i, high, choice, budget).is_some())
            .fold(ACTS[JUMP_BACK].bit(), |mask, choice| {
                mask | ACTS[choice].bit()
            })
    }

    fn martial_safe_destination(
        &self,
        i: usize,
        choice: usize,
        remaining: i32,
    ) -> Option<(Point, i32, Option<bool>)> {
        if self.speed(i) == 0 {
            return None;
        }
        if choice < 8 {
            let (destination, cost) = self.movement_step(i, choice)?;
            let point = Point {
                cell: destination,
                altitude: self.fighter_point(i).altitude,
            };
            return (cost <= remaining && self.movement_available(i, destination))
                .then_some((point, cost, None));
        }
        let vertical = match choice {
            8..=14 => choice - 8,
            16..=20 => choice - 9,
            _ => return None,
        };
        let (point, cost, climbing) = self.traversal_safe_vertical(i, vertical, remaining)?;
        Some((point, cost, Some(climbing)))
    }

    pub(in crate::arena2) fn hit_decision_label(
        &self,
        i: usize,
        phase: i32,
        action: Act,
    ) -> Option<String> {
        if action == Act::Pass && phase != SAFE_JUMP {
            return Some(
                if phase == SAFE_MOVE {
                    "Finish safe movement"
                } else {
                    "Decline"
                }
                .into(),
            );
        }
        let index = action as usize;
        Some(match phase {
            OPEN_HAND => match index {
                0 => "Open Hand Technique: Addle".to_owned(),
                1..=3 => format!("Open Hand Technique: Push {} ft", 5 * index),
                4 => "Open Hand Technique: Topple".to_owned(),
                _ => return None,
            },
            SNEAK => match index {
                0 => "Sneak Attack (full damage)",
                1 => "Cunning Strike: Poison (forgo 1d6)",
                2 => "Cunning Strike: Trip (forgo 1d6)",
                3 => "Cunning Strike: Withdraw (forgo 1d6)",
                _ => return None,
            }
            .to_owned(),
            SMITE if index == 0 => "Divine Smite (free use)".to_owned(),
            SMITE => format!("Divine Smite (level {index} slot)"),
            SACRED => "Sacred Weapon: radiant damage".to_owned(),
            RECKLESS => "Attack Recklessly".to_owned(),
            COLOSSUS => "Colossus Slayer".to_owned(),
            DREADFUL_STRIKES => "Dreadful Strikes".to_owned(),
            DREAD_AMBUSHER => "Dread Ambusher".to_owned(),
            SLOW => "Weapon Mastery: Slow".to_owned(),
            STUN => "Stunning Strike (1 Focus Point)".to_owned(),
            REDIRECT => format!(
                "Redirect attack at {} (1 Focus Point)",
                self.martial_target_label(index)
            ),
            CLEAVE => format!("Cleave {}", self.martial_target_label(index)),
            SAFE_MOVE if action == Act::JumpLong => "Choose a safe long jump".into(),
            SAFE_MOVE if action == Act::JumpHigh => "Choose a safe high jump".into(),
            SAFE_MOVE if action == Act::Stand => "Stand up within the movement allowance".into(),
            SAFE_JUMP if index == JUMP_BACK => "Back to safe movement".into(),
            SAFE_JUMP => {
                let context = self.f[i].martial.hit?;
                let (point, cost, _) =
                    self.traversal_safe_jump(i, context.jump_high, index, context.movement)?;
                format!(
                    "Jump safely to ({}, {}, {}) ({} ft of movement)",
                    point.cell.x,
                    point.cell.y,
                    point.altitude,
                    cost * 5
                )
            }
            SAFE_MOVE => {
                let (destination, _, vertical) =
                    self.martial_safe_destination(i, index, self.f[i].martial.hit?.movement)?;
                let method = match vertical {
                    Some(true) => "Climb",
                    Some(false) => "Fly",
                    None => "Move",
                };
                format!(
                    "{method} safely to ({}, {}, {})",
                    destination.cell.x, destination.cell.y, destination.altitude
                )
            }
            _ => return None,
        })
    }

    pub(in crate::arena2) fn hit_decision_step(&mut self, i: usize, phase: i32, choice: Act) {
        if phase == RECKLESS {
            if choice == Act::Rider {
                self.martial_enable_reckless(i);
            }
            self.resume_reckless();
            return;
        }
        if phase == SACRED {
            self.f[i].martial.hit = None;
            self.resume_sacred_weapon(choice == Act::Rider);
            return;
        }
        let mut context = self.f[i]
            .martial
            .hit
            .take()
            .expect("hit choice has a context");
        let hit = context.hit;
        let index = choice as usize;
        let accepted = choice != Act::Pass;
        let build = self.f[i].build;
        let scale = if hit.crit { 2 } else { 1 };
        match phase {
            super::fighter_rogue::TRIP_ATTACK if accepted => {
                context.weapon_extra += self.fighter_trip_attack(hit);
            }
            SNEAK if accepted => {
                self.f[i].martial.sneak_turn = self.turns;
                context.cunning = [
                    Cunning::None,
                    Cunning::Poison,
                    Cunning::Trip,
                    Cunning::Withdraw,
                ][index];
                let count = super::sneak_attack_dice(build.level) - i32::from(index != 0);
                context.weapon_extra += (0..count * scale).map(|_| self.roll(6)).sum::<i32>();
                if self.is_subclass(i, Class::Rogue, 1) && build.level >= 3 && self.turns <= 2 {
                    context.weapon_extra += build.level;
                }
            }
            COLOSSUS if accepted => {
                self.f[i].martial.colossus_turn = self.turns;
                context.weapon_extra += (0..scale).map(|_| self.roll(8)).sum::<i32>();
            }
            DREADFUL_STRIKES if accepted => {
                self.f[i].bard_ranger.dreadful_turns[hit.defender] = self.turns;
                context.psychic_extra += (0..scale).map(|_| self.roll(4)).sum::<i32>();
            }
            DREAD_AMBUSHER if accepted => {
                self.f[i].bard_ranger.dread_turn = self.turns;
                self.f[i].bard_ranger.dread_uses -= 1;
                context.psychic_extra += (0..2 * scale).map(|_| self.roll(6)).sum::<i32>();
            }
            SMITE if accepted => {
                self.bonus = false;
                if index == 0 {
                    self.f[i].martial.free_smite = false;
                } else {
                    assert!(self.spend_slot(i, index));
                }
                let fiend = self.entity(hit.defender).is_some_and(|entity| {
                    matches!(
                        entity.kind,
                        EntityKind::Steed {
                            spirit: Spirit::Fiend,
                            ..
                        } | EntityKind::Familiar {
                            spirit: Spirit::Fiend,
                            ..
                        }
                    )
                });
                let dice = 1 + index.max(1) as i32 + i32::from(fiend);
                context.radiant_extra += (0..dice * scale).map(|_| self.roll(8)).sum::<i32>();
                if self.divine_offer_smite(i) {
                    self.f[i].martial.hit = Some(context);
                    return;
                }
            }
            STUN if accepted => {
                self.martial_followup(i, hit);
            }
            OPEN_HAND if accepted => {
                let dc = 8 + build.pb + build.stats[4];
                match index {
                    0 => {
                        if hit.defender < 2 {
                            self.f[hit.defender].addled = true;
                        } else {
                            self.entity_addle(hit.defender);
                        }
                    }
                    1..=3 if !self.martial_save(hit.defender, 0, dc) => {
                        self.martial_push(i, hit.defender, index as i32);
                    }
                    4 if !self.martial_save(hit.defender, 1, dc) => {
                        self.martial_knock_prone(hit.defender);
                    }
                    _ => {}
                }
            }
            SLOW => {
                if accepted {
                    if hit.defender < 2 {
                        self.f[hit.defender].slow = true;
                    } else {
                        self.entity_slow(i, hit.defender);
                    }
                }
                self.complete_attack(i);
                return;
            }
            CLEAVE => {
                if accepted {
                    self.f[i].martial.cleave_turn = self.turns;
                    self.cleave_attack(i, index);
                } else {
                    self.complete_attack(i);
                }
                return;
            }
            REDIRECT => {
                if accepted {
                    self.f[i].resource -= 1;
                    if !self.martial_save(index, 1, 8 + build.pb + build.stats[4]) {
                        let die = if build.level >= 5 { 8 } else { 6 };
                        let damage = self.roll(die) + self.roll(die) + build.stats[1];
                        self.martial_redirect_damage(index, damage, hit);
                    }
                }
                if !self.martial_after_hit_request(hit, 0, false) {
                    self.complete_attack(hit.attacker);
                }
                return;
            }
            SAFE_MOVE => {
                if !accepted {
                    if !self.martial_post_choice(i, context) {
                        self.complete_attack(i);
                    }
                    return;
                }
                if matches!(choice, Act::JumpLong | Act::JumpHigh) {
                    context.jump_high = choice == Act::JumpHigh;
                    self.f[i].martial.hit = Some(context);
                    self.request(i, SAFE_JUMP);
                    return;
                }
                if choice == Act::Stand {
                    context.movement -= (self.speed(i) + 1) / 2;
                    self.f[i].prone = false;
                } else {
                    let (destination, cost, vertical) = self
                        .martial_safe_destination(i, index, context.movement)
                        .expect("legal safe movement");
                    if !self.try_point_collision(i, destination) {
                        context.movement -= cost;
                        self.f[i].martial.shifted = true;
                        let previous = self.fighter_point(i);
                        self.commit_move_point(i, destination);
                        self.traversal_safe_finish(i, previous, self.fighter_point(i), vertical);
                    }
                }
                self.martial_resume_safe_move(i, context);
                return;
            }
            SAFE_JUMP => {
                if index != JUMP_BACK {
                    let (destination, cost, assisted) = self
                        .traversal_safe_jump(i, context.jump_high, index, context.movement)
                        .expect("legal jump using special movement");
                    if !self.try_point_collision(i, destination) {
                        context.movement -= cost;
                        self.f[i].martial.shifted = true;
                        let previous = self.fighter_point(i);
                        self.commit_move_point(i, destination);
                        self.traversal_safe_jump_finish(
                            i,
                            previous,
                            self.fighter_point(i),
                            context.jump_high,
                            assisted,
                        );
                    }
                }
                self.martial_resume_safe_move(i, context);
                return;
            }
            _ => {}
        }
        self.f[i].martial.hit = Some(context);
        if !self.martial_next_hit_choice(i) {
            self.finish_hit(context.weapon_extra, context.radiant_extra, context.psychic_extra, false);
        }
    }

    pub(in crate::arena2) fn martial_resume_inspiring_smite(&mut self, i: usize) {
        let context = self.f[i].martial.hit.expect("smite hit awaiting resolution");
        if !self.martial_next_hit_choice(i) {
            self.finish_hit(context.weapon_extra, context.radiant_extra, context.psychic_extra, false);
        }
    }

    fn martial_resume_safe_move(&mut self, i: usize, context: MartialHit) {
        self.f[i].martial.hit = Some(context);
        if self.phase_id() != 0 {
            self.defer_martial_decision(i, SAFE_MOVE);
        } else if self.f[i].hp > 0 && self.hit_decision_mask(i, SAFE_MOVE) != Act::Pass.bit() {
            self.request(i, SAFE_MOVE);
        } else {
            self.f[i].martial.hit = None;
            if !self.martial_post_choice(i, context) {
                self.complete_attack(i);
            }
        }
    }

    pub(in crate::arena2) fn martial_after_hit_request(
        &mut self,
        hit: Hit,
        dealt: i32,
        deflected: bool,
    ) -> bool {
        let i = hit.attacker;
        let target = hit.defender;
        let prior = (i < 2).then(|| self.f[i].martial.hit.take()).flatten();
        if self.f.iter().any(|f| f.hp == 0) {
            return false;
        }
        if deflected
            && dealt == 0
            && target < 2
            && self.f[target].resource > 0
            && self.redirect_targets(target, hit).next().is_some()
        {
            if i < 2 {
                self.f[i].martial.hit = prior;
            }
            self.f[target].martial.hit = Some(MartialHit::new(hit, self.footprint(target)));
            self.request(target, REDIRECT);
            return true;
        }
        if i >= 2 || !hit.weapon {
            return false;
        }
        let mut context = prior.unwrap_or_else(|| MartialHit::new(hit, self.footprint(target)));
        if dealt == 0 {
            context.cunning = Cunning::None;
        }
        let dc = 8 + self.f[i].build.pb + self.f[i].build.stats[1];
        match context.cunning {
            Cunning::Poison
                if self.martial_pos(target).is_some() && !self.martial_poison_save(target, dc) =>
            {
                self.f[i].martial.poison_until[target] = self.turns + 20;
            }
            Cunning::Trip
                if self.martial_pos(target).is_some() && !self.martial_save(target, 1, dc) =>
            {
                self.martial_knock_prone(target);
            }
            _ => {}
        }
        context.dealt = dealt;
        context.stage = 0;
        self.martial_post_choice(i, context)
    }

    fn martial_post_choice(&mut self, i: usize, mut context: MartialHit) -> bool {
        let hit = context.hit;
        let class = self.f[i].build.class;
        if context.stage == 0
            && (context.cunning == Cunning::Withdraw
                || (class == Class::Fighter && self.is_subclass(i, Class::Fighter, 0) && self.f[i].build.level >= 3 && hit.crit))
        {
            context.stage = SAFE_MOVE;
            context.movement = self.speed(i) / 2;
            self.f[i].martial.hit = Some(context);
            if self.hit_decision_mask(i, SAFE_MOVE) != Act::Pass.bit() {
                self.request(i, SAFE_MOVE);
                return true;
            }
            self.f[i].martial.hit = None;
        }
        if self.hit_mastery(hit) == Some(MasteryWeapon::Greataxe)
            && self.f[i].martial.cleave_turn != self.turns
            && self.cleave_targets(i, context).next().is_some()
        {
            context.stage = CLEAVE;
            self.f[i].martial.hit = Some(context);
            self.request(i, CLEAVE);
            return true;
        }
        if self.hit_mastery(hit) == Some(MasteryWeapon::Longbow)
            && context.dealt > 0
            && self.martial_pos(hit.defender).is_some()
        {
            context.stage = SLOW;
            self.f[i].martial.hit = Some(context);
            self.request(i, SLOW);
            return true;
        }
        false
    }

    fn martial_redirect_damage(&mut self, id: usize, damage: i32, hit: Hit) {
        let kind = hit.damage_kind;
        let before = self.footprint(id);
        self.damage_typed(id, damage, kind);
        self.magic_blessing_at_zero(Some(hit.defender), id, before);
    }

    fn martial_push(&mut self, source: usize, target: usize, distance: i32) {
        if target < 2 {
            self.forced_push(source, target, distance);
        } else {
            self.entity_forced_push(source, target, distance);
        }
    }

    fn martial_knock_prone(&mut self, target: usize) {
        if target < 2 {
            self.knock_prone(target);
        } else {
            self.entity_knock_prone(target);
        }
    }

    pub fn martial_decision_features(&self, viewer: usize) -> [f32; 16] {
        let mut result = [0.; 16];
        if viewer >= 2 {
            return result;
        }
        use crate::arena2::target_ids::relative_id;

        let state = &self.f[viewer].martial;
        let phase = self.phase_id();
        result[0] = if (20..=33).contains(&phase) {
            phase as f32 / 33.
        } else {
            0.
        };
        result[1] = u8::from(self.martial_poisoned(viewer)).into();
        result[2] = u8::from(self.f[viewer].gear.poisoners_kit).into();
        result[3] = u8::from(state.sense_until >= self.turns).into();
        if let Some(id) = state.lay_target {
            result[4] = (relative_id(viewer, id) + 1) as f32 / 4.;
            result[5] = self.martial_missing_hp(id) as f32 / 60.;
        }
        if let Some(hit) = state.hit {
            result[6] = (relative_id(viewer, hit.hit.defender) + 1) as f32 / 4.;
            result[7] = hit.weapon_extra as f32 / 48.;
            result[8] = hit.radiant_extra as f32 / 48.;
            result[9] = hit.movement as f32 / 8.;
            result[10] =
                u8::from(hit.hit.damage_kind == crate::arena2::equipment::DamageType::Radiant)
                    .into();
            result[11] = u8::from(hit.hit.flurry).into();
            result[12] = hit.cunning as u8 as f32 / 3.;
        }
        if let Some(sequence) = state.bonus_unarmed {
            result[0] = if self.phase_id() == 90 { 0.97 } else { 0.98 };
            result[9] = f32::from(sequence.remaining) / 2.;
            result[11] = u8::from(sequence.flurry).into();
            result[12] = match sequence.mode {
                Some(Act::Unarmed) => 0.25,
                Some(Act::Grapple) => 0.5,
                Some(Act::ShovePush) => 0.75,
                Some(Act::ShoveProne) => 1.,
                _ => 0.,
            };
        }
        if self.actor() == Some(viewer) {
            if phase == SAFE_JUMP {
                result[0] = 1.;
                result[12] = u8::from(state.hit.is_some_and(|hit| hit.jump_high)).into();
            } else if phase == super::choices::DISARM_TRAP {
                result[0] = 0.99;
                result[12] = u8::from(state.disarm_bonus).into();
            }
        }
        if let Some(check) = state.primal_check {
            result[12] = match check {
                super::Check::Hide => 0.25,
                super::Check::Search => 0.5,
                super::Check::Escape(_) => 0.75,
                super::Check::Study(_) => 1.,
                super::Check::Disarm(_) => 0.,
                super::Check::ExaminePhantasm => -0.25,
            }
        }
        if let Some(traits) = self.hunters_lore(viewer) {
            use crate::arena2::equipment::DamageType::*;
            result[13] = 1.;
            result[14] = u8::from(traits.contains(&Bludgeoning)).into();
            result[15] = (f32::from(u8::from(traits.contains(&Poison)))
                + 2. * f32::from(u8::from(traits.contains(&Fire))))
                / 3.;
        }
        result
    }
}
