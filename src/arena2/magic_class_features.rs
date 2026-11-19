//! Non-spell Cleric and Bard features. Resource payments occur at commitment.
use super::*;

const TARGET_PHASE: i32 = 78;
const PARAMETER_PHASE: i32 = 79;
const FONT_PHASE: i32 = 97;
const CANCEL: Act = Act::Other8;
const COMMIT: Act = Act::Other7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Feature {
    SparkDamage,
    SparkHealing,
    PreserveLife,
    BardicInspiration,
    Font,
}

#[derive(Clone, Copy, Debug)]
struct Selection {
    feature: Feature,
    target: Option<usize>,
    healing: [i32; 4],
    readied: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ClassFeatureState {
    active: Option<Selection>,
    inspired_until: [i32; 4],
}

impl Battle {
    fn class_feature_state(&self, caster: usize) -> ClassFeatureState {
        self.f[caster].magic.class_features
    }

    fn class_feature_healing_capacity(&self, target: usize, bloodied_only: bool) -> i32 {
        let (hp, cap) = if target < 2 {
            (self.f[target].hp, self.magic_max_hp(target))
        } else {
            let Some(e) = self.entity(target) else {
                return 0;
            };
            (e.hp, e.max_hp())
        };
        if hp <= 0 {
            return 0;
        }
        if bloodied_only {
            (cap / 2 - hp).max(0)
        } else {
            (cap - hp).max(0)
        }
    }

    fn class_feature_target_mask(&self, i: usize, selection: Selection) -> u128 {
        let source = self.fighter_point(i);
        (0..4).fold(0, |mask, id| {
            if !self.rope_same_space(i, id) {
                return mask;
            }
            let Some(destination) = self.creature_point(id) else {
                return mask;
            };
            let distance = source.dist(destination);
            let valid = match selection.feature {
                Feature::SparkDamage => id != i && distance <= 6 && self.entity_visible(i, id),
                Feature::SparkHealing => {
                    id != i
                        && distance <= 6
                        && self.entity_visible(i, id)
                        && self.class_feature_healing_capacity(id, false) > 0
                }
                Feature::PreserveLife => {
                    distance <= 6
                        && (id == i || self.entity_visible(i, id))
                        && self.class_feature_healing_capacity(id, true) > selection.healing[id]
                }
                Feature::BardicInspiration => {
                    id != i
                        && distance <= 12
                        && (id < 2 || self.entity_visible(i, id))
                        && !self.bardic_inspiration_available(id)
                }
                Feature::Font => false,
            };
            if valid {
                mask | ACTS[id].bit()
            } else {
                mask
            }
        })
    }

    fn class_feature_for(&self, i: usize, action: Act) -> Option<Feature> {
        use Class::*;
        match (self.f[i].build.class, action) {
            (Cleric, Act::Special1) => Some(Feature::SparkDamage),
            (Cleric, Act::Magic20) => Some(Feature::SparkHealing),
            (Cleric, Act::Special2) if self.f[i].build.variant == 0 => Some(Feature::PreserveLife),
            (Bard, Act::Magic18) => Some(Feature::BardicInspiration),
            (Bard, Act::Special2) => Some(Feature::Font),
            _ => None,
        }
    }

    pub(in crate::arena2) fn class_magic_mask(&self, i: usize) -> u128 {
        let f = &self.f[i];
        [Act::Special1, Act::Special2, Act::Magic18, Act::Magic20]
            .into_iter()
            .filter(|&action| {
                let Some(feature) = self.class_feature_for(i, action) else {
                    return false;
                };
                let initial = Selection {
                    feature,
                    target: None,
                    healing: [0; 4],
                    readied: false,
                };
                match feature {
                    Feature::Font => {
                        f.build.level >= 5
                            && f.resource < f.build.casting.max(1)
                            && f.slots.iter().any(|&slots| slots > 0)
                    }
                    Feature::BardicInspiration => {
                        self.bonus
                            && f.resource > 0
                            && self.class_feature_target_mask(i, initial) != 0
                    }
                    Feature::SparkDamage | Feature::SparkHealing | Feature::PreserveLife => {
                        f.build.level
                            >= if feature == Feature::PreserveLife {
                                3
                            } else {
                                2
                            }
                            && self.action
                            && f.resource > 0
                            && self.class_feature_target_mask(i, initial) != 0
                    }
                }
            })
            .fold(0, |mask, action| mask | action.bit())
    }

