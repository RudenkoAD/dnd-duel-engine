//! Mode and target choices shared by Monk bonus unarmed strikes.
use super::{Battle, Class};
use crate::arena2::{
    common::UnarmedManeuver,
    geometry::{Point, Pos, MAX_ELEVATION},
    Act, ACTS,
};

pub(super) const UNARMED_MODE: i32 = 90;
pub(super) const UNARMED_TARGET: i32 = 91;
const FIRST_GUESS: usize = 16;
const GUESS_COUNT: usize = 27;

#[derive(Clone, Copy, Debug)]
pub(super) struct BonusUnarmed {
    pub(super) remaining: u8,
    pub(super) flurry: bool,
    paid: bool,
    pub(super) mode: Option<Act>,
}

impl Battle {
    pub(super) fn martial_unarmed_begin(&mut self, i: usize, flurry: bool) {
        self.f[i].martial.mercy_heal_flurry = false;
        self.f[i].martial.bonus_unarmed = Some(BonusUnarmed {
            remaining: if flurry { 2 } else { 1 },
            flurry,
            paid: false,
            mode: None,
        });
        self.request(i, UNARMED_MODE);
    }

    pub(super) fn martial_unarmed_can_start(&self, i: usize, flurry: bool) -> bool {
        let available = if flurry {
            self.f[i].resource > 0
        } else {
            self.martial_arts_active(i)
        };
        self.bonus
            && self.f[i].build.class == Class::Monk
            && available
            && (self.martial_unarmed_targets(i, Act::Unarmed) != 0
                || flurry && self.has_martial_subclass(i, Class::Monk, 1))
    }

    pub(in crate::arena2) fn martial_continue_flurry(&mut self, i: usize) -> bool {
        let Some(sequence) = self.f[i].martial.bonus_unarmed else {
            return false;
        };
        if sequence.remaining == 0
            || self.incap(i)
            || self.martial_unarmed_targets(i, Act::Unarmed) == 0
                && !self.mercy_flurry_heal_available(i)
        {
            self.f[i].martial.bonus_unarmed = None;
            return false;
        }
        self.request(i, UNARMED_MODE);
        true
    }

    pub(super) fn martial_bonus_mode_mask(&self, i: usize) -> u128 {
        [Act::Unarmed, Act::Grapple, Act::ShovePush, Act::ShoveProne]
            .into_iter()
            .filter(|&mode| self.martial_unarmed_targets(i, mode) != 0)
            .fold(Act::Pass.bit(), |mask, mode| mask | mode.bit())
            | if self.mercy_flurry_heal_available(i) { Act::Martial1.bit() } else { 0 }
    }

    pub(super) fn pay_mercy_flurry_strike(&mut self, i: usize) {
        let sequence = self.f[i].martial.bonus_unarmed.as_mut().expect("ongoing Flurry of Blows");
        if !sequence.paid {
            self.bonus = false;
            self.f[i].resource -= 1;
            sequence.paid = true;
        }
        sequence.remaining -= 1;
    }

    fn martial_reachable_target(&self, i: usize, target: usize, mode: Act) -> bool {
        if target == i || self.martial_pos(target).is_none() {
            return false;
        }
        if mode != Act::Unarmed && self.combatant_size(target) > self.combatant_size(i) + 1 {
            return false;
        }
        if mode == Act::Grapple && self.martial_target_grappled(target) {
            return false;
        }
        self.martial_observed_in_range(i, target, self.unarmed_reach(i))
    }

    fn martial_target_grappled(&self, target: usize) -> bool {
        self.f
            .get(target)
            .is_some_and(|fighter| fighter.grappled.is_some())
            || self
                .entity(target)
                .is_some_and(|entity| entity.martial.grappled.is_some())
    }

    pub(in crate::arena2) fn martial_unarmed_targets(&self, i: usize, mode: Act) -> u128 {
        if mode == Act::Grapple && !self.gear_free_hand(i) {
            return 0;
        }
        let creatures = (0..4)
            .filter(|&target| self.martial_reachable_target(i, target, mode))
            .fold(0, |bits, target| bits | ACTS[target].bit());
        if self.observed_pos(i, 1 - i).is_some() {
            return creatures;
        }
        (FIRST_GUESS..FIRST_GUESS + GUESS_COUNT)
            .filter(|&index| self.martial_unarmed_guess(i, index).is_some())
            .fold(creatures, |bits, index| bits | ACTS[index].bit())
    }

