//! Recipient selection and support effects shared by players and companions.
use super::*;
use crate::arena2::entities::EntityKind;

const REMOVE_PARALYZED: u8 = 16;
const REFUSED_CONSENT: u8 = 32;

fn recipient_action(subset: u8) -> Act {
    ACTS[if subset == 15 { 31 } else { subset as usize }]
}

fn recipient_subset(action_index: u8) -> u8 {
    if action_index == 31 {
        15
    } else {
        action_index
    }
}

impl Spell {
    pub(super) fn is_ally_support(self) -> bool {
        matches!(
            self,
            Self::Bless
                | Self::Aid
                | Self::ShieldOfFaith
                | Self::MassHealingWord
                | Self::Protection
                | Self::Sanctuary
                | Self::LesserRestoration
                | Self::MageArmor
                | Self::Longstrider
                | Self::Heroism
                | Self::Fly
                | Self::Guidance
                | Self::Invisibility
                | Self::Nondetection
                | Self::EnhanceAbility
                | Self::Resistance
                | Self::ProtectionFromEnergy
                | Self::Jump
        )
    }

    fn support_range(self) -> i32 {
        match self {
            Self::Protection
            | Self::LesserRestoration
            | Self::MageArmor
            | Self::Longstrider
            | Self::Heroism
            | Self::Fly
            | Self::Guidance
            | Self::Invisibility
            | Self::Nondetection
            | Self::EnhanceAbility
            | Self::Resistance
            | Self::ProtectionFromEnergy
            | Self::Jump => 1,
            Self::Bless | Self::Aid | Self::Sanctuary => 6,
            Self::ShieldOfFaith | Self::MassHealingWord => 12,
            _ => 0,
        }
    }
}

impl Battle {
    pub(super) fn magic_ally_candidates(&self, side: usize, spell: Spell) -> u8 {
        if !spell.is_ally_support() {
            return 0;
        }
        let range = spell.support_range();
        (0..4).fold(0, |mask, id| {
            let sight = spell == Spell::MassHealingWord;
            let direct = self.magic_reaches_known_creature(side, id, range, sight);
            if !direct && (range != 1 || !self.entity_can_deliver_touch(side, id)) {
                return mask;
            }
            let relevant = spell != Spell::MageArmor || id >= 2 || !self.armor_worn(id);
            if relevant {
                mask | 1 << id
            } else {
                mask
            }
        })
    }

    pub(super) fn magic_ally_choice_mask(&self, side: usize, action: Act) -> u128 {
        let Some(spell) = self.prepared_spell(side, action) else {
            return 0;
        };
        let available = self.magic_ally_candidates(side, spell);
        if spell == Spell::LesserRestoration {
            return (0..4).fold(Act::Pass.bit(), |mask, id| {
                let mut choices = mask;
                let subset = 1u8 << id;
                if available & subset == 0 {
                    return choices;
                }
                let poisoned = self.martial_poisoned(id)
                    || self.magic_area_poisoned(id)
                    || id < 2 && self.magic_poisoned(id);
                let paralyzed = id < 2 && self.magic_paralyzed(id);
                if poisoned || !paralyzed {
                    choices |= ACTS[subset as usize].bit();
                }
                if paralyzed {
                    choices |= ACTS[(REMOVE_PARALYZED + subset) as usize].bit();
                }
                choices
            });
        }
        let maximum = match spell {
            Spell::Bless => 2 + self.ally_spell_tier(side, action) as u32,
            Spell::Longstrider | Spell::Heroism | Spell::Jump => {
                self.ally_spell_tier(side, action) as u32
            }
            Spell::Fly => self.ally_spell_tier(side, action).saturating_sub(2).max(1) as u32,
            Spell::Invisibility | Spell::EnhanceAbility => {
                self.ally_spell_tier(side, action).saturating_sub(1).max(1) as u32
            }
            Spell::Aid => 3,
            Spell::MassHealingWord => 6,
            _ => 1,
        };
        (1u8..16)
            .filter(|&subset| subset & !available == 0 && subset.count_ones() <= maximum)
            .fold(Act::Pass.bit(), |mask, subset| {
                mask | recipient_action(subset).bit()
            })
    }

