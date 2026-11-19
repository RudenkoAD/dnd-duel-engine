use super::martial::fighter_rogue::*;
use super::class::{build, Build, Class};
use super::common::{
    Check, FailedCheck, Readied, ReadyTrigger, Skill, UnarmedManeuver, READY_TRIGGERS,
};
use super::entities::{Entities, EntityPhase};
use super::equipment::{DamageType, GearState, WeaponKind};
use super::geometry::{Map, Point, Pos};
pub use super::magic::Concentration;
use super::magic::MagicState;
use super::martial::{attacks_per_action, critical_threshold, MartialState};
use super::{Act, ACTS};
use crate::dice::Dice;

#[path = "sanctuary.rs"]
pub(super) mod sanctuary;
#[path = "target_ids_common.rs"]
mod target_ids_common;

#[path = "divine.rs"]
pub(super) mod divine;

#[path = "arcane_initiative.rs"]
mod arcane_initiative;

const TURN_LIMIT: i32 = 60;
const DECISION_LIMIT: i32 = 800;

#[derive(Clone, Debug)]
pub struct Fighter {
    pub build: Build,
    pub hp: i32,
    pub temp: i32,
    pub pos: Pos,
    pub slots: [i32; 3],
    pub resource: i32,
    pub secondary: i32,
    pub reaction: bool,
    pub arrows: i32,
    pub hidden_dc: Option<i32>,
    pub grappled: Option<i32>,
    pub dragging: bool,
    pub prone: bool,
    pub elevation: i8,
    pub gear: GearState,
    pub(super) armor: super::armor::ArmorState,
    pub(crate) origin: super::origins::OriginState,
    pub readied: Option<Readied>,
    pub(crate) help_entity_target: Option<usize>,
    pub concentration: Option<Concentration>,
    pub dodge: bool,
    pub shield: i32,
    pub sap: bool,
    pub slow: bool,
    pub stunned: bool,
    pub stun_slow: bool,
    pub stun_exposed: bool,
    pub attack_advantage: bool,
    pub attack_disadvantage: bool,
    pub speed_zero: bool,
    pub raging: bool,
    pub reckless: bool,
    pub addled: bool,
    pub slot_turn: i32,
    pub(super) martial: MartialState,
    pub(super) bard_ranger: super::bard_ranger::BardRangerState,
    pub(super) magic: MagicState,
    pub(super) divine: divine::DivineState,
    pub(super) arcane: super::magic_arcane::ArcaneState,
}

