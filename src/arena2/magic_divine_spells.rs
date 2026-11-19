//! Always-prepared spells added by the alternate divine subclasses.
use super::{Act, Spell};
use crate::arena2::class::{Build, Class};

pub(super) fn prepared(build: Build, action: Act) -> Option<Option<Spell>> {
    use Act::*;
    use Class::*;
    use Spell::*;

    if build.variant == 0 {
        return None;
    }
    Some(match (build.class, build.variant, action) {
        (Cleric, _, Magic3 | Magic4)
        | (Cleric, 2, Magic1 | Magic2 | Magic9 | Magic16)
        | (Druid, _, Spell4 | Spell5 | Magic5 | Magic6)
        | (Paladin, _, Magic17)
        | (Paladin, 2, Magic15) => None,
        (Paladin, 1, Magic1) => Some(EnsnaringStrike),
        (Paladin, 1, Magic15) => Some(MistyStep),
        (Paladin, 1, Magic16) => Some(Moonbeam),
        (Cleric, 1, Magic12) => Some(MagicWeapon),
        (Cleric, 1, Magic13) => Some(CrusadersMantle),
        (Cleric, 2, Magic12) => Some(CharmPerson),
        (Cleric, 2, Magic13) => Some(Invisibility),
        (Cleric, 2, Magic14) => Some(PassWithoutTrace),
        (Cleric, 2, Magic15) => Some(HypnoticPattern),
        (Cleric, 2, Magic17) => Some(DisguiseSelf),
        (Cleric, 2, Magic18) => Some(Nondetection),
        (Druid, 1, Magic9) => Some(Guidance),
        (Druid, 1, Magic10) => Some(GuidingBolt),
        (Druid, 2, Magic9) => Some(RayOfFrost),
        (Druid, 2, Magic10) => Some(Shatter),
        (Druid, 2, Magic11) => Some(FogCloud),
        (Druid, 2, Magic12) => Some(GustOfWind),
        (Druid, 2, Magic13) => Some(LightningBolt),
        (Druid, 2, Magic14) => Some(Jump),
        (Paladin, 2, Magic11) => Some(GuidingBolt),
        (Paladin, 2, Magic12) => Some(EnhanceAbility),
        _ => return None,
    })
}

pub(super) fn unavailable(build: Build) -> &'static [(&'static str, usize, usize, &'static str)] {
    match (build.class, build.variant) {
        (Class::Druid, 2) => &[("Water Breathing", 3, 5, "no underwater terrain")],
        (Class::Paladin, 1) => &[("Speak with Animals", 1, 3, "no conversation with beasts")],
        _ => &[],
    }
}

