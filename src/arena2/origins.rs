//! Fixed 2024 character creation choices for the twelve arena presets.
use super::battle::{Battle, Concentration};
use super::class::{Build, Class};
use super::equipment::Material;
use super::Act;

pub(super) const MEND_TARGET: i32 = 86;
pub(super) const STUDY_TARGET: i32 = 87;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct StudyTarget {
    pub id: u8,
    pub stamp: i32,
    pub identity: &'static str,
    pub clue: &'static str,
}
const MEND_TURNS: i32 = 10;

#[derive(Clone, Copy, Debug)]
struct MendProgress {
    object: usize,
    completed: i32,
    worked_on_turn: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
    Intimidation,
}

impl Skill {
    /// STR, DEX, CON, INT, WIS, CHA.
    pub fn ability(self) -> usize {
        use Skill::*;
        match self {
            Athletics => 0,
            Acrobatics | SleightOfHand | Stealth => 1,
            Arcana | History | Investigation | Nature | Religion => 3,
            AnimalHandling | Insight | Medicine | Perception | Survival => 4,
            Deception | Intimidation | Performance | Persuasion => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    Artisan,
    Entertainer,
    Guide,
    Noble,
    Charlatan,
    Scribe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginFeat {
    Crafter,
    Musician,
    MagicInitiateDruid,
    Skilled,
}

impl Background {
    pub const fn skills(self) -> [Skill; 2] {
        use Skill::*;
        match self {
            Self::Artisan => [Investigation, Persuasion],
            Self::Entertainer => [Acrobatics, Performance],
            Self::Guide => [Stealth, Survival],
            Self::Noble => [History, Persuasion],
            Self::Charlatan => [Deception, SleightOfHand],
            Self::Scribe => [Investigation, Perception],
        }
    }

    pub const fn feat(self) -> OriginFeat {
        match self {
            Self::Artisan => OriginFeat::Crafter,
            Self::Entertainer => OriginFeat::Musician,
            Self::Guide => OriginFeat::MagicInitiateDruid,
            Self::Noble | Self::Charlatan | Self::Scribe => OriginFeat::Skilled,
        }
    }

    pub const fn tool(self) -> &'static str {
        match self {
            Self::Artisan => "Smith's Tools",
            Self::Entertainer => "Lute",
            Self::Guide => "Cartographer's Tools",
            Self::Noble => "Dragonchess Set",
            Self::Charlatan => "Forgery Kit",
            Self::Scribe => "Calligrapher's Supplies",
        }
    }

    #[cfg(test)]
    const fn score_indices(self) -> [usize; 3] {
        match self {
            Self::Artisan => [0, 1, 3],
            Self::Entertainer => [0, 1, 5],
            Self::Guide => [1, 2, 4],
            Self::Noble => [0, 3, 5],
            Self::Charlatan => [1, 2, 5],
            Self::Scribe => [1, 3, 4],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Origin {
    pub background: Background,
    /// Standard array assigned before the background increases.
    pub array: [i32; 6],
    /// Indices of the +2 and +1 background increases.
    pub boosts: [usize; 2],
    pub asi: usize,
    pub subclass: &'static str,
    pub class_skills: &'static [Skill],
    pub skilled: &'static [Skill],
    pub class_tools: &'static [&'static str],
}

pub static ORIGINS: [Origin; 12] = {
    use Background::*;
    use Skill::*;
    [
        Origin {
            background: Artisan,
            array: [15, 13, 14, 8, 12, 10],
            boosts: [0, 1],
            asi: 0,
            subclass: "Berserker",
            class_skills: &[Athletics, Perception],
            skilled: &[],
            class_tools: &[],
        },
        Origin {
            background: Entertainer,
            array: [8, 14, 13, 10, 12, 15],
            boosts: [5, 1],
            asi: 5,
            subclass: "Lore",
            class_skills: &[Perception, Stealth, Insight],
            skilled: &[],
            class_tools: &["Bagpipes", "Drum", "Flute"],
        },
        Origin {
            background: Guide,
            array: [13, 12, 14, 8, 15, 10],
            boosts: [4, 2],
            asi: 4,
            subclass: "Life",
            class_skills: &[Insight, Religion],
            skilled: &[],
            class_tools: &[],
        },
        Origin {
            background: Guide,
            array: [8, 14, 13, 12, 15, 10],
            boosts: [4, 2],
            asi: 4,
            subclass: "Land (Arid)",
            class_skills: &[Perception, Nature],
            skilled: &[],
            class_tools: &["Herbalism Kit"],
        },
        Origin {
            background: Artisan,
            array: [15, 13, 14, 8, 12, 10],
            boosts: [0, 1],
            asi: 0,
            subclass: "Champion",
            class_skills: &[Athletics, Perception],
            skilled: &[],
            class_tools: &[],
        },
        Origin {
            background: Guide,
            array: [10, 15, 13, 8, 14, 12],
            boosts: [1, 4],
            asi: 1,
            subclass: "Open Hand",
            class_skills: &[Acrobatics, Athletics],
            skilled: &[],
            class_tools: &["Cook's Utensils"],
        },
        Origin {
            background: Noble,
            array: [15, 10, 13, 8, 12, 14],
            boosts: [0, 5],
            asi: 0,
            subclass: "Devotion",
            class_skills: &[Athletics, Insight],
            skilled: &[Perception, Stealth, Medicine],
            class_tools: &[],
        },
        Origin {
            background: Guide,
            array: [10, 15, 13, 8, 14, 12],
            boosts: [1, 4],
            asi: 1,
            subclass: "Hunter",
            class_skills: &[Perception, Investigation, AnimalHandling],
            skilled: &[],
            class_tools: &[],
        },
        Origin {
            background: Artisan,
            array: [10, 15, 14, 13, 12, 8],
            boosts: [1, 3],
            asi: 1,
            subclass: "Thief",
            class_skills: &[Acrobatics, Perception, Stealth, SleightOfHand],
            skilled: &[],
            class_tools: &["Thieves' Tools"],
        },
        Origin {
            background: Charlatan,
            array: [8, 14, 13, 10, 12, 15],
            boosts: [5, 2],
            asi: 5,
            subclass: "Draconic",
            class_skills: &[Arcana, Insight],
            skilled: &[Acrobatics, Athletics, Perception],
            class_tools: &[],
        },
        Origin {
            background: Charlatan,
            array: [8, 14, 13, 10, 12, 15],
            boosts: [5, 2],
            asi: 5,
            subclass: "Fiend",
            class_skills: &[Arcana, Investigation],
            skilled: &[Acrobatics, Athletics, Perception],
            class_tools: &[],
        },
        Origin {
            background: Scribe,
            array: [8, 13, 14, 15, 12, 10],
            boosts: [3, 1],
            asi: 3,
            subclass: "Evoker",
            class_skills: &[Arcana, Nature],
            skilled: &[Acrobatics, Stealth, Athletics],
            class_tools: &[],
        },
    ]
};

pub const fn origin(class: Class) -> &'static Origin {
    &ORIGINS[class as usize]
}

pub fn class_tools(build: Build) -> &'static [&'static str] {
    if build.class == Class::Monk && build.variant == 1 && build.level >= 3 {
        &["Cook's Utensils", "Herbalism Kit"]
    } else {
        origin(build.class).class_tools
    }
}

pub const BASE_LANGUAGES: [&str; 3] = ["Common", "Dwarvish", "Elvish"];

pub fn additional_languages(class: Class, level: i32) -> &'static [&'static str] {
    match class {
        Class::Druid => &["Druidic"],
        Class::Rogue => &["Thieves' Cant", "Goblin"],
        Class::Ranger if level >= 2 => &["Draconic", "Halfling"],
        _ => &[],
    }
}

pub fn feat_tools(feat: OriginFeat) -> &'static [&'static str] {
    match feat {
        OriginFeat::Crafter => &[
            "Leatherworker's Tools",
            "Woodcarver's Tools",
            "Tinker's Tools",
        ],
        OriginFeat::Musician => &["Horn", "Lyre", "Viol"],
        _ => &[],
    }
}

