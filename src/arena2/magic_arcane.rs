//! Level-three subclass features for arcane caster presets.
use super::equipment::DamageType;
use super::{
    battle::Battle,
    class::{Build, Class},
    magic::{Concentration, Spell},
    Act, ACTS,
};

pub(super) fn prepared(build: Build, action: Act) -> Option<Option<Spell>> {
    use Act::*;
    use Class::*;
    use Spell::*;
    let level = build.level;
    Some(match (build.class, build.variant, action) {
        (Sorcerer, 1, Magic8) | (Warlock, 2, Spell2) if level >= 3 => Some(DissonantWhispers),
        (Sorcerer, 1, Magic9) if level >= 3 => Some(MindSliver),
        (Sorcerer, 1, Magic10) if level >= 3 => Some(ArmsOfHadar),
        (Sorcerer, 1, Magic15) if level >= 5 => Some(HungerOfHadar),
        (Sorcerer, 1, Magic16) if level >= 3 => Some(CalmEmotions),
        (Sorcerer, 1, Magic20) | (Warlock, 2, Magic20) if level >= 3 => Some(DetectThoughts),
        (Sorcerer, 2, Magic8) if level >= 3 => Some(Aid),
        (Sorcerer, 2, Magic15) if level >= 5 => Some(DispelMagic),
        (Sorcerer, 2, Magic16) if level >= 3 => Some(LesserRestoration),
        (Sorcerer, 2, Magic17) if level >= 3 => Some(Protection),
        (Sorcerer, 2, Magic18) if level >= 5 => Some(ProtectionFromEnergy),
        (Sorcerer, 2, Magic19) if level >= 3 => Some(Alarm),
        (Sorcerer, 1 | 2, Magic4 | Magic7 | Magic8 | Magic9 | Magic11) => None,
        (Sorcerer, 1 | 2, Magic13) if level >= 5 => None,

        (Warlock, 1, Magic8) if level >= 3 => Some(FaerieFire),
        (Warlock, 1, Magic15) if level >= 5 => Some(Blink),
        (Warlock, 1, Magic16) if level >= 5 => Some(PlantGrowth),
        (Warlock, 1, Magic17) if level >= 3 => Some(CalmEmotions),
        (Warlock, 1, Magic18) if level >= 3 => Some(Sleep),
        (Warlock, 1 | 2, Magic22) if level >= 3 => Some(PhantasmalForce),
        (Warlock, 2, Spell3) if level >= 3 => Some(HideousLaughter),
        (Warlock, 2, Spell4) if level >= 5 => Some(HungerOfHadar),
        (Warlock, 1 | 2, Spell2 | Spell3 | Spell4 | Magic8 | Magic14) => None,
        (Warlock, 1, Magic13) if level < 4 => None,
        (Warlock, 2, Magic13) => None,

        (Wizard, 1, Magic7) if level >= 5 => Some(Counterspell),
        (Wizard, 1, Magic8) if level >= 3 => Some(Protection),
        (Wizard, 1, Magic9) if level >= 5 => Some(DispelMagic),
        (Wizard, 1, Magic10) if level >= 5 => Some(ProtectionFromEnergy),
        (Wizard, 1, Magic6) if level >= 4 => Some(MistyStep),
        (Wizard, 1, Spell2 | Spell3) if level >= 5 => None,
        (Wizard, 1, Magic5) => None,
        (Wizard, 2, Magic8) if level >= 3 => Some(SeeInvisibility),
        (Wizard, 1 | 2, Magic3) if level >= 3 => None,
        (_, _, _) => return None,
    })
}

#[derive(Clone, Debug, Default)]
pub(super) struct ArcaneState {
    ward: Option<i32>,
    pub(super) fey_steps: i32,
    pub(super) balance: i32,
    pub(super) portent: [i32; 2],
    pub(super) psychic: bool,
    pub(super) psychic_hex: bool,
    pub(super) psychic_hadar: bool,
    pub(super) portent_used: bool,
    step_departure: Option<super::geometry::Point>,
    taunted: u8,
    taunt_until: i32,
    calm_pending: u8,
    calm_immune: u8,
    calm_indifferent: u8,
    pub(super) see_invisibility_until: i32,
    sensed_presence: bool,
}

