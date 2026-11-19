//! Explicit controller choices for source-driven spell movement.
use super::{
    battle::Battle,
    class::Class,
    entities::EntityKind,
    equipment::Carried,
    footprint::Footprint,
    geometry::{Map, Point, Pos, DIRECTIONS, HEIGHT, MAX_ELEVATION, WIDTH},
    magic::Concentration,
    objects::{ObjectKind, END_OBJECTS, FIRST_OBJECT},
    Act, ACTS,
};
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
};

const TRAVEL: i32 = 73;
const PROPOSAL: i32 = 74;
const PROPOSAL_ROW: i32 = 75;
const PROPOSAL_COLUMN: i32 = 76;
const PROPOSAL_OBJECT: i32 = 77;
const PROPOSAL_HEIGHT: i32 = 117;
const TRAVEL_JUMP: i32 = 121;
const ASCEND: usize = 126;
const DESCEND: usize = 127;
const FLOORS: usize = MAX_ELEVATION as usize + 1;
const CELLS: usize = WIDTH as usize * HEIGHT as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Compulsion {
    Approach,
    Flee,
    Grovel,
    Halt,
    Drop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Suggestion {
    MoveTo(Point),
    SetItemsAside,
    Dodge,
    Gear(Act),
    PickUp(usize),
    Prone,
    Stand,
    RefrainAttack,
    EndTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Fear,
    Whispers,
    Command,
    Suggestion,
}

#[derive(Clone, Copy, Debug)]
struct Travel {
    subject: usize,
    source: usize,
    purpose: Purpose,
    goal: Goal,
    remaining: i32,
    charged: i32,
    climbing: Option<bool>,
    jump_menu: Option<bool>,
    jumped: Option<(bool, bool)>,
}

#[derive(Clone, Copy, Debug)]
enum Goal {
    Away(Point),
    Toward { point: Point, reach: i32 },
    Finish,
    EndTurn,
}

impl Goal {
    fn progress(self, at: Point) -> i32 {
        match self {
            Self::Away(origin) => at.dist(origin),
            Self::Toward { point, reach } => -(at.dist(point) - reach).max(0),
            Self::Finish | Self::EndTurn => 0,
        }
    }

    fn complete(self, at: Point) -> bool {
        match self {
            Self::Finish | Self::EndTurn => true,
            Self::Toward { point, reach } => at.dist(point) <= reach,
            Self::Away(_) => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct SpellMovementState {
    travel: Vec<Travel>,
    suggestions: [Option<(usize, Suggestion)>; 4],
    entity_commands: [Option<(usize, Compulsion)>; 2],
    proposal_target: Option<usize>,
    proposal_row: Option<i8>,
    proposal_column: Option<i8>,
    refrain: [Option<(usize, i32)>; 4],
}

type Rank = (i32, i32, i32);

struct Routes {
    map: Map,
    blocked: [[bool; CELLS]; FLOORS],
    hazards: [[bool; CELLS]; FLOORS],
    perceived_reactors: [Option<Footprint>; 2],
    movement_extra: i32,
    allow_unsupported: bool,
    vertical: Option<(bool, bool, i32)>,
    goal: Goal,
    distances: Option<[[i32; CELLS]; FLOORS]>,
    safe_only: bool,
    prefer_safe: bool,
}

impl Routes {
    fn index(point: Pos) -> usize {
        point.y as usize * WIDTH as usize + point.x as usize
    }

    fn progress(&self, at: Point) -> i32 {
        self.distances.as_ref().map_or_else(
            || self.goal.progress(at),
            |distances| -distances[at.altitude as usize][Self::index(at.cell)],
        )
    }

    fn prepare_distances(&mut self, current: Point) {
        let Goal::Toward { point, reach } = self.goal else {
            return;
        };
        let mut distances = [[i32::MAX / 2; CELLS]; FLOORS];
        let mut queue = BinaryHeap::new();
        for altitude in (point.altitude - reach as i8).max(0)
            ..=(point.altitude + reach as i8).min(MAX_ELEVATION)
        {
            for y in
                (point.cell.y - reach as i8).max(0)..=(point.cell.y + reach as i8).min(HEIGHT - 1)
            {
                for x in (point.cell.x - reach as i8).max(0)
                    ..=(point.cell.x + reach as i8).min(WIDTH - 1)
                {
                    let destination = Point {
                        cell: Pos { x, y },
                        altitude,
                    };
                    let key = Self::index(destination.cell);
                    if self.map.can_occupy(destination)
                        && !self.blocked[altitude as usize][key]
                        && !(self.safe_only && self.hazards[altitude as usize][key])
                    {
                        distances[altitude as usize][key] = 0;
                        queue.push(Reverse((0, altitude, x, y)));
                    }
                }
            }
        }
        while let Some(Reverse((cost, altitude, x, y))) = queue.pop() {
            let at = Point {
                cell: Pos { x, y },
                altitude,
            };
            if cost != distances[altitude as usize][Self::index(at.cell)] {
                continue;
            }
            if at == current {
                break;
            }
            let predecessors = DIRECTIONS
                .into_iter()
                .map(|(dx, dy)| Point {
                    cell: Pos {
                        x: x - dx,
                        y: y - dy,
                    },
                    ..at
                })
                .chain([
                    Point {
                        altitude: altitude - 1,
                        ..at
                    },
                    Point {
                        altitude: altitude + 1,
                        ..at
                    },
                ]);
            for (direction, from) in predecessors.enumerate() {
                if !self.map.can_occupy(from)
                    || self.blocked[from.altitude as usize][Self::index(from.cell)]
                {
                    continue;
                }
                let Some((destination, step_cost)) = self.step(from, direction) else {
                    continue;
                };
                if destination != at || self.safe_only && self.hazard(from, at) {
                    continue;
                }
                let known = &mut distances[from.altitude as usize][Self::index(from.cell)];
                if cost + step_cost < *known {
                    *known = cost + step_cost;
                    queue.push(Reverse((*known, from.altitude, from.cell.x, from.cell.y)));
                }
            }
        }
        self.distances = Some(distances);
    }

    fn hazard(&self, from: Point, to: Point) -> bool {
        self.hazards[to.altitude as usize][Self::index(to.cell)]
            || self.perceived_reactors.iter().flatten().any(|reactor| {
                reactor.distance(Footprint::point(from)) <= 1
                    && reactor.distance(Footprint::point(to)) > 1
                    && reactor.squares().any(|foe| self.map.los_at(foe, from))
            })
    }

    fn compare_rank(&self, rank: Rank) -> Rank {
        if self.prefer_safe {
            (rank.1, rank.0, rank.2)
        } else {
            rank
        }
    }

    fn climbing_contact(&self, at: Point) -> bool {
        self.vertical.is_some_and(|(_, spider, _)| {
            if spider {
                self.map.touches_structure(at)
            } else {
                self.map.touches_wall(at)
            }
        })
    }

    fn step(&self, at: Point, direction: usize) -> Option<(Point, i32)> {
        let (next, cost) = if direction < DIRECTIONS.len() {
            let (next, terrain) = self.map.step_at(at, direction)?;
            if !self.allow_unsupported
                && !self.map.supported(next)
                && !(self.climbing_contact(at) && self.climbing_contact(next))
            {
                return None;
            }
            (next, terrain + self.movement_extra)
        } else {
            let (fly, _, climb_cost) = self.vertical?;
            let altitude = at.altitude + if direction == DIRECTIONS.len() { 1 } else { -1 };
            let next = Point { altitude, ..at };
            if !self.map.can_occupy(next) {
                return None;
            }
            if fly {
                (next, 1)
            } else if self.climbing_contact(at)
                && (self.climbing_contact(next) || self.map.supported(next))
            {
                (
                    next,
                    climb_cost + i32::from(next.altitude == 0 && self.map.tile(next.cell) == 2),
                )
            } else {
                return None;
            }
        };
        (!self.blocked[next.altitude as usize][Self::index(next.cell)]).then_some((next, cost))
    }

    fn best(&self, start: Point, through: Point, initial_cost: i32, budget: i32) -> Option<Rank> {
        if initial_cost > budget {
            return None;
        }
        let hazardous = self.hazard(start, through);
        if hazardous && self.safe_only {
            return None;
        }
        let mut queue = VecDeque::from([(through, initial_cost, hazardous)]);
        let mut costs = [[[i32::MAX; 2]; CELLS]; FLOORS];
        costs[through.altitude as usize][Self::index(through.cell)][usize::from(hazardous)] =
            initial_cost;
        let mut best = None;
        while let Some((at, spent, hazard)) = queue.pop_front() {
            if costs[at.altitude as usize][Self::index(at.cell)][usize::from(hazard)] != spent {
                continue;
            }
            let safety = i32::from(!hazard);
            let rank = if matches!(self.goal, Goal::Toward { .. }) {
                (self.progress(at), -spent, safety)
            } else {
                (self.progress(at), safety, -spent)
            };
            if (!self.prefer_safe || rank.0 > self.progress(start))
                && best.is_none_or(|current| self.compare_rank(rank) > self.compare_rank(current))
            {
                best = Some(rank);
            }
            if self.goal.complete(at) {
                continue;
            }
            for direction in 0..DIRECTIONS.len() + 2 * usize::from(self.vertical.is_some()) {
                let Some((next, cost)) = self.step(at, direction) else {
                    continue;
                };
                let cost = spent + cost;
                let hazard = hazard || self.hazard(at, next);
                let next_cost =
                    &mut costs[next.altitude as usize][Self::index(next.cell)][usize::from(hazard)];
                if cost <= budget && cost < *next_cost && !(hazard && self.safe_only) {
                    *next_cost = cost;
                    queue.push_back((next, cost, hazard));
                }
            }
        }
        best
    }

    fn reachable(&self, origin: Point) -> [[bool; CELLS]; FLOORS] {
        let mut reached = [[false; CELLS]; FLOORS];
        let mut queue = VecDeque::from([origin]);
        reached[origin.altitude as usize][Self::index(origin.cell)] = true;
        while let Some(at) = queue.pop_front() {
            for direction in 0..DIRECTIONS.len() + 2 * usize::from(self.vertical.is_some()) {
                let Some((next, _)) = self.step(at, direction) else {
                    continue;
                };
                let seen = &mut reached[next.altitude as usize][Self::index(next.cell)];
                if !*seen && !self.hazard(at, next) {
                    *seen = true;
                    queue.push_back(next);
                }
            }
        }
        reached[origin.altitude as usize][Self::index(origin.cell)] = false;
        reached
    }
}

impl Battle {
    fn visible_spell_hazards(&self, viewer: usize, subject: usize, altitude: i8) -> [bool; CELLS] {
        let mut hazards = [false; CELLS];
        if altitude == 0 {
            if let Some(pos) = self.caltrops(viewer) {
                hazards[Routes::index(pos)] = true;
            }
        }
        for object in (FIRST_OBJECT..END_OBJECTS).filter_map(|id| self.observed_object(viewer, id))
        {
            if object.point.altitude == altitude
                && (object.burning || object.kind == ObjectKind::Caltrops)
            {
                hazards[Routes::index(object.point.cell)] = true;
            }
        }
        if self.f.iter().any(|f| {
            matches!(
                f.concentration,
                Some(
                    Concentration::Moonbeam
                        | Concentration::SpiritGuardians
                        | Concentration::StinkingCloud
                )
            )
        }) {
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let cell = Pos { x, y };
                    if self.visible_movement_spell_hazard(viewer, subject, Point { cell, altitude })
                    {
                        hazards[Routes::index(cell)] = true;
                    }
                }
            }
        }
        hazards
    }

    pub(super) fn start_whispers_flee(&mut self, source: usize, target: usize) {
        let viewer = target % 2;
        let budget = if target < 2 {
            if !self.can_react(target) {
                return;
            }
            self.f[target].reaction = false;
            if self.entities.mounted[target] {
                self.entity_effective_speed(target + 2)
            } else {
                self.speed(target).max(self.traversal_fly_speed(target))
            }
        } else {
            let Some(budget) = self.entity_reaction_budget(target) else {
                return;
            };
            if !self.entity_spend_reaction(target) {
                return;
            }
            budget
        };
        if let Some(point) = self.observed_spell_source(viewer, source) {
            self.push_spell_travel(target, source, Purpose::Whispers, Goal::Away(point), budget);
        }
    }

    pub(super) fn start_compelled_turn(
        &mut self,
        source: usize,
        subject: usize,
        command: Compulsion,
    ) -> bool {
        if subject >= 2 || self.owner != subject || self.f[subject].hp <= 0 {
            return false;
        }
        let source_point = self.observed_spell_source(subject, source);
        let goal = match command {
            Compulsion::Grovel => {
                self.knock_prone(subject);
                Goal::Finish
            }
            Compulsion::Drop => {
                self.drop_held(subject);
                Goal::Finish
            }
            Compulsion::Halt => Goal::Finish,
            Compulsion::Approach => {
                source_point.map_or(Goal::Finish, |point| Goal::Toward { point, reach: 1 })
            }
            Compulsion::Flee => source_point.map_or(Goal::Finish, Goal::Away),
        };
        self.start_spell_travel(subject, source, Purpose::Command, goal);
        true
    }

    pub(super) fn issue_entity_command(
        &mut self,
        source: usize,
        target: usize,
        command: Compulsion,
    ) {
        if (2..4).contains(&target) && self.entity(target).is_some() {
            self.spell_movement.entity_commands[target - 2] = Some((source, command));
        }
    }

    pub(super) fn start_fear_travel(&mut self, subject: usize) -> bool {
        let Some(source) = self.fear_source(subject) else {
            return false;
        };
        let Some(origin) = self.fear_anchor(subject, source) else {
            return false;
        };
        let remaining = if subject < 2 {
            self.movement_left()
        } else {
            self.entity_movement_left(subject)
        };
        let movement_entity = self.travel_movement_entity(subject);
        let action_can_dash = if let Some(id) = movement_entity {
            self.entity(id).is_some_and(|e| e.turn.action) && !self.magic_area_poisoned(id)
        } else {
            self.action && !self.magic_quiet_turn(subject) && !self.incap(subject)
        };
        let extra = if action_can_dash {
            movement_entity.map_or_else(
                || self.speed(subject).max(self.traversal_fly_speed(subject)),
                |id| self.entity_effective_speed(id),
            )
        } else {
            0
        };
        let goal = Goal::Away(origin);
        let travel = Travel {
            subject,
            source,
            purpose: Purpose::Fear,
            goal,
            remaining,
            charged: 0,
            climbing: None,
            jump_menu: None,
            jumped: None,
        };
        if self.spell_travel_options(travel, remaining + extra).1 == 0 {
            return false;
        }
        if extra > 0 {
            if let Some(id) = movement_entity {
                if let Some(entity) = self.entity_mut(id) {
                    entity.turn.action = false;
                    entity.turn.dash += extra;
                }
            } else {
                self.action = false;
                self.dash += extra;
            }
        }
        self.push_spell_travel(subject, source, Purpose::Fear, goal, remaining + extra);
        true
    }

    pub(super) fn start_entity_compulsion(&mut self, id: usize) -> bool {
        if let Some((source, command)) = self.spell_movement.entity_commands[id - 2].take() {
            let source_point = self.observed_spell_source(id % 2, source);
            let goal = match command {
                Compulsion::Grovel => {
                    self.entity_knock_prone(id);
                    Goal::Finish
                }
                Compulsion::Approach => {
                    source_point.map_or(Goal::Finish, |point| Goal::Toward { point, reach: 1 })
                }
                Compulsion::Flee => source_point.map_or(Goal::Finish, Goal::Away),
                Compulsion::Drop | Compulsion::Halt => Goal::Finish,
            };
            self.start_spell_travel(id, source, Purpose::Command, goal);
            return true;
        }
        if self.start_fear_travel(id) {
            return true;
        }
        let Some((source, suggestion)) = self.spell_movement.suggestions[id] else {
            return false;
        };
        if self.f[source].concentration != Some(Concentration::Suggestion) {
            self.spell_movement.suggestions[id] = None;
            return false;
        }
        if Self::suggestion_needs_safe_stance(suggestion)
            && !self.suggestion_safe_stance(id % 2, id)
        {
            return false;
        }
        match suggestion {
            Suggestion::MoveTo(point) => {
                if self.creature_point(id) == Some(point) {
                    self.complete_suggestion(source, id);
                    return false;
                }
                self.start_spell_travel(
                    id,
                    source,
                    Purpose::Suggestion,
                    Goal::Toward { point, reach: 0 },
                );
                return true;
            }
            Suggestion::Dodge => {
                self.entity_compelled_dodge(id);
            }
            Suggestion::Prone => {
                if !self.entities.mounted[id % 2]
                    && self.entity(id).is_some_and(|entity| entity.altitude == 0)
                {
                    self.entity_knock_prone(id);
                }
            }
            Suggestion::Stand => {
                self.entity_compelled_stand(id);
            }
            Suggestion::RefrainAttack => {
                self.spell_movement.refrain[id] = Some((source, self.turns));
                return false;
            }
            Suggestion::EndTurn => {
                self.start_spell_travel(id, source, Purpose::Suggestion, Goal::EndTurn);
                return true;
            }
            Suggestion::PickUp(_) | Suggestion::Gear(_) | Suggestion::SetItemsAside => {}
        }
        self.complete_suggestion(source, id);
        false
    }

    pub(super) fn start_suggestion_cast(&mut self, source: usize, target: usize) {
        self.spell_movement.proposal_target = Some(target);
        self.spell_movement.proposal_row = None;
        self.spell_movement.proposal_column = None;
        self.request(source, PROPOSAL);
    }

    fn suggestion_subject(&self, source: usize) -> usize {
        self.spell_movement.proposal_target.unwrap_or(1 - source)
    }

    fn suggestion_destinations(&self, source: usize) -> [[bool; CELLS]; FLOORS] {
        let subject = self.suggestion_subject(source);
        let Some(origin) = self.observed_spell_source(source, subject) else {
            return [[false; CELLS]; FLOORS];
        };
        let travel = Travel {
            subject,
            source,
            purpose: Purpose::Suggestion,
            goal: Goal::Finish,
            remaining: 0,
            charged: 0,
            climbing: None,
            jump_menu: None,
            jumped: None,
        };
        self.spell_routes_for(travel, source, origin)
            .reachable(origin)
    }

    fn suggestion_column_mask(destinations: &[[bool; CELLS]; FLOORS], y: i8) -> u128 {
        (0..WIDTH)
            .filter(|&x| {
                destinations
                    .iter()
                    .any(|floor| floor[Routes::index(Pos { x, y })])
            })
            .fold(0, |mask, x| mask | 1 << x)
    }

    fn suggestion_altitude_mask(&self, source: usize, cell: Pos) -> u128 {
        self.suggestion_destinations(source)
            .into_iter()
            .enumerate()
            .filter(|(_, floor)| floor[Routes::index(cell)])
            .fold(0, |mask, (altitude, _)| mask | 1 << altitude)
    }

    fn suggestion_selected_cell(&self) -> Option<Pos> {
        Some(Pos {
            x: self.spell_movement.proposal_column?,
            y: self.spell_movement.proposal_row?,
        })
    }

    fn suggestion_recipe(&self, actor: usize, index: usize) -> Option<Suggestion> {
        let subject = self.suggestion_subject(actor);
        if subject >= 2 {
            let creature = self.observed_entity(actor, subject)?;
            if !matches!(creature.kind, EntityKind::Steed { .. }) {
                return None;
            }
            let hazards = self.visible_spell_hazards(actor, subject, creature.altitude);
            let safe = !hazards[Routes::index(creature.position)];
            return Some(match index {
                2 if safe => Suggestion::Dodge,
                7 if safe
                    && !creature.martial.prone
                    && creature.altitude == 0
                    && !self.entities.mounted[subject % 2] =>
                {
                    Suggestion::Prone
                }
                8 if creature.martial.prone => Suggestion::Stand,
                9 if safe => Suggestion::RefrainAttack,
                10 if safe => Suggestion::EndTurn,
                _ => return None,
            });
        }
        if !self.can_see(actor, subject) || self.incap(subject) {
            return None;
        }
        let fighter = &self.f[subject];
        let other_gear = &self.f[1 - subject].gear;
        let held = [fighter.gear.melee, fighter.gear.bow].contains(&Carried::Held)
            || [other_gear.melee, other_gear.bow].contains(&Carried::ForeignHeld)
            || self.gear_loose_shields(subject) != 0;
        let hazards =
            self.visible_spell_hazards(actor, subject, self.fighter_point(subject).altitude);
        let stay_legal = !hazards[Routes::index(fighter.pos)];
        let gear_mask = self.gear_mask(subject);
        Some(match index {
            1 if held => Suggestion::SetItemsAside,
            2 if stay_legal => Suggestion::Dodge,
            3 if gear_mask & Act::Gear2.bit() != 0 => Suggestion::Gear(Act::Gear2),
            4 if gear_mask & Act::Gear5.bit() != 0 => Suggestion::Gear(Act::Gear5),
            7 if stay_legal
                && !fighter.prone
                && self.fighter_point(subject).altitude == 0
                && !self.entities.mounted[subject] =>
            {
                Suggestion::Prone
            }
            8 if fighter.prone && self.speed(subject) > 0 => Suggestion::Stand,
            9 if stay_legal => Suggestion::RefrainAttack,
            10 if stay_legal => Suggestion::EndTurn,
            _ => return None,
        })
    }

    fn suggestion_needs_safe_stance(suggestion: Suggestion) -> bool {
        matches!(
            suggestion,
            Suggestion::Dodge | Suggestion::Prone | Suggestion::RefrainAttack | Suggestion::EndTurn
        )
    }

    fn suggestion_safe_stance(&self, viewer: usize, subject: usize) -> bool {
        self.creature_point(subject).is_some_and(|point| {
            !self.visible_spell_hazards(viewer, subject, point.altitude)[Routes::index(point.cell)]
        })
    }

    fn suggestion_pickup_mask(&self, viewer: usize, subject: usize) -> u128 {
        if subject >= 2 || !self.gear_free_hand(subject) || !self.can_see(viewer, subject) {
            return 0;
        }
        let at = self.fighter_point(subject);
        (FIRST_OBJECT..END_OBJECTS)
            .filter_map(|id| self.observed_object(viewer, id))
            .filter(|obj| {
                obj.point == at && !obj.burning && matches!(obj.kind, ObjectKind::Weapon(_))
            })
            .fold(0, |mask, obj| mask | 1 << obj.id)
    }

    fn suggestion_proposal_step(&mut self, actor: usize, phase: i32, choice: Act) {
        if choice == Act::Pass {
            self.spell_movement.proposal_target = None;
            self.spell_movement.proposal_row = None;
            self.spell_movement.proposal_column = None;
            self.magic_cancel_suggestion(actor);
            return;
        }
        match phase {
            PROPOSAL => {
                if choice == ACTS[0] {
                    self.request(actor, PROPOSAL_ROW);
                } else if choice == ACTS[5] {
                    self.request(actor, PROPOSAL_OBJECT);
                } else if let Some(suggestion) = self.suggestion_recipe(actor, choice as usize) {
                    self.finish_suggestion_cast(actor, suggestion);
                }
            }
            PROPOSAL_ROW => {
                self.spell_movement.proposal_row = Some(choice as i8);
                self.request(actor, PROPOSAL_COLUMN);
            }
            PROPOSAL_COLUMN => {
                self.spell_movement.proposal_column = Some(choice as i8);
                let cell = self.suggestion_selected_cell().expect("selected cell");
                let altitudes = self.suggestion_altitude_mask(actor, cell);
                if altitudes.count_ones() == 1 {
                    self.finish_suggestion_cast(
                        actor,
                        Suggestion::MoveTo(Point {
                            cell,
                            altitude: altitudes.trailing_zeros() as i8,
                        }),
                    );
                } else {
                    self.request(actor, PROPOSAL_HEIGHT);
                }
            }
            PROPOSAL_HEIGHT => {
                let cell = self.suggestion_selected_cell().expect("selected cell");
                self.finish_suggestion_cast(
                    actor,
                    Suggestion::MoveTo(Point {
                        cell,
                        altitude: choice as i8,
                    }),
                );
            }
            PROPOSAL_OBJECT => {
                self.finish_suggestion_cast(actor, Suggestion::PickUp(choice as usize))
            }
            _ => unreachable!("validated suggestion phase"),
        }
    }

    pub(super) fn spell_movement_row(&self, viewer: usize) -> Option<i8> {
        (self.actor() == Some(viewer)
            && matches!(self.phase_id(), PROPOSAL_COLUMN | PROPOSAL_HEIGHT))
        .then_some(self.spell_movement.proposal_row)
        .flatten()
    }

    pub(super) fn spell_movement_pending(&self, viewer: usize) -> String {
        if self.actor() != Some(viewer) {
            return String::new();
        }
        match self.phase_id() {
            TRAVEL_JUMP => self
                .spell_movement
                .travel
                .last()
                .map_or_else(String::new, |travel| {
                    format!(
                        "Choose a compelled {} jump",
                        if travel.jump_menu == Some(true) {
                            "high"
                        } else {
                            "long"
                        }
                    )
                }),
            TRAVEL => self
                .spell_movement
                .travel
                .last()
                .map_or_else(String::new, |travel| {
                    let purpose = match travel.purpose {
                        Purpose::Fear => "Fear",
                        Purpose::Whispers => "Dissonant Whispers",
                        Purpose::Command => "Command",
                        Purpose::Suggestion => "Suggestion",
                    };
                    let role = self
                        .entity(travel.subject)
                        .map_or_else(String::new, |entity| format!("{} · ", entity.kind.name()));
                    format!(
                        "{purpose}: {role}compelled movement, {} ft remaining",
                        5 * travel.remaining
                    )
                }),
            PROPOSAL => "Choose a safe Suggestion".into(),
            PROPOSAL_ROW => "Choose a destination row for Suggestion".into(),
            PROPOSAL_COLUMN => "Choose a destination cell for Suggestion".into(),
            PROPOSAL_OBJECT => "Choose a reachable unattended weapon for Suggestion".into(),
            PROPOSAL_HEIGHT => self
                .suggestion_selected_cell()
                .map_or_else(String::new, |cell| {
                    format!(
                        "Choose a height at {}{} · Suggestion",
                        char::from(b'A' + cell.x as u8),
                        cell.y + 1
                    )
                }),
            _ => String::new(),
        }
    }

    pub(super) fn spell_movement_features(&self, viewer: usize) -> [f32; 8] {
        let mut values = [0.0; 8];
        if self.actor() != Some(viewer) {
            return values;
        }
        match self.phase_id() {
            TRAVEL | TRAVEL_JUMP => {
                let Some(travel) = self.spell_movement.travel.last() else {
                    return values;
                };
                values[0] = if travel.subject < 2 { 1.0 } else { 1.5 };
                values[1] = match travel.purpose {
                    Purpose::Fear => 1.0,
                    Purpose::Whispers => 0.25,
                    Purpose::Command => 0.5,
                    Purpose::Suggestion => 0.75,
                };
                values[2] = travel.remaining as f32 / 24.0;
                let point = match travel.goal {
                    Goal::Away(point) => {
                        values[3] = -1.0;
                        Some(point)
                    }
                    Goal::Toward { point, reach } => {
                        values[3] = 1.0;
                        values[7] = reach as f32;
                        Some(point)
                    }
                    Goal::Finish | Goal::EndTurn => None,
                };
                let current_anchor_known = travel.purpose == Purpose::Suggestion
                    || self.observed_spell_source(viewer, travel.source).is_some();
                if let Some(point) = point.filter(|_| current_anchor_known) {
                    values[4] = f32::from(point.cell.x) / f32::from(WIDTH - 1);
                    values[5] = f32::from(point.cell.y) / f32::from(HEIGHT - 1);
                    values[6] =
                        f32::from(point.altitude) / f32::from(super::geometry::MAX_ELEVATION);
                }
                if let Some(high) = travel.jump_menu {
                    values[7] = if high { 3.0 } else { 2.0 };
                }
            }
            PROPOSAL..=PROPOSAL_OBJECT | PROPOSAL_HEIGHT => {
                values[0] = 2.0;
                if let Some(row) = self.spell_movement_row(viewer) {
                    values[5] = f32::from(row) / f32::from(HEIGHT - 1);
                }
                if self.phase_id() == PROPOSAL_HEIGHT {
                    if let Some(cell) = self.suggestion_selected_cell() {
                        values[4] = f32::from(cell.x) / f32::from(WIDTH - 1);
                    }
                }
            }
            _ => {}
        }
        values
    }

    pub(super) fn activate_suggestion(
        &mut self,
        source: usize,
        target: usize,
        suggestion: Suggestion,
    ) {
        if target < 4 {
            for item in &mut self.spell_movement.suggestions {
                if item.is_some_and(|(issuer, _)| issuer == source) {
                    *item = None;
                }
            }
            for effect in &mut self.spell_movement.refrain {
                if effect.is_some_and(|(issuer, _)| issuer == source) {
                    *effect = None;
                }
            }
            self.spell_movement.suggestions[target] = Some((source, suggestion));
        }
    }

    pub(super) fn suggestion_charmed_by(&self, subject: usize, source: usize) -> bool {
        self.arcane_indifferent(subject, source)
            || !self.arcane_emotions_immune(subject) && (self.bard_ranger_charmed(subject, source)
            || source < 2
            && self.f[source].concentration == Some(Concentration::Suggestion)
            && self
                .spell_movement
                .suggestions
                .get(subject)
                .is_some_and(|plan| plan.is_some_and(|(issuer, _)| issuer == source)))
    }

    pub(super) fn suggestion_hurt_from(&mut self, source: usize, target: usize, amount: i32) {
        if amount > 0 && source < 4
            && self.spell_movement.suggestions[target].is_some_and(|(issuer, _)| issuer == source % 2)
        {
            self.complete_suggestion(source % 2, target);
        }
    }

    pub(super) fn start_suggested_turn(&mut self, source: usize, target: usize) -> bool {
        let Some((issuer, suggestion)) = self.spell_movement.suggestions[target] else {
            return false;
        };
        if issuer != source
            || self.f[source].concentration != Some(Concentration::Suggestion)
            || self.incap(target)
            || self.arcane_emotions_immune(target)
        {
            return false;
        }
        if Self::suggestion_needs_safe_stance(suggestion)
            && !self.suggestion_safe_stance(target, target)
        {
            return false;
        }
        match suggestion {
            Suggestion::MoveTo(point) => {
                if point == self.fighter_point(target) {
                    self.complete_suggestion(source, target);
                } else {
                    self.start_spell_travel(
                        target,
                        source,
                        Purpose::Suggestion,
                        Goal::Toward { point, reach: 0 },
                    );
                }
            }
            Suggestion::SetItemsAside => {
                self.drop_held(target);
                self.complete_suggestion(source, target);
            }
            Suggestion::Dodge if self.action => {
                self.action = false;
                self.f[target].dodge = true;
                self.complete_suggestion(source, target);
            }
            Suggestion::Dodge => {}
            Suggestion::Gear(action) => {
                if self.gear_mask(target) & action.bit() != 0 {
                    self.gear_step(target, action);
                }
                self.complete_suggestion(source, target);
            }
            Suggestion::PickUp(id) => {
                if self.suggestion_pickup_mask(target, target) & (1 << id) != 0 {
                    let ranged = id >= 6;
                    if id % 2 == target {
                        let action = if ranged { Act::Gear4 } else { Act::Gear1 };
                        if self.gear_mask(target) & action.bit() != 0 {
                            self.gear_step(target, action);
                        }
                    } else {
                        let action = ACTS[if ranged { 4 } else { 0 }];
                        if self.gear_decision_mask(target, 71) & action.bit() != 0 {
                            self.gear_decision_step(target, 71, action);
                        }
                    }
                }
                self.complete_suggestion(source, target);
            }
            Suggestion::Prone => {
                if self.fighter_point(target).altitude == 0 && !self.entities.mounted[target] {
                    self.knock_prone(target);
                }
                self.complete_suggestion(source, target);
            }
            Suggestion::Stand => {
                let cost = (self.speed(target) + 1) / 2;
                if self.f[target].prone && cost > 0 && self.movement_left() >= cost {
                    self.moved += cost;
                    self.f[target].prone = false;
                    self.complete_suggestion(source, target);
                }
            }
            Suggestion::RefrainAttack => {
                self.spell_movement.refrain[target] = Some((source, self.turns));
            }
            Suggestion::EndTurn => {
                self.start_spell_travel(target, source, Purpose::Suggestion, Goal::EndTurn)
            }
        }
        true
    }

    pub(super) fn spell_filter_attack_actions(&self, actor: usize, mask: u128) -> u128 {
        if !self.spell_movement.refrain[actor].is_some_and(|(source, turn)| {
            turn == self.turns && self.f[source].concentration == Some(Concentration::Suggestion)
        }) {
            return mask;
        }
        use Act::*;
        let mut attack_actions = Melee.bit()
            | Ranged.bit()
            | Unarmed.bit()
            | Grapple.bit()
            | ShovePush.bit()
            | ShoveProne.bit()
            | Gear11.bit()
            | Other8.bit();
        if actor < 2 && self.f[actor].build.class == Class::Barbarian {
            attack_actions |= Special2.bit();
        }
        mask & !attack_actions
    }

    pub(super) fn spell_movement_turn_end(&mut self, actor: usize) {
        self.beguiling_end_turn(actor);
        if let Some((source, turn)) = self.spell_movement.refrain[actor].take() {
            if turn == self.turns {
                self.complete_suggestion(source, actor);
            }
        }
    }

    fn complete_suggestion(&mut self, source: usize, target: usize) {
        if !self.spell_movement.suggestions[target].is_some_and(|(issuer, _)| issuer == source) {
            return;
        }
        self.spell_movement.suggestions[target] = None;
        self.spell_movement.refrain[target] = None;
        if self.f[source].concentration == Some(Concentration::Suggestion) {
            self.f[source].concentration = None;
        }
    }

    fn observed_spell_source(&self, viewer: usize, source: usize) -> Option<Point> {
        if source >= 2 {
            self.observed_entity(viewer, source)
                .map(|entity| entity.point())
        } else {
            self.observed_pos(viewer, source).map(|cell| Point {
                cell,
                altitude: self.fighter_point(source).altitude,
            })
        }
    }

    fn start_spell_travel(&mut self, subject: usize, source: usize, purpose: Purpose, goal: Goal) {
        let remaining = if subject < 2 {
            self.movement_left()
        } else {
            self.entity_movement_left(subject)
        };
        self.push_spell_travel(subject, source, purpose, goal, remaining);
    }

    fn push_spell_travel(
        &mut self,
        subject: usize,
        source: usize,
        purpose: Purpose,
        goal: Goal,
        remaining: i32,
    ) {
        self.spell_movement.travel.push(Travel {
            subject,
            source,
            purpose,
            goal,
            remaining,
            charged: 0,
            climbing: None,
            jump_menu: None,
            jumped: None,
        });
        self.request(subject % 2, TRAVEL);
    }

    fn spell_routes(&self, travel: Travel, at: Point) -> Routes {
        self.spell_routes_for(travel, travel.subject % 2, at)
    }

    fn spell_routes_for(&self, travel: Travel, viewer: usize, at: Point) -> Routes {
        let subject = travel.subject;
        let owner = subject % 2;
        let foe = 1 - owner;
        let vertical = self.spell_vertical_profile(subject);
        let mut blocked = [[false; CELLS]; FLOORS];
        for id in 0..4 {
            if id == subject
                || (self.entities.mounted[owner]
                    && id == (if subject < 2 { owner + 2 } else { owner }))
            {
                continue;
            }
            if let Some(footprint) = self.observed_footprint(viewer, id) {
                for point in footprint.squares() {
                    blocked[point.altitude as usize][Routes::index(point.cell)] = true;
                }
            }
        }
        let hazards = if vertical.is_some() {
            std::array::from_fn(|floor| self.visible_spell_hazards(viewer, subject, floor as i8))
        } else {
            let mut hazards = [[false; CELLS]; FLOORS];
            hazards[at.altitude as usize] =
                self.visible_spell_hazards(viewer, subject, at.altitude);
            hazards
        };
        let bypasses_opportunity = if subject < 2 {
            viewer == subject && self.f[subject].hidden_dc.is_some()
        } else {
            self.entity(subject)
                .is_some_and(|entity| entity.kind.stats().flyby)
        };
        let mut perceived_reactors = [None; 2];
        if !bypasses_opportunity {
            perceived_reactors[0] = self.observed_footprint(viewer, foe);
            perceived_reactors[1] = self
                .observed_entity(viewer, foe + 2)
                .filter(|e| matches!(e.kind, EntityKind::Steed { .. }))
                .and_then(|_| self.observed_footprint(viewer, foe + 2));
        }
        Routes {
            map: self.map,
            blocked,
            hazards,
            vertical,
            perceived_reactors,
            movement_extra: if subject < 2 {
                i32::from(self.f[owner].prone)
                    + i32::from(self.f[foe].grappled.is_some() && self.can_see(viewer, foe))
            } else {
                i32::from(
                    self.entity(subject)
                        .is_some_and(|entity| entity.martial.prone),
                )
            },
            allow_unsupported: vertical.is_some_and(|(fly, _, _)| fly)
                || (subject < 2 && self.traversal_fly_speed(owner) > 0)
                || ((subject >= 2 || self.entities.mounted[owner])
                    && self
                        .entity(subject % 2 + 2)
                        .is_some_and(|mount| mount.kind.stats().fly_speed > 0)),
            goal: travel.goal,
            distances: None,
            safe_only: travel.purpose == Purpose::Suggestion,
            prefer_safe: travel.purpose == Purpose::Fear,
        }
    }

    fn spell_travel_direction(
        &self,
        subject: usize,
        direction: usize,
        budget: i32,
    ) -> Option<(Point, i32)> {
        if subject >= 2 {
            return self.entity_reaction_destination(subject, direction, budget);
        }
        if self.speed(subject) == 0 || self.incap(subject) {
            return None;
        }
        let (cell, cost) = self.movement_step(subject, direction)?;
        self.movement_available(subject, cell).then_some((
            Point {
                cell,
                altitude: self.fighter_point(subject).altitude,
            },
            cost,
        ))
    }

    fn spell_vertical_profile(&self, subject: usize) -> Option<(bool, bool, i32)> {
        if let Some(id) = self.travel_movement_entity(subject) {
            let entity = self.entity(id)?;
            let stats = entity.kind.stats();
            if self.entity_effective_speed(id) == 0 || entity.martial.prone {
                return None;
            }
            let fly = stats.fly_speed > 0 || self.magic_fly_speed(id) > 0;
            let spider = matches!(
                entity.kind,
                EntityKind::Familiar {
                    form: super::entities::FamiliarForm::Spider,
                    ..
                }
            );
            return (fly || stats.climb_speed > 0).then_some((fly, spider, 1));
        }
        if self.speed(subject) == 0 || self.f[subject].prone || self.incap(subject) {
            return None;
        }
        let fly = self.traversal_fly_speed(subject) > 0;
        let climb = self.magic_climb_speed(subject) > 0
            || self.f[subject].build.class == Class::Rogue && self.f[subject].build.level >= 3;
        Some((
            fly,
            self.shape_spider_climb(subject),
            if climb { 1 } else { 2 },
        ))
    }

    fn spell_travel_step(
        &self,
        travel: Travel,
        choice: usize,
        budget: i32,
    ) -> Option<(Point, i32, Option<bool>)> {
        if choice < DIRECTIONS.len() {
            return self
                .spell_travel_direction(travel.subject, choice, budget)
                .map(|(point, cost)| (point, cost, None));
        }
        if !matches!(choice, ASCEND | DESCEND) {
            return None;
        }
        let direction = if choice == ASCEND { 1 } else { -1 };
        let subject = travel.subject;
        if let Some(id) = self.travel_movement_entity(subject) {
            let point = self.entity_vertical_destination_with_budget(id % 2, direction, budget)?;
            return (budget > 0).then_some((point, 1, None));
        }
        let vertical = usize::from(direction < 0);
        let (point, cost, climb) = self
            .traversal_safe_vertical(subject, 10 + vertical, budget)
            .or_else(|| self.traversal_safe_vertical(subject, vertical, budget))?;
        Some((point, cost, Some(climb)))
    }

    fn spell_travel_jump(
        &self,
        travel: Travel,
        high: bool,
        choice: usize,
        budget: i32,
    ) -> Option<(Point, i32, bool)> {
        let subject = travel.subject;
        let (point, cost, assisted) = if let Some(id) = self.travel_movement_entity(subject) {
            (
                self.entity_spell_jump(id % 2, high, choice, budget)?,
                2,
                true,
            )
        } else {
            self.traversal_safe_jump(subject, high, choice, budget)?
        };
        let safe_stay = self.map.supported(point)
            || self
                .spell_vertical_profile(subject)
                .is_some_and(|(fly, spider, _)| {
                    fly || if spider {
                        self.map.touches_structure(point)
                    } else {
                        self.map.touches_wall(point)
                    }
                });
        (safe_stay || matches!(travel.purpose, Purpose::Command)).then_some((point, cost, assisted))
    }

    fn spell_travel_options(&self, travel: Travel, budget: i32) -> (Option<Rank>, u128, [u128; 2]) {
        let Some(at) = self.creature_point(travel.subject) else {
            return (None, 0, [0; 2]);
        };
        if budget <= 0 || travel.goal.complete(at) {
            return (None, 0, [0; 2]);
        }
        let mut routes = self.spell_routes(travel, at);
        routes.prepare_distances(at);
        let mut best = None;
        let mut mask = 0;
        let mut jumps = [0; 2];
        let mut rank_move = |next, cost| {
            let rank = routes.best(at, next, cost, budget)?;
            if rank.0 <= routes.progress(at) {
                return None;
            }
            if best.is_none_or(|old| routes.compare_rank(rank) > routes.compare_rank(old)) {
                best = Some(rank);
                Some(true)
            } else {
                (best == Some(rank)).then_some(false)
            }
        };
        for choice in (0..DIRECTIONS.len()).chain(
            [ASCEND, DESCEND]
                .into_iter()
                .filter(|_| routes.vertical.is_some()),
        ) {
            let Some((next, cost, _)) = self.spell_travel_step(travel, choice, budget) else {
                continue;
            };
            if let Some(improved) = rank_move(next, cost) {
                if improved {
                    mask = 0;
                }
                mask |= ACTS[choice].bit();
            }
        }
        for high in [false, true] {
            for choice in 0..if high { 12 } else { 96 } {
                let Some((next, cost, _)) = self.spell_travel_jump(travel, high, choice, budget)
                else {
                    continue;
                };
                if let Some(improved) = rank_move(next, cost) {
                    if improved {
                        mask = 0;
                        jumps = [0; 2];
                    }
                    jumps[usize::from(high)] |= 1 << choice;
                }
            }
        }
        if jumps[0] != 0 {
            mask |= Act::JumpLong.bit();
        }
        if jumps[1] != 0 {
            mask |= Act::JumpHigh.bit();
        }
        (best, mask, jumps)
    }

    fn travel_movement_entity(&self, subject: usize) -> Option<usize> {
        if subject >= 2 {
            Some(subject)
        } else if self.entities.mounted[subject] {
            Some(subject + 2)
        } else {
            None
        }
    }

    fn charge_travel(&mut self, travel: Travel, cost: i32) {
        if travel.purpose == Purpose::Whispers {
            return;
        }
        if let Some(id) = self.travel_movement_entity(travel.subject) {
            if let Some(entity) = self.entity_mut(id) {
                entity.turn.moved += cost;
            }
        } else {
            self.moved += cost;
        }
    }

    pub(super) fn spell_movement_mask(&self, actor: usize, phase: i32) -> u128 {
        if phase == PROPOSAL {
            let move_to = self
                .suggestion_destinations(actor)
                .into_iter()
                .flatten()
                .any(|reached| reached);
            let pickups = self.suggestion_pickup_mask(actor, self.suggestion_subject(actor));
            let pickup = if pickups != 0 { ACTS[5].bit() } else { 0 };
            return (1..=10)
                .filter(|&index| self.suggestion_recipe(actor, index).is_some())
                .fold(
                    u128::from(move_to) | pickup | Act::Pass.bit(),
                    |mask, index| mask | 1 << index,
                );
        }
        if phase == PROPOSAL_ROW {
            let destinations = self.suggestion_destinations(actor);
            return (0..HEIGHT)
                .filter(|&y| Self::suggestion_column_mask(&destinations, y) != 0)
                .fold(Act::Pass.bit(), |mask, y| mask | 1 << y);
        }
        if phase == PROPOSAL_COLUMN {
            return self.spell_movement.proposal_row.map_or(0, |y| {
                Self::suggestion_column_mask(&self.suggestion_destinations(actor), y)
            }) | Act::Pass.bit();
        }
        if phase == PROPOSAL_HEIGHT {
            return self
                .suggestion_selected_cell()
                .map_or(0, |cell| self.suggestion_altitude_mask(actor, cell))
                | Act::Pass.bit();
        }
        if phase == PROPOSAL_OBJECT {
            return self.suggestion_pickup_mask(actor, self.suggestion_subject(actor))
                | Act::Pass.bit();
        }
        let Some(&travel) = self
            .spell_movement
            .travel
            .last()
            .filter(|t| t.subject % 2 == actor)
        else {
            return Act::End.bit();
        };
        if travel.purpose == Purpose::Suggestion
            && !self.suggestion_charmed_by(travel.subject, travel.source)
        {
            return Act::End.bit();
        }
        if travel.purpose == Purpose::Fear
            && self.fear_source(travel.subject) != Some(travel.source)
        {
            return Act::End.bit();
        }
        let (normal, movement, jumps) = self.spell_travel_options(travel, travel.remaining);
        if phase == TRAVEL_JUMP {
            return jumps[usize::from(travel.jump_menu.expect("selected jump mode"))];
        }
        let dash_eligible = !matches!(travel.purpose, Purpose::Whispers | Purpose::Fear)
            && matches!(travel.goal, Goal::Away(_) | Goal::Toward { .. });
        let movement_entity = self.travel_movement_entity(travel.subject);
        let extra = movement_entity.map_or_else(
            || self.speed(actor).max(self.traversal_fly_speed(actor)),
            |id| self.entity_effective_speed(id),
        );
        if dash_eligible && extra > 0 {
            let (dashed, _, _) = self.spell_travel_options(travel, travel.remaining + extra);
            if dashed.is_some_and(|candidate| normal.is_none_or(|old| candidate.0 > old.0)) {
                let action = movement_entity.map_or(
                    self.action && !self.magic_quiet_turn(actor) && !self.incap(actor),
                    |id| {
                        self.entity(id).is_some_and(|entity| entity.turn.action)
                            && !self.magic_area_poisoned(id)
                    },
                );
                let mut dash = if action { Act::Dash.bit() } else { 0 };
                if movement_entity.is_none()
                    && self.bonus
                    && !self.magic_quiet_turn(actor)
                    && !self.incap(actor)
                {
                    dash |= match self.f[actor].build.class {
                        Class::Rogue if self.f[actor].build.level >= 2 => Act::Special2.bit(),
                        Class::Monk if self.f[actor].build.level >= 2 => Act::Special4.bit(),
                        _ => 0,
                    };
                }
                if dash != 0 {
                    return dash;
                }
            }
        }
        if movement == 0 {
            Act::End.bit()
        } else {
            movement
        }
    }

    pub(super) fn spell_movement_step(&mut self, actor: usize, phase: i32, choice: Act) {
        if phase == TRAVEL_JUMP {
            let travel = *self.spell_movement.travel.last().expect("compelled jump");
            let high = travel.jump_menu.expect("selected jump mode");
            let (point, cost, assisted) = self
                .spell_travel_jump(travel, high, choice as usize, travel.remaining)
                .expect("legal compelled jump");
            self.commit_compelled_step(travel, point, cost, None, Some((high, assisted)));
            return;
        }
        if phase != TRAVEL {
            self.suggestion_proposal_step(actor, phase, choice);
            return;
        }
        if choice == Act::End {
            if let Some(travel) = self.spell_movement.travel.pop() {
                match travel.purpose {
                    Purpose::Command if travel.subject >= 2 => {
                        self.entity_complete_compulsion(travel.subject)
                    }
                    Purpose::Command => self.end_turn(),
                    Purpose::Fear if travel.subject >= 2 => {
                        self.entity_resume_control(travel.subject)
                    }
                    Purpose::Fear | Purpose::Whispers => {}
                    Purpose::Suggestion => {
                        let was_active = self.suggestion_charmed_by(travel.subject, travel.source);
                        if self
                            .creature_point(travel.subject)
                            .is_some_and(|point| travel.goal.complete(point))
                        {
                            self.complete_suggestion(travel.source, travel.subject);
                        }
                        if travel.subject >= 2 {
                            if was_active && matches!(travel.goal, Goal::EndTurn) {
                                self.entity_complete_compulsion(travel.subject);
                            } else {
                                self.entity_resume_control(travel.subject);
                            }
                        } else if was_active && matches!(travel.goal, Goal::EndTurn) {
                            self.end_turn();
                        }
                    }
                }
            }
        } else if matches!(choice, Act::JumpLong | Act::JumpHigh) {
            self.spell_movement
                .travel
                .last_mut()
                .expect("compelled jump")
                .jump_menu = Some(choice == Act::JumpHigh);
            self.request(actor, TRAVEL_JUMP);
        } else if matches!(choice, Act::Dash | Act::Special2 | Act::Special4) {
            let subject = self
                .spell_movement
                .travel
                .last()
                .expect("compelled dash")
                .subject;
            let extra = if let Some(id) = self.travel_movement_entity(subject) {
                let speed = self.entity_effective_speed(id);
                if let Some(entity) = self.entity_mut(id) {
                    entity.turn.action = false;
                    entity.turn.dash += speed;
                }
                speed
            } else {
                if choice == Act::Dash {
                    self.action = false;
                } else {
                    self.bonus = false;
                }
                let speed = self.speed(actor).max(self.traversal_fly_speed(actor));
                self.dash += speed;
                speed
            };
            self.spell_movement
                .travel
                .last_mut()
                .expect("compelled dash")
                .remaining += extra;
            self.request(actor, TRAVEL);
        } else {
            let current = self
                .spell_movement
                .travel
                .last()
                .copied()
                .expect("compelled move");
            let Some((destination, cost, climbing)) =
                self.spell_travel_step(current, choice as usize, current.remaining)
            else {
                return;
            };
            self.commit_compelled_step(current, destination, cost, climbing, None);
        }
    }

    fn commit_compelled_step(
        &mut self,
        current: Travel,
        destination: Point,
        cost: i32,
        climbing: Option<bool>,
        jumped: Option<(bool, bool)>,
    ) {
        let travel = self
            .spell_movement
            .travel
            .last_mut()
            .expect("compelled move");
        travel.remaining -= cost;
        travel.charged = cost;
        travel.climbing = climbing;
        travel.jump_menu = None;
        travel.jumped = jumped;
        self.charge_travel(current, cost);
        self.queue_spell_reaction_move(current.subject, destination);
    }

    pub(super) fn spell_movement_after_step(&mut self, subject: usize, previous: Point) {
        let current = self.creature_point(subject);
        let Some(travel) = self
            .spell_movement
            .travel
            .last_mut()
            .filter(|t| t.subject == subject)
        else {
            return;
        };
        let attempted = *travel;
        if Some(previous) == current {
            travel.remaining += travel.charged;
        }
        travel.charged = 0;
        travel.climbing = None;
        travel.jumped = None;
        if Some(previous) == current {
            self.charge_travel(attempted, -attempted.charged);
        } else if let Some(point) = current {
            if let Some(id) = self.travel_movement_entity(subject) {
                if attempted.jumped.is_some() {
                    if let Some(entity) = self.entity_mut(id) {
                        entity.turn.used_spell_jump = true;
                    }
                }
            } else if let Some((high, assisted)) = attempted.jumped {
                self.traversal_safe_jump_finish(subject, previous, point, high, assisted);
            } else {
                self.traversal_safe_finish(subject, previous, point, attempted.climbing);
            }
        }
        if current.is_some() && (subject >= 2 || self.f[subject].hp > 0) {
            self.request(subject % 2, TRAVEL);
        } else {
            self.spell_movement.travel.pop();
        }
    }

    pub(super) fn spell_movement_label(
        &self,
        actor: usize,
        phase: i32,
        choice: Act,
    ) -> Option<String> {
        if phase == TRAVEL_JUMP {
            let travel = *self.spell_movement.travel.last()?;
            let (point, cost, assisted) = self.spell_travel_jump(
                travel,
                travel.jump_menu?,
                choice as usize,
                travel.remaining,
            )?;
            return Some(format!(
                "Compelled jump to {}{} at {} ft ({} ft movement{})",
                char::from(b'A' + point.cell.x as u8),
                point.cell.y + 1,
                5 * point.altitude,
                5 * cost,
                if assisted { "; Jump spell" } else { "" }
            ));
        }
        if phase != TRAVEL && choice == Act::Pass {
            return Some("Cancel Suggestion".into());
        }
        if phase == PROPOSAL_ROW {
            return Some(format!("Row {}", choice as usize + 1));
        }
        if phase == PROPOSAL_COLUMN {
            return self
                .spell_movement
                .proposal_row
                .map(|row| format!("Cell {}{}", char::from(b'A' + choice as u8), row + 1));
        }
        if phase == PROPOSAL_HEIGHT {
            return Some(format!("{} ft above the floor", choice as usize * 5));
        }
        if phase == PROPOSAL_OBJECT {
            return self
                .observed_object(actor, choice as usize)
                .map(|object| format!("Pick up {}", object.label()));
        }
        if phase == PROPOSAL {
            return Some(
                match choice as usize {
                    0 => "Suggest moving to a safe cell",
                    1 => "Suggest setting held items aside",
                    2 => "Suggest taking the Dodge action",
                    3 => "Suggest stowing the melee weapon",
                    4 => "Suggest stowing the bow",
                    5 => "Suggest picking up a weapon from the ground",
                    7 => "Suggest dropping prone",
                    8 => "Suggest standing up",
                    9 => "Suggest refraining from the Attack action this turn",
                    10 => "Suggest ending the turn",
                    _ => return None,
                }
                .into(),
            );
        }
        let direction = [
            "north",
            "north-east",
            "east",
            "south-east",
            "south",
            "south-west",
            "west",
            "north-west",
        ];
        if let Some(name) = direction.get(choice as usize) {
            let travel = self.spell_movement.travel.last()?;
            let (point, _) =
                self.spell_travel_direction(travel.subject, choice as usize, travel.remaining)?;
            return Some(format!(
                "Compelled move {name} to {}{}",
                char::from(b'A' + point.cell.x as u8),
                point.cell.y + 1
            ));
        }
        if matches!(choice as usize, ASCEND | DESCEND) {
            let travel = *self.spell_movement.travel.last()?;
            let (point, _, _) =
                self.spell_travel_step(travel, choice as usize, travel.remaining)?;
            return Some(format!(
                "Compelled move to elevation {} ft",
                point.altitude * 5
            ));
        }
        Some(
            match choice {
                Act::End => "Finish compelled movement",
                Act::JumpLong => "Compelled long jump",
                Act::JumpHigh => "Compelled high jump",
                Act::Dash => "Compelled Dash (action)",
                Act::Special2 | Act::Special4 => "Compelled Dash (bonus action)",
                _ => return None,
            }
            .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::entities::{Entity, FamiliarForm, Spirit};

    fn fixture(caster: Class, source: Pos, target: Pos) -> Battle {
        let mut battle = Battle::new(0, [caster, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = source;
        battle.f[1].pos = target;
        battle
    }

    #[test]
    fn whispers_consume_the_subject_reaction_and_offer_opportunity_to_the_source() {
        let mut battle = fixture(Class::Bard, Pos { x: 1, y: 0 }, Pos { x: 0, y: 0 });
        let start = battle.fighter_point(1);
        battle.start_whispers_flee(0, 1);
        assert_eq!((battle.phase_id(), battle.actor()), (TRAVEL, Some(1)));
        assert!(!battle.f[1].reaction);
        assert_eq!(battle.mask() & Act::Dash.bit(), 0);
        let mut opportunity = false;
        for _ in 0..16 {
            match battle.phase_id() {
                TRAVEL => {
                    let choice = ACTS
                        .into_iter()
                        .find(|a| battle.mask() & a.bit() != 0)
                        .unwrap();
                    battle.step(choice).unwrap();
                }
                1 => {
                    opportunity = true;
                    assert_eq!(battle.actor(), Some(0));
                    assert_ne!(battle.mask() & Act::Opportunity.bit(), 0);
                    battle.step(Act::Pass).unwrap();
                }
                _ => break,
            }
        }
        assert!(opportunity);
        assert_ne!(battle.fighter_point(1), start);
        assert_eq!((battle.phase_id(), battle.owner), (0, 0));
        assert!(battle.f[0].reaction);
    }

    #[test]
    fn whispers_move_the_companion_and_offer_opportunity_against_that_creature() {
        let mut battle = fixture(Class::Bard, Pos { x: 1, y: 0 }, Pos { x: 8, y: 3 });
        let cat = Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Cat,
                spirit: Spirit::Fey,
            },
            1,
            Pos { x: 0, y: 0 },
            2,
        );
        battle.fixture_companion(1, cat);
        let owner_hp = battle.f[1].hp;
        let start = battle.creature_point(3);
        battle.start_whispers_flee(0, 3);
        assert_eq!((battle.actor(), battle.phase_id()), (Some(1), TRAVEL));
        assert!(!battle.entity(3).unwrap().reaction);
        let mut attacked = false;
        for _ in 0..25 {
            let mask = battle.mask();
            let choice = match battle.phase_id() {
                0 => break,
                1 => {
                    assert_eq!(battle.actor(), Some(0));
                    attacked = true;
                    Act::Opportunity
                }
                _ if mask & Act::Pass.bit() != 0 => Act::Pass,
                _ => ACTS.into_iter().find(|a| mask & a.bit() != 0).unwrap(),
            };
            battle.step(choice).unwrap();
        }
        assert!(attacked);
        assert_eq!(battle.f[1].hp, owner_hp);
        assert_ne!(battle.creature_point(3), start);
        assert!(!battle.f[0].reaction);
    }

    #[test]
    fn command_approaches_reach_by_shortest_route_then_ends_subject_turn() {
        let mut battle = fixture(Class::Bard, Pos { x: 3, y: 1 }, Pos { x: 0, y: 1 });
        battle.owner = 1;
        assert!(battle.start_compelled_turn(0, 1, Compulsion::Approach));
        assert!(battle.step(Act::End).is_err());
        assert!(battle.step(Act::W).is_err());
        battle.step(Act::E).unwrap();
        battle.step(Act::E).unwrap();
        assert_eq!(battle.f[1].pos, Pos { x: 2, y: 1 });
        assert_eq!(battle.mask(), Act::End.bit());
        battle.step(Act::End).unwrap();
        assert_eq!(battle.owner, 0);
    }

    #[test]
    fn command_flee_uses_dash_when_it_can_extend_the_route() {
        let mut battle = fixture(Class::Bard, Pos { x: 0, y: 0 }, Pos { x: 2, y: 0 });
        battle.owner = 1;
        battle.moved = battle.speed(1) - 1;
        assert!(battle.start_compelled_turn(0, 1, Compulsion::Flee));
        assert_eq!(battle.mask(), Act::Dash.bit());
        battle.step(Act::Dash).unwrap();
        assert!(!battle.action);
        assert!(battle.spell_movement.travel.last().unwrap().remaining > 1);
    }

    #[test]
    fn proposal_can_be_cancelled_after_choosing_row_without_spending_a_slot() {
        let mut battle = fixture(Class::Warlock, Pos { x: 0, y: 1 }, Pos { x: 1, y: 1 });
        let slots = battle.f[0].slots;
        battle.start_suggestion_cast(0, 1);
        battle.step(Act::N).unwrap();
        assert_eq!(battle.phase_id(), PROPOSAL_ROW);
        let row = (0..HEIGHT)
            .find(|&row| battle.mask() & ACTS[row as usize].bit() != 0)
            .unwrap();
        battle.step(ACTS[row as usize]).unwrap();
        assert_eq!(battle.phase_id(), PROPOSAL_COLUMN);
        assert_eq!(battle.spell_movement_row(0), Some(row));
        assert_eq!(battle.spell_movement_row(1), None);
        assert_eq!(battle.spell_movement_features(1), [0.0; 8]);
        if row == 1 {
            assert_eq!(battle.mask() & (Act::N.bit() | Act::NE.bit()), 0);
        }
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.phase_id(), 0);
        assert_eq!(battle.f[0].slots, slots);
        assert!(battle.action);
    }

    #[test]
    fn suggestion_offers_reachable_elevation_and_cancellation_preserves_payment() {
        let mut battle = fixture(Class::Warlock, Pos { x: 0, y: 0 }, Pos { x: 8, y: 3 });
        battle.fixture_companion(
            1,
            Entity::new(
                EntityKind::Steed {
                    spirit: Spirit::Celestial,
                    spell_level: 4,
                },
                1,
                Pos { x: 3, y: 0 },
                3,
            ),
        );
        let slots = battle.f[0].slots;
        battle.start_suggestion_cast(0, 3);
        battle.step(Act::N).unwrap();
        let ((x, y), altitudes) = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .map(|(x, y)| ((x, y), battle.suggestion_altitude_mask(0, Pos { x, y })))
            .find(|(_, mask)| mask.count_ones() > 1)
            .unwrap();
        battle.step(ACTS[y as usize]).unwrap();
        battle.step(ACTS[x as usize]).unwrap();
        assert_eq!(battle.phase_id(), PROPOSAL_HEIGHT);
        assert_eq!(battle.mask(), altitudes | Act::Pass.bit());
        assert_eq!(battle.spell_movement_features(1), [0.; 8]);
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.f[0].slots, slots);
        assert!(battle.action);
    }

    #[test]
    fn suggestion_hands_control_back_after_reaching_the_selected_elevation() {
        let mut battle = fixture(Class::Warlock, Pos { x: 0, y: 0 }, Pos { x: 8, y: 3 });
        let cell = Pos { x: 4, y: 0 };
        battle.fixture_companion(
            1,
            Entity::new(
                EntityKind::Steed {
                    spirit: Spirit::Celestial,
                    spell_level: 4,
                },
                1,
                cell,
                3,
            ),
        );
        battle.f[0].concentration = Some(Concentration::Suggestion);
        let destination = Point { cell, altitude: 2 };
        battle.activate_suggestion(0, 3, Suggestion::MoveTo(destination));
        assert!(battle.start_entity_compulsion(3));
        for _ in 0..2 {
            assert_ne!(battle.mask() & ACTS[ASCEND].bit(), 0);
            battle.step(ACTS[ASCEND]).unwrap();
        }
        assert_eq!(battle.creature_point(3), Some(destination));
        assert_eq!(battle.mask(), Act::End.bit());
        battle.step(Act::End).unwrap();
        assert!(battle.f[0].concentration.is_none());
        assert_eq!(battle.active_entity_id(), Some(3));
    }

    #[test]
    fn whispers_can_ascend_off_turn_even_after_using_all_ordinary_movement() {
        let mut battle = fixture(Class::Bard, Pos { x: 4, y: 3 }, Pos { x: 8, y: 6 });
        let mut owl = Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Owl,
                spirit: Spirit::Fey,
            },
            1,
            Pos { x: 4, y: 3 },
            3,
        );
        owl.altitude = 4;
        owl.turn.moved = 40;
        battle.fixture_companion(1, owl);
        assert_eq!(battle.entity_movement_left(3), 0);
        battle.start_whispers_flee(0, 3);
        assert_eq!((battle.phase_id(), battle.actor()), (TRAVEL, Some(1)));
        assert_ne!(battle.mask() & ACTS[ASCEND].bit(), 0);
        battle.step(ACTS[ASCEND]).unwrap();
        assert_eq!(battle.creature_point(3).unwrap().altitude, 5);
        assert_eq!(battle.entity(3).unwrap().turn.moved, 40);
        assert!(!battle.entity(3).unwrap().reaction);
    }

    #[test]
    fn hostile_steed_can_decline_opportunity_and_resume_spell_travel() {
        let mut battle = fixture(Class::Bard, Pos { x: 7, y: 0 }, Pos { x: 0, y: 2 });
        battle.fixture_companion(
            0,
            Entity::new(
                EntityKind::Steed {
                    spirit: Spirit::Fey,
                    spell_level: 2,
                },
                0,
                Pos { x: 1, y: 0 },
                3,
            ),
        );
        let start = battle.fighter_point(1);
        let destination = Point::ground(Pos { x: 0, y: 3 });
        battle.push_spell_travel(
            1,
            0,
            Purpose::Whispers,
            Goal::Away(battle.fighter_point(0)),
            6,
        );
        battle.queue_spell_reaction_move(1, destination);
        assert_eq!((battle.phase_id(), battle.actor()), (63, Some(0)));
        assert_eq!(battle.fighter_point(1), start);
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.fighter_point(1), destination);
        assert_eq!((battle.phase_id(), battle.actor()), (TRAVEL, Some(1)));
        assert!(battle.entity(2).unwrap().reaction);
    }

    #[test]
    fn compelled_jump_spends_the_reaction_budget_without_charging_companion_turn() {
        let mut battle = fixture(Class::Bard, Pos { x: 0, y: 0 }, Pos { x: 8, y: 6 });
        let mut steed = Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Fey,
                spell_level: 2,
            },
            1,
            Pos { x: 1, y: 0 },
            3,
        );
        steed.jump_until = 100;
        battle.fixture_companion(1, steed);
        let start = battle.creature_point(3).unwrap();
        assert!(battle.entity_spend_reaction(3));
        battle.push_spell_travel(
            3,
            0,
            Purpose::Whispers,
            Goal::Away(battle.fighter_point(0)),
            2,
        );
        assert_ne!(battle.mask() & Act::JumpLong.bit(), 0);
        battle.step(Act::JumpLong).unwrap();
        assert_eq!(battle.phase_id(), TRAVEL_JUMP);
        let landing = ACTS[battle.mask().trailing_zeros() as usize];
        assert!(battle
            .spell_movement_label(1, TRAVEL_JUMP, landing)
            .unwrap()
            .contains("10 ft movement"));
        battle.step(landing).unwrap();
        if battle.phase_id() == 1 {
            battle.step(Act::Pass).unwrap();
        }
        assert!(battle.creature_point(3).unwrap().dist(start) >= 3);
        let companion = battle.entity(3).unwrap();
        assert_eq!(companion.turn.moved, 0);
        assert!(companion.turn.used_spell_jump);
        assert!(!companion.reaction);
        assert_eq!(battle.mask(), Act::End.bit());
    }

    #[test]
    fn command_drop_grovel_and_halt_apply_at_the_recipient_turn() {
        for command in [Compulsion::Drop, Compulsion::Grovel, Compulsion::Halt] {
            let mut battle = fixture(Class::Bard, Pos { x: 3, y: 0 }, Pos { x: 0, y: 0 });
            battle.owner = 1;
            let before = battle.fighter_point(1);
            assert!(battle.start_compelled_turn(0, 1, command));
            assert_eq!(battle.mask(), Act::End.bit());
            assert_eq!(battle.f[1].prone, command == Compulsion::Grovel);
            assert_eq!(
                battle.f[1].gear.melee == Carried::Ground(before.cell),
                command == Compulsion::Drop
            );
            assert!(battle.f[1].reaction);
            assert_eq!(battle.fighter_point(1), before);
            battle.step(Act::End).unwrap();
            assert_eq!(battle.owner, 0);
        }
    }

    #[test]
    fn suggestion_dodge_pays_an_action_and_refrain_only_filters_attack_actions() {
        let mut battle = fixture(Class::Warlock, Pos { x: 3, y: 0 }, Pos { x: 0, y: 0 });
        battle.owner = 1;
        battle.f[0].concentration = Some(Concentration::Suggestion);
        battle.activate_suggestion(0, 1, Suggestion::Dodge);
        assert!(battle.start_suggested_turn(0, 1));
        assert!(!battle.action);
        assert!(battle.f[1].dodge);
        assert!(battle.f[0].concentration.is_none());

        battle.action = true;
        battle.f[0].concentration = Some(Concentration::Suggestion);
        battle.activate_suggestion(0, 1, Suggestion::RefrainAttack);
        assert!(battle.start_suggested_turn(0, 1));
        let allowed = Act::Spell0.bit() | Act::Dash.bit() | Act::Ready.bit();
        let prohibited =
            Act::Melee.bit() | Act::Ranged.bit() | Act::Grapple.bit() | Act::Other8.bit();
        assert_eq!(
            battle.spell_filter_attack_actions(1, allowed | prohibited),
            allowed
        );
        battle.spell_movement_turn_end(1);
        assert!(battle.f[0].concentration.is_none());
    }

    #[test]
    fn suggestion_offers_only_executable_recipes_for_a_steed() {
        let mut battle = fixture(Class::Warlock, Pos { x: 0, y: 1 }, Pos { x: 8, y: 3 });
        let steed = Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            1,
            Pos { x: 1, y: 1 },
            2,
        );
        battle.fixture_companion(1, steed);
        let slots = battle.f[0].slots;
        battle.start_suggestion_cast(0, 3);
        let allowed = battle.mask();
        assert_ne!(allowed & ACTS[0].bit(), 0);
        assert_ne!(allowed & ACTS[2].bit(), 0);
        assert_ne!(allowed & ACTS[10].bit(), 0);
        assert_eq!(
            allowed & (ACTS[1].bit() | ACTS[3].bit() | ACTS[4].bit() | ACTS[5].bit()),
            0
        );
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.f[0].slots, slots);
    }

    #[test]
    fn suggestion_ends_on_damage_to_its_recipient_from_issuer_side_only() {
        let mut battle = fixture(Class::Warlock, Pos { x: 0, y: 1 }, Pos { x: 8, y: 3 });
        battle.f[0].concentration = Some(Concentration::Suggestion);
        battle.activate_suggestion(0, 3, Suggestion::Dodge);
        assert!(battle.suggestion_charmed_by(3, 0));
        assert!(!battle.suggestion_charmed_by(1, 0));
        battle.suggestion_hurt_from(0, 1, 4);
        battle.suggestion_hurt_from(3, 3, 4);
        battle.suggestion_hurt_from(0, 3, 0);
        assert!(battle.suggestion_charmed_by(3, 0));
        battle.suggestion_hurt_from(2, 3, 4);
        assert!(!battle.suggestion_charmed_by(3, 0));
        assert_eq!(battle.f[0].concentration, None);
    }
}
