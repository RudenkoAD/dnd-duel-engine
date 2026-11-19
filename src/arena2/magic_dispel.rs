use super::{Battle, Concentration};
use crate::arena2::class::Class;
use crate::arena2::equipment::Carried;
use crate::arena2::geometry::Point;
use crate::arena2::objects::{END_OBJECTS, FIRST_OBJECT};

const CARRIED_FOCUS: usize = 28;
const AREA_EFFECT: usize = 32;
const ZONE_OF_TRUTH: usize = 34;
const ROPE_TRICK: usize = 36;
const RANGE: i32 = 24;

impl Battle {
    pub(super) fn dispel_targets(&self, actor: usize) -> u128 {
        let origin = self.fighter_point(actor);
        let mut choices = (0..4)
            .filter(|&target| {
                !self.suggestion_charmed_by(actor, target)
                    && self.observed_footprint(actor, target).is_some_and(|body| {
                        body.squares()
                            .any(|point| self.dispel_reaches(origin, point))
                    })
            })
            .fold(0, |mask, id| mask | (1 << id));
        for id in FIRST_OBJECT..END_OBJECTS {
            if self
                .dispel_object_point(actor, id)
                .is_some_and(|point| self.dispel_reaches(origin, point))
            {
                choices |= 1 << id;
            }
        }
        for source in 0..2 {
            let focus = !matches!(
                self.f[source].build.class,
                Class::Barbarian | Class::Fighter | Class::Monk | Class::Rogue
            );
            if focus
                && self.can_see(actor, source)
                && self.dispel_reaches(origin, self.fighter_point(source))
            {
                choices |= 1 << (CARRIED_FOCUS + source);
            }
            for base in [AREA_EFFECT, ZONE_OF_TRUTH, ROPE_TRICK] {
                if self
                    .dispel_field(base + source)
                    .is_some_and(|(_, point)| self.dispel_reaches(origin, point))
                {
                    choices |= 1 << (base + source);
                }
            }
        }
        choices
    }

    fn dispel_reaches(&self, from: Point, to: Point) -> bool {
        from.dist(to) <= RANGE && self.map.los_at(from, to)
    }

    fn dispel_object_point(&self, actor: usize, id: usize) -> Option<Point> {
        if let Some(object) = self.observed_object(actor, id) {
            return Some(object.point);
        }
        if id < 8 {
            let holder = (0..2).find(|&holder| {
                self.can_see(actor, holder) && self.gear_wielded_by(holder, id % 2, id >= 6)
            })?;
            Some(self.fighter_point(holder))
        } else if (10..12).contains(&id) {
            let wearer = self.armor_holder(id - 10)?;
            (self.armor_worn(wearer) && self.can_see(actor, wearer))
                .then(|| self.fighter_point(wearer))
        } else if (12..14).contains(&id) {
            let issuer = id - 12;
            let holder = self.shield_holder(issuer)?;
            let drawn = matches!(
                self.f[issuer].gear.shield,
                Carried::Held | Carried::ForeignHeld
            );
            (drawn && self.can_see(actor, holder)).then(|| self.fighter_point(holder))
        } else {
            None
        }
    }

    fn dispel_field(&self, id: usize) -> Option<(&'static str, Point)> {
        if (ROPE_TRICK..ROPE_TRICK + 2).contains(&id) {
            return self.rope_portal_point(id - ROPE_TRICK).map(|point| ("Rope Trick", point));
        }
        if (ZONE_OF_TRUTH..ZONE_OF_TRUTH + 2).contains(&id) {
            let (point, expires) = self.f[id - ZONE_OF_TRUTH].magic.zone_truth?;
            return (expires > self.turns).then_some(("Zone of Truth", point));
        }
        let source = id.checked_sub(AREA_EFFECT)?;
        let f = self.f.get(source)?;
        let name = match f.concentration? {
            Concentration::Moonbeam => "Moonbeam",
            Concentration::SpiritualWeapon => "Spiritual Weapon",
            Concentration::SpiritGuardians => {
                return Some(("Spirit Guardians", self.fighter_point(source)))
            }
            Concentration::HungerOfHadar => "Hunger of Hadar",
            Concentration::StinkingCloud => "Stinking Cloud",
            Concentration::FogCloud => "Fog Cloud",
            Concentration::Entangle => "Entangle",
            _ => return None,
        };
        Some((name, f.magic.field?))
    }

    pub(super) fn dispel_target_label(&self, actor: usize, id: usize) -> Option<String> {
        let label: String = match id {
            id if id == actor => "yourself".into(),
            0 | 1 => "opposing adventurer".into(),
            2 | 3 => self.observed_entity(actor, id)?.kind.name().into(),
            id if (CARRIED_FOCUS..CARRIED_FOCUS + 2).contains(&id) => {
                if id - CARRIED_FOCUS == actor {
                    "your carried spellcasting focus".into()
                } else {
                    "opposing carried spellcasting focus".into()
                }
            }
            id if id >= AREA_EFFECT => self.dispel_field(id)?.0.into(),
            id => {
                if let Some(object) = self.observed_object(actor, id) {
                    object.label().into()
                } else if id < 8 {
                    self.gear_weapon_kind_for(id % 2, id >= 6)?.name().into()
                } else if (10..12).contains(&id) {
                    "worn armor".into()
                } else if (12..14).contains(&id) {
                    "equipped shield".into()
                } else {
                    return None;
                }
            }
        };
        Some(format!("Dispel magic on {label}"))
    }