/// Each grant retains its own resource source even when the same spell is
/// granted twice, as with Guide and Druidic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OriginMagicSource {
    GuideInitiate,
    Druidic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OriginSpell {
    pub name: &'static str,
    pub tier: u8,
    pub source: OriginMagicSource,
    pub free_cast_per_long_rest: bool,
}

pub const GUIDE_MAGIC: [OriginSpell; 3] = [
    OriginSpell {
        name: "Druidcraft",
        tier: 0,
        source: OriginMagicSource::GuideInitiate,
        free_cast_per_long_rest: false,
    },
    OriginSpell {
        name: "Mending",
        tier: 0,
        source: OriginMagicSource::GuideInitiate,
        free_cast_per_long_rest: false,
    },
    OriginSpell {
        name: "Speak with Animals",
        tier: 1,
        source: OriginMagicSource::GuideInitiate,
        free_cast_per_long_rest: true,
    },
];
pub const DRUIDIC_MAGIC: OriginSpell = OriginSpell {
    name: "Speak with Animals",
    tier: 1,
    source: OriginMagicSource::Druidic,
    free_cast_per_long_rest: false,
};

pub fn origin_spells(class: Class) -> impl Iterator<Item = OriginSpell> {
    let from_guide = origin(class).background == Background::Guide;
    GUIDE_MAGIC
        .into_iter()
        .filter(move |_| from_guide)
        .chain((class == Class::Druid).then_some(DRUIDIC_MAGIC))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookEntry {
    pub name: &'static str,
    pub unlock: i32,
    pub tier: i32,
    pub savant: bool,
}

const fn book(name: &'static str, unlock: i32, tier: i32, savant: bool) -> BookEntry {
    BookEntry {
        name,
        unlock,
        tier,
        savant,
    }
}

/// Fixed Evoker spellbook independent of the spells prepared in combat.
pub static EVOKER_BOOK: [BookEntry; 17] = [
    book("Magic Missile", 1, 1, false),
    book("Burning Hands", 1, 1, false),
    book("Shield", 1, 1, false),
    book("Mage Armor", 1, 1, false),
    book("Thunderwave", 1, 1, false),
    book("Find Familiar", 1, 1, false),
    book("Grease", 2, 1, false),
    book("Expeditious Retreat", 2, 1, false),
    book("Scorching Ray", 3, 2, false),
    book("Misty Step", 3, 2, false),
    book("Shatter", 3, 2, true),
    book("Gust of Wind", 3, 2, true),
    book("Blur", 4, 2, false),
    book("Mirror Image", 4, 2, false),
    book("Fireball", 5, 3, false),
    book("Counterspell", 5, 3, false),
    book("Lightning Bolt", 5, 3, true),
];

pub fn evoker_book(level: i32) -> impl Iterator<Item = BookEntry> {
    EVOKER_BOOK
        .into_iter()
        .filter(move |entry| entry.unlock <= level)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasteryWeapon {
    Greataxe,
    Longsword,
    Rapier,
    Shortbow,
    Shortsword,
    Longbow,
    PsychicBlade,
}

/// Learning a mastery does not supply that weapon as starting equipment.
pub fn weapon_masteries(class: Class, level: i32) -> &'static [MasteryWeapon] {
    use MasteryWeapon::*;
    match class {
        Class::Barbarian if level >= 4 => &[Greataxe, Longsword, Rapier],
        Class::Barbarian => &[Greataxe, Longsword],
        Class::Fighter if level >= 4 => &[Longsword, Greataxe, Shortbow, Longbow],
        Class::Fighter => &[Longsword, Greataxe, Shortbow],
        Class::Paladin => &[Longsword, Rapier],
        Class::Ranger => &[Shortsword, Longbow],
        Class::Rogue => &[Rapier, Shortbow],
        _ => &[],
    }
}

pub fn raw_scores(class: Class, level: i32) -> [i32; 6] {
    let preset = origin(class);
    let mut scores = preset.array;
    scores[preset.boosts[0]] += 2;
    scores[preset.boosts[1]] += 1;
    if level >= 4 {
        scores[preset.asi] += 2;
    }
    scores
}

fn extra_skills(build: Build) -> &'static [Skill] {
    use Skill::*;
    match build.class {
        Class::Barbarian if build.level >= 3 => &[Nature],
        Class::Monk if build.variant == 1 && build.level >= 3 => &[Insight, Medicine],
        Class::Fighter if build.variant == 1 && build.level >= 3 => &[History],
        Class::Bard if build.variant == 0 && build.level >= 3 => &[Athletics, Deception, Investigation],
        Class::Ranger if build.variant == 1 && build.level >= 3 => &[Persuasion],
        _ => &[],
    }
}

