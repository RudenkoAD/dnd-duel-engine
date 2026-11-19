//! Bard colleges and Ranger archetypes unlocked at level three.
use super::{
    battle::Battle,
    class::{Build, Class},
};

pub(super) fn is_valor(build: Build) -> bool {
    build.class == Class::Bard && build.variant == 1 && build.level >= 3
}

pub(super) fn is_glamour(build: Build) -> bool {
    build.class == Class::Bard && build.variant == 2 && build.level >= 3
}

pub(super) fn is_fey_wanderer(build: Build) -> bool {
    build.class == Class::Ranger && build.variant == 1 && build.level >= 3
}

pub(super) fn is_gloom_stalker(build: Build) -> bool {
    build.class == Class::Ranger && build.variant == 2 && build.level >= 3
}

pub(super) fn gloom_initiative(build: Build) -> i32 {
    if is_gloom_stalker(build) {
        build.stats[4]
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct BardRangerState {
    pub dread_uses: i32,
    pub dread_turn: i32,
    pub dreadful_turns: [i32; 4],
    pub first_turn: i32,
    rope: Option<RopePortal>,
    rope_inside: Option<usize>,
    pub mirrors: u8,
    pub mirror_until: i32,
    pub mirror_tier: usize,
    pub disguise_until: i32,
    pub disguise_tier: usize,
    pub person_charm: [i32; 4],
    pub person_charm_tier: [usize; 4],
    beguiling_targets: u8,
    beguiling_target: usize,
    beguiling_used: bool,
    beguiling_charm: [i32; 4],
    beguiling_fear: [i32; 4],
    mantle_targets: u8,
    mantle_subject: Option<usize>,
    mantle_budget: i32,
    mantle_allowance: i32,
    mantle_started: bool,
}

impl BardRangerState {
    pub(super) fn new(build: Build) -> Self {
        Self {
            dread_uses: if is_gloom_stalker(build) {
                build.stats[4].max(1)
            } else {
                0
            },
            dread_turn: -1,
            dreadful_turns: [-1; 4],
            first_turn: 0,
            rope: None,
            rope_inside: None,
            mirrors: 0,
            mirror_until: 0,
            mirror_tier: 0,
            disguise_until: 0,
            disguise_tier: 0,
            person_charm: [0; 4],
            person_charm_tier: [0; 4],
            beguiling_targets: 0,
            beguiling_target: 0,
            beguiling_used: false,
            beguiling_charm: [0; 4],
            beguiling_fear: [0; 4],
            mantle_targets: 0,
            mantle_subject: None,
            mantle_budget: 0,
            mantle_allowance: 0,
            mantle_started: false,
        }
    }
}

impl Battle {
    pub(super) fn gloom_speed_bonus(&self, i: usize) -> i32 {
        let fighter = &self.f[i];
        if is_gloom_stalker(fighter.build) && fighter.bard_ranger.first_turn == self.turns {
            2
        } else {
            0
        }
    }
}

use super::{magic::Spell, temp_hp::TempSource, Act, ACTS};

pub(super) const BEGUILING_TARGET: i32 = 183;
pub(super) const BEGUILING_CONDITION: i32 = 184;
pub(super) const MANTLE_TARGETS: i32 = 185;
pub(super) const MANTLE_MOVE: i32 = 186;
pub(super) const MANTLE: Act = Act::Magic21;
pub(super) const RESTORE_BEGUILING: Act = Act::Magic22;
pub(super) const DISGUISE_STUDY_ID: u8 = 61;

pub(super) fn prepared_spell(build: Build, action: Act) -> Option<Spell> {
    match action {
        Act::Magic6 if is_glamour(build) || is_fey_wanderer(build) => Some(Spell::CharmPerson),
        Act::Magic7 if is_glamour(build) => Some(Spell::MirrorImage),
        Act::Magic7 if is_fey_wanderer(build) && build.level >= 5 => Some(Spell::MistyStep),
        Act::Magic6 if is_gloom_stalker(build) => Some(Spell::DisguiseSelf),
        Act::Magic7 if is_gloom_stalker(build) && build.level >= 5 => Some(Spell::RopeTrick),
        _ => None,
    }
}

impl Battle {
    pub(super) fn mark_beguiling_spell(&mut self, caster: usize, spell: Spell, tier: usize) {
        if tier > 0
            && is_glamour(self.f[caster].build)
            && matches!(
                spell,
                Spell::DissonantWhispers
                    | Spell::Command
                    | Spell::CharmPerson
                    | Spell::HoldPerson
                    | Spell::HypnoticPattern
                    | Spell::MirrorImage
            )
        {
            self.f[caster].bard_ranger.beguiling_targets = (0..4)
                .filter(|&id| {
                    id != caster
                        && (id >= 2 || self.f[id].hp > 0)
                        && self.rope_same_space(caster, id)
                        && self.entity_visible(caster, id)
                        && self.combatant_distance(caster, id).is_some_and(|d| d <= 12)
                })
                .fold(0, |targets, id| targets | (1 << id));
        }
    }

    pub(super) fn begin_beguiling_prompt(&mut self) {
        for caster in 0..2 {
            let state = &self.f[caster].bard_ranger;
            if state.beguiling_targets == 0 {
                continue;
            }
            if (!state.beguiling_used || self.f[caster].resource > 0)
                && (0..4).any(|id| {
                    state.beguiling_targets & (1 << id) != 0 && self.creature_point(id).is_some()
                })
            {
                self.request(caster, BEGUILING_TARGET);
                return;
            }
            self.f[caster].bard_ranger.beguiling_targets = 0;
        }
    }

    pub(super) fn beguiling_restore_available(&self, i: usize) -> bool {
        is_glamour(self.f[i].build)
            && self.f[i].bard_ranger.beguiling_used
            && self.f[i].resource > 0
    }

    pub(super) fn restore_beguiling(&mut self, i: usize) {
        self.f[i].resource -= 1;
        self.f[i].bard_ranger.beguiling_used = false;
    }

    pub(super) fn mantle_available(&self, i: usize) -> bool {
        is_glamour(self.f[i].build)
            && self.bonus
            && self.f[i].resource > 0
            && (0..4).any(|id| self.mantle_recipient(i, id))
    }

    fn mantle_recipient(&self, bard: usize, subject: usize) -> bool {
        subject != bard
            && (subject >= 2 || self.f[subject].hp > 0)
            && self.rope_same_space(bard, subject)
            && self
                .combatant_distance(bard, subject)
                .is_some_and(|d| d <= 12)
    }

    pub(super) fn start_mantle(&mut self, i: usize) {
        self.f[i].bard_ranger.mantle_targets = 0;
        self.request(i, MANTLE_TARGETS);
    }

    pub(super) fn bard_ranger_mask(&self, i: usize, phase: i32) -> u128 {
        let state = &self.f[i].bard_ranger;
        match phase {
            BEGUILING_TARGET => (0..4)
                .filter(|&id| {
                    state.beguiling_targets & (1 << id) != 0 && self.creature_point(id).is_some()
                })
                .fold(Act::Pass.bit(), |mask, id| mask | ACTS[id].bit()),
            BEGUILING_CONDITION => Act::Pass.bit() | ACTS[0].bit() | ACTS[1].bit(),
            MANTLE_TARGETS => {
                let mut mask = Act::Pass.bit();
                if state.mantle_targets != 0 {
                    mask |= Act::Other7.bit();
                }
                if state.mantle_targets.count_ones() as i32 >= self.f[i].build.casting.max(1) {
                    return mask;
                }
                for target in 0..4 {
                    if state.mantle_targets & (1 << target) == 0 && self.mantle_recipient(i, target)
                    {
                        mask |= ACTS[target].bit();
                    }
                }
                mask
            }
            MANTLE_MOVE => self.mantle_move_mask(self.mantle_source()),
            _ => 0,
        }
    }

    fn mantle_source(&self) -> usize {
        (0..2)
            .find(|&id| self.f[id].bard_ranger.mantle_subject.is_some())
            .expect("a Mantle movement selection has a source")
    }

    fn mantle_move_mask(&self, bard: usize) -> u128 {
        let state = &self.f[bard].bard_ranger;
        let mut mask = Act::Pass.bit();
        if let Some(subject) = state.mantle_subject {
            if self
                .mantle_stand_cost(subject)
                .is_some_and(|cost| cost <= state.mantle_budget)
            {
                mask |= Act::Stand.bit();
            }
        }
        ACTS[..8]
            .iter()
            .copied()
            .chain(
                ACTS[Act::Magic1 as usize..=Act::Magic10 as usize]
                    .iter()
                    .copied(),
            )
            .chain([Act::OtherCommon1, Act::OtherCommon2])
            .filter(|&action| self.mantle_destination(bard, action).is_some())
            .fold(mask, |mask, action| mask | action.bit())
    }

    fn mantle_stand_cost(&self, subject: usize) -> Option<i32> {
        let (prone, speed) = if subject < 2 {
            (self.f[subject].prone, self.speed(subject))
        } else {
            (
                self.entity(subject)?.martial.prone,
                self.entity_effective_speed(subject),
            )
        };
        (prone && speed > 0).then_some((speed + 1) / 2)
    }

    fn spend_mantle_reaction(&mut self, bard: usize, subject: usize) {
        if self.f[bard].bard_ranger.mantle_started {
            return;
        }
        if subject < 2 {
            self.f[subject].reaction = false;
        } else {
            assert!(self.entity_spend_reaction(subject));
        }
        self.f[bard].bard_ranger.mantle_started = true;
    }

    fn mantle_max_speed(&self, subject: usize) -> i32 {
        let walk = self.speed(subject);
        if walk > 0 {
            walk.max(self.climb_speed(subject))
                .max(self.traversal_fly_speed(subject))
        } else {
            0
        }
    }

    fn mantle_vertical_action(action: Act) -> Option<usize> {
        let id = action as usize;
        match action {
            Act::OtherCommon1 => Some(10),
            Act::OtherCommon2 => Some(11),
            _ if (Act::Magic1 as usize..=Act::Magic10 as usize).contains(&id) => {
                Some(id - Act::Magic1 as usize)
            }
            _ => None,
        }
    }

    fn mantle_destination(&self, i: usize, action: Act) -> Option<(Point, i32, Option<bool>)> {
        let state = &self.f[i].bard_ranger;
        let subject = state.mantle_subject?;
        let index = action as usize;
        let remaining = state.mantle_budget;
        if subject >= 2 {
            if index < 8 {
                let (point, cost) = self.entity_reaction_destination(subject, index, remaining)?;
                return Some((point, cost, None));
            }
            let vertical = Self::mantle_vertical_action(action)?;
            let direction = match vertical {
                10 => 1,
                11 => -1,
                _ => return None,
            };
            let point =
                self.entity_vertical_destination_with_budget(subject - 2, direction, remaining)?;
            return Some((point, 1, None));
        }
        let spent = state.mantle_allowance - remaining;
        let walk = self.speed(subject);
        let fly = self.traversal_fly_speed(subject);
        let climb = self.climb_speed(subject);
        if index < 8 {
            let from = self.fighter_point(subject);
            let mut speed = walk;
            if !self.map.supported(from) {
                speed = speed.max(fly).max(climb);
            }
            let (cell, cost) = self.movement_step(subject, index)?;
            return (cost <= remaining.min(speed - spent)
                && self.movement_available(subject, cell))
            .then_some((
                Point {
                    cell,
                    altitude: from.altitude,
                },
                cost,
                None,
            ));
        }
        let vertical = Self::mantle_vertical_action(action)?;
        let speed = if vertical < 10 { climb.max(walk) } else { fly };
        let (point, cost, climbing) =
            self.traversal_safe_vertical(subject, vertical, remaining.min(speed - spent))?;
        Some((point, cost, Some(climbing)))
    }

    fn next_mantle_move(&mut self, i: usize) {
        self.f[i].bard_ranger.mantle_subject = None;
        self.f[i].bard_ranger.mantle_budget = 0;
        while self.f[i].bard_ranger.mantle_targets != 0 {
            let subject = self.f[i].bard_ranger.mantle_targets.trailing_zeros() as usize;
            self.f[i].bard_ranger.mantle_targets &= !(1 << subject);
            let budget = if subject < 2 {
                self.can_react(subject)
                    .then(|| self.mantle_max_speed(subject))
            } else {
                self.entity_reaction_budget(subject)
                    .filter(|_| !self.magic_no_reactions(subject))
            };
            if let Some(budget) = budget.filter(|&budget| budget > 0) {
                self.f[i].bard_ranger.mantle_subject = Some(subject);
                self.f[i].bard_ranger.mantle_budget = budget;
                self.f[i].bard_ranger.mantle_allowance = budget;
                self.f[i].bard_ranger.mantle_started = false;
                if self.mantle_move_mask(i) != Act::Pass.bit() {
                    self.request(subject % 2, MANTLE_MOVE);
                    return;
                }
                self.f[i].bard_ranger.mantle_subject = None;
                self.f[i].bard_ranger.mantle_budget = 0;
            }
        }
    }

    pub(super) fn bard_ranger_step(&mut self, i: usize, phase: i32, choice: Act) {
        match phase {
            BEGUILING_TARGET if choice == Act::Pass => {
                self.f[i].bard_ranger.beguiling_targets = 0;
            }
            BEGUILING_TARGET => {
                self.f[i].bard_ranger.beguiling_target = choice as usize;
                self.request(i, BEGUILING_CONDITION);
            }
            BEGUILING_CONDITION => {
                let state = &mut self.f[i].bard_ranger;
                state.beguiling_targets = 0;
                if choice == Act::Pass {
                    return;
                }
                let subject = state.beguiling_target;
                if state.beguiling_used {
                    self.f[i].resource -= 1;
                }
                self.f[i].bard_ranger.beguiling_used = true;
                let dc = self.spell_dc(i);
                let saved = if subject < 2 {
                    self.save(subject, 4, dc)
                } else {
                    self.entity_save(subject, 4, dc)
                };
                if !saved {
                    let expires = self.turns + 20;
                    if choice == ACTS[0] {
                        self.f[i].bard_ranger.beguiling_charm[subject] = expires;
                    } else if !self.magic_heroism_immune(subject) {
                        self.f[i].bard_ranger.beguiling_fear[subject] = expires;
                    }
                }
            }
            MANTLE_TARGETS if choice == Act::Other7 => {
                self.bonus = false;
                self.f[i].resource -= 1;
                let amount = 2 * self.roll(if self.f[i].build.level >= 5 { 8 } else { 6 });
                let targets = self.f[i].bard_ranger.mantle_targets;
                for subject in (0..4).filter(|id| targets & (1 << id) != 0) {
                    self.grant_temporary_hp(subject, amount, TempSource::MantleOfInspiration);
                }
                self.next_mantle_move(i);
            }
            MANTLE_TARGETS if choice == Act::Pass => {
                self.f[i].bard_ranger.mantle_targets = 0;
            }
            MANTLE_TARGETS => {
                self.f[i].bard_ranger.mantle_targets |= 1 << choice as usize;
                self.request(i, MANTLE_TARGETS);
            }
            MANTLE_MOVE if choice == Act::Pass => self.next_mantle_move(self.mantle_source()),
            MANTLE_MOVE => {
                let actor = i;
                let i = self.mantle_source();
                let subject = self.f[i].bard_ranger.mantle_subject.unwrap();
                self.spend_mantle_reaction(i, subject);
                if choice == Act::Stand {
                    let cost = self
                        .mantle_stand_cost(subject)
                        .expect("can stand using Mantle");
                    self.f[i].bard_ranger.mantle_budget -= cost;
                    if subject < 2 {
                        self.f[subject].prone = false;
                    } else {
                        self.entity_mut(subject).unwrap().martial.prone = false;
                    }
                } else {
                    let (destination, cost, vertical) = self
                        .mantle_destination(i, choice)
                        .expect("legal Mantle destination");
                    if subject < 2 {
                        if !self.try_point_collision(subject, destination) {
                            self.f[i].bard_ranger.mantle_budget -= cost;
                            let before = self.fighter_point(subject);
                            self.commit_move_point(subject, destination);
                            self.traversal_safe_finish(
                                subject,
                                before,
                                self.fighter_point(subject),
                                vertical,
                            );
                        }
                    } else if self.entity_commit_reaction_move(subject, destination) {
                        self.f[i].bard_ranger.mantle_budget -= cost;
                    }
                }
                if self.mantle_move_mask(i) != Act::Pass.bit() {
                    self.request(actor, MANTLE_MOVE);
                } else {
                    self.next_mantle_move(i);
                }
            }
            _ => unreachable!("unassigned Bard decision"),
        }
    }

    pub(super) fn bard_ranger_label(&self, _i: usize, phase: i32, choice: Act) -> String {
        match (phase, choice) {
            (BEGUILING_TARGET, Act::Pass) => "Decline Beguiling Magic".into(),
            (BEGUILING_TARGET, _) => format!(
                "Beguiling Magic on {}",
                self.martial_target_label(choice as usize)
            ),
            (BEGUILING_CONDITION, Act::N) => "Charm (Wisdom save)".into(),
            (BEGUILING_CONDITION, Act::NE) => "Frighten (Wisdom save)".into(),
            (MANTLE_TARGETS, Act::Other7) => "Grant Mantle of Inspiration".into(),
            (MANTLE_TARGETS, Act::Pass) => "Cancel Mantle of Inspiration".into(),
            (MANTLE_TARGETS, _) => {
                format!("Include {}", self.martial_target_label(choice as usize))
            }
            (MANTLE_MOVE, Act::Pass) => "Finish Mantle movement".into(),
            (MANTLE_MOVE, Act::Stand) => "Stand using Mantle movement".into(),
            (MANTLE_MOVE, _) => {
                let bard = self.mantle_source();
                let Some((point, cost, _)) = self.mantle_destination(bard, choice) else {
                    return "Movement unavailable".into();
                };
                format!(
                    "Move to {}{} at {} ft ({} ft of movement)",
                    char::from(b'A' + point.cell.x as u8),
                    point.cell.y + 1,
                    point.altitude * 5,
                    cost * 5
                )
            }
            _ => "Decline".into(),
        }
    }

    pub(super) fn beguiling_end_turn(&mut self, target: usize) {
        for source in 0..2 {
            let state = self.f[source].bard_ranger;
            if state.beguiling_charm[target] <= self.turns
                && state.beguiling_fear[target] <= self.turns
            {
                continue;
            }
            let dc = self.spell_dc(source);
            let saved = if target < 2 {
                self.save(target, 4, dc)
            } else {
                self.entity_save(target, 4, dc)
            };
            if saved {
                self.f[source].bard_ranger.beguiling_charm[target] = 0;
                self.f[source].bard_ranger.beguiling_fear[target] = 0;
            }
        }
    }

    pub(super) fn bard_ranger_charmed(&self, subject: usize, source: usize) -> bool {
        source < 2 && subject < 4 && {
            let state = &self.f[source].bard_ranger;
            state.beguiling_charm[subject] > self.turns || state.person_charm[subject] > self.turns
        }
    }

    pub(super) fn bard_ranger_fear_source(&self, subject: usize) -> Option<usize> {
        if subject >= 4 || self.magic_heroism_immune(subject) {
            return None;
        }
        (0..2).find(|&source| self.f[source].bard_ranger.beguiling_fear[subject] > self.turns)
    }

    pub(super) fn mirror_image_intercepts(&mut self, hit: super::battle::Hit) -> bool {
        if hit.defender >= 2
            || hit.d20 == 0
            || !self.entity_visible(hit.attacker, hit.defender)
            || (hit.attacker < 2 && self.entity_shared_blindsight(hit.attacker, hit.defender))
            || self
                .entity(hit.attacker)
                .is_some_and(|e| e.kind.stats().blindsight > 0)
        {
            return false;
        }
        let state = self.f[hit.defender].bard_ranger;
        if state.mirror_until <= self.turns {
            return false;
        }
        let mut redirected = false;
        for _ in 0..state.mirrors {
            redirected |= self.roll(6) >= 3;
        }
        if redirected {
            self.f[hit.defender].bard_ranger.mirrors -= 1;
        }
        redirected
    }

    pub(super) fn dispel_bard_ranger_spells(&mut self, actor: usize, target: usize, tier: usize) {
        for source in 0..2 {
            let state = self.f[source].bard_ranger;
            if state.person_charm[target] > self.turns
                && self.dispel_succeeds(actor, state.person_charm_tier[target], tier)
            {
                self.f[source].bard_ranger.person_charm[target] = 0;
            }
        }
        if target < 2 {
            let state = self.f[target].bard_ranger;
            if state.mirror_until > self.turns
                && state.mirrors > 0
                && self.dispel_succeeds(actor, state.mirror_tier, tier)
            {
                self.f[target].bard_ranger.mirrors = 0;
                self.f[target].bard_ranger.mirror_until = 0;
            }
            if state.disguise_until > self.turns
                && self.dispel_succeeds(actor, state.disguise_tier, tier)
            {
                self.f[target].bard_ranger.disguise_until = 0;
            }
        }
    }

    pub(super) fn bard_ranger_subclass_features(&self, viewer: usize) -> [f32; 8] {
        let state = &self.f[viewer].bard_ranger;
        [
            state.dread_uses as f32 / 5.,
            self.gloom_speed_bonus(viewer) as f32 / 2.,
            if state.mirror_until > self.turns {
                state.mirrors as f32 / 3.
            } else {
                0.
            },
            u8::from(state.beguiling_used).into(),
            u8::from(self.valor_inspiration_available(viewer)).into(),
            u8::from(self.bard_ranger_fear_source(viewer).is_some()).into(),
            u8::from(self.bard_ranger_charmed(viewer, 1 - viewer)).into(),
            if self.rope_space(viewer).is_some() {
                1.
            } else if state.disguise_until > self.turns {
                0.5
            } else {
                state.mantle_budget as f32 / 12.
            },
        ]
    }

    pub(super) fn bard_ranger_subclass_policy(
        &self,
        actor: usize,
        mask: u128,
        _expert: bool,
    ) -> Option<Act> {
        let legal = |act: Act| (mask & act.bit() != 0).then_some(act);
        match self.phase_id() {
            2 | 180..=182 => legal(Act::Rider),
            BEGUILING_TARGET => legal(ACTS[1 - actor]).or_else(|| legal(ACTS[3 - actor])),
            BEGUILING_CONDITION => legal(ACTS[0]),
            MANTLE_TARGETS => legal(ACTS[actor + 2]).or_else(|| legal(Act::Other7)),
            _ => None,
        }
    }
}

use super::geometry::{Point, MAX_ELEVATION};

pub(super) const CLIMB_ROPE: Act = Act::OtherCommon5;
pub(super) const LEAVE_ROPE: Act = Act::OtherCommon6;
pub(super) const TOGGLE_ROPE: Act = Act::OtherCommon7;

#[derive(Clone, Copy, Debug)]
pub(super) struct RopePortal {
    base: Point,
    expires: i32,
    pulled_in: bool,
    tier: usize,
}

impl Battle {
    fn square_is_dark(&self, target: Point) -> bool {
        if self.hadar_dark_at(target) {
            return true;
        }
        if !self.map.has_ambient_darkness() {
            return false;
        }
        let reaches =
            |light: Point, range| light.dist(target) <= range && self.map.los_at(light, target);
        if (0..2).any(|source| {
            self.present_on_arena_plane(source)
                && (self
                    .light_source_point(source)
                    .is_some_and(|light| reaches(light, 8))
                    || self.divine_starlight(source) && reaches(self.fighter_point(source), 4))
        }) {
            return false;
        }
        let creature_glow = (0..4).any(|source| {
            self.present_on_arena_plane(source)
                && (self.projectile_glowing(source) || self.faerie_outlined(source))
                && self
                    .footprint(source)
                    .is_some_and(|body| body.squares().any(|point| reaches(point, 2)))
        });
        let object_glow = (4..super::objects::END_OBJECTS).any(|source| {
            self.projectile_glowing(source)
                && self
                    .object(source)
                    .is_some_and(|object| reaches(object.point, 2))
        });
        !creature_glow && !object_glow
    }

    pub(super) fn darkness_allows_sight(
        &self,
        observer: usize,
        subject: usize,
        origin: Point,
        target: Point,
    ) -> bool {
        if !self.square_is_dark(target) {
            return true;
        }
        let darkvision = if observer < 2 {
            // Duelists use the Dwarf origin's 120-foot Darkvision.
            24 + if is_gloom_stalker(self.f[observer].build) {
                12
            } else {
                0
            }
        } else {
            self.entity(observer)
                .map_or(0, |entity| entity.kind.stats().darkvision)
        };
        if darkvision <= 0 || darkvision < origin.dist(target) {
            return false;
        }
        subject >= 2
            || !is_gloom_stalker(self.f[subject].build)
            || self.projectile_glowing(subject)
            || self.faerie_outlined(subject)
            || (observer < 2 && self.arcane_sees_invisible(observer))
            || self
                .footprint(subject)
                .is_some_and(|body| body.squares().any(|point| !self.square_is_dark(point)))
    }

    pub(super) fn rope_space(&self, id: usize) -> Option<usize> {
        if id < 2 {
            self.f[id].bard_ranger.rope_inside
        } else {
            self.entity(id).and_then(|companion| companion.rope_inside)
        }
    }

    pub(super) fn rope_same_space(&self, one: usize, two: usize) -> bool {
        self.rope_space(one) == self.rope_space(two)
            && self.blink_ethereal(one) == self.blink_ethereal(two)
    }

    pub(super) fn present_on_arena_plane(&self, id: usize) -> bool {
        self.rope_space(id).is_none() && !self.blink_ethereal(id)
    }

    pub(super) fn rope_can_see(&self, viewer: usize, subject: usize) -> bool {
        self.rope_same_space(viewer, subject)
            || !self.present_on_arena_plane(viewer) && self.present_on_arena_plane(subject)
            || (viewer < 2
                && self.present_on_arena_plane(viewer)
                && self.blink_ethereal(subject)
                && self.arcane_sees_invisible(viewer))
    }

    pub(super) fn portal_viewpoint(&self, viewer: usize, subject: usize) -> Option<Point> {
        if self.present_on_arena_plane(subject) {
            self.rope_space(viewer)
                .and_then(|caster| self.rope_portal_point(caster))
        } else {
            None
        }
    }

    pub(super) fn rope_portal_point(&self, caster: usize) -> Option<Point> {
        let portal = self.f[caster].bard_ranger.rope?;
        (portal.expires > self.turns).then_some(Point {
            cell: portal.base.cell,
            altitude: portal.base.altitude + 1,
        })
    }

    pub(super) fn rope_spell_rank(&self, caster: usize) -> Option<usize> {
        self.f[caster].bard_ranger.rope.map(|portal| portal.tier)
    }

    pub(super) fn cast_rope_trick(&mut self, caster: usize, tier: usize) {
        self.end_rope_trick(caster);
        self.f[caster].bard_ranger.rope = Some(RopePortal {
            base: self.fighter_point(caster),
            expires: self.turns + 1200,
            pulled_in: false,
            tier,
        });
    }

    pub(super) fn rope_trick_castable(&self, caster: usize) -> bool {
        let base = self.fighter_point(caster);
        base.altitude < MAX_ELEVATION
            && self.map.supported(base)
            && self.map.can_occupy(Point {
                altitude: base.altitude + 1,
                ..base
            })
    }

    pub(super) fn expire_rope_portals(&mut self) {
        for caster in 0..2 {
            if self.f[caster]
                .bard_ranger
                .rope
                .is_some_and(|portal| portal.expires <= self.turns)
            {
                self.end_rope_trick(caster);
            }
        }
    }

    pub(super) fn end_rope_trick(&mut self, caster: usize) {
        if let Some(portal) = self.f[caster].bard_ranger.rope.take() {
            for id in 0..4 {
                if self.rope_space(id) == Some(caster) {
                    self.exit_rope_space(id, portal.base);
                }
            }
        }
    }

    fn rope_exit_position(&self, i: usize, base: Point) -> Option<Point> {
        std::iter::once(base)
            .chain(
                (0..8).filter_map(|direction| {
                    self.map.step_at(base, direction).map(|(point, _)| point)
                }),
            )
            .find(|&destination| {
                self.map.supported(destination)
                    && (0..4)
                        .filter(|&id| id != i && self.present_on_arena_plane(id))
                        .all(|id| {
                            self.footprint(id)
                                .is_none_or(|body| !body.squares().any(|cell| cell == destination))
                        })
            })
    }

    fn reachable_rope(&self, i: usize) -> Option<usize> {
        let prone_or_mounted = if i < 2 {
            self.f[i].prone || self.entity_mount_movement_left(i).is_some()
        } else {
            self.entity(i)?.martial.prone || self.entity_mount_movement_left(i - 2).is_some()
        };
        if self.combatant_size(i) > 2 || prone_or_mounted {
            return None;
        }
        let location = self.creature_point(i)?;
        (0..2).find(|&caster| {
            self.f[caster].bard_ranger.rope.is_some_and(|portal| {
                portal.expires > self.turns
                    && !portal.pulled_in
                    && location.altitude == portal.base.altitude
                    && location.dist(portal.base) <= 1
                    && self.map.los_at(location, portal.base)
            })
        })
    }

    pub(super) fn rope_action_mask(&self, i: usize) -> u128 {
        // Climbing five feet costs two cells of movement.
        let can_climb = !self.blink_ethereal(i)
            && !self.incap(i)
            && self.speed(i) > 0
            && self.movement_left() >= 2;
        if let Some(caster) = self.rope_space(i) {
            let Some(portal) = self.f[caster].bard_ranger.rope else {
                return Act::End.bit();
            };
            let mut mask = Act::End.bit();
            if can_climb && !portal.pulled_in && self.rope_exit_position(i, portal.base).is_some() {
                mask |= LEAVE_ROPE.bit();
            }
            if !self.incap(i) && self.f[i].gear.interaction {
                mask |= TOGGLE_ROPE.bit();
            }
            if self.action && !self.incap(i) {
                mask |= Act::Dash.bit() | Act::Dodge.bit();
            }
            if self.f[i].concentration.is_some() {
                mask |= Act::DropConcentration.bit();
            }
            mask
        } else if can_climb && self.reachable_rope(i).is_some() {
            CLIMB_ROPE.bit()
        } else {
            0
        }
    }

    fn exit_rope_space(&mut self, id: usize, base: Point) {
        let destination = self.rope_exit_position(id, base).unwrap_or(base);
        if id < 2 {
            self.f[id].bard_ranger.rope_inside = None;
            self.commit_forced_move(id, destination);
        } else if let Some(companion) = self.entity_mut(id) {
            companion.rope_inside = None;
            self.entity_force_move(id, destination);
        }
    }

    pub(super) fn companion_rope_actions(&self, id: usize) -> u128 {
        if self.entity_movement_left(id) < 2 {
            return 0;
        }
        if let Some(caster) = self.rope_space(id) {
            self.f[caster]
                .bard_ranger
                .rope
                .filter(|portal| {
                    !portal.pulled_in && self.rope_exit_position(id, portal.base).is_some()
                })
                .map_or(0, |_| LEAVE_ROPE.bit())
        } else if self.reachable_rope(id).is_some() {
            CLIMB_ROPE.bit()
        } else {
            0
        }
    }

    pub(super) fn companion_rope_step(&mut self, id: usize, action: Act) -> bool {
        if self.companion_rope_actions(id) & action.bit() == 0 {
            return false;
        }
        if action == CLIMB_ROPE {
            let caster = self.reachable_rope(id).expect("reachable rope");
            let base = self.f[caster].bard_ranger.rope.unwrap().base;
            let companion = self.entity_mut(id).expect("present companion");
            companion.rope_inside = Some(caster);
            companion.position = base.cell;
            companion.altitude = base.altitude;
        } else {
            let caster = self.rope_space(id).expect("Rope Trick occupant");
            let base = self.f[caster].bard_ranger.rope.unwrap().base;
            self.exit_rope_space(id, base);
        }
        self.entity_mut(id).unwrap().turn.moved += 2;
        true
    }

    pub(super) fn rope_action_step(&mut self, i: usize, action: Act) -> bool {
        if self.rope_action_mask(i) & action.bit() == 0 {
            return false;
        }
        match action {
            CLIMB_ROPE => {
                let caster = self.reachable_rope(i).expect("reachable rope");
                self.f[i].bard_ranger.rope_inside = Some(caster);
                let base = self.f[caster].bard_ranger.rope.unwrap().base;
                self.f[i].pos = base.cell;
                self.f[i].elevation = base.altitude;
                self.moved += 2;
                self.f[1 - i].grappled = None;
                self.armor_interrupt(i);
            }
            LEAVE_ROPE => {
                let caster = self.rope_space(i).expect("Rope Trick occupant");
                let base = self.f[caster].bard_ranger.rope.unwrap().base;
                let destination = self
                    .rope_exit_position(i, base)
                    .expect("legal Rope Trick exit");
                self.f[i].bard_ranger.rope_inside = None;
                self.commit_forced_move(i, destination);
                self.moved += 2;
            }
            TOGGLE_ROPE => {
                let caster = self.rope_space(i).expect("Rope Trick occupant");
                let portal = self.f[caster].bard_ranger.rope.as_mut().unwrap();
                portal.pulled_in = !portal.pulled_in;
                self.f[i].gear.interaction = false;
            }
            _ => return false,
        }
        true
    }

    pub(super) fn rope_action_label(&self, i: usize, action: Act) -> Option<&'static str> {
        (self.rope_action_mask(i) & action.bit() != 0).then_some(())?;
        match action {
            CLIMB_ROPE => Some("Climb into Rope Trick"),
            LEAVE_ROPE => Some("Climb out of Rope Trick"),
            TOGGLE_ROPE => {
                let caster = self.rope_space(i)?;
                if self.f[caster].bard_ranger.rope?.pulled_in {
                    Some("Lower the rope")
                } else {
                    Some("Pull the rope inside")
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::geometry::{Pos, DARK_LAYOUT};

    const VALOR: u8 = 4;
    const GLAMOUR: u8 = 5;
    const FEY_WANDERER: u8 = 22;
    const GLOOM_STALKER: u8 = 23;
    const ROGUE: u8 = 24;

    fn duel(seed: u64, roster_id: u8, layout: usize) -> Battle {
        let mut battle = Battle::new_roster_in(seed, [roster_id, ROGUE], layout);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle
    }

    #[test]
    fn fey_wanderer_prepares_misty_step_with_the_shared_teleport_menu() {
        let mut battle = duel(0, FEY_WANDERER, 0);
        assert_eq!(
            battle.prepared_spell(0, Act::Magic6),
            Some(Spell::CharmPerson)
        );
        battle.step(Act::Magic7).unwrap();
        let first = battle.fighter_point(0);
        battle.step(ACTS[1]).unwrap();
        battle.step(ACTS[0]).unwrap();
        if battle.phase_id() == 19 {
            battle.step(ACTS[0]).unwrap();
        }
        assert_ne!(first, battle.fighter_point(0));
        assert_eq!(battle.f[0].slots[1], 1);
    }

    #[test]
    fn gloom_stalker_is_invisible_in_darkness_and_visible_when_lit() {
        let mut battle = duel(1, GLOOM_STALKER, DARK_LAYOUT);
        assert!(battle.can_see(0, 1));
        assert!(!battle.can_see(1, 0));
        battle.f[1].build = crate::arena2::class::roster_build(6).unwrap();
        battle.owner = 1;
        let light = Act::Magic7;
        battle.step(light).unwrap();
        battle.step(ACTS[0]).unwrap();
        battle.step(ACTS[1]).unwrap();
        assert!(battle.can_see(1, 0));
    }

    #[test]
    fn gloom_speed_bonus_ends_after_each_gloom_stalkers_first_turn() {
        let mut battle = Battle::new_roster_in(6, [GLOOM_STALKER, GLOOM_STALKER], 0);
        let first = battle.owner;
        assert_eq!(battle.speed(first), 8);
        battle.step(Act::End).unwrap();
        assert_eq!(battle.speed(1 - first), 8);
        assert_eq!(battle.speed(first), 6);
        battle.step(Act::End).unwrap();
        assert_eq!(battle.speed(first), 6);
    }

    #[test]
    fn ranger_riders_use_their_own_subclass_and_gloom_spends_a_charge() {
        for (roster_id, phase) in [(FEY_WANDERER, 180), (GLOOM_STALKER, 181)] {
            let mut battle = (0..100)
                .find_map(|seed| {
                    let mut battle = duel(seed, roster_id, 0);
                    battle.step(Act::Melee).unwrap();
                    while matches!(battle.phase_id(), 2 | 3) {
                        battle.step(Act::Pass).unwrap();
                    }
                    (battle.phase_id() == phase).then_some(battle)
                })
                .unwrap();
            let before = battle.f[0].bard_ranger.dread_uses;
            let damage_without_rider = {
                let mut decline = battle.clone();
                decline.step(Act::Pass).unwrap();
                decline.f[1].hp
            };
            battle.step(Act::Rider).unwrap();
            assert!(battle.f[1].hp < damage_without_rider);
            assert_eq!(
                battle.f[0].bard_ranger.dread_uses,
                before - i32::from(roster_id == GLOOM_STALKER)
            );
        }
    }

    #[test]
    fn glamour_casting_mirror_image_offers_beguiling_magic() {
        let mut battle = duel(4, GLAMOUR, 0);
        battle.f[0].slots[2] = 0;
        battle.step(Act::Magic7).unwrap();
        assert_eq!(battle.f[0].bard_ranger.mirrors, 3);
        assert_eq!(battle.phase_id(), BEGUILING_TARGET);
        battle.step(ACTS[1]).unwrap();
        assert_eq!(battle.phase_id(), BEGUILING_CONDITION);
        let resource = battle.f[0].resource;
        battle.step(Act::N).unwrap();
        assert_eq!(battle.f[0].resource, resource);
        assert!(battle.f[0].bard_ranger.beguiling_used);
    }

    #[test]
    fn glamour_can_grant_mantle_but_cannot_target_itself() {
        let mut battle = duel(0, GLAMOUR, 0);
        let resource = battle.f[0].resource;
        battle.step(MANTLE).unwrap();
        assert_eq!(battle.phase_id(), MANTLE_TARGETS);
        assert_eq!(battle.raw_action_mask() & ACTS[0].bit(), 0);
        battle.step(ACTS[1]).unwrap();
        battle.step(Act::Other7).unwrap();
        assert!(battle.f[1].temp >= 2 && battle.f[1].temp <= 16);
        assert_eq!(battle.f[0].resource, resource - 1);
        assert_eq!(battle.actor(), Some(1));
        assert!(battle.f[1].reaction);
        battle.step(Act::Pass).unwrap();
        assert!(battle.f[1].reaction);
    }

    #[test]
    fn mantle_lets_a_familiar_fly_up_using_its_own_reaction() {
        use crate::arena2::entities::{Entity, EntityKind, FamiliarForm, Spirit};
        let mut battle = duel(0, GLAMOUR, 0);
        let companion = Entity::new(
            EntityKind::Familiar {
                spirit: Spirit::Fey,
                form: FamiliarForm::Owl,
            },
            0,
            Pos { x: 0, y: 1 },
            20,
        );
        battle.entities.test_install(companion);
        battle.step(MANTLE).unwrap();
        battle.step(ACTS[2]).unwrap();
        battle.step(Act::Other7).unwrap();
        let before = battle.f[0].bard_ranger.mantle_budget;
        battle.step(Act::OtherCommon1).unwrap();
        let familiar = battle.entity(2).unwrap();
        assert_eq!(familiar.altitude, 1);
        assert!(!familiar.reaction);
        assert_eq!(familiar.turn.moved, 0);
        assert_eq!(battle.f[0].bard_ranger.mantle_budget, before - 1);
        assert!(battle.f[0].reaction);
    }

    #[test]
    fn valor_combat_inspiration_is_available_to_the_recipient() {
        let mut battle = duel(0, VALOR, 0);
        battle.step(Act::Magic18).unwrap();
        battle.step(ACTS[1]).unwrap();
        assert!(battle.valor_inspiration_available(1));
        assert!(!battle.valor_inspiration_available(0));
    }

    #[test]
    fn valor_inspiration_can_be_spent_as_a_defense_reaction() {
        let mut battle = (0..100)
            .find_map(|seed| {
                let mut battle = duel(seed, VALOR, 0);
                battle.step(Act::Magic18).unwrap();
                battle.step(ACTS[1]).unwrap();
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == 2 && battle.mask() & Act::Rider.bit() != 0).then_some(battle)
            })
            .unwrap();
        battle.step(Act::Rider).unwrap();
        assert!(!battle.valor_inspiration_available(1));
        assert!(!battle.f[1].reaction);
    }

    #[test]
    fn valor_inspiration_can_be_spent_on_attack_damage() {
        let mut battle = (0..100)
            .find_map(|seed| {
                let mut battle = duel(seed, VALOR, 0);
                battle.step(Act::Magic18).unwrap();
                battle.step(ACTS[1]).unwrap();
                battle.step(Act::End).unwrap();
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == 182).then_some(battle)
            })
            .unwrap();
        let mut decline = battle.clone();
        decline.step(Act::Pass).unwrap();
        battle.step(Act::Rider).unwrap();
        assert!(!battle.valor_inspiration_available(1));
        assert!(battle.f[0].hp < decline.f[0].hp);
    }

    #[test]
    fn familiar_can_climb_into_and_leave_rope_trick() {
        use crate::arena2::entities::{Entity, EntityKind, EntityPhase, FamiliarForm, Spirit};
        let mut battle = duel(0, GLOOM_STALKER, 0);
        battle.step(Act::Magic7).unwrap();
        let mut companion = Entity::new(
            EntityKind::Familiar {
                spirit: Spirit::Fey,
                form: FamiliarForm::Owl,
            },
            0,
            Pos { x: 0, y: 1 },
            20,
        );
        companion.begin_turn();
        battle.entities.test_install(companion);
        battle.set_entity_phase(Some(EntityPhase::Turn(0)));
        battle.step(CLIMB_ROPE).unwrap();
        assert_eq!(battle.rope_space(2), Some(0));
        assert!(!battle.entity_visible(1, 2));
        battle.step(LEAVE_ROPE).unwrap();
        assert_eq!(battle.rope_space(2), None);
        assert_eq!(battle.entity(2).unwrap().turn.moved, 4);
    }

    #[test]
    fn disguise_self_can_be_recognized_with_investigation() {
        let mut battle = duel(0, GLOOM_STALKER, 0);
        battle.f[0].slots[1] = 0;
        battle.step(Act::Magic6).unwrap();
        battle.step(Act::End).unwrap();
        let disguise = battle
            .study_candidates(1)
            .into_iter()
            .find(|target| target.id == DISGUISE_STUDY_ID)
            .expect("visible disguise");
        battle.f[1].build.stats[3] = 30;
        assert!(battle
            .try_check(1, crate::arena2::common::Check::Study(disguise))
            .is_none());
        assert!(battle.knows_spell(1, disguise));
    }

    #[test]
    fn opponent_can_follow_and_fight_inside_rope_trick() {
        let mut battle = Battle::new_roster_in(0, [GLOOM_STALKER, 27], 0);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 1, y: 0 };
        battle.step(Act::Magic7).unwrap();
        if battle.phase_id() == 47 {
            battle.step(Act::Pass).unwrap();
        }
        battle.step(CLIMB_ROPE).unwrap();
        assert_eq!(battle.creature_targets(Act::Melee), 0);
        battle.step(Act::End).unwrap();
        assert_eq!(battle.creature_targets(Act::Melee), 0);
        battle.step(CLIMB_ROPE).unwrap();
        assert_eq!(battle.rope_space(1), Some(0));
        let mut melee = battle.clone();
        melee.step(Act::Melee).unwrap();
        assert!(!melee.action);
        let mut ranged = battle.clone();
        ranged.step(Act::End).unwrap();
        let arrows = ranged.f[0].arrows;
        ranged.f[0].gear.melee = super::super::equipment::Carried::Packed;
        ranged.f[0].gear.bow = super::super::equipment::Carried::Held;
        ranged.step(Act::Ranged).unwrap();
        assert_eq!(ranged.f[0].arrows, arrows - 1);
        assert_ne!(battle.mask() & Act::Spell0.bit(), 0);
        assert_eq!(battle.mask() & Act::E.bit(), 0);
        let initial_hp = battle.f[0].hp;
        let slots = battle.f[1].slots;
        battle.step(Act::Spell2).unwrap();
        for _ in 0..12 {
            if battle.phase_id() == 0 {
                break;
            }
            let action = (0..128)
                .find(|&index| battle.mask() & ACTS[index].bit() != 0)
                .expect("a legal spell resolution choice");
            // Choose the opposing occupant when aiming, or the first legal slot.
            let action = if battle.phase_id() == 80 {
                ACTS[1]
            } else {
                ACTS[action]
            };
            battle.step(action).unwrap();
        }
        assert_ne!(battle.f[1].slots, slots);
        assert!(battle.f[0].hp < initial_hp);
        battle.step(LEAVE_ROPE).unwrap();
        assert_eq!(battle.f[1].pos, Pos { x: 0, y: 0 });
        assert!(!battle.rope_same_space(1, 0));
        assert_eq!(battle.creature_targets(Act::Melee), 0);
    }

    #[test]
    fn rope_trick_uses_movement_and_separates_lines_of_effect() {
        let mut battle = duel(0, GLOOM_STALKER, 0);
        battle.step(Act::Magic7).unwrap();
        assert_eq!(battle.f[0].slots[1], 1);
        battle.step(CLIMB_ROPE).unwrap();
        assert_eq!(battle.rope_space(0), Some(0));
        assert!(battle.can_see(0, 1));
        assert!(!battle.can_see(1, 0));
        assert!(!battle.combatant_los(1, 0));
        assert_eq!(battle.moved, 2);
        battle.step(LEAVE_ROPE).unwrap();
        assert_eq!(battle.rope_space(0), None);
        assert!(battle.can_see(1, 0));
        assert_eq!(battle.moved, 4);
    }
}