    fn ally_spell_tier(&self, side: usize, action: Act) -> usize {
        self.f[side]
            .magic
            .cast_tier
            .or_else(|| self.slot_for_action(side, action))
            .unwrap_or(1)
    }

    pub(super) fn magic_ally_choice_label(
        &self,
        side: usize,
        action: Act,
        choice: Act,
    ) -> Option<String> {
        if self.magic_ally_choice_mask(side, action) & choice.bit() == 0 {
            return None;
        }
        if choice == Act::Pass {
            return Some("Cancel recipient selection".to_owned());
        }
        let chosen = recipient_subset(choice as u8);
        let recipients: Vec<_> = (0..4)
            .filter(|&id| chosen & (1 << id) != 0)
            .map(|id| {
                if id == side {
                    "self"
                } else if id < 2 {
                    "opposing adventurer"
                } else {
                    self.entity(id).expect("legal recipient").kind.name()
                }
            })
            .collect();
        match self.prepared_spell(side, action) {
            Some(Spell::LesserRestoration) => {
                let condition = if chosen & REMOVE_PARALYZED != 0 {
                    "Paralyzed"
                } else {
                    "Poisoned"
                };
                Some(format!("End {condition} on {}", recipients.join(" and ")))
            }
            _ => Some(format!("Target {}", recipients.join(" and "))),
        }
    }

    pub(super) fn magic_commit_ally_choice(&mut self, caster: usize, action: Act, subset: u8) {
        let subset = recipient_subset(subset);
        let hostile = (1 << (1 - caster)) | (1 << (3 - caster));
        if matches!(
            self.prepared_spell(caster, action),
            Some(
                Spell::Protection
                    | Spell::MageArmor
                    | Spell::Heroism
                    | Spell::Fly
                    | Spell::Guidance
                    | Spell::Invisibility
                    | Spell::Nondetection
                    | Spell::EnhanceAbility
                    | Spell::Resistance
                    | Spell::ProtectionFromEnergy
                    | Spell::Jump
            )
        ) && subset & hostile != 0
        {
            self.magic_request(
                1 - caster,
                MagicChoice::AllyConsent {
                    caster,
                    action,
                    subset,
                },
            );
        } else {
            self.magic_complete_ally_consent(caster, action, subset, Act::Defend);
        }
    }

    pub(super) fn magic_ally_consent_mask(&self, side: usize, subset: u8) -> u128 {
        let mut mask = Act::Defend.bit() | Act::Pass.bit();
        let controlled = (1 << side) | (1 << (side + 2));
        if subset & controlled == controlled {
            mask |= ACTS[side].bit() | ACTS[side + 2].bit();
        }
        mask
    }

    pub(super) fn magic_ally_consent_label(&self, side: usize, choice: Act) -> Option<String> {
        Some(match choice {
            Act::Defend => "Accept spell for all selected creatures".into(),
            Act::Pass => "Decline spell for all selected creatures".into(),
            choice if choice as usize == side => "Accept spell for yourself only".into(),
            choice if choice as usize == side + 2 => "Accept spell for your companion only".into(),
            _ => return None,
        })
    }

    pub(super) fn accepted_magic_ally_subset(side: usize, subset: u8, consent: Act) -> u8 {
        let controlled = (1 << side) | (1 << (side + 2));
        match consent {
            Act::Defend => subset,
            Act::Pass => subset & !controlled,
            id => (subset & !controlled) | (subset & (1 << id as usize)),
        }
    }

    pub(super) fn magic_complete_ally_consent(
        &mut self,
        caster: usize,
        action: Act,
        subset: u8,
        consent: Act,
    ) {
        let recipients = Self::accepted_magic_ally_subset(1 - caster, subset, consent);
        self.f[caster].magic.ally_selection = if recipients == 0 {
            REFUSED_CONSENT
        } else {
            recipients
        };
        self.execute_magic_step(caster, action);
    }

