//! Sanctuary saves and continuation of a previously paid attack or spell.
use super::{Attack, Battle};
use crate::arena2::equipment::DamageType;
use crate::arena2::geometry::Point;
use crate::arena2::magic::SpellRider;
use crate::arena2::objects::END_OBJECTS;
use crate::arena2::{Act, ACTS};

pub(in crate::arena2) const RETARGET: i32 = 119;

#[derive(Clone, Copy, Debug)]
pub(in crate::arena2) struct TargetedDamage {
    pub stat: usize,
    pub dice: (i32, i32),
    pub bonus: i32,
    pub half: bool,
    pub kind: DamageType,
    pub rider: SpellRider,
    pub range: i32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::arena2) enum WardRedirect {
    Attack(Attack),
    SaveSpell(TargetedDamage),
    Ray(Point),
    Missile(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WardPending {
    source: usize,
    attempt: WardRedirect,
    rejected: u128,
    thrown_item: Option<(usize, bool)>,
}

impl Battle {
    pub(in crate::arena2) fn sanctuary_reject(
        &mut self,
        source: usize,
        defender: usize,
        attempt: WardRedirect,
        rejected: u128,
    ) -> bool {
        let Some(warden) = self.magic_ward_source(defender) else {
            return false;
        };
        let dc = self.spell_dc(warden);
        let succeeded = if source < 2 {
            self.save(source, 4, dc)
        } else {
            self.entity_save(source, 4, dc)
        };
        if succeeded {
            return false;
        }
        self.pending_sanctuary = Some(WardPending {
            source,
            attempt,
            rejected: rejected | ACTS[defender].bit(),
            thrown_item: None,
        });
        self.request(source % 2, RETARGET);
        true
    }

    pub(in crate::arena2) fn sanctuary_defer_throw(
        &mut self,
        issuer: usize,
        from_bow: bool,
    ) -> bool {
        if let Some(pending) = &mut self.pending_sanctuary {
            if matches!(pending.attempt, WardRedirect::Attack(attack) if attack.thrown) {
                pending.thrown_item = Some((issuer, from_bow));
                return true;
            }
        }
        false
    }

    pub(super) fn sanctuary_mask(&self) -> u128 {
        let Some(pending) = self.pending_sanctuary else {
            return 0;
        };
        let actor = pending.source % 2;
        let alternatives = match pending.attempt {
            WardRedirect::Attack(attack) => self.sanctuary_attack_targets(pending.source, attack),
            WardRedirect::SaveSpell(spell) => (0..4).fold(0, |mask, target| {
                if self.magic_reaches_known_creature(actor, target, spell.range, true)
                    && !self.suggestion_charmed_by(actor, target)
                {
                    mask | ACTS[target].bit()
                } else {
                    mask
                }
            }),
            WardRedirect::Ray(_) | WardRedirect::Missile(_) => (0..END_OBJECTS)
                .filter(|&target| self.projectile_eligible(actor, target).is_some())
                .fold(0, |mask, target| mask | ACTS[target].bit()),
        };
        Act::Pass.bit() | (alternatives & !pending.rejected)
    }

    fn sanctuary_attack_targets(&self, source: usize, attack: Attack) -> u128 {
        let actor = source % 2;
        let area = if !attack.weapon {
            self.magic_attack_source(actor)
        } else {
            None
        };
        let range = if !attack.ranged {
            1
        } else if attack.thrown {
            12
        } else {
            24
        };
        let targets = (0..4)
            .filter(|&target| {
                target % 2 != actor
                    && !self.suggestion_charmed_by(source, target)
                    && self
                        .observed_footprint(actor, target)
                        .is_some_and(|destination| {
                            if let Some(from) = area {
                                destination
                                    .squares()
                                    .any(|to| from.dist(to) <= range && self.map.los_at(from, to))
                            } else {
                                self.footprint(source).is_some_and(|origin| {
                                    origin.squares().any(|from| {
                                        destination.squares().any(|to| {
                                            from.dist(to) <= range && self.map.los_at(from, to)
                                        })
                                    })
                                })
                            }
                        })
            })
            .fold(0, |mask, target| mask | ACTS[target].bit());
        targets
            | if source < 2 && attack.weapon {
                self.object_mask(actor, range)
            } else {
                0
            }
    }

    pub(super) fn sanctuary_label(&self, action: Act) -> Option<String> {
        if action == Act::Pass {
            Some("Lose this attack or spell".into())
        } else if self.sanctuary_mask() & action.bit() != 0 {
            Some(format!(
                "Redirect to {}",
                self.creature_label(action as usize)
            ))
        } else {
            None
        }
    }

    pub(super) fn sanctuary_step(&mut self, action: Act) {
        let pending = self.pending_sanctuary.take().expect("pending Sanctuary");
        let actor = pending.source % 2;
        if action == Act::Pass {
            match pending.attempt {
                WardRedirect::Attack(_) => self.cancel_sanctuary_attack(pending.source),
                WardRedirect::SaveSpell(_) => {}
                WardRedirect::Ray(point) => self.sanctuary_finish_ray(actor, None, point),
                WardRedirect::Missile(count) => self.sanctuary_assign_darts(actor, None, count),
            }
            return;
        }

        let target = action as usize;
        let rejected = match pending.attempt {
            WardRedirect::Missile(count) => {
                self.sanctuary_missile_reject(actor, target, count, pending.rejected)
            }
            other => self.sanctuary_reject(pending.source, target, other, pending.rejected),
        };
        if rejected {
            if let Some((issuer, from_bow)) = pending.thrown_item {
                self.sanctuary_defer_throw(issuer, from_bow);
            }
            return;
        }
        match pending.attempt {
            WardRedirect::Attack(mut attack) => {
                let origin = self.fighter_point(actor);
                let landing = pending.thrown_item.and_then(|_| {
                    if target < 4 {
                        self.observed_footprint(actor, target)?
                            .squares()
                            .filter(|&point| {
                                self.map.los_at(origin, point) && origin.dist(point) <= 12
                            })
                            .min_by_key(|&point| origin.dist(point))
                    } else {
                        Some(self.observed_object(actor, target)?.point)
                    }
                });
                if let Some(point) = landing {
                    attack.long_range = origin.dist(point) > 4;
                }
                self.resolve_sanctuary_attack(actor, target, attack);
                if let Some(((issuer, from_bow), point)) = pending.thrown_item.zip(landing) {
                    self.gear_drop_weapon_instance(
                        actor,
                        issuer,
                        from_bow,
                        self.map.landing(point).cell,
                    );
                }
            }
            WardRedirect::SaveSpell(spell) => {
                self.resolve_targeted_save_spell(actor, target, spell)
            }
            WardRedirect::Ray(_) => {
                let point = self
                    .projectile_eligible(actor, target)
                    .expect("legal retarget");
                self.sanctuary_finish_ray(actor, Some(target), point);
            }
            WardRedirect::Missile(count) => self.sanctuary_assign_darts(actor, Some(target), count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::class::Class;
    use crate::arena2::entities::{Entity, EntityKind, FamiliarForm, Spirit};
    use crate::arena2::equipment::Carried;
    use crate::arena2::geometry::Pos;

    fn warded_match(seed: u64, opponent: Class) -> Battle {
        let mut battle = Battle::new(seed, [opponent, Class::Cleric], [5; 2]);
        battle.owner = 1;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle.f[1].gear.melee = Carried::Packed;
        battle.step(Act::Magic10).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        if battle.phase_id() == 48 {
            battle.step(ACTS[0]).unwrap();
        }
        if battle.phase_id() == 47 {
            battle.step(Act::Pass).unwrap(); // Decline the opponent's Counterspell.
        }
        assert_eq!(battle.magic_ward_source(1), Some(1));
        battle.owner = 0;
        battle.action = true;
        battle.bonus = true;
        battle
    }

    #[test]
    fn ward_failed_weapon_save_can_redirect_without_spending_another_attack() {
        let exercised = (0..64).any(|seed| {
            let mut battle = warded_match(seed, Class::Fighter);
            battle.fixture_companion(
                1,
                Entity::new(
                    EntityKind::Familiar {
                        form: FamiliarForm::Cat,
                        spirit: Spirit::Fey,
                    },
                    1,
                    Pos { x: 0, y: 1 },
                    10,
                ),
            );
            let original = battle.f[1].hp;
            battle.step(Act::Melee).unwrap();
            assert_eq!(battle.phase_id(), 14);
            battle.step(ACTS[1]).unwrap();
            if battle.phase_id() != RETARGET {
                return false;
            }
            assert_eq!(battle.mask() & ACTS[1].bit(), 0);
            assert_ne!(battle.mask() & ACTS[3].bit(), 0);
            let attacks = battle.attacks;
            assert_eq!(attacks, 1);
            battle.step(ACTS[3]).unwrap();
            assert_eq!(battle.f[1].hp, original);
            assert_eq!(battle.attacks, attacks);
            assert!(!battle.action);
            true
        });
        assert!(
            exercised,
            "a failed Sanctuary save should offer a different legal target"
        );
    }

    #[test]
    fn declining_magic_missile_redirection_keeps_the_paid_slot_and_loses_the_darts() {
        let exercised = (0..64).any(|seed| {
            let mut battle = warded_match(seed, Class::Wizard);
            let slots = battle.f[0].slots[0];
            let hp = battle.f[1].hp;
            battle.step(Act::Spell1).unwrap();
            if battle.phase_id() == 38 {
                battle.step(ACTS[0]).unwrap();
            }
            assert_eq!(battle.phase_id(), 80);
            battle.step(ACTS[17]).unwrap();
            if battle.phase_id() != RETARGET {
                return false;
            }
            assert_ne!(battle.mask() & ACTS[0].bit(), 0);
            battle.step(Act::Pass).unwrap();
            assert_eq!(battle.f[1].hp, hp);
            assert_eq!(battle.f[0].slots[0], slots - 1);
            assert_eq!(battle.phase_id(), 0);
            true
        });
        assert!(
            exercised,
            "Sanctuary should stop a damaging spell on a failed save"
        );
    }

    #[test]
    fn deflected_throw_keeps_the_item_until_a_replacement_target_is_chosen() {
        let exercised = (0..64).any(|seed| {
            let mut battle = warded_match(seed, Class::Bard);
            let replacement = Pos { x: 2, y: 0 };
            battle.fixture_companion(
                1,
                Entity::new(
                    EntityKind::Familiar {
                        form: FamiliarForm::Cat,
                        spirit: Spirit::Fey,
                    },
                    1,
                    replacement,
                    10,
                ),
            );
            battle.step(Act::Gear11).unwrap();
            battle.step(ACTS[1]).unwrap();
            if battle.phase_id() != RETARGET {
                return false;
            }
            assert_eq!(battle.f[0].gear.melee, Carried::Held);
            let mut declined = battle.clone();
            declined.step(Act::Pass).unwrap();
            assert_eq!(declined.f[0].gear.melee, Carried::Held);
            battle.step(ACTS[3]).unwrap();
            assert_eq!(battle.f[0].gear.melee, Carried::Ground(replacement));
            true
        });
        assert!(exercised);
    }

    #[test]
    fn ray_redirects_to_opposing_companion_and_spends_only_its_single_attack() {
        let exercised = (0..64).any(|seed| {
            let mut battle = warded_match(seed, Class::Wizard);
            battle.fixture_companion(
                1,
                Entity::new(
                    EntityKind::Familiar {
                        form: FamiliarForm::Cat,
                        spirit: Spirit::Fey,
                    },
                    1,
                    Pos { x: 2, y: 0 },
                    10,
                ),
            );
            let hp = battle.f[1].hp;
            battle.step(Act::Spell0).unwrap();
            assert_eq!(battle.phase_id(), 80);
            battle.step(ACTS[1]).unwrap();
            if battle.phase_id() != RETARGET {
                return false;
            }
            assert_ne!(battle.mask() & ACTS[3].bit(), 0);
            battle.step(ACTS[3]).unwrap();
            assert_eq!(battle.f[1].hp, hp);
            assert!(!matches!(battle.phase_id(), 80 | RETARGET));
            true
        });
        assert!(exercised);
    }

    #[test]
    fn non_damaging_hold_person_bypasses_sanctuary_and_leaves_the_ward_active() {
        let exercised = (0..64).any(|seed| {
            let mut battle = warded_match(seed, Class::Warlock);
            battle.step(Act::Magic4).unwrap();
            if battle.phase_id() == 38 {
                battle.step(ACTS[1]).unwrap();
            }
            assert_eq!(battle.phase_id(), 48);
            battle.step(ACTS[1]).unwrap();
            assert_ne!(battle.phase_id(), RETARGET);
            assert_eq!(battle.magic_ward_source(1), Some(1));
            battle.magic_paralyzed(1)
        });
        assert!(exercised);
    }

    #[test]
    fn targeted_save_spell_can_redirect_and_uses_the_new_target_for_riders() {
        let exercised = (0..64).any(|seed| {
            let mut battle = warded_match(seed, Class::Bard);
            let hp = battle.f[1].hp;
            battle.step(Act::Spell0).unwrap();
            assert_eq!(battle.phase_id(), 48);
            battle.step(ACTS[1]).unwrap();
            if battle.phase_id() != RETARGET {
                return false;
            }
            battle.step(ACTS[0]).unwrap();
            assert_eq!(battle.f[1].hp, hp);
            assert!(!battle.action);
            true
        });
        assert!(exercised);
    }

    #[test]
    fn ward_breaks_when_its_recipient_casts_a_non_damaging_spell() {
        let mut battle = warded_match(3, Class::Fighter);
        battle.owner = 1;
        battle.f[1].slot_turn = -1;
        battle.step(Act::Spell5).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        if battle.phase_id() == 48 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.magic_ward_source(1), None);
    }
}
