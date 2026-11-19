//! Independent cross-domain checks used by the coverage catalog.
use super::battle::Battle;
use super::class::{build, Class};
use super::equipment::DamageType;
use super::geometry::Pos;
use super::Act;

#[test]
fn action_bonus_action_movement_and_reaction_refresh_independently() {
    let mut duel = Battle::new(0, [Class::Fighter; 2], [1; 2]);
    let owner = duel.actor().unwrap();
    let other = 1 - owner;
    duel.f[owner].reaction = false;
    duel.f[other].reaction = false;
    let movement = duel.movement_left();
    assert!(duel.action && duel.bonus);

    duel.step(Act::Dodge).unwrap();
    assert!(!duel.action && duel.bonus);
    assert_eq!(duel.movement_left(), movement);
    assert_eq!(
        duel.mask() & (Act::Dodge.bit() | Act::Dash.bit() | Act::Disengage.bit()),
        0
    );

    duel.step(Act::Other1).unwrap(); // Stonecunning uses a bonus action.
    assert!(!duel.action && !duel.bonus);
    assert_eq!(duel.movement_left(), movement);
    assert_eq!(duel.mask() & Act::Other1.bit(), 0);

    duel.step(Act::End).unwrap();
    assert_eq!(duel.actor(), Some(other));
    assert!(duel.action && duel.bonus);
    assert!(duel.f[other].reaction);
    assert!(!duel.f[owner].reaction);

    duel.step(Act::End).unwrap();
    assert_eq!(duel.actor(), Some(owner));
    assert!(duel.action && duel.bonus && duel.f[owner].reaction);
}

#[test]
fn defense_archery_and_unarmored_class_values_have_exact_level_boundaries() {
    assert_eq!(build(Class::Fighter, 1).ac, 19);
    assert_eq!(build(Class::Paladin, 1).ac, 18);
    assert_eq!(build(Class::Paladin, 2).ac, 19);

    for level in 1..=5 {
        let duel = Battle::new(0, [Class::Ranger, Class::Fighter], [level; 2]);
        assert_eq!(duel.martial_attack_bonus(0, false), 0);
        assert_eq!(
            duel.martial_attack_bonus(0, true),
            if level == 1 { 0 } else { 2 }
        );
        let barbarian = build(Class::Barbarian, level);
        let monk = build(Class::Monk, level);
        assert_eq!(barbarian.ac, 10 + barbarian.stats[1] + barbarian.stats[2]);
        assert_eq!(monk.ac, 10 + monk.stats[1] + monk.stats[4]);
        let sorcerer = build(Class::Sorcerer, level);
        assert_eq!(
            sorcerer.ac,
            10 + sorcerer.stats[1] + if level >= 3 { sorcerer.stats[5] } else { 0 }
        );
        assert_eq!(barbarian.speed, if level == 5 { 8 } else { 6 });
        assert_eq!(monk.speed, if level >= 2 { 8 } else { 6 });
    }
}

#[test]
fn extra_attack_applies_to_each_selected_build_only_from_level_five() {
    use Class::*;
    let eligible = [Barbarian, Fighter, Monk, Paladin, Ranger];
    for class in super::class::CLASSES {
        for level in 1..=5 {
            let duel = Battle::new(0, [class, Fighter], [level, 1]);
            assert_eq!(
                duel.extra_attack(0),
                1 + i32::from(level == 5 && eligible.contains(&class))
            );
        }
    }
}

#[test]
fn temporary_hp_absorbs_typed_damage_without_increasing_current_or_max_hp() {
    let mut duel = Battle::new(0, [Class::Fighter; 2], [1; 2]);
    let current = duel.f[0].hp;
    let maximum = duel.f[0].build.hp;
    duel.f[0].temp = 3;
    duel.damage_typed(0, 5, DamageType::Fire);
    assert_eq!(duel.f[0].temp, 0);
    assert_eq!(duel.f[0].hp, current - 2);
    assert_eq!(duel.f[0].build.hp, maximum);
}

#[test]
fn ordinary_unarmed_attack_remains_available_with_held_weapon_and_shield() {
    let mut duel = Battle::new(0, [Class::Fighter; 2], [1; 2]);
    let owner = duel.actor().unwrap();
    duel.f[owner].pos = Pos { x: 0, y: 0 };
    duel.f[1 - owner].pos = Pos { x: 1, y: 0 };
    assert_ne!(duel.mask() & Act::Unarmed.bit(), 0);
    duel.step(Act::Unarmed).unwrap();
    assert!(!duel.action);
}

fn inspired_opponent(seed: u64) -> Battle {
    let mut duel = Battle::new(seed, [Class::Bard, Class::Fighter], [1; 2]);
    duel.owner = 0;
    duel.f[0].pos = Pos { x: 0, y: 0 };
    duel.f[1].pos = Pos { x: 1, y: 0 };
    duel.step(Act::Magic18).unwrap();
    duel.step(super::ACTS[1]).unwrap();
    duel.step(Act::End).unwrap();
    assert_eq!(duel.actor(), Some(1));
    assert!(duel.bardic_inspiration_available(1));
    duel
}