    pub(in crate::arena2) fn magic_ally_policy(&self, actor: usize, mask: u128) -> Option<Act> {
        if self.phase_id() != 48 {
            return None;
        }
        match self.f[actor].magic.choice? {
            MagicChoice::Allies(_) => {
                let own = (1 << actor) | (1 << (2 + actor));
                ACTS.iter()
                    .copied()
                    .filter(|action| mask & action.bit() != 0 && *action != Act::Pass)
                    .filter_map(|action| {
                        let recipients = recipient_subset(action as u8) & 0b1111;
                        (recipients & !own == 0).then_some((
                            recipients.count_ones(),
                            -i16::from(action as u8),
                            action,
                        ))
                    })
                    .max_by_key(|&(count, tie, _)| (count, tie))
                    .map(|(_, _, action)| action)
                    .or(Some(Act::Pass))
            }
            MagicChoice::AllyConsent { .. } => Some(Act::Defend),
            _ => None,
        }
    }

    pub(super) fn magic_apply_ally_spell(
        &mut self,
        side: usize,
        spell: Spell,
        tier: usize,
    ) -> bool {
        use Spell::*;
        if !spell.is_ally_support() {
            return false;
        }
        let chosen = std::mem::take(&mut self.f[side].magic.ally_selection);
        let recipients = if chosen == 0 {
            1 << side
        } else {
            chosen & 0b1111
        };
        if recipients == 0 {
            return true;
        }
        if spell.support_range() == 1 {
            let remote = (0..4).find(|&id| {
                recipients & (1 << id) != 0
                    && !self.magic_reaches_known_creature(side, id, 1, false)
            });
            if let Some(target) = remote {
                if !self.entity_touch_reaction(side, target) {
                    return true;
                }
            }
        }
        match spell {
            Heroism | Fly | Guidance | Resistance | ProtectionFromEnergy | Jump | Invisibility | EnhanceAbility => {
                self.magic_apply_willing_buff(side, spell, tier, recipients);
                if spell == Jump {
                    let expires = self.turns + 20;
                    for id in 2..4 {
                        if recipients & (1 << id) != 0 {
                            if let Some(entity) = self.entity_mut(id) {
                                entity.jump_until = expires;
                            }
                        }
                    }
                }
            }
            Nondetection => {
                for id in (0..4).filter(|&id| recipients & (1 << id) != 0) {
                    self.f[side].magic.nondetection_until[id] = self.turns + 9_600;
                }
            }
            MageArmor | Longstrider => {
                let until = self.turns + if spell == MageArmor { 9_600 } else { 1_200 };
                for id in (0..4).filter(|&id| recipients & (1 << id) != 0) {
                    if id < 2 {
                        if spell == MageArmor {
                            self.f[id].magic.mage_armor_until = until;
                        } else {
                            self.f[id].magic.longstrider_until = until;
                        }
                    } else if let Some(entity) = self.entity_mut(id) {
                        if spell == MageArmor {
                            entity.mage_armor_until = until;
                        } else {
                            entity.longstrider_until = until;
                        }
                    }
                }
            }
            Bless | ShieldOfFaith | Protection => {
                let effect = match spell {
                    Bless => Concentration::Bless,
                    ShieldOfFaith => Concentration::ShieldOfFaith,
                    _ => Concentration::Protection,
                };
                let rounds = if spell == Bless { 10 } else { 100 };
                self.concentrate(side, effect, rounds, None, tier);
                self.f[side].magic.ally_recipients = recipients;
            }
            Aid => {
                let bonus = 5 * (tier as i32 - 1);
                for id in (0..4).filter(|&i| recipients & (1 << i) != 0) {
                    if id < 2 {
                        let previous = self.f[id].magic.aid_hp;
                        self.f[id].magic.aid_hp = previous.max(bonus);
                        self.f[id].hp += (bonus - previous).max(0);
                    } else if let Some(entity) = self.entity_mut(id) {
                        let previous = entity.aid_bonus;
                        entity.aid_bonus = previous.max(bonus);
                        entity.hp += (bonus - previous).max(0);
                    }
                }
            }
            Sanctuary => {
                let until = self.turns + 20;
                for id in (0..4).filter(|&i| recipients & (1 << i) != 0) {
                    if id < 2 {
                        self.f[id].magic.sanctuary_until = until;
                        self.f[id].magic.sanctuary_source = Some(side);
                    } else if let Some(entity) = self.entity_mut(id) {
                        entity.sanctuary = Some((side, until));
                    }
                }
            }
            MassHealingWord => {
                let amount = self.roll_many(tier as i32 - 1, 4)
                    + self.f[side].build.casting
                    + if self.is_subclass(side, Class::Cleric, 0) && self.f[side].build.level >= 3
                    {
                        tier as i32 + 2
                    } else {
                        0
                    };
                for id in (0..4).filter(|&i| recipients & (1 << i) != 0) {
                    if id < 2 {
                        self.magic_heal(id, amount);
                    } else {
                        self.entity_heal(id, amount);
                    }
                }
            }
            LesserRestoration => {
                for id in (0..4).filter(|&i| recipients & (1 << i) != 0) {
                    if id < 2
                        && (chosen & REMOVE_PARALYZED != 0
                            || chosen == 0 && self.magic_paralyzed(id))
                    {
                        self.f[id].magic.held = false;
                    } else {
                        if id < 2 {
                            self.f[id].magic.poisoned_turn = -1;
                        }
                        self.martial_clear_poison(id);
                        self.area_cure_poison(id);
                    }
                }
            }
            _ => unreachable!(),
        }
        true
    }

