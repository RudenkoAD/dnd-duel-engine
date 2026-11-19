use super::*;
use crate::arena2::{
    entities::{Entity, EntityKind, Spirit},
    equipment::Carried,
    geometry::Pos,
};

fn encounter(seed: u64, caster: Class, foe: Class, level: i32) -> Battle {
    let mut battle = Battle::new(seed, [caster, foe], [level, 5]);
    battle.owner = 0;
    battle.f[0].pos = Pos { x: 0, y: 0 };
    battle.f[1].pos = Pos { x: 1, y: 0 };
    battle
}

fn companion(battle: &mut Battle) {
    battle.fixture_companion(
        1,
        Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Fey,
                spell_level: 2,
            },
            1,
            Pos { x: 0, y: 1 },
            10,
        ),
    );
}

#[test]
fn hex_affects_only_the_selected_ability_and_needs_a_later_turn_to_transfer() {
    let mut battle = encounter(3, Class::Warlock, Class::Paladin, 3);
    companion(&mut battle);
    battle.step(Act::Spell1).unwrap();
    for _ in 0..4 {
        if battle.magic_mark_target(0).is_some() {
            break;
        }
        let id = if battle.phase_id() == 41 {
            0
        } else if battle.mask() & ACTS[3].bit() != 0 {
            3
        } else {
            battle.mask().trailing_zeros() as usize
        };
        battle.step(ACTS[id]).unwrap();
    }
    assert_eq!(battle.magic_mark_target(0), Some(3));
    assert!(battle.magic_hex_disadvantage(3, 0));
    assert!(!battle.magic_hex_disadvantage(3, 1));
    assert!(!battle.magic_hex_disadvantage(1, 0));
    battle.f[0].hidden_dc = Some(18);
    assert!(battle.magic_hex_disadvantage(3, 0));
    battle.bonus = true;
    let familiar = battle.entity(3).unwrap().clone();
    battle.entity_mut(3).unwrap().dismissed = true;
    assert_eq!(battle.mask() & Act::Magic24.bit(), 0);
    battle.fixture_companion(1, familiar);
    battle.entity_damage(3, 100);
    assert_eq!(battle.mask() & Act::Magic24.bit(), 0);
    battle.turns += 2;
    battle.step(Act::Magic24).unwrap();
    battle.step(ACTS[1]).unwrap();
    assert!(battle.magic_hex_disadvantage(1, 0));
    assert!(!battle.magic_hex_disadvantage(3, 0));
    battle.turns += 2;
    battle.bonus = true;
    assert!(battle.mask() & Act::Spell1.bit() != 0);
    battle.step(Act::DropConcentration).unwrap();
    assert!(!battle.magic_hex_disadvantage(1, 0));
}

#[test]
fn shillelagh_switches_attack_ability_and_type_without_recasting_and_ends_on_release() {
    let mut battle = encounter(3, Class::Druid, Class::Fighter, 5);
    battle.step(Act::Special4).unwrap();
    battle.step(ACTS[1]).unwrap();
    assert_eq!(
        battle.magic_weapon(0, false),
        Some((10, battle.f[0].build.casting))
    );
    assert_eq!(battle.magic_attack_kind(0, true), Some(DamageType::Force));
    battle.step(Act::Magic24).unwrap();
    battle.step(ACTS[2]).unwrap();
    assert_eq!(
        battle.magic_weapon(0, false),
        Some((10, battle.f[0].build.stats[0]))
    );
    assert_ne!(battle.magic_attack_kind(0, true), Some(DamageType::Force));
    battle.gear_drop_weapon_instance(0, 0, false, battle.f[0].pos);
    assert_eq!(battle.magic_weapon(0, false), None);
}

#[test]
fn divine_favor_only_applies_to_weapons_and_coexists_with_concentration() {
    let mut battle = encounter(3, Class::Paladin, Class::Fighter, 2);
    battle.f[0].gear.melee = Carried::Packed;
    battle.f[0].concentration = Some(Concentration::Bless);
    battle.step(Act::Spell1).unwrap();
    assert_eq!(battle.f[0].concentration, Some(Concentration::Bless));
    assert_eq!(battle.magic_divine_favor_damage(0, false, false), 0);
    assert!((1..=4).contains(&battle.magic_divine_favor_damage(0, false, true)));
    assert!((2..=8).contains(&battle.magic_divine_favor_damage(0, true, true)));
    battle.turns += 20;
    assert_eq!(battle.magic_divine_favor_damage(0, false, true), 0);
}