impl Fighter {
    fn new(build: Build, seat: usize) -> Self {
        Self {
            build,
            hp: build.hp,
            temp: 0,
            pos: Map::spawn(seat),
            slots: build.slots,
            resource: 0,
            secondary: 0,
            reaction: true,
            arrows: if build.ranged_die > 0 { 20 } else { 0 },
            hidden_dc: None,
            grappled: None,
            dragging: true,
            prone: false,
            elevation: 0,
            gear: GearState::new(build),
            armor: super::armor::ArmorState::new(build),
            origin: super::origins::OriginState::new(build.pb),
            readied: None,
            help_entity_target: None,
            concentration: None,
            dodge: false,
            shield: 0,
            sap: false,
            slow: false,
            stunned: false,
            stun_slow: false,
            stun_exposed: false,
            attack_advantage: false,
            attack_disadvantage: false,
            speed_zero: false,
            raging: false,
            reckless: false,
            addled: false,
            slot_turn: -1,
            martial: MartialState::default(),
            bard_ranger: super::bard_ranger::BardRangerState::new(build),
            magic: MagicState::default(),
            divine: divine::DivineState::default(),
            arcane: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Hit {
    pub attacker: usize,
    pub defender: usize,
    pub d20: i32,
    pub total: i32,
    pub ac: i32,
    pub ranged: bool,
    pub weapon: bool,
    pub physical: bool,
    pub damage_kind: DamageType,
    pub crit: bool,
    pub advantage: bool,
    pub disadvantage: bool,
    pub flurry: bool,
    pub unarmed: bool,
    pub thrown: bool,
    pub weapon_kind: Option<WeaponKind>,
    pub improvised: bool,
    pub strength: bool,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::arena2) struct Attack {
    die: i32,
    count: i32,
    bonus: i32,
    to_hit: i32,
    ranged: bool,
    weapon: bool,
    flurry: bool,
    unarmed: bool,
    thrown: bool,
    long_range: bool,
    improvised: bool,
    kind_override: Option<WeaponKind>,
    strength: bool,
    dexterity: bool,
}

#[derive(Clone, Copy, Debug)]
enum Defense {
    Pass,
    Magic,
    Martial,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    ValorDamage,
    Main,
    Opportunity,
    Defend {
        hit: Hit,
        attack: Attack,
    },
    Rider {
        hit: Hit,
        damage: i32,
        magic: bool,
    },
    Finished(Option<usize>),
    TargetRow(Act),
    TargetColumn(Act, i8),
    TargetAltitude(Act, Pos),
    CreatureTarget(Act),
    HelpTarget,
    FlurryTarget {
        actor: usize,
        count: i32,
    },
    CutDamage(usize),
    InspirationAttack(Hit, Attack),
    InspirationCheck(usize, FailedCheck),
    CutCheck {
        source: usize,
        result: FailedCheck,
        tactical: bool,
    },
    Decision {
        actor: usize,
        id: i32,
    },
    Entity(EntityPhase),
    ReadyTrigger,
    ReadyAction(ReadyTrigger),
    ReadyRelease(usize),
    ReadyGroundHeight {
        actor: usize,
        action: Act,
        cell: Pos,
    },
    ReadyTarget {
        actor: usize,
        action: Act,
    },
    ReadyMove {
        actor: usize,
        remaining: i32,
    },
    TacticalMind(usize, FailedCheck),
    ManeuverSave {
        source: usize,
        target: usize,
        maneuver: UnarmedManeuver,
        opportunity: bool,
        bonus_strike: bool,
    },
}

#[derive(Clone, Copy, Debug)]
enum ReadyContinuation {
    Phase(Phase),
    AfterAttack(usize),
}

#[derive(Clone, Debug)]
pub struct Battle {
    pub map: Map,
    pub f: [Fighter; 2],
    pub entities: Entities,
    pub(super) help_skill: [Option<Skill>; 2],
    companion_check_help: [Option<Skill>; 2],
    pub(super) footprint_squeezed: [bool; 4],
    pub(super) save_choice: super::save_choice::SaveChoice,
    arcane_initiative: Option<arcane_initiative::State>,
    pub(super) temporary_hp: super::temp_hp::TempHpState,
    pub(super) objects: super::objects::ObjectState,
    pub(super) areas: super::magic::AreaState,
    pub(super) spell_movement: super::spell_movement::SpellMovementState,
    pub(super) traversal: [super::traversal::TraversalState; 2],
    pub owner: usize,
    pub turns: i32,
    pub action: bool,
    pub bonus: bool,
    pub attacks: i32,
    pub moved: i32,
    pub dash: i32,
    pub disengaged: bool,
    pub tactical_steps: i32,
    pub(super) selected_target: Option<Pos>,
    selected_altitude: i8,
    dice: Dice,
    phase: Phase,
    decisions: i32,
    pending_move: Option<(Point, Point, i32, bool)>,
    bonus_attacks: Option<(usize, i32, bool, usize)>,
    pending_hit: Option<(Hit, i32, Defense)>,
    pending_reckless: Option<(usize, usize, Attack)>,
    pending_sanctuary: Option<sanctuary::WardPending>,
    pub(super) ward_preapproved_ray: Option<(usize, usize)>,
    pending_sacred: Option<(Hit, Attack)>,
    pending_elemental_object: Option<(usize, usize, i32, DamageType)>,
    pending_shield_hit: Option<(Hit, Attack)>,
    pending_precision: Option<(Hit, Attack)>,
    attack_resume: Option<EntityPhase>,
    attack_source: Option<usize>,
    pending_damage: Option<(Hit, i32, bool)>,
    ready_pending_move: Option<(usize, Point, Pos, i32, i32)>,
    ready_resume: Vec<ReadyContinuation>,
    ready_events: Vec<(usize, usize, ReadyTrigger)>,
    pending_turn_end: bool,
    pending_spell_move: Option<(usize, Point, Point)>,
    pending_spell_resume: Option<(usize, Point)>,
    area_resume: Option<Phase>,
}

impl Battle {
    pub(super) fn is_subclass(&self, seat: usize, class: Class, variant: u8) -> bool {
        self.f[seat].build.is_subclass(class, variant)
    }

    pub fn new(seed: u64, classes: [Class; 2], levels: [i32; 2]) -> Self {
        Self::new_with_surprise(seed, classes, levels, [false; 2])
    }

    /// Start a duel with a scenario's declared surprise for each seat.
    pub fn new_with_surprise(
        seed: u64,
        classes: [Class; 2],
        levels: [i32; 2],
        surprised: [bool; 2],
    ) -> Self {
        Self::with_builds(
            seed,
            std::array::from_fn(|i| build(classes[i], levels[i])),
            surprised,
            Map::new(seed),
        )
    }

    /// Start a level-five duel from two stable roster IDs in seat order.
    pub fn new_roster(seed: u64, builds: [u8; 2]) -> Self {
        let includes_gloom_stalker = builds.iter().copied().any(|id| {
            super::class::roster_build(id).is_some_and(super::bard_ranger::is_gloom_stalker)
        });
        let arena = if includes_gloom_stalker {
            Map::seeded_with_darkness(seed)
        } else {
            Map::new(seed)
        };
        Self::roster_on_map(seed, builds, arena)
    }

    /// Choose the arena without modifying the encounter's random seed.
    pub fn new_roster_in(seed: u64, builds: [u8; 2], layout: usize) -> Self {
        Self::roster_on_map(seed, builds, Map::from_layout(layout))
    }

    fn roster_on_map(seed: u64, builds: [u8; 2], map: Map) -> Self {
        Self::with_builds(
            seed,
            builds.map(|id| super::class::roster_build(id).expect("valid roster ID")),
            [false; 2],
            map,
        )
    }

    fn with_builds(seed: u64, builds: [Build; 2], surprised: [bool; 2], map: Map) -> Self {
        let mut dice = Dice(seed);
        let mut f = std::array::from_fn(|i| Fighter::new(builds[i], i));
        let mut initiative = |i: usize| {
            let build = f[i].build;
            let first = dice.roll(20);
            let advantage = build.level >= 3
                && (build.is_subclass(Class::Fighter, 0) || build.is_subclass(Class::Rogue, 1));
            let roll = match (advantage, surprised[i]) {
                (true, false) => first.max(dice.roll(20)),
                (false, true) => first.min(dice.roll(20)),
                _ => first,
            };
            roll + build.stats[1] + super::bard_ranger::gloom_initiative(build)
        };
        let deferred_initiative = Battle::arcane_initiative_required(builds);
        let [left, right] = if deferred_initiative { [0; 2] } else { [initiative(0), initiative(1)] };
        let owner = if deferred_initiative { 0 } else { match left.cmp(&right) {
            std::cmp::Ordering::Greater => 0,
            std::cmp::Ordering::Less => 1,
            std::cmp::Ordering::Equal => (dice.roll(2) - 1) as usize,
        } };
        f[owner].bard_ranger.first_turn = 1;
        f[1 - owner].bard_ranger.first_turn = 2;
        let mut battle = Self {
            map,
            f,
            entities: Entities::new([left, right]),
            help_skill: [None; 2],
            companion_check_help: [None; 2],
            footprint_squeezed: [false; 4],
            save_choice: super::save_choice::SaveChoice::default(),
            arcane_initiative: deferred_initiative.then(|| arcane_initiative::State::new(surprised)),
            temporary_hp: super::temp_hp::TempHpState::default(),
            objects: super::objects::ObjectState::default(),
            areas: super::magic::AreaState::default(),
            spell_movement: super::spell_movement::SpellMovementState::default(),
            traversal: [super::traversal::TraversalState::default(); 2],
            owner,
            turns: 1,
            action: true,
            bonus: true,
            attacks: 0,
            moved: 0,
            dash: 0,
            disengaged: false,
            tactical_steps: 0,
            selected_target: None,
            selected_altitude: 0,
            dice,
            phase: Phase::Main,
            decisions: 0,
            pending_move: None,
            bonus_attacks: None,
            pending_hit: None,
            pending_reckless: None,
            pending_sanctuary: None,
            ward_preapproved_ray: None,
            pending_sacred: None,
            pending_elemental_object: None,
            pending_shield_hit: None,
            pending_precision: None,
            attack_resume: None,
            attack_source: None,
            pending_damage: None,
            ready_pending_move: None,
            ready_resume: Vec::new(),
            ready_events: Vec::new(),
            pending_turn_end: false,
            pending_spell_move: None,
            pending_spell_resume: None,
            area_resume: None,
        };
        for i in 0..2 {
            battle.init_magic(i);
            battle.init_arcane(i);
            battle.init_martial(i);
        }
        if deferred_initiative {
            battle.advance_arcane_initiative();
        } else {
            battle.subclass_start_turn(owner);
        }
        battle
    }

    pub fn actor(&self) -> Option<usize> {
        if let Some(actor) = self.save_choice_actor() {
            return Some(actor);
        }
        match self.phase {
            Phase::Main
            | Phase::TargetRow(_)
            | Phase::TargetColumn(..)
            | Phase::TargetAltitude(..)
            | Phase::CreatureTarget(_)
            | Phase::HelpTarget => Some(self.owner),
            Phase::Opportunity => Some(
                self.pending_spell_move
                    .map(|(mover, ..)| 1 - mover % 2)
                    .unwrap_or_else(|| {
                        self.ready_pending_move
                            .map_or(1 - self.owner, |(mover, ..)| 1 - mover)
                    }),
            ),
            Phase::Defend { hit, .. } => Some(hit.defender % 2),
            Phase::ValorDamage => self.pending_hit.map(|(hit, _, _)| hit.attacker % 2),
            Phase::Rider { hit, .. } => Some(hit.attacker),
            Phase::Decision { actor, .. } | Phase::FlurryTarget { actor, .. } => Some(actor),
            Phase::Entity(phase) => Some(self.entity_phase_actor(phase)),
            Phase::ReadyTrigger | Phase::ReadyAction(_) => Some(self.owner),
            Phase::InspirationAttack(hit, _) => Some(hit.attacker % 2),
            Phase::InspirationCheck(actor, _)
            | Phase::TacticalMind(actor, _)
            | Phase::ReadyTarget { actor, .. }
            | Phase::ReadyGroundHeight { actor, .. } => Some(actor),
            Phase::ReadyRelease(actor)
            | Phase::ReadyMove { actor, .. }
            | Phase::CutDamage(actor) => Some(actor),
            Phase::ManeuverSave { target, .. } => Some(target % 2),
            Phase::CutCheck { source, .. } => Some(1 - source),
            Phase::Finished(_) => None,
        }
    }

    pub fn phase_id(&self) -> i32 {
        if self.save_choice.concentration_menu {
            return 118;
        }
        if let Some(phase) = self.replayed_choice_phase() {
            return phase;
        }
        match self.phase {
            Phase::Main => 0,
            Phase::Opportunity => 1,
            Phase::Defend { .. } => 2,
            Phase::ValorDamage => 182,
            Phase::Rider { .. } => 3,
            Phase::Finished(_) => 4,
            Phase::TargetRow(_) => 5,
            Phase::TargetColumn(..) => 6,
            Phase::TargetAltitude(..) => 19,
            Phase::CreatureTarget(_) | Phase::FlurryTarget { .. } | Phase::HelpTarget => 14,
            Phase::CutDamage(_) | Phase::CutCheck { .. } => 15,
            Phase::InspirationAttack(..) => 98,
            Phase::InspirationCheck(..) => 99,
            Phase::ReadyTrigger => 7,
            Phase::ReadyAction(_) => 8,
            Phase::ReadyRelease(_) => 9,
            Phase::ReadyTarget { .. } | Phase::ReadyGroundHeight { .. } => 106,
            Phase::ReadyMove { .. } => 10,
            Phase::TacticalMind(..) => 11,
            Phase::ManeuverSave { .. } => 12,
            Phase::Decision { id, .. } => id,
            Phase::Entity(phase) => phase.id(),
        }
    }

    pub(super) fn request(&mut self, actor: usize, id: i32) {
        self.phase = Phase::Decision { actor, id };
    }

    pub(super) fn set_entity_phase(&mut self, phase: Option<EntityPhase>) {
        self.phase = phase.map_or(Phase::Main, Phase::Entity);
    }

    pub(super) fn entity_phase(&self) -> Option<EntityPhase> {
        match self.phase {
            Phase::Entity(phase) => Some(phase),
            _ => None,
        }
    }

    pub fn fighter_point(&self, i: usize) -> Point {
        Point {
            cell: self.f[i].pos,
            altitude: self.f[i].elevation,
        }
    }

    pub fn selected_point(&self) -> Option<Point> {
        self.selected_target.map(|cell| Point {
            cell,
            altitude: self.selected_altitude,
        })
    }

    pub fn target_altitude_request(&self) -> Option<(Act, Pos)> {
        match self.phase {
            Phase::TargetAltitude(action, cell) => Some((action, cell)),
            _ => None,
        }
    }

    pub(super) fn begin_targeting(&mut self, action: Act) -> bool {
        if self.target_available(action) {
            self.phase = Phase::TargetRow(action);
            true
        } else {
            false
        }
    }

    fn resolve_target(&mut self, action: Act, point: Point) {
        self.phase = Phase::Main;
        self.selected_target = Some(point.cell);
        self.selected_altitude = point.altitude;
        if self.magic_continues_cast(self.owner, action) {
            self.magic_step(self.owner, action);
        } else {
            self.main_step(action);
        }
        self.selected_target = None;
        self.selected_altitude = 0;
    }

    /// Returns the ability being aimed and the chosen row, if any.
    pub fn target_request(&self) -> Option<(Act, Option<i8>)> {
        match self.phase {
            Phase::TargetRow(action) => Some((action, None)),
            Phase::TargetColumn(action, row) => Some((action, Some(row))),
            _ => None,
        }
    }

    /// Hit margin, critical, ranged, weapon, physical, visible damage,
    /// and the horizontal and vertical pending movement direction.
    /// While targeting, the last two features carry the normalized action and row.
    pub fn pending_features(&self) -> [f32; 8] {
        if let Some(features) = self.save_choice_features() {
            return features;
        }
        let mut values = [0.0; 8];
        if let Some((action, row)) = self.target_request() {
            values[6] = action as u8 as f32 / (ACTS.len() - 1) as f32;
            values[7] = row.map_or(-1.0, |y| {
                f32::from(y) / f32::from(super::geometry::HEIGHT - 1)
            });
            return values;
        }
        let hit = match self.phase {
            Phase::Defend { hit, .. } => Some(hit),
            Phase::Decision { id: PRECISION_ATTACK, .. } => self.pending_precision.map(|(hit, _)| hit),
            Phase::Rider { hit, damage, .. } => {
                values[5] = damage as f32 / 40.0;
                Some(hit)
            }
            _ => None,
        };
        if let Some(hit) = hit {
            values[0] = if hit.d20 == 0 {
                0.0
            } else {
                (hit.total - hit.ac) as f32 / 20.0
            };
            values[1] = u8::from(hit.crit).into();
            values[2] = u8::from(hit.ranged).into();
            values[3] = u8::from(hit.weapon).into();
            values[4] = u8::from(hit.physical).into();
        }
        if let Some((_, destination, ..)) = self.pending_move {
            let start = self.f[self.owner].pos;
            values[6] = (destination.cell.x - start.x).into();
            values[7] = (destination.cell.y - start.y).into();
        }
        values
    }

    pub fn common_decision_features(&self, viewer: usize) -> [f32; 24] {
        let mut out = [0.0; 24];
        let flag = |value| f32::from(u8::from(value));
        let me = &self.f[viewer];
        let foe = &self.f[1 - viewer];
        out[0] = flag(me.prone);
        out[1] = flag(me.grappled.is_some());
        out[2] = me.grappled.unwrap_or_default() as f32 / 30.0;
        out[3] = flag(me.hidden_dc.is_some());
        out[4] = me.hidden_dc.unwrap_or_default() as f32 / 30.0;
        out[5] = flag(self.can_see(viewer, 1 - viewer));
        if self.can_see(viewer, 1 - viewer) {
            out[6] = flag(foe.prone);
            out[7] = flag(foe.grappled.is_some());
            out[8] = f32::from(foe.elevation) / 6.0;
        }
        out[9] = f32::from(me.elevation) / 6.0;
        out[23] = flag(me.dragging && foe.grappled.is_some());
        if let Some(ready) = me.readied {
            out[10] = (1 + ready.trigger as u8) as f32 / READY_TRIGGERS.len() as f32;
            out[11] = (1 + ready.action as u8) as f32 / 128.0;
        }
        if self.actor() != Some(viewer) {
            return out;
        }
        match self.phase {
            Phase::ReadyAction(trigger) => {
                out[12] = (1 + trigger as u8) as f32 / READY_TRIGGERS.len() as f32
            }
            Phase::ReadyMove { remaining, .. } => out[13] = remaining as f32 / 12.0,
            Phase::TacticalMind(_, check) | Phase::InspirationCheck(_, check) => {
                out[14] = match check.check {
                    Check::Hide => 0.25,
                    Check::Search => 0.5,
                    Check::Escape(Skill::Athletics) => 0.75,
                    Check::Escape(_) => 1.0,
                    Check::Study(_) => -0.25,
                    Check::ExaminePhantasm => -0.75,
                    Check::Disarm(_) => -0.5,
                };
                out[15] = check.total as f32 / 30.0;
                if check.check != Check::Search {
                    out[16] = check.dc as f32 / 30.0;
                }
            }
            Phase::ManeuverSave { maneuver, .. } => {
                out[17] = (1 + maneuver as u8) as f32 / 3.0;
            }
            Phase::TargetAltitude(action, cell) => {
                out[18] = action as u8 as f32 / 127.0;
                out[19] = f32::from(cell.x) / 8.0;
                out[20] = f32::from(cell.y) / 6.0;
            }
            Phase::CreatureTarget(action) => out[21] = flag(action == Act::Ranged),
            Phase::FlurryTarget { count, .. } => out[21] = count as f32 / 2.0,
            Phase::HelpTarget => out[21] = -1.0,
            Phase::InspirationAttack(hit, _) => out[15] = (hit.total - hit.ac) as f32 / 20.0,
            Phase::CutCheck { result, .. } => out[14] = result.total as f32 / 30.0,
            Phase::CutDamage(_) => {
                out[22] = self
                    .pending_damage
                    .map_or(0.0, |(_, damage, _)| damage as f32 / 50.0);
            }
            _ => {}
        }
        out
    }

    pub fn pending_summary(&self) -> String {
        if let Some(summary) = self.arcane_initiative_summary() {
            return summary;
        }
        if self.save_choice.concentration_menu {
            return "Choose whether to end your concentration".into();
        }
        if let Some(summary) = self.save_choice_summary() {
            return summary;
        }
        if let Some((action, row)) = self.target_request() {
            let class = self.f[self.owner].build.class;
            let name = super::martial::label(class, action)
                .or_else(|| self.prepared_spell(self.owner, action).map(|spell| spell.label()))
                .or_else(|| super::magic::label(class, action))
                .unwrap_or("ability");
            return match row {
                None => format!("Choose a target row for {name}"),
                Some(y) => format!("Choose a target column in row {} for {name}", y + 1),
            };
        }
        match self.phase {
            Phase::TargetAltitude(action, cell) => format!(
                "Choose a height at {}{} for {}",
                char::from(b'A' + cell.x as u8),
                cell.y + 1,
                self.main_choice_label(self.owner, action),
            ),
            Phase::HelpTarget => "Choose the enemy to distract for your companion".into(),
            Phase::CreatureTarget(_) | Phase::FlurryTarget { .. } => {
                "Choose a creature to attack".into()
            }
            Phase::ReadyTrigger => "Choose a Ready trigger".into(),
            Phase::ReadyAction(trigger) => format!("Ready: {}", trigger.label()),
            Phase::ReadyRelease(actor) => {
                let action = self.f[actor].readied.expect("readied action").action;
                format!("Ready triggered: {}", self.main_choice_label(actor, action))
            }
            Phase::ReadyGroundHeight {
                actor,
                action,
                cell,
            } => format!(
                "Choose height for {} at {}{}",
                self.main_choice_label(actor, action),
                char::from(b'A' + cell.x as u8),
                cell.y + 1
            ),
            Phase::ReadyTarget { actor, action } => format!(
                "Choose a target for {}",
                self.main_choice_label(actor, action)
            ),
            Phase::ReadyMove { remaining, .. } => {
                format!("Readied movement: {} ft remaining", remaining * 5)
            }
            Phase::TacticalMind(_, check) | Phase::InspirationCheck(_, check) => {
                format!("Ability check failed (total {})", check.total)
            }
            Phase::InspirationAttack(hit, _) => {
                format!("Attack total {} missed AC {}", hit.total, hit.ac)
            }
            Phase::Decision {
                actor,
                id: 73..=77 | 117 | 121,
            } => self.spell_movement_pending(actor),
            Phase::Decision {
                id: sanctuary::RETARGET,
                ..
            } => "Sanctuary save failed. Choose a different target or lose the attack or spell."
                .into(),
            Phase::Decision { id: PRECISION_ATTACK, .. } => {
                let (hit, _) = self.pending_precision.expect("missed attack");
                format!("Attack total {} missed AC {}", hit.total, hit.ac)
            }
            Phase::Decision { id: PSIONIC_KNACK, actor } => {
                let check = self.f[actor].martial.fighter_rogue.knack.expect("failed check");
                format!("Ability check failed (total {})", check.total)
            }
            Phase::Decision { id: TRIP_ATTACK, .. } => "Spend one Superiority Die to try Trip Attack".into(),
            Phase::Decision { id: PSIONIC_STRIKE, .. } => "Spend one Psionic Energy Die for Psionic Strike".into(),
            Phase::Decision { id: PSYCHIC_BLADE_TARGET, .. } => "Choose the Psychic Blade target".into(),
            Phase::Decision { id: TELEKINETIC_SUBJECT, .. } => "Choose a creature or object to move".into(),
            Phase::Decision { id: TELEKINETIC_CONSENT, .. } => "Allow the Psi Warrior to move the selected creature?".into(),
            Phase::Decision { id: TELEKINETIC_CELL, .. } => "Choose the destination for Telekinetic Movement".into(),
            Phase::Decision { id: TELEKINETIC_HEIGHT, .. } => "Choose the height for Telekinetic Movement".into(),
            Phase::Decision { id: 141..=150, .. } => self.subclass_pending_label(self.phase_id()).into(),
            Phase::Decision { id: super::magic::PHANTASM_CELL, .. } =>
                "Choose the 10-foot cube for Phantasmal Force".into(),
            Phase::Decision { id: super::magic::PHANTASM_HEIGHT, .. } =>
                "Choose the height of the phantasm".into(),
            Phase::ManeuverSave { maneuver, .. } => format!("{maneuver:?}: choose a saving throw"),
            Phase::CutCheck { result, .. } => format!("Enemy ability check total {}", result.total),
            Phase::CutDamage(_) => format!(
                "Incoming damage: {}",
                self.pending_damage.map_or(0, |(_, n, _)| n)
            ),
            Phase::Opportunity => {
                let destination = self
                    .pending_spell_move
                    .map(|(_, _, point)| point.cell)
                    .or_else(|| {
                        self.ready_pending_move
                            .map(|(_, _, destination, ..)| destination)
                    })
                    .or_else(|| {
                        self.pending_move
                            .map(|(_, destination, ..)| destination.cell)
                    })
                    .expect("opportunity requires movement");
                format!(
                    "Opportunity attack before movement to ({}, {})",
                    destination.x, destination.y
                )
            }
            Phase::Defend { hit, .. } if hit.d20 == 0 => "Magic Missile incoming".into(),
            Phase::Defend { hit, .. } => format!(
                "{} (d20 {}, total {}, AC {}{})",
                if hit.crit { "Critical hit" } else { "Hit" },
                hit.d20,
                hit.total,
                hit.ac,
                if hit.disadvantage {
                    ", disadvantage"
                } else if hit.advantage {
                    ", advantage"
                } else {
                    ""
                },
            ),
            Phase::Rider { hit, damage, .. } => format!(
                "{} for {damage} damage before additional effects",
                match (hit.crit, hit.thrown) {
                    (true, true) => "Critical thrown hit",
                    (true, false) => "Critical hit",
                    (false, true) => "Thrown hit",
                    (false, false) => "Hit",
                },
            ),
            _ => String::new(),
        }
    }

    pub fn winner(&self) -> Option<usize> {
        match self.phase {
            Phase::Finished(winner) => winner,
            _ => None,
        }
    }

    pub fn finished(&self) -> bool {
        matches!(self.phase, Phase::Finished(_))
    }

    pub fn dist(&self) -> i32 {
        self.combatant_distance(0, 1)
            .expect("both player footprints")
    }

    pub(super) fn incap(&self, i: usize) -> bool {
        self.f[i].stunned || self.magic_incap(i) || self.magic_paralyzed(i)
    }

    pub(super) fn speed(&self, i: usize) -> i32 {
        let f = &self.f[i];
        if f.speed_zero
            || f.grappled.is_some()
            || self.magic_area_speed_zero(i)
            || self.magic_restrained(i)
            || self.magic_paralyzed(i)
        {
            return 0;
        }
        let mut speed = self.magic_speed(i) + self.gloom_speed_bonus(i) - self.armor_speed_penalty(i);
        if f.build.class == Class::Monk && f.build.level >= 2 && !self.martial_unarmored_active(i) {
            speed -= 2;
        }
        if f.slow {
            speed -= 2;
        }
        speed -= self.frost_slow(i);
        if f.stun_slow {
            speed /= 2;
        }
        speed.max(0)
    }

    pub fn movement_left(&self) -> i32 {
        if let Some(movement) = self.entity_mount_movement_left(self.owner) {
            return movement;
        }
        let normal = self.normal_movement_left();
        if self.speed(self.owner) == 0 {
            return normal;
        }
        normal.max(self.tactical_steps)
    }

    fn normal_movement_left(&self) -> i32 {
        let speed = self
            .speed(self.owner)
            .max(self.traversal_fly_speed(self.owner));
        if speed == 0 {
            return 0;
        }
        (speed + self.dash - self.moved).max(0)
    }

    pub(super) fn can_react(&self, i: usize) -> bool {
        self.f[i].reaction && !self.incap(i)
    }

    pub(super) fn extra_attack(&self, i: usize) -> i32 {
        attacks_per_action(self.f[i].build).max(self.magic_extra_attack(i))
    }

    pub(super) fn raw_action_mask(&self) -> u128 {
        if self.save_choice.concentration_menu {
            return Act::DropConcentration.bit() | Act::Pass.bit();
        }
        let mut mask = self.unmodified_mask();
        if self.controlled_concentrator().is_some() {
            if mask & Act::DropConcentration.bit() == 0 {
                mask |= Act::DropConcentration.bit();
            } else if let Some(action) = self.concentration_menu_action_for(mask) {
                mask |= action.bit();
            }
        }
        mask
    }

    fn unmodified_mask(&self) -> u128 {
        self.save_choice_mask().unwrap_or_else(|| self.phase_mask())
    }

    fn controlled_concentrator(&self) -> Option<usize> {
        if self.save_choice_actor().is_some() {
            return self.replay_concentrator();
        }
        let actor = self.actor()?;
        self.f[actor].concentration.is_some().then_some(actor)
    }

    pub(super) fn optional_concentrator(&self) -> Option<usize> {
        let actor = self.controlled_concentrator()?;
        (self.unmodified_mask() & Act::DropConcentration.bit() == 0
            || (matches!(self.phase, Phase::Main) && self.save_choice_actor().is_none()))
        .then_some(actor)
    }

    pub(super) fn concentration_menu_action(&self) -> Option<Act> {
        self.controlled_concentrator()?;
        self.concentration_menu_action_for(self.unmodified_mask())
    }

    fn concentration_menu_action_for(&self, mask: u128) -> Option<Act> {
        if matches!(self.phase, Phase::Main) && self.save_choice_actor().is_none() {
            return None;
        }
        if mask & Act::DropConcentration.bit() == 0 {
            return None;
        }
        ACTS.iter()
            .rev()
            .copied()
            .find(|action| mask & action.bit() == 0)
    }

    pub(super) fn drop_at_control_window(&mut self) {
        let actor = self.actor().expect("active concentration controller");
        self.drop_concentration(actor);
        if !self.finished() && self.phase_mask() == 0 {
            self.phase = Phase::Main;
        }
        if matches!(self.phase, Phase::Main) {
            self.resume_ready_continuation();
            self.resume_queued_movement();
        }
        self.settle_decision();
        self.save_choice_complete_decisions(1);
    }

    fn phase_mask(&self) -> u128 {
        let i = self.owner;
        match self.phase {
            Phase::Finished(_) => return 0,
            Phase::Opportunity => {
                let actor = self.actor().expect("opportunity reactor");
                let target = self.opportunity_target(actor);
                if self.suggestion_charmed_by(actor, target) {
                    return Act::Pass.bit();
                }
                let mut mask = Act::Opportunity.bit() | Act::Pass.bit();
                if self.rogue_blade_available(actor) { mask |= Act::Special3.bit(); }
                if self.combatant_size(target) <= self.combatant_size(actor) + 1 {
                    mask |= Act::ShovePush.bit() | Act::ShoveProne.bit();
                }
                if self.gear_wielded(actor, false) {
                    mask |= Act::Unarmed.bit();
                }
                let grappled = if target < 2 {
                    self.f[target].grappled.is_some()
                } else {
                    self.entity(target)
                        .is_some_and(|creature| creature.martial.grappled.is_some())
                };
                if self.gear_free_hand(actor)
                    && !grappled
                    && self.combatant_size(target) <= self.combatant_size(actor) + 1
                {
                    mask |= Act::Grapple.bit();
                }
                return mask;
            }
            Phase::ValorDamage => return Act::Rider.bit() | Act::Pass.bit(),
            Phase::Defend { hit, .. } => {
                let defender = hit.defender;
                let mut mask = Act::Pass.bit();
                if defender < 2 && (self.magic_defend(defender, hit) || self.martial_defend(defender, hit)) {
                    mask |= Act::Defend.bit();
                }
                if self.valor_can_defend(hit) {
                    mask |= Act::Rider.bit();
                }
                return mask;
            }
            Phase::Rider { hit, .. } => {
                let mask = Act::Rider.bit() | Act::Pass.bit();
                return if self.magic_can_repel(hit) {
                    mask | Act::N.bit()
                } else {
                    mask
                };
            }
            Phase::TargetRow(action) => return self.target_mask(action, None),
            Phase::TargetColumn(action, row) => return self.target_mask(action, Some(row)),
            Phase::TargetAltitude(action, cell) => return self.target_altitude_mask(action, cell),
            Phase::CreatureTarget(action) => {
                let mut mask = self.creature_targets(action);
                if self.target_available(action) {
                    mask |= Act::Search.bit();
                }
                return mask;
            }
            Phase::HelpTarget => return self.primary_help_targets(i),
            Phase::FlurryTarget { actor, .. } => return self.attack_targets(actor, Act::Unarmed),
            Phase::CutDamage(_) | Phase::CutCheck { .. } => {
                return Act::Defend.bit() | Act::Pass.bit()
            }
            Phase::Decision { actor, id } => return self.decision_mask(actor, id),
            Phase::Entity(phase) => return self.entity_phase_mask(phase),
            Phase::ReadyTrigger => return (1 << READY_TRIGGERS.len()) - 1,
            Phase::ReadyAction(_) => return self.ready_action_mask(i),
            Phase::ReadyGroundHeight {
                actor,
                action,
                cell,
            } => return Act::Other8.bit() | self.ready_ground_heights(actor, action, cell),
            Phase::ReadyTarget { actor, action } => {
                return Act::Other8.bit() | self.ready_targets(actor, action)
            }
            Phase::ReadyRelease(_) | Phase::TacticalMind(..) => {
                return Act::Pass.bit() | Act::Ready.bit()
            }
            Phase::ReadyMove { actor, remaining } => return self.ready_move_mask(actor, remaining),
            Phase::ManeuverSave { .. } => return 0b11 | Act::Pass.bit(),
            Phase::InspirationAttack(..) | Phase::InspirationCheck(..) => {
                return Act::Defend.bit() | Act::Pass.bit()
            }
            Phase::Main => {}
        }
        if self.rope_space(i).is_some() {
            return self.rope_action_mask(i) | self.rope_combat_mask(i);
        }
        let mut mask = Act::End.bit() | self.rope_action_mask(i);
        let range = self.movement_left();
        for (dir, act) in ACTS[..8].iter().enumerate() {
            if let Some((next, cost)) = self.movement_step(i, dir) {
                if cost <= range && self.movement_available(i, next) {
                    mask |= act.bit();
                }
            }
        }
        mask |= self.common_mask(i) | self.traversal_mask(i);
        if self.incap(i) || self.magic_quiet_turn(i) {
            return mask;
        }
        if self.action {
            mask |= Act::Dash.bit() | Act::Disengage.bit() | Act::Dodge.bit();
        }
        if self.action || self.attacks > 0 {
            if self.gear_allows_weapon(i, false)
                && (self.creature_targets(Act::Melee) != 0 || self.observed_pos(i, 1 - i).is_none())
            {
                mask |= Act::Melee.bit();
            }
            if self.f[i].arrows > 0
                && self.weapon(i, true).0 > 0
                && self.gear_allows_weapon(i, true)
                && (self.creature_targets(Act::Ranged) != 0
                    || self.observed_pos(i, 1 - i).is_none())
            {
                mask |= Act::Ranged.bit();
            }
        }
        let special = self.area_action_mask(i)
            | self.armor_mask(i)
            | self.martial_mask(i)
            | self.fighter_rogue_mask(i)
            | self.magic_mask(i)
            | self.gear_mask(i)
            | self.entity_main_mask()
            | self.origin_mask(i)
            | self.divine_mask(i);
        mask |= special;
        let mut candidates = mask;
        while candidates != 0 {
            let action = ACTS[candidates.trailing_zeros() as usize];
            candidates &= candidates - 1;
            if self.target_spec(action).is_some() && !self.target_available(action) {
                mask &= !action.bit();
            }
        }
        self.spell_filter_attack_actions(i, mask)
    }

    fn rope_combat_mask(&self, i: usize) -> u128 {
        use Act::*;
        let mut mask = self.common_mask(i) & (Stand.bit() | DropProne.bit());
        if self.incap(i) || self.magic_quiet_turn(i) {
            return mask;
        }
        if self.action || self.attacks > 0 {
            for action in [Melee, Ranged, Unarmed, Grapple, ShoveProne] {
                let equipped = match action {
                    Melee => self.gear_allows_weapon(i, false),
                    Ranged => self.gear_allows_weapon(i, true)
                        && self.weapon(i, true).0 > 0 && self.f[i].arrows > 0,
                    _ => true,
                };
                if equipped && self.creature_targets(action) != 0 {
                    mask |= action.bit();
                }
            }
        }
        mask |= self.common_mask(i) & (EscapeAthletics.bit() | EscapeAcrobatics.bit() | ReleaseGrapple.bit());
        let gear = &self.f[i].gear;
        let mut equipment = Gear2.bit() | Gear5.bit() | Gear9.bit() | Gear10.bit();
        if gear.melee == super::equipment::Carried::Packed {
            equipment |= Gear1.bit();
        }
        if gear.bow == super::equipment::Carried::Packed {
            equipment |= Gear4.bit();
        }
        mask |= self.gear_mask(i) & equipment;
        mask |= self.rope_cast_mask(i);
        self.spell_filter_attack_actions(i, mask)
    }

    pub(super) fn step_core(&mut self, choice: Act) -> Result<(), &'static str> {
        if self.raw_action_mask() & choice.bit() == 0 {
            return Err("illegal action");
        }
        self.decisions += 1;
        let prior_owner = self.owner;
        let prior_economy = (self.action, self.bonus);
        let action_witnessed = self.f[1 - prior_owner]
            .readied
            .is_some_and(|r| r.trigger == ReadyTrigger::Action)
            && self.can_see(1 - prior_owner, prior_owner);
        match self.phase {
            Phase::Main => {
                if (matches!(
                    choice,
                    Act::Melee
                        | Act::Ranged
                        | Act::Unarmed
                        | Act::Grapple
                        | Act::ShovePush
                        | Act::ShoveProne
                        | Act::ReleaseGrapple
                ) || self.is_martial_targeted_attack(self.owner, choice))
                    && self.creature_targets(choice) & !0b11 != 0
                {
                    self.phase = Phase::CreatureTarget(choice);
                } else if !self.begin_targeting(choice) {
                    self.main_step(choice);
                }
            }
            Phase::TargetRow(action) => self.phase = Phase::TargetColumn(action, choice as i8),
            Phase::TargetColumn(action, row) => {
                let cell = Pos {
                    x: choice as i8,
                    y: row,
                };
                let heights = self.target_altitude_mask(action, cell);
                if self.target_uses_altitude(action) && heights.count_ones() > 1 {
                    self.phase = Phase::TargetAltitude(action, cell);
                } else {
                    debug_assert_ne!(heights, 0);
                    self.resolve_target(
                        action,
                        Point {
                            cell,
                            altitude: heights.trailing_zeros() as i8,
                        },
                    );
                }
            }
            Phase::TargetAltitude(action, cell) => self.resolve_target(
                action,
                Point {
                    cell,
                    altitude: choice as i8,
                },
            ),
            Phase::CreatureTarget(action) => {
                self.phase = Phase::Main;
                let i = self.owner;
                let target = choice as usize;
                if choice == Act::Search {
                    self.begin_targeting(action);
                } else {
                    self.armor_interrupt(i);
                    if action == Act::ReleaseGrapple {
                        if target < 2 {
                            self.f[target].grappled = None;
                        } else {
                            self.entity_release_grapple(target);
                        }
                    } else if let Some(maneuver) = Self::common_maneuver(action) {
                        self.spend_attack(i);
                        self.reveal(i);
                        self.phase = Phase::ManeuverSave {
                            source: i,
                            target,
                            maneuver,
                            opportunity: false,
                            bonus_strike: false,
                        };
                    } else if self.is_martial_targeted_attack(i, action) {
                        self.martial_targeted_step(i, action, target);
                    } else if action == Act::Unarmed {
                        self.spend_attack(i);
                        self.start_attack_against(i, target, self.unarmed_spec(i));
                    } else {
                        self.weapon_attack_against(i, target, action == Act::Ranged);
                    }
                }
            }
            Phase::HelpTarget => {
                self.phase = Phase::Main;
                self.action = false;
                self.apply_primary_help(self.owner, choice as usize);
            }
            Phase::FlurryTarget { actor, count } => {
                self.phase = Phase::Main;
                self.flurry_strike(actor, choice as usize, false, count);
            }
            Phase::Opportunity => self.choose_opportunity(choice),
            Phase::ValorDamage => {
                let (hit, _, _) = self.pending_hit.expect("Combat Inspiration follows a hit");
                if choice == Act::Rider {
                    let bonus = self.spend_bardic_inspiration(hit.attacker);
                    self.pending_hit.as_mut().unwrap().1 += bonus;
                }
                self.begin_martial_hit();
            }
            Phase::Defend { hit, attack } => {
                let defender = hit.defender;
                if choice == Act::Rider {
                    if defender < 2 {
                        self.f[defender].reaction = false;
                    } else {
                        self.entity_spend_reaction(defender);
                    }
                    let bonus = self.spend_bardic_inspiration(defender);
                    if hit.total < hit.ac + bonus {
                        self.finish_missed_attack(hit, attack);
                    } else {
                        self.resolve_defense(hit, attack, Defense::Pass);
                    }
                } else
                if choice == Act::Defend && self.magic_start_shield(defender, hit) {
                    self.pending_shield_hit = Some((hit, attack));
                } else {
                    let defense = if choice == Act::Pass {
                        Defense::Pass
                    } else if self.magic_defend(defender, hit) {
                        Defense::Magic
                    } else {
                        Defense::Martial
                    };
                    if choice == Act::Defend {
                        self.f[defender].reaction = false;
                    }
                    self.resolve_defense(hit, attack, defense);
                }
            }
            Phase::Rider { hit, .. } => {
                if matches!(self.phase, Phase::Rider { magic: true, .. }) {
                    self.phase = Phase::Main;
                    if let Some(distance) = self.magic_repelling_choice(hit, choice) {
                        self.magic_repel(hit, distance);
                    } else if choice == Act::Rider {
                        self.magic_followup(hit.attacker, hit);
                    }
                    if !self.magic_rider_pending(hit.attacker) {
                        self.offer_valor_damage();
                    }
                } else {
                    let extra = if choice == Act::Rider {
                        self.martial_followup(hit.attacker, hit)
                    } else {
                        0
                    };
                    self.finish_hit(0, extra, 0, false);
                }
            }
            Phase::Decision { actor, id } => {
                self.phase = Phase::Main;
                self.decision_step(actor, id, choice);
            }
            Phase::Entity(phase) => {
                self.phase = Phase::Main;
                self.entity_phase_step(phase, choice);
            }
            Phase::CutCheck {
                source,
                result,
                tactical,
            } => {
                self.phase = Phase::Main;
                let reduction = if choice == Act::Defend {
                    self.magic_cut_damage(1 - source)
                } else {
                    0
                };
                let total = result.total - reduction;
                if total >= result.dc {
                    if tactical {
                        self.f[source].resource -= 1;
                    }
                    self.apply_check(source, result.check, total);
                } else if !tactical
                    && self.f[source].build.class == Class::Fighter
                    && self.f[source].build.level >= 2
                    && self.f[source].resource > 0
                {
                    self.phase = Phase::TacticalMind(source, FailedCheck { total, ..result });
                }
            }
            Phase::InspirationAttack(mut hit, attack) => {
                self.phase = Phase::Main;
                if choice == Act::Defend {
                    hit.total += self.spend_bardic_inspiration(hit.attacker);
                }
                if hit.total >= hit.ac {
                    let source = self.creature_point(hit.attacker).expect("active attacker");
                    let target = self.creature_point(hit.defender).expect("active target");
                    hit.crit |= hit.defender < 2
                        && (self.magic_paralyzed(hit.defender) || self.sleep_unconscious(hit.defender))
                        && source.dist(target) <= 1;
                    self.confirm_roll_hit(hit, attack);
                } else {
                    self.offer_precision_or_finish(hit, attack);
                }
            }
            Phase::InspirationCheck(actor, mut result) => {
                self.phase = Phase::Main;
                if choice == Act::Defend {
                    result.total += self.spend_bardic_inspiration(actor);
                }
                if result.total >= result.dc {
                    self.resolve_successful_check(actor, result, false);
                } else {
                    self.check_tactical_mind(actor, result);
                }
            }
            Phase::CutDamage(actor) => {
                self.phase = Phase::Main;
                let (hit, damage, deflected) = self.pending_damage.take().expect("damage decision");
                let reduction = if choice == Act::Defend {
                    self.magic_cut_damage(actor)
                } else {
                    0
                };
                self.apply_attack_damage(hit, (damage - reduction).max(0), deflected);
            }
            Phase::ReadyTrigger => self.phase = Phase::ReadyAction(READY_TRIGGERS[choice as usize]),
            Phase::ReadyAction(trigger) => self.set_readied(trigger, choice),
            Phase::ReadyRelease(actor) => self.ready_release(actor, choice),
            Phase::ReadyGroundHeight {
                actor,
                action,
                cell,
            } => {
                self.phase = Phase::Main;
                if choice != Act::Other8 {
                    self.release_readied_ground(
                        actor,
                        action,
                        Point {
                            cell,
                            altitude: choice as i8,
                        },
                    );
                }
            }
            Phase::ReadyTarget { actor, action } => {
                self.resolve_ready_target(actor, action, choice as usize)
            }
            Phase::ReadyMove { actor, remaining } => self.ready_move(actor, remaining, choice),
            Phase::TacticalMind(actor, check) => {
                self.phase = Phase::Main;
                if choice == Act::Ready {
                    self.tactical_mind(actor, check);
                }
            }
            Phase::ManeuverSave {
                source,
                target,
                maneuver,
                opportunity,
                bonus_strike,
            } => {
                self.phase = Phase::Main;
                self.maneuver_against(
                    source,
                    target,
                    maneuver,
                    (choice != Act::Pass).then_some(choice as usize),
                );
                if matches!(self.phase, Phase::Main) {
                    if bonus_strike {
                        self.complete_attack(source);
                    } else if opportunity {
                        self.resume_queued_movement();
                    }
                }
            }
            Phase::Finished(_) => unreachable!(),
        }
        if matches!(self.phase, Phase::Main) {
            if let Some(resume) = self.area_resume.take() {
                self.phase = resume;
            }
        }
        if self.owner == prior_owner
            && ((prior_economy.0 && !self.action) || (prior_economy.1 && !self.bonus))
        {
            if action_witnessed {
                self.ready_events
                    .push((1 - prior_owner, prior_owner, ReadyTrigger::Action));
            } else {
                self.queue_readied_event(prior_owner, ReadyTrigger::Action);
            }
        }
        self.flush_area_request();
        self.flush_readied_event();
        if matches!(self.phase, Phase::Main) {
            self.resume_queued_movement();
        }
        self.resume_spell_movement_callback();
        if matches!(self.phase, Phase::Main) && self.pending_turn_end {
            self.pending_turn_end = false;
            self.finish_player_turn();
        }
        self.settle_decision();
        Ok(())
    }

    fn settle_decision(&mut self) {
        self.clear_broken_grapples();
        self.martial_break_entity_grapples();
        for i in 0..2 {
            if self.incap(i) || self.f[i].hp == 0 {
                self.magic_end_wild_shape(i);
                self.divine_end_on_incapacitation(i);
                self.f[i].raging = false;
                self.clear_martial_subclass_on_incapacitation(i);
                self.f[i].concentration = None;
                self.armor_interrupt(i);
            }
            if self.speed(i) == 0 {
                self.f[i].dodge = false;
            }
        }
        if matches!(self.phase, Phase::Main) && self.f.iter().all(|f| f.hp > 0) {
            self.traversal_falls();
            if matches!(self.phase, Phase::Main) {
                self.settle_companion_flight();
            }
        }
        self.check_ko();
        if matches!(self.phase, Phase::Main) {
            self.begin_beguiling_prompt();
        }
        if self.decisions >= DECISION_LIMIT && !self.finished() {
            self.phase = Phase::Finished(None);
        }
    }

    pub(super) fn save_choice_complete_decisions(&mut self, count: usize) {
        self.decisions = self.decisions.saturating_add(count as i32);
        if self.decisions >= DECISION_LIMIT && !self.finished() {
            self.phase = Phase::Finished(None);
        }
    }

    pub(super) fn disengaged_for(&self, actor: usize) -> bool {
        actor < 2 && actor == self.owner && self.disengaged
    }

    fn main_step(&mut self, choice: Act) {
        let i = self.owner;
        use Act::*;
        if matches!(choice, super::bard_ranger::CLIMB_ROPE | super::bard_ranger::LEAVE_ROPE
            | super::bard_ranger::TOGGLE_ROPE) && self.rope_action_step(i, choice)
        {
            return;
        }
        if choice as usize >= 8
            && !matches!(
                choice,
                JumpLong | JumpHigh | Climb | OtherCommon1 | OtherCommon2
            )
        {
            self.tactical_steps = 0;
        }
        if self.armor_mask(i) & choice.bit() != 0 {
            self.traversal_break_runup(i);
            self.armor_step(i, choice);
            return;
        }
        if choice != End {
            self.armor_interrupt(i);
        }
        if self.area_action_step(i, choice) || self.traversal_step(i, choice) {
            return;
        }
        if choice as usize >= 8 {
            self.traversal_break_runup(i);
        }
        if self.origin_mask(i) & choice.bit() != 0 {
            self.origin_step(i, choice);
            return;
        }
        if self.entity_main_step(choice) {
            return;
        }
        if self.gear_mask(i) & choice.bit() != 0 {
            self.gear_step(i, choice);
            return;
        }
        self.f[i].gear.after_attack = false;
        if self.divine_main_step(i, choice) { return; }
        if self.fighter_rogue_mask(i) & choice.bit() != 0 {
            self.fighter_rogue_main_step(i, choice);
            return;
        }
        match choice {
            N | NE | E | SE | S | SW | W | NW => self.move_step(choice as usize),
            End => self.end_turn(),
            Melee | Ranged => self.weapon_attack(i, choice == Ranged),
            Dash => {
                self.action = false;
                self.dash += self.speed(i);
            }
            Disengage => {
                self.action = false;
                self.disengaged = true;
            }
            Dodge => {
                self.action = false;
                self.f[i].dodge = self.speed(i) > 0;
            }
            Special1 | Special2 | Special3 | Special4 | Special5 | Special6 | Special7
            | Special8 => {
                if self.martial_mask(i) & choice.bit() != 0 {
                    self.martial_step(i, choice);
                } else {
                    self.magic_step(i, choice);
                }
            }
            Spell0 | Spell1 | Spell2 | Spell3 | Spell4 | Spell5 => self.magic_step(i, choice),
            Opportunity | Pass | Defend | Rider => unreachable!(),
            _ if self.martial_mask(i) & choice.bit() != 0 => self.martial_step(i, choice),
            _ if self.magic_mask(i) & choice.bit() != 0 => self.magic_step(i, choice),
            _ => self.common_step(i, choice),
        }
    }

    fn drag_adds_cost(&self, source: usize, target: usize) -> bool {
        let target_size = self.combatant_size(target);
        target_size != 0 && target_size + 2 > self.combatant_size(source)
    }

    pub(in crate::arena2) fn movement_step(&self, i: usize, dir: usize) -> Option<(Pos, i32)> {
        if self.entities.mounted[i] {
            return self.entity_destination(i, dir);
        }
        let (next, mut terrain) = self.map.step_at(self.fighter_point(i), dir)?;
        if self.magic_difficult(next) {
            terrain = terrain.max(2);
        }
        terrain = self.overgrowth_cost(next, terrain);
        let climbing_cost = self.traversal_walk_candidate(i, next)?;
        let drag_costly = self.f[i].dragging
            && ((self.f[1 - i].grappled.is_some() && self.drag_adds_cost(i, 1 - i))
                || (self.martial_entity_grapple_count(i) > 0 && self.drag_adds_cost(i, 3 - i)));
        let extra = i32::from(self.f[i].prone) + i32::from(drag_costly);
        let wind = i32::from(self.gust_hinders(self.fighter_point(i), next));
        let (cost, _) = self.footprint_movement(i, next, terrain + extra + climbing_cost + wind)?;
        self.entity_fear_allows_move(i, next.cell)
            .then_some((next.cell, cost))
    }

    pub(in crate::arena2) fn movement_available(&self, i: usize, destination: Pos) -> bool {
        let point = Point {
            cell: destination,
            altitude: self.f[i].elevation,
        };
        let foe_known = self.observed_pos(i, 1 - i).is_some();
        (!foe_known || self.fighter_point(1 - i) != point)
            && (2..4).all(|id| {
                self.observed_entity(i, id)
                    .is_none_or(|e| e.point() != point)
            })
    }

    pub(in crate::arena2) fn try_collision(&mut self, i: usize, destination: Pos) -> bool {
        self.try_point_collision(
            i,
            Point {
                cell: destination,
                altitude: self.f[i].elevation,
            },
        )
    }

    pub(in crate::arena2) fn try_point_collision(&mut self, i: usize, point: Point) -> bool {
        if (2..4).any(|id| self.creature_point(id) == Some(point)) {
            return true;
        }
        if self.fighter_point(1 - i) == point {
            self.reveal(1 - i);
            return true;
        }
        false
    }

    fn move_step(&mut self, dir: usize) {
        let i = self.owner;
        let (destination, cost) = self.movement_step(i, dir).expect("legal movement");
        if self.try_collision(i, destination) {
            return;
        }
        if self.tactical_steps >= cost {
            let previous = self.fighter_point(i);
            self.commit_move(i, destination);
            let current = self.fighter_point(i);
            if current != previous {
                self.tactical_steps -= cost;
                self.traversal_after_step(i, previous, current, true);
            }
            return;
        }
        self.queue_movement(
            i,
            Point {
                cell: destination,
                altitude: self.f[i].elevation,
            },
            cost,
            true,
        );
    }

    pub(super) fn queue_traversal_move(&mut self, i: usize, destination: Point, cost: i32) {
        if self.try_point_collision(i, destination) {
            self.traversal_cancel_pending(i);
        } else if self.tactical_steps >= cost {
            let previous = self.fighter_point(i);
            self.commit_move_point(i, destination);
            if self.fighter_point(i) == destination {
                self.tactical_steps -= cost;
                self.traversal_after_step(i, previous, destination, false);
            } else {
                self.traversal_cancel_pending(i);
            }
        } else {
            self.queue_movement(i, destination, cost, false);
        }
    }

    pub(super) fn mover_would_leave_reach(
        &self,
        mover: usize,
        destination: Point,
        reactor: usize,
    ) -> bool {
        let Some(reactor_body) = self.footprint(reactor) else {
            return false;
        };
        let squeeze = self
            .footprint_movement(mover, destination, 1)
            .is_some_and(|(_, squeezing)| squeezing);
        let edge = if self.combatant_size(mover) >= 3 && !squeeze {
            2
        } else {
            1
        };
        let moved = super::footprint::Footprint {
            anchor: destination,
            edge,
        };
        let reach = self.unarmed_reach(reactor);
        self.combatant_distance(mover, reactor)
            .is_some_and(|distance| distance <= reach)
            && moved.distance(reactor_body) > reach
    }

    fn queue_movement(&mut self, i: usize, destination: Point, cost: i32, ordinary: bool) {
        self.tactical_steps = 0;
        self.entity_begin_player_move();
        self.pending_move = Some((self.fighter_point(i), destination, cost, ordinary));
        if !self.disengaged_for(i)
            && !self.suggestion_charmed_by(1 - i, i)
            && !self.magic_agile(i)
            && self.can_react(1 - i)
            && !self.magic_no_reactions(1 - i)
            && !self.f[1 - i].addled
            && self.can_see(1 - i, i)
            && self.mover_would_leave_reach(i, destination, 1 - i)
        {
            self.phase = Phase::Opportunity;
        } else {
            self.resume_move();
        }
    }

    fn choose_opportunity(&mut self, choice: Act) {
        let reactor = self.actor().expect("reactor");
        let target = self.opportunity_target(reactor);
        if choice == Act::Pass {
            self.resume_queued_movement();
            return;
        }
        self.f[reactor].reaction = false;
        self.armor_interrupt(reactor);
        if matches!(choice, Act::Grapple | Act::ShovePush | Act::ShoveProne) {
            let maneuver = match choice {
                Act::Grapple => UnarmedManeuver::Grapple,
                Act::ShovePush => UnarmedManeuver::ShovePush,
                _ => UnarmedManeuver::ShoveProne,
            };
            self.phase = Phase::ManeuverSave {
                source: reactor,
                target,
                maneuver,
                opportunity: true,
                bonus_strike: false,
            };
        } else {
            let attack = if choice == Act::Special3 {
                self.note_rogue_blade(reactor);
                self.rogue_blade_spec(reactor, target, choice)
            } else if choice == Act::Opportunity
                && self.gear_wielded(reactor, false)
                && self.combatant_distance(reactor, target).is_some_and(|distance| distance <= 1)
            {
                self.weapon_spec(reactor, false, false)
            } else {
                self.unarmed_spec(reactor)
            };
            self.start_attack_against(reactor, target, attack);
        }
    }

    fn opportunity_target(&self, reactor: usize) -> usize {
        self.pending_spell_move
            .map_or(1 - reactor, |(mover, ..)| mover)
    }

    fn resume_queued_movement(&mut self) {
        if let Some((mover, origin, destination)) = self.pending_spell_move {
            if self.creature_point(mover) != Some(origin)
                || !self.entity_offer_spell_opportunity(mover, destination)
            {
                self.resume_spell_reaction_move();
            }
        } else if self.ready_pending_move.is_some() {
            self.resume_ready_move();
        } else if self.pending_move.is_some() {
            self.resume_move();
        } else {
            self.resume_ready_continuation();
        }
    }

    pub(super) fn resume_move(&mut self) {
        self.phase = Phase::Main;
        if let Some((origin, destination, cost, ordinary)) = self.pending_move {
            if origin == self.fighter_point(self.owner)
                && self.entity_offer_player_opportunity(self.owner, destination.cell)
            {
                return;
            }
            self.pending_move = None;
            let i = self.owner;
            if origin == self.fighter_point(i)
                && self.movement_left() >= cost
                && (ordinary || self.traversal_can_resume(i))
            {
                if self.entities.mounted[i] {
                    let previous = self.fighter_point(i);
                    let before = self.ready_movement_context(i);
                    self.move_companion(i, destination.cell, cost);
                    self.martial_moved(i);
                    self.divine_moved(i);
                    self.magic_moved(i, previous);
                    self.move_trigger(i, before);
                } else {
                    let previous = self.fighter_point(i);
                    self.commit_move_point(i, destination);
                    if self.fighter_point(i) == destination {
                        self.moved += cost;
                        self.traversal_after_step(i, previous, destination, ordinary);
                    } else if !ordinary {
                        self.traversal_cancel_pending(i);
                    }
                }
            } else if !ordinary {
                self.traversal_cancel_pending(i);
            }
        }
    }

    pub(super) fn commit_move(&mut self, i: usize, destination: Pos) {
        self.commit_move_point(
            i,
            Point {
                cell: destination,
                altitude: self.f[i].elevation,
            },
        );
    }

    pub(super) fn commit_move_point(&mut self, i: usize, destination: Point) {
        self.commit_position(i, destination, true);
    }

    pub(super) fn commit_forced_move(&mut self, i: usize, destination: Point) {
        self.commit_position(i, destination, false);
        self.clear_broken_grapples();
    }

    fn commit_position(&mut self, i: usize, destination: Point, allow_drag: bool) {
        let before = self.ready_movement_context(i);
        if !self.commit_footprint(i, destination, false) {
            if self
                .footprint(1 - i)
                .is_some_and(|f| f.squares().any(|p| p == destination))
            {
                self.reveal(1 - i);
            }
            return;
        }
        self.armor_interrupt(i);
        let previous = self.fighter_point(i);
        let was_dragging = allow_drag && self.f[i].dragging && self.f[1 - i].grappled.is_some();
        self.f[i].pos = destination.cell;
        self.f[i].elevation = destination.altitude;
        if was_dragging
            && self.map.can_occupy(previous)
            && (2..4).all(|id| self.creature_point(id) != Some(previous))
        {
            let target = 1 - i;
            let original = self.fighter_point(target);
            self.f[target].pos = previous.cell;
            self.f[target].elevation = previous.altitude;
            self.martial_moved(target);
            if self.f[target].hp > 0 {
                self.magic_moved(target, original);
            }
        }
        if allow_drag && self.f[i].dragging {
            self.martial_drag_grappled_entity(i, previous);
        }
        if self
            .map
            .cover_at(self.fighter_point(i), self.fighter_point(1 - i))
            == super::geometry::Cover::None
            && !self.magic_cloud_blocks(self.fighter_point(i), self.fighter_point(1 - i))
        {
            self.reveal(i);
            self.reveal(1 - i);
        }
        self.martial_moved(i);
        self.divine_moved(i);
        if self.f[i].hp > 0 {
            self.magic_moved(i, previous);
        }
        self.move_trigger(i, before);
    }

    fn ready_movement_context(&self, mover: usize) -> (bool, bool) {
        let reactor = 1 - mover;
        if self.f[reactor].readied.is_none() {
            return (false, false);
        }
        (self.can_see(reactor, mover), self.dist() <= self.unarmed_reach(reactor))
    }

    fn move_trigger(&mut self, mover: usize, before: (bool, bool)) {
        let reactor = 1 - mover;
        if let Some(readied) = self.f[reactor].readied {
            let reach = self.unarmed_reach(reactor);
            let trigger = match readied.trigger {
                ReadyTrigger::EnterReach
                    if !before.1 && self.dist() <= reach && self.can_see(reactor, mover) =>
                {
                    Some(ReadyTrigger::EnterReach)
                }
                ReadyTrigger::EnterSight if !before.0 && self.can_see(reactor, mover) => {
                    Some(ReadyTrigger::EnterSight)
                }
                ReadyTrigger::LeaveReach if before.0 && before.1 && self.dist() > reach => {
                    Some(ReadyTrigger::LeaveReach)
                }
                ReadyTrigger::Move if before.0 || self.can_see(reactor, mover) => {
                    Some(ReadyTrigger::Move)
                }
                _ => None,
            };
            if let Some(trigger) = trigger {
                self.offer_readied(reactor, trigger);
            }
        }
    }

    pub(super) fn end_turn(&mut self) {
        self.f[self.owner].bard_ranger.first_turn = 0;
        if !self.entity_end_turn(self.owner) {
            self.advance_player_turn();
        }
    }

    pub(super) fn advance_player_turn(&mut self) {
        let old = self.owner;
        self.divine_turn_end(old);
        self.armor_end(old);
        self.martial_end_turn(old);
        self.martial_poison_end(old);
        self.origin_end(old);
        self.magic_end(old);
        self.spell_movement_turn_end(old);
        self.flush_area_request();
        self.f[old].attack_advantage = false;
        if !matches!(self.phase, Phase::Main) {
            self.pending_turn_end = true;
            return;
        }
        self.finish_player_turn();
    }

    fn finish_player_turn(&mut self) {
        let old = self.owner;
        self.check_ko();
        if self.finished() {
            return;
        }
        if self.turns >= TURN_LIMIT {
            self.phase = Phase::Finished(None);
            return;
        }
        self.blink_end_turn(old);
        self.owner = 1 - old;
        let i = self.owner;
        self.turns += 1;
        self.expire_rope_portals();
        self.action = true;
        self.bonus = true;
        self.attacks = 0;
        self.dash = 0;
        self.moved = 0;
        self.disengaged = false;
        let active = &mut self.f[i];
        active.reaction = true;
        active.dodge = false;
        active.shield = 0;
        active.reckless = false;
        active.addled = false;
        active.speed_zero = false;
        active.readied = None;
        active.help_entity_target = None;
        self.companion_check_help[i] = None;
        let opponent = &mut self.f[old];
        opponent.sap = false;
        opponent.slow = false;
        opponent.stunned = false;
        opponent.stun_slow = false;
        opponent.stun_exposed = false;
        if !self.blink_start_turn(i) { self.finish_start_turn(i); }
    }

    pub(super) fn finish_start_turn(&mut self, i: usize) {
        self.martial_turn(i);
        for f in &mut self.f {
            f.arcane.portent_used = false;
        }
        self.magic_turn(i);
        self.gear_turn_start(i);
        self.entity_start_turn(i);
        self.object_begin_turn(i);
        self.traversal_begin_turn(i);
        self.subclass_start_turn(i);
    }

    pub(super) fn roll(&mut self, sides: i32) -> i32 {
        debug_assert!(sides > 0);
        self.dice.roll(sides as u64)
    }

    pub(super) fn d20(&mut self, advantage: bool, disadvantage: bool) -> i32 {
        let first = self.roll(20);
        match (advantage, disadvantage) {
            (true, false) => first.max(self.roll(20)),
            (false, true) => first.min(self.roll(20)),
            _ => first,
        }
    }

    pub(super) fn save(&mut self, i: usize, stat: usize, dc: i32) -> bool {
        self.save_advantage(i, stat, dc, false)
    }

    pub(super) fn save_advantage(
        &mut self,
        i: usize,
        stat: usize,
        dc: i32,
        disadvantage: bool,
    ) -> bool {
        self.save_with_cover(i, stat, dc, disadvantage, 0, false)
    }

    pub(super) fn save_from_point(
        &mut self,
        i: usize,
        stat: usize,
        dc: i32,
        disadvantage: bool,
        origin: Point,
    ) -> bool {
        let cover_bonus = if stat == 1 {
            let target = self.fighter_point(i);
            let interveners = (0..4).filter_map(|id| self.creature_point(id));
            self.map
                .cover_from_point_with_creatures(origin, target, interveners)
                .bonus()
        } else {
            0
        };
        self.save_with_cover(i, stat, dc, disadvantage, cover_bonus, false)
    }

    pub(in crate::arena2) fn save_with_cover(
        &mut self,
        i: usize,
        stat: usize,
        dc: i32,
        disadvantage: bool,
        cover_bonus: i32,
        forced_advantage: bool,
    ) -> bool {
        if (self.f[i].stunned || self.magic_paralyzed(i) || self.sleep_unconscious(i)) && matches!(stat, 0 | 1) {
            return false;
        }
        let f = &self.f[i];
        let advantage = forced_advantage
            || (stat == 1 && f.dodge && self.speed(i) > 0 && !self.incap(i))
            || (f.raging && stat == 0)
            || (stat == 1
                && f.build.class == Class::Barbarian
                && f.build.level >= 2
                && !self.incap(i));
        let base =
            self.magic_save_modifier(i, stat) + if f.build.saves[stat] { f.build.pb } else { 0 };
        let total = self.d20_for(
            i,
            advantage,
            disadvantage
                || self.armor_disadvantages_ability(i, stat)
                || (stat == 1 && (self.magic_restrained(i) || self.is_squeezing(i))),
        ) + base
            + cover_bonus
            + self.magic_roll_bonus(i);
        self.inspired_save(i, stat, total, dc, false)
    }

    pub(in crate::arena2) fn save_poison_condition(&mut self, i: usize, dc: i32) -> bool {
        let advantage = self.origin_poison_save_advantage(i);
        self.save_with_cover(i, 2, dc, false, 0, advantage)
    }

    fn save_concentration(&mut self, i: usize, dc: i32) -> bool {
        let advantage = self.magic_concentration_advantage(i);
        let f = &self.f[i];
        let base = self.magic_save_modifier(i, 2) + if f.build.saves[2] { f.build.pb } else { 0 };
        let floor = if self.divine_dragon(i) { 10 } else { 1 };
        let total = self.d20_for(i, advantage, false).max(floor) + base + self.magic_roll_bonus(i);
        self.inspired_save(i, 2, total, dc, true)
    }

    pub(super) fn damage_typed(&mut self, i: usize, amount: i32, kind: DamageType) {
        if i >= 2 {
            self.entity_damage_typed(i, amount, kind);
            return;
        }
        let amount = self.damage_parts_after_resistance(i, &mut [(amount, kind)]);
        self.commit_resolved_damage(i, amount);
    }

    pub(super) fn damage_parts_after_resistance(
        &mut self,
        target: usize,
        parts: &mut [(i32, DamageType)],
    ) -> i32 {
        let total: i32 = parts.iter().map(|&(amount, _)| amount.max(0)).sum();
        let mut reduction = total - self.fighter_resolve_fields(target, total);
        parts.iter().map(|&(amount, kind)| {
            let used = reduction.min(amount.max(0));
            reduction -= used;
            self.damage_after_resistance(target, (amount - used).max(0), kind)
        }).sum()
    }

    pub(super) fn damage_after_resistance(
        &mut self,
        i: usize,
        amount: i32,
        kind: DamageType,
    ) -> i32 {
        if amount <= 0 {
            return amount;
        }
        if i >= 2 {
            return self.entity_damage_after_resistance(i, amount, kind);
        }
        let amount = self.magic_flat_protection(i, amount, kind);
        let f = &self.f[i];
        let native = (is_physical(kind) && f.raging && !self.incap(i))
            || (kind == DamageType::Poison
                && (self.origin_resists_poison(i) || self.magic_resists_poison(i)));
        if native || self.magic_energy_resistance(i, kind) {
            amount / 2
        } else {
            amount
        }
    }

    pub(super) fn heal(&mut self, i: usize, amount: i32) {
        let max_hp = self.magic_max_hp(i);
        let f = &mut self.f[i];
        f.hp = (f.hp + amount).min(max_hp);
    }

    pub(super) fn damage(&mut self, i: usize, amount: i32) {
        let amount = self.fighter_resolve_fields(i, amount);
        self.commit_resolved_damage(i, amount);
    }

    fn commit_resolved_damage(&mut self, i: usize, amount: i32) -> i32 {
        if amount <= 0 { return 0; }
        if i >= 2 { return self.entity_damage_resolved(i, amount); }
        let amount = self.arcane_ward_absorb(i, amount);
        if amount == 0 { return 0; }
        self.armor_interrupt(i);
        let temp = amount.min(self.f[i].temp);
        self.f[i].temp -= temp;
        self.f[i].hp = (self.f[i].hp - (amount - temp)).max(0);
        self.magic_hurt(i);
        if self.f[i].concentration.is_some()
            && (self.f[i].hp == 0
                || self.incap(i)
                || !self.save_concentration(i, (amount / 2).clamp(10, 30)))
        {
            self.f[i].concentration = None;
        }
        amount
    }

    pub(super) fn can_slot(&self, i: usize, level: usize) -> bool {
        (1..=3).contains(&level)
            && self.f[i].slots[level - 1] > 0
            && self.f[i].slot_turn != self.turns
    }

    pub(super) fn spend_slot(&mut self, i: usize, level: usize) -> bool {
        if self.can_slot(i, level) {
            self.f[i].slots[level - 1] -= 1;
            self.f[i].slot_turn = self.turns;
            true
        } else {
            false
        }
    }

    fn check_ko(&mut self) {
        match (self.f[0].hp == 0, self.f[1].hp == 0) {
            (true, true) => self.phase = Phase::Finished(None),
            (true, false) => self.phase = Phase::Finished(Some(1)),
            (false, true) => self.phase = Phase::Finished(Some(0)),
            _ => {}
        }
    }

    fn weapon(&self, i: usize, ranged: bool) -> (i32, i32) {
        self.magic_weapon(i, ranged)
            .or_else(|| self.gear_weapon_stats(i, ranged))
            .unwrap_or_else(|| {
                let b = self.f[i].build;
                if ranged {
                    (b.ranged_die, b.ranged_mod)
                } else {
                    (b.melee_die, b.melee_mod)
                }
            })
    }

    fn weapon_spec(&self, i: usize, ranged: bool, flurry: bool) -> Attack {
        let (mut die, mut modifier) = self.weapon(i, ranged);
        let kind = self.gear_weapon_kind(i, ranged);
        let monk_weapon = kind.and_then(|w| self.martial_monk_weapon_stats(i, w));
        if let Some((minimum, monk_modifier)) = monk_weapon {
            die = die.max(minimum);
            if self.magic_weapon(i, ranged).is_none() {
                modifier = monk_modifier;
            }
        }
        let unarmed = self.f[i].build.class == Class::Monk && kind.is_none();
        if unarmed && self.martial_arts_active(i) {
            modifier = self.weapon_finesse_modifier(i);
        }
        let ability = if unarmed {
            self.martial_unarmed_ability(i)
        } else if ranged {
            1
        } else if monk_weapon.is_some() || kind.is_some_and(WeaponKind::finesse) {
            self.weapon_finesse_ability(i)
        } else {
            0
        };
        let build = self.f[i].build;
        let proficiency = if kind.is_none_or(|kind| kind.proficient(build)) {
            build.pb
        } else {
            0
        };
        let enhancement = if unarmed {
            0
        } else {
            self.magic_weapon_bonus(i, ranged)
        };
        Attack {
            die,
            count: self.magic_weapon_dice(i),
            bonus: modifier + enhancement,
            to_hit: self
                .magic_weapon_attack_bonus(i, ranged)
                .unwrap_or(modifier + proficiency)
                + self.martial_attack_bonus(i, ranged)
                + enhancement,
            ranged,
            weapon: true,
            flurry,
            unarmed,
            thrown: false,
            long_range: false,
            improvised: false,
            kind_override: None,
            dexterity: ability == 1,
            strength: ability == 0,
        }
    }

    fn spend_attack(&mut self, i: usize) {
        if self.attacks == 0 {
            self.action = false;
            self.attacks = self.extra_attack(i);
        }
        self.attacks -= 1;
    }

    pub(super) fn weapon_attack(&mut self, i: usize, ranged: bool) {
        self.spend_attack(i);
        self.gear_strike_equip(i, ranged);
        self.note_light_equipped(i, ranged);
        if let Some(target) = self.guessed_attack_target(i, ranged) {
            self.start_attack_against(i, target, self.weapon_spec(i, ranged, false));
        }
    }

    pub(in crate::arena2) fn weapon_attack_against(
        &mut self,
        i: usize,
        target: usize,
        ranged: bool,
    ) {
        self.spend_attack(i);
        self.gear_strike_equip(i, ranged);
        self.note_light_equipped(i, ranged);
        self.start_attack_against(i, target, self.weapon_spec(i, ranged, false));
    }

    pub(in crate::arena2) fn bonus_unarmed_attack_against(
        &mut self,
        i: usize,
        target: usize,
        count: i32,
    ) {
        self.bonus = false;
        if count > 1 {
            self.bonus_attacks = Some((i, count - 1, false, target));
        }
        let mut attack = self.unarmed_spec(i);
        attack.flurry = count > 1;
        self.start_attack_against(i, target, attack);
    }

    fn flurry_strike(&mut self, i: usize, target: usize, ranged: bool, count: i32) {
        if count > 1 {
            self.bonus_attacks = Some((i, count - 1, ranged, target));
        }
        let mut attack = if ranged {
            self.weapon_spec(i, true, true)
        } else {
            self.unarmed_spec(i)
        };
        attack.flurry = true;
        self.start_attack_against(i, target, attack);
    }

    pub(super) fn form_attack_target(&mut self, i: usize, target: Option<usize>) {
        if let Some(target) = target {
            self.start_attack_against(i, target, self.weapon_spec(i, false, false));
        } else {
            self.reveal(i);
            self.martial_attack_rolled(i, 1 - i);
            self.complete_attack(i);
        }
    }

    pub(super) fn true_strike_range(&self, i: usize, ranged: bool) -> (i32, i32) {
        match self.gear_weapon_kind(i, ranged) {
            Some(WeaponKind::Longbow) if ranged => (30, 120),
            Some(WeaponKind::Shortbow) if ranged => (16, 64),
            Some(_) if !ranged => (1, 1),
            _ => (0, 0),
        }
    }

    pub(super) fn true_strike_attack(
        &mut self,
        i: usize,
        target: usize,
        ranged: bool,
        long_range: bool,
    ) {
        let build = self.f[i].build;
        let kind = self.gear_weapon_kind(i, ranged);
        let proficiency = if kind.is_some_and(|w| w.proficient(build)) {
            build.pb
        } else {
            0
        };
        let enhancement = self.magic_weapon_bonus(i, ranged);
        let mut attack = self.weapon_spec(i, ranged, false);
        attack.bonus = build.casting + enhancement;
        attack.to_hit = attack.bonus + proficiency + self.martial_attack_bonus(i, ranged);
        attack.strength = false;
        attack.dexterity = false;
        attack.long_range = long_range;
        self.start_attack_against(i, target, attack);
    }

    pub(super) fn true_strike_throw_range(&self, i: usize) -> Option<(i32, i32)> {
        (self.gear_wielded(i, false) && self.gear_weapon_kind(i, false) == Some(WeaponKind::Dagger))
            .then_some((4, 12))
    }

    pub(super) fn true_strike_throw(
        &mut self,
        i: usize,
        target: Option<usize>,
        landing: Point,
        long_range: bool,
    ) {
        let issuer = self.gear_weapon_owner(i, false);
        let build = self.f[i].build;
        let bonus = build.casting + self.magic_weapon_bonus(i, false);
        self.gear_drop_weapon_instance(i, issuer, false, self.map.landing(landing).cell);
        let Some(target) = target else {
            self.true_strike_miss(i, false);
            return;
        };
        self.start_attack_against(
            i,
            target,
            Attack {
                die: 4,
                count: 1,
                bonus,
                to_hit: bonus + build.pb,
                ranged: true,
                weapon: true,
                flurry: false,
                unarmed: false,
                thrown: true,
                long_range,
                improvised: false,
                kind_override: Some(WeaponKind::Dagger),
                strength: false,
                dexterity: false,
            },
        );
    }

    pub(super) fn true_strike_miss(&mut self, i: usize, ranged: bool) {
        self.resolve_guessed_miss(i, ranged);
    }

    pub(super) fn spell_attack_target(
        &mut self,
        i: usize,
        target: usize,
        die: i32,
        count: i32,
        bonus: i32,
        ranged: bool,
    ) {
        self.start_attack_against(
            i,
            target,
            Attack {
                die,
                count,
                bonus,
                to_hit: self.f[i].build.pb + self.f[i].build.casting,
                ranged,
                weapon: false,
                flurry: false,
                unarmed: false,
                thrown: false,
                long_range: false,
                improvised: false,
                kind_override: None,
                dexterity: false,
                strength: false,
            },
        );
    }

    pub(super) fn magic_missile(&mut self, i: usize, damage: i32) {
        let defender = 1 - i;
        if self.f[defender].shield > 0 {
            return;
        }
        let hit = Hit {
            attacker: i,
            defender,
            d20: 0,
            total: 0,
            ac: self.ac(defender),
            ranged: true,
            weapon: false,
            physical: false,
            damage_kind: DamageType::Force,
            crit: false,
            advantage: false,
            disadvantage: false,
            flurry: false,
            unarmed: false,
            thrown: false,
            weapon_kind: None,
            improvised: false,
            strength: false,
        };
        let attack = Attack {
            die: 0,
            count: 0,
            bonus: damage,
            to_hit: 0,
            ranged: true,
            weapon: false,
            flurry: false,
            unarmed: false,
            thrown: false,
            long_range: false,
            improvised: false,
            kind_override: None,
            dexterity: false,
            strength: false,
        };
        if self.can_react(defender) && self.magic_defend(defender, hit) {
            self.phase = Phase::Defend { hit, attack };
        } else {
            let before = self.footprint(defender);
            self.damage(defender, damage);
            self.magic_blessing_at_zero(Some(i), defender, before);
        }
    }

    fn cover_to(&self, origin: Point, target: usize) -> super::geometry::Cover {
        let destination = self.footprint(target).expect("living target");
        destination
            .squares()
            .map(|point| {
                let interveners = (0..4)
                    .filter(|&id| id != target && self.rope_same_space(id, target))
                    .filter_map(|id| self.footprint(id))
                    .filter(|footprint| !footprint.squares().any(|p| p == origin))
                    .flat_map(|footprint| footprint.squares());
                self.map.cover_with_creatures(origin, point, interveners)
            })
            .min_by_key(|&cover| cover as u8)
            .expect("occupied target")
    }

    pub(super) fn ac(&self, i: usize) -> i32 {
        if i >= 2 {
            return self.magic_entity_base_ac(i).unwrap_or_else(|| {
                i32::from(self.entity(i).expect("living entity").kind.stats().ac)
            }) + if self.magic_ally_shielded(i) { 2 } else { 0 };
        }
        self.magic_ac(i)
            .unwrap_or(self.f[i].build.ac + self.gear_ac_adjust(i) + self.armor_adjust(i))
            + self.f[i].shield
            + self.magic_ac_bonus(i)
    }

    pub(super) fn entity_weapon_attack(
        &mut self,
        side: usize,
        defender: usize,
        die: i32,
        bonus: i32,
        to_hit: i32,
    ) {
        self.attack_resume = self.entity_phase();
        self.attack_source = Some(2 + side);
        self.start_attack_against(
            side,
            defender,
            Attack {
                die,
                count: 1,
                bonus,
                to_hit,
                ranged: false,
                weapon: true,
                flurry: false,
                unarmed: false,
                thrown: false,
                long_range: false,
                improvised: false,
                kind_override: None,
                dexterity: false,
                strength: false,
            },
        );
    }

    pub(super) fn entity_start_opportunity(&mut self, reactor: usize, target: usize, psychic_blade: bool) {
        self.attack_resume = self.entity_phase();
        self.f[reactor].reaction = false;
        let use_weapon = self.gear_wielded(reactor, false);
        let attack = if psychic_blade {
            self.note_rogue_blade(reactor);
            self.rogue_blade_spec(reactor, target, Act::Special3)
        } else if use_weapon {
            self.weapon_spec(reactor, false, false)
        } else {
            self.unarmed_spec(reactor)
        };
        self.start_attack_against(reactor, target, attack);
    }

    pub(super) fn launch_dagger(&mut self, i: usize, defender: Option<usize>, long_range: bool) {
        self.spend_attack(i);
        let issuer = self.gear_weapon_owner(i, false);
        self.weapon_note_light(i, issuer, Some(WeaponKind::Dagger));
        let Some(defender) = defender else {
            self.resolve_guessed_miss(i, false);
            return;
        };
        let f = &self.f[i];
        let modifier = f.build.stats[self.weapon_finesse_ability(i)];
        let enhancement = self.magic_weapon_bonus(i, false);
        let attack = Attack {
            die: 4,
            count: 1,
            bonus: modifier + enhancement,
            to_hit: modifier + f.build.pb + enhancement,
            ranged: true,
            weapon: true,
            flurry: false,
            unarmed: false,
            thrown: true,
            long_range,
            improvised: false,
            kind_override: Some(WeaponKind::Dagger),
            dexterity: self.weapon_finesse_ability(i) == 1,
            strength: self.weapon_finesse_ability(i) == 0,
        };
        self.start_attack_against(i, defender, attack);
    }

    fn note_light_equipped(&mut self, i: usize, ranged: bool) {
        self.weapon_note_light(
            i,
            self.gear_weapon_owner(i, ranged),
            self.gear_weapon_kind(i, ranged),
        );
    }

    pub(super) fn weapon_option_attack(
        &mut self,
        i: usize,
        target: Option<usize>,
        spec: super::weapon_options::OptionAttack,
    ) {
        if spec.bonus_action {
            self.bonus = false;
        } else {
            self.spend_attack(i);
        }
        let Some(defender) = target else {
            self.reveal(i);
            return;
        };
        let attack = Attack {
            die: spec.die,
            count: 1,
            bonus: spec.damage_bonus,
            to_hit: spec.to_hit,
            ranged: spec.ranged,
            weapon: true,
            flurry: false,
            unarmed: false,
            thrown: spec.ranged,
            long_range: spec.long_range,
            improvised: spec.improvised,
            kind_override: spec.kind,
            dexterity: !spec.strength,
            strength: spec.strength,
        };
        self.start_attack_against(i, defender, attack);
    }

    pub(super) fn cleave_attack(&mut self, i: usize, target: usize) {
        let mut attack = self.weapon_spec(i, false, false);
        attack.bonus = attack.bonus.min(0);
        self.start_attack_against(i, target, attack);
    }

    fn start_attack_against(&mut self, i: usize, defender: usize, attack: Attack) {
        if self.attack_source.is_none() && self.martial_can_offer_reckless(i) {
            self.pending_reckless = Some((i, defender, attack));
            self.request(i, 32);
        } else {
            self.roll_attack_against(i, defender, attack);
        }
    }

    pub(in crate::arena2) fn resume_reckless(&mut self) {
        let (i, defender, attack) = self
            .pending_reckless
            .take()
            .expect("pending first attack roll");
        self.roll_attack_against(i, defender, attack);
    }

    fn untrained_armor_attack(&self, i: usize, attack: Attack) -> bool {
        (attack.strength && self.armor_disadvantages_ability(i, 0))
            || (attack.dexterity && self.armor_disadvantages_ability(i, 1))
    }

    fn roll_attack_against(&mut self, i: usize, defender: usize, attack: Attack) {
        let source = self.attack_source.unwrap_or(i);
        if !self.rope_same_space(source, defender) {
            self.complete_attack(source);
            return;
        }
        let approved = self.ward_preapproved_ray.take() == Some((i, defender));
        if !approved
            && self.sanctuary_reject(source, defender, sanctuary::WardRedirect::Attack(attack), 0)
        {
            return;
        }
        self.resolve_sanctuary_attack(i, defender, attack);
    }

    fn resolve_sanctuary_attack(&mut self, i: usize, defender: usize, attack: Attack) {
        if defender >= 4 {
            self.roll_attack_against_object(i, defender, attack);
            return;
        }
        if attack.weapon && attack.ranged && !attack.thrown {
            self.f[i].arrows -= 1;
        }
        let source = self.attack_source.unwrap_or(i);
        if self.suggestion_charmed_by(source, defender) {
            self.complete_attack(source);
            return;
        }
        self.magic_offensive(source);
        let source_point = if !attack.weapon {
            self.magic_attack_source(i)
        } else {
            None
        }
        .or_else(|| self.creature_point(source))
        .expect("attacking creature");
        let pc_source = source < 2;
        let adjacent = if !attack.weapon && self.magic_attack_source(i).is_some() {
            self.footprint(defender)
                .is_some_and(|f| f.squares().any(|p| source_point.dist(p) <= 1))
        } else {
            self.combatant_distance(source, defender)
                .is_some_and(|d| d <= 1)
        };
        let defender_sees_attacker = if pc_source && defender < 2 {
            self.can_see(defender, i)
        } else {
            self.entity_visible(defender, source)
        };
        let attacker_sees_defender = if pc_source && defender < 2 {
            self.can_see(i, defender)
        } else {
            self.entity_visible(source, defender)
        };
        let target_can_act = defender >= 2 || !self.incap(defender);
        let hostile_close = attack.ranged
            && (0..4).any(|nearby| {
                nearby % 2 != i
                    && self.rope_same_space(nearby, source)
                    && (nearby >= 2 || !self.incap(nearby))
                    && self
                        .creature_point(nearby)
                        .is_some_and(|point| point.dist(source_point) <= 1)
                    && if nearby < 2 && source < 2 {
                        self.can_see(nearby, source)
                    } else {
                        self.entity_visible(nearby, source)
                    }
            });
        let target_dodges = if defender < 2 {
            self.f[defender].dodge && self.speed(defender) > 0 && target_can_act
        } else {
            self.entity(defender).is_some_and(|e| e.dodging)
        };
        let magic_advantage = pc_source && self.magic_attack_advantage(i, attack.weapon);
        let bolt_advantage = self.take_guiding_advantage(defender);
        let allied_help = if pc_source {
            self.entity_help_attack(i, defender)
        } else if self.f[i].help_entity_target == Some(defender) {
            self.f[i].help_entity_target = None;
            true
        } else {
            false
        };
        let target_prone = defender < 2 && self.f[defender].prone;
        let target_restrained = defender < 2 && self.magic_restrained(defender);
        let target_paralyzed = (defender < 2 && self.magic_paralyzed(defender))
            || self.sleep_unconscious(defender);
        let weapon_kind = if !pc_source || !attack.weapon || attack.unarmed || attack.improvised {
            None
        } else {
            attack
                .kind_override
                .or_else(|| self.gear_weapon_kind(i, attack.ranged))
        };
        if pc_source { self.f[i].martial.fighter_rogue.maneuver_spent = false; }
        let assassinate = pc_source && self.is_subclass(i, Class::Rogue, 1)
            && self.f[i].build.level >= 3 && self.turns == 1;
        let advantage = assassinate || !defender_sees_attacker
            || self.is_squeezing(defender)
            || target_restrained
            || target_paralyzed
            || allied_help
            || bolt_advantage
            || attacker_sees_defender && self.faerie_outlined(defender)
            || pc_source && self.magic_pack_advantage(i, defender)
            || pc_source && self.divine_duplicate_advantage(i, defender)
            || (target_prone && adjacent)
            || (defender < 2
                && (self.f[defender].reckless
                    || self.f[defender].stunned
                    || self.f[defender].stun_exposed))
            || (pc_source
                && (self.f[i].attack_advantage
                    || self.martial_vex_against(i, defender)
                    || magic_advantage
                    || self.f[i].reckless && attack.strength));
        let disadvantage = !attacker_sees_defender
            || self.blur_disadvantage(source, defender)
            || self.is_squeezing(source)
            || pc_source && self.untrained_armor_attack(i, attack)
            || attack.long_range
            || self.weapon_heavy_disadvantage(i, weapon_kind)
            || self.entity_frightened(source)
            || self.arcane_taunt_disadvantage(source, defender)
            || self.magic_ally_protection(source, defender)
            || (target_prone && !adjacent)
            || (target_dodges && defender_sees_attacker)
            || hostile_close
            || (pc_source
                && (self.f[i].prone
                    || self.f[i].sap
                    || self.f[i].attack_disadvantage
                    || self.magic_attack_disadvantage(i)
                    || self.martial_poisoned(i)
                    || self.magic_restrained(i)));
        if pc_source {
            self.reveal(i);
            self.magic_offensive(i);
            self.f[i].attack_advantage = false;
            self.f[i].attack_disadvantage = false;
            self.f[i].sap = false;
            self.martial_attack_rolled(i, defender);
        }
        if defender < 2 {
            self.f[defender].stun_exposed = false;
        }
        self.magic_end_invisibility_for(source);
        let d20 = self.d20_for(source, advantage, disadvantage);
        let total = d20
            + attack.to_hit
            + if pc_source {
                self.magic_roll_bonus(i)
            } else {
                0
            };
        let ac = self.ac(defender) + self.cover_to(source_point, defender).bonus();
        let automatic_crit =
            d20 == 20 || (pc_source && attack.weapon && d20 >= critical_threshold(self.f[i].build));
        let damage_kind = if !pc_source {
            self.entity_slam_damage_type(i)
        } else if attack.unarmed {
            self.magic_unarmed_type(i)
                .unwrap_or(DamageType::Bludgeoning)
        } else {
            let kind = self.magic_attack_kind(i, attack.weapon)
                .or_else(|| weapon_kind.map(WeaponKind::damage_type))
                .unwrap_or(DamageType::Bludgeoning);
            if !attack.weapon || self.arcane_weapon_spell(i) {
                self.arcane_damage_kind(i, kind)
            } else {
                kind
            }
        };
        let hit = Hit {
            attacker: source,
            defender,
            d20,
            total,
            ac,
            ranged: attack.ranged,
            weapon: attack.weapon,
            physical: is_physical(damage_kind),
            damage_kind,
            crit: automatic_crit || (target_paralyzed && adjacent && d20 != 1 && total >= ac),
            advantage: advantage && !disadvantage,
            disadvantage: disadvantage && !advantage,
            flurry: attack.flurry,
            unarmed: attack.unarmed,
            thrown: attack.thrown,
            weapon_kind,
            improvised: attack.improvised,
            strength: attack.strength,
        };
        if !automatic_crit && (d20 == 1 || total < ac) {
            if !self.divine_offer_guided(hit, attack) {
                self.offer_inspiration_after_miss(hit, attack);
            }
        } else {
            self.confirm_roll_hit(hit, attack);
        }
    }

    fn offer_inspiration_after_miss(&mut self, hit: Hit, attack: Attack) {
        if hit.d20 != 1 && self.bardic_inspiration_available(hit.attacker) {
            self.phase = Phase::InspirationAttack(hit, attack);
        } else {
            self.offer_precision_or_finish(hit, attack);
        }
    }

    fn offer_precision_or_finish(&mut self, hit: Hit, attack: Attack) {
        if self.fighter_precision_available(hit) {
            self.pending_precision = Some((hit, attack));
            self.request(hit.attacker, PRECISION_ATTACK);
        } else {
            self.finish_missed_attack(hit, attack);
        }
    }

    fn finish_missed_attack(&mut self, hit: Hit, attack: Attack) {
        if hit.attacker < 2 {
            self.magic_miss(hit, attack.die, attack.count, attack.bonus);
        }
        self.complete_attack(hit.attacker);
    }

    fn confirm_roll_hit(&mut self, hit: Hit, attack: Attack) {
        if self.offer_elemental_damage(hit.attacker, hit.unarmed)
            || hit.attacker < 2 && self.martial_sacred_request(hit)
        {
            self.pending_sacred = Some((hit, attack));
        } else {
            self.offer_defense(hit, attack);
        }
    }

    fn roll_attack_against_object(&mut self, i: usize, id: usize, attack: Attack) {
        if attack.weapon && attack.ranged && !attack.thrown {
            self.f[i].arrows -= 1;
        }
        let f = &self.f[i];
        let origin = self.fighter_point(i);
        let kind = if attack.unarmed || attack.improvised {
            None
        } else {
            attack
                .kind_override
                .or_else(|| self.gear_weapon_kind(i, attack.ranged))
        };
        let advantage = f.attack_advantage || f.reckless && attack.strength;
        let disadvantage = self.untrained_armor_attack(i, attack)
            || f.prone
            || f.sap
            || f.attack_disadvantage
            || attack.long_range
            || self.weapon_heavy_disadvantage(i, kind)
            || self.martial_poisoned(i);
        let crit_min = if attack.weapon {
            critical_threshold(f.build)
        } else {
            20
        };
        let damage_kind = self
            .magic_attack_kind(i, attack.weapon)
            .or_else(|| kind.map(WeaponKind::damage_type))
            .unwrap_or(DamageType::Bludgeoning);
        let damage_kind = if !attack.weapon || self.arcane_weapon_spell(i) {
            self.arcane_damage_kind(i, damage_kind)
        } else {
            damage_kind
        };
        let to_hit = attack.to_hit + self.magic_roll_bonus(i);
        self.reveal(i);
        self.magic_offensive(i);
        self.f[i].attack_advantage = false;
        self.f[i].attack_disadvantage = false;
        self.f[i].sap = false;
        self.martial_attack_rolled(i, id);
        self.f[i].martial.fighter_rogue.maneuver_spent = false;
        self.magic_end_invisibility_for(i);
        let Some((d20, total, ac)) =
            self.object_attack_roll(i, origin, id, to_hit, advantage, disadvantage)
        else {
            self.complete_attack(i);
            return;
        };
        let hit = Hit {
            attacker: i,
            defender: id,
            d20,
            total,
            ac,
            ranged: attack.ranged,
            weapon: attack.weapon,
            physical: is_physical(damage_kind),
            damage_kind,
            crit: d20 >= crit_min,
            advantage: advantage && !disadvantage,
            disadvantage: disadvantage && !advantage,
            flurry: attack.flurry,
            unarmed: attack.unarmed,
            thrown: attack.thrown,
            weapon_kind: kind,
            improvised: attack.improvised,
            strength: attack.strength,
        };
        if d20 != 1 && (hit.crit || total >= ac) {
            self.finish_object_hit(hit, attack);
        } else if self.fighter_precision_available(hit) {
            self.pending_precision = Some((hit, attack));
            self.request(i, PRECISION_ATTACK);
        } else {
            self.complete_attack(i);
        }
    }

    fn finish_object_hit(&mut self, hit: Hit, attack: Attack) {
        let Hit { attacker: i, defender: id, crit: critical, damage_kind, .. } = hit;
        if !attack.weapon {
            self.projectile_hit(i, id);
        }
        let dice = attack.count * if critical && attack.die > 1 { 2 } else { 1 };
        let mut amount = attack.bonus;
        for _ in 0..dice {
            let raw = if attack.die == 1 {
                1
            } else {
                self.roll(attack.die)
            };
            if !attack.weapon && attack.die > 1 {
                self.projectile_object_die(i, raw);
            }
            amount += raw;
        }
        if self.offer_elemental_damage(i, attack.unarmed) {
            self.pending_elemental_object = Some((i, id, amount.max(0), damage_kind));
            return;
        }
        let favor =
            self.magic_divine_favor_damage(i, critical, attack.weapon && !attack.unarmed);
        let dealt = self.damage_object(id, amount.max(0), damage_kind);
        if favor > 0 {
            self.damage_object(id, favor, DamageType::Radiant);
        }
        let extra = self.magic_true_strike_extra(i, critical);
        if extra > 0 {
            self.damage_object(id, extra, self.arcane_damage_kind(i, DamageType::Radiant));
        }
        if !self.fighter_psionic_strike_request(hit, dealt) {
            self.complete_attack(i);
        }
    }

    pub(in crate::arena2) fn resume_elemental_damage(&mut self, kind: Option<DamageType>) {
        if let Some((attacker, object, damage, original)) = self.pending_elemental_object.take() {
            self.damage_object(object, damage, kind.unwrap_or(original));
            self.complete_attack(attacker);
            return;
        }
        let (mut hit, attack) = self.pending_sacred.take().expect("pending Elemental Strikes");
        if let Some(kind) = kind {
            hit.damage_kind = kind;
            hit.physical = false;
        }
        self.offer_defense(hit, attack);
    }

    pub(in crate::arena2) fn resume_sacred_weapon(&mut self, radiant: bool) {
        let (mut hit, attack) = self.pending_sacred.take().expect("pending Sacred Weapon");
        if radiant {
            hit.damage_kind = DamageType::Radiant;
            hit.physical = false;
        }
        self.offer_defense(hit, attack);
    }

    fn valor_can_defend(&self, hit: Hit) -> bool {
        !hit.crit
            && hit.d20 != 0
            && self.valor_inspiration_available(hit.defender)
            && if hit.defender < 2 {
                self.can_react(hit.defender)
            } else {
                !self.magic_no_reactions(hit.defender)
                    && self.entity_reaction_budget(hit.defender).is_some()
            }
    }

    fn offer_defense(&mut self, hit: Hit, attack: Attack) {
        let defender = hit.defender;
        if self.mirror_image_intercepts(hit) {
            self.complete_attack(hit.attacker);
            return;
        }
        if self.valor_can_defend(hit)
            || defender < 2 && self.can_react(defender)
                && (self.magic_defend(defender, hit) || self.martial_defend(defender, hit))
        {
            self.phase = Phase::Defend { hit, attack };
        } else {
            self.resolve_defense(hit, attack, Defense::Pass);
        }
    }

    pub(super) fn resume_magic_shield(&mut self, blocks_hit: bool) {
        let (hit, attack) = self
            .pending_shield_hit
            .take()
            .expect("pending Shield reaction");
        if blocks_hit {
            if hit.attacker < 2 && hit.d20 != 0 {
                self.magic_miss(hit, attack.die, attack.count, attack.bonus);
            }
            self.complete_attack(hit.attacker);
        } else {
            self.resolve_defense(hit, attack, Defense::Pass);
        }
    }

    fn resolve_defense(&mut self, hit: Hit, attack: Attack, defense: Defense) {
        let attacker = hit.attacker;
        let defender = hit.defender;
        if matches!(defense, Defense::Magic) && self.magic_defense(defender, hit) {
            if hit.attacker < 2 && hit.d20 != 0 {
                self.magic_miss(hit, attack.die, attack.count, attack.bonus);
            }
            self.complete_attack(attacker);
            return;
        }
        if hit.d20 == 0 {
            let before = self.footprint(defender);
            self.damage_typed(defender, attack.bonus, DamageType::Force);
            self.magic_blessing_at_zero(Some(attacker), defender, before);
            self.complete_attack(attacker);
            return;
        }
        if attacker < 2 && !attack.weapon {
            self.projectile_hit(attacker, defender);
        }
        let dice = attack.count * if hit.crit && attack.die > 1 { 2 } else { 1 };
        let mut damage = attack.bonus;
        for _ in 0..dice {
            let raw = if attack.die == 1 {
                1
            } else {
                self.roll(attack.die)
            };
            if attack.die > 1 {
                self.projectile_die(hit, raw);
            }
            damage += raw;
        }
        damage = damage.max(0);
        if attacker < 2 {
            damage += self.martial_damage(attacker, hit);
        }
        self.pending_hit = Some((hit, damage, defense));
        if attacker < 2 && self.magic_rider(attacker, hit) {
            self.phase = Phase::Rider {
                hit,
                damage,
                magic: true,
            };
        } else {
            self.offer_valor_damage();
        }
    }

    pub(super) fn resume_magic_rider(&mut self) {
        self.offer_valor_damage();
    }

    fn offer_valor_damage(&mut self) {
        let (hit, _, _) = self.pending_hit.expect("pending attack");
        if hit.d20 != 0 && self.valor_inspiration_available(hit.attacker) {
            self.phase = Phase::ValorDamage;
        } else {
            self.begin_martial_hit();
        }
    }

    fn begin_martial_hit(&mut self) {
        let (hit, damage, _) = self.pending_hit.expect("pending attack");
        let attacker = hit.attacker;
        let defender = hit.defender;
        if self.subclass_divine_fury(hit) {
            return;
        }
        if attacker < 2 && self.martial_hit_request(hit, damage) {
            return;
        }
        if attacker < 2
            && (defender < 2 || self.f[attacker].build.class == Class::Monk)
            && self.martial_rider(attacker, hit)
        {
            self.phase = Phase::Rider {
                hit,
                damage,
                magic: false,
            };
            return;
        }
        self.finish_hit(0, 0, 0, false);
    }

    pub(super) fn finish_hit(
        &mut self,
        weapon_extra: i32,
        radiant_extra: i32,
        psychic_extra: i32,
        radiant_weapon: bool,
    ) {
        let (hit, base, defense) = self.pending_hit.take().expect("pending attack hit");
        let kind = if radiant_weapon {
            DamageType::Radiant
        } else {
            hit.damage_kind
        };
        let mut damage_parts = [
            (base + weapon_extra, kind),
            (radiant_extra, DamageType::Radiant),
            (psychic_extra, DamageType::Psychic),
            self.take_divine_fury(hit),
            (self.magic_crusaders_mantle(hit), DamageType::Radiant),
            (0, DamageType::Force),
            (0, DamageType::Radiant),
            (0, DamageType::Poison),
            (0, DamageType::Radiant),
        ];
        if hit.attacker < 2 {
            damage_parts[5..].copy_from_slice(&self.magic_damage_parts(hit.attacker, hit));
        }
        let defender = hit.defender;
        let parry = matches!(defense, Defense::Martial) && self.is_subclass(defender, Class::Fighter, 1);
        if parry {
            let total: i32 = damage_parts.iter().map(|&(amount, _)| amount.max(0)).sum();
            let mut reduction = total - self.fighter_reduce_damage(defender, total);
            for (part, _) in &mut damage_parts {
                let used = reduction.min((*part).max(0));
                *part -= used;
                reduction -= used;
            }
        }
        let mut damage = self.damage_parts_after_resistance(defender, &mut damage_parts);
        let deflected =
            matches!(defense, Defense::Martial) && self.f[hit.defender].build.class == Class::Monk;
        if matches!(defense, Defense::Martial) && !parry {
            damage = self.martial_defense(hit.defender, hit, damage);
        }
        if damage > 0
            && hit.attacker < 2
            && hit.defender < 2
            && self.magic_can_cut_damage(hit.defender, hit.attacker)
        {
            self.pending_damage = Some((hit, damage, deflected));
            self.phase = Phase::CutDamage(hit.defender);
        } else {
            self.apply_attack_damage(hit, damage, deflected);
        }
    }

    fn apply_attack_damage(&mut self, hit: Hit, damage: i32, deflected: bool) {
        if self.offer_hand_of_harm(hit, damage) {
            self.pending_damage = Some((hit, damage, deflected));
            return;
        }
        self.commit_attack_damage(hit, damage, deflected);
    }

    pub(in crate::arena2) fn resume_hand_of_harm(&mut self, accepted: bool) {
        let (hit, damage, deflected) = self.pending_damage.take().expect("pending Hand of Harm");
        let extra = if accepted { self.hand_of_harm_damage(hit) } else { 0 };
        self.commit_attack_damage(hit, damage + extra, deflected);
    }

    fn commit_attack_damage(&mut self, hit: Hit, damage: i32, deflected: bool) {
        let defender = hit.defender;
        let before = self.footprint(defender);
        let damage = if defender < 2 {
            let dealt = self.commit_resolved_damage(defender, damage);
            self.magic_hurt_from(hit.attacker, defender, dealt);
            dealt
        } else {
            self.entity_damage_resolved(defender, damage)
        };
        if hit.attacker < 2 {
            self.martial_after_hit(hit, damage);
            self.shape_bite_prone(hit);
        }
        self.magic_blessing_at_zero(Some(hit.attacker), defender, before);
        self.magic_apply_repel(hit);
        if !self.fighter_psionic_strike_request(hit, damage)
            && !self.offer_elemental_movement(hit, damage)
            && !self.martial_after_hit_request(hit, damage, deflected)
        {
            self.complete_attack(hit.attacker);
        }
    }

    pub(super) fn complete_attack(&mut self, attacker: usize) {
        self.after_attack(attacker % 2);
    }

    fn after_attack(&mut self, i: usize) {
        self.phase = Phase::Main;
        self.check_ko();
        if self.finished() {
            return;
        }
        let reaction_attack_in_motion = self.pending_move.is_some()
            || self.pending_spell_move.is_some()
            || self.ready_pending_move.is_some()
            || self.attack_resume.is_some();
        if self.attack_source.is_none() && !reaction_attack_in_motion && self.can_see(1 - i, i) {
            self.offer_readied(1 - i, ReadyTrigger::Attack);
            if !matches!(self.phase, Phase::Main) {
                self.ready_resume.push(ReadyContinuation::AfterAttack(i));
                return;
            }
        }
        self.continue_after_attack(i, reaction_attack_in_motion, true);
    }

    fn cancel_sanctuary_attack(&mut self, source: usize) {
        self.phase = Phase::Main;
        self.continue_after_attack(source % 2, false, false);
    }

    fn continue_after_attack(&mut self, i: usize, attack_in_motion: bool, rolled: bool) {
        if self.martial_continue_flurry(i) {
            return;
        }
        if self.bonus_attacks.is_some_and(|(source, ..)| source == i) {
            let (_, count, ranged, target) = self.bonus_attacks.take().expect("pending attacks");
            if !self.incap(i) {
                let candidates =
                    self.attack_targets(i, if ranged { Act::Ranged } else { Act::Unarmed });
                if !ranged && candidates.count_ones() > 1 {
                    self.phase = Phase::FlurryTarget { actor: i, count };
                    return;
                }
                if candidates != 0 {
                    let target = if candidates & (1 << target) != 0 {
                        target
                    } else {
                        candidates.trailing_zeros() as usize
                    };
                    self.flurry_strike(i, target, ranged, count);
                    return;
                }
            }
        }
        let companion = self.attack_source.take();
        let is_companion_attack = companion.is_some();
        if let Some(subject) = companion.filter(|_| rolled) {
            self.notify_companion_event(subject, ReadyTrigger::Attack);
        }
        if let Some(phase) = self.attack_resume.take() {
            self.entity_resume_after_attack(phase);
            return;
        }
        if !is_companion_attack {
            self.magic_continue(i);
        }
        if rolled && attack_in_motion && matches!(self.phase, Phase::Main) && self.can_see(1 - i, i)
        {
            self.offer_readied(1 - i, ReadyTrigger::Attack);
        }
        if matches!(self.phase, Phase::Main) {
            self.resume_queued_movement();
        }
    }
}

impl Battle {
    fn flush_area_request(&mut self) {
        if self.area_resume.is_none() && !self.finished() {
            let resume = self.phase;
            if self.area_request_damage() {
                self.area_resume = Some(resume);
            }
        }
    }

    pub(in crate::arena2) fn bonus_unarmed_maneuver(
        &mut self,
        source: usize,
        target: usize,
        maneuver: UnarmedManeuver,
    ) {
        self.reveal(source);
        self.phase = Phase::ManeuverSave {
            source,
            target,
            maneuver,
            opportunity: false,
            bonus_strike: true,
        };
    }

    pub(in crate::arena2) fn martial_bonus_hit(
        &mut self,
        source: usize,
        target: usize,
        flurry: bool,
    ) {
        let mut attack = self.unarmed_spec(source);
        attack.flurry = flurry;
        self.start_attack_against(source, target, attack);
    }

    fn rogue_blade_spec(&self, i: usize, target: usize, choice: Act) -> Attack {
        let ranged = matches!(choice, Act::Special4 | Act::Special6);
        let stat = self.f[i].build.stats[1];
        Attack {
            die: if matches!(choice, Act::Special5 | Act::Special6) { 4 } else { 6 },
            count: 1,
            bonus: stat,
            to_hit: self.f[i].build.pb + stat,
            ranged,
            weapon: true,
            flurry: false,
            unarmed: false,
            thrown: ranged,
            long_range: ranged && self.combatant_distance(i, target)
                    .or_else(|| self.object(target).map(|object| self.fighter_point(i).dist(object.point)))
                    .is_some_and(|distance| distance > 12),
            improvised: false,
            kind_override: Some(WeaponKind::PsychicBlade),
            strength: false,
            dexterity: true,
        }
    }

    fn fighter_rogue_phase_step(&mut self, actor: usize, phase: i32, choice: Act) {
        let accepted = choice != Act::Pass;
        match phase {
            PRECISION_ATTACK => {
                let (mut hit, attack) = self.pending_precision.take().expect("missed attack");
                if accepted {
                    hit.total += self.fighter_spend_maneuver(actor);
                }
                if hit.defender >= super::objects::FIRST_OBJECT {
                    if hit.total >= hit.ac {
                        self.finish_object_hit(hit, attack);
                    } else {
                        self.complete_attack(actor);
                    }
                } else if hit.total >= hit.ac {
                    hit.crit |= (hit.defender < 2 && self.magic_paralyzed(hit.defender)
                        || self.sleep_unconscious(hit.defender))
                        && self.combatant_distance(actor, hit.defender).is_some_and(|distance| distance <= 1);
                    self.confirm_roll_hit(hit, attack);
                } else {
                    self.finish_missed_attack(hit, attack);
                }
            }
            TRIP_ATTACK => self.hit_decision_step(actor, phase, choice),
            PSIONIC_STRIKE => self.fighter_finish_psionic_strike(actor, accepted),
            PSIONIC_KNACK => self.rogue_finish_knack(actor, accepted),
            PSYCHIC_BLADE_TARGET => {
                let action = self.f[actor].martial.fighter_rogue.blade_action.take().expect("blade choice");
                if accepted {
                    if matches!(action, Act::Special5 | Act::Special6) {
                        self.bonus = false;
                    } else {
                        self.spend_attack(actor);
                    }
                    self.note_rogue_blade(actor);
                    let target = choice as usize;
                    self.start_attack_against(actor, target, self.rogue_blade_spec(actor, target, action));
                }
            }
            TELEKINETIC_SUBJECT..=TELEKINETIC_HEIGHT | TELEKINETIC_CONSENT => self.telekinetic_step(actor, phase, choice),
            _ => unreachable!("invalid Fighter/Rogue phase"),
        }
    }

    fn decision_mask(&self, actor: usize, id: i32) -> u128 {
        match id {
            PRECISION_ATTACK..=TELEKINETIC_HEIGHT | TELEKINETIC_CONSENT => self.fighter_rogue_phase_mask(actor, id),
            super::origins::MEND_TARGET => self.mend_target_mask(actor),
            87 => self.study_mask(actor),
            13 | 16..=18 => self.traversal_decision_mask(actor, id),
            20..=33 | 90..=91 | 113..=114 | 180..=181 => self.martial_decision_mask(actor, id),
            141..=151 => self.subclass_decision_mask(actor, id),
            super::magic::PHANTASM_CELL | super::magic::PHANTASM_HEIGHT =>
                self.phantasm_placement_mask(actor, id),
            183..=186 => self.bard_ranger_mask(actor, id),
            34..=49 => self.magic_decision_mask(actor, id),
            66..=72 => self.gear_decision_mask(actor, id),
            73..=77 | 117 | 121 => self.spell_movement_mask(actor, id),
            78..=79 | 97 => self.class_magic_mask_phase(actor, id),
            200..=202 => self.roster_field_mask(actor, id),
            160..=169 => self.divine_decision_mask(actor, id),
            177 => self.area_decision_mask(actor, id),
            178 | 179 => self.arcane_initiative_mask(actor, id),
            172..=179 => self.arcane_choice_mask(actor, id),
            92..=96 | 104..=105 => self.area_decision_mask(actor, id),
            100 | 102 | 115 => self.shape_mask(actor, id),
            107..=111 => self.ready_spell_mask(actor, id),
            112 => self.armor_decision_mask(actor),
            super::footprint::TRANSFORM_FOOTPRINT => self.footprint_transform_mask(actor),
            80..=85 => self.projectile_mask(actor, id),
            sanctuary::RETARGET => self.sanctuary_mask(),
            _ => 0,
        }
    }

    fn decision_step(&mut self, actor: usize, id: i32, choice: Act) {
        match id {
            PRECISION_ATTACK..=TELEKINETIC_HEIGHT | TELEKINETIC_CONSENT => self.fighter_rogue_phase_step(actor, id, choice),
            super::origins::MEND_TARGET => self.begin_mend(actor, choice),
            87 => self.command_study(actor, choice),
            13 | 16..=18 => self.traversal_decision_step(actor, id, choice),
            20..=33 | 90..=91 | 113..=114 | 180..=181 => self.martial_decision_step(actor, id, choice),
            141..=151 => self.subclass_decision_step(actor, id, choice),
            super::magic::PHANTASM_CELL | super::magic::PHANTASM_HEIGHT =>
                self.phantasm_placement_step(actor, id, choice),
            183..=186 => self.bard_ranger_step(actor, id, choice),
            34..=49 => self.magic_decision_step(actor, id, choice),
            66..=72 => self.gear_decision_step(actor, id, choice),
            73..=77 | 117 | 121 => self.spell_movement_step(actor, id, choice),
            78..=79 | 97 => self.class_magic_step_phase(actor, id, choice),
            200..=202 => self.roster_field_step(actor, id, choice),
            160..=169 => self.divine_decision_step(actor, id, choice),
            177 => self.area_decision_step(actor, id, choice),
            178 | 179 => self.arcane_initiative_step(actor, id, choice),
            172..=179 => self.arcane_choice_step(actor, id, choice),
            92..=96 | 104..=105 => self.area_decision_step(actor, id, choice),
            100 | 102 | 115 => self.shape_step(actor, id, choice),
            107..=111 => self.ready_spell_step(actor, id, choice),
            112 => self.armor_decision_step(actor, choice),
            super::footprint::TRANSFORM_FOOTPRINT => self.footprint_transform_step(actor, choice),
            80..=85 => self.projectile_step(actor, id, choice),
            sanctuary::RETARGET => self.sanctuary_step(choice),
            _ => unreachable!("unassigned decision id"),
        }
    }

    pub(super) fn raw_choice_label(&self, viewer: usize, a: Act) -> String {
        if self.actor() == Some(viewer) {
            if self.save_choice.concentration_menu {
                return if a == Act::DropConcentration {
                    "Drop concentration"
                } else {
                    "Return"
                }
                .into();
            }
            if self.concentration_menu_action() == Some(a) {
                return "Concentration options".into();
            }
            if a == Act::DropConcentration && self.optional_concentrator() == Some(viewer) {
                return "Drop concentration".into();
            }
            if let Some(label) = self.save_choice_label(a) {
                return label;
            }
        }
        if self.save_choice_actor().is_some() || self.save_choice.concentration_menu {
            return format!("{a:?}");
        }
        let label = match self.phase {
            Phase::Main if a == super::magic::PHANTASM_DAMAGE
                && self.phantasmal_damage_available(viewer) => return "Deal Phantasmal Force damage".into(),
            Phase::Decision { id: id @ (super::magic::PHANTASM_CELL | super::magic::PHANTASM_HEIGHT), .. } =>
                return self.phantasm_placement_label(id, a),
            Phase::Decision {actor: _, id: phase @ 200..=202} => return self.roster_field_label(phase, a),
            Phase::Decision { actor, id: id @ (PRECISION_ATTACK..=TELEKINETIC_HEIGHT | TELEKINETIC_CONSENT) } =>
                self.fighter_rogue_phase_label(actor, id, a),
            Phase::Opportunity if a == Act::Special3 => return "Opportunity Attack with Psychic Blade".into(),
            Phase::Opportunity if a == Act::Opportunity => {
                return if self.gear_wielded(viewer, false) {
                    "Opportunity weapon attack"
                } else {
                    "Opportunity unarmed strike"
                }
                .into()
            }
            Phase::TargetRow(_) => return format!("Row {}", a as u8 + 1),
            Phase::TargetColumn(_, y) => {
                return format!("Cell {}{}", char::from(b'A' + a as u8), y + 1)
            }
            Phase::TargetAltitude(..) => return format!("{} ft above the floor", a as u8 * 5),
            Phase::CreatureTarget(_) if a == Act::Search => {
                return "Guess an unobserved cell".into()
            }
            Phase::CreatureTarget(_) | Phase::FlurryTarget { .. } => {
                return self.creature_label(a as usize)
            }
            Phase::HelpTarget => return self.primary_help_label(a as usize),
            Phase::Decision {
                actor,
                id: 13 | 16..=18,
            } => self.traversal_decision_label(actor, self.phase_id(), a),
            Phase::Decision {
                id: super::origins::MEND_TARGET,
                ..
            } => self.mend_target_label(a),
            Phase::Decision {
                actor,
                id: 73..=77 | 117 | 121,
            } => self.spell_movement_label(actor, self.phase_id(), a),
            Phase::Decision {
                actor,
                id: 78..=79 | 97,
            } => self.class_magic_label(actor, self.phase_id(), a),
            Phase::Decision { id: 177, actor } => self.area_decision_label(actor, self.phase_id(), a),
            Phase::Decision { id: id @ (178 | 179), actor } => Some(self.arcane_initiative_label(actor, id, a)),
            Phase::Decision { id: 172..=179, actor } => self.arcane_choice_label(actor, self.phase_id(), a),
            Phase::Decision {
                actor,
                id: 107..=111,
            } => self.ready_spell_label(actor, self.phase_id(), a),
            Phase::Decision { actor, id: 112 } => self.armor_decision_label(actor, a),
            Phase::Decision { actor, id: 160..=169 } => self.divine_decision_label(actor, self.phase_id(), a),
            Phase::Decision { actor, id: 87 } => self.study_label(actor, a),
            Phase::Decision {
                actor,
                id: 100 | 102 | 115,
            } => self.shape_label(actor, self.phase_id(), a),
            Phase::Decision {
                actor,
                id: super::footprint::TRANSFORM_FOOTPRINT,
            } => self.footprint_transform_label(actor, a),
            Phase::Decision {
                actor,
                id: 92..=96 | 104..=105,
            } => self.area_decision_label(actor, self.phase_id(), a),
            Phase::Decision { actor, id: 80..=85 } => {
                self.projectile_label(actor, self.phase_id(), a)
            }
            Phase::Decision {
                id: sanctuary::RETARGET,
                ..
            } => self.sanctuary_label(a),

            Phase::Decision { actor, id: 66..=72 } => {
                self.gear_decision_label(actor, self.phase_id(), a)
            }
            Phase::Decision {
                actor,
                id: 20..=33 | 90..=91 | 113..=114 | 180..=181,
            } => self.martial_decision_label(actor, self.phase_id(), a),
            Phase::Decision { actor, id: id @ 183..=186 } => {
                return self.bard_ranger_label(actor, id, a)
            }
            Phase::Decision { actor, id: 141..=151 } => {
                Some(self.subclass_decision_label(actor, self.phase_id(), a))
            }
            Phase::Decision { actor, id: 34..=49 } => {
                self.magic_decision_label(actor, self.phase_id(), a)
            }
            Phase::Entity(_) => self.entity_choice_label(viewer, a),
            Phase::ReadyTrigger => return READY_TRIGGERS[a as usize].label().into(),
            Phase::ReadyAction(_) if a == Act::Dash => return "Move up to Speed".into(),
            Phase::ReadyAction(_) if self.divine_ready_mask(self.owner) & a.bit() != 0 => {
                return self.divine_ready_label(self.owner).expect("legal readied divine feature");
            }
            Phase::ReadyTarget { .. } | Phase::ReadyGroundHeight { .. } if a == Act::Other8 => {
                return "Do not choose a target".into()
            }
            Phase::ReadyTarget {
                actor,
                action: Act::Study,
            } => return self.study_label(actor, a).unwrap_or_default(),
            Phase::ReadyGroundHeight { .. } => return format!("Height {} ft", a as u8 * 5),
            Phase::ReadyTarget {
                actor,
                action: action @ (Act::Special3 | Act::Special5),
            } if self.can_ready_ground(actor, action) => {
                let cell = Self::ready_ground_cell(a as usize);
                return format!(
                    "Center {} at {}{}",
                    self.main_choice_label(actor, action),
                    char::from(b'A' + cell.x as u8),
                    cell.y + 1
                );
            }
            Phase::ReadyTarget {
                action: Act::Utilize,
                ..
            } => {
                let cell = Self::ready_ground_cell(a as usize);
                return format!(
                    "Spread Caltrops at {}{}",
                    char::from(b'A' + cell.x as u8),
                    cell.y + 1
                );
            }
            Phase::ReadyTarget {
                action: Act::Help, ..
            } => return self.primary_help_label(a as usize),
            Phase::ReadyTarget { .. } => return self.creature_label(a as usize),
            Phase::ReadyRelease(_) if a == Act::Ready => return "Take readied reaction".into(),
            Phase::ReadyMove { .. } if a == Act::End => return "Finish readied movement".into(),
            Phase::TacticalMind(..) if a == Act::Ready => return "Try Tactical Mind (d10)".into(),
            Phase::Rider { hit, .. } if self.magic_can_repel(hit) => {
                return match a {
                    Act::N => "Push 5 feet",
                    Act::Rider => "Push 10 feet",
                    _ => "Decline Repelling Blast",
                }
                .into();
            }
            Phase::Rider {
                hit, magic: true, ..
            } if a == Act::Rider => {
                return self.magic_rider_label(hit.attacker).into();
            }
            Phase::CutCheck { .. } if a == Act::Defend => {
                return "Cutting Words (ability check)".into()
            }
            Phase::ValorDamage if a == Act::Rider => return "Combat Inspiration (attack damage)".into(),
            Phase::Defend { .. } if a == Act::Rider => return "Combat Inspiration (Armor Class)".into(),
            Phase::CutDamage(_) if a == Act::Defend => return "Cutting Words (damage roll)".into(),
            Phase::ManeuverSave { .. } if a == Act::Pass => return "Voluntarily fail save".into(),
            Phase::InspirationAttack(..) | Phase::InspirationCheck(..) if a == Act::Defend => {
                return "Roll Bardic Inspiration die".into();
            }
            Phase::ManeuverSave { .. } => {
                return match a as usize {
                    0 => "Strength saving throw",
                    1 => "Dexterity saving throw",
                    _ => "Unknown saving throw",
                }
                .into()
            }
            _ => None,
        };
        label.unwrap_or_else(|| self.main_choice_label(viewer, a))
    }

    fn main_choice_label(&self, viewer: usize, a: Act) -> String {
        let class = self.f[viewer].build.class;
        self.rope_action_label(viewer, a).map(str::to_owned)
            .or_else(|| (a == super::bard_ranger::RESTORE_BEGUILING
                && self.beguiling_restore_available(viewer))
                .then(|| "Restore Beguiling Magic".into()))
            .or_else(|| self.mantle_available(viewer)
            .then_some("Mantle of Inspiration")
            .filter(|_| a == super::bard_ranger::MANTLE)
            .map(str::to_owned))
            .or_else(|| self.divine_main_label(viewer, a))
            .or_else(|| self.traversal_label(viewer, a))
            .or_else(|| Self::weapon_options_label(a).map(str::to_owned))
            .or_else(|| self.area_action_label(a))
            .or_else(|| self.armor_label(viewer, a))
            .or_else(|| self.origin_label(viewer, a))
            .or_else(|| {
                (108..120)
                    .contains(&(a as usize))
                    .then(|| self.gear_label(viewer, a))
            })
            .or_else(|| super::entities::entity_action_name(class, a).map(str::to_owned))
            .or_else(|| self.martial_subclass_label(viewer, a).map(str::to_owned))
            .or_else(|| self.fighter_rogue_label(viewer, a))
            .or_else(|| super::martial::label(class, a).map(str::to_owned))
            .or_else(|| (a == Act::Magic21 && self.is_subclass(viewer, Class::Druid, 2))
                .then(|| "Redirect Gust of Wind".to_owned()))
            .or_else(|| self.arcane_action_label(viewer, a))
            .or_else(|| self.prepared_spell(viewer, a).map(|spell| spell.label().to_owned()))
            .or_else(|| super::magic::label(class, a).map(str::to_owned))
            .unwrap_or_else(|| match a {
                Act::N => "North".into(),
                Act::NE => "North-east".into(),
                Act::E => "East".into(),
                Act::SE => "South-east".into(),
                Act::S => "South".into(),
                Act::SW => "South-west".into(),
                Act::W => "West".into(),
                Act::NW => "North-west".into(),
                Act::End => "End turn".into(),
                Act::Melee => "Melee attack".into(),
                Act::Ranged => "Ranged attack".into(),
                Act::ShovePush => "Shove (push)".into(),
                Act::ShoveProne => "Shove (knock prone)".into(),
                Act::EscapeAthletics => "Escape grapple (Athletics)".into(),
                Act::EscapeAcrobatics => "Escape grapple (Acrobatics)".into(),
                Act::ReleaseGrapple => "Release grapple".into(),
                Act::DropProne => "Drop prone".into(),
                Act::Drag => if self.f[viewer].dragging {
                    "Stop dragging"
                } else {
                    "Drag grappled foe"
                }
                .into(),
                Act::DropConcentration => "Drop concentration".into(),
                _ => format!("{a:?}"),
            })
    }

    pub(super) fn resolve_successful_check(
        &mut self,
        i: usize,
        result: FailedCheck,
        tactical: bool,
    ) {
        if self.magic_can_cut_damage(1 - i, i) {
            self.phase = Phase::CutCheck {
                source: i,
                result,
                tactical,
            };
        } else {
            if tactical {
                self.f[i].resource -= 1;
            }
            self.apply_check(i, result.check, result.total);
        }
    }

    pub(in crate::arena2) fn perform_common_check(&mut self, actor: usize, check: Check) {
        if let Some(result) = self.try_check(actor, check) {
            if self.bardic_inspiration_available(actor) {
                self.phase = Phase::InspirationCheck(actor, result);
            } else {
                self.check_tactical_mind(actor, result);
            }
        }
    }

    fn check_tactical_mind(&mut self, actor: usize, result: FailedCheck) {
        if self.rogue_knack_request(actor, result) { return; }
        if self.f[actor].build.class == Class::Fighter
            && self.f[actor].build.level >= 2
            && self.f[actor].resource > 0
        {
            self.phase = Phase::TacticalMind(actor, result);
        }
    }

    fn primary_help_targets(&self, actor: usize) -> u128 {
        let Some(ally) = self.entity(2 + actor) else {
            return 0;
        };
        let mut mask = if matches!(ally.kind, super::entities::EntityKind::Steed { .. }) {
            self.attack_targets(actor, Act::Unarmed)
        } else {
            0
        };
        if ally.martial.grappled.is_some()
            && self
                .combatant_distance(actor, 2 + actor)
                .is_some_and(|d| d <= 1)
        {
            let build = self.f[actor].build;
            for (choice, skill) in [(16, Skill::Athletics), (17, Skill::Acrobatics)] {
                if super::origins::skill_bonus(build, skill, skill.ability())
                    - build.stats[skill.ability()]
                    >= build.pb
                {
                    mask |= ACTS[choice].bit();
                }
            }
        }
        mask
    }

    fn primary_help_label(&self, option: usize) -> String {
        match option {
            16 => "Help companion escape with Athletics".into(),
            17 => "Help companion escape with Acrobatics".into(),
            _ => self.creature_label(option),
        }
    }

    fn apply_primary_help(&mut self, actor: usize, target: usize) {
        match target {
            16 => self.companion_check_help[actor] = Some(Skill::Athletics),
            17 => self.companion_check_help[actor] = Some(Skill::Acrobatics),
            _ => self.f[actor].help_entity_target = Some(target),
        }
    }

    pub(super) fn consume_companion_check_help(&mut self, side: usize, skill: Skill) -> bool {
        if self.companion_check_help[side] == Some(skill) {
            self.companion_check_help[side] = None;
            true
        } else {
            false
        }
    }

    fn common_mask(&self, i: usize) -> u128 {
        use Act::*;
        let f = &self.f[i];
        let mut mask = 0;
        if self.creature_targets(ReleaseGrapple) != 0 {
            mask |= ReleaseGrapple.bit() | Drag.bit();
        }
        if f.concentration.is_some() {
            mask |= DropConcentration.bit();
        }
        if f.prone {
            let speed = self.speed(i);
            if speed > 0 && !self.magic_laughter(i) && self.normal_movement_left() >= (speed + 1) / 2 {
                mask |= Stand.bit();
            }
        } else {
            mask |= DropProne.bit();
        }
        if self.incap(i) || self.magic_quiet_turn(i) {
            return mask;
        }
        if self.action {
            mask |= Search.bit();
            mask |= self.study_main_mask(i);
            if self.primary_help_targets(i) != 0 {
                mask |= Help.bit();
            }
            if self.can_react(i) {
                mask |= Ready.bit();
            }
            if self.can_hide(i) {
                mask |= Hide.bit();
            }
            if f.grappled.is_some() {
                mask |= EscapeAthletics.bit() | EscapeAcrobatics.bit();
            }
        }
        if self.action || self.attacks > 0 {
            for action in [Unarmed, ShovePush, ShoveProne, Grapple] {
                if self.creature_targets(action) != 0
                    || action == Unarmed && self.observed_pos(i, 1 - i).is_none()
                {
                    mask |= action.bit();
                }
            }
        }
        mask
    }

    fn common_maneuver(choice: Act) -> Option<UnarmedManeuver> {
        match choice {
            Act::Grapple => Some(UnarmedManeuver::Grapple),
            Act::ShovePush => Some(UnarmedManeuver::ShovePush),
            Act::ShoveProne => Some(UnarmedManeuver::ShoveProne),
            _ => None,
        }
    }

    fn common_step(&mut self, i: usize, choice: Act) {
        use Act::*;
        match choice {
            Hide | Search | EscapeAthletics | EscapeAcrobatics => {
                self.action = false;
                let check = match choice {
                    Hide => Check::Hide,
                    Search => Check::Search,
                    EscapeAthletics => Check::Escape(Skill::Athletics),
                    EscapeAcrobatics => Check::Escape(Skill::Acrobatics),
                    _ => unreachable!(),
                };
                self.perform_common_check(i, check);
            }
            DropProne => self.f[i].prone = true,
            Stand => {
                self.moved += (self.speed(i) + 1) / 2;
                self.f[i].prone = false;
            }
            DropConcentration => self.drop_concentration(i),
            ReleaseGrapple => self.f[1 - i].grappled = None,
            Drag => self.f[i].dragging = !self.f[i].dragging,
            Grapple | ShovePush | ShoveProne => {
                let maneuver = match choice {
                    Grapple => UnarmedManeuver::Grapple,
                    ShovePush => UnarmedManeuver::ShovePush,
                    ShoveProne => UnarmedManeuver::ShoveProne,
                    _ => unreachable!(),
                };
                self.spend_attack(i);
                self.reveal(i);
                self.phase = Phase::ManeuverSave {
                    source: i,
                    target: 1 - i,
                    maneuver,
                    opportunity: false,
                    bonus_strike: false,
                };
            }
            Unarmed => {
                self.spend_attack(i);
                if let Some(target) = self.guessed_attack_target(i, false) {
                    self.start_attack_against(i, target, self.unarmed_spec(i));
                }
            }
            Study => self.request(i, 87),
            Help => self.phase = Phase::HelpTarget,
            Ready => self.phase = Phase::ReadyTrigger,
            _ => unreachable!("common action {choice:?}"),
        }
    }

    fn unarmed_spec(&self, i: usize) -> Attack {
        let build = self.f[i].build;
        if self.martial_arts_active(i) {
            let mut attack = self.weapon_spec(i, false, false);
            attack.die = self.martial_arts_die(i);
            let ability = self.martial_unarmed_ability(i);
            let modifier = self.magic_save_modifier(i, ability);
            attack.bonus = modifier;
            attack.to_hit = modifier + build.pb;
            attack.unarmed = true;
            attack.strength = ability == 0;
            attack.dexterity = ability == 1;
            attack
        } else if let Some((die, modifier)) = self.magic_unarmed_override(i) {
            Attack {
                die,
                count: 1,
                bonus: modifier,
                to_hit: modifier + self.f[i].build.pb,
                ranged: false,
                weapon: true,
                flurry: false,
                unarmed: true,
                thrown: false,
                long_range: false,
                improvised: false,
                kind_override: None,
                dexterity: false,
                strength: true,
            }
        } else {
            let strength = self.magic_save_modifier(i, 0);
            Attack {
                die: 0,
                count: 0,
                bonus: 1 + strength,
                to_hit: strength + build.pb,
                ranged: false,
                weapon: true,
                flurry: false,
                unarmed: true,
                thrown: false,
                long_range: false,
                improvised: false,
                kind_override: None,
                dexterity: false,
                strength: true,
            }
        }
    }

    pub(super) fn drop_concentration(&mut self, actor: usize) {
        let old = self.f[actor].concentration.take();
        if old == Some(Concentration::ReadiedSpell) {
            self.f[actor].readied = None;
            if matches!(self.phase, Phase::ReadyRelease(reactor) if reactor == actor) {
                self.phase = Phase::Main;
            }
        }
    }

    fn ready_action_mask(&self, i: usize) -> u128 {
        use Act::*;
        let mut mask = Pass.bit()
            | Study.bit()
            | Unarmed.bit()
            | ShovePush.bit()
            | ShoveProne.bit()
            | Dodge.bit()
            | Search.bit()
            | Hide.bit()
            | self.magic_prepare_mask(i)
            | self.class_ready_mask(i)
            | self.divine_ready_mask(i)
            | self.gear_ready_mask(i);
        if self.gear_wielded(i, false) {
            mask |= Melee.bit();
        }
        if self.gear_free_hand(i) {
            mask |= Grapple.bit();
        }
        if self.speed(i) > 0 {
            mask |= Dash.bit();
        }
        if self.f[i].arrows > 0 && self.gear_wielded(i, true) {
            mask |= Ranged.bit();
        }
        if self.f[i].grappled.is_some() {
            mask |= EscapeAthletics.bit() | EscapeAcrobatics.bit();
        }
        if self.rogue_blade_available(i) {
            mask |= Special3.bit() | Special4.bit();
        }
        for action in [Special3, Special5] {
            if self.can_ready_ground(i, action) {
                mask |= action.bit();
            }
        }
        if self.f[i].build.class == Class::Rogue {
            if self.f[i].secondary > 0 {
                mask |= Utilize.bit();
            }
            mask |= Martial7.bit();
        }
        if self.entity(2 + i).is_some() {
            mask |= Help.bit();
        }
        mask
    }

    fn ready_is_common(action: Act) -> bool {
        use Act::*;
        matches!(
            action,
            Melee
                | Ranged
                | Dash
                | Unarmed
                | Grapple
                | ShovePush
                | ShoveProne
                | Dodge
                | Search
                | Hide
                | EscapeAthletics
                | EscapeAcrobatics
                | Study
                | Utilize
                | Martial7
                | Help
        )
    }

    fn set_readied(&mut self, trigger: ReadyTrigger, action: Act) {
        self.phase = Phase::Main;
        if action == Act::Pass {
            return;
        }
        let i = self.owner;
        let specialized = (self.class_ready_mask(i) | self.divine_ready_mask(i) | self.gear_ready_mask(i)) & action.bit() != 0
            || self.can_ready_ground(i, action) || self.can_ready_blade(i, action);
        if !specialized && !Self::ready_is_common(action) && !self.magic_prepare(i, action) {
            return;
        }
        self.action = false;
        self.f[i].readied = Some(Readied { trigger, action });
    }

    fn ready_ground_cell(index: usize) -> Pos {
        let width = super::geometry::WIDTH as usize;
        Pos {
            x: (index % width) as i8,
            y: (index / width) as i8,
        }
    }

    fn ready_targets(&self, actor: usize, action: Act) -> u128 {
        use Act::*;
        let candidates = self.attack_targets(actor, action);
        match action {
            Melee if self.gear_wielded(actor, false) => candidates | self.object_mask(actor, 1),
            Ranged if self.f[actor].arrows > 0 && self.gear_wielded(actor, true) => {
                candidates | self.object_mask(actor, 24)
            }
            Unarmed => candidates | self.object_mask(actor, self.unarmed_reach(actor)),
            Help => self.primary_help_targets(actor),
            Study => self.study_mask(actor),
            Special3 | Special4 if self.can_ready_blade(actor, action) => self.rogue_blade_targets(actor, action),
            Special3 | Special5 if self.can_ready_ground(actor, action) => {
                let squares = super::geometry::WIDTH as usize * super::geometry::HEIGHT as usize;
                (0..squares)
                    .filter(|&idx| {
                        self.ready_ground_heights(actor, action, Self::ready_ground_cell(idx)) != 0
                    })
                    .fold(0, |mask, index| mask | ACTS[index].bit())
            }
            Utilize if self.f[actor].build.class == Class::Rogue && self.f[actor].secondary > 0 => {
                let origin = self.fighter_point(actor);
                let squares = super::geometry::WIDTH as usize * super::geometry::HEIGHT as usize;
                (0..squares)
                    .filter(|&index| {
                        let cell = Self::ready_ground_cell(index);
                        let point = Point::ground(cell);
                        origin.dist(point) <= 1
                            && self.map.can_aim_point(origin, point)
                            && (0..2).all(|seat| self.caltrops(seat) != Some(cell))
                    })
                    .fold(0, |mask, index| mask | ACTS[index].bit())
            }
            Martial7 => self.martial_disarm_targets(actor),
            Grapple if !self.gear_free_hand(actor) => 0,
            Grapple | ShovePush | ShoveProne => (0..4)
                .filter(|&id| {
                    candidates & (1 << id) != 0
                        && self.combatant_size(id) <= self.combatant_size(actor) + 1
                        && (action != Grapple
                            || if id < 2 {
                                self.f[id].grappled.is_none()
                            } else {
                                self.entity(id)
                                    .is_some_and(|e| e.martial.grappled.is_none())
                            })
                })
                .fold(0, |mask, id| mask | (1 << id)),
            _ => 0,
        }
    }

    fn ready_uses_target(action: Act) -> bool {
        matches!(
            action,
            Act::Melee
                | Act::Ranged
                | Act::Unarmed
                | Act::Grapple
                | Act::ShovePush
                | Act::ShoveProne
                | Act::Study
                | Act::Utilize
                | Act::Martial7
                | Act::Help
        )
    }

    fn can_ready_blade(&self, actor: usize, action: Act) -> bool {
        matches!(action, Act::Special3 | Act::Special4) && self.rogue_blade_available(actor)
    }

    fn can_ready_ground(&self, actor: usize, action: Act) -> bool {
        let f = &self.f[actor];
        match action {
            Act::Special3 => f.build.is_subclass(Class::Druid, 0)
                && f.build.level >= 3 && f.resource > 0,
            Act::Special5 => f.concentration == Some(Concentration::Moonbeam)
                && self.magic_field(actor).is_some(),
            _ => false,
        }
    }

    fn ready_ground_heights(&self, actor: usize, action: Act, cell: Pos) -> u128 {
        let origin = if action == Act::Special5 {
            let Some(field) = self.magic_field(actor) else {
                return 0;
            };
            field
        } else {
            self.fighter_point(actor)
        };
        (0..=super::geometry::MAX_ELEVATION)
            .filter(|&altitude| {
                let point = Point { cell, altitude };
                origin.dist(point) <= 12
                    && if action == Act::Special5 {
                        self.map.can_occupy(point)
                    } else {
                        self.map.can_aim_point(origin, point)
                    }
            })
            .fold(0, |mask, z| mask | ACTS[z as usize].bit())
    }

    fn release_readied_ground(&mut self, actor: usize, action: Act, point: Point) {
        if action == Act::Special5 {
            self.move_moonbeam_at(actor, point);
        } else {
            self.f[actor].resource -= 1;
            self.start_lands_aid_at(actor, point);
        }
    }

    fn offer_readied(&mut self, reactor: usize, trigger: ReadyTrigger) {
        if !matches!(self.phase, Phase::Main) || !self.can_react(reactor) {
            return;
        }
        if let Some(readied) = self.f[reactor].readied {
            let valid = match readied.action {
                a if Self::ready_uses_target(a) || self.can_ready_ground(reactor, a) || self.can_ready_blade(reactor, a) => {
                    self.ready_targets(reactor, a) != 0
                }
                Act::Dash | Act::Dodge => self.speed(reactor) > 0,
                Act::Hide => self.can_hide(reactor),
                Act::Search => true,
                Act::EscapeAthletics | Act::EscapeAcrobatics => self.f[reactor].grappled.is_some(),
                a if self.divine_ready_mask(reactor) & a.bit() != 0 => self.divine_ready_valid(reactor),
                a if self.class_ready_mask(reactor) & a.bit() != 0 => {
                    self.class_ready_valid(reactor, a)
                }
                a if matches!(a, Act::Gear7 | Act::Gear8) => {
                    self.gear_ready_mask(reactor) & a.bit() != 0
                }
                _ => self.magic_readied_valid(reactor),
            };
            if readied.trigger == trigger && valid {
                self.phase = Phase::ReadyRelease(reactor);
            }
        }
    }

    pub(super) fn notify_readied_cast(&mut self, caster: usize) {
        self.queue_readied_event(caster, ReadyTrigger::Cast);
    }

    pub(super) fn notify_companion_event(&mut self, subject: usize, trigger: ReadyTrigger) {
        if subject >= 2 {
            self.queue_readied_event(subject, trigger);
        }
    }

    fn queue_readied_event(&mut self, subject: usize, trigger: ReadyTrigger) {
        let reactor = 1 - subject % 2;
        if self.f[reactor]
            .readied
            .is_some_and(|ready| ready.trigger == trigger)
            && (if subject < 2 {
                self.can_see(reactor, subject)
            } else {
                self.entity_visible(reactor, subject)
            })
        {
            self.ready_events.push((reactor, subject, trigger));
        }
    }

    fn flush_readied_event(&mut self) {
        while matches!(self.phase, Phase::Main | Phase::Entity(_)) && !self.ready_events.is_empty()
        {
            let (reactor, _, trigger) = self.ready_events.remove(0);
            let prior = self.phase;
            self.phase = Phase::Main;
            self.offer_readied(reactor, trigger);
            if matches!(self.phase, Phase::Main) {
                self.phase = prior;
            } else if !matches!(prior, Phase::Main) {
                self.ready_resume.push(ReadyContinuation::Phase(prior));
            }
        }
    }

    fn ready_release(&mut self, actor: usize, choice: Act) {
        self.phase = Phase::Main;
        if choice == Act::Pass {
            self.resume_ready_continuation();
            return;
        }
        self.f[actor].reaction = false;
        let action = self.f[actor].readied.take().expect("pending Ready").action;
        use Act::*;
        if Self::ready_uses_target(action) || self.can_ready_ground(actor, action) || self.can_ready_blade(actor, action) {
            let targets = self.ready_targets(actor, action);
            if targets.count_ones() > 1 {
                self.phase = Phase::ReadyTarget { actor, action };
            } else if targets != 0 {
                self.resolve_ready_target(actor, action, targets.trailing_zeros() as usize);
            }
            return;
        }
        match action {
            Dash => {
                self.phase = Phase::ReadyMove {
                    actor,
                    remaining: self.speed(actor),
                }
            }
            Dodge => self.f[actor].dodge = true,
            Hide => self.perform_common_check(actor, Check::Hide),
            Search => self.perform_common_check(actor, Check::Search),
            EscapeAthletics => self.perform_common_check(actor, Check::Escape(Skill::Athletics)),
            EscapeAcrobatics => self.perform_common_check(actor, Check::Escape(Skill::Acrobatics)),
            a if self.divine_ready_mask(actor) & a.bit() != 0 => self.divine_ready_release(actor),
            a if self.class_ready_mask(actor) & a.bit() != 0 => self.class_ready_release(actor, a),
            a if matches!(a, Gear7 | Gear8) => self.gear_ready_release(actor, a),
            _ => self.magic_release(actor, action),
        }
        self.resume_ready_continuation();
    }

    fn resolve_ready_target(&mut self, actor: usize, action: Act, target: usize) {
        self.phase = Phase::Main;
        if target == Act::Other8 as usize {
            self.resume_ready_continuation();
            return;
        }
        if let Some(maneuver) = Self::common_maneuver(action) {
            self.reveal(actor);
            self.phase = Phase::ManeuverSave {
                source: actor,
                target,
                maneuver,
                opportunity: false,
                bonus_strike: false,
            };
        } else if action == Act::Help {
            self.apply_primary_help(actor, target);
        } else if action == Act::Study {
            self.resolve_readied_study(actor, ACTS[target]);
        } else if self.can_ready_ground(actor, action) {
            let cell = Self::ready_ground_cell(target);
            let heights = self.ready_ground_heights(actor, action, cell);
            if heights.count_ones() > 1 {
                self.phase = Phase::ReadyGroundHeight {
                    actor,
                    action,
                    cell,
                };
            } else if heights != 0 {
                self.release_readied_ground(
                    actor,
                    action,
                    Point {
                        cell,
                        altitude: heights.trailing_zeros() as i8,
                    },
                );
            }
        } else if action == Act::Utilize {
            self.place_readied_caltrops(actor, Self::ready_ground_cell(target));
        } else if action == Act::Martial7 {
            self.disarm_readied_caltrops(actor, ACTS[target]);
        } else {
            let attack = if self.can_ready_blade(actor, action) {
                self.note_rogue_blade(actor);
                self.rogue_blade_spec(actor, target, action)
            } else if action == Act::Unarmed {
                self.unarmed_spec(actor)
            } else {
                self.weapon_spec(actor, action == Act::Ranged, false)
            };
            self.start_attack_against(actor, target, attack);
        }
    }

    fn ready_move_mask(&self, actor: usize, remaining: i32) -> u128 {
        let mut mask = Act::End.bit();
        let speed = self.speed(actor);
        if speed == 0 {
            return mask;
        }
        if self.f[actor].prone && !self.magic_laughter(actor) && remaining >= (speed + 1) / 2 {
            mask |= Act::Stand.bit();
        }
        for (dir, act) in ACTS[..8].iter().enumerate() {
            if let Some((next, cost)) = self.movement_step(actor, dir) {
                if cost <= remaining && self.movement_available(actor, next) {
                    mask |= act.bit();
                }
            }
        }
        mask
    }

    fn ready_move(&mut self, actor: usize, remaining: i32, choice: Act) {
        self.phase = Phase::Main;
        if choice == Act::End {
            self.resume_ready_continuation();
            return;
        }
        if choice == Act::Stand {
            self.f[actor].prone = false;
            self.phase = Phase::ReadyMove {
                actor,
                remaining: remaining - (self.speed(actor) + 1) / 2,
            };
            return;
        }
        let (destination, cost) = self
            .movement_step(actor, choice as usize)
            .expect("legal Ready move");
        if self.try_collision(actor, destination) {
            self.phase = Phase::ReadyMove { actor, remaining };
            return;
        }
        self.ready_pending_move = Some((
            actor,
            self.fighter_point(actor),
            destination,
            cost,
            remaining,
        ));
        let reactor = 1 - actor;
        let end = Point {
            cell: destination,
            altitude: self.f[actor].elevation,
        };
        if self.can_react(reactor)
            && !self.magic_no_reactions(reactor)
            && !self.f[reactor].addled
            && !self.magic_agile(actor)
            && self.can_see(reactor, actor)
            && self.mover_would_leave_reach(actor, end, reactor)
        {
            self.phase = Phase::Opportunity;
        } else {
            self.resume_ready_move();
        }
    }

    fn resume_ready_move(&mut self) {
        let (actor, origin, destination, cost, remaining) =
            self.ready_pending_move.take().expect("pending Ready move");
        self.phase = Phase::Main;
        if self.speed(actor) == 0 || self.fighter_point(actor) != origin {
            self.resume_ready_continuation();
            return;
        }
        self.commit_move(actor, destination);
        let remaining = remaining - cost;
        if remaining > 0 && self.speed(actor) > 0 && self.f[actor].hp > 0 {
            let next = Phase::ReadyMove { actor, remaining };
            if matches!(self.phase, Phase::Main) {
                self.phase = next;
            } else {
                self.ready_resume.push(ReadyContinuation::Phase(next));
            }
        } else {
            self.resume_ready_continuation();
        }
    }

    pub(super) fn queue_spell_reaction_move(&mut self, mover: usize, destination: Point) {
        let reactor = 1 - mover % 2;
        let Some(origin) = self.creature_point(mover) else {
            return;
        };
        self.pending_spell_move = Some((mover, origin, destination));
        let provokes = if mover >= 2 {
            self.entity_reaction_step_provokes(mover, destination)
        } else {
            self.can_react(reactor)
                && !self.magic_no_reactions(reactor)
                && !self.f[reactor].addled
                && !self.magic_agile(mover)
                && !(self.owner == mover && self.disengaged)
                && self.can_see(reactor, mover)
                && self.mover_would_leave_reach(mover, destination, reactor)
        };
        if provokes {
            self.phase = Phase::Opportunity;
        } else {
            self.resume_queued_movement();
        }
    }

    pub(super) fn resume_spell_reaction_move(&mut self) {
        let (mover, origin, destination) =
            self.pending_spell_move.take().expect("queued spell move");
        self.phase = Phase::Main;
        let previous = self.creature_point(mover).unwrap_or(origin);
        if mover >= 2 {
            if previous == origin {
                self.entity_commit_reaction_move(mover, destination);
            }
        } else if previous == origin
            && self.f[mover].hp > 0
            && self.speed(mover) > 0
            && !self.try_point_collision(mover, destination)
        {
            if self.entities.mounted[mover] {
                let before = self.ready_movement_context(mover);
                self.move_companion_point(mover, destination, 0);
                self.martial_moved(mover);
                self.magic_moved(mover, previous);
                self.move_trigger(mover, before);
            } else {
                self.commit_move_point(mover, destination);
            }
        }
        self.pending_spell_resume = Some((mover, previous));
        self.resume_spell_movement_callback();
    }

    fn resume_spell_movement_callback(&mut self) {
        if matches!(self.phase, Phase::Main) {
            if let Some((mover, previous)) = self.pending_spell_resume.take() {
                self.spell_movement_after_step(mover, previous);
            }
        }
    }

    pub(in crate::arena2) fn defer_martial_decision(&mut self, actor: usize, id: i32) {
        self.ready_resume
            .push(ReadyContinuation::Phase(Phase::Decision { actor, id }));
    }

    fn resume_ready_continuation(&mut self) {
        if matches!(self.phase, Phase::Main) {
            match self.ready_resume.pop() {
                Some(ReadyContinuation::Phase(phase)) => self.phase = phase,
                Some(ReadyContinuation::AfterAttack(attacker)) => {
                    self.continue_after_attack(attacker, false, true)
                }
                None => {}
            }
        }
    }

    fn is_martial_targeted_attack(&self, i: usize, action: Act) -> bool {
        matches!(
            (self.f[i].build.class, action),
            (Class::Monk, Act::Special1 | Act::Special2) | (Class::Barbarian, Act::Special2)
        )
    }

    pub(in crate::arena2) fn creature_targets(&self, action: Act) -> u128 {
        let i = self.owner;
        if action == Act::ReleaseGrapple {
            let mut targets = 0;
            if self.f[1 - i].grappled.is_some() {
                targets |= 1 << (1 - i);
            }
            let entity = 3 - i;
            if self
                .entity(entity)
                .is_some_and(|e| e.martial.grappled.is_some_and(|(source, _)| source == i))
            {
                targets |= 1 << entity;
            }
            return targets;
        }
        if Self::common_maneuver(action).is_some() {
            let max_size = self.combatant_size(i) + 1;
            let too_large = (0..4)
                .filter(|&id| self.combatant_size(id) > max_size)
                .fold(0u128, |bits, id| bits | (1 << id));
            let candidates = self.attack_targets(i, Act::Unarmed) & !too_large;
            if action == Act::Grapple {
                if !self.gear_free_hand(i) {
                    return 0;
                }
                let held_foe = (self.f[1 - i].grappled.is_some() as u128) << (1 - i);
                let entity = 3 - i;
                let held_entity = self
                    .entity(entity)
                    .is_some_and(|e| e.martial.grappled.is_some());
                return candidates & !(held_foe | (held_entity as u128) << entity);
            }
            return candidates;
        }
        let reach = if action == Act::Ranged { 24 } else if action == Act::Unarmed { self.unarmed_reach(i) } else { 1 };
        let objects = if matches!(action, Act::Melee | Act::Ranged | Act::Unarmed) {
            self.object_mask(i, reach)
        } else {
            0
        };
        self.attack_targets(i, action) | objects
    }

    fn attack_targets(&self, i: usize, action: Act) -> u128 {
        let ranged = action == Act::Ranged;
        let reach = if matches!(action, Act::Unarmed | Act::Special1 | Act::Special2) { self.unarmed_reach(i) } else { 1 };
        let origin = self.footprint(i).expect("acting combatant");
        (0..4)
            .filter(|&target| {
                target % 2 != i
                    && self.rope_same_space(i, target)
                    && !self.suggestion_charmed_by(i, target)
                    && self
                        .observed_footprint(i, target)
                        .is_some_and(|destination| {
                            (ranged || origin.distance(destination) <= reach)
                                && origin.squares().any(|from| {
                                    destination.squares().any(|to| self.map.los_at(from, to))
                                })
                        })
            })
            .fold(0, |mask, target| mask | 1 << target)
    }

    pub(super) fn creature_label(&self, id: usize) -> String {
        if id < 2 {
            format!("{:?}", self.f[id].build.class)
        } else if id < 4 {
            self.entity(id).expect("legal creature").kind.name().into()
        } else {
            format!("{:?}", self.object(id).expect("legal object").kind)
        }
    }

    fn guessed_attack_target(&mut self, actor: usize, ranged: bool) -> Option<usize> {
        let Some(center) = self.selected_point() else {
            return (!self.suggestion_charmed_by(actor, 1 - actor)).then_some(1 - actor);
        };
        let target = (0..4)
            .filter(|&id| {
                id % 2 != actor
                    && self.rope_same_space(actor, id)
                    && !self.suggestion_charmed_by(actor, id)
            })
            .find(|&id| {
                self.footprint(id)
                    .is_some_and(|fp| fp.squares().any(|p| p == center))
            });
        if target.is_none() {
            self.resolve_guessed_miss(actor, ranged);
        }
        target
    }

    pub(in crate::arena2) fn matches_target_guess(&mut self, actor: usize, ranged: bool) -> bool {
        if !self.suggestion_charmed_by(actor, 1 - actor)
            && self.selected_point().is_none_or(|center| {
                self.footprint(1 - actor)
                    .is_some_and(|fp| fp.squares().any(|p| p == center))
            })
        {
            return true;
        }
        self.resolve_guessed_miss(actor, ranged);
        false
    }

    fn resolve_guessed_miss(&mut self, actor: usize, ranged: bool) {
        if ranged {
            self.f[actor].arrows -= 1;
        }
        self.reveal(actor);
        self.magic_offensive(actor);
        self.martial_attack_rolled(actor, 1 - actor);
        self.complete_attack(actor);
    }
}

fn is_physical(kind: DamageType) -> bool {
    matches!(
        kind,
        DamageType::Bludgeoning | DamageType::Piercing | DamageType::Slashing
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn monk_martial_arts_selects_strength_or_dexterity_for_strikes() {
        let mut battle = (0..32)
            .map(|seed| Battle::new(seed, [Class::Monk, Class::Rogue], [2, 2]))
            .find(|battle| battle.owner == 0)
            .unwrap();
        assert_ne!(battle.f[0].build.stats[0], battle.f[0].build.stats[1]);
        for ability in 0..2 {
            battle.step(Act::Other6).unwrap();
            battle.step(ACTS[ability]).unwrap();
            let modifier = battle.f[0].build.stats[ability];
            for strike in [battle.unarmed_spec(0), battle.weapon_spec(0, false, false)] {
                assert_eq!(strike.die, 6);
                assert_eq!(strike.bonus, modifier);
                assert_eq!(strike.to_hit, modifier + battle.f[0].build.pb);
                assert_eq!(strike.strength, ability == 0);
                assert_eq!(strike.dexterity, ability == 1);
            }
        }
    }

    use super::*;

    #[test]
    fn roster_arena_choice_preserves_initiative_and_random_state() {
        let seed = 31;
        let ids = [Class::Fighter as u8 * 3, Class::Rogue as u8 * 3];
        let original = Battle::new(seed, [Class::Fighter, Class::Rogue], [5; 2]);
        let roster = Battle::new_roster(seed, ids);
        assert_eq!(roster.map.layout_id(), original.map.layout_id());
        assert_eq!(roster.actor(), original.actor());
        assert_eq!(roster.mask(), original.mask());

        for layout in 0..super::super::geometry::LAYOUT_COUNT {
            let chosen = Battle::new_roster_in(seed, ids, layout);
            assert_eq!(chosen.map.layout_id(), layout);
            assert_eq!(chosen.actor(), original.actor());
            assert_eq!(chosen.dice.0, original.dice.0);
        }
    }

    fn adjacent(seed: u64, classes: [Class; 2], levels: [i32; 2]) -> Battle {
        let mut battle = Battle::new(seed, classes, levels);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 1, y: 0 };
        battle.f[1].pos = Pos { x: 0, y: 0 };
        battle
    }

    #[test]
    fn opportunity_pass_preserves_reaction_and_commits_move() {
        let mut b = adjacent(0, [Class::Fighter; 2], [1; 2]);
        b.step(Act::E).unwrap();
        assert_eq!(b.actor(), Some(1));
        assert_eq!(b.phase_id(), 1);
        assert_eq!(b.moved, 0);
        assert_eq!(b.f[0].pos.x, 1);
        assert!(b.step(Act::E).is_err());
        b.step(Act::Pass).unwrap();
        assert!(b.f[1].reaction);
        assert_eq!(b.actor(), Some(0));
        assert_eq!(b.f[0].pos.x, 2);
        assert_eq!(b.moved, 1);
    }

    #[test]
    fn lethal_opportunity_attack_stops_movement() {
        let mut b = (0..100)
            .find_map(|seed| {
                let mut b = adjacent(seed, [Class::Fighter; 2], [1; 2]);
                b.f[0].hp = 1;
                b.step(Act::E).unwrap();
                b.step(Act::Opportunity).unwrap();
                b.finished().then_some(b)
            })
            .unwrap();
        assert_eq!(b.winner(), Some(1));
        assert_eq!(b.f[0].pos.x, 1);
        assert!(!b.f[1].reaction);
        assert_eq!(b.mask(), 0);
        assert!(b.step(Act::End).is_err());
    }

    #[test]
    fn shield_breaks_hit_chain_before_smite() {
        let mut b = (0..100)
            .find_map(|seed| {
                let mut b = adjacent(seed, [Class::Paladin, Class::Wizard], [2, 2]);
                b.step(Act::Melee).unwrap();
                matches!(b.phase, Phase::Defend { hit, .. } if !hit.crit && hit.total < hit.ac + 5)
                    .then_some(b)
            })
            .unwrap();
        let hp = b.f[1].hp;
        let slots = b.f[1].slots[0];
        let mut accept_hit = b.clone();
        accept_hit.step(Act::Pass).unwrap();
        assert_eq!(accept_hit.actor(), Some(0));
        assert_eq!(accept_hit.phase_id(), 23);
        b.step(Act::Defend).unwrap();
        assert_eq!(b.f[1].slots[0], slots);
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.phase_id(), 0);
        assert_eq!(b.f[1].hp, hp);
        assert_eq!(b.f[1].slots[0], slots - 1);
        assert!(!b.f[1].reaction);
        assert!(b.bonus);
    }

    #[test]
    fn extra_attack_changes_actual_attack_budget_at_level_five() {
        for class in [
            Class::Barbarian,
            Class::Fighter,
            Class::Monk,
            Class::Paladin,
            Class::Ranger,
            Class::Rogue,
        ] {
            for level in 1..=5 {
                let eligible = level == 5 && class != Class::Rogue;
                let mut battle = adjacent(0, [class, Class::Fighter], [level, 5]);
                battle.step(Act::Unarmed).unwrap();
                while battle.phase_id() != 0 {
                    battle.step(Act::Pass).unwrap();
                }
                assert!(!battle.action, "{class:?} level {level}");
                assert!(battle.bonus, "{class:?} level {level}");
                assert_eq!(
                    battle.attacks,
                    i32::from(eligible),
                    "{class:?} level {level}"
                );
                assert_eq!(battle.mask() & Act::Unarmed.bit() != 0, eligible);
                if eligible {
                    battle.step(Act::S).unwrap();
                    battle.step(Act::Unarmed).unwrap();
                    while battle.phase_id() != 0 {
                        battle.step(Act::Pass).unwrap();
                    }
                    assert_eq!(battle.attacks, 0);
                }
                assert!(battle.step(Act::Unarmed).is_err());
            }
        }
    }

    #[test]
    fn ranger_archery_improves_bow_hits_without_increasing_damage() {
        use super::super::equipment::Carried;

        for level in 1..=5 {
            let battle = adjacent(0, [Class::Ranger, Class::Fighter], [level, 5]);
            let (bow_die, bow_modifier) = battle.weapon(0, true);
            let (sword_die, sword_modifier) = battle.weapon(0, false);
            let pb = battle.f[0].build.pb;
            let bow = battle.weapon_spec(0, true, false);
            let sword = battle.weapon_spec(0, false, false);
            assert_eq!((bow.die, bow.bonus), (bow_die, bow_modifier));
            assert_eq!(
                bow.to_hit,
                pb + bow_modifier + if level >= 2 { 2 } else { 0 }
            );
            assert_eq!(
                (sword.die, sword.bonus, sword.to_hit),
                (sword_die, sword_modifier, sword_modifier + pb)
            );
        }

        let gained_hits = (0..128)
            .filter(|seed| {
                let damage = |level| {
                    let mut battle = adjacent(*seed, [Class::Ranger, Class::Fighter], [level, 5]);
                    battle.f[0].gear.melee = Carried::Packed;
                    let hp = battle.f[1].hp;
                    battle.step(Act::Ranged).unwrap();
                    hp - battle.f[1].hp
                };
                let normal = damage(1);
                let archery = damage(2);
                assert!(archery >= normal);
                archery > normal
            })
            .count();
        assert!(gained_hits > 0);
    }

    #[test]
    fn extra_attack_can_be_used_after_moving() {
        let mut b = adjacent(0, [Class::Fighter; 2], [5; 2]);
        b.step(Act::Melee).unwrap();
        assert!(!b.action);
        assert_eq!(b.attacks, 1);
        b.step(Act::S).unwrap();
        b.step(Act::Melee).unwrap();
        assert_eq!(b.attacks, 0);
        assert!(b.step(Act::Melee).is_err());
    }

    #[test]
    fn draw_caps_a_noninteractive_stalemate() {
        let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
        for _ in 0..TURN_LIMIT {
            b.step(Act::End).unwrap();
        }
        assert!(b.finished());
        assert_eq!(b.winner(), None);
        assert_eq!(b.actor(), None);
    }

    #[test]
    fn slotted_spell_limit_is_per_combatant_and_turn() {
        let mut b = adjacent(0, [Class::Wizard; 2], [1; 2]);
        assert!(b.spend_slot(0, 1));
        assert!(!b.spend_slot(0, 1));
        assert!(b.spend_slot(1, 1));
        b.step(Act::End).unwrap();
        assert!(b.spend_slot(0, 1));
        assert!(b.spend_slot(1, 1));
    }

    #[test]
    fn arrow_is_spent_even_if_attack_misses() {
        let mut b = (0..100)
            .find_map(|seed| {
                let mut b = adjacent(seed, [Class::Ranger, Class::Fighter], [1; 2]);
                b.f[0].arrows = 1;
                b.f[0].gear.melee = super::super::equipment::Carried::Packed;
                let hp = b.f[1].hp;
                b.step(Act::Ranged).unwrap();
                (b.f[1].hp == hp).then_some(b)
            })
            .unwrap();
        assert_eq!(b.f[0].arrows, 0);
        b.action = true;
        assert_eq!(b.mask() & Act::Ranged.bit(), 0);
    }

    #[test]
    fn addle_expires_at_targets_next_turn() {
        let mut b = adjacent(0, [Class::Monk, Class::Fighter], [3, 1]);
        b.f[1].addled = true;
        b.step(Act::E).unwrap();
        assert_eq!(b.phase_id(), 0);
        b.step(Act::End).unwrap();
        assert!(!b.f[1].addled);
    }

    #[test]
    fn fireball_waits_for_a_legal_cell_then_spends_slots_once() {
        let mut b = Battle::new(0, [Class::Wizard, Class::Fighter], [5, 5]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 8, y: 0 };
        let dice = b.dice.clone();
        let slots = b.f[0].slots;
        let hp = [b.f[0].hp, b.f[1].hp];
        b.step(Act::Spell4).unwrap();
        assert_eq!(b.phase_id(), 5);
        assert_eq!(b.actor(), Some(0));
        assert_eq!(b.target_request(), Some((Act::Spell4, None)));
        assert_eq!(b.mask() & !0x7f, 0);
        assert!(b.step(ACTS[7]).is_err());
        assert_eq!(b.target_request(), Some((Act::Spell4, None)));
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.phase_id(), 6);
        assert_eq!(b.target_request(), Some((Act::Spell4, Some(0))));
        assert_eq!(b.mask() & !0x1ff, 0);
        assert!(b.step(ACTS[9]).is_err());
        assert_eq!(b.target_request(), Some((Act::Spell4, Some(0))));
        assert_eq!(b.dice, dice);
        assert_eq!(b.f[0].slots, slots);
        assert_eq!([b.f[0].hp, b.f[1].hp], hp);
        assert!(b.action);

        let mut aimed_at_caster = b.clone();
        b.step(ACTS[8]).unwrap();
        if b.phase_id() == 19 {
            b.step(ACTS[0]).unwrap();
        }
        assert_eq!(b.target_request(), None);
        assert_eq!(b.selected_target, None);
        assert_eq!(b.f[0].slots, [slots[0], slots[1], slots[2] - 1]);
        assert_eq!(b.f[0].hp, hp[0]);
        assert!(b.f[1].hp < hp[1]);
        assert!(!b.action);

        aimed_at_caster.step(ACTS[0]).unwrap();
        if aimed_at_caster.phase_id() == 19 {
            aimed_at_caster.step(ACTS[0]).unwrap();
        }
        assert_eq!(aimed_at_caster.f[0].slots, b.f[0].slots);
        assert!(aimed_at_caster.f[0].hp < hp[0]);
        assert_eq!(aimed_at_caster.f[1].hp, hp[1]);
    }

