//! Parameter choices for the martial class features.
use super::{hits, unarmed, Battle, Class};
use crate::arena2::{
    common::{Check, Skill},
    entities::{EntityKind, Spirit},
    geometry::Pos,
    Act, ACTS,
};

pub(super) const LAY_TARGET: i32 = 24;
pub(super) const LAY_AMOUNT: i32 = 25;
pub(super) const PRIMAL: i32 = 33;
pub(super) const DISARM_TRAP: i32 = 113;
const LAY_CANCEL: usize = 127;
const CURE_POISON: usize = 32;

fn action_bit(index: usize) -> u128 {
    ACTS[index].bit()
}

impl Battle {
    pub(in crate::arena2) fn martial_observed_in_range(
        &self,
        observer: usize,
        target: usize,
        range: i32,
    ) -> bool {
        self.footprint(observer)
            .zip(self.observed_footprint(observer, target))
            .is_some_and(|(origin, destination)| {
                origin.distance(destination) <= range
                    && origin
                        .squares()
                        .any(|from| destination.squares().any(|to| self.map.los_at(from, to)))
            })
    }

    pub(in crate::arena2) fn martial_pos(&self, id: usize) -> Option<Pos> {
        self.f
            .get(id)
            .filter(|f| f.hp > 0)
            .map(|f| f.pos)
            .or_else(|| self.entity_position(id))
    }

    pub(in crate::arena2) fn martial_missing_hp(&self, id: usize) -> i32 {
        if let Some(fighter) = self.f.get(id) {
            self.magic_max_hp(id) - fighter.hp
        } else {
            self.entity(id)
                .map_or(0, |entity| entity.kind.stats().hp - entity.hp)
        }
    }

    pub(in crate::arena2) fn martial_target_label(&self, id: usize) -> String {
        if let Some(fighter) = self.f.get(id) {
            format!("{:?} (seat {})", fighter.build.class, id + 1)
        } else {
            self.entity(id).map_or_else(
                || format!("Creature {id}"),
                |entity| entity.kind.name().to_owned(),
            )
        }
    }

    pub(in crate::arena2) fn martial_poisoned(&self, id: usize) -> bool {
        self.f
            .iter()
            .any(|f| f.martial.poison_until[id] > self.turns)
    }

    pub(in crate::arena2) fn martial_poison_end(&mut self, id: usize) {
        for source in 0..2 {
            if self.f[source].martial.poison_until[id] > self.turns {
                let build = self.f[source].build;
                if self.martial_poison_save(id, 8 + build.pb + build.stats[1]) {
                    self.f[source].martial.poison_until[id] = 0;
                }
            }
        }
    }

    pub(in crate::arena2) fn martial_clear_poison(&mut self, id: usize) {
        for source in 0..2 {
            self.f[source].martial.poison_until[id] = 0;
        }
    }

    pub(in crate::arena2) fn martial_poison_save(&mut self, id: usize, dc: i32) -> bool {
        if id < 2 {
            self.save_poison_condition(id, dc)
        } else {
            self.martial_save(id, 2, dc)
        }
    }

    pub(in crate::arena2) fn martial_save(&mut self, id: usize, stat: usize, dc: i32) -> bool {
        if id < 2 {
            self.save(id, stat, dc)
        } else if self.entity(id).is_some() {
            self.entity_save(id, stat, dc)
        } else {
            true
        }
    }

    pub(in crate::arena2) fn lay_targets(&self, i: usize) -> u128 {
        (0..4)
            .filter(|&target| {
                self.martial_pos(target).is_some()
                    && self.martial_observed_in_range(i, target, 1)
                    && (self.martial_missing_hp(target) > 0
                        || (self.f[i].resource >= 5 && self.martial_poisoned(target)))
            })
            .fold(0, |mask, target| mask | action_bit(target))
    }