impl Battle {
    pub(super) fn init_arcane(&mut self, i: usize) {
        let build = self.f[i].build;
        if build.level < 3 {
            return;
        }
        let modifier = build.casting;
        if self.is_subclass(i, Class::Warlock, 1) {
            self.f[i].arcane.fey_steps = modifier.max(1);
        }
        if self.is_subclass(i, Class::Sorcerer, 2) {
            self.f[i].arcane.balance = modifier.max(1);
        }
        if self.is_subclass(i, Class::Wizard, 2) {
            self.f[i].arcane.portent = [self.roll(20), self.roll(20)];
        }
    }

    fn arcane_ward_max(&self, i: usize) -> i32 {
        self.f[i].build.level * 2 + self.f[i].build.casting
    }

    pub(super) fn arcane_ward_cast(&mut self, i: usize, spell: Spell, tier: usize) {
        use Spell::*;
        if self.f[i].build.level < 3
            || !self.is_subclass(i, Class::Wizard, 1)
            || !matches!(
                spell,
                MageArmor | Shield | Counterspell | DispelMagic | Protection | ProtectionFromEnergy
            )
        {
            return;
        }
        let max = self.arcane_ward_max(i);
        let ward = &mut self.f[i].arcane.ward;
        *ward = Some(ward.map_or(max, |hp| (hp + 2 * tier as i32).min(max)));
    }

    pub(super) fn arcane_ward_absorb(&mut self, i: usize, amount: i32) -> i32 {
        let Some(remaining) = self.f[i].arcane.ward.as_mut() else {
            return amount;
        };
        let blocked = amount.min(*remaining);
        *remaining -= blocked;
        amount - blocked
    }

    pub(super) fn arcane_choose_psychic(&mut self, i: usize, spell: Spell) -> bool {
        use Spell::*;
        self.f[i].build.level >= 3
            && self.is_subclass(i, Class::Warlock, 2)
            && matches!(
                spell,
                EldritchBlast | PoisonSpray | TrueStrike | Hex | HungerOfHadar
            )
            && self.arcane_psychic_choice(i, spell)
    }

    pub(super) fn arcane_damage_kind(&self, i: usize, kind: DamageType) -> DamageType {
        if self.is_subclass(i, Class::Warlock, 2) && self.f[i].arcane.psychic {
            DamageType::Psychic
        } else {
            kind
        }
    }
}

impl Battle {
    pub(super) fn may_need_arcane_roll(&self) -> bool {
        self.f.iter().any(|f| {
            f.arcane.balance > 0
                || f.arcane.portent.iter().any(|&roll| roll > 0)
                || f.build.level >= 3 && f.build.is_subclass(Class::Warlock, 2)
        })
    }

    pub(super) fn d20_for(
        &mut self,
        roller: usize,
        mut advantage: bool,
        mut disadvantage: bool,
    ) -> i32 {
        for caster in [self.owner, 1 - self.owner] {
            let state = &self.f[caster].arcane;
            if state.balance <= 0 && state.portent.iter().all(|&roll| roll == 0) {
                continue;
            }
            let visible = if caster == roller {
                true
            } else if roller < 2 {
                self.can_see(caster, roller)
            } else {
                self.entity_visible(caster, roller)
            };
            if !visible || self.f[caster].hp <= 0 {
                continue;
            }
            if !self.f[caster].arcane.portent_used
                && self.f[caster].arcane.portent.iter().any(|&roll| roll > 0)
            {
                if let Some(choice @ (Act::N | Act::NE)) =
                    self.arcane_roll_choice(caster, roller, true, advantage && !disadvantage)
                {
                    self.f[caster].arcane.portent_used = true;
                    return std::mem::take(&mut self.f[caster].arcane.portent[choice as usize]);
                }
            }
            if advantage != disadvantage
                && self.f[caster].arcane.balance > 0
                && self.can_react(caster)
                && self
                    .combatant_distance(caster, roller)
                    .is_some_and(|distance| distance <= 12)
                && self.arcane_roll_choice(caster, roller, false, advantage) == Some(Act::Defend)
            {
                self.f[caster].arcane.balance -= 1;
                self.f[caster].reaction = false;
                advantage = false;
                disadvantage = false;
            }
        }
        self.d20(advantage, disadvantage)
    }
}