fn assert_optional_inspiration(mut duel: Battle, phase: i32) {
    assert_eq!(duel.phase_id(), phase);
    assert_eq!(duel.actor(), Some(1));
    assert_eq!(duel.mask(), Act::Pass.bit() | Act::Defend.bit());
    let mut decline = duel.clone();
    decline.step(Act::Pass).unwrap();
    assert!(decline.bardic_inspiration_available(1));
    duel.step(Act::Defend).unwrap();
    assert!(!duel.bardic_inspiration_available(1));
}

#[test]
fn bardic_inspiration_is_optional_on_a_failed_attack_roll() {
    let duel = (0..64)
        .filter_map(|seed| {
            let mut duel = inspired_opponent(seed);
            duel.step(Act::Unarmed).unwrap();
            (duel.phase_id() == 98).then_some(duel)
        })
        .next()
        .expect("an inspired attack misses without a natural one");
    assert_optional_inspiration(duel, 98);
}

#[test]
fn bardic_inspiration_is_optional_on_an_unsuccessful_skill_check() {
    let mut duel = inspired_opponent(1);
    // An assured failed check independently exercises the reaction decision.
    duel.f[1].grappled = Some(40);
    duel.step(Act::EscapeAcrobatics).unwrap();
    assert_optional_inspiration(duel, 99);
}

#[test]
fn dash_disengage_and_dodge_have_distinct_action_costs_and_effects() {
    let mut source = Battle::new(0, [Class::Fighter; 2], [1; 2]);
    source.owner = 0;
    source.f[0].pos = Pos { x: 1, y: 0 };
    source.f[1].pos = Pos { x: 0, y: 0 };
    let initial_movement = source.movement_left();

    let mut dash = source.clone();
    dash.step(Act::Dash).unwrap();
    assert_eq!(dash.movement_left(), 2 * initial_movement);
    assert!(!dash.action && dash.bonus);

    let mut exposed = source.clone();
    exposed.step(Act::E).unwrap();
    assert_eq!(exposed.actor(), Some(1));
    assert_ne!(exposed.phase_id(), 0);
    assert_eq!(
        exposed.f[0].pos.x, 1,
        "movement waits for the opportunity choice"
    );

    let mut disengaged = source.clone();
    disengaged.step(Act::Disengage).unwrap();
    assert!(!disengaged.action && disengaged.bonus);
    disengaged.step(Act::E).unwrap();
    assert_eq!(disengaged.actor(), Some(0));
    assert_eq!(disengaged.f[0].pos.x, 2);

    source.step(Act::Dodge).unwrap();
    assert!(source.f[0].dodge);
    assert!(!source.action && source.bonus);
    source.step(Act::End).unwrap();
    assert!(source.f[0].dodge);
    source.step(Act::End).unwrap();
    assert!(!source.f[0].dodge);
}

#[test]
fn always_prepared_spells_obey_arena_build_level_gates() {
    // Bless and Cure Wounds are chosen at earlier levels before their fixed grants.
    for (class, action, first_level) in [
        (Class::Cleric, Act::Spell5, 1),
        (Class::Cleric, Act::Special4, 1),
        (Class::Cleric, Act::Magic1, 3),
        (Class::Cleric, Act::Magic3, 5),
        (Class::Druid, Act::Magic5, 3),
        (Class::Druid, Act::Spell5, 3),
        (Class::Druid, Act::Spell4, 5),
        (Class::Paladin, Act::Magic15, 3),
        (Class::Paladin, Act::Magic16, 3),
        (Class::Paladin, Act::Magic1, 5),
        (Class::Ranger, Act::Spell0, 1),
        (Class::Sorcerer, Act::Magic7, 3),
        (Class::Sorcerer, Act::Magic8, 3),
        (Class::Sorcerer, Act::Magic9, 3),
        (Class::Sorcerer, Act::Magic11, 5),
        (Class::Sorcerer, Act::Magic12, 5),
        (Class::Warlock, Act::Spell2, 3),
        (Class::Warlock, Act::Magic8, 3),
        (Class::Warlock, Act::Magic13, 3),
        (Class::Warlock, Act::Spell4, 5),
        (Class::Warlock, Act::Magic14, 5),
    ] {
        for level in 1..=5 {
            let mut duel = Battle::new(12, [class, Class::Fighter], [level, 5]);
            duel.owner = 0;
            duel.f[0].pos = Pos { x: 0, y: 0 };
            duel.f[1].pos = Pos { x: 1, y: 0 };
            if duel.mask() & Act::Gear2.bit() != 0 {
                duel.step(Act::Gear2).unwrap();
            }
            assert_eq!(
                duel.mask() & action.bit() != 0,
                level >= first_level,
                "{class:?} level {level}: {action:?}"
            );
        }
    }
}
