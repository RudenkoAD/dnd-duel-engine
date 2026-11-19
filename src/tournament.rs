//! One-on-one tournament policy, including starting conditions and victory.
use crate::{
    dice::Dice,
    presets::{Build, BUILDS},
    Fighter,
};

pub const ARENA_FEET: i32 = 60;
const START: [i32; 2] = [15, 45];

pub(crate) fn entrants() -> [Fighter; 2] {
    std::array::from_fn(|i| fresh(&BUILDS[i], START[i]))
}

fn fresh(build: &Build, position: i32) -> Fighter {
    Fighter {
        hp: build.hp,
        position,
        winds: build.winds,
        arrows: build.arrows,
        reaction: true,
        sapped: false,
        slowed: false,
    }
}

pub(crate) fn initiative(dice: &mut Dice) -> usize {
    let scores = BUILDS.map(|b| dice.roll(20) + b.dex);
    match scores[0].cmp(&scores[1]) {
        std::cmp::Ordering::Greater => 0,
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => (dice.roll(2) - 1) as usize,
    }
}

pub(crate) fn winner(fighters: &[Fighter; 2]) -> Option<usize> {
    fighters
        .iter()
        .position(|f| f.hp == 0)
        .map(|loser| 1 - loser)
}