pub(super) const WARD_RECHARGE: i32 = 172;
pub(super) const FEY_STEP: i32 = 173;
pub(super) const CALM_EMOTIONS: i32 = 176;

impl Battle {
    pub(super) fn arcane_action_mask(&self, i: usize) -> u128 {
        let sense = if self.action && self.f[i].concentration == Some(Concentration::DetectThoughts)
        {
            Act::Magic20.bit()
        } else {
            0
        };
        sense
            | if self.bonus
                && self.f[i]
                    .arcane
                    .ward
                    .is_some_and(|hp| hp < self.arcane_ward_max(i))
                && self.f[i].slots.iter().any(|&slots| slots > 0)
            {
                Act::Magic21.bit()
            } else {
                0
            }
    }

    pub(super) fn arcane_action_label(&self, i: usize, action: Act) -> Option<String> {
        if action == Act::Magic20 && self.f[i].concentration == Some(Concentration::DetectThoughts)
        {
            return Some("Sense Thoughts".into());
        }
        (self.is_subclass(i, Class::Wizard, 1) && action == Act::Magic21)
            .then(|| "Recharge Arcane Ward".into())
    }

    pub(super) fn arcane_action_step(&mut self, i: usize, action: Act) -> bool {
        if action == Act::Magic20
            && self.action
            && self.f[i].concentration == Some(Concentration::DetectThoughts)
        {
            self.action = false;
            self.arcane_sense_thoughts(i);
            return true;
        }
        if self.arcane_action_mask(i) & action.bit() == 0 {
            return false;
        }
        self.request(i, WARD_RECHARGE);
        true
    }

    pub(super) fn arcane_choice_mask(&self, i: usize, phase: i32) -> u128 {
        match phase {
            WARD_RECHARGE => (0..3)
                .filter(|&slot| self.f[i].slots[slot] > 0)
                .fold(Act::Pass.bit(), |mask, slot| mask | ACTS[slot].bit()),
            FEY_STEP => {
                let recipients = (0..4)
                    .filter(|&target| {
                        self.combatant_distance(i, target)
                            .is_some_and(|distance| distance <= 2)
                            && (target == i
                                || if target < 2 {
                                    self.can_see(i, target)
                                } else {
                                    self.entity_visible(i, target)
                                })
                    })
                    .fold(ACTS[i].bit(), |mask, target| mask | ACTS[target].bit());
                recipients | Act::Defend.bit() | Act::Pass.bit()
            }
            CALM_EMOTIONS => Act::N.bit() | Act::NE.bit(),
            _ => 0,
        }
    }

    pub(super) fn arcane_choice_step(&mut self, i: usize, phase: i32, action: Act) {
        match phase {
            WARD_RECHARGE if action != Act::Pass => {
                let slot = action as usize;
                self.f[i].slots[slot] -= 1;
                self.bonus = false;
                self.arcane_ward_cast(i, Spell::MageArmor, slot + 1);
            }
            CALM_EMOTIONS => {
                let state = &mut self.f[i].arcane;
                let bit = 1 << state.calm_pending.trailing_zeros();
                state.calm_pending &= !bit;
                if action == Act::N {
                    state.calm_immune |= bit;
                } else {
                    state.calm_indifferent |= bit;
                }
                if state.calm_pending != 0 {
                    self.request(i, CALM_EMOTIONS);
                }
            }
            FEY_STEP => {
                let departure = self.f[i]
                    .arcane
                    .step_departure
                    .take()
                    .expect("Fey departure point");
                if action == Act::Defend {
                    let dc = self.spell_dc(i);
                    let mut taunted = 0;
                    for target in 0..4 {
                        if self.footprint(target).is_none_or(|body| {
                            body.squares().all(|point| departure.dist(point) > 1)
                        }) {
                            continue;
                        }
                        let saved = if target < 2 {
                            self.save(target, 4, dc)
                        } else {
                            self.entity_save(target, 4, dc)
                        };
                        if !saved {
                            taunted |= 1 << target;
                        }
                    }
                    self.f[i].arcane.taunted = taunted;
                    self.f[i].arcane.taunt_until = self.turns + 2;
                } else if action != Act::Pass {
                    let amount = self.roll(10);
                    self.grant_temporary_hp(
                        action as usize,
                        amount,
                        super::temp_hp::TempSource::RefreshingStep,
                    );
                }
            }
            _ => {}
        }
    }

