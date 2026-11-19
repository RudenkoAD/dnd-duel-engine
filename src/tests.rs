use super::*;
use Action::*;

fn fixture(owner: usize, adjacent: bool, seed: u64) -> Duel {
    let mut d = Duel::new(seed);
    d.begin_turn(owner);
    d.fighters[1].position = if adjacent { 20 } else { 45 };
    d.dice = Dice(seed);
    d
}

fn legal(d: &Duel, action: Action) -> bool {
    d.legal_mask() & action.bit() != 0
}

#[test]
fn fresh_tournament_and_initiative() {
    let mut openings = [0; 2];
    let mut ties = 0;
    for seed in 0..200 {
        let d = Duel::new(seed);
        let mut rolls = Dice(seed);
        let scores = [rolls.roll(20) + 1, rolls.roll(20) + 3];
        let first = match scores[0].cmp(&scores[1]) {
            std::cmp::Ordering::Equal => {
                ties += 1;
                rolls.roll(2) as usize - 1
            }
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Greater => 0,
        };
        assert_eq!(d.actor(), Some(first));
        openings[first] += 1;
        assert_eq!(
            (d.distance(), d.movement_left(), d.turns_started()),
            (30, 30, 1)
        );
        for f in d.fighters {
            assert_eq!(
                (f.hp, f.winds, f.reaction, f.sapped, f.slowed),
                (12, 2, true, false, false)
            );
        }
    }
    assert!(ties > 0 && openings.iter().all(|n| *n > 0));
}

#[test]
fn leaving_reach_pauses_before_cost_and_pass_can_be_saved_for_next_exit() {
    let mut d = fixture(0, false, 0);
    for _ in 0..5 {
        assert_eq!(d.step(Approach), Ok(Event::Moved));
    }
    assert_eq!(d.distance(), 5);
    assert!(!legal(&d, Approach));
    let before = d.clone();
    assert_eq!(d.step(Retreat), Ok(Event::ReactionOffered));
    assert_eq!(d.actor(), Some(1));
    assert_eq!(d.fighters, before.fighters);
    assert_eq!(d.turn, before.turn);
    assert_eq!(d.legal_mask(), Opportunity.bit() | Pass.bit());
    assert_eq!(d.dice, before.dice);
    assert_eq!(d.step(Pass), Ok(Event::Passed));
    assert!(d.fighters[1].reaction);
    assert_eq!(
        (d.actor(), d.distance(), d.movement_left()),
        (Some(0), 10, 0)
    );
    assert_eq!(d.dice, before.dice);
}

#[test]
fn a_spent_reaction_does_not_reopen_until_reactors_next_turn() {
    let mut d = fixture(0, true, 5);
    d.step(Retreat).unwrap();
    let event = d.step(Opportunity).unwrap();
    assert!(matches!(
        event,
        Event::Attack {
            attacker: 1,
            damage: 0..=1,
            ..
        }
    ));
    assert_eq!(d.fighters[1].arrows, 20); // the archer kicks
    assert!(!d.fighters[1].reaction);
    assert!(d.turn.action && d.turn.bonus);
    d.step(Approach).unwrap();
    assert_eq!(d.step(Retreat), Ok(Event::Moved));
    d.step(EndTurn).unwrap();
    assert!(d.fighters[1].reaction);
}

#[test]
fn lethal_and_missed_opportunities_resolve_at_original_location() {
    let mut saw = [false; 2];
    for seed in 0..64 {
        let mut d = fixture(1, true, seed);
        d.fighters[1].hp = 1;
        d.step(Retreat).unwrap();
        let Event::Attack { hit, .. } = d.step(Opportunity).unwrap() else {
            panic!()
        };
        assert!(!d.fighters[0].reaction);
        assert!(d.turn.action && d.turn.bonus);
        saw[hit as usize] = true;
        if hit {
            assert_eq!(d.phase, Phase::Finished { winner: 0 });
            assert_eq!(
                (
                    d.actor(),
                    d.distance(),
                    d.turn.movement_spent,
                    d.legal_mask()
                ),
                (None, 5, 0, 0)
            );
            let snapshot = d.clone();
            for a in ACTIONS {
                assert!(d.step(a).is_err());
            }
            assert_eq!(d, snapshot);
        } else {
            assert_eq!(
                (d.phase, d.distance(), d.turn.movement_spent),
                (Phase::Main, 10, 5)
            );
        }
    }
    assert_eq!(saw, [true, true]);
}

