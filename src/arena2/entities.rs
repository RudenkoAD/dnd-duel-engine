//! Statistics and lifecycles for spell-created creatures and objects.
use super::equipment::{DamageType, Material};
use super::footprint::Footprint;
use super::geometry::{Point, Pos};
use super::martial::EntityMartial;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spirit {
    Celestial,
    Fey,
    Fiend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FamiliarForm {
    Cat,
    Owl,
    Spider,
    Baboon,
    Badger,
    Bat,
    Crab,
    Deer,
    Eagle,
    Frog,
    GiantFireBeetle,
    GiantFly,
    Goat,
    Hawk,
    Hyena,
    Jackal,
    Lizard,
    Octopus,
    Piranha,
    Rat,
    Raven,
    Scorpion,
    Seahorse,
    Vulture,
    Weasel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Familiar { form: FamiliarForm, spirit: Spirit },
    Steed { spirit: Spirit, spell_level: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityStats {
    pub hp: i32,
    pub ac: i32,
    pub ground_speed: i32,
    pub fly_speed: i32,
    pub climb_speed: i32,
    pub swim_speed: i32,
    pub burrow_speed: i32,
    pub initiative_modifier: i32,
    pub darkvision: i32,
    pub blindsight: i32,
    pub water_only: bool,
    pub save_modifiers: [i32; 6],
    pub flyby: bool,
}

impl EntityKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Familiar { form, .. } => form.name(),
            Self::Steed { spirit, .. } => match spirit {
                Spirit::Celestial => "Celestial steed",
                Spirit::Fey => "Fey steed",
                Spirit::Fiend => "Fiend steed",
            },
        }
    }

    /// Arena size category with Tiny = 0, Small = 1, Medium = 2 and Large = 3.
    pub fn size_category(self) -> u8 {
        use FamiliarForm::*;
        match self {
            Self::Steed { .. } | Self::Familiar { form: GiantFly, .. } => 3,
            Self::Familiar {
                form: Deer | Goat | Hyena | Vulture,
                ..
            } => 2,
            Self::Familiar {
                form: Baboon | Eagle | GiantFireBeetle | Jackal | Octopus,
                ..
            } => 1,
            Self::Familiar { .. } => 0,
        }
    }

    pub fn stats(self) -> EntityStats {
        match self {
            Self::Familiar { form, .. } => form.stats(),
            Self::Steed { spell_level, .. } => EntityStats {
                hp: 5 + 10 * spell_level,
                ac: 10 + spell_level,
                ground_speed: 12,
                fly_speed: if spell_level >= 4 { 12 } else { 0 },
                climb_speed: 0,
                swim_speed: 0,
                burrow_speed: 0,
                initiative_modifier: 1,
                save_modifiers: [4, 1, 2, -2, 1, -1],
                darkvision: 0,
                blindsight: 0,
                water_only: false,
                flyby: false,
            },
        }
    }
}