fn expertise(class: Class, level: i32) -> &'static [Skill] {
    use Skill::*;
    match class {
        Class::Rogue => &[Stealth, SleightOfHand],
        Class::Bard if level >= 2 => &[Perception, Stealth],
        Class::Ranger if level >= 2 => &[Perception],
        Class::Wizard if level >= 2 => &[Arcana],
        _ => &[],
    }
}

/// Check modifier before advantage or dice. A rule that substitutes an
/// ability retains any proficiency in the skill.
pub fn skill_bonus(build: Build, skill: Skill, check_ability: usize) -> i32 {
    let preset = origin(build.class);
    let qualified = preset.class_skills.contains(&skill)
        || preset.background.skills().contains(&skill)
        || preset.skilled.contains(&skill)
        || extra_skills(build).contains(&skill);
    let proficiency = if expertise(build.class, build.level).contains(&skill) {
        build.pb * 2
    } else if qualified {
        build.pb
    } else if build.class == Class::Bard && build.level >= 2 {
        build.pb / 2
    } else {
        0
    };
    let otherworldly_glamour = if build.class == Class::Ranger && build.variant == 1
        && build.level >= 3 && check_ability == 5
    {
        build.stats[4].max(1)
    } else {
        0
    };
    build.stats[check_ability] + proficiency + otherworldly_glamour
}

#[derive(Clone, Debug, Default)]
pub struct OriginState {
    stonecunning_uses: i32,
    stonecunning_until: i32,
    mending: Option<MendProgress>,
    identified: Vec<(u8, i32)>,
}

impl OriginState {
    pub fn new(proficiency_bonus: i32) -> Self {
        Self {
            stonecunning_uses: proficiency_bonus,
            stonecunning_until: 0,
            mending: None,
            identified: Vec::new(),
        }
    }

    pub fn tremorsense_active(&self, turn: i32) -> bool {
        turn < self.stonecunning_until
    }
}

impl Battle {
    pub(super) fn study_main_mask(&self, i: usize) -> u128 {
        if self.action && self.study_mask(i) != 0 {
            Act::Study.bit()
        } else {
            0
        }
    }