    pub(in crate::arena2) fn class_magic_step(&mut self, i: usize, action: Act) -> bool {
        let Some(feature) = self.class_feature_for(i, action) else {
            return false;
        };
        self.begin_class_feature(i, feature, false);
        true
    }

    fn begin_class_feature(&mut self, i: usize, feature: Feature, readied: bool) {
        self.class_magic_request(
            i,
            Selection {
                feature,
                target: None,
                healing: [0; 4],
                readied,
            },
            if feature == Feature::Font {
                FONT_PHASE
            } else {
                TARGET_PHASE
            },
        );
    }

    pub(in crate::arena2) fn class_ready_mask(&self, i: usize) -> u128 {
        let fighter = &self.f[i];
        if fighter.build.class != Class::Cleric || fighter.build.level < 2 || fighter.resource <= 0
        {
            return 0;
        }
        let mut mask = Act::Special1.bit() | Act::Magic20.bit();
        if fighter.build.level >= 3 && fighter.build.variant == 0 {
            mask |= Act::Special2.bit();
        }
        mask
    }

    pub(in crate::arena2) fn class_ready_valid(&self, i: usize, action: Act) -> bool {
        if self.class_ready_mask(i) & action.bit() == 0 {
            return false;
        }
        let feature = self
            .class_feature_for(i, action)
            .expect("readied Channel Divinity");
        self.class_feature_target_mask(
            i,
            Selection {
                feature,
                target: None,
                healing: [0; 4],
                readied: true,
            },
        ) != 0
    }

    pub(in crate::arena2) fn class_ready_release(&mut self, i: usize, action: Act) {
        let feature = self
            .class_feature_for(i, action)
            .expect("readied Channel Divinity");
        self.begin_class_feature(i, feature, true);
    }

    fn class_magic_request(&mut self, i: usize, choice: Selection, phase: i32) {
        self.f[i].magic.class_features.active = Some(choice);
        self.request(i, phase);
    }

    fn preserve_available(&self, i: usize, choice: Selection) -> i32 {
        self.f[i].build.level * 5 - choice.healing.iter().sum::<i32>()
    }

    pub(in crate::arena2) fn class_magic_mask_phase(&self, i: usize, phase: i32) -> u128 {
        let Some(choice) = self.class_feature_state(i).active else {
            return 0;
        };
        let mask = match phase {
            TARGET_PHASE => {
                self.class_feature_target_mask(i, choice)
                    | if choice.feature == Feature::PreserveLife
                        && choice.healing.iter().any(|&n| n > 0)
                    {
                        COMMIT.bit()
                    } else {
                        0
                    }
            }
            PARAMETER_PHASE if choice.feature == Feature::PreserveLife => {
                let target = choice.target.expect("Preserve Life recipient");
                let cap =
                    self.class_feature_healing_capacity(target, true) - choice.healing[target];
                let max = cap.min(self.preserve_available(i, choice)).max(0);
                (1 << max) - 1
            }
            PARAMETER_PHASE => ACTS[0].bit() | ACTS[1].bit(),
            FONT_PHASE => (0..3)
                .filter(|&slot| self.f[i].slots[slot] > 0)
                .fold(0, |mask, slot| mask | ACTS[slot].bit()),
            _ => 0,
        };
        mask | CANCEL.bit()
    }

    pub(in crate::arena2) fn class_magic_label(
        &self,
        i: usize,
        phase: i32,
        input: Act,
    ) -> Option<String> {
        let choice = self.class_feature_state(i).active?;
        if input == CANCEL {
            return Some("Cancel class feature before payment".into());
        }
        if input == COMMIT {
            let total = choice.healing.iter().sum::<i32>();
            return Some(format!(
                "Use Channel Divinity and restore {total} HP as allocated"
            ));
        }
        let id = input as usize;
        Some(match phase {
            TARGET_PHASE => {
                let name = if id == i {
                    "yourself"
                } else if id < 2 {
                    "opposing duelist"
                } else {
                    self.entity(id)?.kind.name()
                };
                format!("Choose {name}")
            }
            PARAMETER_PHASE if choice.feature == Feature::PreserveLife => {
                format!("Allocate {} HP", id + 1)
            }
            PARAMETER_PHASE => if id == 0 {
                "Radiant damage"
            } else {
                "Necrotic damage"
            }
            .into(),
            FONT_PHASE => format!("Spend level {} slot to recover one Inspiration use", id + 1),
            _ => return None,
        })
    }