// SRD 5.2's CR 0 beasts plus the explicitly listed Octopus from the
// 2024 Basic Rules. Speeds and sense ranges are in five-foot cells.
impl FamiliarForm {
    const fn name(self) -> &'static str {
        match self {
            Self::Cat => "Cat familiar",
            Self::Owl => "Owl familiar",
            Self::Spider => "Spider familiar",
            Self::Baboon => "Baboon familiar",
            Self::Badger => "Badger familiar",
            Self::Bat => "Bat familiar",
            Self::Crab => "Crab familiar",
            Self::Deer => "Deer familiar",
            Self::Eagle => "Eagle familiar",
            Self::Frog => "Frog familiar",
            Self::GiantFireBeetle => "Giant Fire Beetle familiar",
            Self::GiantFly => "Giant Fly familiar",
            Self::Goat => "Goat familiar",
            Self::Hawk => "Hawk familiar",
            Self::Hyena => "Hyena familiar",
            Self::Jackal => "Jackal familiar",
            Self::Lizard => "Lizard familiar",
            Self::Octopus => "Octopus familiar",
            Self::Piranha => "Piranha familiar",
            Self::Rat => "Rat familiar",
            Self::Raven => "Raven familiar",
            Self::Scorpion => "Scorpion familiar",
            Self::Seahorse => "Seahorse familiar",
            Self::Vulture => "Vulture familiar",
            Self::Weasel => "Weasel familiar",
        }
    }

    fn skill_proficiencies(self) -> &'static [Skill] {
        use FamiliarForm::*;
        match self {
            Weasel => &[Skill::Acrobatics, Skill::Perception, Skill::Stealth],
            Cat | Frog | Jackal | Octopus | Owl | Seahorse => &[Skill::Perception, Skill::Stealth],
            Badger | Deer | Eagle | Goat | Hawk | Hyena | Rat | Raven | Vulture => {
                &[Skill::Perception]
            }
            Crab | Spider => &[Skill::Stealth],
            _ => &[],
        }
    }

    fn stats(self) -> EntityStats {
        let (
            hp,
            ac,
            initiative_modifier,
            ground_speed,
            fly_speed,
            climb_speed,
            swim_speed,
            burrow_speed,
            save_modifiers,
            darkvision,
            blindsight,
        ) = match self {
            Self::Cat => (2, 12, 2, 8, 0, 8, 0, 0, [-4, 4, 0, -4, 1, -2], 12, 0),
            Self::Owl => (1, 11, 1, 1, 12, 0, 0, 0, [-4, 1, -1, -4, 1, -2], 24, 0),
            Self::Spider => (1, 12, 2, 4, 0, 4, 0, 0, [-4, 2, -1, -5, 0, -4], 6, 0),
            Self::Baboon => (3, 12, 2, 6, 0, 6, 0, 0, [-1, 2, 0, -3, 1, -2], 0, 0),
            Self::Badger => (5, 11, 0, 4, 0, 0, 0, 1, [0, 0, 3, -4, 1, -3], 6, 0),
            Self::Bat => (1, 12, 2, 1, 6, 0, 0, 0, [-4, 2, -1, -4, 1, -3], 0, 12),
            Self::Crab => (3, 11, 0, 4, 0, 0, 4, 0, [-2, 0, 1, -5, -1, -4], 0, 6),
            Self::Deer => (4, 13, 3, 10, 0, 0, 0, 0, [0, 3, 0, -4, 2, -3], 12, 0),
            Self::Eagle => (4, 12, 2, 2, 12, 0, 0, 0, [-2, 2, 1, -4, 2, -2], 0, 0),
            Self::Frog => (1, 11, 1, 4, 0, 0, 4, 0, [-5, 1, -1, -5, -1, -4], 6, 0),
            Self::GiantFireBeetle => (4, 13, 0, 6, 0, 6, 0, 0, [-1, 0, 1, -5, -2, -4], 0, 6),
            Self::GiantFly => (19, 11, 1, 6, 12, 0, 0, 0, [2, 1, 1, -4, 0, -4], 12, 0),
            Self::Goat => (4, 10, 0, 8, 0, 6, 0, 0, [2, 0, 0, -4, 0, -3], 12, 0),
            Self::Hawk => (1, 13, 3, 2, 12, 0, 0, 0, [-3, 3, -1, -4, 2, -2], 0, 0),
            Self::Hyena => (5, 11, 1, 10, 0, 0, 0, 0, [0, 1, 1, -4, 1, -3], 12, 0),
            Self::Jackal => (3, 12, 2, 8, 0, 0, 0, 0, [-1, 2, 0, -4, 1, -2], 18, 0),
            Self::Lizard => (2, 10, 0, 4, 0, 4, 0, 0, [-4, 0, 0, -5, -1, -4], 6, 0),
            Self::Octopus => (3, 12, 2, 1, 0, 0, 6, 0, [-3, 2, 0, -4, 0, -3], 6, 0),
            Self::Piranha => (1, 13, 3, 1, 0, 0, 8, 0, [-4, 3, -1, -5, -2, -4], 12, 0),
            Self::Rat => (1, 10, 0, 4, 0, 4, 0, 0, [-4, 0, -1, -4, 0, -3], 6, 0),
            Self::Raven => (2, 12, 2, 2, 10, 0, 0, 0, [-4, 2, 0, -3, 1, -2], 0, 0),
            Self::Scorpion => (1, 11, 0, 2, 0, 0, 0, 0, [-4, 0, -1, -5, -1, -4], 0, 2),
            Self::Seahorse => (1, 12, 1, 1, 0, 0, 4, 0, [-5, 1, -1, -5, 0, -4], 0, 0),
            Self::Vulture => (5, 10, 0, 2, 10, 0, 0, 0, [-2, 0, 1, -4, 1, -3], 0, 0),
            Self::Weasel => (1, 13, 3, 6, 0, 6, 0, 0, [-4, 3, -1, -4, 1, -4], 12, 0),
        };
        EntityStats {
            hp,
            ac,
            initiative_modifier,
            ground_speed,
            fly_speed,
            climb_speed,
            swim_speed,
            burrow_speed,
            save_modifiers,
            darkvision,
            blindsight,
            water_only: matches!(self, Self::Octopus | Self::Piranha | Self::Seahorse),
            flyby: self == Self::Owl,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub kind: EntityKind,
    pub side: usize,
    pub position: Pos,
    pub altitude: i8,
    pub martial: EntityMartial,
    pub hp: i32,
    pub temp_hp: i32,
    pub aid_bonus: i32,
    pub sanctuary: Option<(usize, i32)>,
    pub mage_armor_until: i32,
    pub longstrider_until: i32,
    pub jump_until: i32,
    pub initiative: i32,
    pub exhaustion: i32,
    dry_turns: i32,
    pub reaction: bool,
    pub dodging: bool,
    pub dismissed: bool,
    pub spirit_power: bool,
    pub turn: EntityTurn,
    pub(super) rope_inside: Option<usize>,
}

impl Entity {
    pub fn new(kind: EntityKind, side: usize, position: Pos, initiative: i32) -> Self {
        Self {
            kind,
            side,
            position,
            altitude: 0,
            martial: EntityMartial::default(),
            hp: kind.stats().hp,
            temp_hp: 0,
            aid_bonus: 0,
            sanctuary: None,
            mage_armor_until: 0,
            longstrider_until: 0,
            jump_until: 0,
            initiative,
            exhaustion: 0,
            dry_turns: 0,
            reaction: true,
            dodging: false,
            dismissed: false,
            spirit_power: matches!(kind, EntityKind::Steed { .. }),
            turn: EntityTurn::default(),
            rope_inside: None,
        }
    }

    pub fn begin_turn(&mut self) {
        self.reaction = true;
        self.dodging = false;
        self.martial.addled = false;
        self.martial.caltrops_stopped = false;
        self.turn = EntityTurn::new();
    }

    pub fn speed(&self) -> i32 {
        let stats = self.kind.stats();
        stats.ground_speed.max(stats.fly_speed)
    }

    pub fn movement_left(&self) -> i32 {
        (self.speed() + self.turn.dash - self.turn.moved).max(0)
    }

    pub fn damage(&mut self, amount: i32) {
        self.hp = (self.hp - amount.max(0)).max(0);
    }

    fn end_turn_in_dry_arena(&mut self) {
        let stats = self.kind.stats();
        if !stats.water_only {
            return;
        }
        self.dry_turns += 1;
        let breath_rounds = (10 * (1 + stats.save_modifiers[2])).max(5);
        if self.dry_turns > breath_rounds {
            self.exhaustion += 1;
            if self.exhaustion >= 6 {
                self.hp = 0;
            }
        }
    }

    pub fn dexterity_modifier(&self) -> i32 {
        if matches!(
            self.kind,
            EntityKind::Familiar {
                form: FamiliarForm::Cat,
                ..
            }
        ) {
            2
        } else {
            self.kind.stats().save_modifiers[1]
        }
    }

    pub fn max_hp(&self) -> i32 {
        self.kind.stats().hp + self.aid_bonus
    }

    pub fn heal(&mut self, amount: i32) {
        if self.present() {
            self.hp = (self.hp + amount.max(0)).min(self.max_hp());
        }
    }

    fn escape_modifier(&self, action: Act) -> i32 {
        let stats = self.kind.stats();
        let base = match (self.kind, action) {
            (
                EntityKind::Familiar {
                    form: FamiliarForm::Cat,
                    ..
                },
                Act::EscapeAcrobatics,
            ) => 2,
            (
                EntityKind::Familiar {
                    form: FamiliarForm::Goat,
                    ..
                },
                Act::EscapeAthletics,
            ) => 0,
            (
                EntityKind::Familiar {
                    form: FamiliarForm::Weasel,
                    ..
                },
                Act::EscapeAcrobatics,
            ) => 5,
            (_, Act::EscapeAthletics) => stats.save_modifiers[0],
            _ => stats.save_modifiers[1],
        };
        base - self.exhaustion * 2
    }

    pub fn point(&self) -> Point {
        Point {
            cell: self.position,
            altitude: self.altitude,
        }
    }

    pub fn present(&self) -> bool {
        self.hp > 0 && !self.dismissed
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityTurn {
    pub ended: bool,
    pub action: bool,
    pub(super) used_spell_jump: bool,
    pending_spell_jump: bool,
    pub bonus: bool,
    pub moved: i32,
    pub dash: i32,
    pub disengaged: bool,
}

impl EntityTurn {
    pub fn new() -> Self {
        Self {
            action: true,
            used_spell_jump: false,
            pending_spell_jump: false,
            bonus: true,
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steed_lifecycle_and_speeds_follow_spell_level() {
        let kind = EntityKind::Steed {
            spirit: Spirit::Celestial,
            spell_level: 2,
        };
        let mut steed = Entity::new(kind, 0, Pos { x: 1, y: 1 }, 15);
        assert_eq!(steed.hp, 25);
        assert_eq!(kind.stats().ac, 12);
        assert_eq!(kind.stats().fly_speed, 0);
        assert_eq!(steed.speed(), 12);
        assert!(!steed.turn.action);
        steed.begin_turn();
        assert!(steed.turn.action);
        assert_eq!(steed.movement_left(), 12);
        steed.damage(14);
        steed.heal(20);
        assert_eq!(steed.hp, 25);
        steed.damage(27);
        steed.heal(20);
        assert!(!steed.present());
    }

    #[test]
    fn water_only_familiar_suffocates_in_the_dry_arena_after_breath_runs_out() {
        let mut piranha = Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Piranha,
                spirit: Spirit::Fey,
            },
            0,
            Pos { x: 1, y: 0 },
            6,
        );
        for _ in 0..5 {
            piranha.end_turn_in_dry_arena();
            assert_eq!(piranha.exhaustion, 0);
        }
        for expected in 1..=6 {
            piranha.end_turn_in_dry_arena();
            assert_eq!(piranha.exhaustion, expected);
        }
        assert!(!piranha.present());
    }

    #[test]
    fn familiars_keep_distinct_movement_and_defense_stats() {
        let stats = |form| {
            EntityKind::Familiar {
                form,
                spirit: Spirit::Fey,
            }
            .stats()
        };
        let cat = stats(FamiliarForm::Cat);
        let owl = stats(FamiliarForm::Owl);
        let spider = stats(FamiliarForm::Spider);
        assert_eq!(cat.ground_speed, 8);
        assert_eq!(cat.save_modifiers[1], 4);
        assert!(owl.flyby);
        assert_eq!(owl.fly_speed, 12);
        assert_eq!(spider.climb_speed, 4);
    }
}

use super::battle::Battle;
use super::class::{Class, NAMES};
use super::common::ReadyTrigger;
use super::geometry::{HEIGHT, WIDTH};
use super::origins::Skill;
use super::{Act, ACTS};

const SUMMON_STEED_RANK: usize = 2;
pub const FORMS: [FamiliarForm; 25] = [
    FamiliarForm::Cat,
    FamiliarForm::Owl,
    FamiliarForm::Spider,
    FamiliarForm::Baboon,
    FamiliarForm::Badger,
    FamiliarForm::Bat,
    FamiliarForm::Crab,
    FamiliarForm::Deer,
    FamiliarForm::Eagle,
    FamiliarForm::Frog,
    FamiliarForm::GiantFireBeetle,
    FamiliarForm::GiantFly,
    FamiliarForm::Goat,
    FamiliarForm::Hawk,
    FamiliarForm::Hyena,
    FamiliarForm::Jackal,
    FamiliarForm::Lizard,
    FamiliarForm::Octopus,
    FamiliarForm::Piranha,
    FamiliarForm::Rat,
    FamiliarForm::Raven,
    FamiliarForm::Scorpion,
    FamiliarForm::Seahorse,
    FamiliarForm::Vulture,
    FamiliarForm::Weasel,
];
const SPIRITS: [Spirit; 3] = [Spirit::Celestial, Spirit::Fey, Spirit::Fiend];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityPhase {
    Form(Act),
    Row {
        action: Act,
        kind: EntityKind,
    },
    Column {
        action: Act,
        kind: EntityKind,
        row: i8,
    },
    Turn(usize),
    Jump {
        side: usize,
        high: bool,
    },
    Dismount,
    PowerTarget {
        side: usize,
        resume: bool,
    },
    PowerRow {
        side: usize,
        resume: bool,
    },
    PowerColumn {
        side: usize,
        resume: bool,
        row: i8,
    },
    Strike(usize),
    Help(usize),
    Payment(Act),
    Leaving {
        side: usize,
        destination: Point,
        cost: i32,
    },
    SteedOpportunity {
        side: usize,
        spell_mover: Option<usize>,
    },
    SteedOpportunityTarget {
        side: usize,
        spell_move: bool,
    },
    ResumePlayerMove {
        side: usize,
        spell_move: bool,
    },
    SteedIdentity {
        action: Act,
        kind: EntityKind,
    },
    ResumeMove {
        side: usize,
        destination: Point,
        cost: i32,
    },
}

impl EntityPhase {
    pub fn id(self) -> i32 {
        match self {
            Self::Form(_) => 50,
            Self::Row { .. } => 51,
            Self::Column { .. } => 52,
            Self::Turn(_) => 53,
            Self::Dismount => 54,
            Self::PowerTarget { .. } => 55,
            Self::PowerRow { .. } => 56,
            Self::PowerColumn { .. } => 57,
            Self::Strike(_) => 58,
            Self::Help(_) => 59,
            Self::Jump { .. } => 55,
            Self::Payment(_) => 60,
            Self::Leaving { .. } => 61,
            Self::ResumeMove { .. } => 62,
            Self::SteedOpportunity { .. } => 63,
            Self::SteedOpportunityTarget { .. } => 58,
            Self::ResumePlayerMove { .. } => 64,
            Self::SteedIdentity { .. } => 65,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entities {
    pub mounted: [bool; 2],
    companion: [Option<Entity>; 2],
    initiatives: [i32; 2],
    steed_free: [bool; 2],
    reuse_steed_identity: [bool; 2],
    mounted_start: [i32; 2],
    next: Vec<usize>,
    help_target: [Option<usize>; 2],
    spawn_slot: [usize; 2],
    frightened: [Option<(usize, i32)>; 4],
    player_move_asked: bool,
    shared_senses: [bool; 2],
}

impl Entities {
    #[cfg(test)]
    pub(super) fn test_install(&mut self, companion: Entity) {
        let side = companion.side;
        self.companion[side] = Some(companion);
    }

    pub fn new(initiatives: [i32; 2]) -> Self {
        Self {
            mounted: [false; 2],
            companion: [None, None],
            initiatives,
            steed_free: [true; 2],
            reuse_steed_identity: [false; 2],
            mounted_start: [-1; 2],
            next: Vec::new(),
            help_target: [None; 2],
            spawn_slot: [0; 2],
            frightened: [None; 4],
            player_move_asked: false,
            shared_senses: [false; 2],
        }
    }
}

pub(crate) fn entity_action_name(class: Class, action: Act) -> Option<&'static str> {
    use Act::*;
    use Class::*;
    Some(match (class, action) {
        (Druid, Entity1) => "Wild Companion (Wild Shape)",
        (Druid, Entity2) => "Wild Companion (spell slot)",
        (Druid | Wizard | Warlock, Entity3) => "Dismiss Familiar Temporarily",
        (Druid | Wizard | Warlock, Entity4) => "Recall Familiar",
        (Druid | Wizard | Warlock, Entity5) => "Dismiss Familiar Permanently",
        (Druid | Wizard | Warlock, Entity6) => "Share Familiar Senses",
        (Paladin, Entity1) => "Find Steed (Faithful Steed)",
        (Paladin, Entity2) => "Find Steed (spell slot)",
        (Paladin, Entity3) => "Mount Steed",
        (Paladin, Entity4) => "Dismount",
        (Paladin, Entity5) => "Steed Dash",
        (Paladin, Entity6) => "Steed Disengage",
        (Paladin, Entity7) => "Steed Dodge",
        (Paladin, Entity8) => "Steed Power",
        _ => return None,
    })
}

impl Battle {
    #[cfg(test)]
    pub(super) fn fixture_companion(&mut self, side: usize, entity: Entity) {
        let size = entity.kind.size_category();
        let footprint = self.footprint_fit_kind(2 + side, entity.point(), size, false, false);
        self.footprint_squeezed[2 + side] = footprint.expect("fixture fits on the arena");
        self.entities.companion[side] = Some(entity);
    }

    pub fn entity(&self, id: usize) -> Option<&Entity> {
        self.entities
            .companion
            .get(id.checked_sub(2)?)?
            .as_ref()
            .filter(|entity| entity.present())
    }

    pub fn observed_entity(&self, viewer: usize, id: usize) -> Option<&Entity> {
        let entity = self.entity(id)?;
        if viewer == entity.side || self.entity_visible(viewer, id) {
            self.entity(id)
        } else {
            None
        }
    }

    pub fn active_entity_id(&self) -> Option<usize> {
        use EntityPhase::*;
        let side = match self.entity_phase()? {
            Turn(side)
            | Jump { side, .. }
            | Strike(side)
            | Help(side)
            | SteedOpportunity { side, .. }
            | SteedOpportunityTarget { side, .. }
            | PowerTarget { side, resume: true }
            | PowerRow { side, resume: true }
            | PowerColumn {
                side, resume: true, ..
            } => side,
            _ => return None,
        };
        Some(2 + side)
    }

    pub(super) fn entity_mut(&mut self, id: usize) -> Option<&mut Entity> {
        self.entities
            .companion
            .get_mut(id.checked_sub(2)?)?
            .as_mut()
            .filter(|entity| entity.present())
    }

    pub fn entity_at(&self, pos: Pos) -> Option<usize> {
        let target = Footprint::point(Point::ground(pos));
        (2..4).find(|&id| {
            self.footprint(id)
                .is_some_and(|region| region.overlaps(target))
        })
    }

    pub fn entity_position(&self, id: usize) -> Option<Pos> {
        self.entity(id).map(|entity| entity.position)
    }

    pub(super) fn entity_can_deliver_touch(&self, side: usize, target: usize) -> bool {
        let id = 2 + side;
        let known_target = self.observed_footprint(side, target).or_else(|| {
            self.entity_visible(id, target)
                .then(|| self.footprint(target))
                .flatten()
        });
        self.entity(id).is_some_and(|familiar| {
            matches!(familiar.kind, EntityKind::Familiar { .. })
                && familiar.reaction
                && !familiar.martial.stunned(self.turns)
                && !self.magic_no_reactions(id)
                && !self.magic_area_incapacitated(id)
                && self.combatant_distance(side, id).is_some_and(|d| d <= 20)
                && self
                    .footprint(id)
                    .zip(known_target)
                    .is_some_and(|(from, to)| {
                        from.squares().any(|origin| {
                            to.squares().any(|target| {
                                origin.dist(target) <= 1 && self.map.los_at(origin, target)
                            })
                        })
                    })
        })
    }

    pub(super) fn entity_touch_reaction(&mut self, side: usize, target: usize) -> bool {
        if !self.entity_can_deliver_touch(side, target) {
            return false;
        }
        self.entity_mut(2 + side).unwrap().reaction = false;
        true
    }

    pub fn familiar_sight(&self, observer: usize, subject: usize) -> bool {
        if !self
            .entities
            .shared_senses
            .get(observer)
            .copied()
            .unwrap_or(false)
        {
            return false;
        }
        let Some(familiar) = self.entity(2 + observer) else {
            return false;
        };
        let Some(target) = self.creature_point(subject) else {
            return false;
        };
        if self.entity_shared_blindsight(observer, subject) {
            return true;
        }
        let hidden = self
            .f
            .get(subject)
            .is_some_and(|fighter| fighter.hidden_dc.is_some());
        !hidden
            && self.map.los_at(familiar.point(), target)
            && !self.magic_cloud_blocks(familiar.point(), target)
    }

    pub(super) fn entity_shared_blindsight(&self, observer: usize, subject: usize) -> bool {
        if !self
            .entities
            .shared_senses
            .get(observer)
            .copied()
            .unwrap_or(false)
        {
            return false;
        }
        self.entity(2 + observer)
            .filter(|entity| matches!(entity.kind, EntityKind::Familiar { .. }))
            .zip(self.creature_point(subject))
            .is_some_and(|(familiar, subject)| {
                let range = familiar.kind.stats().blindsight;
                range > 0
                    && familiar.point().dist(subject) <= range
                    && self.map.los_at(familiar.point(), subject)
            })
    }

    pub fn entity_visible(&self, observer: usize, id: usize) -> bool {
        if observer < 2 && id < 2 {
            return self.can_see(observer, id);
        }
        if !self.rope_can_see(observer, id) {
            return false;
        }
        let Some((from, to)) = self.footprint(observer).zip(self.footprint(id)) else {
            return false;
        };
        let from = self.portal_viewpoint(observer, id)
            .map(super::footprint::Footprint::point).unwrap_or(from);
        let sense_range = self
            .entity(observer)
            .map_or(0, |entity| entity.kind.stats().blindsight);
        let known = id >= 2 || self.observed_pos(observer % 2, id).is_some();
        from.squares().any(|a| {
            to.squares().any(|b| {
                let sensing = sense_range > 0 && a.dist(b) <= sense_range;
                self.map.los_at(a, b) && (sensing || known && (!self.magic_invisible(id) || self.arcane_sees_invisible(observer)) && !self.magic_cloud_blocks(a, b) && self.darkness_allows_sight(observer, id, a, b))
            })
        })
    }

    pub fn creature_point(&self, id: usize) -> Option<Point> {
        self.f
            .get(id)
            .map(|_| self.fighter_point(id))
            .or_else(|| self.entity(id).map(Entity::point))
    }

    pub fn entity_damage(&mut self, id: usize, amount: i32) {
        let amount = self.fighter_resolve_fields(id, amount);
        self.entity_damage_resolved(id, amount);
    }

    pub(super) fn entity_damage_resolved(&mut self, id: usize, amount: i32) -> i32 {
        if self.entity(id).is_none() || amount <= 0 { return 0; }
        let entity = self.entity_mut(id).expect("existing damage target");
        let absorbed = amount.max(0).min(entity.temp_hp);
        entity.temp_hp -= absorbed;
        entity.damage(amount - absorbed);
        if !entity.present() {
            self.entities.mounted[id - 2] = false;
            self.entities.help_target[id - 2] = None;
            self.magic_mark_target_downed(id);
        }
        if amount > 0 {
            self.magic_area_hurt(id);
        }
        amount
    }

    pub fn entity_damage_typed(&mut self, id: usize, amount: i32, kind: DamageType) {
        self.entity_damage_typed_resolved(id, amount, kind);
    }

    pub(super) fn entity_damage_typed_resolved(&mut self, id: usize, amount: i32, kind: DamageType) -> i32 {
        let adjusted = self.damage_parts_after_resistance(id, &mut [(amount, kind)]);
        self.entity_damage_resolved(id, adjusted)
    }

    pub(super) fn entity_damage_after_resistance(&mut self, id: usize, amount: i32, kind: DamageType) -> i32 {
        let resistant = self.entity(id).is_some_and(|entity| {
            matches!(
                (entity.kind, kind),
                (
                    EntityKind::Familiar {
                        form: FamiliarForm::Badger,
                        ..
                    },
                    DamageType::Poison
                ) | (
                    EntityKind::Familiar {
                        form: FamiliarForm::GiantFireBeetle,
                        ..
                    },
                    DamageType::Fire
                )
            )
        });
        let reduced = self.magic_flat_protection(id, amount, kind);
        let resistant = resistant || self.magic_energy_resistance(id, kind);
        if resistant { reduced / 2 } else { reduced }
    }

    pub(super) fn entity_save(&mut self, id: usize, stat: usize, dc: i32) -> bool {
        self.entity_save_with_cover(id, stat, dc, 0, false, false)
    }

    pub(in crate::arena2) fn entity_save_with_advantage(
        &mut self,
        id: usize,
        stat: usize,
        dc: i32,
        advantage: bool,
    ) -> bool {
        self.entity_save_with_cover(id, stat, dc, 0, false, advantage)
    }

    pub(super) fn entity_save_from(
        &mut self,
        id: usize,
        stat: usize,
        dc: i32,
        origin: Point,
        disadvantage: bool,
    ) -> bool {
        let Some(destination) = self.creature_point(id) else {
            return false;
        };
        let cover = if stat == 1 {
            self.map.cover_at(origin, destination).bonus()
        } else {
            0
        };
        self.entity_save_with_cover(id, stat, dc, cover, disadvantage, false)
    }

    fn entity_save_with_cover(
        &mut self,
        id: usize,
        stat: usize,
        dc: i32,
        cover: i32,
        disadvantage: bool,
        forced_advantage: bool,
    ) -> bool {
        let Some(entity) = self.entity(id) else {
            return false;
        };
        if matches!(stat, 0 | 1) && (entity.martial.stunned(self.turns) || self.sleep_unconscious(id)) {
            return false;
        }
        let modifier = entity.kind.stats().save_modifiers[stat] - 2 * entity.exhaustion;
        let advantage =
            forced_advantage || stat == 1 && entity.dodging && self.entity_effective_speed(id) > 0;
        let disadvantage = disadvantage || stat == 1 && self.magic_area_restrained(id);
        let blessing = if self.magic_blessed(id) {
            self.roll(4)
        } else {
            0
        };
        let total = self.d20_for(id, advantage, disadvantage) + modifier + blessing + cover;
        self.inspired_save(id, stat, total, dc, false)
    }

    pub fn entity_heal(&mut self, id: usize, amount: i32) {
        if let Some(entity) = self.entity_mut(id) {
            entity.heal(amount);
        }
    }

    pub(super) fn entity_life_bond(&mut self, side: usize, restored: i32) {
        let id = 2 + side;
        if self.entity(id).is_some_and(|steed| {
            matches!(steed.kind, EntityKind::Steed { .. })
                && steed.position.dist(self.f[side].pos) <= 1
        }) {
            self.entity_heal(id, restored);
        }
    }

    pub(super) fn entity_help_attack(&mut self, side: usize, target: usize) -> bool {
        if self.entities.help_target[side] == Some(target) {
            self.entities.help_target[side] = None;
            true
        } else {
            false
        }
    }

    pub(super) fn entity_main_mask(&self) -> u128 {
        use Act::*;
        let i = self.owner;
        let me = &self.f[i];
        let magic_action = self.action && !self.incap(i) && !self.magic_quiet_turn(i);
        let can_cast = magic_action
            && !self.magic_is_wolf(i)
            && self.can_cast_components(i, true, true, Material::None);
        let current = self.entities.companion[i].as_ref();
        let have_space = |range| (0..HEIGHT).any(|row| self.entity_column_mask(i, range, row) != 0);
        let mut mask = 0;
        let mut allow = |action: Act, condition| {
            if condition {
                mask |= action.bit();
            }
        };
        match me.build.class {
            Class::Druid if me.build.level >= 2 => {
                if can_cast {
                    let slot = (1..=3).any(|level| self.can_slot(i, level));
                    let summon = (slot || me.resource > 0) && have_space(2);
                    allow(Entity1, summon && me.resource > 0);
                    allow(Entity2, summon && slot);
                }
            }
            Class::Paladin if me.build.level >= 5 => {
                if can_cast {
                    let slot = self.can_slot(i, SUMMON_STEED_RANK);
                    let summon = (slot || self.entities.steed_free[i]) && have_space(6);
                    allow(Entity1, summon && self.entities.steed_free[i]);
                    allow(Entity2, summon && slot);
                }
                if let Some(steed) = current.filter(|steed| steed.present()) {
                    let mounted = self.entities.mounted[i];
                    let mount_cost = self.speed(i) / 2;
                    let able_to_mount = !self.incap(i)
                        && !me.prone
                        && (self.speed(i) + self.dash - self.moved) >= mount_cost;
                    allow(
                        Entity3,
                        !mounted
                            && able_to_mount
                            && self
                                .combatant_distance(i, 2 + i)
                                .is_some_and(|distance| distance <= 1),
                    );
                    allow(
                        Entity4,
                        mounted && able_to_mount && self.dismount_mask(i) != 0,
                    );
                    if mounted && !self.incap(i) {
                        allow(Entity5, steed.turn.action);
                        allow(Entity6, steed.turn.action);
                        allow(Entity7, steed.turn.action);
                        allow(Entity8, self.steed_power_mask(i) != 0);
                    }
                }
            }
            _ => {}
        }
        if let Some(familiar) = current.filter(|familiar| {
            matches!(familiar.kind, EntityKind::Familiar { .. }) && familiar.hp > 0
        }) {
            allow(Entity3, magic_action && familiar.present());
            allow(Entity4, magic_action && familiar.dismissed && have_space(6));
            allow(Entity5, magic_action);
            allow(
                Entity6,
                self.bonus
                    && !self.incap(i)
                    && familiar.present()
                    && !self.entities.shared_senses[i]
                    && self.fighter_point(i).dist(familiar.point()) <= 20,
            );
        }
        mask
    }

    pub(super) fn entity_main_step(&mut self, action: Act) -> bool {
        use Act::*;
        let side = self.owner;
        let class = self.f[side].build.class;
        if entity_action_name(class, action).is_none() {
            return false;
        }
        match (class, action) {
            (Class::Druid | Class::Paladin, Entity1 | Entity2) => {
                let phase = if class == Class::Druid && action == Entity2 {
                    EntityPhase::Payment(action)
                } else {
                    EntityPhase::Form(action)
                };
                self.set_entity_phase(Some(phase));
            }
            (Class::Paladin, Entity3) => {
                self.moved += self.speed(side) / 2;
                self.f[side].pos = self.entity(2 + side).unwrap().position;
                self.entities.mounted[side] = true;
                self.start_controlled_mount(side);
            }
            (Class::Paladin, Entity4) => self.set_entity_phase(Some(EntityPhase::Dismount)),
            (Class::Paladin, Entity5 | Entity6 | Entity7) => {
                let steed = self.entity_mut(2 + side).unwrap();
                steed.turn.action = false;
                match action {
                    Entity5 => steed.turn.dash += steed.speed(),
                    Entity6 => steed.turn.disengaged = true,
                    _ => steed.dodging = true,
                }
            }
            (Class::Paladin, Entity8) => self.open_steed_power(side, false),
            (_, Entity3) => {
                self.action = false;
                self.entities.help_target[side] = None;
                self.entity_mut(2 + side).unwrap().dismissed = true;
            }
            (_, Entity4) => {
                let kind = self.entities.companion[side].as_ref().unwrap().kind;
                self.set_entity_phase(Some(EntityPhase::Row { action, kind }));
            }
            (_, Entity6) => {
                self.bonus = false;
                self.entities.shared_senses[side] = true;
            }
            (_, Entity5) => {
                self.action = false;
                self.entities.help_target[side] = None;
                self.entities.companion[side] = None;
            }
            _ => return false,
        }
        true
    }

    fn placement_range(&self, i: usize, action: Act) -> i32 {
        if action == Act::Entity4 || self.f[i].build.class == Class::Paladin {
            6
        } else {
            2
        }
    }

    fn space_free(&self, position: Pos) -> bool {
        self.entity_point_free(Point::ground(position))
    }

    fn entity_known_free(&self, side: usize, point: Point) -> bool {
        let square = Footprint::point(point);
        self.map.can_occupy(point)
            && (0..4).all(|id| {
                self.observed_footprint(side, id)
                    .is_none_or(|body| !body.overlaps(square))
            })
    }

    fn entity_column_mask(&self, side: usize, range: i32, row: i8) -> u128 {
        let origin = self.f[side].pos;
        (0..WIDTH).fold(0, |mask, column| {
            let position = Pos { x: column, y: row };
            if origin.dist(position) <= range
                && self.map.los(origin, position)
                && self.entity_known_free(side, Point::ground(position))
            {
                mask | ACTS[column as usize].bit()
            } else {
                mask
            }
        })
    }

    fn dismount_mask(&self, side: usize) -> u128 {
        (0..8).fold(0, |mask, dir| {
            if self
                .map
                .step(self.f[side].pos, dir)
                .is_some_and(|(destination, _)| {
                    self.entity_known_free(side, Point::ground(destination))
                })
            {
                mask | ACTS[dir].bit()
            } else {
                mask
            }
        })
    }
}

impl Battle {
    pub(super) fn entity_phase_actor(&self, phase: EntityPhase) -> usize {
        match phase {
            EntityPhase::Turn(side)
            | EntityPhase::Jump { side, .. }
            | EntityPhase::Strike(side)
            | EntityPhase::Help(side)
            | EntityPhase::PowerTarget { side, .. }
            | EntityPhase::PowerRow { side, .. }
            | EntityPhase::PowerColumn { side, .. } => side,
            EntityPhase::Leaving { side, .. } => 1 - side,
            EntityPhase::ResumeMove { side, .. }
            | EntityPhase::SteedOpportunity { side, .. }
            | EntityPhase::SteedOpportunityTarget { side, .. }
            | EntityPhase::ResumePlayerMove { side, .. } => side,
            _ => self.owner,
        }
    }

    pub(super) fn entity_phase_mask(&self, phase: EntityPhase) -> u128 {
        use EntityPhase::*;
        let side = self.entity_phase_actor(phase);
        match phase {
            SteedIdentity { .. } => 0b11,
            Form(_) => {
                let options = if self.f[side].build.class == Class::Paladin {
                    SPIRITS.len()
                } else {
                    FORMS.len()
                };
                (1u128 << options) - 1
            }
            Payment(_) => (1..=3).fold(0, |mask, tier| {
                mask | if self.can_slot(side, tier) {
                    ACTS[tier - 1].bit()
                } else {
                    0
                }
            }),
            Row { action, .. } => self.entity_row_mask(side, self.placement_range(side, action)),
            Column { action, row, .. } => {
                self.entity_column_mask(side, self.placement_range(side, action), row)
            }
            Turn(side) => self.companion_turn_mask(side),
            Jump { side, high } => self.entity_jump_mask(side, high) | Act::Pass.bit(),
            Dismount => self.dismount_mask(side),
            PowerTarget { side, .. } => self.steed_power_mask(side),
            PowerRow { side, .. } => (0..HEIGHT).fold(0, |mask, row| {
                mask | if self.teleport_column_mask(side, row) != 0 {
                    ACTS[row as usize].bit()
                } else {
                    0
                }
            }),
            PowerColumn { side, row, .. } => self.teleport_column_mask(side, row),
            Strike(side) => self.entity_attack_targets(side),
            Help(side) => self.entity_near_enemies(side) | self.entity_help_skills(side),
            SteedOpportunityTarget { side, .. } => self.steed_opportunity_targets(side),
            Leaving { .. } | SteedOpportunity { .. } => {
                Act::Opportunity.bit() | Act::Pass.bit()
                    | if matches!(phase, Leaving { .. }) && self.rogue_blade_available(1 - side) {
                        Act::Special3.bit()
                    } else { 0 }
            },
            ResumePlayerMove { .. } | ResumeMove { .. } => {
                unreachable!("move continuation is internal")
            }
        }
    }

    fn entity_row_mask(&self, side: usize, range: i32) -> u128 {
        (0..HEIGHT).fold(0, |mask, row| {
            mask | if self.entity_column_mask(side, range, row) != 0 {
                ACTS[row as usize].bit()
            } else {
                0
            }
        })
    }

    pub(super) fn entity_phase_step(&mut self, phase: EntityPhase, choice: Act) {
        use EntityPhase::*;
        let side = self.entity_phase_actor(phase);
        let index = choice as usize;
        match phase {
            Payment(action) => {
                self.entities.spawn_slot[side] = index + 1;
                self.set_entity_phase(Some(Form(action)));
            }
            Form(action) => {
                let kind = if self.f[side].build.class == Class::Paladin {
                    EntityKind::Steed {
                        spirit: SPIRITS[index],
                        spell_level: SUMMON_STEED_RANK as i32,
                    }
                } else {
                    EntityKind::Familiar {
                        form: FORMS[index],
                        spirit: Spirit::Fey,
                    }
                };
                self.entities.reuse_steed_identity[side] = false;
                let can_restore_steed = matches!(kind, EntityKind::Steed { .. })
                    && self.entities.companion[side]
                        .as_ref()
                        .is_some_and(|old| old.hp == 0 && old.kind == kind);
                if can_restore_steed {
                    self.set_entity_phase(Some(SteedIdentity { action, kind }));
                } else {
                    self.set_entity_phase(Some(Row { action, kind }));
                }
            }
            SteedIdentity { action, kind } => {
                self.entities.reuse_steed_identity[side] = index == 0;
                self.set_entity_phase(Some(Row { action, kind }));
            }
            Row { action, kind } => self.set_entity_phase(Some(Column {
                action,
                kind,
                row: index as i8,
            })),
            Column { action, kind, row } => {
                self.spawn_companion(
                    side,
                    action,
                    kind,
                    Pos {
                        x: index as i8,
                        y: row,
                    },
                );
                self.set_entity_phase(None);
            }
            Turn(side) => self.companion_turn_step(side, choice),
            Dismount => {
                let destination = self.map.step(self.f[side].pos, index).unwrap().0;
                if !self.space_free(destination) {
                    if self.f[1 - side].pos == destination {
                        self.reveal(1 - side);
                    }
                    self.set_entity_phase(None);
                    return;
                }
                self.entities.mounted[side] = false;
                self.moved += self.speed(side) / 2;
                self.f[side].pos = destination;
                self.set_entity_phase(None);
            }
            PowerRow { side, resume } => {
                self.set_entity_phase(Some(PowerColumn {
                    side,
                    resume,
                    row: index as i8,
                }));
            }
            PowerColumn { side, resume, row } => {
                let destination = Pos {
                    x: index as i8,
                    y: row,
                };
                self.power_cost(side, Spirit::Fey);
                if !self.commit_footprint(2 + side, Point::ground(destination), false) {
                    self.finish_power(side, resume);
                    return;
                }
                let previous = self.creature_point(2 + side).unwrap();
                let steed = self.entity_mut(2 + side).unwrap();
                steed.position = destination;
                steed.altitude = 0;
                if self.entities.mounted[side] {
                    self.f[side].pos = destination;
                    self.f[side].elevation = 0;
                }
                self.martial_moved(2 + side);
                self.magic_area_moved(2 + side, previous);
                self.finish_power(side, resume);
            }
            PowerTarget { side, resume } => {
                self.apply_steed_power(side, index);
                self.finish_power(side, resume);
            }
            Jump { side, high } => {
                if choice == Act::Pass {
                    self.set_entity_phase(Some(Turn(side)));
                    return;
                }
                let destination = self.entity_jump_destination(side, high, index).unwrap();
                let turn = &mut self.entity_mut(2 + side).unwrap().turn;
                turn.used_spell_jump = true;
                turn.pending_spell_jump = true;
                if self.leaving_provokes(side, destination) {
                    self.set_entity_phase(Some(Leaving {
                        side,
                        destination,
                        cost: 2,
                    }));
                } else {
                    self.set_entity_phase(Some(Turn(side)));
                    self.commit_entity_move(side, destination, 2);
                }
            }
            Strike(side) => {
                let steed = self.entity_mut(2 + side).unwrap();
                steed.turn.action = false;
                let build = self.f[side].build;
                self.set_entity_phase(Some(Turn(side)));
                self.entity_weapon_attack(
                    side,
                    index,
                    8,
                    SUMMON_STEED_RANK as i32,
                    build.pb + build.casting,
                );
            }
            Leaving {
                side,
                destination,
                cost,
            } => {
                if matches!(choice, Act::Opportunity | Act::Special3) {
                    self.set_entity_phase(Some(ResumeMove {
                        side,
                        destination,
                        cost,
                    }));
                    self.entity_start_opportunity(1 - side, 2 + side, choice == Act::Special3);
                } else {
                    self.entity_resume_after_attack(ResumeMove {
                        side,
                        destination,
                        cost,
                    });
                }
            }
            SteedOpportunity { side, spell_mover } => {
                let mover = spell_mover.unwrap_or(1 - side);
                let spell_move = spell_mover.is_some();
                if choice == Act::Opportunity {
                    if self.entities.mounted[1 - side] && self.steed_opportunity_targets(side).count_ones() > 1 {
                        self.set_entity_phase(Some(SteedOpportunityTarget { side, spell_move }));
                    } else {
                        let target = if self.entities.mounted[1 - side] {
                            self.steed_opportunity_targets(side).trailing_zeros() as usize
                        } else { mover };
                        self.resolve_steed_opportunity(side, target, spell_move);
                    }
                } else {
                    self.entity_resume_after_attack(ResumePlayerMove { side, spell_move });
                }
            }
            SteedOpportunityTarget { side, spell_move } => self.resolve_steed_opportunity(side, index, spell_move),
            ResumePlayerMove { .. } | ResumeMove { .. } => {
                unreachable!("move continuation is internal")
            }
            Help(side) => {
                self.entity_mut(2 + side).unwrap().turn.action = false;
                if index < 4 {
                    self.entities.help_target[side] = Some(index);
                } else {
                    let skill = self
                        .entity_help_skill(side, choice)
                        .expect("legal skill help");
                    self.help_skill[side] = Some(skill);
                }
                self.set_entity_phase(Some(Turn(side)));
                self.notify_companion_event(2 + side, ReadyTrigger::Action);
            }
        }
    }

    fn spawn_companion(&mut self, side: usize, action: Act, kind: EntityKind, position: Pos) {
        self.action = false;
        let footprint = self.footprint_fit_kind(
            2 + side,
            Point::ground(position),
            kind.size_category(),
            false,
            false,
        );
        let available = footprint.is_some();
        if action == Act::Entity4 {
            if !available {
                return;
            }
            let familiar = self.entities.companion[side].as_mut().unwrap();
            familiar.dismissed = false;
            familiar.position = position;
            familiar.altitude = 0;
            self.footprint_squeezed[2 + side] = footprint.unwrap();
            return;
        }
        let initiative = match kind {
            EntityKind::Steed { .. } => {
                if action == Act::Entity1 {
                    self.entities.steed_free[side] = false;
                } else {
                    assert!(self.spend_slot(side, SUMMON_STEED_RANK));
                }
                self.entities.initiatives[side]
            }
            EntityKind::Familiar { form, .. } => {
                if action == Act::Entity1 {
                    self.f[side].resource -= 1;
                } else {
                    assert!(self.spend_slot(side, self.entities.spawn_slot[side]));
                }
                if available {
                    self.roll(20) + form.stats().initiative_modifier
                } else {
                    0
                }
            }
        };
        if !available {
            return;
        }
        self.entities.mounted[side] = false;
        self.entities.help_target[side] = None;
        self.magic_forget_companion_benefits(side);
        let mut entity = Entity::new(kind, side, position, initiative);
        if self.entities.reuse_steed_identity[side] {
            entity.spirit_power = self.entities.companion[side].as_ref().unwrap().spirit_power;
        }
        self.magic_forget_replaced_mark(2 + side);
        self.entities.companion[side] = Some(entity);
        self.footprint_squeezed[2 + side] = footprint.unwrap();
    }

    pub(super) fn entity_choice_label(&self, viewer: usize, action: Act) -> Option<String> {
        use EntityPhase::*;
        let index = action as usize;
        let own_phase = self.entity_phase();
        Some(match own_phase {
            Some(SteedIdentity { .. }) => {
                if index == 0 {
                    "Return the same steed".to_owned()
                } else {
                    "Summon a different steed".to_owned()
                }
            }
            Some(Payment(_)) => format!("Use level {} spell slot", index + 1),
            Some(Form(_)) if self.f[viewer].build.class == Class::Paladin => {
                match SPIRITS.get(index)? {
                    Spirit::Celestial => "Celestial steed",
                    Spirit::Fey => "Fey steed",
                    Spirit::Fiend => "Fiend steed",
                }
                .to_owned()
            }
            Some(Form(_)) => EntityKind::Familiar {
                form: *FORMS.get(index)?,
                spirit: Spirit::Fey,
            }
            .name()
            .to_owned(),
            Some(Row { .. } | PowerRow { .. }) => format!("Choose row {}", index + 1),
            Some(Column { .. } | PowerColumn { .. }) => format!("Choose column {}", index + 1),
            Some(Help(side)) if index >= 16 => {
                let skill = self.entity_help_skill(side, action)?;
                format!("Help your adventurer with {skill:?}")
            }
            Some(Strike(_) | Help(_) | PowerTarget { .. } | SteedOpportunityTarget { .. }) => {
                if index < 2 {
                    format!("Target {}", NAMES[self.f[index].build.class as usize])
                } else {
                    format!("Target {}", self.entity(index)?.kind.name())
                }
            }
            Some(Leaving { .. }) => match action {
                Act::Opportunity => "Opportunity Attack against companion",
                Act::Special3 => "Opportunity Attack with Psychic Blade",
                Act::Pass => "Pass on Opportunity Attack",
                _ => return None,
            }
            .to_owned(),
            Some(SteedOpportunity { .. }) => match action {
                Act::Opportunity => "Steed Opportunity Attack",
                Act::Pass => "Pass on Steed Opportunity Attack",
                _ => return None,
            }
            .to_owned(),
            Some(ResumePlayerMove { .. } | ResumeMove { .. }) => return None,
            Some(Dismount) => format!("Dismount {}", entity_direction(action)?),
            Some(Jump { high, .. }) => {
                if action == Act::Pass {
                    return Some("Cancel companion jump".into());
                }
                let index = if index == 48 { 15 } else { index };
                let distance = 5 * (index % 6 + 1);
                if high {
                    format!("Jump up {distance} feet")
                } else {
                    format!(
                        "Jump {} {distance} feet",
                        entity_direction(ACTS[index / 6])?
                    )
                }
            }
            Some(Turn(_)) if index < 8 => format!("Move companion {}", entity_direction(action)?),
            Some(Turn(_)) => match action {
                Act::End => "End companion turn",
                super::bard_ranger::CLIMB_ROPE => "Climb into Rope Trick",
                super::bard_ranger::LEAVE_ROPE => "Climb out of Rope Trick",
                Act::Melee => "Otherworldly Slam",
                Act::Dash => "Companion Dash",
                Act::Disengage => "Companion Disengage",
                Act::Dodge => "Companion Dodge",
                Act::Stand => "Companion Stand Up",
                Act::JumpLong => "Companion jump with Jump spell",
                Act::JumpHigh => "Companion jump up with Jump spell",
                Act::Magic10 => "Companion Exhale (Dragon’s Breath)",
                Act::Entity11 => "Companion breaks a spell restraint",
                Act::EscapeAthletics => "Companion Escape Grapple (Athletics)",
                Act::EscapeAcrobatics => "Companion Escape Grapple (Acrobatics)",
                Act::Help => "Companion Help",
                Act::Entity8 => self.steed_power_label(viewer)?,
                Act::Entity9 => "Companion ascend",
                Act::Entity10 => "Companion descend",
                _ => return None,
            }
            .to_owned(),
            None if action == Act::Entity8 && self.entities.mounted[viewer] => {
                self.steed_power_label(viewer)?.to_owned()
            }
            None => entity_action_name(self.f[viewer].build.class, action)?.to_owned(),
        })
    }

    fn companion_turn_mask(&self, side: usize) -> u128 {
        let Some(companion) = self.entity(2 + side) else {
            return Act::End.bit();
        };
        let id = 2 + side;
        let inactive = companion.martial.stunned(self.turns) || self.magic_area_incapacitated(id);
        let mut mask = Act::End.bit();
        if !inactive {
            mask |= self.companion_rope_actions(id);
        }
        if self.rope_space(id).is_some() {
            if companion.turn.action && !inactive {
                mask |= Act::Dash.bit() | Act::Dodge.bit();
            }
            return mask;
        }
        if companion.martial.prone
            && !self.magic_laughter(id)
            && !self.sleep_unconscious(id)
            && self.entity_movement_left(2 + side) >= self.entity_effective_speed(2 + side) / 2
        {
            mask |= Act::Stand.bit();
        }
        for (direction, action) in ACTS[..8].iter().enumerate() {
            if self.entity_destination(side, direction).is_some() {
                mask |= action.bit();
            }
        }
        if self.entity_vertical_destination(side, 1).is_some() {
            mask |= Act::Entity9.bit();
        }
        if self.entity_vertical_destination(side, -1).is_some() {
            mask |= Act::Entity10.bit();
        }
        if companion.turn.action && !inactive {
            if self.companion_can_exhale(2 + side) {
                mask |= Act::Magic10.bit();
            }
            if self.magic_area_restrained(2 + side)
                || self.ensnaring_escape_target(2 + side).is_some()
            {
                mask |= Act::Entity11.bit();
            }
            if companion.martial.grappled.is_some() {
                mask |= Act::EscapeAthletics.bit() | Act::EscapeAcrobatics.bit();
            }
            mask |= Act::Dash.bit() | Act::Disengage.bit() | Act::Dodge.bit();
            let has_enemy = self.entity_near_enemies(side) != 0;
            if self.entity_attack_targets(side) != 0 && matches!(companion.kind, EntityKind::Steed { .. }) {
                mask |= Act::Melee.bit();
            }
            if has_enemy || self.entity_help_skills(side) != 0 {
                mask |= Act::Help.bit();
            }
        }
        if !inactive && self.steed_power_mask(side) != 0 {
            mask |= Act::Entity8.bit();
        }
        if self.entity_jump_mask(side, false) != 0 {
            mask |= Act::JumpLong.bit();
        }
        if self.entity_jump_mask(side, true) != 0 {
            mask |= Act::JumpHigh.bit();
        }
        self.spell_filter_attack_actions(2 + side, mask)
    }

    fn companion_turn_step(&mut self, side: usize, action: Act) {
        if matches!(action, super::bard_ranger::CLIMB_ROPE | super::bard_ranger::LEAVE_ROPE)
            && self.companion_rope_step(2 + side, action)
        {
            self.set_entity_phase(Some(EntityPhase::Turn(side)));
            return;
        }
        if action == Act::End {
            self.spell_movement_turn_end(2 + side);
            self.magic_area_end(2 + side);
            self.divine_turn_end(2 + side);
            self.martial_poison_end(2 + side);
            if let Some(companion) = self.entity_mut(2 + side) {
                companion.end_turn_in_dry_arena();
            }
            self.next_companion_or_advance();
            return;
        }
        if (action as usize) < 8 || matches!(action, Act::Entity9 | Act::Entity10) {
            let (destination, cost) = if (action as usize) < 8 {
                let (cell, cost) = self.entity_destination(side, action as usize).unwrap();
                (
                    Point {
                        cell,
                        altitude: self.entity(2 + side).unwrap().altitude,
                    },
                    cost,
                )
            } else {
                let direction = if action == Act::Entity9 { 1 } else { -1 };
                (
                    self.entity_vertical_destination(side, direction).unwrap(),
                    1,
                )
            };
            if self.leaving_provokes(side, destination) {
                self.set_entity_phase(Some(EntityPhase::Leaving {
                    side,
                    destination,
                    cost,
                }));
                return;
            }
            self.commit_entity_move(side, destination, cost);
        } else if matches!(action, Act::JumpLong | Act::JumpHigh) {
            self.set_entity_phase(Some(EntityPhase::Jump {
                side,
                high: action == Act::JumpHigh,
            }));
            return;
        } else if action == Act::Magic10 {
            self.entity_mut(2 + side).unwrap().turn.action = false;
            self.begin_companion_exhale(2 + side);
            return;
        } else if action == Act::Entity11 {
            self.entity_mut(2 + side).unwrap().turn.action = false;
            let helper = 2 + side;
            let target = self.ensnaring_escape_target(helper).unwrap_or(helper);
            self.area_escape_restraint_as(helper, target);
        } else if matches!(action, Act::EscapeAthletics | Act::EscapeAcrobatics) {
            let id = 2 + side;
            let companion = self.entity_mut(id).unwrap();
            companion.turn.action = false;
            let dc = companion.martial.grappled.unwrap().1;
            let modifier = companion.escape_modifier(action);
            let impaired = self.martial_poisoned(id)
                || self.entity_frightened(id)
                || self.magic_area_poisoned(id);
            let skill = if action == Act::EscapeAthletics {
                Skill::Athletics
            } else {
                Skill::Acrobatics
            };
            let impaired = impaired || self.magic_hex_disadvantage(id, skill.ability());
            let helped = self.consume_companion_check_help(side, skill);
            let guidance = self.magic_guidance(id, skill);
            if self.d20_for(id, helped, impaired) + modifier + guidance >= dc {
                self.entity_release_grapple(id);
            }
        } else {
            let speed = self.entity_effective_speed(2 + side);
            let companion = self.entity_mut(2 + side).unwrap();
            match action {
                Act::Stand => {
                    companion.martial.prone = false;
                    companion.turn.moved += speed / 2;
                }
                Act::Dash => {
                    companion.turn.action = false;
                    companion.turn.dash += speed;
                }
                Act::Disengage => {
                    companion.turn.action = false;
                    companion.turn.disengaged = true;
                }
                Act::Dodge => {
                    companion.turn.action = false;
                    companion.dodging = true;
                }
                Act::Melee => {
                    self.set_entity_phase(Some(EntityPhase::Strike(side)));
                    return;
                }
                Act::Help => {
                    self.set_entity_phase(Some(EntityPhase::Help(side)));
                    return;
                }
                Act::Entity8 => {
                    self.open_steed_power(side, true);
                    return;
                }
                _ => unreachable!(),
            }
        }
        self.set_entity_phase(Some(EntityPhase::Turn(side)));
        if matches!(
            action,
            Act::Dash
                | Act::Disengage
                | Act::Dodge
                | Act::Entity11
                | Act::EscapeAthletics
                | Act::EscapeAcrobatics
        ) {
            self.notify_companion_event(2 + side, ReadyTrigger::Action);
        }
    }

    fn entity_help_skill(&self, side: usize, choice: Act) -> Option<Skill> {
        let EntityKind::Familiar { form, .. } = self.entity(2 + side)?.kind else {
            return None;
        };
        form.skill_proficiencies()
            .iter()
            .copied()
            .find(|&skill| choice as usize == 16 + skill as usize)
    }

    fn entity_help_skills(&self, side: usize) -> u128 {
        let Some(EntityKind::Familiar { form, .. }) =
            self.entity(2 + side).map(|entity| entity.kind)
        else {
            return 0;
        };
        if !self
            .combatant_distance(side, 2 + side)
            .is_some_and(|dist| dist <= 1)
            || !self.combatant_los(side, 2 + side)
            || self.incap(side)
        {
            return 0;
        }
        form.skill_proficiencies()
            .iter()
            .fold(0, |mask, skill| mask | ACTS[16 + *skill as usize].bit())
    }

    fn entity_near_enemies(&self, side: usize) -> u128 {
        let id = 2 + side;
        let opposing = 1 - side;
        [opposing, 2 + opposing]
            .into_iter()
            .fold(0, |mask, target| {
                if self
                    .combatant_distance(id, target)
                    .is_some_and(|distance| distance <= 1)
                    && self.entity_visible(id, target)
                {
                    mask | ACTS[target].bit()
                } else {
                    mask
                }
            })
    }

    fn entity_attack_targets(&self, side: usize) -> u128 {
        let opposing = 1 - side;
        let targets = self.entity_near_enemies(side);
        if self.suggestion_charmed_by(2 + side, opposing) {
            targets & !ACTS[opposing].bit()
        } else {
            targets
        }
    }

    pub(super) fn entity_end_turn(&mut self, side: usize) -> bool {
        if self.entities.mounted_start[side] == self.turns && self.entity(2 + side).is_some() {
            self.spell_movement_turn_end(2 + side);
            self.magic_area_end(2 + side);
            self.divine_turn_end(2 + side);
        }
        if (0..2).all(|i| self.entities.mounted[i] || self.entity(2 + i).is_none()) {
            return false;
        }
        let mut order = vec![
            (side, self.entities.initiatives[side]),
            (1 - side, self.entities.initiatives[1 - side]),
        ];
        for i in 0..2 {
            if let Some(companion) = self.entity(2 + i).filter(|_| {
                !self.entities.mounted[i] && self.entities.mounted_start[i] != self.turns
            }) {
                order.push((2 + i, companion.initiative));
            }
        }
        order.sort_by_key(|&(id, initiative)| {
            // PCs precede ties. A steed on its rider's Initiative follows
            // the rider before unrelated creatures tied at that count.
            let tie = if id < 2 {
                2 * id
            } else if matches!(self.entity(id).unwrap().kind, EntityKind::Steed { .. }) {
                2 * (id - 2) + 1
            } else {
                id + 2
            };
            (-initiative, tie)
        });
        let current = order.iter().position(|&(id, _)| id == side).unwrap();
        self.entities.next = order
            .iter()
            .cycle()
            .skip(current + 1)
            .take_while(|&&(id, _)| id >= 2)
            .map(|&(id, _)| id - 2)
            .collect();
        self.entities.next.reverse();
        self.start_next_companion()
    }

    fn start_next_companion(&mut self) -> bool {
        while let Some(side) = self.entities.next.pop() {
            if let Some(companion) = self.entity_mut(2 + side) {
                companion.begin_turn();
                self.help_skill[side] = None;
                self.magic_reset_opportunity(2 + side);
                self.apply_companion_heroism(2 + side);
                self.entities.help_target[side] = None;
                self.set_entity_phase(Some(EntityPhase::Turn(side)));
                self.magic_area_start(2 + side);
                if self.entity(2 + side).is_some() {
                    self.start_entity_compulsion(2 + side);
                    return true;
                }
            }
        }
        false
    }

    fn next_companion_or_advance(&mut self) {
        if !self.start_next_companion() {
            self.set_entity_phase(None);
            self.advance_player_turn();
        }
    }

    pub(super) fn entity_start_turn(&mut self, i: usize) {
        self.entities.shared_senses[i] = false;
        if self.entities.mounted[i] {
            self.start_controlled_mount(i);
        }
    }

    fn start_controlled_mount(&mut self, side: usize) {
        if self.entities.mounted_start[side] != self.turns {
            self.entities.mounted_start[side] = self.turns;
            if let Some(steed) = self.entity_mut(2 + side) {
                steed.begin_turn();
                self.help_skill[side] = None;
                self.magic_reset_opportunity(2 + side);
                self.apply_companion_heroism(2 + side);
                self.magic_area_start(2 + side);
                if self.entity(2 + side).is_some() {
                    self.start_entity_compulsion(2 + side);
                }
            }
        }
    }

    pub(super) fn entity_destination(&self, side: usize, dir: usize) -> Option<(Pos, i32)> {
        let id = 2 + side;
        let companion = self.entity(id)?;
        let (destination, base_cost) = self.map.step_at(companion.point(), dir)?;
        let base_cost = self.overgrowth_cost(destination, base_cost) + i32::from(companion.martial.prone);
        let (cost, _) = self.footprint_movement(id, destination, base_cost)?;
        let stats = companion.kind.stats();
        let can_stay = self.map.supported(destination)
            || stats.fly_speed > 0
            || self.magic_fly_speed(id) > 0
            || stats.climb_speed > 0 && self.map.touches_structure(destination);
        (can_stay
            && self.entity_movement_left(id) >= cost
            && self.entity_fear_allows_move(id, destination.cell))
        .then_some((destination.cell, cost))
    }

    fn entity_jump_mask(&self, side: usize, high: bool) -> u128 {
        (0..if high { 6 } else { 49 })
            .filter(|&choice| self.entity_spell_jump(side, high, choice, self.entity_movement_left(2 + side)).is_some())
            .fold(0, |mask, choice| mask | ACTS[choice].bit())
    }

    pub(super) fn entity_spell_jump(&self, side: usize, high: bool, choice: usize, budget: i32) -> Option<Point> {
        let companion = self.entity(2 + side)?;
        if budget < 2 || choice >= if high { 6 } else {49}
            || companion.jump_until <= self.turns || companion.turn.used_spell_jump
            || companion.martial.prone || !self.map.supported(companion.point()) {
            return None;
        }
        self.entity_jump_destination(side, high, choice)
    }

    fn entity_jump_destination(&self, side: usize, high: bool, choice: usize) -> Option<Point> {
        // Choice 15 is reserved for cancel, so use choice 48 for the
        // corresponding fourth square to the east.
        let choice = if choice == 48 {
            15
        } else if choice == Act::Pass as usize {
            return None;
        } else {
            choice
        };
        let id = 2 + side;
        let companion = self.entity(id)?;
        let mut point = companion.point();
        let distance = choice % 6 + 1;
        if high {
            point.altitude += distance as i8;
        } else {
            for _ in 0..distance {
                point = self.map.step_at(point, choice / 6)?.0;
            }
        }
        (self.entity_fear_allows_move(id, point.cell)
            && self
                .footprint_fit_kind(id, point, companion.kind.size_category(), true, true)
                .is_some())
        .then_some(point)
    }

    fn entity_point_free(&self, point: Point) -> bool {
        let target = Footprint::point(point);
        self.map.can_occupy(point)
            && (0..4).all(|id| {
                self.footprint(id)
                    .is_none_or(|footprint| !footprint.overlaps(target))
            })
    }

    pub(super) fn entity_vertical_destination(&self, side: usize, direction: i8) -> Option<Point> {
        self.entity_vertical_destination_with_budget(side, direction, self.entity_movement_left(2 + side))
    }

    pub(super) fn entity_vertical_destination_with_budget(
        &self, side: usize, direction: i8, budget: i32,
    ) -> Option<Point> {
        let companion = self.entity(2 + side)?;
        let target = Point {
            cell: companion.position,
            altitude: companion.altitude + direction,
        };
        let stats = companion.kind.stats();
        let can_climb = stats.climb_speed > 0
            && (self.map.touches_structure(companion.point())
                || self.map.touches_structure(target));
        ((stats.fly_speed > 0 || self.magic_fly_speed(2 + side) > 0 || can_climb)
            && !companion.martial.prone
            && budget > 0
            && self.entity_effective_speed(2 + side) > 0
            && self
                .footprint_fit_kind(2 + side, target, companion.kind.size_category(), true, true)
                .is_some())
        .then_some(target)
    }

    pub(super) fn settle_companion_flight(&mut self) {
        for id in 2..4 {
            if self.entity(id).is_some_and(|e| {
                let stats = e.kind.stats();
                !self.map.supported(e.point())
                    && stats.fly_speed == 0
                    && (stats.climb_speed == 0 || !self.map.touches_structure(e.point()))
            }) && self.magic_fly_speed(id) == 0
            {
                self.martial_entity_fall(id);
            }
        }
    }

    pub(super) fn entity_effective_speed(&self, id: usize) -> i32 {
        let Some(companion) = self.entity(id) else {
            return 0;
        };
        if self.magic_area_speed_zero(id) || companion.martial.caltrops_stopped || companion.martial.grappled.is_some() {
            return 0;
        }
        let longstrider = if companion.longstrider_until > self.turns {
            2
        } else {
            0
        };
        let slow = if companion.martial.slowed(self.turns) {
            2
        } else {
            0
        };
        let speed = companion.speed().max(self.magic_fly_speed(id)) + longstrider
            - companion.exhaustion
            - self.frost_slow(id)
            - slow;
        if companion.martial.stun_slowed(self.turns) {
            self.magic_area_speed(id, speed.max(0) / 2)
        } else {
            self.magic_area_speed(id, speed.max(0))
        }
    }

    pub fn entity_movement_left(&self, id: usize) -> i32 {
        let Some(companion) = self.entity(id) else {
            return 0;
        };
        let speed = self.entity_effective_speed(id);
        if speed == 0 || companion.turn.ended {
            return 0;
        }
        (speed + companion.turn.dash - companion.turn.moved).max(0)
    }

    pub(super) fn entity_mount_movement_left(&self, side: usize) -> Option<i32> {
        if !self.entities.mounted[side] {
            return None;
        }
        self.entity(2 + side)
            .map(|_| self.entity_movement_left(2 + side))
    }

    pub(super) fn entity_complete_compulsion(&mut self, id: usize) {
        let side = id - 2;
        if self.entities.mounted[side] {
            if let Some(entity) = self.entity_mut(id) {
                entity.turn.ended = true;
                entity.turn.action = false;
                entity.turn.bonus = false;
            }
            self.set_entity_phase(None);
        } else {
            self.spell_movement_turn_end(id);
            self.magic_area_end(id);
            self.divine_turn_end(id);
            self.martial_poison_end(id);
            self.next_companion_or_advance();
        }
    }

    pub(super) fn entity_resume_control(&mut self, id: usize) {
        let side = id - 2;
        if self.entities.mounted[side] {
            self.set_entity_phase(None);
        } else if self.entity(id).is_some() {
            self.set_entity_phase(Some(EntityPhase::Turn(side)));
        } else {
            self.next_companion_or_advance();
        }
    }

    pub(super) fn entity_compelled_dodge(&mut self, id: usize) -> bool {
        if let Some(entity) = self.entity_mut(id).filter(|e| e.turn.action) {
            entity.turn.action = false;
            entity.dodging = true;
            true
        } else {
            false
        }
    }

    pub(super) fn entity_compelled_stand(&mut self, id: usize) -> bool {
        let movement = self.entity_effective_speed(id) / 2;
        if self.entity_movement_left(id) < movement {
            return false;
        }
        if let Some(entity) = self.entity_mut(id).filter(|e| e.martial.prone) {
            entity.martial.prone = false;
            entity.turn.moved += movement;
            true
        } else {
            false
        }
    }

    pub(super) fn entity_reaction_budget(&self, id: usize) -> Option<i32> {
        self.entity(id)
            .filter(|entity| {
                entity.reaction
                    && !entity.martial.stunned(self.turns)
                    && !self.magic_area_incapacitated(id)
            })
            .map(|_| self.entity_effective_speed(id))
    }

    pub(super) fn entity_spend_reaction(&mut self, id: usize) -> bool {
        if self.entity_reaction_budget(id).is_none() {
            return false;
        }
        self.entity_mut(id).unwrap().reaction = false;
        true
    }

    pub(super) fn entity_reaction_destination(
        &self,
        id: usize,
        direction: usize,
        budget: i32,
    ) -> Option<(Point, i32)> {
        if self.entity_effective_speed(id) <= 0 {
            return None;
        }
        let entity = self.entity(id)?;
        let (destination, cost) = self.map.step_at(entity.point(), direction)?;
        let base = self.overgrowth_cost(destination, cost) + i32::from(entity.martial.prone);
        let (cost, _) = self.footprint_movement(id, destination, base)?;
        let stats = entity.kind.stats();
        let traversable = self.map.supported(destination)
            || stats.fly_speed > 0
            || self.magic_fly_speed(id) > 0
            || stats.climb_speed > 0 && self.map.touches_structure(destination);
        (cost <= budget && traversable && self.entity_fear_allows_move(id, destination.cell))
            .then_some((destination, cost))
    }

    pub(super) fn entity_commit_reaction_move(&mut self, id: usize, destination: Point) -> bool {
        self.entity_force_move(id, destination)
    }

    pub(super) fn entity_reaction_step_provokes(&self, id: usize, destination: Point) -> bool {
        let Some(companion) = self.entity(id) else {
            return false;
        };
        let reactor = 1 - companion.side;
        !companion.kind.stats().flyby
            && self.can_react(reactor)
            && !self.f[reactor].addled
            && self.leaves_melee_reach(id, destination, reactor)
            && self.entity_visible(reactor, id)
    }

    /// Applies induced movement without asking for an opportunity attack.
    pub(super) fn entity_force_move(&mut self, id: usize, destination: Point) -> bool {
        let Some(side) = self.entity(id).map(|entity| entity.side) else {
            return false;
        };
        if !self.commit_footprint(id, destination, false) {
            return false;
        }
        if self.entities.mounted[side] {
            if self.save(side, 1, 10) {
                self.f[side].pos = destination.cell;
                self.f[side].elevation = destination.altitude;
            } else {
                self.entities.mounted[side] = false;
                self.f[side].prone = true;
                if self.f[side].pos.dist(destination.cell) > 1 {
                    if let Some(position) = (0..8)
                        .filter_map(|dir| self.map.step(destination.cell, dir))
                        .map(|(position, _)| position)
                        .find(|&position| self.space_free(position))
                    {
                        self.f[side].pos = position;
                    }
                }
            }
        }
        let previous = self.creature_point(id).unwrap();
        let entity = self.entity_mut(id).unwrap();
        entity.position = destination.cell;
        entity.altitude = destination.altitude;
        self.martial_moved(id);
        self.magic_area_moved(id, previous);
        true
    }

    pub(super) fn move_companion(&mut self, side: usize, destination: Pos, cost: i32) {
        let target = Point {
            cell: destination,
            altitude: self.entity(2 + side).map_or(0, |steed| steed.altitude),
        };
        self.move_companion_point(side, target, cost);
    }

    pub(super) fn move_companion_point(&mut self, side: usize, target: Point, cost: i32) {
        let mounted = self.entities.mounted[side];
        if !self.commit_footprint(2 + side, target, false) {
            if self.fighter_point(1 - side) == target {
                self.reveal(1 - side);
            } else if let Some(steed) = self.entity_mut(2 + side) {
                steed.turn.moved += cost;
            }
            return;
        }
        let Some(previous) = self.creature_point(2 + side) else {
            return;
        };
        if let Some(companion) = self.entity_mut(2 + side) {
            companion.position = target.cell;
            companion.altitude = target.altitude;
            companion.turn.moved += cost;
        }
        self.martial_moved(2 + side);
        if mounted && self.entities.mounted[side] {
            self.f[side].pos = target.cell;
            self.f[side].elevation = target.altitude;
        }
        self.magic_area_moved(2 + side, previous);
        self.notify_companion_event(2 + side, ReadyTrigger::Move);
    }
}

fn entity_direction(action: Act) -> Option<&'static str> {
    [
        "north",
        "northeast",
        "east",
        "southeast",
        "south",
        "southwest",
        "west",
        "northwest",
    ]
    .get(action as usize)
    .copied()
}

impl Battle {
    pub(super) fn entity_slam_damage_type(&self, side: usize) -> DamageType {
        match self.steed_spirit(side).expect("steed attacks with Slam") {
            Spirit::Celestial => DamageType::Radiant,
            Spirit::Fey => DamageType::Psychic,
            Spirit::Fiend => DamageType::Necrotic,
        }
    }

    fn steed_spirit(&self, side: usize) -> Option<Spirit> {
        match self.entity(2 + side)?.kind {
            EntityKind::Steed { spirit, .. } => Some(spirit),
            _ => None,
        }
    }

    fn steed_power_label(&self, side: usize) -> Option<&'static str> {
        Some(match self.steed_spirit(side)? {
            Spirit::Celestial => "Healing Touch",
            Spirit::Fey => "Fey Step",
            Spirit::Fiend => "Fell Glare",
        })
    }

    fn entity_target_pos(&self, id: usize) -> Option<Pos> {
        self.f
            .get(id)
            .map(|fighter| fighter.pos)
            .or_else(|| self.entity_position(id))
    }

    fn steed_power_mask(&self, side: usize) -> u128 {
        let Some(steed) = self
            .entity(2 + side)
            .filter(|steed| steed.spirit_power && steed.turn.bonus)
        else {
            return 0;
        };
        match self.steed_spirit(side) {
            Some(Spirit::Celestial) => (0..4).fold(0, |mask, id| {
                let injured = if id < 2 {
                    self.f[id].hp < self.f[id].build.hp && self.f[id].hp > 0
                } else {
                    self.entity(id)
                        .is_some_and(|entity| entity.hp < entity.kind.stats().hp)
                };
                if injured
                    && self
                        .entity_target_pos(id)
                        .is_some_and(|pos| steed.position.dist(pos) <= 1)
                {
                    mask | ACTS[id].bit()
                } else {
                    mask
                }
            }),
            Some(Spirit::Fiend) => [1 - side, 3 - side].into_iter().fold(0, |mask, id| {
                if self
                    .entity_target_pos(id)
                    .is_some_and(|pos| steed.position.dist(pos) <= 12)
                    && self.entity_visible(2 + side, id)
                {
                    mask | ACTS[id].bit()
                } else {
                    mask
                }
            }),
            Some(Spirit::Fey)
                if (0..HEIGHT).any(|row| self.teleport_column_mask(side, row) != 0) =>
            {
                1
            }
            _ => 0,
        }
    }

    fn teleport_column_mask(&self, side: usize, row: i8) -> u128 {
        let Some(origin) = self.entity_position(2 + side) else {
            return 0;
        };
        (0..WIDTH).fold(0, |mask, column| {
            let pos = Pos { x: column, y: row };
            if self
                .footprint_fit_kind(2 + side, Point::ground(pos), 3, true, true)
                .is_some()
                && origin.dist(pos) <= 12
            {
                mask | ACTS[column as usize].bit()
            } else {
                mask
            }
        })
    }

    fn open_steed_power(&mut self, side: usize, resume: bool) {
        let phase = if self.steed_spirit(side) == Some(Spirit::Fey) {
            EntityPhase::PowerRow { side, resume }
        } else {
            EntityPhase::PowerTarget { side, resume }
        };
        self.set_entity_phase(Some(phase));
    }

    fn power_cost(&mut self, side: usize, spirit: Spirit) {
        debug_assert_eq!(self.steed_spirit(side), Some(spirit));
        let steed = self.entity_mut(side + 2).unwrap();
        steed.spirit_power = false;
        steed.turn.bonus = false;
    }

    fn finish_power(&mut self, side: usize, resume: bool) {
        self.set_entity_phase(resume.then_some(EntityPhase::Turn(side)));
    }

    fn apply_steed_power(&mut self, side: usize, target: usize) {
        let spirit = self.steed_spirit(side).unwrap();
        self.power_cost(side, spirit);
        match spirit {
            Spirit::Celestial => {
                let amount = self.roll(8) + self.roll(8) + SUMMON_STEED_RANK as i32;
                if target < 2 {
                    self.heal(target, amount);
                } else {
                    self.entity_heal(target, amount);
                }
            }
            Spirit::Fiend => {
                if self.magic_ally_protection(side + 2, target) {
                    return;
                }
                let build = self.f[side].build;
                let dc = 8 + build.pb + build.casting;
                let saved = if target < 2 {
                    self.save(target, 4, dc)
                } else {
                    self.entity_save(target, 4, dc)
                };
                if !saved && !self.magic_ally_protection(2 + side, target) {
                    self.entities.frightened[target] = Some((side, self.turns + 2));
                }
            }
            Spirit::Fey => unreachable!(),
        }
    }

    pub(super) fn entity_frightened(&self, id: usize) -> bool {
        self.fear_source(id)
            .or_else(|| self.bard_ranger_fear_source(id))
            .is_some_and(|source| self.entity_visible(id, source))
            || !self.magic_heroism_immune(id)
                && self
                    .entities
                    .frightened
                    .get(id)
                    .copied()
                    .flatten()
                    .is_some_and(|(source, expires)| {
                        self.turns <= expires && self.entity_visible(id, 2 + source)
                    })
    }

    pub(super) fn entity_fear_allows_move(&self, id: usize, destination: Pos) -> bool {
        if let Some(source) = self.bard_ranger_fear_source(id).or_else(|| {
            self.fear_source(id).filter(|&source| self.entity_visible(id, source))
        }) {
            let origin = self.fighter_point(source).cell;
            if self
                .creature_point(id)
                .is_some_and(|at| destination.dist(origin) < at.cell.dist(origin))
            {
                return false;
            }
        }
        self.magic_heroism_immune(id)
            || self
                .entities
                .frightened
                .get(id)
                .copied()
                .flatten()
                .is_none_or(|(source, expires)| {
                    if self.turns > expires {
                        return true;
                    }
                    self.entity_target_pos(id)
                        .zip(self.entity_position(2 + source))
                        .is_none_or(|(from, source)| destination.dist(source) >= from.dist(source))
                })
    }
}

impl Battle {
    pub(super) fn entity_policy(&self) -> Option<Act> {
        use EntityPhase::*;
        let phase = self.entity_phase()?;
        let side = self.entity_phase_actor(phase);
        let opponent = 1 - side;
        let enemy_position = self
            .observed_pos(side, opponent)
            .unwrap_or_else(|| super::geometry::Map::spawn(opponent));
        let mask = self.entity_phase_mask(phase);
        let first = || ACTS[mask.trailing_zeros() as usize];
        let zone_score = |position: Pos, is_familiar: bool| {
            let toward = self.map.path(position, enemy_position);
            let cost = self.map.path(self.f[side].pos, position);
            if is_familiar
                && self
                    .entity_target_pos(opponent)
                    .is_some_and(|p| p.dist(position) == 1)
            {
                return -16;
            }
            8 - toward - cost
        };
        let placement = |row: i8, kind: EntityKind, action: Act| {
            let columns = self.entity_column_mask(side, self.placement_range(side, action), row);
            let familiar = matches!(kind, EntityKind::Familiar { .. });
            (0..WIDTH)
                .filter(|&column| columns & ACTS[column as usize].bit() != 0)
                .map(move |column| (zone_score(Pos { x: column, y: row }, familiar), column))
                .max_by_key(|&(score, col)| (score, if side == 0 { col } else { WIDTH - col }))
        };
        Some(match phase {
            Form(_) => {
                ACTS[if self.f[side].build.class == Class::Paladin {
                    0
                } else {
                    1
                }]
            }
            Row { kind, action } => {
                let row = (0..HEIGHT)
                    .filter_map(|row| placement(row, kind, action).map(|(score, _)| (score, row)))
                    .max()
                    .unwrap()
                    .1;
                ACTS[row as usize]
            }
            Column { kind, action, row } => ACTS[placement(row, kind, action).unwrap().1 as usize],
            Turn(side) => {
                let Some(companion) = self.entity(2 + side) else {
                    return Some(Act::End);
                };
                if mask & Act::EscapeAthletics.bit() != 0 {
                    return Some(
                        if companion.escape_modifier(Act::EscapeAthletics)
                            >= companion.escape_modifier(Act::EscapeAcrobatics)
                        {
                            Act::EscapeAthletics
                        } else {
                            Act::EscapeAcrobatics
                        },
                    );
                }
                for action in [Act::Melee, Act::Help, Act::Entity8] {
                    if action == Act::Help
                        && self.entity_near_enemies(side) == 0
                        && self.f[side].grappled.is_none()
                    {
                        continue;
                    }
                    if mask & action.bit() != 0 {
                        return Some(action);
                    }
                }
                let enemy = enemy_position;
                let path = self.map.path(companion.position, enemy);
                let direction = (0..8)
                    .filter_map(|direction| {
                        let (destination, cost) = self.entity_destination(side, direction)?;
                        let remaining = self.map.path(destination, enemy);
                        (remaining < path && remaining >= 1).then_some((
                            path - remaining,
                            -cost,
                            direction,
                        ))
                    })
                    .max()
                    .map(|(_, _, direction)| ACTS[direction]);
                let dangerous = companion.position.dist(enemy) == 1;
                if mask & Act::Stand.bit() != 0 {
                    Act::Stand
                } else if let Some(action) = direction.filter(|_| !dangerous) {
                    action
                } else if path > 1 && mask & Act::Dash.bit() != 0 {
                    Act::Dash
                } else if mask & Act::Dodge.bit() != 0 {
                    Act::Dodge
                } else {
                    Act::End
                }
            }
            Leaving { .. } | SteedOpportunity { .. } => Act::Opportunity,
            ResumePlayerMove { .. } | ResumeMove { .. } => {
                unreachable!("move continuation is internal")
            }
            Strike(_) | Help(_) | SteedOpportunityTarget { .. }
                if mask & ACTS[opponent].bit() != 0 =>
            {
                ACTS[opponent]
            }
            PowerTarget { .. }
                if self.steed_spirit(side) == Some(Spirit::Celestial)
                    && mask & ACTS[side].bit() != 0 =>
            {
                ACTS[side]
            }
            _ => first(),
        })
    }
}

#[cfg(test)]
mod battle_tests {
    use super::*;

    fn cast_summon(battle: &mut Battle, action: Act) {
        if battle.mask() & action.bit() == 0 && battle.mask() & Act::Gear2.bit() != 0 {
            battle.step(Act::Gear2).unwrap();
        }
        battle.step(action).unwrap();
    }

    fn finish_slot_selection(battle: &mut Battle) {
        if battle.phase_id() == 38 {
            first_choice(battle);
        }
    }

    fn first_choice(battle: &mut Battle) {
        let index = battle.mask().trailing_zeros() as usize;
        battle.step(ACTS[index]).unwrap();
    }

    #[test]
    fn familiar_placement_mask_does_not_reveal_hidden_occupancy() {
        let mut occupied = Battle::new(0, [Class::Druid, Class::Rogue], [2, 2]);
        occupied.owner = 0;
        occupied.f[0].pos = Pos { x: 0, y: 0 };
        occupied.f[1].pos = Pos { x: 1, y: 0 };
        occupied.f[1].hidden_dc = Some(22);
        let mut vacant = occupied.clone();
        vacant.f[1].pos = Pos { x: 0, y: 1 };
        for duel in [&mut occupied, &mut vacant] {
            cast_summon(duel, Act::Entity1);
            duel.step(Act::N).unwrap();
            duel.step(Act::N).unwrap();
        }
        assert_eq!(occupied.mask(), vacant.mask());
        assert_ne!(occupied.mask() & ACTS[1].bit(), 0);
        occupied.step(ACTS[1]).unwrap();
        vacant.step(ACTS[1]).unwrap();
        assert!(occupied.entity(2).is_none());
        assert!(vacant.entity(2).is_some());
    }

    #[test]
    fn wild_companion_pays_explicit_slot_only_on_final_placement() {
        let mut battle = Battle::new(123, [Class::Druid, Class::Fighter], [5, 5]);
        battle.owner = 0;
        let slots = battle.f[0].slots;
        cast_summon(&mut battle, Act::Entity2);
        assert_eq!(battle.phase_id(), 60);
        battle.step(Act::NE).unwrap(); // choose level 2
        assert_eq!(battle.f[0].slots, slots);
        battle.step(Act::NE).unwrap(); // owl
        first_choice(&mut battle); // row
        assert_eq!(battle.f[0].slots, slots);
        assert!(battle.entity(2).is_none());
        first_choice(&mut battle); // column
        assert_eq!(battle.f[0].slots, [slots[0], slots[1] - 1, slots[2]]);
        let companion = battle.entity(2).unwrap();
        assert!(matches!(
            companion.kind,
            EntityKind::Familiar {
                form: FamiliarForm::Owl,
                ..
            }
        ));
        assert_eq!(companion.side, 0);
        assert!(!battle.action);
    }

    #[test]
    fn free_steed_cast_and_separate_companion_turn() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        let slots = battle.f[0].slots;
        cast_summon(&mut battle, Act::Entity1);
        battle.step(Act::N).unwrap(); // celestial
        first_choice(&mut battle);
        first_choice(&mut battle);
        assert_eq!(battle.f[0].slots, slots);
        assert_eq!(battle.entity(2).unwrap().hp, 25);
        assert!(!battle.entities.steed_free[0]);
        battle.step(Act::End).unwrap();
        assert_eq!(battle.entity_phase(), Some(EntityPhase::Turn(0)));
        assert_ne!(battle.mask() & Act::Dash.bit(), 0);
        battle.step(Act::Dash).unwrap();
        assert_eq!(battle.entity(2).unwrap().movement_left(), 24);
        assert_eq!(battle.mask() & Act::Dodge.bit(), 0);
        battle.step(Act::End).unwrap();
        assert_eq!(battle.owner, 1);
        assert!(battle.action);
    }

    fn druid_with_familiar(form: FamiliarForm) -> Battle {
        let mut battle = Battle::new(1, [Class::Druid, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 7, y: 0 };
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Familiar {
                form,
                spirit: Spirit::Fey,
            },
            0,
            Pos { x: 5, y: 0 },
            12,
        ));
        battle
    }

    #[test]
    fn sharing_bat_senses_can_locate_hidden_foe_until_next_turn() {
        let mut battle = druid_with_familiar(FamiliarForm::Bat);
        battle.f[1].hidden_dc = Some(21);
        assert!(battle.observed_pos(0, 1).is_none());
        battle.step(Act::Entity6).unwrap();
        assert_eq!(battle.observed_pos(0, 1), Some(Pos { x: 7, y: 0 }));
        assert!(!battle.bonus);
        battle.entity_start_turn(0);
        assert!(battle.observed_pos(0, 1).is_none());
    }

    #[test]
    fn familiar_dismiss_recall_and_permanent_release_are_distinct() {
        let mut battle = druid_with_familiar(FamiliarForm::Cat);
        battle.step(Act::Entity3).unwrap();
        assert!(battle.entity(2).is_none());
        battle.action = true;
        battle.step(Act::Entity4).unwrap();
        first_choice(&mut battle);
        first_choice(&mut battle);
        assert!(battle.entity(2).is_some());
        battle.action = true;
        battle.step(Act::Entity5).unwrap();
        assert!(battle.entities.companion[0].is_none());
        battle.action = true;
        assert_eq!(battle.mask() & Act::Entity4.bit(), 0);
    }

    #[test]
    fn cure_wounds_on_distant_familiar_consumes_its_touch_reaction() {
        let mut battle = druid_with_familiar(FamiliarForm::Badger);
        battle.entity_damage(2, 2);
        let hp = battle.entity(2).unwrap().hp;
        if battle.mask() & Act::Special6.bit() == 0 && battle.mask() & Act::Gear2.bit() != 0 {
            battle.step(Act::Gear2).unwrap();
        }
        battle.step(Act::Special6).unwrap();
        assert_eq!(battle.phase_id(), 48);
        assert_ne!(battle.mask() & ACTS[2].bit(), 0);
        battle.step(ACTS[2]).unwrap();
        finish_slot_selection(&mut battle);
        let familiar = battle.entity(2).unwrap();
        assert!(familiar.hp > hp);
        assert!(!familiar.reaction);
    }

    #[test]
    fn paladin_can_choose_the_steed_and_self_for_aid() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            0,
            Pos { x: 1, y: 0 },
            battle.entities.initiatives[0],
        ));
        let player_hp = battle.f[0].hp;
        battle.step(Act::Magic1).unwrap(); // Aid
        assert_eq!(battle.phase_id(), 48);
        let self_and_companion = ACTS[32 + 0b0101];
        assert_ne!(battle.mask() & self_and_companion.bit(), 0);
        battle.step(self_and_companion).unwrap();
        assert_eq!(battle.f[0].hp, player_hp + 5);
        assert_eq!(battle.entity(2).unwrap().hp, 30);
        assert_eq!(battle.entity(2).unwrap().max_hp(), 30);
    }

    #[test]
    fn aid_can_increase_only_the_opposing_adventurers_hit_points() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 2, y: 0 };
        let hit_points = [battle.f[0].hp, battle.f[1].hp];
        battle.step(Act::Magic1).unwrap();
        assert_eq!(battle.phase_id(), 48);
        assert_ne!(battle.mask() & ACTS[1].bit(), 0);
        battle.step(ACTS[1]).unwrap();
        assert_eq!(battle.f[0].hp, hit_points[0]);
        assert_eq!(battle.f[1].hp, hit_points[1] + 5);
    }

    #[test]
    fn opposing_target_controls_consent_to_protection() {
        let mut setup = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        setup.owner = 0;
        setup.f[0].pos = Pos { x: 0, y: 0 };
        setup.f[1].pos = Pos { x: 1, y: 0 };
        setup.entities.companion[0] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            0,
            Pos { x: 0, y: 1 },
            setup.entities.initiatives[0],
        ));
        for (response, accepted) in [(Act::Pass, false), (Act::Defend, true)] {
            let mut battle = setup.clone();
            let slots = battle.f[0].slots;
            battle.step(Act::Gear2).unwrap(); // Free a hand for the consumed holy water.
            battle.step(Act::Magic15).unwrap(); // Protection from Evil and Good
            finish_slot_selection(&mut battle);
            assert_eq!(battle.phase_id(), 48);
            battle.step(ACTS[1]).unwrap();
            assert_eq!(battle.actor(), Some(1));
            assert_eq!(battle.f[0].slots, slots);
            assert_eq!(battle.mask(), Act::Pass.bit() | Act::Defend.bit());
            battle.step(response).unwrap();
            finish_slot_selection(&mut battle);
            assert_eq!(battle.magic_ally_protection(2, 1), accepted);
            assert_eq!(battle.actor(), Some(0));
        }
    }

    #[test]
    fn mage_armor_uses_familiar_dexterity_after_hostile_controller_accepts() {
        let mut battle = Battle::new(1, [Class::Wizard, Class::Druid], [1, 2]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 8, y: 0 };
        battle.fixture_companion(
            1,
            Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Cat,
                    spirit: Spirit::Fey,
                },
                1,
                Pos { x: 1, y: 0 },
                15,
            ),
        );
        battle.step(Act::Magic2).unwrap();
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[3]).unwrap();
        assert_eq!(battle.actor(), Some(1));
        battle.step(Act::Defend).unwrap();
        assert_eq!(battle.magic_entity_base_ac(3), Some(15));
    }

    #[test]
    fn longstrider_increases_only_the_selected_companions_speed() {
        let mut battle = Battle::new(1, [Class::Druid, Class::Fighter], [2, 2]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.fixture_companion(
            0,
            Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Cat,
                    spirit: Spirit::Fey,
                },
                0,
                Pos { x: 1, y: 0 },
                15,
            ),
        );
        battle.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        let owner_speed = battle.speed(0);
        let companion_speed = battle.entity_effective_speed(2);
        battle.step(Act::Magic1).unwrap();
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[2]).unwrap();
        assert_eq!(battle.entity_effective_speed(2), companion_speed + 2);
        assert_eq!(battle.speed(0), owner_speed);
    }

    #[test]
    fn shield_of_faith_can_target_a_companion_instead_of_self() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            0,
            Pos { x: 1, y: 0 },
            battle.entities.initiatives[0],
        ));
        battle.step(Act::Magic16).unwrap(); // Shield of Faith
        finish_slot_selection(&mut battle);
        assert_eq!(battle.phase_id(), 48);
        battle.step(ACTS[2]).unwrap();
        finish_slot_selection(&mut battle);
        assert!(!battle.magic_ally_shielded(0));
        assert!(battle.magic_ally_shielded(2));
    }

    #[test]
    fn all_25_familiar_forms_can_be_selected_by_identity() {
        for (index, expected) in FORMS.into_iter().enumerate() {
            let mut battle = Battle::new(321, [Class::Druid, Class::Fighter], [2, 2]);
            battle.owner = 0;
            cast_summon(&mut battle, Act::Entity1);
            assert_eq!(battle.mask().count_ones(), 25);
            assert_eq!(battle.choice_label(0, ACTS[index]), expected.name());
            battle.step(ACTS[index]).unwrap();
            first_choice(&mut battle);
            first_choice(&mut battle);
            assert_eq!(
                battle.entity(2).unwrap().kind,
                EntityKind::Familiar {
                    form: expected,
                    spirit: Spirit::Fey,
                }
            );
        }
    }

    #[test]
    fn returning_a_steed_preserves_its_expended_power() {
        let mut setup = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        setup.owner = 0;
        let mut old_steed = Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            0,
            Pos { x: 1, y: 0 },
            setup.entities.initiatives[0],
        );
        old_steed.spirit_power = false;
        old_steed.hp = 0;
        setup.entities.companion[0] = Some(old_steed);
        for (choice, fresh) in [(ACTS[0], false), (ACTS[1], true)] {
            let mut battle = setup.clone();
            cast_summon(&mut battle, Act::Entity1);
            battle.step(ACTS[0]).unwrap(); // Celestial template.
            assert_eq!(battle.phase_id(), 65);
            battle.step(choice).unwrap();
            first_choice(&mut battle);
            first_choice(&mut battle);
            let steed = battle.entity(2).unwrap();
            assert_eq!(steed.hp, 25);
            assert_eq!(steed.spirit_power, fresh);
        }
    }

    #[test]
    fn celestial_steed_chooses_its_healing_target_without_using_riders_action() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[0].hp -= 20;
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            0,
            Pos { x: 1, y: 0 },
            battle.entities.initiatives[0],
        ));
        battle.entity_mut(2).unwrap().begin_turn();
        battle.set_entity_phase(Some(EntityPhase::Turn(0)));
        let hp = battle.f[0].hp;
        let slots = battle.f[0].slots;
        battle.step(Act::Entity8).unwrap();
        assert_eq!(battle.phase_id(), 55);
        assert_eq!(battle.mask(), ACTS[0].bit());
        battle.step(ACTS[0]).unwrap();
        assert!(battle.f[0].hp > hp);
        assert_eq!(battle.f[0].slots, slots);
        assert!(battle.action);
        assert!(!battle.entity(2).unwrap().spirit_power);
        assert_eq!(battle.mask() & Act::Entity8.bit(), 0);
    }

    #[test]
    fn fey_steed_teleports_with_its_mounted_rider_to_chosen_square() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Fey,
                spell_level: 2,
            },
            0,
            Pos { x: 1, y: 0 },
            battle.entities.initiatives[0],
        ));
        battle.step(Act::Entity3).unwrap();
        battle.step(Act::Entity8).unwrap();
        assert_eq!(battle.phase_id(), 56);
        battle.step(ACTS[0]).unwrap();
        assert_eq!(battle.phase_id(), 57);
        battle.step(ACTS[7]).unwrap();
        assert_eq!(battle.phase_id(), 0);
        assert_eq!(battle.f[0].pos, Pos { x: 7, y: 0 });
        assert_eq!(battle.entity(2).unwrap().position, Pos { x: 7, y: 0 });
        assert!(!battle.entity(2).unwrap().spirit_power);
    }

    #[test]
    fn fiend_steed_frightens_target_and_limits_approach_on_failed_save() {
        let failed = (0..32).any(|seed| {
            let mut battle = Battle::new(seed, [Class::Paladin, Class::Fighter], [5, 5]);
            battle.owner = 0;
            battle.f[0].pos = Pos { x: 0, y: 0 };
            battle.f[1].pos = Pos { x: 7, y: 0 };
            battle.entities.companion[0] = Some(Entity::new(
                EntityKind::Steed {
                    spirit: Spirit::Fiend,
                    spell_level: 2,
                },
                0,
                Pos { x: 1, y: 0 },
                battle.entities.initiatives[0],
            ));
            battle.entity_mut(2).unwrap().begin_turn();
            battle.set_entity_phase(Some(EntityPhase::Turn(0)));
            battle.step(Act::Entity8).unwrap();
            assert_eq!(battle.mask(), ACTS[1].bit());
            battle.step(ACTS[1]).unwrap();
            assert!(!battle.entity(2).unwrap().spirit_power);
            if battle.entity_frightened(1) {
                assert!(!battle.entity_fear_allows_move(1, Pos { x: 6, y: 0 }));
                true
            } else {
                false
            }
        });
        assert!(failed);
    }

    #[test]
    fn mounted_dash_and_remount_do_not_refresh_the_steeds_action() {
        let mut battle = Battle::new(1, [Class::Paladin, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            0,
            Pos { x: 1, y: 0 },
            battle.entities.initiatives[0],
        ));
        battle.dash = 6; // Enough rider movement for mounting twice and dismounting.
        battle.step(Act::Entity3).unwrap();
        assert!(battle.entities.mounted[0]);
        battle.step(Act::Entity5).unwrap();
        assert_eq!(battle.movement_left(), 24);
        battle.step(Act::Entity4).unwrap();
        assert_eq!(battle.phase_id(), 54);
        battle.step(Act::W).unwrap();
        assert!(!battle.entities.mounted[0]);
        battle.step(Act::Entity3).unwrap();
        assert_eq!(battle.mask() & Act::Entity5.bit(), 0);
        assert!(battle.entities.mounted[0]);
    }

    #[test]
    fn grappled_companion_chooses_an_escape_check_before_moving() {
        let mut battle = druid_with_familiar(FamiliarForm::Owl);
        battle.f[1].pos = Pos { x: 6, y: 0 };
        let familiar = battle.entity_mut(2).unwrap();
        familiar.begin_turn();
        familiar.martial.grappled = Some((1, 1));
        battle.set_entity_phase(Some(EntityPhase::Turn(0)));
        assert_eq!(battle.entity_movement_left(2), 0);
        assert_eq!(battle.mask() & 0xff, 0);
        assert_ne!(battle.mask() & Act::EscapeAthletics.bit(), 0);
        assert_ne!(battle.mask() & Act::EscapeAcrobatics.bit(), 0);
        battle.step(Act::EscapeAcrobatics).unwrap();
        let familiar = battle.entity(2).unwrap();
        assert!(familiar.martial.grappled.is_none());
        assert!(!familiar.turn.action);
        assert!(battle.entity_movement_left(2) > 0);
    }

    #[test]
    fn companion_jump_spends_two_movement_once_per_turn() {
        let mut battle = druid_with_familiar(FamiliarForm::Cat);
        battle.f[0].pos = Pos { x: 0, y: 1 };
        battle.f[1].pos = Pos { x: 6, y: 0 };
        let now = battle.turns;
        let familiar = battle.entity_mut(2).unwrap();
        familiar.position = Pos { x: 0, y: 0 };
        familiar.begin_turn();
        familiar.jump_until = now + 20;
        battle.set_entity_phase(Some(EntityPhase::Turn(0)));
        let speed = battle.entity_movement_left(2);
        battle.step(Act::JumpLong).unwrap();
        let east_3 = ACTS[2 * 6 + 2];
        assert_ne!(battle.mask() & east_3.bit(), 0);
        battle.step(east_3).unwrap();
        assert_eq!(battle.entity(2).unwrap().position, Pos { x: 3, y: 0 });
        assert_eq!(battle.entity_movement_left(2), speed - 2);
        assert_eq!(
            battle.mask() & (Act::JumpLong.bit() | Act::JumpHigh.bit()),
            0
        );
    }

    #[test]
    fn familiar_help_requires_proficiency_and_expires_at_its_next_turn() {
        let mut battle = druid_with_familiar(FamiliarForm::Weasel);
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 6, y: 0 };
        let familiar = battle.entity_mut(2).unwrap();
        familiar.position = Pos { x: 1, y: 0 };
        familiar.begin_turn();
        battle.set_entity_phase(Some(EntityPhase::Turn(0)));
        battle.step(Act::Help).unwrap();
        let acrobatics = ACTS[16 + Skill::Acrobatics as usize];
        let athletics = ACTS[16 + Skill::Athletics as usize];
        assert_ne!(battle.mask() & acrobatics.bit(), 0);
        assert_eq!(battle.mask() & athletics.bit(), 0);
        battle.step(acrobatics).unwrap();
        assert_eq!(battle.help_skill[0], Some(Skill::Acrobatics));
        battle.entities.next.push(0);
        assert!(battle.start_next_companion());
        assert_eq!(battle.help_skill[0], None);
    }

    #[test]
    fn mundane_familiar_cannot_attack_and_can_climb() {
        let mut battle = Battle::new(0, [Class::Druid, Class::Fighter], [2, 2]);
        battle.owner = 0;
        battle.entities.companion[0] = Some(Entity::new(
            EntityKind::Familiar {
                form: FamiliarForm::Spider,
                spirit: Spirit::Fey,
            },
            0,
            Pos { x: 1, y: 3 },
            15,
        ));
        battle.entity_mut(2).unwrap().begin_turn();
        battle.set_entity_phase(Some(EntityPhase::Turn(0)));
        assert_eq!(battle.mask() & Act::Melee.bit(), 0);
        assert_ne!(battle.mask() & Act::Entity9.bit(), 0);
        battle.step(Act::Entity9).unwrap();
        assert_eq!(battle.entity(2).unwrap().altitude, 1);
        assert_eq!(battle.entity(2).unwrap().movement_left(), 3);
        assert_eq!(battle.entity_at(Pos { x: 1, y: 3 }), None);
    }
}