    pub(super) fn arcane_choice_label(
        &self,
        actor: usize,
        phase: i32,
        action: Act,
    ) -> Option<String> {
        if phase == WARD_RECHARGE {
            return Some(if action == Act::Pass {
                "Keep spell slots".into()
            } else {
                format!("Expend level {} spell slot", action as usize + 1)
            });
        }
        if phase == CALM_EMOTIONS {
            let target = self.f[actor].arcane.calm_pending.trailing_zeros() as usize;
            return Some(format!(
                "{}: {}",
                self.creature_label(target),
                if action == Act::N {
                    "suppress Charmed and Frightened"
                } else {
                    "make indifferent to enemies"
                }
            ));
        }
        if phase == FEY_STEP {
            return Some(match action {
                Act::Pass => "Skip Steps of the Fey".into(),
                Act::Defend => "Taunting Step".into(),
                _ => format!(
                    "Refreshing Step on {}",
                    self.creature_label(action as usize)
                ),
            });
        }
        None
    }

    pub(super) fn arcane_fey_step(&mut self, i: usize, departure: super::geometry::Point) {
        if self.f[i].build.level >= 3
            && self.is_subclass(i, Class::Warlock, 1)
            && self.fighter_point(i) != departure
        {
            self.f[i].arcane.step_departure = Some(departure);
            self.request(i, FEY_STEP);
        }
    }

    pub(super) fn arcane_taunt_disadvantage(&self, source: usize, defender: usize) -> bool {
        (0..2).any(|i| {
            let state = &self.f[i].arcane;
            defender != i && state.taunt_until > self.turns && state.taunted & (1 << source) != 0
        })
    }

    pub(super) fn arcane_subclass_policy(
        &self,
        actor: usize,
        mask: u128,
        _expert: bool,
    ) -> Option<Act> {
        let choice = match self.phase_id() {
            170 | 171 | 175 => self.arcane_roll_policy(actor)?,
            178 | 179 => {
                let [_, _, own, advantage, _, _, _, _] = self.arcane_initiative_features(actor)?;
                if self.phase_id() == 179 {
                    if (own > 0.) != (advantage > 0.) {
                        Act::Defend
                    } else {
                        Act::Pass
                    }
                } else {
                    let own = own > 0.;
                    self.f[actor]
                        .arcane
                        .portent
                        .into_iter()
                        .enumerate()
                        .filter(|&(_, die)| {
                            if own {
                                die >= 16
                            } else {
                                (1..=5).contains(&die)
                            }
                        })
                        .max_by_key(|&(_, die)| if own { die } else { -die })
                        .map_or(Act::Pass, |(index, _)| ACTS[index])
                }
            }
            WARD_RECHARGE => ACTS[..3]
                .iter()
                .copied()
                .find(|action| mask & action.bit() != 0)
                .unwrap_or(Act::Pass),
            FEY_STEP => ACTS[actor],
            CALM_EMOTIONS => {
                if self.f[actor].arcane.calm_pending.trailing_zeros() as usize == actor {
                    Act::N
                } else {
                    Act::NE
                }
            }
            _ => return None,
        };
        (mask & choice.bit() != 0).then_some(choice)
    }

    pub(super) fn arcane_subclass_features(&self, viewer: usize) -> [f32; 8] {
        if self.is_subclass(viewer, Class::Sorcerer, 2) {
            return self.alarm_features(viewer);
        }
        let state = &self.f[viewer].arcane;
        [
            if self.f[viewer].concentration == Some(Concentration::DetectThoughts) {
                f32::from(u8::from(state.sensed_presence))
            } else {
                state.ward.unwrap_or(0) as f32 / 20.
            },
            if state.ward.is_some() { 1. } else { 0. },
            state.portent[0] as f32 / 20.,
            state.portent[1] as f32 / 20.,
            if state.portent_used { 1. } else { 0. },
            state.balance as f32 / 5.,
            state.fey_steps as f32 / 5.,
            if state.psychic { 1. } else { 0. },
        ]
    }
}