    fn unknown_effects(&self, i: usize) -> impl Iterator<Item = StudyTarget> + '_ {
        self.study_candidates(i)
            .into_iter()
            .filter(move |target| !self.knows_spell(i, *target))
    }

    pub(super) fn study_mask(&self, i: usize) -> u128 {
        let ordinary = (1 << self.unknown_effects(i).count()) - 1;
        if self.phantasmal_force_dc(i).is_some() {
            ordinary | Act::Rider.bit()
        } else {
            ordinary
        }
    }

    pub(super) fn study_label(&self, i: usize, action: Act) -> Option<String> {
        if action == Act::Rider && self.phantasmal_force_dc(i).is_some() {
            return Some("Examine the phantasm".into());
        }
        self.unknown_effects(i)
            .nth(action as usize)
            .map(|target| format!("Study unidentified effect: {}", target.clue))
    }

    pub(super) fn command_study(&mut self, i: usize, action: Act) {
        self.action = false;
        self.resolve_readied_study(i, action);
    }

    pub(super) fn resolve_readied_study(&mut self, i: usize, action: Act) {
        let check = if action == Act::Rider {
            super::common::Check::ExaminePhantasm
        } else if let Some(target) = self.unknown_effects(i).nth(action as usize) {
            super::common::Check::Study(target)
        } else {
            return;
        };
        self.perform_common_check(i, check);
    }

    pub(super) fn can_study(&self, i: usize, target: StudyTarget) -> bool {
        self.unknown_effects(i).any(|effect| effect == target)
    }

    pub(super) fn identify_spell(&mut self, i: usize, effect: StudyTarget) {
        if self.can_study(i, effect) {
            let known = &mut self.f[i].origin.identified;
            known.retain(|&(id, _)| id != effect.id);
            known.push((effect.id, effect.stamp));
        }
    }

    pub(super) fn knows_spell(&self, i: usize, effect: StudyTarget) -> bool {
        self.f[i]
            .origin
            .identified
            .contains(&(effect.id, effect.stamp))
    }

    /// Viewer-safe metadata for the two origin decision menus.
    pub fn origin_decision_features(&self, viewer: usize) -> [f32; 7] {
        let mut features = [0.0; 7];
        if self.actor() != Some(viewer) {
            return features;
        }
        match self.phase_id() {
            MEND_TARGET => {
                let point = self.fighter_point(viewer);
                let mask = self.mend_target_mask(viewer);
                for object in 4..8 {
                    if mask & (1 << object) != 0 {
                        features[(object ^ viewer) - 4] = self
                            .mendable_object(object)
                            .map_or(0.0, |target| (point.dist(target) + 1) as f32 / 2.0);
                    }
                }
                features[4] = mask.count_ones() as f32 / 4.0;
                features[5] = 1.0; // Verbal, Somatic and Material access verified by the entry mask.
                features[6] = MEND_TURNS as f32 / 10.0;
            }
            STUDY_TARGET => {
                for (index, effect) in self.unknown_effects(viewer).enumerate().take(4) {
                    features[index] = match effect.clue {
                        "shimmering silhouette"
                        | "visibly transformed body"
                        | "shimmering field surrounding a creature" => 0.2,
                        "silver column of light"
                        | "floating spectral shape"
                        | "lingering yellow cloud"
                        | "ground covered by animated growth"
                        | "orbiting shapes" => 0.4,
                        "combatant suspended in the air" | "body immobilized without bindings" => {
                            0.6
                        }
                        "radiant coating on a weapon"
                        | "enchanted wooden weapon"
                        | "glowing held object" => 0.8,
                        _ => 1.0,
                    };
                }
                features[4] = self.study_mask(viewer).count_ones() as f32 / 4.0;
                features[5] = skill_bonus(self.f[viewer].build, Skill::Arcana, 3) as f32 / 10.0;
                features[6] = 15.0 / 30.0;
            }
            _ => {}
        }
        features
    }

    pub fn origin_mending_progress(&self, i: usize) -> Option<(usize, i32)> {
        if self.f[i].concentration != Some(Concentration::Mending) {
            return None;
        }
        self.f[i]
            .origin
            .mending
            .map(|work| (work.object, work.completed))
    }

    pub fn origin_stonecunning_uses(&self, i: usize) -> i32 {
        self.f[i].origin.stonecunning_uses
    }

    pub fn origin_tremorsense_active(&self, i: usize) -> bool {
        !self.magic_is_wolf(i) && self.f[i].origin.tremorsense_active(self.turns)
    }

    pub(super) fn origin_mask(&self, i: usize) -> u128 {
        let mut mask = 0;
        if self.bonus
            && !self.magic_is_wolf(i)
            && self.f[i].origin.stonecunning_uses > 0
            && self.map.touches_stone(self.fighter_point(i))
        {
            mask |= Act::Other1.bit();
        }
        if self.legal_mending(i) {
            let active = self.f[i]
                .origin
                .mending
                .filter(|_| self.f[i].concentration == Some(Concentration::Mending));
            if active.map_or_else(
                || self.mend_target_mask(i) != 0,
                |work| self.can_touch_break(i, work.object),
            ) {
                mask |= Act::Other2.bit();
            }
        }
        mask
    }

    pub(super) fn origin_step(&mut self, i: usize, action: Act) {
        if action == Act::Other2 {
            if self.f[i].concentration == Some(Concentration::Mending) {
                self.reveal(i);
                if let Some(work) = self.f[i].origin.mending.as_mut() {
                    self.action = false;
                    work.completed += 1;
                    work.worked_on_turn = self.turns;
                    if !self.magic_offer_mending_counterspell(i) {
                        self.origin_finish_mend_if_ready(i);
                    }
                    return;
                }
            }
            self.request(i, MEND_TARGET);
            return;
        }

        debug_assert_eq!(action, Act::Other1);
        self.bonus = false;
        let origin = &mut self.f[i].origin;
        origin.stonecunning_uses -= 1;
        // One round is six seconds and has one main turn per combatant.
        origin.stonecunning_until = self.turns + 200;
    }

    fn legal_mending(&self, i: usize) -> bool {
        origin(self.f[i].build.class).background == Background::Guide
            && !self.magic_is_wolf(i)
            && self.action
            && self.can_cast_components(i, true, true, Material::Ordinary)
    }

    fn can_touch_break(&self, i: usize, id: usize) -> bool {
        let from = self.fighter_point(i);
        self.mendable_object(id)
            .is_some_and(|point| from.dist(point) <= 1 && self.map.los_at(from, point))
    }

    pub(super) fn mend_target_mask(&self, i: usize) -> u128 {
        (4..8)
            .filter(|&id| self.can_touch_break(i, id))
            .fold(0, |mask, id| mask | (1 << id))
    }

    pub(super) fn begin_mend(&mut self, i: usize, action: Act) {
        let id = action as usize;
        debug_assert!(self.legal_mending(i) && self.can_touch_break(i, id));
        self.action = false;
        self.reveal(i);
        self.concentrate(i, Concentration::Mending, MEND_TURNS, None, 0);
        self.f[i].origin.mending = Some(MendProgress {
            object: id,
            completed: 1,
            worked_on_turn: self.turns,
        });
        self.magic_offer_mending_counterspell(i);
    }

    pub(super) fn origin_finish_mend_if_ready(&mut self, i: usize) {
        if self.f[i].concentration != Some(Concentration::Mending) {
            self.f[i].origin.mending = None;
            return;
        }
        if let Some(work) = self.f[i].origin.mending {
            if work.completed == MEND_TURNS {
                self.f[i].origin.mending = None;
                self.f[i].concentration = None;
                self.mend_object(work.object);
            }
        }
    }

    pub(super) fn origin_end(&mut self, i: usize) {
        if let Some(work) = self.f[i].origin.mending {
            if self.f[i].concentration != Some(Concentration::Mending)
                || work.worked_on_turn != self.turns
            {
                self.f[i].origin.mending = None;
                if self.f[i].concentration == Some(Concentration::Mending) {
                    self.f[i].concentration = None;
                }
            }
        }
    }

    pub(super) fn origin_label(&self, i: usize, action: Act) -> Option<String> {
        match action {
            Act::Other1 => Some(format!(
                "Stonecunning ({} uses)",
                self.f[i].origin.stonecunning_uses
            )),
            Act::Other2 => Some(
                self.f[i]
                    .origin
                    .mending
                    .filter(|_| self.f[i].concentration == Some(Concentration::Mending))
                    .map_or_else(
                        || "Mending (choose broken item)".into(),
                        |work| {
                            format!("Continue Mending ({}/{MEND_TURNS} actions)", work.completed)
                        },
                    ),
            ),
            _ => None,
        }
    }

    pub(super) fn mend_target_label(&self, action: Act) -> Option<String> {
        let id = action as usize;
        let relative = id ^ self.actor()?;
        self.mendable_object(id).map(|point| {
            let col = char::from(b'A' + point.cell.x as u8);
            format!("Repair object {relative} at {col}{}", point.cell.y + 1)
        })
    }

    pub(super) fn origin_senses(&self, observer: usize, subject: usize) -> bool {
        let viewer = self.fighter_point(observer);
        let target = self.fighter_point(subject);
        self.origin_tremorsense_active(observer)
            && viewer.dist(target) <= 12
            && self.map.shares_stone_surface(viewer, target)
    }

    /// Resistance is a typed-damage rule; it does not apply to a saving throw.
    pub(super) fn origin_resists_poison(&self, subject: usize) -> bool {
        !self.magic_is_wolf(subject)
    }

    /// Advantage applies to the Poisoned condition, not to saving throws
    /// whose only outcome is poison damage.
    pub(super) fn origin_poison_save_advantage(&self, subject: usize) -> bool {
        !self.magic_is_wolf(subject)
    }

    pub(super) fn origin_can_use_primal_strength(&self, subject: usize, skill: Skill) -> bool {
        use Skill::*;
        let fighter = &self.f[subject];
        fighter.build.class == Class::Barbarian
            && fighter.build.level >= 3
            && fighter.raging
            && matches!(
                skill,
                Acrobatics | Intimidation | Perception | Stealth | Survival
            )
    }
}