#[test]
fn second_wind_uses_bonus_and_two_charges_with_capped_healing() {
    for seed in 0..64 {
        let mut d = fixture(0, false, seed);
        d.fighters[0].hp = 1;
        let mut rolls = Dice(seed);
        let restored = rolls.roll(10) + 1;
        assert_eq!(
            d.step(SecondWind),
            Ok(Event::Heal {
                fighter: 0,
                restored
            })
        );
        assert_eq!(d.fighters[0].hp, 1 + restored);
        assert!(d.turn.action && !d.turn.bonus && d.fighters[0].winds == 1);
        assert!(!legal(&d, SecondWind));
        d.step(EndTurn).unwrap();
        d.step(EndTurn).unwrap();
        d.fighters[0].hp = 11;
        assert_eq!(
            d.step(SecondWind),
            Ok(Event::Heal {
                fighter: 0,
                restored: 1
            })
        );
        d.step(EndTurn).unwrap();
        d.step(EndTurn).unwrap();
        assert!(!legal(&d, SecondWind) && d.turn.bonus);
    }
}

#[test]
fn attack_roll_threshold_crit_dice_and_ammunition_on_miss() {
    let mut saw = [false; 21];
    for seed in 0..512 {
        let mut d = fixture(1, false, seed);
        d.fighters[1].arrows = 1;
        let mut rolls = Dice(seed);
        let face = rolls.roll(20);
        saw[face as usize] = true;
        let hit = face + 7 >= 19;
        let damage = if hit {
            rolls.roll(8) + 3 + if face == 20 { rolls.roll(8) } else { 0 }
        } else {
            0
        };
        assert_eq!(
            d.step(Weapon),
            Ok(Event::Attack {
                attacker: 1,
                d20: face,
                hit,
                damage
            })
        );
        assert_eq!(d.fighters[1].arrows, 0);
        assert!(!d.turn.action && d.turn.bonus);
        if d.actor().is_some() {
            d.step(EndTurn).unwrap();
            d.step(EndTurn).unwrap();
            assert!(!legal(&d, Weapon));
        }
    }
    assert!(saw[1..].iter().all(|s| *s));
}

#[test]
fn close_range_and_sap_share_one_disadvantage_and_sap_consumes_on_miss() {
    for (adjacent, sapped) in [(true, false), (false, true), (true, true)] {
        for seed in 0..32 {
            let mut d = fixture(1, adjacent, seed);
            d.fighters[1].sapped = sapped;
            let mut rolls = Dice(seed);
            let face = rolls.roll(20).min(rolls.roll(20));
            assert!(matches!(d.step(Weapon), Ok(Event::Attack {d20, ..}) if d20 == face));
            assert!(!d.fighters[1].sapped);
        }
    }
}

#[test]
fn weapon_masteries_apply_on_hit_and_expire_at_source_start() {
    for i in 0..2 {
        let mut confirmed = 0;
        for seed in 0..100 {
            let mut d = fixture(i, true, seed);
            if let Ok(Event::Attack { hit: true, .. }) = d.step(Weapon) {
                if d.actor().is_none() {
                    continue;
                }
                let j = 1 - i;
                d.step(EndTurn).unwrap();
                assert_eq!(d.fighters[j].sapped, i == 0);
                assert_eq!(d.fighters[j].slowed, i == 1);
                assert_eq!(d.movement_left(), if i == 1 { 20 } else { 30 });
                d.step(EndTurn).unwrap();
                assert!(!d.fighters[j].sapped && !d.fighters[j].slowed);
                confirmed += 1;
            }
        }
        assert!(confirmed > 0);
    }
}

#[test]
fn boundaries_and_rejected_actions_preserve_rng_and_state() {
    let mut d = fixture(0, false, 2);
    for _ in 0..3 {
        d.step(Retreat).unwrap();
    }
    assert_eq!(d.fighters[0].position, 0);
    assert!(!legal(&d, Retreat));
    for a in [Retreat, Weapon, Pass, Opportunity, Unarmed] {
        let snapshot = d.clone();
        assert!(d.step(a).is_err());
        assert_eq!(snapshot, d);
    }
}

#[test]
fn independent_replays_are_identical_with_interleaved_duels() {
    let mut a = Duel::new(42);
    let mut b = Duel::new(42);
    let mut other = Duel::new(97);
    for _ in 0..2048 {
        assert_eq!(a, b);
        let Some(action) = a.bot() else { return };
        let event = a.step(action).unwrap();
        if let Some(choice) = other.bot() {
            other.step(choice).unwrap();
        }
        assert_eq!(b.step(action).unwrap(), event);
    }
    panic!("replay did not terminate");
}