impl Battle {
    pub(super) fn arcane_apply_calm(
        &mut self,
        caster: usize,
        center: super::geometry::Point,
        slot: usize,
        heightened: bool,
    ) {
        self.f[caster].concentration = None;
        let dc = self.spell_dc(caster);
        let mut affected = 0;
        for id in [caster, 1 - caster] {
            if self.footprint(id).is_some_and(|body| {
                body.squares()
                    .any(|p| center.dist(p) <= 4 && self.map.los_at(center, p))
            }) && !self.save_advantage(
                id,
                5,
                dc,
                heightened && self.arcane_heightened_for(caster, id),
            ) {
                affected |= 1 << id;
            }
        }
        if affected != 0 {
            self.concentrate(caster, Concentration::CalmEmotions, 20, Some(center), slot);
            let state = &mut self.f[caster].arcane;
            state.calm_pending = affected;
            state.calm_immune = 0;
            state.calm_indifferent = 0;
            self.request(caster, CALM_EMOTIONS);
        }
    }

    pub(super) fn arcane_emotions_immune(&self, target: usize) -> bool {
        self.f.iter().any(|fighter| {
            fighter.concentration == Some(Concentration::CalmEmotions)
                && fighter.arcane.calm_immune & (1 << target) != 0
        })
    }

    pub(super) fn arcane_indifferent(&self, subject: usize, other: usize) -> bool {
        subject % 2 != other % 2
            && self.f.iter().any(|fighter| {
                fighter.concentration == Some(Concentration::CalmEmotions)
                    && fighter.arcane.calm_indifferent & (1 << subject) != 0
            })
    }

    pub(super) fn arcane_damage_ends_indifference(&mut self, target: usize) {
        let viewer = target % 2;
        let affected = (if target < 2 {
            true
        } else {
            self.entity_visible(viewer, target)
        })
        .then_some(1 << viewer)
        .unwrap_or(0);
        for fighter in &mut self.f {
            if fighter.concentration == Some(Concentration::CalmEmotions) {
                fighter.arcane.calm_indifferent &= !affected;
            }
        }
    }
}