#[test]
fn magic_weapon_targets_a_ground_item_and_preserves_the_enchantment_when_picked_up() {
    let mut battle = encounter(3, Class::Paladin, Class::Fighter, 5);
    battle.f[0].gear.melee = Carried::Packed;
    battle.gear_drop_weapon_instance(1, 1, false, battle.f[1].pos);
    battle.step(Act::Magic10).unwrap();
    battle.step(ACTS[2]).unwrap();
    assert!(battle.magic_enchanted_weapon(1, false));
    assert!(!battle.magic_enchanted_weapon(0, false));
    battle.f[1].gear.melee = Carried::Held;
    assert_eq!(battle.magic_weapon_bonus(1, false), 1);
    assert_eq!(battle.magic_weapon_bonus(0, false), 0);
    battle.turns += 2;
    battle.f[0].gear.shield = Carried::Packed;
    battle.f[0].gear.melee = Carried::Held;
    battle.bonus = true;
    battle.step(Act::Magic10).unwrap();
    assert_eq!(battle.mask() & ACTS[2].bit(), 0);
    battle.step(ACTS[0]).unwrap();
    assert_eq!(battle.magic_weapon_bonus(1, false), 0);
    assert_eq!(battle.magic_weapon_bonus(0, false), 1);
    battle.turns += 1200;
    assert_eq!(battle.magic_weapon_bonus(0, false), 0);
}

#[test]
fn ensnaring_strike_can_bind_a_companion_and_be_ended_by_its_nearby_ally() {
    let checked = (0..128).any(|seed| {
        let mut battle = encounter(seed, Class::Ranger, Class::Paladin, 4);
        companion(&mut battle);
        battle.step(Act::Melee).unwrap();
        battle.step(ACTS[3]).unwrap();
        if battle.phase_id() != 3 {
            return false;
        }
        battle.step(Act::Rider).unwrap();
        if !battle.magic_area_restrained(3) {
            return false;
        }
        assert_eq!(battle.f[0].slots[0], 2);
        let mut dropped = battle.clone();
        dropped.step(Act::DropConcentration).unwrap();
        assert!(!dropped.magic_area_restrained(3));
        let hp = battle.entity(3).unwrap().hp;
        battle.magic_area_start(3);
        battle.area_request_damage();
        if battle.entity(3).is_none() {
            return false;
        }
        assert!(battle.entity(3).unwrap().hp < hp);
        // The Paladin is adjacent to the steed and can use its action to help.
        battle.owner = 1;
        battle.action = true;
        assert!(battle.mask() & Act::Entity11.bit() != 0);
        battle.step(Act::Entity11).unwrap();
        if battle.magic_area_restrained(3) {
            return false;
        }
        assert!(battle.f[0].concentration.is_none());
        assert!(!battle.action);
        true
    });
    assert!(checked);
}

#[test]
fn hunters_mark_improves_perception_only_when_searching_for_its_quarry() {
    use crate::arena2::common::Check;

    let improved = (0..128).any(|seed| {
        let mut marked = encounter(seed, Class::Ranger, Class::Barbarian, 3);
        marked.f[1].hidden_dc = Some(17);
        marked.f[0].concentration = Some(Concentration::HunterMark);
        marked.f[0].magic.mark_target = Some(1);
        let mut unmarked = marked.clone();
        unmarked.f[0].magic.mark_target = Some(0);
        marked.try_check_as(0, Check::Search, false);
        unmarked.try_check_as(0, Check::Search, false);
        let marked_found = marked.f[1].hidden_dc.is_none();
        let unmarked_found = unmarked.f[1].hidden_dc.is_none();
        assert!(!unmarked_found || marked_found);
        marked_found && !unmarked_found
    });
    assert!(improved);
}