    pub(in crate::arena2) fn apply_companion_heroism(&mut self, id: usize) {
        let Some(source) = self.magic_heroism_source(id) else {
            return;
        };
        let amount = self.f[source].build.casting;
        self.grant_temporary_hp(id, amount, TempSource::Heroism(source));
    }

    fn magic_ally_has_effect(&self, id: usize, effect: Concentration) -> bool {
        self.f
            .iter()
            .any(|f| f.concentration == Some(effect) && (f.magic.ally_recipients & (1 << id) != 0))
    }

    pub(in crate::arena2) fn magic_entity_base_ac(&self, id: usize) -> Option<i32> {
        let entity = self.entity(id)?;
        Some(if entity.mage_armor_until > self.turns {
            13 + entity.dexterity_modifier()
        } else {
            entity.kind.stats().ac
        })
    }

    pub(in crate::arena2) fn magic_blessed(&self, id: usize) -> bool {
        self.magic_ally_has_effect(id, Concentration::Bless)
    }

    pub(in crate::arena2) fn magic_ally_shielded(&self, id: usize) -> bool {
        self.magic_ally_has_effect(id, Concentration::ShieldOfFaith)
    }

    pub(in crate::arena2) fn magic_ally_protection(&self, source: usize, target: usize) -> bool {
        source >= 2
            && self.entity(source).is_some_and(|entity| {
                matches!(
                    entity.kind,
                    EntityKind::Familiar { .. } | EntityKind::Steed { .. }
                )
            })
            && self.magic_ally_has_effect(target, Concentration::Protection)
    }

    pub(in crate::arena2) fn magic_forget_companion_benefits(&mut self, side: usize) {
        self.divine_forget_companion(side + 2);
        // Entity IDs are stable slots. Replacing a creature must not transfer
        // concentration effects intended for its predecessor.
        for caster in &mut self.f {
            caster.magic.ally_recipients &= !(1 << (side + 2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::entities::{Entity, FamiliarForm, Spirit};
    use crate::arena2::geometry::Pos;

    fn companion_encounter() -> Battle {
        companion_encounter_for(Class::Bard)
    }

    fn companion_encounter_for(class: Class) -> Battle {
        let mut battle = Battle::new(0, [class, Class::Druid], [5; 2]);
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 0, y: 1 };
        battle.fixture_companion(
            1,
            Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Cat,
                    spirit: Spirit::Fey,
                },
                1,
                Pos { x: 1, y: 0 },
                10,
            ),
        );
        battle
    }

    #[test]
    fn healing_word_can_target_full_health_and_heal_an_injured_companion() {
        let mut battle = companion_encounter();
        battle.owner = 0;
        let maximum = battle.magic_max_hp(1);
        assert_eq!(battle.f[1].hp, maximum);
        assert_ne!(
            battle.creature_choices(0, Spell::HealingWord) & ACTS[1].bit(),
            0
        );
        assert_ne!(
            battle.creature_choices(0, Spell::HealingWord) & ACTS[3].bit(),
            0
        );
        let tier_one_slots = battle.f[0].slots[0];
        battle.step(Act::Spell2).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[1]).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.f[1].hp, maximum);
        assert_eq!(battle.f[0].slots[0], tier_one_slots - 1);