impl Battle {
    pub(super) fn arcane_sees_invisible(&self, viewer: usize) -> bool {
        viewer < 2 && self.f[viewer].arcane.see_invisibility_until > self.turns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::geometry::Pos;

    fn close_duel(roster: [u8; 2]) -> Battle {
        let mut duel = Battle::new_roster(2, roster);
        while matches!(duel.phase_id(), 178 | 179) {
            duel.step_raw_action(Act::Pass).unwrap();
        }
        duel.owner = 0;
        duel.f[0].pos = Pos { x: 0, y: 0 };
        duel.f[1].pos = Pos { x: 1, y: 0 };
        duel
    }

    #[test]
    fn portent_is_available_before_initiative() {
        let mut duel = Battle::new_roster(2, [35, 12]);
        assert_eq!(duel.phase_id(), 178);
        assert_eq!(duel.actor(), Some(0));
        duel.f[0].arcane.portent = [20, 1];
        duel.step_raw_action(Act::N).unwrap();
        while matches!(duel.phase_id(), 178 | 179) {
            duel.step_raw_action(Act::Pass).unwrap();
        }
        assert_eq!(duel.f[0].arcane.portent, [0, 1]);
        assert_eq!(duel.turns, 1);
    }

    #[test]
    fn sense_thoughts_does_not_reveal_the_hidden_square_and_has_limited_range() {
        let mut duel = close_duel([32, 12]);
        duel.f[1].hidden_dc = Some(20);
        assert!(duel.observed_pos(0, 1).is_none());
        duel.step_raw_action(Act::Magic20).unwrap();
        assert!(duel.f[0].arcane.sensed_presence);
        assert!(duel.observed_pos(0, 1).is_none());

        duel.f[1].pos = Pos { x: 8, y: 0 };
        duel.arcane_sense_thoughts(0);
        assert!(!duel.f[0].arcane.sensed_presence);
    }

    #[test]
    fn savant_book_gains_two_matching_spells_at_three_and_another_at_five() {
        for id in [34, 35] {
            let duel = close_duel([id, 12]);
            let mut build = duel.f[0].build;
            let at_five = build.savant_spellbook();
            assert_eq!(at_five.len(), 3);
            assert_eq!(at_five[2].learned_at, 5);
            assert_eq!(at_five[2].rank, 3);
            build.level = 3;
            let at_three = build.savant_spellbook();
            assert_eq!(at_three.len(), 2);
            assert!(at_three
                .iter()
                .all(|entry| entry.learned_at == 3 && entry.rank <= 2));
            assert!(at_three
                .iter()
                .all(|entry| entry.school == at_five[2].school));
            build.level = 2;
            assert!(build.savant_spellbook().is_empty());
        }
    }

    #[test]
    fn archfey_sleep_targets_selected_creatures_in_the_area() {
        for seed in 0..24 {
            let mut duel = Battle::new_roster(seed, [31, 12]);
            duel.owner = 0;
            duel.f[0].pos = Pos { x: 0, y: 0 };
            duel.f[1].pos = Pos { x: 1, y: 0 };
            duel.step_raw_action(Act::Magic18).unwrap();
            duel.step_raw_action(ACTS[0]).unwrap();
            duel.step_raw_action(ACTS[1]).unwrap();
            if duel.phase_id() == 19 {
                duel.step_raw_action(ACTS[0]).unwrap();
            }
            assert_eq!(duel.phase_id(), 177);
            assert_ne!(duel.raw_action_mask() & ACTS[2].bit(), 0);
            duel.step_raw_action(ACTS[2]).unwrap();
            if duel.sleep_affected(1) {
                assert!(!duel.sleep_affected(0));
                assert_eq!(duel.f[0].slots[2], 1);
                return;
            }
        }
        panic!("the opposing fighter succeeded on all twenty-four Wisdom saves");
    }

    #[test]
    fn psychic_spells_choose_damage_type_before_the_attack_is_rolled() {
        let mut duel = close_duel([32, 12]);
        duel.step_raw_action(Act::Spell0).unwrap();
        assert_eq!(duel.phase_id(), 175);
        assert_eq!(duel.actor(), Some(0));
        assert!(!duel.f[0].arcane.psychic);
        duel.step_raw_action(Act::Defend).unwrap();
        assert_eq!(
            duel.arcane_damage_kind(0, DamageType::Force),
            DamageType::Psychic
        );
    }

    #[test]
    fn arcane_ward_blocks_damage_before_temporary_hp_and_recharges_without_casting() {
        let mut duel = close_duel([34, 12]);
        let initial_hp = duel.f[0].hp;
        duel.damage(0, 1);
        assert_eq!(duel.f[0].hp, initial_hp - 1);
        assert!(duel.f[0].arcane.ward.is_none());
        duel.step_raw_action(Act::Magic2).unwrap();
        assert_eq!(duel.phase_id(), 38);
        duel.step_raw_action(Act::N).unwrap();
        let capacity = duel.arcane_ward_max(0);
        assert_eq!(duel.f[0].arcane.ward, Some(capacity));

        duel.f[0].temp = 4;
        duel.damage(0, 8);
        assert_eq!(duel.f[0].arcane.ward, Some(capacity - 8));
        assert_eq!(duel.f[0].temp, 4);
        assert_eq!(duel.f[0].hp, initial_hp - 1);

        let slots = duel.f[0].slots;
        duel.step_raw_action(Act::Magic21).unwrap();
        assert_eq!(duel.phase_id(), WARD_RECHARGE);
        duel.step_raw_action(Act::NE).unwrap();
        assert_eq!(duel.f[0].slots[1], slots[1] - 1);
        assert_eq!(duel.f[0].arcane.ward, Some(capacity - 4));
        assert!(!duel.bonus);
    }

    #[test]
    fn portent_interrupts_an_enemy_attack_before_the_roll_and_can_force_a_miss() {
        let mut duel = close_duel([12, 35]);
        duel.f[1].arcane.portent = [20, 1];
        let hp = duel.f[1].hp;
        duel.step_raw_action(Act::Melee).unwrap();
        if duel.phase_id() == 14 {
            duel.step_raw_action(Act::NE).unwrap();
        }
        assert_eq!(duel.phase_id(), 170);
        assert_eq!(duel.actor(), Some(1));
        assert_eq!(duel.f[1].arcane.portent, [20, 1]);
        duel.step_raw_action(Act::NE).unwrap();
        assert_eq!(duel.f[1].hp, hp);
        assert_eq!(duel.f[1].arcane.portent, [20, 0]);
        assert!(duel.f[1].arcane.portent_used);
    }

    #[test]
    fn restore_balance_pays_one_use_and_reaction_on_an_advantaged_attack() {
        let mut duel = close_duel([12, 29]);
        duel.f[0].attack_advantage = true;
        let uses = duel.f[1].arcane.balance;
        duel.step_raw_action(Act::Melee).unwrap();
        if duel.phase_id() == 14 {
            duel.step_raw_action(Act::NE).unwrap();
        }
        assert_eq!(duel.phase_id(), 171);
        assert_eq!(duel.actor(), Some(1));
        duel.step_raw_action(Act::Defend).unwrap();
        assert!(!duel.f[1].reaction);
        assert_eq!(duel.f[1].arcane.balance, uses - 1);
    }

    #[test]
    fn archfey_can_cast_misty_step_without_a_pact_slot() {
        let mut duel = close_duel([31, 12]);
        duel.f[0].slots = [0; 3];
        assert_eq!(duel.slot_for_action(0, Act::Magic6), Some(0));
        assert!(duel.f[0].arcane.fey_steps > 0);
        assert_eq!(duel.prepared_spell(0, Act::Magic8), Some(Spell::FaerieFire));
        assert_eq!(duel.prepared_spell(0, Act::Spell4), None);

        let charges = duel.f[0].arcane.fey_steps;
        let origin = duel.fighter_point(0);
        duel.step_raw_action(Act::Magic6).unwrap();
        while matches!(duel.phase_id(), 5 | 6 | 19) {
            let first = duel.raw_action_mask().trailing_zeros() as usize;
            duel.step_raw_action(ACTS[first]).unwrap();
        }
        assert_eq!(duel.phase_id(), FEY_STEP);
        assert_ne!(duel.fighter_point(0), origin);
        assert_eq!(duel.f[0].arcane.fey_steps, charges - 1);
        assert_eq!(duel.f[0].slots, [0; 3]);
        duel.step_raw_action(ACTS[0]).unwrap();
        assert!((1..=10).contains(&duel.f[0].temp));
    }
}

impl Battle {
    pub(super) fn arcane_calm_affects(&self, caster: usize, target: usize) -> bool {
        let state = &self.f[caster].arcane;
        (state.calm_indifferent | state.calm_immune) & (1 << target) != 0
    }

