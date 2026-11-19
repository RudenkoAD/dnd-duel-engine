//! Common actions and conditions shared across duel builds.
use super::battle::Battle;
use super::geometry::Pos;

pub use super::origins::Skill;

impl Battle {
    /// In the same plane, noises expose a combatant's position until they Hide.
    /// Across a planar boundary, only the visibility granted by the effect applies.
    pub fn observed_pos(&self, observer: usize, subject: usize) -> Option<Pos> {
        let same_plane = self.rope_same_space(observer, subject);
        let senses = same_plane
            && (self.origin_senses(observer, subject)
                || self.entity_shared_blindsight(observer, subject));
        (observer == subject
            || (self.f[subject].hidden_dc.is_none() || senses)
                && (same_plane || self.clear_sight(observer, subject)))
            .then_some(self.f[subject].pos)
    }

    fn clear_sight(&self, observer: usize, subject: usize) -> bool {
        if !self.rope_can_see(observer, subject) {
            return false;
        }
        self.footprint(observer)
            .zip(self.footprint(subject))
            .is_some_and(|(from, to)| {
                let from = self.portal_viewpoint(observer, subject)
                    .map(super::footprint::Footprint::point).unwrap_or(from);
                from.squares().any(|origin| {
                    to.squares().any(|target| {
                        self.map.los_at(origin, target)
                            && !self.magic_cloud_blocks(origin, target)
                            && self.darkness_allows_sight(observer, subject, origin, target)
                    })
                })
            })
    }

    pub fn can_see(&self, observer: usize, subject: usize) -> bool {
        observer == subject
            || (!self.sleep_unconscious(observer) && self.rope_can_see(observer, subject)
            && ((self.rope_same_space(observer, subject) && self.entity_shared_blindsight(observer, subject)
                && self
                    .map
                    .los_at(self.fighter_point(observer), self.fighter_point(subject)))
            || ((self.f[subject].hidden_dc.is_none() || self.projectile_glowing(subject))
                && (!self.magic_invisible(subject) || self.arcane_sees_invisible(observer))
                && self.clear_sight(observer, subject))))
    }

    pub(super) fn reveal(&mut self, i: usize) {
        self.f[i].hidden_dc = None;
    }