impl Battle {
    fn leaving_provokes(&self, side: usize, destination: Point) -> bool {
        let Some(companion) = self.entity(2 + side) else {
            return false;
        };
        let reactor = 1 - side;
        !companion.turn.disengaged
            && !companion.kind.stats().flyby
            && self.can_react(reactor)
            && !self.f[reactor].addled
            && self.leaves_melee_reach(2 + side, destination, reactor)
            && self.entity_visible(reactor, 2 + side)
    }

    fn commit_entity_move(&mut self, side: usize, destination: Point, cost: i32) {
        if !self.commit_footprint(2 + side, destination, false) {
            if self.fighter_point(1 - side) == destination {
                self.reveal(1 - side);
            } else if let Some(companion) = self.entity_mut(2 + side) {
                companion.turn.moved += cost;
            }
            return;
        }
        let Some(previous) = self.creature_point(2 + side) else {
            return;
        };
        if let Some(companion) = self.entity_mut(2 + side) {
            let jumped = std::mem::take(&mut companion.turn.pending_spell_jump);
            companion.position = destination.cell;
            companion.altitude = destination.altitude;
            companion.turn.moved += cost;
            self.martial_moved(2 + side);
            self.magic_area_moved(2 + side, previous);
            self.notify_companion_event(2 + side, ReadyTrigger::Move);
            if jumped {
                self.settle_companion_flight();
                let id = 2 + side;
                let Some(companion) = self.entity(id) else {
                    return;
                };
                let landed = companion.point();
                let difficult = (landed.altitude == 0 && self.map.tile(landed.cell) == 2)
                    || self.magic_difficult(landed);
                if difficult {
                    let modifier = companion.escape_modifier(Act::EscapeAcrobatics);
                    let guidance = self.magic_guidance(id, Skill::Acrobatics);
                    let impaired = self.entity_frightened(id) || self.martial_poisoned(id);
                    if self.d20_for(id, false, impaired) + modifier + guidance < 10 {
                        self.entity_knock_prone(id);
                    }
                }
            }
        }
    }