    pub(super) fn arcane_end_calm_target(&mut self, caster: usize, target: usize) {
        let state = &mut self.f[caster].arcane;
        state.calm_indifferent &= !(1 << target);
        state.calm_immune &= !(1 << target);
        if state.calm_indifferent | state.calm_immune == 0 {
            self.f[caster].concentration = None;
        }
    }
}

impl Battle {
    pub(super) fn arcane_sense_thoughts(&mut self, caster: usize) {
        let origin = self.fighter_point(caster);
        let sensed = (0..4).filter(|&target| target != caster).any(|target| {
            self.f
                .get(target)
                .map_or_else(|| self.entity(target).is_some(), |fighter| fighter.hp > 0)
                && self.rope_same_space(caster, target)
                && !self.magic_nondetection(target)
                && self.footprint(target).is_some_and(|body| {
                    body.squares()
                        .any(|point| origin.dist(point) <= 6 && self.map.los_at(origin, point))
                })
        });
        self.f[caster].arcane.sensed_presence = sensed;
    }

    pub(super) fn arcane_status(&self, i: usize) -> Vec<String> {
        let state = &self.f[i].arcane;
        let mut items = Vec::new();
        if let Some(ward) = state.ward {
            items.push(format!(
                "Arcane Ward: {ward}/{} HP",
                self.arcane_ward_max(i)
            ));
        }
        if state.portent.iter().any(|&die| die > 0) {
            let dice = state
                .portent
                .iter()
                .filter(|&&die| die > 0)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            items.push(format!("Portent: {dice}"));
        }
        if self.f[i].concentration == Some(Concentration::DetectThoughts) {
            items.push(
                if state.sensed_presence {
                    "Sense Thoughts: presence detected"
                } else {
                    "Sense Thoughts: no presence detected"
                }
                .into(),
            );
        }
        items
    }
}