#[cfg(test)]
mod tests {
    use super::super::class::{build, CLASSES};
    use super::*;

    #[test]
    fn fixed_manifest_uses_legal_arrays_and_nondegenerate_selections() {
        let class_quota = [2, 3, 2, 2, 2, 2, 2, 3, 4, 2, 2, 2];
        for class in CLASSES {
            let record = origin(class);
            let mut array = record.array;
            array.sort_unstable();
            assert_eq!(array, [8, 10, 12, 13, 14, 15]);
            assert_ne!(record.boosts[0], record.boosts[1]);
            assert!(record
                .boosts
                .iter()
                .all(|i| record.background.score_indices().contains(i)));
            assert_eq!(record.class_skills.len(), class_quota[class as usize]);
            assert_eq!(
                record.skilled.len(),
                3 * usize::from(record.background.feat() == OriginFeat::Skilled)
            );
            for level in 1..=5 {
                let stats = raw_scores(class, level).map(|s| (s - 10).div_euclid(2));
                assert_eq!(build(class, level).stats, stats);
                let mut skills = Vec::new();
                for &skill in record
                    .class_skills
                    .iter()
                    .chain(&record.background.skills())
                    .chain(record.skilled)
                    .chain(extra_skills(build(class, level)))
                {
                    assert!(!skills.contains(&skill));
                    skills.push(skill);
                }
                assert!(expertise(class, level)
                    .iter()
                    .all(|skill| skills.contains(skill)));
            }
        }
    }