    pub(in crate::arena2) fn dispel_succeeds(&mut self, actor: usize, spell_rank: usize, tier: usize) -> bool {
        spell_rank <= tier || self.d20_for(actor, false, false) + self.f[actor].build.casting >= 10 + spell_rank as i32
    }

    pub(super) fn resolve_dispel_target(&mut self, actor: usize, target: usize, tier: usize) {
        if target < 4 {
            self.dispel_creature(actor, target, tier);
        } else if self.dispel_field(target).is_some() {
            if target >= ROPE_TRICK {
                let source = target - ROPE_TRICK;
                if self.rope_spell_rank(source).is_some_and(|rank| self.dispel_succeeds(actor, rank, tier)) {
                    self.end_rope_trick(source);
                }
            } else if target >= ZONE_OF_TRUTH {
                self.f[target - ZONE_OF_TRUTH].magic.zone_truth = None;
            } else {
                let source = target - AREA_EFFECT;
                if self.dispel_succeeds(actor, self.f[source].magic.field_level, tier) {
                    self.f[source].concentration = None;
                }
            }
        } else {
            for source in 0..2 {
                let f = &mut self.f[source];
                let light_object = match f.magic.light_target {
                    0 => CARRIED_FOCUS + source,
                    1 => 12 + source,
                    id => id,
                };
                if light_object == target {
                    f.magic.light_until = 0;
                }
                if f.magic
                    .shillelagh_weapon
                    .is_some_and(|owner| FIRST_OBJECT + owner == target)
                {
                    f.magic.shillelagh_until = 0;
                    f.magic.shillelagh_weapon = None;
                }
                if let Some((owner, ranged)) = f.magic.spell_weapon {
                    if (if ranged { 6 } else { 4 }) + owner == target {
                        f.magic.spell_weapon_until = 0;
                        f.magic.spell_weapon = None;
                    }
                }
            }
        }
    }