    fn class_feature_heal(&mut self, target: usize, amount: i32) {
        if target < 2 {
            self.magic_heal(target, amount);
        } else {
            self.entity_heal(target, amount);
        }
    }

    fn spend_channel_divinity(&mut self, i: usize, readied: bool) {
        if !readied {
            self.action = false;
        }
        self.f[i].resource -= 1;
    }

    fn complete_preserve_life(&mut self, i: usize, choice: Selection) {
        self.spend_channel_divinity(i, choice.readied);
        for (target, amount) in choice
            .healing
            .into_iter()
            .enumerate()
            .filter(|(_, n)| *n > 0)
        {
            self.class_feature_heal(target, amount);
        }
    }

    pub(in crate::arena2) fn class_magic_step_phase(&mut self, i: usize, phase: i32, input: Act) {
        let mut choice = self.f[i]
            .magic
            .class_features
            .active
            .take()
            .expect("class feature");
        if input == CANCEL {
            return;
        }
        let id = input as usize;
        match (phase, choice.feature) {
            (TARGET_PHASE, Feature::PreserveLife) if input == COMMIT => {
                self.complete_preserve_life(i, choice)
            }
            (TARGET_PHASE, Feature::PreserveLife | Feature::SparkDamage) => {
                choice.target = Some(id);
                self.class_magic_request(i, choice, PARAMETER_PHASE);
            }
            (PARAMETER_PHASE, Feature::PreserveLife) => {
                let target = choice.target.take().expect("Preserve Life recipient");
                choice.healing[target] += id as i32 + 1;
                if self.preserve_available(i, choice) == 0
                    || self.class_feature_target_mask(i, choice) == 0
                {
                    self.complete_preserve_life(i, choice);
                } else {
                    self.class_magic_request(i, choice, TARGET_PHASE);
                }
            }
            (TARGET_PHASE, Feature::SparkHealing) => {
                self.spend_channel_divinity(i, choice.readied);
                let amount = self.roll(8) + self.f[i].build.casting;
                self.class_feature_heal(id, amount);
            }
            (PARAMETER_PHASE, Feature::SparkDamage) => {
                self.spend_channel_divinity(i, choice.readied);
                self.spell_save_damage(
                    i,
                    CON,
                    (1, 8),
                    self.f[i].build.casting,
                    true,
                    if id == 0 {
                        DamageType::Radiant
                    } else {
                        DamageType::Necrotic
                    },
                    SpellRider::None,
                    choice.target.expect("Divine Spark recipient"),
                    false,
                );
            }
            (TARGET_PHASE, Feature::BardicInspiration) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].magic.class_features.inspired_until[id] = self.turns + 1200;
                if id == i + 2 {
                    self.f[i].magic.ally_inspiration_until = self.turns + 1200;
                }
            }
            (FONT_PHASE, Feature::Font) => {
                self.f[i].slots[id] -= 1;
                self.f[i].resource += 1;
            }
            _ => unreachable!("class feature phase validated"),
        }
    }

    pub(in crate::arena2) fn has_bardic_inspiration(&self) -> bool {
        self.f.iter().any(|source| {
            source
                .magic
                .class_features
                .inspired_until
                .iter()
                .any(|&until| until > self.turns)
        })
    }

    pub(in crate::arena2) fn bardic_inspiration_available(&self, recipient: usize) -> bool {
        self.f
            .iter()
            .any(|source| source.magic.class_features.inspired_until[recipient] > self.turns)
    }

    pub(in crate::arena2) fn valor_inspiration_available(&self, recipient: usize) -> bool {
        self.f.iter().any(|source| {
            crate::arena2::bard_ranger::is_valor(source.build)
                && source.magic.class_features.inspired_until[recipient] > self.turns
        })
    }

    pub(in crate::arena2) fn spend_bardic_inspiration(&mut self, recipient: usize) -> i32 {
        let Some(source) = (0..2).find(|&source| {
            self.f[source].magic.class_features.inspired_until[recipient] > self.turns
        }) else {
            return 0;
        };
        self.f[source].magic.class_features.inspired_until[recipient] = 0;
        if recipient == source + 2 {
            self.f[source].magic.ally_inspiration_until = 0;
        }
        self.roll(if self.f[source].build.level >= 5 {
            8
        } else {
            6
        })
    }

    pub fn class_feature_observation(&self, i: usize) -> [f32; 8] {
        use crate::arena2::target_ids::relative_id;

        let mut data = [0.; 8];
        let state = self.class_feature_state(i);
        data[7] = u8::from(self.bardic_inspiration_available(i)).into();
        if let Some(selection) = state.active {
            data[0] = (selection.feature as usize + 1) as f32 / 5.;
            data[1] = selection
                .target
                .map_or(0., |target| (relative_id(i, target) + 1) as f32 / 4.);
            for relative in 0..4 {
                data[2 + relative] = selection.healing[relative_id(i, relative)] as f32 / 25.;
            }
            data[6] = if selection.feature == Feature::PreserveLife {
                self.preserve_available(i, selection) as f32 / 25.
            } else {
                0.
            };
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encounter(class: Class, level: i32) -> Battle {
        let mut b = Battle::new(1, [class, Class::Fighter], [level; 2]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 1, y: 0 };
        b
    }

    #[test]
    fn preserve_life_allocates_distinct_creatures_and_never_crosses_half() {
        let mut b = encounter(Class::Cleric, 3);
        b.f[0].hp = b.magic_max_hp(0) / 2 - 4;
        b.f[1].hp = b.magic_max_hp(1) / 2 - 7;
        let start = [b.f[0].hp, b.f[1].hp];
        let charges = b.f[0].resource;
        b.step(Act::Special2).unwrap();
        assert_eq!(b.phase_id(), TARGET_PHASE);
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.phase_id(), PARAMETER_PHASE);
        assert_eq!(b.mask() & ACTS[4].bit(), 0, "no allocation beyond half HP");
        b.step(ACTS[3]).unwrap();
        assert_eq!(b.f[0].hp, start[0]);
        assert_eq!(b.f[0].resource, charges);
        b.step(ACTS[1]).unwrap();
        b.step(ACTS[1]).unwrap();
        b.step(COMMIT).unwrap();
        assert_eq!([b.f[0].hp, b.f[1].hp], [start[0] + 4, start[1] + 2]);
        assert_eq!(b.f[0].resource, charges - 1);
        assert!(!b.action);
    }

    #[test]
    fn spark_heal_spends_divinity_once_and_cannot_target_self() {
        let mut b = encounter(Class::Cleric, 3);
        b.f[0].hp = 2;
        b.f[1].hp = 2;
        let charges = b.f[0].resource;
        let slots = b.f[0].slots;
        b.step(Act::Magic20).unwrap();
        assert_eq!(b.mask() & ACTS[0].bit(), 0);
        b.step(ACTS[1]).unwrap();
        assert!((4..=11).contains(&(b.f[1].hp - 2)));
        assert_eq!(b.f[0].hp, 2);
        assert_eq!(b.f[0].resource, charges - 1);
        assert_eq!(b.f[0].slots, slots);
    }

    #[test]
    fn spark_damage_explicit_radiant_and_necrotic_choices() {
        for choice in 0..2 {
            let mut b = encounter(Class::Cleric, 3);
            let charges = b.f[0].resource;
            let initial = b.f[1].hp;
            b.step(Act::Special1).unwrap();
            b.step(ACTS[1]).unwrap();
            assert_eq!(b.phase_id(), PARAMETER_PHASE);
            assert_eq!(
                b.choice_label(0, ACTS[choice]),
                if choice == 0 {
                    "Radiant damage"
                } else {
                    "Necrotic damage"
                }
            );
            b.step(ACTS[choice]).unwrap();
            assert!(b.f[1].hp < initial);
            assert_eq!(b.f[0].resource, charges - 1);
        }
    }

    #[test]
    fn readied_divine_spark_preserves_the_current_duelists_action() {
        let mut b = encounter(Class::Cleric, 3);
        b.owner = 1;
        b.f[1].hp = 1;
        let uses = b.f[0].resource;
        assert!(b.class_ready_valid(0, Act::Magic20));
        b.class_ready_release(0, Act::Magic20);
        assert_eq!(b.actor(), Some(0));
        assert_eq!(b.owner, 1);
        b.step(ACTS[1]).unwrap();
        assert!(b.f[1].hp > 1);
        assert_eq!(b.f[0].resource, uses - 1);
        assert!(b.action);
    }

    #[test]
    fn disciple_of_life_adds_two_plus_slot_to_healing_not_channel_divinity() {
        let mut b = encounter(Class::Cleric, 3);
        b.f[0].hp = 1;
        let mut dice = b.clone();
        let expected = 1 + dice.roll_many(2, 8) + b.f[0].build.casting + 3;
        b.resolve_spell(0, Spell::CureWounds, 1, false);
        assert_eq!(b.f[0].hp, expected.min(b.magic_max_hp(0)));
    }

    #[test]
    fn cutting_words_can_react_to_an_opponent_successful_ability_check() {
        let mut b = encounter(Class::Bard, 3);
        b.owner = 1;
        b.f[1].grappled = Some(1);
        let uses = b.f[0].resource;
        b.step(Act::EscapeAthletics).unwrap();
        assert_eq!(b.phase_id(), 15);
        assert_eq!(b.actor(), Some(0));
        assert!(b.choice_label(0, Act::Defend).contains("Cutting Words"));
        b.step(Act::Defend).unwrap();
        assert!(!b.f[0].reaction);
        assert_eq!(b.f[0].resource, uses - 1);
    }

    #[test]
    fn cutting_words_can_react_to_an_opponent_successful_attack_roll() {
        let mut found = false;
        for seed in 1..=100 {
            let mut b = Battle::new(seed, [Class::Bard, Class::Fighter], [3; 2]);
            b.owner = 1;
            b.f[0].pos = Pos { x: 0, y: 0 };
            b.f[1].pos = Pos { x: 1, y: 0 };
            b.step(Act::Melee).unwrap();
            if b.phase_id() != 2 || b.actor() != Some(0) {
                continue;
            }
            let uses = b.f[0].resource;
            assert!(b.choice_label(0, Act::Defend).contains("Cutting Words"));
            b.step(Act::Defend).unwrap();
            assert!(!b.f[0].reaction);
            assert_eq!(b.f[0].resource, uses - 1);
            found = true;
            break;
        }
        assert!(found);
    }

    #[test]
    fn font_accepts_selected_slot_without_consuming_action_or_cast_allowance() {
        let mut b = encounter(Class::Bard, 5);
        b.f[0].resource -= 1;
        let charges = b.f[0].resource;
        let slots = b.f[0].slots;
        b.step(Act::Special2).unwrap();
        assert_eq!(b.phase_id(), FONT_PHASE);
        b.step(ACTS[1]).unwrap();
        assert!(b.action && b.bonus);
        assert_eq!(b.f[0].resource, charges + 1);
        assert_eq!(b.f[0].slots, [slots[0], slots[1] - 1, slots[2]]);
        assert!(b.can_slot(0, 1));
    }

    #[test]
    fn bardic_inspiration_can_target_another_creature_and_is_single_use() {
        let mut b = encounter(Class::Bard, 5);
        let charges = b.f[0].resource;
        b.step(Act::Magic18).unwrap();
        assert_eq!(b.mask() & ACTS[0].bit(), 0);
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.f[0].resource, charges - 1);
        assert!(!b.bonus);
        assert!(b.bardic_inspiration_available(1));
        assert!((1..=8).contains(&b.spend_bardic_inspiration(1)));
        assert_eq!(b.spend_bardic_inspiration(1), 0);
    }
}