    #[test]
    fn skill_bonus_uses_expertise_selected_ability_and_jack_of_all_trades() {
        let bard = build(Class::Bard, 2);
        assert_eq!(skill_bonus(bard, Skill::Stealth, 1), bard.stats[1] + 4);
        assert_eq!(skill_bonus(bard, Skill::Nature, 3), bard.stats[3] + 1);
        assert_eq!(skill_bonus(bard, Skill::Performance, 5), bard.stats[5] + 2);
        let rogue = build(Class::Rogue, 1);
        assert_eq!(
            skill_bonus(rogue, Skill::SleightOfHand, 1),
            rogue.stats[1] + 4
        );
        let wizard = build(Class::Wizard, 2);
        assert_eq!(skill_bonus(wizard, Skill::Arcana, 3), wizard.stats[3] + 4);
        let ranger = build(Class::Ranger, 2);
        assert_eq!(
            skill_bonus(ranger, Skill::Perception, 4),
            ranger.stats[4] + 4
        );
        assert_eq!(additional_languages(Class::Ranger, 1), &[] as &[&str]);
        assert_eq!(
            additional_languages(Class::Ranger, 2),
            &["Draconic", "Halfling"]
        );
        let barbarian = build(Class::Barbarian, 3);
        assert_eq!(
            skill_bonus(barbarian, Skill::Perception, 0),
            barbarian.stats[0] + 2
        );
        assert_eq!(
            skill_bonus(barbarian, Skill::Perception, 4),
            barbarian.stats[4] + 2
        );
    }

    #[test]
    fn guide_and_druidic_grants_and_evoker_spellbook_keep_their_distinct_sources() {
        let guide = origin_spells(Class::Monk).collect::<Vec<_>>();
        assert_eq!(guide, GUIDE_MAGIC);
        let druid = origin_spells(Class::Druid).collect::<Vec<_>>();
        let speak = druid
            .iter()
            .filter(|spell| spell.name == "Speak with Animals")
            .collect::<Vec<_>>();
        assert_eq!(speak.len(), 2);
        assert_eq!(additional_languages(Class::Druid, 1), &["Druidic"]);
        assert_ne!(speak[0].source, speak[1].source);
        assert!(speak[0].free_cast_per_long_rest);
        assert!(!speak[1].free_cast_per_long_rest);
        assert_eq!(origin_spells(Class::Wizard).count(), 0);

        for (index, count) in [6, 8, 12, 14, 17].into_iter().enumerate() {
            let level = index as i32 + 1;
            let book = evoker_book(level).collect::<Vec<_>>();
            assert_eq!(book.len(), count);
            assert!(book.iter().all(|entry| entry.tier <= (level + 1) / 2));
            assert_eq!(
                book.iter().filter(|entry| entry.savant).count(),
                if level >= 5 {
                    3
                } else if level >= 3 {
                    2
                } else {
                    0
                }
            );
            assert!(book.iter().any(|entry| entry.name == "Find Familiar"));
        }
    }

    #[test]
    fn fixed_presets_have_exact_level_hp_and_saves() {
        // PHB core traits: STR, DEX, CON, INT, WIS, CHA.
        let saves = [
            (0, 2),
            (1, 5),
            (4, 5),
            (3, 4),
            (0, 2),
            (0, 1),
            (4, 5),
            (0, 1),
            (1, 3),
            (2, 5),
            (4, 5),
            (3, 4),
        ];
        // Max first die, fixed advancement with CON, Dwarven Toughness,
        // and the published Draconic level-3 increase.
        let hit_points = [
            [15, 25, 35, 45, 55],
            [10, 17, 24, 31, 38],
            [11, 19, 27, 35, 43],
            [11, 19, 27, 35, 43],
            [13, 22, 31, 40, 49],
            [10, 17, 24, 31, 38],
            [12, 20, 28, 36, 44],
            [12, 20, 28, 36, 44],
            [11, 19, 27, 35, 43],
            [9, 16, 26, 34, 42],
            [11, 19, 27, 35, 43],
            [9, 16, 23, 30, 37],
        ];
        for class in CLASSES {
            let index = class as usize;
            let (first, second) = saves[index];
            for level in 1..=5 {
                let character = build(class, level);
                assert_eq!(
                    character.saves,
                    std::array::from_fn(|stat| stat == first || stat == second)
                );
                assert_eq!(character.hp, hit_points[index][level as usize - 1]);
                assert_eq!(character.pb, if level < 5 { 2 } else { 3 });
                if class == Class::Sorcerer {
                    assert_eq!(
                        character.ac,
                        10 + character.stats[1] + if level >= 3 { character.stats[5] } else { 0 }
                    );
                }
            }
        }
    }

