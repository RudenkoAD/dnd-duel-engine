//! Optional pre-roll choices during setup for characters with arcane d20 features.
use super::{Battle, Build, Class, Entities};
use crate::arena2::{Act, ACTS};

pub(super) const PORTENT: i32 = 178;
pub(super) const BALANCE: i32 = 179;

#[derive(Clone, Debug)]
pub(super) struct State {
    totals: [i32; 2],
    surprised: [bool; 2],
    target: usize,
    next_candidate: usize,
    balance_restored: bool,
    replacement: Option<i32>,
}

impl State {
    pub(super) fn new(surprised: [bool; 2]) -> Self {
        Self {
            totals: [0; 2],
            surprised,
            target: 0,
            next_candidate: 0,
            balance_restored: false,
            replacement: None,
        }
    }

    fn advantage(&self, build: Build) -> bool {
        build.level >= 3
            && (build.is_subclass(Class::Fighter, 0) || build.is_subclass(Class::Rogue, 1))
            && !self.balance_restored
    }

    fn disadvantage(&self) -> bool {
        self.surprised[self.target] && !self.balance_restored
    }
}

impl Battle {
    pub(super) fn arcane_initiative_required(builds: [Build; 2]) -> bool {
        builds.iter().any(|build| {
            build.level >= 3
                && (build.is_subclass(Class::Wizard, 2) || build.is_subclass(Class::Sorcerer, 2))
        })
    }

    pub(super) fn advance_arcane_initiative(&mut self) {
        let mut setup = self.arcane_initiative.take().expect("deferred initiative");
        while setup.target < 2 {
            let target = setup.target;
            let build = self.f[target].build;
            let advantage = setup.advantage(build);
            let disadvantage = setup.disadvantage();
            if setup.replacement.is_none() {
                while setup.next_candidate < 2 {
                    let caster = target ^ setup.next_candidate;
                    setup.next_candidate += 1;
                    if !self.can_see(caster, target) {
                        continue;
                    }
                    let state = &self.f[caster].arcane;
                    let phase = if state.portent.iter().any(|&die| die > 0) {
                        PORTENT
                    } else if state.balance > 0
                        && self.can_react(caster)
                        && advantage != disadvantage
                        && self
                            .combatant_distance(caster, target)
                            .is_some_and(|distance| distance <= 12)
                    {
                        BALANCE
                    } else {
                        continue;
                    };
                    self.arcane_initiative = Some(setup);
                    self.request(caster, phase);
                    return;
                }
            }
            let roll = setup
                .replacement
                .take()
                .unwrap_or_else(|| self.d20(advantage, disadvantage));
            setup.totals[target] =
                roll + build.stats[1] + crate::arena2::bard_ranger::gloom_initiative(build);
            setup.target += 1;
            setup.next_candidate = 0;
            setup.balance_restored = false;
        }
        self.entities = Entities::new(setup.totals);
        self.owner = if setup.totals[0] > setup.totals[1] {
            0
        } else if setup.totals[1] > setup.totals[0] {
            1
        } else {
            (self.roll(2) - 1) as usize
        };
        self.f[self.owner].bard_ranger.first_turn = 1;
        self.f[1 - self.owner].bard_ranger.first_turn = 2;
        self.f[self.owner].reaction = true;
        self.subclass_start_turn(self.owner);
    }

    pub(super) fn arcane_initiative_mask(&self, actor: usize, phase: i32) -> u128 {
        if phase == BALANCE {
            return Act::Pass.bit() | Act::Defend.bit();
        }
        self.f[actor]
            .arcane
            .portent
            .into_iter()
            .enumerate()
            .filter(|&(_, die)| die > 0)
            .fold(Act::Pass.bit(), |mask, (index, _)| mask | ACTS[index].bit())
    }

    pub(super) fn arcane_initiative_step(&mut self, actor: usize, phase: i32, action: Act) {
        let setup = self.arcane_initiative.as_mut().expect("initiative choice");
        if phase == PORTENT && action != Act::Pass {
            setup.replacement = Some(std::mem::take(
                &mut self.f[actor].arcane.portent[action as usize],
            ));
        } else if phase == BALANCE && action == Act::Defend {
            setup.balance_restored = true;
            self.f[actor].arcane.balance -= 1;
            self.f[actor].reaction = false;
        }
        self.advance_arcane_initiative();
    }

    pub(super) fn arcane_initiative_label(&self, actor: usize, phase: i32, action: Act) -> String {
        if action == Act::Pass {
            "Let initiative roll normally".into()
        } else if phase == BALANCE {
            "Restore Balance".into()
        } else {
            format!(
                "Use Portent: {}",
                self.f[actor].arcane.portent[action as usize]
            )
        }
    }

    pub(super) fn arcane_initiative_summary(&self) -> Option<String> {
        let setup = self.arcane_initiative.as_ref()?;
        Some(format!(
            "Choose before {} rolls initiative",
            self.creature_label(setup.target)
        ))
    }

    pub(in crate::arena2) fn arcane_initiative_features(&self, actor: usize) -> Option<[f32; 8]> {
        if self.actor() != Some(actor) {
            return None;
        }
        let setup = self.arcane_initiative.as_ref()?;
        let [first, second] = self.f[actor].arcane.portent;
        Some([
            first as f32 / 20.,
            second as f32 / 20.,
            u8::from(actor == setup.target).into(),
            u8::from(setup.advantage(self.f[setup.target].build)).into(),
            u8::from(setup.disadvantage()).into(),
            0.,
            0.,
            0.,
        ])
    }
}