    fn steed_opportunity_targets(&self, side: usize) -> u128 {
        [1 - side, 3 - side].into_iter()
            .filter(|&target| !self.suggestion_charmed_by(2 + side, target))
            .fold(0, |mask, target| mask | ACTS[target].bit())
    }

    fn resolve_steed_opportunity(&mut self, side: usize, target: usize, spell_move: bool) {
        let steed = self.entity_mut(2 + side).unwrap();
        steed.reaction = false;
        self.set_entity_phase(Some(EntityPhase::ResumePlayerMove { side, spell_move }));
        let build = self.f[side].build;
        self.entity_weapon_attack(
            side,
            target,
            8,
            SUMMON_STEED_RANK as i32,
            build.pb + build.casting,
        );
    }

    pub(super) fn entity_begin_player_move(&mut self) {
        self.entities.player_move_asked = false;
    }

    pub(super) fn entity_offer_player_opportunity(
        &mut self,
        mover: usize,
        destination: Pos,
    ) -> bool {
        let disengaged = if self.entities.mounted[mover] {
            self.entity(2 + mover)
                .is_some_and(|steed| steed.turn.disengaged)
        } else {
            self.disengaged
        };
        if self.entities.player_move_asked || disengaged {
            return false;
        }
        let side = 1 - mover;
        let source = if self.entities.mounted[mover] {
            2 + mover
        } else {
            mover
        };
        let destination = Point {
            cell: destination,
            altitude: self.fighter_point(mover).altitude,
        };
        let allowed = self.entity(2 + side).is_some_and(|steed| {
            matches!(steed.kind, EntityKind::Steed { .. })
                && steed.reaction
                && (if self.entities.mounted[mover] {self.steed_opportunity_targets(side)!=0}
                    else {!self.suggestion_charmed_by(2 + side,mover)})
                && !steed.martial.addled
                && !steed.martial.stunned(self.turns)
                && !self.magic_area_incapacitated(2 + side)
                && !self.magic_no_reactions(2 + side)
                && self.leaves_melee_reach(source, destination, 2 + side)
                && self.entity_visible(2 + side, source)
        });
        if allowed {
            self.entities.player_move_asked = true;
            self.set_entity_phase(Some(EntityPhase::SteedOpportunity { side, spell_mover: None }));
        }
        allowed
    }