    #[test]
    fn hidden_positions_and_guessed_attack_masks_are_viewer_relative() {
        for owner in 0..2 {
            let mut b = Battle::new(0, [Class::Rogue; 2], [1; 2]);
            b.owner = owner;
            b.f[owner].pos = Pos { x: 0, y: 0 };
            b.f[1 - owner].pos = Pos { x: 3, y: 0 };
            b.f[1 - owner].hidden_dc = Some(23);
            b.f[owner].gear.melee = super::super::equipment::Carried::Packed;
            assert_eq!(b.observed_pos(owner, 1 - owner), None);
            assert_eq!(
                b.observed_pos(1 - owner, 1 - owner),
                Some(Pos { x: 3, y: 0 })
            );
            assert_ne!(b.mask() & Act::Ranged.bit(), 0);
            let state = b.common_decision_features(owner);
            let mask = b.mask();
            let mut alternate = b.clone();
            alternate.f[1 - owner].pos = Pos { x: 8, y: 4 };
            assert_eq!(alternate.mask(), mask);
            assert_eq!(alternate.common_decision_features(owner), state);
            b.step(Act::Ranged).unwrap();
            alternate.step(Act::Ranged).unwrap();
            assert_eq!(b.mask(), alternate.mask());
            b.step(Act::N).unwrap(); // select row zero
            alternate.step(Act::N).unwrap();
            assert_eq!(b.mask(), alternate.mask());
            let hp = alternate.f[1 - owner].hp;
            let ammo = alternate.f[owner].arrows;
            alternate.step(Act::SE).unwrap(); // select column three
            if alternate.phase_id() == 19 {
                alternate.step(Act::N).unwrap();
            }
            assert_eq!(alternate.f[1 - owner].hp, hp);
            assert_eq!(alternate.f[owner].arrows, ammo - 1);
            assert!(!alternate.action);
        }
    }

