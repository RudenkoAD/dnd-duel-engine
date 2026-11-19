//! Transactional continuations for optional choices inside synchronous effects.
//!
//! Run an action against a private trial state only if a choice may be needed.
//! When the first unanswered choice appears, restore the pre-action state and
//! expose that prompt alone. Replay the action with all accepted answers in
//! roll order, preserving dice, resource costs and other combat interruptions.

use super::{
    battle::Battle,
    temp_hp::{TempOffer, TEMP_CHOICE_PHASE},
    Act,
};

pub(super) const SAVE_CHOICE_PHASE: i32 = 101;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SavePrompt {
    recipient: usize,
    stat: usize,
    total: i32,
    dc: i32,
    concentration: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prompt {
    Save(SavePrompt),
    Temp(TempOffer),
    Arcane { caster: usize, roller: usize, portent: bool, advantage: bool, dice: [i32; 2] },
    Psychic { caster: usize, spell: super::magic::Spell },
    ProtectiveField { caster: usize, target: usize, damage: i32 },
}

impl Prompt {
    fn recipient(self) -> usize {
        match self {
            Self::Save(save) => save.recipient,
            Self::Temp(offer) => offer.recipient,
            Self::Arcane { caster, .. } | Self::Psychic { caster, .. }
            | Self::ProtectiveField { caster, .. } => caster,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Answer {
    prompt: Prompt,
    action: Act,
    drop_concentration: bool,
    auto_pass: bool,
}

#[derive(Clone, Debug)]
struct Pending {
    action: Act,
    drop_at_control_window: bool,
    prompt: Prompt,
    answers: Vec<Answer>,
    can_drop: bool,
    dropped: bool,
}

#[derive(Clone, Debug)]
struct Replay {
    answers: Vec<Answer>,
    cursor: usize,
    undecided: Option<(Prompt, bool)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SaveChoice {
    pending: Option<Pending>,
    replay: Option<Replay>,
    pub(super) concentration_menu: bool,
}

impl Battle {
    pub(super) fn step_raw_action(&mut self, choice: Act) -> Result<(), &'static str> {
        if self.save_choice.concentration_menu {
            match choice {
                Act::Pass => {
                    self.save_choice.concentration_menu = false;
                    return Ok(());
                }
                Act::DropConcentration => {
                    self.save_choice.concentration_menu = false;
                    return self.confirm_drop_concentration();
                }
                _ => return Err("illegal concentration response"),
            }
        }
        if self.concentration_menu_action() == Some(choice) {
            self.save_choice.concentration_menu = true;
            return Ok(());
        }
        if choice == Act::DropConcentration && self.optional_concentrator().is_some() {
            return self.confirm_drop_concentration();
        }
        self.step_with_save_choices(choice, Self::step_core)
    }

    fn confirm_drop_concentration(&mut self) -> Result<(), &'static str> {
        if let Some(pending) = &mut self.save_choice.pending {
            // Apply the drop at the prompt's position in the action replay.
            pending.dropped = true;
            if matches!(pending.prompt, Prompt::Save(save)
                if save.concentration && save.recipient < 2)
            {
                return self.step_with_save_choices(Act::Pass, Self::step_core);
            }
            return Ok(());
        }
        self.step_with_effect_choices(Act::DropConcentration, true, Self::step_core)
    }

    pub(super) fn concentration_dropped_in_prompt(&self, actor: usize) -> bool {
        self.save_choice.pending.as_ref().is_some_and(|pending| {
            pending.dropped && pending.prompt.recipient() % 2 == actor
        })
    }

    pub(super) fn replay_concentrator(&self) -> Option<usize> {
        let pending = self.save_choice.pending.as_ref()?;
        (pending.can_drop && !pending.dropped).then(|| pending.prompt.recipient() % 2)
    }

    fn step_with_save_choices(
        &mut self,
        choice: Act,
        step: impl FnOnce(&mut Self, Act) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        self.step_with_effect_choices(choice, false, step)
    }

    fn step_with_effect_choices(
        &mut self,
        choice: Act,
        direct_drop: bool,
        step: impl FnOnce(&mut Self, Act) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        let (action, drop_at_control_window, answers) = if let Some(pending) = &self.save_choice.pending {
            if self.save_choice_mask().is_none_or(|mask| mask & choice.bit() == 0) {
                return Err("illegal effect response");
            }
            let mut answers = pending.answers.clone();
            answers.push(Answer {
                prompt: pending.prompt,
                action: choice,
                drop_concentration: pending.dropped,
                auto_pass: pending.dropped && matches!(pending.prompt, Prompt::Save(save)
                    if save.concentration && save.recipient < 2),
            });
            (pending.action, pending.drop_at_control_window, answers)
        } else if self.has_bardic_inspiration() || self.may_need_temp_choice() || self.may_need_arcane_roll()
            || (0..2).any(|i| self.fighter_has_field(i)) {
            (choice, direct_drop, Vec::new())
        } else if direct_drop {
            self.drop_at_control_window();
            return Ok(());
        } else {
            return step(self, choice);
        };

        let original = self.clone();
        self.save_choice.pending = None;
        self.save_choice.replay = Some(Replay {
            answers,
            cursor: 0,
            undecided: None,
        });
        let result = if drop_at_control_window {
            self.drop_at_control_window();
            Ok(())
        } else {
            step(self, action)
        };
        let replay = self
            .save_choice
            .replay
            .take()
            .expect("active effect replay");
        if result.is_err() {
            *self = original;
            return result;
        }
        debug_assert_eq!(replay.cursor, replay.answers.len());
        if let Some((prompt, can_drop)) = replay.undecided {
            *self = original;
            self.save_choice.pending = Some(Pending {
                action,
                drop_at_control_window,
                prompt,
                answers: replay.answers,
                can_drop,
                dropped: false,
            });
        } else {
            self.save_choice_complete_decisions(replay.answers.iter().map(|answer| {
                usize::from(!answer.auto_pass) + usize::from(answer.drop_concentration)
            }).sum());
        }
        Ok(())
    }

    fn record_effect_choice(&mut self, prompt: Prompt) -> Option<Act> {
        let actor = prompt.recipient() % 2;
        let can_drop = self.f[actor].concentration.is_some();
        let replay = self.save_choice.replay.as_mut()?;
        if replay.undecided.is_some() {
            return None;
        }
        if let Some(answer) = replay.answers.get(replay.cursor).copied() {
            debug_assert_eq!(
                answer.prompt, prompt,
                "effect replay changed an answered prompt"
            );
            replay.cursor += 1;
            if answer.drop_concentration {
                self.drop_concentration(actor);
            }
            Some(answer.action)
        } else {
            replay.undecided = Some((prompt, can_drop));
            None
        }
    }

    pub(super) fn fighter_field_choice(&mut self, caster: usize, target: usize, damage: i32) -> bool {
        self.record_effect_choice(Prompt::ProtectiveField { caster, target, damage }) == Some(Act::Defend)
    }

    pub(super) fn decide_temp_replacement(&mut self, offer: TempOffer) -> bool {
        self.record_effect_choice(Prompt::Temp(offer)) == Some(Act::Defend)
    }

    pub(super) fn arcane_roll_choice(
        &mut self,
        caster: usize,
        roller: usize,
        portent: bool,
        advantage: bool,
    ) -> Option<Act> {
        let dice = self.f[caster].arcane.portent;
        self.record_effect_choice(Prompt::Arcane { caster, roller, portent, advantage, dice })
    }

    pub(super) fn arcane_psychic_choice(&mut self, caster: usize, spell: super::magic::Spell) -> bool {
        self.record_effect_choice(Prompt::Psychic { caster, spell }) == Some(Act::Defend)
    }

    pub(super) fn inspired_save(
        &mut self,
        recipient: usize,
        stat: usize,
        total: i32,
        dc: i32,
        concentration: bool,
    ) -> bool {
        let total = total - self.take_mind_sliver(recipient);
        if total >= dc {
            return true;
        }
        if !self.bardic_inspiration_available(recipient) {
            return false;
        }
        let prompt = Prompt::Save(SavePrompt {
            recipient,
            stat,
            total,
            dc,
            concentration,
        });
        self.record_effect_choice(prompt) == Some(Act::Defend)
            && total + self.spend_bardic_inspiration(recipient) >= dc
    }

    pub(super) fn replayed_choice_phase(&self) -> Option<i32> {
        Some(match self.save_choice.pending.as_ref()?.prompt {
            Prompt::Save(_) => SAVE_CHOICE_PHASE,
            Prompt::Temp(_) => TEMP_CHOICE_PHASE,
            Prompt::Arcane { portent, .. } => if portent { 170 } else { 171 },
            Prompt::Psychic { .. } => 175,
            Prompt::ProtectiveField { .. } => super::martial::fighter_rogue::PROTECTIVE_FIELD,
        })
    }

    pub(super) fn save_choice_actor(&self) -> Option<usize> {
        Some(self.save_choice.pending.as_ref()?.prompt.recipient() % 2)
    }

    pub(super) fn save_choice_mask(&self) -> Option<u128> {
        Some(match self.save_choice.pending.as_ref()?.prompt {
            Prompt::Arcane { dice: rolls, portent: true, .. } => {
                Act::Pass.bit()
                    | if rolls[0] > 0 { Act::N.bit() } else { 0 }
                    | if rolls[1] > 0 { Act::NE.bit() } else { 0 }
            }
            _ => Act::Defend.bit() | Act::Pass.bit(),
        })
    }

    pub(super) fn save_choice_label(&self, action: Act) -> Option<String> {
        let prompt = self.save_choice.pending.as_ref()?.prompt;
        Some(match (prompt, action) {
            (Prompt::ProtectiveField { caster, .. }, Act::Defend) => {
                let sides = if self.f[caster].build.level >= 5 { 8 } else { 6 };
                format!("Protective Field (d{sides})")
            }
            (Prompt::ProtectiveField { .. }, Act::Pass) => "Accept the damage".into(),
            (Prompt::Psychic { .. }, Act::Defend) => "Deal psychic damage".into(),
            (Prompt::Psychic { .. }, Act::Pass) => "Keep the spell’s damage type".into(),
            (Prompt::Arcane { dice, portent: true, .. }, Act::N | Act::NE) => {
                format!("Use Portent: {}", dice[action as usize])
            }
            (Prompt::Arcane { portent: false, .. }, Act::Defend) => "Restore Balance".into(),
            (Prompt::Arcane { .. }, Act::Pass) => "Let the creature roll".into(),
            (Prompt::Save(_), Act::Defend) => "Roll Bardic Inspiration on this saving throw".into(),
            (Prompt::Save(_), Act::Pass) => {
                "Keep Bardic Inspiration and accept the failed save".into()
            }
            (Prompt::Temp(offer), Act::Defend) => format!(
                "Take {} temporary HP from {}",
                offer.proposed,
                offer.source.name(offer.recipient)
            ),
            (Prompt::Temp(offer), Act::Pass) => format!(
                "Keep {} temporary HP from {}",
                offer.current,
                offer.current_source.name(offer.recipient)
            ),
            _ => return None,
        })
    }

    pub(super) fn save_choice_summary(&self) -> Option<String> {
        let prompt = self.save_choice.pending.as_ref()?.prompt;
        let subject = if prompt.recipient() >= 2 {
            "Your companion"
        } else {
            "You"
        };
        Some(match prompt {
            Prompt::ProtectiveField { caster, target, damage } => format!(
                "Protect {} from {damage} damage",
                if caster == target { "yourself".into() } else { self.creature_label(target) },
            ),
            Prompt::Psychic { spell, .. } => format!("Choose the damage type for {}", spell.label()),
            Prompt::Arcane { roller, portent, .. } => format!(
                "{} before {} rolls a d20",
                if portent { "Use Portent" } else { "Restore Balance" },
                self.creature_label(roller)
            ),
            Prompt::Save(save) => {
                let ability = [
                    "Strength",
                    "Dexterity",
                    "Constitution",
                    "Intelligence",
                    "Wisdom",
                    "Charisma",
                ][save.stat];
                let check = if save.concentration {
                    "Concentration"
                } else {
                    ability
                };
                format!(
                    "{subject} rolled {} against DC {} on a {check} saving throw",
                    save.total, save.dc
                )
            }
            Prompt::Temp(offer) => format!(
                "{subject}: {} temporary HP from {}; offered {} from {}",
                offer.current,
                offer.current_source.name(offer.recipient),
                offer.proposed,
                offer.source.name(offer.recipient),
            ),
        })
    }

    pub(super) fn arcane_roll_policy(&self, caster: usize) -> Option<Act> {
        let prompt = self.save_choice.pending.as_ref()?.prompt;
        match prompt {
            Prompt::Arcane { roller, portent: true, dice, .. } => {
                let ally = roller % 2 == caster;
                dice.into_iter().enumerate()
                    .filter(|&(_, roll)| if ally { roll >= 16 } else { (1..=5).contains(&roll) })
                    .max_by_key(|&(_, roll)| if ally { roll } else { -roll })
                    .map_or(Some(Act::Pass), |(index, _)| Some(super::ACTS[index]))
            }
            Prompt::Arcane { roller, advantage, .. } => {
                Some(if (roller % 2 == caster) != advantage { Act::Defend } else { Act::Pass })
            }
            Prompt::Psychic { .. } => Some(Act::Defend),
            _ => None,
        }
    }

    pub(super) fn save_choice_features(&self) -> Option<[f32; 8]> {
        let prompt = self.save_choice.pending.as_ref()?.prompt;
        let companion = f32::from(u8::from(prompt.recipient() >= 2));
        Some(match prompt {
            Prompt::Psychic { .. } => [0.; 8],
            Prompt::ProtectiveField { caster, target, damage } => [
                damage as f32 / 40., u8::from(caster == target).into(),
                u8::from(caster == target % 2).into(), u8::from(target >= 2).into(),
                0., 0., 0., 0.,
            ],
            Prompt::Arcane { caster, roller, portent, dice, advantage } => {
                [dice[0] as f32 / 20., dice[1] as f32 / 20., u8::from(portent).into(),
                 u8::from(caster == roller % 2).into(), u8::from(roller >= 2).into(),
                 self.f[caster].arcane.balance as f32 / 5., u8::from(advantage).into(), 0.]
            }
            Prompt::Save(save) => [
                save.total as f32 / 40.,
                save.dc as f32 / 40.,
                (save.stat + 1) as f32 / 6.,
                u8::from(save.concentration).into(),
                companion,
                0.,
                0.,
                0.,
            ],
            Prompt::Temp(offer) => [
                offer.current as f32 / 20.,
                offer.proposed as f32 / 20.,
                offer.current_source.code(),
                offer.source.code(),
                companion,
                offer.current_source.caster_feature(offer.recipient),
                offer.source.caster_feature(offer.recipient),
                0.,
            ],
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{
        class::Class,
        entities::{Entity, EntityKind, FamiliarForm, Spirit},
        geometry::Pos,
        ACTS,
    };

    fn inspired_battle() -> Battle {
        let mut battle = Battle::new(1, [Class::Bard, Class::Fighter], [5; 2]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle.step(Act::Magic18).unwrap();
        battle.step(ACTS[1]).unwrap();
        battle
    }

    fn resolved_action(battle: &mut Battle, _: Act) -> Result<(), &'static str> {
        battle.f[0].slots[0] -= 1;
        let roll = battle.roll(20);
        if !battle.inspired_save(1, 2, roll, roll + 1, true) {
            battle.f[1].hp -= 7;
        }
        battle.f[0].temp = battle.roll(20);
        Ok(())
    }

    #[test]
    fn protective_field_reduces_poison_before_dwarven_resistance() {
        use crate::arena2::equipment::DamageType;
        use crate::arena2::martial::fighter_rogue::PROTECTIVE_FIELD;

        let mut battle = Battle::new_roster(17, [14, 12]);
        assert!(battle.origin_resists_poison(0));
        let hp = battle.f[0].hp;
        let mut expected = battle.clone();
        let reduction = (expected.roll(8) + expected.f[0].build.stats[3]).max(1);
        let harm = |trial: &mut Battle, _: Act| {
            trial.damage_typed(0, 18, DamageType::Poison);
            Ok(())
        };
        battle.step_with_effect_choices(Act::End, false, harm).unwrap();
        assert_eq!(battle.phase_id(), PROTECTIVE_FIELD);
        assert_eq!(battle.f[0].hp, hp);
        assert_eq!(battle.save_choice_features().unwrap()[0], 18. / 40.);
        battle.step_with_effect_choices(Act::Defend, false, harm).unwrap();
        assert_eq!(battle.f[0].hp, hp - (18 - reduction) / 2);
        assert_eq!(battle.f[0].martial.fighter_rogue.dice, 5);
        assert!(!battle.f[0].reaction);
    }

    #[test]
    fn failed_save_offers_private_pause_then_pass_replays_material_and_rng_once() {
        let mut battle = inspired_battle();
        let before = battle.clone();
        battle
            .step_with_save_choices(Act::End, resolved_action)
            .unwrap();
        assert_eq!(battle.phase_id(), SAVE_CHOICE_PHASE);
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.owner, 0);
        assert_eq!(battle.mask(), Act::Defend.bit() | Act::Pass.bit());
        assert_eq!(battle.f[0].slots, before.f[0].slots);
        assert_eq!(battle.f[1].hp, before.f[1].hp);
        assert_eq!(battle.f[0].temp, before.f[0].temp);
        assert!(battle
            .save_choice_summary()
            .unwrap()
            .contains("Concentration"));
        assert!(battle
            .step_with_save_choices(Act::End, resolved_action)
            .is_err());
        battle
            .step_with_save_choices(Act::Pass, resolved_action)
            .unwrap();
        let mut expected = before;
        expected.roll(20);
        assert_eq!(battle.f[0].temp, expected.roll(20));
        assert_eq!(battle.f[0].slots[0], expected.f[0].slots[0] - 1);
        assert_eq!(battle.f[1].hp, expected.f[1].hp - 7);
        assert!(battle.bardic_inspiration_available(1));
        assert_ne!(battle.phase_id(), SAVE_CHOICE_PHASE);
    }

    #[test]
    fn spending_rolls_at_the_original_save_before_later_randomness() {
        let mut battle = inspired_battle();
        let mut expected = battle.clone();
        expected.roll(20);
        assert!((1..=8).contains(&expected.spend_bardic_inspiration(1)));
        let next = expected.roll(20);
        battle
            .step_with_save_choices(Act::End, resolved_action)
            .unwrap();
        battle
            .step_with_save_choices(Act::Defend, resolved_action)
            .unwrap();
        assert_eq!(battle.f[1].hp, expected.f[1].hp);
        assert_eq!(battle.f[0].temp, next);
        assert!(!battle.bardic_inspiration_available(1));
    }

    #[test]
    fn successful_save_proceeds_without_a_prompt_or_token_spend() {
        let mut battle = inspired_battle();
        battle
            .step_with_save_choices(Act::End, |b, _| {
                assert!(b.inspired_save(1, 3, 15, 15, false));
                b.f[0].temp = 3;
                Ok(())
            })
            .unwrap();
        assert_eq!(battle.f[0].temp, 3);
        assert!(battle.bardic_inspiration_available(1));
        assert_ne!(battle.phase_id(), SAVE_CHOICE_PHASE);
    }

    #[test]
    fn one_action_can_pause_for_both_recipients_in_roll_order() {
        let mut battle = Battle::new(9, [Class::Bard; 2], [5; 2]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle.step(Act::Magic18).unwrap();
        battle.step(ACTS[1]).unwrap();
        battle.step(Act::End).unwrap();
        battle.step(Act::Magic18).unwrap();
        battle.step(ACTS[1]).unwrap();
        fn two(b: &mut Battle, _: Act) -> Result<(), &'static str> {
            for seat in [0, 1] {
                let die = b.roll(20);
                if !b.inspired_save(seat, 1, die, die + 1, false) {
                    b.f[seat].hp -= 1;
                }
            }
            Ok(())
        }
        let hp = [battle.f[0].hp, battle.f[1].hp];
        battle.step_with_save_choices(Act::End, two).unwrap();
        assert_eq!(battle.actor(), Some(0));
        battle.step_with_save_choices(Act::Defend, two).unwrap();
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.f[0].hp, hp[0]);
        assert_eq!(battle.f[1].hp, hp[1]);
        battle.step_with_save_choices(Act::Pass, two).unwrap();
        assert_eq!(battle.f[0].hp, hp[0]);
        assert_eq!(battle.f[1].hp, hp[1] - 1);
        assert!(!battle.bardic_inspiration_available(0));
        assert!(battle.bardic_inspiration_available(1));
    }

    #[test]
    fn real_failed_spell_save_surfaces_the_recipient_decision() {
        for seed in 1..=128 {
            let mut b = Battle::new(seed, [Class::Bard, Class::Fighter], [5; 2]);
            b.owner = 0;
            b.f[0].pos = Pos { x: 0, y: 0 };
            b.f[1].pos = Pos { x: 1, y: 0 };
            b.step(Act::Magic18).unwrap();
            b.step(ACTS[1]).unwrap();
            let hp = b.f[1].hp;
            b.step(Act::Spell0).unwrap();
            for _ in 0..8 {
                if matches!(b.phase_id(), 0 | SAVE_CHOICE_PHASE) {
                    break;
                }
                assert_eq!(b.actor(), Some(0));
                let mask = b.mask();
                b.step(if mask & ACTS[1].bit() != 0 {
                    ACTS[1]
                } else {
                    ACTS[mask.trailing_zeros() as usize]
                })
                .unwrap();
            }
            if b.phase_id() != SAVE_CHOICE_PHASE {
                continue;
            }
            assert_eq!(b.actor(), Some(1));
            assert_eq!(b.f[1].hp, hp);
            assert!(b.pending_summary().contains("Wisdom"));
            b.step(Act::Pass).unwrap();
            assert!(b.f[1].hp < hp);
            assert!(b.bardic_inspiration_available(1));
            return;
        }
        panic!("no seed provoked a failed Vicious Mockery save");
    }

    #[test]
    fn drop_concentration_inside_a_save_keeps_the_save_and_reaction() {
        for seed in 1..=128 {
            let mut b = Battle::new(seed, [Class::Bard, Class::Fighter], [5; 2]);
            b.owner = 0;
            b.f[0].pos = Pos { x: 0, y: 0 };
            b.f[1].pos = Pos { x: 1, y: 0 };
            b.step(Act::Magic18).unwrap();
            b.step(ACTS[1]).unwrap();
            b.f[1].concentration = Some(crate::arena2::battle::Concentration::Bless);
            let hp = b.f[1].hp;
            b.step(Act::Spell0).unwrap();
            for _ in 0..8 {
                if matches!(b.phase_id(), 0 | SAVE_CHOICE_PHASE) {
                    break;
                }
                assert_eq!(b.actor(), Some(0));
                let mask = b.mask();
                b.step(if mask & ACTS[1].bit() != 0 {
                    ACTS[1]
                } else {
                    ACTS[mask.trailing_zeros() as usize]
                })
                .unwrap();
            }
            if b.phase_id() != SAVE_CHOICE_PHASE {
                continue;
            }
            assert_eq!(b.actor(), Some(1));
            assert_eq!(b.f[1].hp, hp);
            assert!(b.pending_summary().contains("Wisdom"));
            let reaction = b.f[1].reaction;
            assert_ne!(b.mask() & Act::DropConcentration.bit(), 0);
            assert!(b.public_concentration(1, 1));
            assert_ne!(b.choice_label(0, Act::DropConcentration), "Drop concentration");
            b.step(Act::DropConcentration).unwrap();
            assert_eq!(b.phase_id(), SAVE_CHOICE_PHASE);
            assert_eq!(b.mask() & Act::DropConcentration.bit(), 0);
            assert!(!b.public_concentration(1, 1));
            assert_eq!(b.f[1].hp, hp);
            b.step(Act::Pass).unwrap();
            assert_eq!(b.f[1].reaction, reaction);
            assert!(b.f[1].concentration.is_none());
            assert!(b.f[1].hp < hp);
            assert!(b.bardic_inspiration_available(1));
            return;
        }
        panic!("no seed provoked a failed Vicious Mockery save");
    }

    #[test]
    fn concentration_damage_uses_the_same_transaction_and_defers_damage() {
        let mut battle = inspired_battle();
        battle.f[1].hp = 1000;
        battle.f[1].concentration = Some(crate::arena2::battle::Concentration::Bless);
        let hit = |b: &mut Battle, _| {
            b.damage(1, 100);
            Ok(())
        };
        battle.step_with_save_choices(Act::End, hit).unwrap();
        assert!(battle.save_choice_features().unwrap()[3] > 0.);
        assert_eq!(battle.f[1].hp, 1000);
        assert!(battle.f[1].concentration.is_some());
        battle.step_with_save_choices(Act::Pass, hit).unwrap();
        assert_eq!(battle.f[1].hp, 900);
        assert!(battle.f[1].concentration.is_none());
    }

    #[test]
    fn companion_temporary_hp_uses_its_owners_choice_without_changing_duelist_hp() {
        use crate::arena2::temp_hp::TempSource;
        let mut battle = Battle::new(4, [Class::Paladin, Class::Wizard], [5; 2]);
        battle.owner = 0;
        battle.entities.test_install(Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Owl,
                spirit: Spirit::Fey,
            },
            1,
            Pos { x: 1, y: 1 },
            1,
        ));
        battle.grant_temporary_hp(3, 7, TempSource::DarkOnesBlessing);
        let grant = |b: &mut Battle, _| {
            b.grant_temporary_hp(3, 3, TempSource::Heroism(0));
            Ok(())
        };
        battle.step_with_save_choices(Act::End, grant).unwrap();
        assert_eq!(battle.phase_id(), TEMP_CHOICE_PHASE);
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.entity(3).unwrap().temp_hp, 7);
        assert!(battle
            .save_choice_summary()
            .unwrap()
            .contains("Your companion"));
        battle.step_with_save_choices(Act::Defend, grant).unwrap();
        assert_eq!(battle.entity(3).unwrap().temp_hp, 3);
        assert_eq!(battle.f[1].temp, 0);
    }

    #[test]
    fn drop_concentration_inside_companion_temporary_hp_choice() {
        use crate::arena2::temp_hp::TempSource;
        let mut battle = Battle::new(4, [Class::Paladin, Class::Wizard], [5; 2]);
        battle.owner = 0;
        battle.f[1].concentration = Some(crate::arena2::battle::Concentration::Bless);
        battle.entities.test_install(Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Owl,
                spirit: Spirit::Fey,
            },
            1,
            Pos { x: 1, y: 1 },
            1,
        ));
        battle.grant_temporary_hp(3, 7, TempSource::DarkOnesBlessing);
        let grant = |b: &mut Battle, _| {
            b.grant_temporary_hp(3, 3, TempSource::Heroism(0));
            Ok(())
        };
        battle.step_with_save_choices(Act::End, grant).unwrap();
        assert_eq!(battle.phase_id(), TEMP_CHOICE_PHASE);
        battle.step(Act::DropConcentration).unwrap();
        assert_eq!(battle.phase_id(), TEMP_CHOICE_PHASE);
        assert_eq!(battle.mask() & Act::DropConcentration.bit(), 0);
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.entity(3).unwrap().temp_hp, 7);
        assert!(battle
            .save_choice_summary()
            .unwrap()
            .contains("Your companion"));
        battle.step_with_save_choices(Act::Defend, grant).unwrap();
        assert_eq!(battle.entity(3).unwrap().temp_hp, 3);
        assert!(battle.f[1].concentration.is_none());
        assert_eq!(battle.f[1].temp, 0);
    }

    #[test]
    fn companion_save_uses_owner_prompt_and_consumes_its_own_token() {
        let mut battle = Battle::new(4, [Class::Bard, Class::Wizard], [5; 2]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle.entities.test_install(Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Owl,
                spirit: Spirit::Fey,
            },
            1,
            Pos { x: 1, y: 1 },
            1,
        ));
        battle.step(Act::Magic18).unwrap();
        battle.step(ACTS[3]).unwrap();
        let save = |b: &mut Battle, _| {
            b.f[0].temp = if b.entity_save(3, 5, 40) { 2 } else { 1 };
            Ok(())
        };
        battle.step_with_save_choices(Act::End, save).unwrap();
        assert_eq!(battle.actor(), Some(1));
        assert!(battle.save_choice_features().unwrap()[4] > 0.);
        battle.step_with_save_choices(Act::Defend, save).unwrap();
        assert_eq!(battle.f[0].temp, 1, "high DC still fails");
        assert!(!battle.bardic_inspiration_available(3));
    }
}