    #[test]
    fn mastery_training_meets_class_quotas_and_preserves_existing_weapon_picks() {
        use MasteryWeapon::*;
        for class in CLASSES {
            for level in 1..=5 {
                let (required, retained): (usize, &[MasteryWeapon]) = match class {
                    Class::Barbarian => (2 + usize::from(level >= 4), &[Greataxe]),
                    Class::Fighter => (3 + usize::from(level >= 4), &[Longsword]),
                    Class::Paladin => (2, &[Longsword]),
                    Class::Ranger => (2, &[Shortsword, Longbow]),
                    Class::Rogue => (2, &[Rapier, Shortbow]),
                    _ => (0, &[]),
                };
                let masteries = weapon_masteries(class, level);
                assert_eq!(masteries.len(), required);
                assert!(retained.iter().all(|weapon| masteries.contains(weapon)));
                for (i, weapon) in masteries.iter().enumerate() {
                    assert!(!masteries[..i].contains(weapon));
                }
            }
        }
    }

    #[test]
    fn dwarven_poison_resilience_is_typed_and_condition_save_uses_advantage() {
        use super::super::equipment::DamageType;

        let mut observed_advantage = false;
        for seed in 0..32 {
            let mut normal = Battle::new(seed, [Class::Fighter; 2], [5; 2]);
            let mut resilient = normal.clone();
            let normal_passed = normal.save(0, 2, 15);
            let resilient_passed = resilient.save_poison_condition(0, 15);
            assert!(!normal_passed || resilient_passed);
            observed_advantage |= !normal_passed && resilient_passed;
        }
        assert!(observed_advantage);

        let mut duel = Battle::new(0, [Class::Fighter; 2], [5; 2]);
        let hp = duel.f[0].hp;
        duel.damage_typed(0, 7, DamageType::Poison);
        assert_eq!(duel.f[0].hp, hp - 3);
        duel.damage_typed(0, 7, DamageType::Piercing);
        assert_eq!(duel.f[0].hp, hp - 10);
    }

    #[test]
    fn study_reveals_an_observable_spell_to_that_viewer_without_exposing_expiry() {
        use super::super::geometry::Pos;
        let mut demonstrated_success = false;
        for seed in 0..32 {
            let mut duel = Battle::new(seed, [Class::Wizard, Class::Druid], [3; 2]);
            if duel.actor() != Some(0) {
                continue;
            }
            duel.f[0].pos = Pos { x: 0, y: 0 };
            duel.f[1].pos = Pos { x: 1, y: 0 };
            duel.concentrate(1, Concentration::Blur, 10, None, 2);
            let before = duel.magic_state_features(0, 1);
            assert_eq!(&before[..3], &[0.0, 0.0, 0.0]);
            assert!(duel.magic_state_features(1, 1)[0] > 0.0);
            let public = duel.magic_public_summary(0, 1);
            assert!(public.contains("shimmering silhouette"));
            assert!(!public.contains("Blur"));

            assert_ne!(duel.mask() & Act::Study.bit(), 0);
            duel.step(Act::Study).unwrap();
            assert_eq!(duel.phase_id(), STUDY_TARGET);
            assert_eq!(duel.mask(), 1);
            duel.step(Act::N).unwrap();
            assert!(!duel.action);
            if !duel.magic_public_summary(0, 1).contains("Identified: Blur") {
                continue;
            }
            demonstrated_success = true;
            let after = duel.magic_state_features(0, 1);
            assert!(after[0] > 0.0);
            assert_eq!(&after[1..3], &[0.0, 0.0]);
            duel.turns += 1;
            duel.concentrate(1, Concentration::Blur, 10, None, 2);
            assert!(!duel.magic_public_summary(0, 1).contains("Identified: Blur"));
            break;
        }
        assert!(demonstrated_success);
    }

    #[test]
    fn imperceptible_concentration_does_not_open_study_or_reveal_spell_identity() {
        use super::super::geometry::Pos;
        let mut duel = Battle::new(0, [Class::Wizard, Class::Cleric], [3; 2]);
        duel.f[0].pos = Pos { x: 0, y: 0 };
        duel.f[1].pos = Pos { x: 1, y: 0 };
        duel.concentrate(1, Concentration::Bless, 10, None, 1);
        assert_eq!(duel.study_mask(0), 0);
        assert_eq!(duel.magic_public_summary(0, 1), "");
        assert!(!duel.public_concentration(0, 1));
        assert!(duel.public_concentration(1, 1));
        assert_eq!(&duel.magic_state_features(0, 1)[..3], &[0.0, 0.0, 0.0]);
    }