    pub(super) fn entity_offer_spell_opportunity(&mut self, mover: usize, destination: Point) -> bool {
        let owner = mover % 2;
        let side = 1 - owner;
        let source = if mover < 2 && self.entities.mounted[owner] {2+owner} else {mover};
        let disengaged = if source >= 2 {
            self.entity(source).is_some_and(|entity| entity.turn.disengaged || entity.kind.stats().flyby)
        } else {
            self.owner == mover && self.disengaged || self.magic_agile(mover)
        };
        let steed_id = 2 + side;
        let allowed = !disengaged && self.entity(steed_id).is_some_and(|steed| {
            matches!(steed.kind, EntityKind::Steed { .. }) && steed.reaction
                && !steed.martial.addled && !steed.martial.stunned(self.turns)
                && !self.magic_area_incapacitated(steed_id) && !self.magic_no_reactions(steed_id)
                && (if self.entities.mounted[owner] { self.steed_opportunity_targets(side)!=0 }
                    else { !self.suggestion_charmed_by(steed_id,mover) })
                && self.leaves_melee_reach(source,destination,steed_id)
                && self.entity_visible(steed_id,source)
        });
        if allowed {
            self.set_entity_phase(Some(EntityPhase::SteedOpportunity { side, spell_mover: Some(mover) }));
        }
        allowed
    }