    fn dispel_creature(&mut self, actor: usize, target: usize, tier: usize) {
        self.dispel_bard_ranger_spells(actor, target, tier);
        for source in 0..2 {
            if self.f[source].magic.nondetection_until[target] > self.turns
                && self.dispel_succeeds(actor, 3, tier)
            {
                self.f[source].magic.nondetection_until[target] = 0;
            }
        }
        for source in 0..2 {
            let Some(effect) = self.f[source].concentration else {
                continue;
            };
            let m = &self.f[source].magic;
            if effect == Concentration::FaerieFire && self.faerie_outlined(target) {
                if self.dispel_succeeds(actor, m.field_level, tier) {
                    self.dispel_faerie_mark(source, target);
                }
                continue;
            }
            if effect == Concentration::CalmEmotions && self.arcane_calm_affects(source, target) {
                if self.dispel_succeeds(actor, m.field_level, tier) {
                    self.arcane_end_calm_target(source, target);
                }
                continue;
            }
            if self.dispellable_area_affliction(source, target, effect) {
                if self.dispel_succeeds(actor, m.field_level, tier) {
                    self.dispel_area_affliction(source, target, effect);
                }
                continue;
            }
            let group = match effect {
                Concentration::Bane => m.bane_recipients,
                Concentration::Bless
                | Concentration::ShieldOfFaith
                | Concentration::Protection
                | Concentration::Fly
                | Concentration::Invisibility
                | Concentration::EnhanceAbility
                | Concentration::Guidance
                | Concentration::Resistance
                | Concentration::Heroism
                | Concentration::ProtectionFromEnergy => m.ally_recipients,
                _ => 0,
            };
            let applies = if group != 0 {
                group & (1 << target) != 0
            } else {
                match effect {
                    Concentration::Hex | Concentration::HunterMark => m.mark_target == Some(target),
                    Concentration::Suggestion => self.suggestion_charmed_by(target, source),
                    Concentration::HoldPerson => {
                        target == 1 - source && self.magic_paralyzed(target)
                    }
                    Concentration::DetectThoughts
                    | Concentration::Blur
                    | Concentration::SpiritGuardians
                    | Concentration::AlterSelf
                    | Concentration::ExpeditiousRetreat
                    | Concentration::PassWithoutTrace
                    | Concentration::CrusadersMantle
                    | Concentration::GustOfWind
                    | Concentration::Heroism
                    | Concentration::Fly
                    | Concentration::Guidance
                    | Concentration::Resistance
                    | Concentration::ProtectionFromEnergy => target == source,
                    _ => false,
                }
            };
            if !applies || !self.dispel_succeeds(actor, m.field_level, tier) {
                continue;
            }
            let remaining = group & !(1 << target);
            if remaining == 0 {
                self.f[source].concentration = None;
            } else if effect == Concentration::Bane {
                self.f[source].magic.bane_recipients = remaining;
            } else {
                self.f[source].magic.ally_recipients = remaining;
            }
        }

        // The fixed level 1–5 loadouts have no slot higher than level 3.
        if target < 2 {
            let f = &mut self.f[target];
            f.magic.longstrider_until = 0;
            f.magic.jump_until = 0;
            f.magic.favor_until = 0;
            f.magic.roster_fields.blink_until = 0;
            f.magic.mage_armor_until = 0;
            f.arcane.see_invisibility_until = 0;
            f.magic.sanctuary_until = 0;
            f.magic.sanctuary_source = None;
            f.hp = (f.hp - f.magic.aid_hp).max((f.hp > 0) as i32);
            f.magic.aid_hp = 0;
        } else if let Some(entity) = self.entity_mut(target) {
            entity.longstrider_until = 0;
            entity.jump_until = 0;
            entity.mage_armor_until = 0;
            entity.sanctuary = None;
            entity.hp = (entity.hp - entity.aid_bonus).max(1);
            entity.aid_bonus = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::entities::{Entity, EntityKind, FamiliarForm, Spirit};
    use crate::arena2::geometry::Pos;
    use crate::arena2::{Act, ACTS};

    fn encounter() -> Battle {
        let mut battle = Battle::new(7, [Class::Bard, Class::Druid], [5; 2]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
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
        battle
    }

    #[test]
    fn dispel_companion_removes_its_boons_while_group_spell_remains_on_other_target() {
        let mut battle = encounter();
        let now = battle.turns;
        let entity = battle.entity_mut(3).unwrap();
        entity.aid_bonus = 5;
        entity.hp += 5;
        entity.mage_armor_until = now + 21;
        entity.longstrider_until = now + 21;
        entity.sanctuary = Some((1, now + 21));
        battle.concentrate(1, Concentration::Fly, 100, None, 3);
        battle.f[1].magic.ally_recipients = 0b1010;
        assert_ne!(battle.dispel_targets(0) & (1 << 3), 0);
        battle.step(Act::Magic5).unwrap();
        assert_eq!(battle.phase_id(), 46);
        battle.step(ACTS[3]).unwrap();
        let entity = battle.entity(3).unwrap();
        assert_eq!(entity.aid_bonus, 0);
        assert_eq!(entity.hp, entity.kind.stats().hp);
        assert_eq!(entity.mage_armor_until, 0);
        assert_eq!(entity.longstrider_until, 0);
        assert!(entity.sanctuary.is_none());
        assert_eq!(battle.f[1].concentration, Some(Concentration::Fly));
        assert_eq!(battle.f[1].magic.ally_recipients, 0b0010);
    }

    #[test]
    fn single_named_magic_effect_and_its_caster_are_distinct_targets() {
        let mut battle = encounter();
        let moonbeam = battle.fighter_point(1);
        battle.concentrate(1, Concentration::Moonbeam, 10, Some(moonbeam), 2);
        assert_ne!(battle.dispel_targets(0) & (1 << (AREA_EFFECT + 1)), 0);
        battle.resolve_dispel_target(0, 1, 3);
        assert_eq!(battle.f[1].concentration, Some(Concentration::Moonbeam));
        battle.resolve_dispel_target(0, AREA_EFFECT + 1, 3);
        assert_eq!(battle.f[1].concentration, None);
    }

    #[test]
    fn dispel_weapon_removes_each_spell_on_the_item_not_on_its_wielder() {
        let mut battle = encounter();
        let id = FIRST_OBJECT + 1;
        assert_ne!(battle.dispel_targets(0) & (1 << id), 0);
        let now = battle.turns;
        let magic = &mut battle.f[1].magic;
        magic.spell_weapon = Some((1, false));
        magic.spell_weapon_until = now + 10;
        magic.shillelagh_weapon = Some(1);
        magic.shillelagh_until = now + 10;
        battle.resolve_dispel_target(0, 1, 3);
        assert_eq!(battle.f[1].magic.spell_weapon, Some((1, false)));
        assert_eq!(battle.f[1].magic.shillelagh_weapon, Some(1));
        battle.resolve_dispel_target(0, id, 3);
        assert!(battle.f[1].magic.spell_weapon.is_none());
        assert!(battle.f[1].magic.shillelagh_weapon.is_none());
    }

    #[test]
    fn dispel_object_douses_light_without_damaging_the_object() {
        let mut battle = encounter();
        battle.f[1].magic.light_target = 0;
        battle.f[1].magic.light_until = battle.turns + 10;
        assert!(battle.magic_visible_light(0, 1).is_some());
        assert_ne!(battle.dispel_targets(0) & (1 << (CARRIED_FOCUS + 1)), 0);
        battle.resolve_dispel_target(0, 1, 3);
        assert!(battle.magic_visible_light(0, 1).is_some());
        battle.resolve_dispel_target(0, CARRIED_FOCUS + 1, 3);
        assert!(battle.magic_visible_light(0, 1).is_none());
    }
}