    fn guide_with_broken_bow() -> Battle {
        use super::super::equipment::{Carried, DamageType};
        let mut duel = (0..32)
            .map(|seed| Battle::new(seed, [Class::Monk, Class::Ranger], [1; 2]))
            .find(|duel| duel.actor() == Some(0))
            .expect("monk wins a suitable initiative");
        duel.f[1].gear.bow = Carried::Ground(duel.f[0].pos);
        assert_eq!(duel.damage_object(7, 1, DamageType::Piercing), 1);
        assert_eq!(duel.mendable_object(7), None);
        assert_eq!(duel.mask() & Act::Other2.bit(), 0);
        duel.damage_object(7, 40, DamageType::Slashing);
        assert!(duel.mendable_object(7).is_some());
        duel
    }

    #[test]
    fn mending_repairs_only_a_touched_break_after_ten_own_turn_actions() {
        let mut duel = guide_with_broken_bow();
        for turn in 1..=10 {
            assert_ne!(duel.mask() & Act::Other2.bit(), 0);
            duel.step(Act::Other2).unwrap();
            if turn == 1 {
                assert_eq!(duel.phase_id(), MEND_TARGET);
                assert_eq!(duel.mask(), 1 << 7);
                duel.step(super::super::ACTS[7]).unwrap();
            }
            assert!(!duel.action);
            if turn < 10 {
                assert_eq!(duel.f[0].concentration, Some(Concentration::Mending));
                assert!(duel.mendable_object(7).is_some());
                duel.step(Act::End).unwrap();
                duel.step(Act::End).unwrap();
            }
        }
        assert_eq!(duel.f[0].concentration, None);
        assert_eq!(duel.mendable_object(7), None);
        assert_eq!(duel.object(7).expect("broken bow restored").hp, 9);
    }

    #[test]
    fn visible_mending_can_be_countered_without_spending_a_caster_slot() {
        use super::super::equipment::{Carried, DamageType};
        use super::super::geometry::Pos;
        let mut demonstrated_counter = false;
        for seed in 0..64 {
            let mut duel = Battle::new(seed, [Class::Ranger, Class::Warlock], [5; 2]);
            if duel.actor() != Some(0) {
                continue;
            }
            duel.f[0].pos = Pos { x: 0, y: 0 };
            duel.f[1].pos = Pos { x: 1, y: 0 };
            duel.f[0].gear.bow = Carried::Ground(duel.f[0].pos);
            duel.damage_object(6, 40, DamageType::Slashing);
            let initial_slots = duel.f[0].slots;

            duel.step(Act::Other2).unwrap();
            duel.step(super::super::ACTS[6]).unwrap();
            assert_eq!(duel.actor(), Some(1));
            assert_eq!(duel.mask(), Act::Defend.bit() | Act::Pass.bit());
            duel.step(Act::Defend).unwrap();
            assert_eq!(duel.f[1].slots, [0, 0, 1]);
            assert_eq!(duel.f[0].slots, initial_slots);
            assert!(!duel.action);
            assert!(duel.mendable_object(6).is_some());
            if duel.f[0].concentration.is_none() {
                demonstrated_counter = true;
                assert_eq!(duel.origin_mending_progress(0), None);
                break;
            }
        }
        assert!(demonstrated_counter);
    }

    #[test]
    fn skipping_a_turn_or_dropping_concentration_resets_mending() {
        let mut duel = guide_with_broken_bow();
        duel.step(Act::Other2).unwrap();
        duel.step(super::super::ACTS[7]).unwrap();
        duel.step(Act::End).unwrap();
        duel.step(Act::End).unwrap();
        duel.step(Act::End).unwrap(); // Skip the next action to work on the cast.
        assert_eq!(duel.f[0].concentration, None);
        duel.step(Act::End).unwrap();

        duel.step(Act::Other2).unwrap();
        assert_eq!(duel.phase_id(), MEND_TARGET);
        duel.step(super::super::ACTS[7]).unwrap();
        duel.step(Act::DropConcentration).unwrap();
        assert_eq!(duel.f[0].concentration, None);
        assert!(duel.mendable_object(7).is_some());
    }

    #[test]
    fn stonecunning_spends_bonus_and_use_and_detects_across_stone_floor() {
        let mut duel = Battle::new(0, [Class::Fighter; 2], [5; 2]);
        let seat = duel.actor().unwrap();
        duel.f[1 - seat].hidden_dc = Some(18);
        assert!(!duel.origin_senses(seat, 1 - seat));
        assert_eq!(duel.observed_pos(seat, 1 - seat), None);
        assert_ne!(duel.mask() & Act::Other1.bit(), 0);
        duel.step(Act::Other1).unwrap();
        assert!(!duel.bonus);
        assert_eq!(duel.f[seat].origin.stonecunning_uses, 2);
        assert!(duel.origin_senses(seat, 1 - seat));
        assert_eq!(
            duel.observed_pos(seat, 1 - seat),
            Some(duel.f[1 - seat].pos)
        );
        assert!(!duel.can_see(seat, 1 - seat));
        duel.turns += 200;
        assert!(!duel.origin_senses(seat, 1 - seat));
    }
}