    pub(super) fn entity_resume_after_attack(&mut self, phase: EntityPhase) {
        if matches!(phase, EntityPhase::ResumePlayerMove { spell_move: true, .. }) {
            self.resume_spell_reaction_move();
        } else if matches!(phase, EntityPhase::ResumePlayerMove { .. }) {
            self.resume_move();
        } else if let EntityPhase::ResumeMove {
            side,
            destination,
            cost,
        } = phase
        {
            if self.entity(2 + side).is_some() {
                self.commit_entity_move(side, destination, cost);
                self.set_entity_phase(Some(EntityPhase::Turn(side)));
            } else {
                self.next_companion_or_advance();
            }
        } else {
            self.set_entity_phase(Some(phase));
        }
    }
}

#[cfg(test)]
mod target_tests {
    use super::*;

    fn duel_with_enemy_steed(seed: u64, player: Class, steed: Pos) -> Battle {
        let mut duel = Battle::new(seed, [player, Class::Paladin], [5, 5]);
        duel.owner = 0;
        duel.f[0].pos = Pos { x: 0, y: 0 };
        duel.f[1].pos = Pos { x: 8, y: 6 };
        duel.entities.companion[1] = Some(Entity::new(
            EntityKind::Steed {
                spirit: Spirit::Celestial,
                spell_level: 2,
            },
            1,
            steed,
            duel.entities.initiatives[1],
        ));
        duel
    }

