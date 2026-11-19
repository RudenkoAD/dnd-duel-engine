//! Exercise real decision boundaries with player-controlled choices for both sides.
use super::{battle::Battle, class::CLASSES, Act, ACTS};
use crate::dice::Dice;

fn play_legal_choices(seed: u64, classes: [super::class::Class; 2], levels: [i32; 2]) {
    let mut battle = Battle::new(seed, classes, levels);
    let mut rng = Dice(seed ^ 0x6eedb0a4);
    let mut history = Vec::new();
    while let Some(actor) = battle.actor() {
        let phase = battle.phase_id();
        let mask = battle.mask();
        assert!(mask!=0,"empty menu seed={seed} classes={classes:?} levels={levels:?} phase={phase} history={history:?}");
        let choice_index = (rng.roll(u64::from(mask.count_ones())) - 1) as usize;
        let choice = ACTS
            .iter()
            .filter(|act| mask & act.bit() != 0)
            .nth(choice_index)
            .copied()
            .unwrap();
        assert!(
            !battle.choice_label(actor, choice).is_empty(),
            "missing label for {choice:?} in phase {phase}"
        );
        history.push((actor, phase, choice as u8));
        let applied =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| battle.step(choice)));
        assert!(matches!(applied,Ok(Ok(()))),"illegal transition seed={seed} levels={levels:?} classes={classes:?} history={history:?}");
        assert!(history.len() <= 800);
    }
    assert_eq!(battle.mask(), 0);
    assert!(battle.finished());
    assert!(battle.step(Act::End).is_err());
}

#[test]
fn legal_choices_always_advance_in_level_and_class_cross_product() {
    let repetitions = std::env::var("DUEL_AUDIT_REPETITIONS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);
    let first_rep = std::env::var("DUEL_AUDIT_START")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    let mut failed = Vec::new();
    for rep in first_rep..first_rep + repetitions {
        for level in 1..=5 {
            for first in 0..CLASSES.len() {
                for reverse in [false, true] {
                    let k =
                        rep * 120 + (level as usize - 1) * 24 + first * 2 + usize::from(reverse);
                    let seed = 0x5345510000000000 + k as u64;
                    let second = (first * 7 + level as usize + rep) % 12;
                    let mut classes = [CLASSES[first], CLASSES[second]];
                    let mut levels = [level, 1 + ((level + rep as i32 + first as i32) % 5)];
                    if reverse {
                        classes.reverse();
                        levels.reverse();
                    }
                    if std::panic::catch_unwind(|| play_legal_choices(seed, classes, levels))
                        .is_err()
                    {
                        failed.push((seed, classes, levels));
                    }
                }
            }
        }
    }
    assert!(failed.is_empty(), "invalid bouts: {failed:?}");
}

#[test]
fn blur_checks_the_actual_defender_and_attackers_blindsight() {
    use super::{
        class::Class,
        entities::{Entity, EntityKind, FamiliarForm, Spirit},
        geometry::Pos,
        magic::Concentration,
    };
    let mut b = Battle::new(0, [Class::Druid, Class::Warlock], [3, 3]);
    b.owner = 0;
    b.f[0].pos = Pos { x: 0, y: 0 };
    b.f[1].pos = Pos { x: 1, y: 0 };
    b.step(Act::Magic5).unwrap();
    assert_eq!(b.f[0].concentration, Some(Concentration::Blur));
    assert!(b.blur_disadvantage(1, 0));
    assert!(!b.blur_disadvantage(0, 1));
    b.fixture_companion(
        1,
        Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Bat,
                spirit: Spirit::Fey,
            },
            1,
            Pos { x: 1, y: 1 },
            0,
        ),
    );
    assert!(!b.blur_disadvantage(3, 0));
    assert!(!b.blur_disadvantage(0, 3));
}

#[test]
fn sacred_flame_uses_the_targets_dexterity_save_and_ignores_partial_cover() {
    use super::{
        class::Class,
        geometry::{Map, Pos},
    };
    let mut failed = false;
    let mut saved = false;
    for seed in 0..64 {
        let mut open = Battle::new(seed, [Class::Cleric, Class::Fighter], [5, 5]);
        open.map = Map::new(0);
        open.owner = 0;
        open.f[0].pos = Pos { x: 0, y: 0 };
        open.f[1].pos = Pos { x: 3, y: 0 };
        open.step(Act::Gear2).unwrap(); // V,S requires a free hand; the shield emblem cannot substitute.
        let mut covered = open.clone();
        covered.f[1].pos = Pos { x: 3, y: 2 };
        let cover = covered
            .map
            .cover_from_point(covered.fighter_point(0), covered.fighter_point(1));
        assert_eq!(cover.bonus(), 5);
        let old = open.f[1].hp;
        for b in [&mut open, &mut covered] {
            b.step(Act::Spell0).unwrap();
            b.step(ACTS[1]).unwrap();
        }
        let damage = old - open.f[1].hp;
        assert_eq!(open.f[1].hp, covered.f[1].hp);
        assert!((0..=16).contains(&damage));
        failed |= damage > 0;
        saved |= damage == 0;
    }
    assert!(failed && saved);
}