    #[test]
    fn hiding_and_searching_use_checks_and_require_an_action() {
        for owner in 0..2 {
            let run = |seed| {
                let mut b = Battle::new(seed, [Class::Rogue; 2], [1; 2]);
                b.owner = owner;
                b.f[owner].pos = Pos { x: 0, y: 3 };
                b.f[1 - owner].pos = Pos { x: 8, y: 3 };
                if b.mask() & Act::Hide.bit() == 0 {
                    return None;
                }
                b.step(Act::Hide).unwrap();
                assert!(!b.action);
                let hidden = b.f[owner].hidden_dc.is_some();
                b.step(Act::End).unwrap();
                assert_ne!(b.mask() & Act::Search.bit(), 0);
                b.step(Act::Search).unwrap();
                assert!(!b.action);
                Some((hidden, b.f[owner].hidden_dc.is_some()))
            };
            let outcomes: Vec<_> = (0..128).filter_map(run).collect();
            assert!(outcomes.contains(&(true, false)));
            assert!(outcomes.contains(&(true, true)));
            assert!(outcomes.contains(&(false, false)));
        }
    }

    #[test]
    fn ready_reacts_after_entering_reach_and_pass_preserves_reaction() {
        for readier in 0..2 {
            let mover = 1 - readier;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = readier;
            b.f[readier].pos = Pos { x: 1, y: 0 };
            b.f[mover].pos = Pos { x: 3, y: 0 };
            b.step(Act::Ready).unwrap();
            assert_eq!(b.phase_id(), 7);
            assert!(b.action);
            b.step(Act::N).unwrap(); // Enter reach
            b.step(Act::Melee).unwrap();
            assert!(!b.action);
            b.step(Act::End).unwrap();
            b.step(Act::W).unwrap();
            assert_eq!(b.phase_id(), 9);
            assert_eq!(b.actor(), Some(readier));
            assert_eq!(b.f[mover].pos.x, 2);
            let mut pass = b.clone();
            pass.step(Act::Pass).unwrap();
            assert!(pass.f[readier].reaction);
            assert!(pass.f[readier].readied.is_some());
            b.step(Act::Ready).unwrap();
            assert!(!b.f[readier].reaction);
            assert!(b.f[readier].readied.is_none());
        }
    }

