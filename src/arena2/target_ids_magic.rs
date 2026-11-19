//! Perspective-relative spell targets and equipment selections.
use super::ready::{OptionKind, ReadyMode};
use super::{Battle, MagicChoice, Spell};
use crate::arena2::objects::{END_OBJECTS, FIRST_OBJECT};
use crate::arena2::target_ids::TargetPermutation;

fn swap_pairs(ids: &mut TargetPermutation, start: usize, end: usize) {
    for raw in (start..end).step_by(2) {
        ids.swap(raw, raw + 1);
    }
}

fn map_recipient_sets(
    ids: &mut TargetPermutation,
    actor: usize,
    first_subset: usize,
    raw_id: impl Fn(usize) -> usize,
) {
    for subset in first_subset..16 {
        let relative = if actor == 0 {
            subset
        } else {
            ((subset & 0b0101) << 1) | ((subset & 0b1010) >> 1)
        };
        let choice = if relative.is_power_of_two() {
            relative.trailing_zeros() as usize
        } else {
            32 + relative
        };
        ids.set(raw_id(subset), choice);
    }
}

fn swap_form_equipment_sets(ids: &mut TargetPermutation) {
    // Slots 0–3 encode each issuer's melee and ranged weapon. The
    // remaining bits already represent the actor's shield and armor.
    for subset in 0..64 {
        let relative =
            (subset & 0b110000) | ((subset & 0b000011) << 2) | ((subset & 0b001100) >> 2);
        if subset < relative {
            ids.swap(subset, relative);
        }
    }
}

fn swap_dispel_targets(ids: &mut TargetPermutation) {
    swap_pairs(ids, 0, END_OBJECTS);
    // Carried focus, ongoing area effect and Zone of Truth, respectively.
    ids.swap(28, 29);
    ids.swap(32, 33);
    ids.swap(34, 35);
}