    #[test]
    fn weapon_selection_can_hit_entity_without_hitting_its_owner() {
        for (class, weapon, target) in [
            (Class::Fighter, Act::Melee, Pos { x: 1, y: 0 }),
            (Class::Ranger, Act::Ranged, Pos { x: 4, y: 0 }),
        ] {
            let hit_occurred = (0..32).any(|seed| {
                let mut duel = duel_with_enemy_steed(seed, class, target);
                let owner_hp = duel.f[1].hp;
                let arrows = duel.f[0].arrows;
                if weapon == Act::Ranged && duel.mask() & Act::Gear2.bit() != 0 {
                    duel.step(Act::Gear2).unwrap(); // Free the hand needed for the longbow.
                }
                assert_ne!(
                    duel.mask() & weapon.bit(),
                    0,
                    "{class:?} seed {seed} target {target:?}"
                );
                duel.step(weapon).unwrap();
                assert_eq!(duel.phase_id(), 14);
                assert_ne!(duel.mask() & ACTS[3].bit(), 0);
                duel.step(ACTS[3]).unwrap();
                while matches!(duel.phase_id(), 2 | 3) && duel.mask() & Act::Pass.bit() != 0 {
                    duel.step(Act::Pass).unwrap(); // Verify the weapon hit before optional riders.
                }
                assert_eq!(duel.f[1].hp, owner_hp);
                assert!(!duel.finished());
                if weapon == Act::Ranged {
                    assert_eq!(duel.f[0].arrows, arrows - 1);
                }
                duel.entity(3).is_none_or(|steed| steed.hp < 25)
            });
            assert!(hit_occurred, "{class:?} failed to hit on 32 seeds");
        }
    }