    #[test]
    fn grapple_allows_target_save_and_drag_costs_extra_movement() {
        for owner in 0..2 {
            let mut b = Battle::new(0, [Class::Monk; 2], [1; 2]);
            b.owner = owner;
            b.f[owner].pos = Pos { x: 1, y: 0 };
            b.f[1 - owner].pos = Pos { x: 0, y: 0 };
            b.f[1 - owner].stunned = true;
            b.step(Act::Grapple).unwrap();
            assert_eq!(b.phase_id(), 12);
            assert_eq!(b.actor(), Some(1 - owner));
            assert!(b.step(Act::E).is_err());
            b.step(Act::NE).unwrap(); // Dexterity save auto-fails while stunned
            assert!(b.f[1 - owner].grappled.is_some());
            assert_eq!(b.speed(1 - owner), 0);
            b.step(Act::E).unwrap();
            assert_eq!(b.moved, 2);
            assert_eq!(b.f[1 - owner].pos, Pos { x: 1, y: 0 });
            b.step(Act::ReleaseGrapple).unwrap();
            assert!(b.f[1 - owner].grappled.is_none());
        }
    }

    #[test]
    fn athletics_and_acrobatics_escape_and_standing_are_separate_choices() {
        let mut b = adjacent(0, [Class::Fighter; 2], [1; 2]);
        b.f[0].grappled = Some(1);
        b.f[0].prone = true;
        assert_eq!(b.mask() & Act::Stand.bit(), 0);
        assert_ne!(b.mask() & Act::EscapeAthletics.bit(), 0);
        assert_ne!(b.mask() & Act::EscapeAcrobatics.bit(), 0);
        b.step(Act::EscapeAcrobatics).unwrap();
        assert!(b.f[0].grappled.is_none());
        assert!(!b.action);
        b.step(Act::Stand).unwrap();
        assert!(!b.f[0].prone);
        assert_eq!(b.moved, 3);
    }