    fn martial_unarmed_guess(&self, i: usize, choice: usize) -> Option<Point> {
        let index = choice.checked_sub(FIRST_GUESS)?;
        if index >= GUESS_COUNT || index == 13 {
            return None;
        }
        let from = self.fighter_point(i);
        let point = Point {
            cell: Pos {
                x: from.cell.x + (index % 3) as i8 - 1,
                y: from.cell.y + (index / 3 % 3) as i8 - 1,
            },
            altitude: from.altitude + (index / 9) as i8 - 1,
        };
        (0..=MAX_ELEVATION)
            .contains(&point.altitude)
            .then_some(point)
            .filter(|&destination| self.map.can_aim_point(from, destination))
    }

    pub(super) fn martial_bonus_target_mask(&self, i: usize) -> u128 {
        self.f[i]
            .martial
            .bonus_unarmed
            .and_then(|value| value.mode)
            .map_or(Act::Pass.bit(), |mode| {
                Act::Pass.bit() | self.martial_unarmed_targets(i, mode)
            })
    }

    pub(super) fn martial_unarmed_label(
        &self,
        i: usize,
        phase: i32,
        action: Act,
    ) -> Option<String> {
        if phase == UNARMED_MODE {
            return match action {
                Act::Unarmed => Some("Unarmed Strike: Damage".into()),
                Act::Grapple => Some("Unarmed Strike: Grapple".into()),
                Act::ShovePush => Some("Unarmed Strike: Push 5 ft".into()),
                Act::ShoveProne => Some("Unarmed Strike: Knock prone".into()),
                Act::Pass => Some("Finish unarmed strikes".into()),
                Act::Martial1 => Some("Replace a Flurry of Blows strike with Hand of Healing".into()),
                _ => None,
            };
        }
        if action == Act::Pass {
            return Some("Choose another strike mode".into());
        }
        let index = action as usize;
        if index < 4 {
            Some(format!("Target {}", self.martial_target_label(index)))
        } else {
            let point = self.martial_unarmed_guess(i, index)?;
            Some(format!(
                "Aim at ({}, {}, {})",
                point.cell.x, point.cell.y, point.altitude
            ))
        }
    }

    pub(super) fn martial_unarmed_step(&mut self, i: usize, phase: i32, action: Act) {
        if phase == UNARMED_MODE {
            if action == Act::Pass {
                let paid = self.f[i]
                    .martial
                    .bonus_unarmed
                    .take()
                    .is_some_and(|seq| seq.paid);
                if paid {
                    self.finish_martial_unarmed(i);
                }
            } else if action == Act::Martial1 {
                self.start_mercy_heal(i, true);
            } else {
                self.f[i].martial.bonus_unarmed.as_mut().unwrap().mode = Some(action);
                self.request(i, UNARMED_TARGET);
            }
            return;
        }
        if action == Act::Pass {
            self.request(i, UNARMED_MODE);
            return;
        }
        let mut sequence = self.f[i].martial.bonus_unarmed.take().unwrap();
        let mode = sequence.mode.take().unwrap();
        let index = action as usize;
        let target = if index < 4 {
            Some(index)
        } else {
            let point = self.martial_unarmed_guess(i, index).expect("legal square");
            (0..4).find(|&target| {
                target != i
                    && self.martial_pos(target).is_some()
                    && self
                        .footprint(target)
                        .is_some_and(|body| body.squares().any(|part| part == point))
            })
        };
        if !sequence.paid {
            self.bonus = false;
            self.f[i].resource -= i32::from(sequence.flurry);
            sequence.paid = true;
        }
        sequence.remaining -= 1;
        self.f[i].martial.bonus_unarmed = Some(sequence);
        let Some(target) = target else {
            self.reveal(i);
            self.finish_martial_unarmed(i);
            return;
        };
        let maneuver = match mode {
            Act::Grapple => Some(UnarmedManeuver::Grapple),
            Act::ShovePush => Some(UnarmedManeuver::ShovePush),
            Act::ShoveProne => Some(UnarmedManeuver::ShoveProne),
            _ => None,
        };
        if let Some(maneuver) = maneuver {
            if self.combatant_size(target) > self.combatant_size(i) + 1
                || (mode == Act::Grapple && self.martial_target_grappled(target))
            {
                self.finish_martial_unarmed(i);
                return;
            }
            self.bonus_unarmed_maneuver(i, target, maneuver);
        } else {
            self.martial_bonus_hit(i, target, sequence.flurry);
        }
    }

    pub(super) fn finish_martial_unarmed(&mut self, i: usize) {
        if !self.martial_continue_flurry(i) {
            self.complete_attack(i);
        }
    }
}