    fn lay_amounts(&self, i: usize) -> u128 {
        let Some(target) = self.f[i].martial.lay_target else {
            return 0;
        };
        let pool = self.f[i].resource;
        let missing = self.martial_missing_hp(target);
        let normal =
            (1..=pool.min(missing)).fold(0, |mask, amount| mask | action_bit(amount as usize));
        if pool >= 5 && self.martial_poisoned(target) {
            (0..=(pool - 5).min(missing)).fold(normal, |mask, amount| {
                mask | action_bit(CURE_POISON + amount as usize)
            })
        } else {
            normal
        }
    }

    pub(in crate::arena2) fn martial_primal_request(&mut self, i: usize, check: Check) {
        self.f[i].martial.primal_check = Some(check);
        self.request(i, PRIMAL);
    }

    pub(in crate::arena2) fn martial_disarm_targets(&self, i: usize) -> u128 {
        self.object_mask(i, 1) & (action_bit(8) | action_bit(9))
    }

    fn martial_disarm_trap(&mut self, i: usize, action: Act) {
        if action == Act::Pass {
            return;
        }
        if self.f[i].martial.disarm_bonus {
            self.bonus = false;
        } else {
            self.action = false;
        }
        self.disarm_readied_caltrops(i, action);
    }

    pub(in crate::arena2) fn disarm_readied_caltrops(&mut self, i: usize, action: Act) {
        let skill = Skill::SleightOfHand;
        let (bonus, _) = self.martial_skill_mod_as(i, skill, false);
        // The fixed Rogue is proficient in both Thieves' Tools and Sleight of Hand.
        let disadvantage = self.magic_ability_disadvantage(i, 1)
            || self.martial_poisoned(i)
            || self.entity_frightened(i);
        let check = self.d20_for(i, true, disadvantage) + bonus + self.magic_guidance(i, skill);
        let seat = action as usize - 8;
        if check >= 15 {
            self.disable_caltrops(seat);
        } else {
            self.rogue_knack_request(i, super::super::common::FailedCheck {
                check: super::Check::Disarm(seat),
                total: check,
                dc: 15,
            });
        }
    }

    pub(in crate::arena2) fn martial_decision_mask(&self, i: usize, phase: i32) -> u128 {
        match phase {
            LAY_TARGET => self.lay_targets(i) | action_bit(LAY_CANCEL),
            LAY_AMOUNT => self.lay_amounts(i) | action_bit(LAY_CANCEL),
            PRIMAL => Act::Rider.bit() | Act::Pass.bit(),
            DISARM_TRAP => self.martial_disarm_targets(i) | Act::Pass.bit(),
            unarmed::UNARMED_MODE => self.martial_bonus_mode_mask(i),
            unarmed::UNARMED_TARGET => self.martial_bonus_target_mask(i),
            _ => self.hit_decision_mask(i, phase),
        }
    }

    pub(in crate::arena2) fn martial_decision_step(&mut self, i: usize, phase: i32, action: Act) {
        if phase == DISARM_TRAP {
            self.martial_disarm_trap(i, action);
            return;
        }
        if matches!(phase, unarmed::UNARMED_MODE | unarmed::UNARMED_TARGET) {
            self.martial_unarmed_step(i, phase, action);
            return;
        }
        if phase == PRIMAL {
            let check = self.f[i]
                .martial
                .primal_check
                .take()
                .expect("Primal Knowledge requires a check");
            self.try_check_as(i, check, action == Act::Rider);
            return;
        }
        if hits::is_hit_phase(phase) {
            self.hit_decision_step(i, phase, action);
            return;
        }
        let index = action as usize;
        if index == LAY_CANCEL {
            self.f[i].martial.lay_target = None;
            return;
        }
        match phase {
            LAY_TARGET => {
                self.f[i].martial.lay_target = Some(index);
                self.request(i, LAY_AMOUNT);
            }
            LAY_AMOUNT => {
                let target = self.f[i]
                    .martial
                    .lay_target
                    .take()
                    .expect("Lay On Hands has a target");
                let cure = index >= CURE_POISON;
                let amount = if cure { index - CURE_POISON } else { index } as i32;
                self.bonus = false;
                self.f[i].resource -= amount + if cure { 5 } else { 0 };
                if cure {
                    self.martial_clear_poison(target);
                }
                if target < 2 {
                    self.heal(target, amount);
                } else {
                    self.entity_heal(target, amount);
                }
            }
            _ => unreachable!("unknown martial decision"),
        }
    }