pub(super) fn unlock(build: Build, spell: Spell) -> Option<i32> {
    use Class::*;
    use Spell::*;

    Some(match (build.class, build.variant, spell) {
        (Cleric, 1, GuidingBolt | MagicWeapon | ShieldOfFaith | SpiritualWeapon)
        | (Cleric, 2, CharmPerson | DisguiseSelf | Invisibility | PassWithoutTrace)
        | (Druid, 1, Guidance | GuidingBolt)
        | (Druid, 2, RayOfFrost | Shatter | FogCloud | GustOfWind | Thunderwave)
        | (Paladin, 1, EnsnaringStrike)
        | (Paladin, 2, GuidingBolt | Heroism) => 3,
        (Cleric, 1, CrusadersMantle | SpiritGuardians)
        | (Cleric, 2, HypnoticPattern | Nondetection)
        | (Druid, 2, LightningBolt)
        | (Paladin, 1, MistyStep | Moonbeam)
        | (Paladin, 2, EnhanceAbility | MagicWeapon) => 5,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{battle::Battle, geometry::Pos, magic::Concentration, ACTS};

    fn duel(class: Class, variant: u8) -> Battle {
        let mut battle =
            Battle::new_roster_in(0, [class as u8 * 3 + variant, Class::Fighter as u8 * 3], 0);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 6, y: 0 };
        battle
    }

    #[test]
    fn divine_spell_grants_have_correct_spell_ranks_and_subclass_gates() {
        use Act::*;
        use Class::*;
        use Spell::*;
        let cases = [
            (Cleric, 1, Magic12, MagicWeapon, 2),
            (Cleric, 1, Magic13, CrusadersMantle, 3),
            (Cleric, 2, Magic13, Invisibility, 2),
            (Cleric, 2, Magic14, PassWithoutTrace, 2),
            (Cleric, 2, Magic15, HypnoticPattern, 3),
            (Druid, 1, Magic9, Guidance, 0),
            (Druid, 1, Magic10, GuidingBolt, 1),
            (Druid, 2, Magic9, RayOfFrost, 0),
            (Druid, 2, Magic10, Shatter, 2),
            (Druid, 2, Magic11, FogCloud, 1),
            (Druid, 2, Magic12, GustOfWind, 2),
            (Druid, 2, Magic13, LightningBolt, 3),
            (Paladin, 1, Magic1, EnsnaringStrike, 1),
            (Paladin, 1, Magic15, MistyStep, 2),
            (Paladin, 1, Magic16, Moonbeam, 2),
            (Paladin, 2, Magic12, EnhanceAbility, 2),
        ];
        for (class, variant, action, spell, rank) in cases {
            let battle = duel(class, variant);
            assert_eq!(battle.prepared_spell(0, action), Some(spell));
            assert_eq!(spell.level(), rank, "{spell:?}");
            assert_ne!(duel(class, 0).prepared_spell(0, action), Some(spell));
        }
        let land = duel(Druid, 0);
        assert_eq!(land.prepared_spell(0, Spell4), Some(Fireball));
        assert_eq!(duel(Druid, 2).prepared_spell(0, Spell4), None);
    }

    #[test]
    fn sea_lightning_bolt_resolves_through_the_direction_menu() {
        let mut battle = duel(Class::Druid, 2);
        // A druidic staff is a material focus while the druid carries a shield.
        let hp = battle.f[1].hp;
        assert_ne!(battle.mask() & Act::Magic13.bit(), 0);
        battle.step(Act::Magic13).unwrap();
        battle.step(Act::E).unwrap();
        assert_eq!(battle.f[0].slots[2], 1);
        assert!(battle.f[1].hp < hp);
    }

    #[test]
    fn fog_cloud_blocks_sight_without_acquiring_stinking_cloud_effects() {
        let mut battle = duel(Class::Druid, 2);
        assert!(battle.can_see(0, 1));
        battle.concentrate(
            0,
            Concentration::FogCloud,
            600,
            Some(crate::arena2::geometry::Point::ground(Pos { x: 3, y: 0 })),
            1,
        );
        assert!(!battle.can_see(0, 1));
        assert!(!battle.magic_area_poisoned(1));
        assert_eq!(battle.fog_radius(0), 4);
    }

    #[test]
    fn trickery_invisibility_conceals_until_the_recipient_attacks() {
        let mut battle = duel(Class::Cleric, 2);
        battle.f[1].pos = Pos { x: 1, y: 0 };
        assert_ne!(battle.mask() & Act::Magic13.bit(), 0);
        battle.step(Act::Magic13).unwrap();
        if battle.phase_id() == 38 {
            battle.step(ACTS[1]).unwrap();
        }
        if battle.phase_id() == 48 {
            battle.step(ACTS[0]).unwrap(); // Public recipient zero is the caster.
        }
        assert_eq!(
            battle.f[0].concentration,
            Some(Concentration::Invisibility),
            "phase {}, mask {:x}",
            battle.phase_id(),
            battle.mask()
        );
        assert!(!battle.can_see(1, 0));
        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap();
        battle.step(Act::Melee).unwrap();
        if battle.phase_id() == 14 {
            battle.step(ACTS[1]).unwrap();
        }
        assert!(battle.can_see(1, 0));
    }
    #[test]
    fn nondetection_consumes_dust_and_can_be_dispelled() {
        let mut battle = duel(Class::Cleric, 2);
        assert_eq!(battle.mask() & Act::Magic18.bit(), 0);
        battle.step(Act::Gear2).unwrap(); // Stow the mace to handle diamond dust.
        assert_ne!(battle.mask() & Act::Magic18.bit(), 0);
        battle.step(Act::Magic18).unwrap();
        assert!(battle.magic_nondetection(0));
        assert_eq!(battle.f[0].slots[2], 1);
        assert!(!battle.magic_components(0, Spell::Nondetection));
        battle.resolve_dispel_target(1, 0, 3);
        assert!(!battle.magic_nondetection(0));
    }
}