    #[test]
    fn shove_offers_a_save_before_prone_or_push() {
        for owner in 0..2 {
            for (choice, prone, position) in
                [(Act::ShoveProne, true, 1), (Act::ShovePush, false, 2)]
            {
                let mut b = Battle::new(0, [Class::Monk; 2], [1; 2]);
                b.owner = owner;
                b.f[owner].pos = Pos { x: 0, y: 0 };
                b.f[1 - owner].pos = Pos { x: 1, y: 0 };
                b.f[1 - owner].stunned = true;
                b.step(choice).unwrap();
                assert_eq!(b.f[1 - owner].pos.x, 1);
                assert!(!b.f[1 - owner].prone);
                assert_eq!(b.mask(), Act::N.bit() | Act::NE.bit() | Act::Pass.bit());
                b.step(Act::N).unwrap();
                assert_eq!(b.f[1 - owner].pos.x, position);
                assert_eq!(b.f[1 - owner].prone, prone);
            }
        }
    }

    #[test]
    fn either_seat_can_shove_a_companion_and_choose_the_defending_save() {
        use super::super::entities::{Entity, EntityKind, FamiliarForm, Spirit};
        for owner in 0..2 {
            let defender = 1 - owner;
            let companion = defender + 2;
            let mut b = Battle::new(0, [Class::Fighter, Class::Fighter], [1; 2]);
            b.owner = owner;
            b.f[owner].pos = Pos { x: 0, y: 0 };
            b.f[defender].pos = Pos { x: 5, y: 0 };
            b.entities.test_install(Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Cat,
                    spirit: Spirit::Fey,
                },
                defender,
                Pos { x: 1, y: 0 },
                1,
            ));
            b.step(Act::ShoveProne).unwrap();
            b.step(ACTS[3]).unwrap();
            assert_eq!(b.actor(), Some(defender));
            assert_eq!(b.mask(), Act::N.bit() | Act::NE.bit() | Act::Pass.bit());
            assert!(!b.entity(companion).unwrap().martial.prone);
            b.step(Act::Pass).unwrap();
            assert!(b.entity(companion).unwrap().martial.prone);
            assert!(!b.f[defender].prone);
            assert!(!b.action);
        }
    }

    #[test]
    fn initiative_surprise_can_change_first_actor_without_skipping_turn() {
        for surprised in [[true, false], [false, true]] {
            let mut changed = false;
            for seed in 0..128 {
                let regular = Battle::new(seed, [Class::Rogue; 2], [1; 2]);
                let opening = Battle::new_with_surprise(seed, [Class::Rogue; 2], [1; 2], surprised);
                assert_eq!(opening.turns, 1);
                assert!(opening.action);
                changed |= regular.owner != opening.owner;
            }
            assert!(changed);
        }
    }

    #[test]
    fn readied_movement_provokes_and_a_successful_grapple_stops_it() {
        for readier in 0..2 {
            let mover = 1 - readier;
            let mut b = Battle::new(0, [Class::Monk; 2], [1; 2]);
            b.owner = readier;
            b.f[readier].pos = Pos { x: 1, y: 0 };
            b.f[mover].pos = Pos { x: 3, y: 0 };
            b.step(Act::Ready).unwrap();
            b.step(Act::N).unwrap(); // Enter reach
            b.step(Act::Dash).unwrap(); // Ready movement
            b.step(Act::End).unwrap();
            b.step(Act::W).unwrap();
            b.step(Act::Ready).unwrap();
            assert_eq!(b.actor(), Some(readier));
            b.step(Act::W).unwrap();
            assert_eq!(b.phase_id(), 1);
            assert_eq!(b.actor(), Some(mover));
            assert_eq!(b.f[readier].pos.x, 1);
            b.step(Act::Grapple).unwrap();
            assert_eq!(b.phase_id(), 12);
            assert!(!b.f[mover].reaction);
            b.f[readier].stunned = true; // Force this save to fail.
            b.step(Act::N).unwrap();
            assert_eq!(b.f[readier].pos.x, 1);
            assert!(b.f[readier].grappled.is_some());
            assert_eq!(b.actor(), Some(mover));
        }
    }

    #[test]
    fn hiding_can_turn_partial_cover_into_an_ambush() {
        use super::super::geometry::{Cover, HEIGHT, WIDTH};
        let mut fixture = None;
        'seeds: for seed in 0..16 {
            let map = Map::new(seed);
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let shelter = Pos { x, y };
                    for (direction, &act) in ACTS[..8].iter().enumerate() {
                        let Some((opening, _)) = map.step(shelter, direction) else {
                            continue;
                        };
                        for foe_y in 0..HEIGHT {
                            for foe_x in 0..WIDTH {
                                let foe = Pos { x: foe_x, y: foe_y };
                                if foe.dist(opening) > 1
                                    && map.los(foe, foe)
                                    && map.cover(shelter, foe) == Cover::Total
                                    && matches!(
                                        map.cover(opening, foe),
                                        Cover::Half | Cover::ThreeQuarters
                                    )
                                {
                                    fixture = Some((seed, shelter, foe, act));
                                    break 'seeds;
                                }
                            }
                        }
                    }
                }
            }
        }
        let (seed, shelter, foe, step) = fixture.expect("the arena has a covered firing position");
        let mut b = Battle::new(seed, [Class::Rogue, Class::Fighter], [1, 1]);
        b.owner = 0;
        b.f[0].pos = shelter;
        b.f[1].pos = foe;
        b.f[0].gear.melee = super::super::equipment::Carried::Packed;
        // Vary only the test dice until a successful Hide can exercise the movement and attack.
        let hidden = (0..64)
            .find_map(|rolls| {
                let mut candidate = b.clone();
                candidate.dice = Dice(rolls);
                candidate.step(Act::Hide).unwrap();
                candidate.f[0].hidden_dc.is_some().then_some(candidate)
            })
            .expect("successes are possible on DC15 Stealth");
        let mut b = hidden;
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        b.step(step).unwrap();
        assert_eq!(b.observed_pos(1, 0), None);
        assert!(b.can_see(0, 1));
        b.step(Act::Ranged).unwrap();
        assert!(b.f[0].hidden_dc.is_none());
        assert!(b.observed_pos(1, 0).is_some());
    }

    #[test]
    fn ready_action_waits_for_action_commitment_in_both_seats() {
        for readier in 0..2 {
            let foe = 1 - readier;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = readier;
            b.f[readier].pos = Pos { x: 0, y: 0 };
            b.f[foe].pos = Pos { x: 3, y: 0 };
            b.step(Act::Ready).unwrap();
            b.step(ACTS[ReadyTrigger::Action as usize]).unwrap();
            b.step(Act::Dodge).unwrap();
            b.step(Act::End).unwrap();
            b.step(Act::E).unwrap();
            assert_eq!(b.actor(), Some(foe));
            b.step(Act::Dodge).unwrap();
            assert_eq!((b.phase_id(), b.actor()), (9, Some(readier)));
            assert!(!b.action);
            assert!(b.f[foe].dodge);
            b.step(Act::Pass).unwrap();
            assert!(b.f[readier].reaction);
        }
    }

    #[test]
    fn ready_leave_reach_is_distinct_from_visible_movement() {
        for readier in 0..2 {
            let foe = 1 - readier;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = readier;
            b.f[readier].pos = Pos { x: 0, y: 0 };
            b.f[foe].pos = Pos { x: 1, y: 0 };
            b.step(Act::Ready).unwrap();
            b.step(ACTS[ReadyTrigger::LeaveReach as usize]).unwrap();
            b.step(Act::Dodge).unwrap();
            b.step(Act::End).unwrap();
            b.step(Act::Disengage).unwrap();
            b.step(Act::S).unwrap();
            assert_eq!(b.actor(), Some(foe));
            b.step(Act::E).unwrap();
            assert_eq!((b.phase_id(), b.actor()), (9, Some(readier)));
            b.step(Act::Ready).unwrap();
            assert!(b.f[readier].dodge && !b.f[readier].reaction);
        }
    }

    #[test]
    fn ready_cast_checks_perception_at_the_spell_event() {
        for readier in 0..2 {
            let caster = 1 - readier;
            let mut b = Battle::new(0, [Class::Cleric; 2], [1; 2]);
            b.owner = readier;
            b.f[readier].pos = Pos { x: 0, y: 0 };
            b.f[caster].pos = Pos { x: 3, y: 0 };
            b.step(Act::Ready).unwrap();
            b.step(ACTS[ReadyTrigger::Cast as usize]).unwrap();
            b.step(Act::Dodge).unwrap();
            b.step(Act::End).unwrap();
            b.f[caster].hidden_dc = Some(20);
            b.notify_readied_cast(caster);
            b.flush_readied_event();
            assert_eq!(b.actor(), Some(caster));
            b.reveal(caster);
            b.notify_readied_cast(caster);
            b.flush_readied_event();
            assert_eq!((b.phase_id(), b.actor()), (9, Some(readier)));
        }
    }

    #[test]
    fn hidden_entry_into_reach_does_not_offer_a_ready_reaction() {
        let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
        b.owner = 1;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 1, y: 0 };
        b.f[1].hidden_dc = Some(20);
        b.f[0].readied = Some(Readied {
            trigger: ReadyTrigger::EnterReach,
            action: Act::Dodge,
        });
        b.move_trigger(1, (false, false));
        assert_eq!(b.actor(), Some(1));
        b.f[0].readied = Some(Readied {
            trigger: ReadyTrigger::EnterSight,
            action: Act::Dodge,
        });
        b.reveal(1);
        b.move_trigger(1, (false, true));
        assert_eq!((b.phase_id(), b.actor()), (9, Some(0)));
    }

    #[test]
    fn ready_dodge_resolves_on_visible_movement_without_spending_the_foes_action() {
        for readier in 0..2 {
            let foe = 1 - readier;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = readier;
            b.f[readier].pos = Pos { x: 0, y: 0 };
            b.f[foe].pos = Pos { x: 3, y: 0 };
            b.step(Act::Ready).unwrap();
            b.step(ACTS[ReadyTrigger::Move as usize]).unwrap();
            b.step(Act::Dodge).unwrap();
            assert!(!b.action);
            assert!(!b.f[readier].dodge);
            b.step(Act::End).unwrap();
            b.step(Act::E).unwrap();
            assert_eq!(b.phase_id(), 9);
            assert_eq!(b.actor(), Some(readier));
            assert!(b.action);
            b.step(Act::Ready).unwrap();
            assert!(b.f[readier].dodge);
            assert!(b.action);
            assert!(!b.f[readier].reaction);
            assert_eq!(b.actor(), Some(foe));
            b.step(Act::End).unwrap();
            assert!(!b.f[readier].dodge);
        }
    }

    #[test]
    fn prone_can_persist_in_wild_shape_and_incapacitation_ends_it() {
        for druid in 0..2 {
            let classes = std::array::from_fn(|i| {
                if i == druid {
                    Class::Druid
                } else {
                    Class::Monk
                }
            });
            let mut b = Battle::new(0, classes, [2; 2]);
            b.owner = druid;
            b.f[druid].pos = Pos { x: 0, y: 0 };
            b.f[1 - druid].pos = Pos { x: 1, y: 0 };
            b.step(Act::Special1).unwrap();
            b.step(ACTS[0]).unwrap();
            if b.phase_id() == 100 {
                b.step(ACTS[0]).unwrap();
            }
            assert!(b.magic_is_wolf(druid));
            b.step(Act::DropProne).unwrap();
            assert!(b.magic_is_wolf(druid));
            assert!(b.f[druid].prone);
            b.step(Act::Stand).unwrap();
            b.f[druid].elevation = 2;
            b.traversal_falls();
            assert_eq!(b.f[druid].elevation, 0);
            assert!(b.f[druid].prone);
            assert!(b.magic_is_wolf(druid));
            b.f[druid].stunned = true;
            b.step(Act::S).unwrap();
            assert!(!b.magic_is_wolf(druid));
            assert!(b.f[druid].prone);
        }
    }

    #[test]
    fn readied_shield_doff_uses_the_reaction_and_leaves_the_foes_action_intact() {
        for actor in 0..2 {
            let foe = 1 - actor;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = actor;
            b.f[actor].pos = Pos { x: 0, y: 0 };
            b.f[foe].pos = Pos { x: 3, y: 0 };
            let armored = b.ac(actor);
            b.step(Act::Ready).unwrap();
            b.step(ACTS[ReadyTrigger::Move as usize]).unwrap();
            b.step(Act::Gear7).unwrap();
            assert_eq!(b.ac(actor), armored);
            b.step(Act::End).unwrap();
            b.step(Act::E).unwrap();
            b.step(Act::Ready).unwrap();
            assert_eq!(b.ac(actor), armored - 2);
            assert_eq!(b.actor(), Some(foe));
            assert!(b.action);
            assert!(!b.f[actor].reaction);
        }
    }

    #[test]
    fn ready_interrupts_flurry_between_its_two_strikes() {
        let mut b = adjacent(0, [Class::Fighter, Class::Monk], [5, 2]);
        b.step(Act::Ready).unwrap();
        b.step(ACTS[ReadyTrigger::Attack as usize]).unwrap();
        b.step(Act::Dodge).unwrap();
        b.step(Act::End).unwrap();
        let focus = b.f[1].resource;
        b.step(Act::Special2).unwrap();
        assert_eq!(b.phase_id(), 9);
        assert_eq!(b.actor(), Some(0));
        assert!(b.bonus_attacks.is_some());
        assert_eq!(b.f[1].resource, focus - 1);
        b.step(Act::Ready).unwrap();
        assert!(b.f[0].dodge);
        assert!(b.bonus_attacks.is_none());
        assert_eq!(b.f[1].resource, focus - 1);
        assert_eq!(b.actor(), Some(1));
    }

    #[test]
    fn readied_caltrops_spend_equipment_at_release_and_keep_enemy_action() {
        let mut b = Battle::new(0, [Class::Rogue, Class::Fighter], [3, 3]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 3, y: 0 };
        let bags = b.f[0].secondary;
        assert!(bags > 0);
        b.step(Act::Ready).unwrap();
        b.step(ACTS[ReadyTrigger::Move as usize]).unwrap();
        b.step(Act::Utilize).unwrap();
        assert_eq!(b.f[0].secondary, bags);
        b.step(Act::End).unwrap();
        b.step(Act::E).unwrap();
        b.step(Act::Ready).unwrap();
        assert_eq!(b.phase_id(), 106);
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.caltrops(0), Some(Pos { x: 1, y: 0 }));
        assert_eq!(b.f[0].secondary, bags - 1);
        assert!(b.action);
        assert_eq!(b.actor(), Some(1));
    }

    #[test]
    fn concentration_menu_returns_without_cost_and_confirmation_counts_once() {
        let mut b = Battle::new(0, [Class::Cleric; 2], [5; 2]);
        let actor = b.owner;
        b.f[actor].concentration = Some(Concentration::Bless);
        b.phase = Phase::TargetRow(Act::Spell3);
        let parameter_choices = Act::DropConcentration.bit() | Act::Pass.bit();
        let menu_action = b.concentration_menu_action_for(parameter_choices).unwrap();
        assert_eq!(parameter_choices & menu_action.bit(), 0);
        b.save_choice.concentration_menu = true;
        let decisions = b.decisions;
        assert_eq!(b.phase_id(), 118);
        assert_eq!(b.actor(), Some(actor));
        assert_eq!(b.mask(), Act::Pass.bit() | Act::DropConcentration.bit());

        b.step(Act::Pass).unwrap();
        assert_eq!(b.phase_id(), 5);
        assert_eq!(b.decisions, decisions);
        assert_eq!(b.f[actor].concentration, Some(Concentration::Bless));

        b.save_choice.concentration_menu = true;
        b.step(Act::DropConcentration).unwrap();
        assert_eq!(b.phase_id(), 5);
        assert_eq!(b.decisions, decisions + 1);
        assert!(b.f[actor].concentration.is_none());
    }

    #[test]
    fn dropping_spiritual_weapon_during_relocation_returns_to_the_same_turn() {
        let mut b = Battle::new(0, [Class::Cleric, Class::Warlock], [5; 2]);
        b.owner = 0;
        let center = b.fighter_point(0);
        b.concentrate(0, Concentration::SpiritualWeapon, 10, Some(center), 2);
        b.step(Act::Special3).unwrap();
        b.step(ACTS[center.cell.y as usize]).unwrap();
        assert_eq!(b.phase_id(), 6);
        assert_ne!(b.mask() & !Act::DropConcentration.bit(), 0);
        assert_ne!(b.mask() & Act::DropConcentration.bit(), 0);
        let before = (b.turns, b.action, b.bonus, b.f[0].reaction, b.decisions);
        b.step(Act::DropConcentration).unwrap();
        assert_eq!(b.phase_id(), 0);
        assert_eq!(b.actor(), Some(0));
        assert_ne!(b.mask(), 0);
        assert_eq!(b.f[0].concentration, None);
        assert_eq!(
            (b.turns, b.action, b.bonus, b.f[0].reaction, b.decisions),
            (before.0, before.1, before.2, before.3, before.4 + 1)
        );
    }

    #[test]
    fn either_seat_can_drop_own_concentration_during_an_existing_reaction() {
        for moving in 0..2 {
            let reactor = 1 - moving;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = moving;
            b.f[moving].pos = Pos { x: 1, y: 0 };
            b.f[reactor].pos = Pos { x: 0, y: 0 };
            b.f[reactor].concentration = Some(Concentration::Bless);
            let mut other = b.clone();
            other.f[reactor].concentration = None;
            // A hidden effect never creates a new window or affects the mover’s main mask.
            assert_eq!(b.mask(), other.mask());
            b.step(Act::E).unwrap();
            other.step(Act::E).unwrap();
            assert_eq!(b.phase_id(), other.phase_id());
            assert_eq!(b.actor(), Some(reactor));
            assert_ne!(b.mask() & Act::DropConcentration.bit(), 0);
            assert_eq!(other.mask() & Act::DropConcentration.bit(), 0);
            b.step(Act::DropConcentration).unwrap();
            assert_eq!(b.f[reactor].concentration, None);
            assert!(b.f[reactor].reaction);
            assert_eq!(b.mask(), other.mask());
            assert_eq!(b.f[moving].pos.x, 1);
            b.step(Act::Pass).unwrap();
            assert_eq!(b.f[moving].pos.x, 2);
        }
    }

    #[test]
    fn declined_ready_expires_at_the_start_of_either_readiers_next_turn() {
        for actor in 0..2 {
            let foe = 1 - actor;
            let mut b = Battle::new(0, [Class::Fighter; 2], [1; 2]);
            b.owner = actor;
            b.f[actor].pos = Pos { x: 0, y: 0 };
            b.f[foe].pos = Pos { x: 3, y: 0 };
            b.step(Act::Ready).unwrap();
            b.step(ACTS[ReadyTrigger::Move as usize]).unwrap();
            b.step(Act::Dodge).unwrap();
            b.step(Act::End).unwrap();
            b.step(Act::E).unwrap();
            assert_eq!(b.actor(), Some(actor));
            b.step(Act::Pass).unwrap();
            assert!(b.f[actor].reaction);
            assert!(b.f[actor].readied.is_some());
            b.step(Act::End).unwrap();
            assert_eq!(b.actor(), Some(actor));
            assert!(b.f[actor].readied.is_none());
        }
    }

    #[test]
    fn readied_moonbeam_move_preserves_the_original_spell_and_does_not_pay_twice() {
        let mut b = Battle::new(0, [Class::Druid, Class::Fighter], [3, 3]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 3, y: 0 };
        b.f[1].hp = 100;
        b.step(Act::Spell3).unwrap();
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[3]).unwrap();
        if b.phase_id() == 19 {
            b.step(ACTS[0]).unwrap();
        }
        assert_eq!(b.f[0].concentration, Some(Concentration::Moonbeam));
        let paid = b.f[0].slots;
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        b.step(Act::Ready).unwrap();
        b.step(ACTS[ReadyTrigger::Move as usize]).unwrap();
        b.step(Act::Special5).unwrap();
        assert_eq!(b.f[0].concentration, Some(Concentration::Moonbeam));
        b.step(Act::End).unwrap();
        b.step(Act::E).unwrap();
        b.step(Act::Ready).unwrap();
        assert_eq!(b.phase_id(), 106);
        b.step(ACTS[5]).unwrap();
        if b.phase_id() == 106 {
            b.step(ACTS[0]).unwrap();
        }
        assert_eq!(b.magic_field(0), Some(Point::ground(Pos { x: 5, y: 0 })));
        assert_eq!(b.f[0].slots, paid);
        assert_eq!(b.f[0].concentration, Some(Concentration::Moonbeam));
        assert_eq!(b.actor(), Some(1));
        assert!(b.action);
    }
}