        let mut battle = companion_encounter();
        battle.owner = 0;
        let familiar = battle.entity_mut(3).unwrap();
        let maximum = familiar.max_hp();
        familiar.hp = 1;
        battle.step(Act::Spell2).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[3]).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.entity(3).unwrap().hp, maximum);
    }

    #[test]
    fn cure_wounds_can_touch_a_known_creature_in_heavy_obscurity() {
        let mut battle = companion_encounter();
        let cloud = battle.fighter_point(1);
        battle.concentrate(1, Concentration::StinkingCloud, 10, Some(cloud), 3);
        assert!(!battle.can_see(0, 1));
        assert!(battle.observed_pos(0, 1).is_some());
        assert_ne!(
            battle.creature_choices(0, Spell::CureWounds) & ACTS[1].bit(),
            0
        );
        assert_eq!(
            battle.creature_choices(0, Spell::HealingWord) & ACTS[1].bit(),
            0
        );
    }

    #[test]
    fn mass_healing_word_heals_the_chosen_group_independently_of_affiliation() {
        let mut battle = companion_encounter_for(Class::Cleric);
        battle.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        battle.owner = 0;
        battle.f[0].hp = 1;
        battle.f[1].hp = 1;
        battle.entity_mut(3).unwrap().hp = 1;
        let familiar_maximum = battle.entity(3).unwrap().max_hp();
        let slots = battle.f[0].slots[2];
        battle.step(Act::Magic3).unwrap();
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[32 + 0b1010]).unwrap();
        assert_eq!(battle.f[0].hp, 1);
        assert!(battle.f[1].hp > 1);
        assert_eq!(battle.entity(3).unwrap().hp, familiar_maximum);
        assert_eq!(battle.f[0].slots[2], slots - 1);
    }

    #[test]
    fn lesser_restoration_can_end_the_selected_poison_on_opponent() {
        let mut battle = companion_encounter_for(Class::Cleric);
        battle.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        battle.owner = 0;
        battle.f[1].magic.poisoned_turn = battle.turns;
        battle.step(Act::Magic2).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[1]).unwrap();
        }
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[1]).unwrap();
        assert!(!battle.magic_poisoned(1));
    }

    #[test]
    fn bless_can_select_the_companion_and_leave_its_owner_unblessed() {
        let mut battle = companion_encounter_for(Class::Cleric);
        battle.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        battle.owner = 0;
        battle.step(Act::Spell5).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[32 + 0b1001]).unwrap();
        assert!(battle.magic_blessed(0));
        assert!(battle.magic_blessed(3));
        assert!(!battle.magic_blessed(1));
    }

    #[test]
    fn controller_may_accept_willing_touch_for_the_companion_only() {
        let mut battle = companion_encounter_for(Class::Paladin);
        battle.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        battle.owner = 0;
        battle.step(Act::Magic9).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[1]).unwrap();
        }
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[32 + 0b1010]).unwrap();
        assert_eq!(battle.actor(), Some(1));
        assert_ne!(battle.mask() & ACTS[2].bit(), 0);
        battle.step(ACTS[2]).unwrap();
        assert!(!battle.magic_heroism_immune(1));
        assert!(battle.magic_heroism_immune(3));
    }

    #[test]
    fn readied_heroism_controller_can_accept_only_for_the_companion() {
        let mut battle = companion_encounter_for(Class::Paladin);
        battle.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        battle.owner = 0;
        let slots = battle.f[0].slots[1];
        battle.step(Act::Ready).unwrap();
        battle.step(ACTS[6]).unwrap(); // Ready when the enemy uses an action.
        battle.step(Act::Magic9).unwrap();
        assert_eq!(battle.phase_id(), 107);
        battle.step(ACTS[1]).unwrap();
        assert_eq!(battle.f[0].slots[1], slots - 1);
        for _ in 0..3 {
            if battle.phase_id() != 0 || battle.actor() != Some(1) {
                battle.step(Act::End).unwrap();
            }
        }
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.phase_id(), 0);
        battle.step(Act::Dodge).unwrap();
        assert_eq!(battle.phase_id(), 9);
        battle.step(Act::Ready).unwrap();
        assert_eq!(battle.phase_id(), 108);
        battle.step(ACTS[32 + 0b1010]).unwrap();
        assert_eq!(battle.phase_id(), 108);
        assert_eq!(battle.actor(), Some(1));
        let mask = battle.mask();
        assert_ne!(mask & ACTS[0].bit(), 0);
        assert_ne!(mask & ACTS[2].bit(), 0);
        battle.step(ACTS[2]).unwrap();
        assert!(battle.magic_heroism_immune(3));
        assert!(!battle.magic_heroism_immune(1));
        assert_eq!(battle.f[0].slots[1], slots - 1);
        assert!(!battle.f[0].reaction);
    }

    #[test]
    fn fly_grants_companion_elevation_and_losing_the_spell_causes_a_fall() {
        let mut battle = companion_encounter();
        battle.magic_apply_willing_buff(0, Spell::Fly, 3, 1 << 3);
        assert_eq!(battle.entity_effective_speed(3), 12);
        let sky = (1..6)
            .flat_map(|x| {
                (1..6).map(move |y| Point {
                    cell: Pos { x, y },
                    altitude: 1,
                })
            })
            .find(|&point| battle.map.can_occupy(point) && !battle.map.touches_structure(point))
            .expect("arena fixture has airborne space away from walls");
        assert!(battle.entity_force_move(3, sky));
        battle.settle_companion_flight();
        assert_eq!(battle.entity(3).unwrap().altitude, 1);
        battle.f[0].concentration = None;
        battle.settle_companion_flight();
        assert_eq!(battle.entity(3).unwrap().altitude, 0);
    }

    #[test]
    fn energy_ward_applies_only_to_chosen_companion_and_element() {
        let mut battle = companion_encounter();
        battle.f[0].magic.element = DamageType::Fire;
        battle.magic_apply_willing_buff(0, Spell::ProtectionFromEnergy, 3, 1 << 3);
        assert!(battle.magic_energy_resistance(3, DamageType::Fire));
        assert!(!battle.magic_energy_resistance(1, DamageType::Fire));
        assert!(!battle.magic_energy_resistance(3, DamageType::Cold));
        let before = battle.entity(3).unwrap().hp;
        battle.entity_damage_typed(3, 3, DamageType::Fire);
        assert_eq!(battle.entity(3).unwrap().hp, before - 1);
    }

    #[test]
    fn companion_heroism_refreshes_temp_hit_points_at_start_of_turn() {
        let mut battle = companion_encounter();
        battle.magic_apply_willing_buff(0, Spell::Heroism, 1, 1 << 3);
        assert_eq!(battle.entity(3).unwrap().temp_hp, 0);
        battle.apply_companion_heroism(3);
        let amount = battle.f[0].build.casting;
        let hp = battle.entity(3).unwrap().hp;
        assert_eq!(battle.entity(3).unwrap().temp_hp, amount);
        battle.entity_damage_typed(3, 1, DamageType::Slashing);
        assert_eq!(battle.entity(3).unwrap().hp, hp);
        assert_eq!(battle.entity(3).unwrap().temp_hp, amount - 1);
        battle.entity_damage_typed(3, amount - 1, DamageType::Slashing);
        assert_eq!(battle.entity(3).unwrap().temp_hp, 0);
        battle.apply_companion_heroism(3);
        assert_eq!(battle.entity(3).unwrap().temp_hp, amount);
    }
}