    pub(super) fn knock_prone(&mut self, i: usize) {
        self.f[i].prone = true;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Check {
    Hide,
    Search,
    Escape(Skill),
    Disarm(usize),
    Study(super::origins::StudyTarget),
    ExaminePhantasm,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FailedCheck {
    pub check: Check,
    pub total: i32,
    pub dc: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnarmedManeuver {
    Grapple,
    ShovePush,
    ShoveProne,
}

impl Battle {
    pub(super) fn can_hide(&self, i: usize) -> bool {
        self.f[i].hidden_dc.is_none()
            && !self.projectile_glowing(i)
            && self
                .observed_pos(i, 1 - i)
                .is_none_or(|_| !self.can_see(1 - i, i))
    }

    pub(super) fn try_check(&mut self, i: usize, check: Check) -> Option<FailedCheck> {
        let (skill, _) = self.check_requirements(i, check)?;
        if self.origin_can_use_primal_strength(i, skill) {
            self.martial_primal_request(i, check);
            return None;
        }
        self.try_check_as(i, check, false)
    }

    pub(super) fn check_requirements(&self, i: usize, check: Check) -> Option<(Skill, i32)> {
        Some(match check {
            Check::Hide => {
                if self.can_see(1 - i, i) {
                    return None;
                }
                (Skill::Stealth, 15)
            }
            Check::Search => (Skill::Perception, self.f[1 - i].hidden_dc?),
            Check::Escape(skill) => (skill, self.f[i].grappled?),
            Check::Disarm(_) => (Skill::SleightOfHand, 15),
            Check::Study(target) => {
                if !self.can_study(i, target) {
                    return None;
                }
                if target.id == super::bard_ranger::DISGUISE_STUDY_ID {
                    (Skill::Investigation, self.spell_dc(1 - i))
                } else {
                    (Skill::Arcana, 15)
                }
            }
            Check::ExaminePhantasm => (Skill::Investigation, self.phantasmal_force_dc(i)?),
        })
    }

    pub(in crate::arena2) fn try_check_as(
        &mut self,
        i: usize,
        check: Check,
        use_primal: bool,
    ) -> Option<FailedCheck> {
        let (skill, dc) = self.check_requirements(i, check)?;
        let (mut bonus, mut advantage) = self.martial_skill_mod_as(i, skill, use_primal);
        if check == Check::Hide && self.magic_pass_without_trace(i) {
            bonus += 10;
        }
        advantage |= check == Check::Search && self.martial_marked_search(i);
        if self.help_skill[i] == Some(skill) {
            self.help_skill[i] = None;
            advantage = true;
        }
        let stat = if use_primal { 0 } else { skill.ability() };
        advantage |= self.magic_enhances_ability(i, stat);
        let disadvantage = self.armor_disadvantages_ability(i, stat)
            || self.magic_ability_disadvantage(i, stat)
            || self.martial_poisoned(i)
            || self.magic_poisoned(i)
            || self.entity_frightened(i)
            || (skill == Skill::Stealth && self.armor_stealth_disadvantage(i));
        let floor = if matches!(stat, 3 | 4) && self.divine_dragon(i) { 10 } else { 1 };
        let total = self.d20_for(i, advantage, disadvantage).max(floor) + bonus + self.magic_guidance(i, skill);
        if total >= dc {
            self.resolve_successful_check(i, FailedCheck { check, total, dc }, false);
        } else {
            return Some(FailedCheck { check, total, dc });
        }
        None
    }

    pub(super) fn tactical_mind(&mut self, i: usize, check: FailedCheck) {
        let total = check.total + self.roll(10);
        if total >= check.dc {
            self.resolve_successful_check(i, FailedCheck { total, ..check }, true);
        }
    }

    pub(super) fn apply_check(&mut self, i: usize, check: Check, total: i32) {
        match check {
            Check::Hide => self.f[i].hidden_dc = Some(total),
            Check::Search => self.reveal(1 - i),
            Check::Escape(_) => self.f[i].grappled = None,
            Check::Disarm(seat) => self.disable_caltrops(seat),
            Check::Study(target) => self.identify_spell(i, target),
            Check::ExaminePhantasm => self.end_phantasmal_force(i),
        }
    }

    pub(super) fn maneuver_against(
        &mut self,
        i: usize,
        opponent: usize,
        maneuver: UnarmedManeuver,
        stat: Option<usize>,
    ) {
        let f = &self.f[i];
        let ability = self.martial_unarmed_ability(i);
        let dc = 8 + self.magic_save_modifier(i, ability) + f.build.pb;
        self.martial_force_save(i);
        if opponent >= 2 {
            if stat.is_none_or(|stat| !self.entity_save(opponent, stat, dc)) {
                match maneuver {
                    UnarmedManeuver::Grapple => self.entity_enter_grapple(opponent, i, dc),
                    UnarmedManeuver::ShovePush => self.entity_forced_push(i, opponent, 1),
                    UnarmedManeuver::ShoveProne => self.entity_knock_prone(opponent),
                }
            }
            return;
        }
        if stat.is_some_and(|stat| self.save(opponent, stat, dc)) {
            return;
        }
        match maneuver {
            UnarmedManeuver::Grapple => {
                self.f[opponent].grappled = Some(dc);
                self.f[i].dragging = true;
            }
            UnarmedManeuver::ShovePush => self.forced_push(i, opponent, 1),
            UnarmedManeuver::ShoveProne => self.knock_prone(opponent),
        }
    }

    pub(super) fn forced_push(&mut self, source: usize, target: usize, steps: i32) {
        use super::geometry::DIRECTIONS;
        let from = self.f[source].pos;
        let pos = self.f[target].pos;
        let direction = ((pos.x - from.x).signum(), (pos.y - from.y).signum());
        let Some(dir) = DIRECTIONS
            .iter()
            .position(|&candidate| candidate == direction)
        else {
            return;
        };
        for _ in 0..steps {
            let Some((point, _)) = self.map.step_at(self.fighter_point(target), dir) else {
                break;
            };
            if (2..4).any(|id| self.creature_point(id) == Some(point)) {
                break;
            }
            self.commit_forced_move(target, point);
            if self.f[target].hp <= 0 {
                break;
            }
        }
        self.clear_broken_grapples();
    }

    pub(super) fn clear_broken_grapples(&mut self) {
        for i in 0..2 {
            if self.f[i].grappled.is_some()
                && (self.dist() > self.unarmed_reach(1 - i) || self.f[1 - i].hp == 0 || self.incap(1 - i))
            {
                self.f[i].grappled = None;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadyTrigger {
    EnterReach,
    EnterSight,
    Attack,
    Move,
    LeaveReach,
    Cast,
    Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readied {
    pub trigger: ReadyTrigger,
    pub action: super::Act,
}

impl ReadyTrigger {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::EnterReach => "Enemy enters melee reach",
            Self::EnterSight => "Enemy enters sight",
            Self::Attack => "Enemy completes an attack",
            Self::Move => "Enemy moves while visible",
            Self::LeaveReach => "Enemy leaves melee reach",
            Self::Cast => "Enemy visibly casts a spell",
            Self::Action => "Enemy uses an action or bonus action",
        }
    }
}

pub(super) const READY_TRIGGERS: [ReadyTrigger; 7] = [
    ReadyTrigger::EnterReach,
    ReadyTrigger::EnterSight,
    ReadyTrigger::Attack,
    ReadyTrigger::Move,
    ReadyTrigger::LeaveReach,
    ReadyTrigger::Cast,
    ReadyTrigger::Action,
];
