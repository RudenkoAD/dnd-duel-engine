//! A duel advances only at explicit decisions. Every random draw belongs to its duel.
mod dice;
mod ffi;
pub mod presets;
pub mod tournament;

use dice::Dice;
use presets::{Mastery, BUILDS};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Approach,
    Retreat,
    Weapon,
    Unarmed,
    SecondWind,
    EndTurn,
    Opportunity,
    Pass,
}
pub const ACTIONS: [Action; 8] = [
    Action::Approach,
    Action::Retreat,
    Action::Weapon,
    Action::Unarmed,
    Action::SecondWind,
    Action::EndTurn,
    Action::Opportunity,
    Action::Pass,
];

impl Action {
    pub fn bit(self) -> u32 {
        1 << (self as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fighter {
    pub hp: i32,
    pub position: i32,
    pub winds: i32,
    pub arrows: i32,
    pub reaction: bool,
    pub sapped: bool,
    pub slowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    pub owner: usize,
    pub movement_spent: i32,
    pub action: bool,
    pub bonus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Main,
    Reaction { destination: i32 },
    Finished { winner: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Attack {
        attacker: usize,
        d20: i32,
        hit: bool,
        damage: i32,
    },
    Heal {
        fighter: usize,
        restored: i32,
    },
    Moved,
    ReactionOffered,
    Passed,
    TurnEnded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duel {
    dice: Dice,
    fighters: [Fighter; 2],
    turn: Turn,
    phase: Phase,
    turns_started: u32,
}

impl Duel {
    pub fn new(seed: u64) -> Self {
        let mut dice = Dice(seed);
        let first = tournament::initiative(&mut dice);
        Self {
            dice,
            fighters: tournament::entrants(),
            phase: Phase::Main,
            turn: Turn {
                owner: first,
                movement_spent: 0,
                action: true,
                bonus: true,
            },
            turns_started: 1,
        }
    }

    pub fn fighters(&self) -> &[Fighter; 2] {
        &self.fighters
    }
    pub fn turn(&self) -> Turn {
        self.turn
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn turns_started(&self) -> u32 {
        self.turns_started
    }
    pub fn actor(&self) -> Option<usize> {
        match self.phase {
            Phase::Main => Some(self.turn.owner),
            Phase::Reaction { .. } => Some(1 - self.turn.owner),
            Phase::Finished { .. } => None,
        }
    }

    pub fn distance(&self) -> i32 {
        (self.fighters[0].position - self.fighters[1].position).abs()
    }

    fn destination(&self, approach: bool) -> i32 {
        let i = self.turn.owner;
        let pos = self.fighters[i].position;
        let toward = (self.fighters[1 - i].position - pos).signum();
        pos + if approach { 5 * toward } else { -5 * toward }
    }

    pub fn movement_left(&self) -> i32 {
        let i = self.turn.owner;
        let penalty = if self.fighters[i].slowed { 10 } else { 0 };
        (BUILDS[i].speed - penalty - self.turn.movement_spent).max(0)
    }

    pub fn legal_mask(&self) -> u32 {
        use Action::*;
        match self.phase {
            Phase::Finished { .. } => return 0,
            Phase::Reaction { .. } => return Opportunity.bit() | Pass.bit(),
            Phase::Main => {}
        }
        let i = self.turn.owner;
        let fighter = self.fighters[i];
        let adjacent = self.distance() == 5;
        let mut mask = EndTurn.bit();
        if self.movement_left() >= 5 {
            if !adjacent {
                mask |= Approach.bit();
            }
            if (0..=tournament::ARENA_FEET).contains(&self.destination(false)) {
                mask |= Retreat.bit();
            }
        }
        if self.turn.action {
            if adjacent {
                mask |= Unarmed.bit();
            }
            if (BUILDS[i].weapon.ranged && fighter.arrows > 0)
                || (!BUILDS[i].weapon.ranged && adjacent)
            {
                mask |= Weapon.bit();
            }
        }
        if self.turn.bonus && fighter.winds > 0 {
            mask |= SecondWind.bit();
        }
        mask
    }

    pub fn step(&mut self, choice: Action) -> Result<Event, &'static str> {
        if self.legal_mask() & choice.bit() == 0 {
            return Err("illegal action");
        }
        let i = self.turn.owner;
        use Action::*;
        Ok(match choice {
            Approach | Retreat => {
                let destination = self.destination(choice == Approach);
                if choice == Retreat && self.distance() == 5 && self.fighters[1 - i].reaction {
                    self.phase = Phase::Reaction { destination };
                    Event::ReactionOffered
                } else {
                    self.move_to(destination);
                    Event::Moved
                }
            }
            Opportunity | Pass => {
                let Phase::Reaction { destination } = self.phase else {
                    unreachable!()
                };
                self.phase = Phase::Main;
                let mut event = Event::Passed;
                if choice == Opportunity {
                    self.fighters[1 - i].reaction = false;
                    event = self.attack(1 - i, !BUILDS[1 - i].weapon.ranged);
                }
                if self.phase == Phase::Main {
                    self.move_to(destination);
                }
                event
            }
            Weapon | Unarmed => {
                self.turn.action = false;
                self.attack(i, choice == Weapon)
            }
            SecondWind => {
                self.turn.bonus = false;
                let fighter = &mut self.fighters[i];
                fighter.winds -= 1;
                let restored =
                    (self.dice.roll(10) + BUILDS[i].level).min(BUILDS[i].hp - fighter.hp);
                fighter.hp += restored;
                Event::Heal {
                    fighter: i,
                    restored,
                }
            }
            EndTurn => {
                self.begin_turn(1 - i);
                Event::TurnEnded
            }
        })
    }

    fn move_to(&mut self, position: i32) {
        self.fighters[self.turn.owner].position = position;
        self.turn.movement_spent += 5;
    }

    fn begin_turn(&mut self, owner: usize) {
        self.turn = Turn {
            owner,
            action: true,
            bonus: true,
            movement_spent: 0,
        };
        self.fighters[owner].reaction = true;
        // In a duel all marks on the opponent have this new turn's owner as source.
        self.fighters[1 - owner].slowed = false;
        self.fighters[1 - owner].sapped = false;
        self.turns_started = self.turns_started.saturating_add(1);
    }

    fn attack(&mut self, i: usize, weapon: bool) -> Event {
        let defender = 1 - i;
        let build = BUILDS[i];
        let ranged = weapon && build.weapon.ranged;
        if ranged {
            self.fighters[i].arrows -= 1;
        }
        let disadvantage = self.fighters[i].sapped || (ranged && self.distance() <= 5);
        self.fighters[i].sapped = false;
        let mut d20 = self.dice.roll(20);
        if disadvantage {
            d20 = d20.min(self.dice.roll(20));
        }
        let ability = if ranged { build.dex } else { build.strength };
        let attack_bonus =
            ability + build.proficiency + if weapon { build.style_attack_bonus } else { 0 };
        let hit = d20 == 20 || (d20 != 1 && d20 + attack_bonus >= BUILDS[defender].ac);
        let mut damage = 0;
        if hit {
            damage = if weapon {
                let sides = build.weapon.die as u64;
                let first = self.dice.roll(sides);
                first + ability + if d20 == 20 { self.dice.roll(sides) } else { 0 }
            } else {
                1 + build.strength
            };
            self.fighters[defender].hp = (self.fighters[defender].hp - damage).max(0);
            if weapon {
                match build.weapon.mastery {
                    Mastery::Sap => self.fighters[defender].sapped = true,
                    Mastery::Slow if damage > 0 => self.fighters[defender].slowed = true,
                    _ => {}
                }
            }
            if let Some(winner) = tournament::winner(&self.fighters) {
                self.phase = Phase::Finished { winner };
            }
        }
        Event::Attack {
            attacker: i,
            d20,
            hit,
            damage,
        }
    }

    /// A deterministic baseline using the same decision interface as external agents.
    pub fn bot(&self) -> Option<Action> {
        use Action::*;
        let i = self.actor()?;
        let can = |a: Action| self.legal_mask() & a.bit() != 0;
        if can(Opportunity) {
            return Some(Opportunity);
        }
        if self.fighters[i].hp <= BUILDS[i].hp / 2 && can(SecondWind) {
            return Some(SecondWind);
        }
        let bow = BUILDS[i].weapon.ranged && self.fighters[i].arrows > 0;
        if bow && self.distance() == 5 && can(Retreat) {
            return Some(Retreat);
        }
        if can(Weapon) {
            return Some(Weapon);
        }
        if can(Unarmed) {
            return Some(Unarmed);
        }
        if !bow && can(Approach) {
            return Some(Approach);
        }
        if bow && self.distance() <= 30 && can(Retreat) {
            return Some(Retreat);
        }
        Some(EndTurn)
    }
}

#[cfg(test)]
mod tests;

pub mod arena2;
