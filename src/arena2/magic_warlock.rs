//! Fiend features and learned invocations of the arena Warlock.
use super::{Act, Battle, Class, Hit, Spell, TempSource};
use crate::arena2::{footprint::Footprint, geometry::Point};

impl Battle {
    pub(super) fn perform_pact_rite(&mut self, i: usize) {
        self.action = false;
        let f = &mut self.f[i];
        let m = &mut f.magic;
        m.rite_actions = if m.awaiting_rite == Some(self.turns - 2) {
            m.rite_actions + 1
        } else {
            1
        };
        m.awaiting_rite = Some(self.turns);
        if m.rite_actions == 10 {
            if let Some(tier) = f.build.slots.iter().rposition(|&count| count > 0) {
                f.slots[tier] =
                    (f.slots[tier] + (f.build.slots[tier] + 1) / 2).min(f.build.slots[tier]);
            }
            m.awaiting_rite = None;
            m.finished_rite = true;
        }
    }

    pub(in crate::arena2) fn warlock_rite_progress(&self, i: usize) -> (u8, bool) {
        let m = &self.f[i].magic;
        (
            if m.awaiting_rite.is_some() {
                m.rite_actions
            } else {
                0
            },
            m.finished_rite,
        )
    }

    pub(in crate::arena2) fn magic_can_repel(&self, hit: Hit) -> bool {
        hit.attacker < 2
            && hit.defender < 4
            && hit.attacker != hit.defender
            && !hit.weapon
            && self.f[hit.attacker].build.class == Class::Warlock
            && self.f[hit.attacker].build.level >= 5
            && self.f[hit.attacker].magic.attack == Some(Spell::EldritchBlast)
            && self.combatant_size(hit.defender) <= 3
    }

    pub(in crate::arena2) fn magic_repelling_choice(&self, hit: Hit, choice: Act) -> Option<i32> {
        self.magic_can_repel(hit).then_some(match choice {
            Act::Rider => 2,
            Act::N => 1,
            _ => 0,
        })
    }

    pub(in crate::arena2) fn magic_repel(&mut self, hit: Hit, spaces: i32) {
        self.f[hit.attacker].magic.repelling_pending = spaces;
    }

    pub(in crate::arena2) fn magic_apply_repel(&mut self, hit: Hit) {
        if hit.attacker >= 2 {
            return;
        }
        let distance = std::mem::take(&mut self.f[hit.attacker].magic.repelling_pending);
        if distance == 0 || hit.defender < 2 && self.f[hit.defender].hp <= 0 {
            return;
        }
        let origin = self.fighter_point(hit.attacker);
        let Some(mut from) = self.creature_point(hit.defender) else {
            return;
        };
        let dx = (from.cell.x - origin.cell.x).signum();
        let dy = (from.cell.y - origin.cell.y).signum();
        let dz = (from.altitude - origin.altitude).signum();
        for _ in 0..distance {
            let to = Point {
                cell: super::Pos {
                    x: from.cell.x + dx,
                    y: from.cell.y + dy,
                },
                altitude: from.altitude + dz,
            };
            if !self.map.can_aim_point(from, to)
                || self
                    .footprint_fit_kind(
                        hit.defender,
                        to,
                        self.combatant_size(hit.defender),
                        false,
                        true,
                    )
                    .is_none()
            {
                break;
            }
            if hit.defender < 2 {
                self.commit_forced_move(hit.defender, to);
            } else if !self.entity_force_move(hit.defender, to) {
                break;
            }
            if self.creature_point(hit.defender) != Some(to) {
                break;
            }
            from = to;
        }
    }