impl Battle {
    pub(in crate::arena2) fn magic_target_id_permutation(&self, actor: usize) -> TargetPermutation {
        let mut ids = TargetPermutation::identity();
        let east = actor == 1;
        let magic = &self.f[actor].magic;
        match self.phase_id() {
            46 if east => swap_dispel_targets(&mut ids),
            173 if east => swap_pairs(&mut ids, 0, 4),
            48 => {
                match magic.choice {
                    Some(MagicChoice::Allies(action)) => {
                        if self.prepared_spell(actor, action)
                            == Some(Spell::LesserRestoration)
                        {
                            for raw in 0..4 {
                                let relative = raw ^ actor;
                                ids.set(1 << raw, relative);
                                ids.set(16 + (1 << raw), 16 + relative);
                            }
                        } else {
                            // The original all-four code is 31 because 15 is Pass.
                            map_recipient_sets(&mut ids, actor, 1, |set| {
                                if set == 15 {
                                    31
                                } else {
                                    set
                                }
                            });
                        }
                    }
                    Some(MagicChoice::GroupTargets(_)) => {
                        map_recipient_sets(&mut ids, actor, 1, |set| 32 + set);
                    }
                    Some(
                        MagicChoice::AllyConsent { .. }
                        | MagicChoice::Creature(_)
                        | MagicChoice::HeightenedTarget { .. }
                        | MagicChoice::SpiritualStrike,
                    ) if east => swap_pairs(&mut ids, 0, FIRST_OBJECT),
                    Some(MagicChoice::LightTarget(_)) if east => {
                        swap_pairs(&mut ids, FIRST_OBJECT, END_OBJECTS);
                    }
                    _ => {}
                }
            }
            77 if east => swap_pairs(&mut ids, FIRST_OBJECT, END_OBJECTS),
            78 | 94 | 95 | 104 if east => swap_pairs(&mut ids, 0, FIRST_OBJECT),
            80 if east => {
                swap_pairs(&mut ids, 0, END_OBJECTS);
                // Magic Missile's "all remaining darts" variants.
                swap_pairs(&mut ids, 16, 20);
            }
            92 | 93 | 177 => map_recipient_sets(&mut ids, actor, 0, |set| set),
            100 if east => swap_form_equipment_sets(&mut ids),
            102 if east => swap_pairs(&mut ids, 0, END_OBJECTS),
            108 => match magic.ready_state.mode {
                Some(ReadyMode::Options(OptionKind::Creatures | OptionKind::Consent { .. }))
                    if east =>
                {
                    swap_pairs(&mut ids, 0, FIRST_OBJECT);
                }
                Some(ReadyMode::Options(OptionKind::Allies | OptionKind::Group)) => {
                    map_recipient_sets(&mut ids, actor, 1, |set| 32 + set);
                }
                Some(ReadyMode::Options(OptionKind::Dispel)) if east => {
                    swap_dispel_targets(&mut ids)
                }
                Some(ReadyMode::Options(OptionKind::LightObject)) if east => {
                    swap_pairs(&mut ids, FIRST_OBJECT, END_OBJECTS);
                }
                _ => {}
            },
            _ => {}
        }
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::entities::{Entity, EntityKind, FamiliarForm, Spirit};
    use crate::arena2::equipment::Carried;
    use crate::arena2::geometry::Pos;
    use crate::arena2::{class::Class, Act, ACTS};

    fn cleric_encounter(actor: usize) -> Battle {
        let mut classes = [Class::Fighter; 2];
        classes[actor] = Class::Cleric;
        let mut battle = Battle::new(1, classes, [5; 2]);
        battle.owner = actor;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle.f[actor].gear.melee = Carried::Packed;
        battle
    }

    #[test]
    fn shield_of_faith_uses_the_same_single_recipient_ids_for_both_actors() {
        for actor in 0..2 {
            let mut battle = cleric_encounter(actor);
            battle.step(Act::Magic16).unwrap();
            if battle.phase_id() == 38 {
                battle.step(ACTS[0]).unwrap();
            }
            assert_eq!(battle.phase_id(), 48);
            assert_eq!(battle.choice_label(actor, ACTS[0]), "Target self");
            assert_eq!(
                battle.choice_label(actor, ACTS[1]),
                "Target opposing adventurer"
            );
            assert_eq!(
                battle.mask(),
                ACTS[0].bit() | ACTS[1].bit() | Act::Pass.bit()
            );
            battle.step(ACTS[0]).unwrap();
            assert_eq!(
                battle.magic_buff_source(actor, super::super::Concentration::ShieldOfFaith),
                Some(actor),
            );
            assert_eq!(
                battle.magic_buff_source(1 - actor, super::super::Concentration::ShieldOfFaith),
                None,
            );
        }
    }

    #[test]
    fn mass_healing_word_group_ids_follow_the_caster_in_both_seats() {
        for actor in 0..2 {
            let mut battle = cleric_encounter(actor);
            battle.fixture_companion(
                actor,
                Entity::new(
                    EntityKind::Familiar {
                        form: FamiliarForm::Cat,
                        spirit: Spirit::Fey,
                    },
                    actor,
                    Pos { x: 0, y: 1 },
                    10,
                ),
            );
            battle.f[0].hp = 1;
            battle.f[1].hp = 1;
            battle.entity_mut(actor + 2).unwrap().hp = 1;
            battle.step(Act::Magic3).unwrap();
            assert_eq!(battle.phase_id(), 48);
            let group = ACTS[32 + 0b0110]; // Opponent and own companion.
            assert_ne!(battle.mask() & group.bit(), 0);
            battle.step(group).unwrap();
            assert_eq!(battle.f[actor].hp, 1);
            assert!(battle.f[1 - actor].hp > 1);
            assert!(battle.entity(actor + 2).unwrap().hp > 1);
        }
    }

    #[test]
    fn support_subset_and_consent_have_distinct_encodings() {
        let mut battle = Battle::new(1, [Class::Fighter, Class::Cleric], [5; 2]);
        battle.request(1, 48);
        battle.f[1].magic.choice = Some(MagicChoice::Allies(Act::Magic10));
        let ids = battle.magic_target_id_permutation(1);
        assert_eq!(ids.encode(0b1010), 32 + 0b0101);
        assert_eq!(ids.decode(32 + 0b0101), 0b1010);
        assert_eq!(ids.encode(31), 47);
        assert_eq!(ids.encode(Act::Pass as usize), Act::Pass as usize);

        battle.f[1].magic.choice = Some(MagicChoice::Allies(Act::Magic2));
        let ids = battle.magic_target_id_permutation(1);
        assert_eq!(ids.encode(16 + 0b0010), 16);
        assert_eq!(ids.decode(16 + 1), 16 + 0b0001);

        battle.f[1].magic.choice = Some(MagicChoice::AllyConsent {
            caster: 0,
            action: Act::Magic10,
            subset: 0b1010,
        });
        let ids = battle.magic_target_id_permutation(1);
        assert_eq!(ids.encode(1), 0);
        assert_eq!(ids.encode(3), 2);
        assert_eq!(ids.encode(Act::Defend as usize), Act::Defend as usize);
        assert_eq!(ids.encode(Act::Pass as usize), Act::Pass as usize);
    }

    #[test]
    fn readied_spell_parameters_switch_only_target_domains() {
        let mut battle = Battle::new(1, [Class::Fighter, Class::Wizard], [5; 2]);
        battle.request(1, 108);
        for (kind, raw, public) in [
            (OptionKind::Creatures, 3, 2),
            (OptionKind::Group, 32 + 0b1010, 32 + 0b0101),
            (OptionKind::Allies, 32 + 0b0001, 1),
            (OptionKind::LightObject, 1, 1),
            (OptionKind::LightObject, 5, 4),
            (OptionKind::Dispel, 29, 28),
            (OptionKind::Dispel, 33, 32),
            (OptionKind::Dispel, 35, 34),
            (OptionKind::CureRoute, 1, 1),
            (OptionKind::Element, 1, 1),
        ] {
            battle.f[1].magic.ready_state.mode = Some(ReadyMode::Options(kind));
            let ids = battle.magic_target_id_permutation(1);
            assert_eq!(ids.encode(raw), public, "{kind:?}");
            assert_eq!(ids.decode(public), raw, "{kind:?}");
        }
    }

    #[test]
    fn magic_missile_and_wild_shape_preserve_choice_modifiers() {
        let mut battle = Battle::new(1, [Class::Fighter, Class::Wizard], [5; 2]);
        battle.request(1, 80);
        let ids = battle.magic_target_id_permutation(1);
        assert_eq!(ids.encode(1), 0);
        assert_eq!(ids.encode(19), 18);
        assert_eq!(ids.encode(126), 126);

        battle.request(1, 100);
        let ids = battle.magic_target_id_permutation(1);
        assert_eq!(ids.encode(0b011001), 0b010110);
        assert_eq!(ids.decode(0b010110), 0b011001);
        assert_eq!(ids.encode(0b110000), 0b110000);
    }
}