    #[test]
    fn steed_can_offer_and_decline_its_own_opportunity_attack() {
        let mut duel = duel_with_enemy_steed(0, Class::Fighter, Pos { x: 1, y: 0 });
        duel.step(Act::S).unwrap();
        duel.step(Act::S).unwrap();
        duel.step(Act::S).unwrap();
        assert_eq!(duel.phase_id(), 63);
        assert_eq!(duel.actor(), Some(1));
        assert_eq!(duel.f[0].pos, Pos { x: 0, y: 2 });
        duel.step(Act::Pass).unwrap();
        assert_eq!(duel.phase_id(), 0);
        assert_eq!(duel.f[0].pos, Pos { x: 0, y: 3 });
        assert!(duel.entity(3).unwrap().reaction);
    }

    #[test]
    fn leaving_reach_offers_reaction_except_with_flyby() {
        for (form, reaction_offered) in [(FamiliarForm::Spider, true), (FamiliarForm::Owl, false)] {
            let mut duel = Battle::new(0, [Class::Druid, Class::Fighter], [2, 2]);
            duel.owner = 0;
            duel.f[1].pos = Pos { x: 0, y: 0 };
            duel.f[0].pos = Pos { x: 8, y: 0 };
            duel.entities.companion[0] = Some(Entity::new(
                EntityKind::Familiar {
                    form,
                    spirit: Spirit::Fey,
                },
                0,
                Pos { x: 1, y: 0 },
                15,
            ));
            duel.entity_mut(2).unwrap().begin_turn();
            duel.set_entity_phase(Some(EntityPhase::Turn(0)));
            duel.step(Act::E).unwrap();
            assert_eq!(duel.phase_id() == 61, reaction_offered);
            if reaction_offered {
                assert_eq!(duel.actor(), Some(1));
                duel.step(Act::Pass).unwrap();
            }
            assert_eq!(duel.entity(2).unwrap().position, Pos { x: 2, y: 0 });
            assert!(duel.f[1].reaction);
        }
    }

    #[test]
    fn direct_fire_bolt_targets_the_selected_entity() {
        let hit_occurred = (0..32).any(|seed| {
            let mut duel = duel_with_enemy_steed(seed, Class::Wizard, Pos { x: 4, y: 0 });
            let owner_hp = duel.f[1].hp;
            duel.step(Act::Spell5).unwrap();
            assert_eq!(duel.phase_id(), 80);
            assert_ne!(duel.mask() & ACTS[3].bit(), 0);
            duel.step(ACTS[3]).unwrap();
            assert_eq!(duel.f[1].hp, owner_hp);
            assert!(!duel.finished());
            duel.entity(3).is_none_or(|steed| steed.hp < 25)
        });
        assert!(hit_occurred);
    }

    #[test]
    fn selected_fireball_center_can_hit_only_an_enemy_entity() {
        let target = Pos { x: 8, y: 0 };
        let mut duel = duel_with_enemy_steed(5, Class::Wizard, target);
        let owner_hp = duel.f[1].hp;
        let self_hp = duel.f[0].hp;
        duel.step(Act::Spell4).unwrap();
        duel.step(ACTS[target.y as usize]).unwrap();
        duel.step(ACTS[target.x as usize]).unwrap();
        if duel.target_altitude_request().is_some() {
            duel.step(ACTS[0]).unwrap();
        }
        assert_eq!(duel.f[1].hp, owner_hp);
        assert_eq!(duel.f[0].hp, self_hp);
        assert!(duel.entity(3).is_none_or(|steed| steed.hp < 25));
    }
}