    pub(in crate::arena2) fn martial_decision_label(
        &self,
        i: usize,
        phase: i32,
        action: Act,
    ) -> Option<String> {
        let index = action as usize;
        if matches!(phase, LAY_TARGET | LAY_AMOUNT) && index == LAY_CANCEL {
            return Some("Cancel Lay On Hands".into());
        }
        match phase {
            DISARM_TRAP if action == Act::Pass => Some("Cancel disarming".into()),
            DISARM_TRAP => {
                let point = self.caltrops(index.checked_sub(8)?)?;
                Some(format!(
                    "Disarm Caltrops at ({}, {}) (DC 15 Sleight of Hand)",
                    point.x, point.y
                ))
            }
            LAY_TARGET => Some(format!("Touch {}", self.martial_target_label(index))),
            LAY_AMOUNT if index == CURE_POISON => Some("Cure Poisoned (5 HP pool)".into()),
            LAY_AMOUNT if index > CURE_POISON => Some(format!(
                "Cure Poisoned and restore {} HP",
                index - CURE_POISON
            )),
            LAY_AMOUNT => Some(format!("Restore {index} HP")),
            PRIMAL if action == Act::Rider => Some("Use Strength (Primal Knowledge)".into()),
            unarmed::UNARMED_MODE | unarmed::UNARMED_TARGET => {
                self.martial_unarmed_label(i, phase, action)
            }
            PRIMAL => Some("Use the skill's normal ability".into()),
            _ => self.hit_decision_label(i, phase, action),
        }
    }

    pub fn hunters_lore(&self, i: usize) -> Option<Vec<crate::arena2::equipment::DamageType>> {
        use crate::arena2::equipment::DamageType::*;
        use crate::arena2::magic::Concentration;
        let ranger = &self.f[i];
        if !ranger.build.is_subclass(Class::Ranger, 0)
            || ranger.build.level < 3
            || ranger.concentration != Some(Concentration::HunterMark)
        {
            return None;
        }
        let target = self.magic_mark_target(i)?;
        let mut resistance = Vec::new();
        if target >= 2 {
            use crate::arena2::entities::FamiliarForm;
            if let Some(entity) = self.entity(target) {
                match entity.kind {
                    EntityKind::Familiar {
                        form: FamiliarForm::Badger,
                        ..
                    } => resistance.push(Poison),
                    EntityKind::Familiar {
                        form: FamiliarForm::GiantFireBeetle,
                        ..
                    } => resistance.push(Fire),
                    _ => {}
                }
            }
            return Some(resistance);
        }
        if self.f[target].raging {
            resistance.extend([Bludgeoning, Piercing, Slashing]);
        }
        if self.origin_resists_poison(target) || self.magic_resists_poison(target) {
            resistance.push(Poison);
        }
        Some(resistance)
    }

    pub fn divine_sense_types(&self, i: usize) -> Vec<(usize, Spirit, Pos)> {
        let fighter = &self.f[i];
        if fighter.build.class != Class::Paladin
            || self.incap(i)
            || fighter.martial.sense_until < self.turns
        {
            return Vec::new();
        }
        (2..4)
            .filter_map(|target| {
                let entity = self.entity(target)?;
                let (EntityKind::Familiar { spirit, .. } | EntityKind::Steed { spirit, .. }) =
                    entity.kind;
                (spirit != Spirit::Fey && self.fighter_point(i).dist(entity.point()) <= 12)
                    .then_some((target, spirit, entity.position))
            })
            .collect()
    }
}
