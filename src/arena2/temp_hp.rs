//! Temporary HP never stacks. Its current source is retained for choice display.

use super::{battle::Battle, class::Class};

pub(super) const TEMP_CHOICE_PHASE: i32 = 116;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TempSource {
    #[default]
    Unspecified,
    Heroism(usize),
    FiendishVigor,
    WildShape,
    DarkOnesBlessing,
    RefreshingStep,
    InspiringSmite,
    VitalitySurge,
    LifeGivingForce(usize),
    MantleOfInspiration,
}

impl TempSource {
    pub(super) fn name(self, recipient: usize) -> &'static str {
        match self {
            Self::Unspecified => "an earlier effect",
            Self::Heroism(caster) if caster == recipient % 2 => "your Heroism",
            Self::Heroism(_) => "the other duelist's Heroism",
            Self::FiendishVigor => "Fiendish Vigor",
            Self::WildShape => "Wild Shape",
            Self::InspiringSmite => "Inspiring Smite",
            Self::DarkOnesBlessing => "Dark One's Blessing",
            Self::RefreshingStep => "Refreshing Step",
            Self::VitalitySurge => "Vitality Surge",
            Self::LifeGivingForce(_) => "Life-Giving Force",
            Self::MantleOfInspiration => "Mantle of Inspiration",
        }
    }

    pub(super) fn caster_feature(self, recipient: usize) -> f32 {
        match self {
            Self::Heroism(caster) if caster == recipient % 2 => 0.5,
            Self::Heroism(_) => 1.0,
            _ => 0.,
        }
    }

    pub(super) fn code(self) -> f32 {
        match self {
            Self::Unspecified => 0.,
            Self::Heroism(_) => 0.25,
            Self::FiendishVigor => 0.50,
            Self::WildShape => 0.75,
            Self::DarkOnesBlessing => 1.,
            Self::RefreshingStep => 0.125,
            Self::InspiringSmite => 0.875,
            Self::VitalitySurge => 0.875,
            Self::LifeGivingForce(_) => 0.9375,
            Self::MantleOfInspiration => 0.96875,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TempOffer {
    pub recipient: usize,
    pub current: i32,
    pub proposed: i32,
    pub current_source: TempSource,
    pub source: TempSource,
}

#[derive(Clone, Debug, Default)]
pub(super) struct TempHpState {
    source: [TempSource; 4],
}

impl Battle {
    pub(super) fn may_need_temp_choice(&self) -> bool {
        self.f.iter().any(|f| {
            f.temp > 0
            // Multiple enemy deaths can grant two Dark One's Blessings in
            // one action, even when the Warlock begins with no temporary HP.
            || f.build.is_subclass(Class::Warlock, 0) && f.build.level >= 3
        }) || (2..4).any(|id| self.entity(id).is_some_and(|entity| entity.temp_hp > 0))
    }

    fn current_temporary_hp(&self, id: usize) -> Option<i32> {
        if id < 2 {
            Some(self.f[id].temp)
        } else {
            self.entity(id).map(|e| e.temp_hp)
        }
    }

    pub(in crate::arena2) fn grant_temporary_hp(
        &mut self,
        recipient: usize,
        amount: i32,
        source: TempSource,
    ) {
        if amount <= 0 {
            return;
        }
        let Some(current) = self.current_temporary_hp(recipient) else {
            return;
        };
        let current_source = self.temporary_hp.source[recipient];
        if current > 0 {
            if amount == current && source == current_source {
                return;
            }
            let offer = TempOffer {
                recipient,
                current,
                proposed: amount,
                current_source,
                source,
            };
            if !self.decide_temp_replacement(offer) {
                return;
            }
        }
        if recipient < 2 {
            self.f[recipient].temp = amount;
        } else if let Some(entity) = self.entity_mut(recipient) {
            entity.temp_hp = amount;
        }
        self.temporary_hp.source[recipient] = source;
    }

    pub(in crate::arena2) fn clear_temporary_hp_by_source(&mut self, source: TempSource) {
        for recipient in 0..4 {
            if self.temporary_hp.source[recipient] != source {
                continue;
            }
            if recipient < 2 {
                self.f[recipient].temp = 0;
            } else if let Some(entity) = self.entity_mut(recipient) {
                entity.temp_hp = 0;
            }
            self.temporary_hp.source[recipient] = TempSource::Unspecified;
        }
    }

    pub fn temporary_hp_source(&self, recipient: usize) -> f32 {
        if self
            .current_temporary_hp(recipient)
            .is_some_and(|hp| hp > 0)
        {
            self.temporary_hp.source[recipient].code()
        } else {
            0.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{battle::Concentration, Act};

    fn heroism_start(current: i32, source: TempSource) -> Battle {
        let mut b = Battle::new(13, [Class::Paladin, Class::Fighter], [5; 2]);
        b.owner = 0;
        b.concentrate(0, Concentration::Heroism, 10, None, 1);
        b.grant_temporary_hp(0, current, source);
        b.step(Act::End).unwrap();
        assert_eq!(b.owner, 1);
        b
    }

    #[test]
    fn heroism_can_replace_a_larger_existing_pool_or_keep_it() {
        for choice in [Act::Defend, Act::Pass] {
            let mut b = heroism_start(12, TempSource::FiendishVigor);
            let grant = b.f[0].build.casting;
            b.step(Act::End).unwrap();
            assert_eq!(b.phase_id(), TEMP_CHOICE_PHASE);
            assert_eq!(b.actor(), Some(0));
            assert_eq!(b.owner, 1);
            assert_eq!(b.f[0].temp, 12);
            assert!(b.choice_label(0, Act::Pass).contains("Fiendish Vigor"));
            b.step(choice).unwrap();
            assert_eq!(b.owner, 0);
            assert!(b.action);
            assert_eq!(b.f[0].temp, if choice == Act::Defend { grant } else { 12 });
            assert_eq!(
                b.temporary_hp_source(0),
                if choice == Act::Defend { 0.25 } else { 0.50 }
            );
        }
    }

    #[test]
    fn equal_pools_from_two_sources_still_offer_a_provenance_choice() {
        let casting = crate::arena2::class::build(Class::Paladin, 5).casting;
        let mut b = heroism_start(casting, TempSource::Heroism(1));
        b.step(Act::End).unwrap();
        assert_eq!(b.phase_id(), TEMP_CHOICE_PHASE);
        assert!(b
            .choice_label(0, Act::Pass)
            .contains("other duelist's Heroism"));
        assert!(b.choice_label(0, Act::Defend).contains("your Heroism"));
        b.step(Act::Defend).unwrap();
        assert_eq!(b.f[0].temp, casting);
        assert_eq!(b.temporary_hp.source[0], TempSource::Heroism(0));
    }

    #[test]
    fn heroism_ends_future_refresh_but_keeps_existing_points_in_2024() {
        let mut b = heroism_start(0, TempSource::Unspecified);
        b.step(Act::End).unwrap();
        let amount = b.f[0].temp;
        assert!(amount > 0);
        b.step(Act::DropConcentration).unwrap();
        assert_eq!(b.f[0].temp, amount);
        assert!(!b.magic_heroism_immune(0));
        b.damage(0, amount);
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        assert_eq!(b.f[0].temp, 0);
    }

    #[test]
    fn wild_shape_replacement_can_be_declined_without_reverting_the_transform() {
        let mut b = Battle::new(3, [Class::Druid, Class::Fighter], [2; 2]);
        b.owner = 0;
        b.grant_temporary_hp(0, 9, TempSource::Heroism(1));
        let uses = b.f[0].resource;
        b.step(Act::Special1).unwrap();
        for _ in 0..8 {
            if b.phase_id() == TEMP_CHOICE_PHASE {
                break;
            }
            assert_ne!(b.phase_id(), 0);
            let selection = crate::arena2::ACTS[b.mask().trailing_zeros() as usize];
            b.step(selection).unwrap();
        }
        assert_eq!(b.phase_id(), TEMP_CHOICE_PHASE);
        assert!(!b.magic_is_wolf(0));
        assert_eq!(b.f[0].resource, uses);
        assert!(b
            .choice_label(0, Act::Defend)
            .contains("2 temporary HP from Wild Shape"));
        b.step(Act::Pass).unwrap();
        assert!(b.magic_is_wolf(0));
        assert_eq!(b.f[0].resource, uses - 1);
        assert_eq!(b.f[0].temp, 9);
        assert!(!b.bonus);
    }

    #[test]
    fn dark_ones_blessing_pauses_at_a_real_defeat_and_commits_once() {
        for seed in 1..=128 {
            let mut b = Battle::new(seed, [Class::Warlock, Class::Fighter], [3; 2]);
            b.owner = 0;
            b.f[0].pos = crate::arena2::geometry::Pos { x: 0, y: 0 };
            b.f[1].pos = crate::arena2::geometry::Pos { x: 1, y: 0 };
            b.f[1].hp = 1;
            b.grant_temporary_hp(0, 12, TempSource::FiendishVigor);
            b.step(Act::Melee).unwrap();
            if b.phase_id() != TEMP_CHOICE_PHASE {
                continue;
            }
            let new_hp = b.f[0].build.casting + b.f[0].build.level;
            assert_eq!(b.actor(), Some(0));
            assert_eq!(b.f[1].hp, 1);
            assert_eq!(b.f[0].temp, 12);
            assert!(b
                .choice_label(0, Act::Defend)
                .contains("Dark One's Blessing"));
            assert_eq!(b.pending_features()[3], 1.);
            b.step(Act::Defend).unwrap();
            assert_eq!(b.winner(), Some(0));
            assert_eq!(b.f[0].temp, new_hp);
            assert_eq!(b.temporary_hp_source(0), 1.);
            return;
        }
        panic!("no seed exercised the qualifying defeat");
    }

    #[test]
    fn fiendish_vigor_exposes_a_real_replacement_choice_after_casting() {
        let mut b = Battle::new(13, [Class::Warlock, Class::Fighter], [2; 2]);
        b.owner = 0;
        b.grant_temporary_hp(0, 5, TempSource::DarkOnesBlessing);
        b.step(Act::Special1).unwrap();
        assert_eq!(b.phase_id(), TEMP_CHOICE_PHASE);
        assert!(b.action, "the cast has not yet committed");
        assert_eq!(b.f[0].temp, 5);
        b.step(Act::Defend).unwrap();
        assert!(!b.action);
        assert_eq!(b.f[0].temp, 12);
        assert_eq!(b.temporary_hp_source(0), 0.50);
    }

    #[test]
    fn wild_shape_replacement_preserves_form_and_bonus_action_cost() {
        use crate::arena2::ACTS;
        for choice in [Act::Defend, Act::Pass] {
            let mut b = Battle::new(19, [Class::Druid, Class::Fighter], [4; 2]);
            b.owner = 0;
            b.grant_temporary_hp(0, 9, TempSource::Heroism(1));
            let before = b.f[0].resource;
            b.step(Act::Special1).unwrap();
            b.step(ACTS[1]).unwrap(); // Rat.
            b.step(ACTS[0]).unwrap(); // Merge gear.
            assert_eq!(b.phase_id(), TEMP_CHOICE_PHASE);
            assert_eq!(b.f[0].temp, 9);
            assert!(b.choice_label(0, Act::Defend).contains("Wild Shape"));
            b.step(choice).unwrap();
            assert!(b.magic_is_wolf(0));
            assert!(b.action && !b.bonus);
            assert_eq!(b.f[0].resource, before - 1);
            assert_eq!(b.f[0].temp, if choice == Act::Defend { 4 } else { 9 });
        }
    }

    #[test]
    fn dark_ones_blessing_replacement_follows_an_actual_companion_defeat() {
        use crate::arena2::{
            entities::{Entity, EntityKind, FamiliarForm, Spirit},
            geometry::Pos,
            ACTS,
        };
        let mut pending = None;
        for seed in 0..32 {
            let mut b = Battle::new(seed, [Class::Warlock, Class::Wizard], [3; 2]);
            b.owner = 0;
            b.f[0].pos = Pos { x: 0, y: 0 };
            b.f[1].pos = Pos { x: 2, y: 0 };
            b.fixture_companion(1, Entity::new(
                EntityKind::Familiar { form: FamiliarForm::Owl, spirit: Spirit::Fey },
                1, Pos { x: 1, y: 0 }, 0,
            ));
            b.grant_temporary_hp(0, 12, TempSource::FiendishVigor);
            b.step(Act::Spell0).unwrap(); // Eldritch Blast.
            b.step(ACTS[3]).unwrap(); // Aim at the enemy familiar.
            if b.phase_id() == TEMP_CHOICE_PHASE {
                pending = Some(b);
                break;
            }
        }
        let pending = pending.expect("at least one seeded beam defeats the familiar");
        let amount = 3 + pending.f[0].build.casting;
        for choice in [Act::Defend, Act::Pass] {
            let mut b = pending.clone();
            assert_eq!(b.actor(), Some(0));
            assert!(b.choice_label(0, Act::Defend).contains("Dark One's Blessing"));
            b.step(choice).unwrap();
            assert!(b.entity(3).is_none());
            assert!(!b.action);
            assert!(b.f[0].reaction);
            assert_eq!(b.f[0].temp, if choice == Act::Defend { amount } else { 12 });
        }
    }

}