    pub(in crate::arena2) fn magic_blessing_at_zero(
        &mut self,
        source: Option<usize>,
        target: usize,
        before: Option<Footprint>,
    ) {
        let Some(before) = before else {
            return;
        };
        let Some(source) = source else {
            return;
        };
        if target >= 4
            || if target < 2 {
                self.f[target].hp > 0
            } else {
                self.entity(target).is_some()
            }
        {
            return;
        }
        self.magic_mark_target_downed(target);
        let warlock = 1 - target % 2;
        let f = &self.f[warlock];
        if !self.is_subclass(warlock, Class::Warlock, 0) || f.build.level < 3 || f.hp == 0 {
            return;
        }
        if source == warlock
            || self
                .footprint(warlock)
                .is_some_and(|body| body.distance(before) <= 2)
        {
            let amount = (self.f[warlock].build.casting + self.f[warlock].build.level).max(1);
            self.grant_temporary_hp(warlock, amount, TempSource::DarkOnesBlessing);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{
        entities::{Entity, EntityKind, FamiliarForm, Spirit},
        equipment::DamageType,
        geometry::Pos,
        ACTS,
    };

    fn duel(seed: u64, level: i32) -> Battle {
        let mut b = Battle::new(seed, [Class::Warlock, Class::Fighter], [level, level]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 2, y: 0 };
        b
    }

    fn familiar(b: &mut Battle, side: usize, p: Pos) {
        b.fixture_companion(
            side,
            Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Owl,
                    spirit: Spirit::Fey,
                },
                side,
                p,
                0,
            ),
        );
    }

    #[test]
    fn invocation_push_can_be_declined_or_set_to_five_or_ten_feet_after_a_beam_hits() {
        let mut seen = false;
        for seed in 0..48 {
            let mut b = duel(seed, 5);
            b.step(Act::Spell0).unwrap();
            b.step(ACTS[1]).unwrap();
            if b.phase_id() != 3 {
                continue;
            }
            assert_eq!(b.actor(), Some(0));
            for (choice, distance) in [(Act::Pass, 0), (Act::N, 1), (Act::Rider, 2)] {
                let mut copy = b.clone();
                assert_ne!(copy.mask() & choice.bit(), 0);
                copy.step(choice).unwrap();
                assert_eq!(
                    copy.f[1].pos,
                    Pos {
                        x: 2 + distance,
                        y: 0
                    }
                );
                assert_eq!(copy.phase_id(), 80);
            }
            seen = true;
            break;
        }
        assert!(seen);
    }

    #[test]
    fn blessing_checks_killer_and_three_dimensional_range_including_companions() {
        for (level, killer, pos, altitude, expected) in [
            (3, 0, Pos { x: 6, y: 0 }, 0, true),
            (3, 1, Pos { x: 1, y: 0 }, 0, true),
            (3, 1, Pos { x: 6, y: 0 }, 0, false),
            (3, 2, Pos { x: 1, y: 0 }, 4, false),
            (2, 0, Pos { x: 1, y: 0 }, 0, false),
        ] {
            let mut b = duel(0, level);
            familiar(&mut b, 1, pos);
            b.entity_mut(3).unwrap().altitude = altitude;
            b.apply_spell_damage(killer, 3, 40, DamageType::Force);
            assert!(b.entity(3).is_none());
            assert_eq!(
                b.f[0].temp,
                if expected {
                    b.f[0].build.casting + level
                } else {
                    0
                }
            );
        }
    }

    #[test]
    fn misty_step_uses_selected_visible_height_and_never_spends_movement_or_provokes() {
        let mut b = duel(5, 4);
        b.f[1].pos = Pos { x: 1, y: 0 };
        let speed = b.movement_left();
        b.step(Act::Magic6).unwrap();
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[4]).unwrap();
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.phase_id(), 0);
        assert_eq!(b.f[0].pos, Pos { x: 4, y: 0 });
        assert_eq!(b.movement_left(), speed);
        assert!(b.f[1].reaction && b.action && !b.bonus);
        assert_eq!(b.f[0].slots[1], 1);
    }

    #[test]
    fn interrupted_pact_rite_restarts_and_fiendish_vigor_uses_no_pact_slot() {
        let mut b = duel(1, 3);
        b.f[0].slots.fill(0);
        b.step(Act::Magic21).unwrap();
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        assert_eq!(b.warlock_rite_progress(0), (1, false));
        b.step(Act::Dodge).unwrap();
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        assert_eq!(b.warlock_rite_progress(0), (0, false));
        b.step(Act::Special1).unwrap();
        assert_eq!(b.f[0].slots, [0, 0, 0]);
        assert_eq!(b.f[0].temp, 12);
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        b.step(Act::Magic21).unwrap();
        assert_eq!(b.warlock_rite_progress(0), (1, false));
    }
}
