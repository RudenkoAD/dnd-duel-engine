use super::battle::{Battle, Hit};
use super::class::Class;
use super::equipment::DamageType;
use super::geometry::{Point, Pos};
use super::spell_movement::{Compulsion, Suggestion as SuggestionPlan};
use super::targeting::Area;
use super::temp_hp::TempSource;
#[path = "magic_roster_fields.rs"]
mod roster_fields;
#[path = "magic_areas.rs"]
mod areas;
use super::{Act, ACTS};
pub(super) use areas::AreaState;

#[path = "magic_allies.rs"]
mod allies;
#[path = "magic_class_features.rs"]
mod class_features;
#[path = "magic_dispel.rs"]
mod dispel;
#[path = "magic_forms.rs"]
mod forms;
#[path = "magic_phantasm.rs"]
mod phantasm;
pub(super) use phantasm::{PHANTASM_CELL, PHANTASM_DAMAGE, PHANTASM_HEIGHT};
#[path = "magic_projectiles.rs"]
pub(super) mod projectiles;
#[path = "magic_ready.rs"]
mod ready;
#[path = "target_ids_magic.rs"]
mod target_ids_magic;
#[path = "magic_divine_spells.rs"]
mod divine_spells;
#[path = "magic_alarm.rs"]
mod alarm;
use forms::{WildForm, WILD_FORMS};

const DEX: usize = 1;
const CON: usize = 2;
const WIS: usize = 4;
const CHA: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Concentration {
    Bless,
    Hex,
    HunterMark,
    HypnoticPattern,
    Moonbeam,
    SpiritualWeapon,
    SpiritGuardians,
    Blur,
    AlterSelf,
    DragonBreath,
    Fear,
    Fly,
    Suggestion,
    StinkingCloud,
    Protection,
    ShieldOfFaith,
    ReadiedSpell,
    HoldPerson,
    Entangle,
    Heroism,
    EnsnaringStrike,
    Bane,
    ExpeditiousRetreat,
    Guidance,
    Resistance,
    ProtectionFromEnergy,
    Mending,
    AlarmCasting,
    FogCloud,
    GustOfWind,
    Invisibility,
    PassWithoutTrace,
    CrusadersMantle,
    EnhanceAbility,
    HungerOfHadar,
    FaerieFire,
    HideousLaughter,
    CalmEmotions,
    Sleep,
    DetectThoughts,
    PhantasmalForce,
}

#[derive(Clone, Debug)]
pub(super) struct MagicState {
    slot_floor: usize,
    cast_tier: Option<usize>,
    expires: i32,
    innate_until: i32,
    favor_until: i32,
    shillelagh_until: i32,
    shillelagh_weapon: Option<usize>,
    shillelagh_strength: bool,
    form: bool,
    form_id: usize,
    next_form: usize,
    shape_drop: u8,
    bear_next_rend: bool,
    field: Option<Point>,
    field_level: usize,
    field_last_turn: [i32; 4],
    field_blocks_shape: [bool; 4],
    attack: Option<Spell>,
    projectiles: projectiles::ProjectileState,
    projectile_marks: projectiles::ProjectileMarks,
    attack_target: Option<usize>,
    cast_target: Option<usize>,
    cure_via_familiar: Option<bool>,
    cast_targets: u8,
    mark_target: Option<usize>,
    phantasm_target: Option<usize>,
    phantasm_cell: Option<Pos>,
    phantasm_damage_turn: i32,
    mark_defeat_turn: Option<i32>,
    ally_inspiration_until: i32,
    class_features: class_features::ClassFeatureState,
    rays: i32,
    mocked_until: i32,
    guided_until: i32,
    resurgence_turn: i32,
    resurgence_slot: bool,
    choice: Option<MagicChoice>,
    counter_suspended: Option<(MagicChoice, i32)>,
    shield_hit: Option<Hit>,
    projectile_shield: bool,
    ensnaring_hit: Option<Hit>,
    shillelagh_force: bool,
    divine_spark_kind: DamageType,
    element: DamageType,
    breath_element: DamageType,
    hex_ability: usize,
    hex_pending_ability: usize,
    force_mark_slot: bool,
    metamagic: Option<(Metamagic, Act)>,
    heightened_target: Option<usize>,
    concentration_heightened_target: Option<usize>,
    resolving_center: Option<Point>,
    direction: usize,
    cube_offset: [i8; 2],
    aid_hp: i32,
    ally_selection: u8,
    ally_recipients: u8,
    word: Option<(usize, CommandWord)>,
    chosen_word: CommandWord,
    form_option: usize,
    suggestion: Option<SuggestionPlan>,
    zone_truth: Option<(Point, i32)>,
    poisoned_turn: i32,
    magic_restricted_turn: i32,
    readied: Option<(Spell, usize)>,
    ready_state: ready::ReadySpellState,
    awaiting_rite: Option<i32>,
    rite_actions: u8,
    longstrider_until: i32,
    jump_until: i32,
    spell_weapon_until: i32,
    spell_weapon_bonus: i32,
    spell_weapon: Option<(usize, bool)>,
    sanctuary_until: i32,
    sanctuary_source: Option<usize>,
    roster_fields: roster_fields::RosterFieldState,
    mage_armor_until: i32,
    shocking_targets: u8,
    held: bool,
    restrained: bool,
    repelling_pending: i32,
    bane_recipients: u8,
    guidance_skill: super::origins::Skill,
    enhanced_ability: usize,
    gust_direction: usize,
    gust_cast_turn: i32,
    alarm: alarm::AlarmState,
    nondetection_until: [i32; 4],
    true_strike_ranged: bool,
    true_strike_thrown: bool,
    true_strike_radiant: bool,
    light_until: i32,
    light_target: usize,
    light_color: usize,
    aux_choice: usize,
    protected_type: DamageType,
    resistance_tick: i32,
    finished_rite: bool,
}

impl Default for MagicState {
    fn default() -> Self {
        Self {
            slot_floor: 1,
            cast_tier: None,
            expires: 0,
            innate_until: 0,
            favor_until: 0,
            shillelagh_until: 0,
            shillelagh_weapon: None,
            shillelagh_strength: false,
            form: false,
            form_id: 0,
            next_form: 0,
            shape_drop: 0,
            bear_next_rend: false,
            field: None,
            field_level: 0,
            field_last_turn: [-1; 4],
            field_blocks_shape: [false; 4],
            attack: None,
            projectiles: projectiles::ProjectileState::default(),
            projectile_marks: projectiles::ProjectileMarks::default(),
            attack_target: None,
            cast_target: None,
            cure_via_familiar: None,
            cast_targets: 0,
            mark_target: None,
            phantasm_target: None,
            phantasm_cell: None,
            phantasm_damage_turn: -1,
            mark_defeat_turn: None,
            ally_inspiration_until: 0,
            class_features: class_features::ClassFeatureState::default(),
            rays: 0,
            mocked_until: 0,
            guided_until: 0,
            resurgence_turn: -1,
            resurgence_slot: false,
            choice: None,
            counter_suspended: None,
            shield_hit: None,
            projectile_shield: false,
            ensnaring_hit: None,
            shillelagh_force: false,
            divine_spark_kind: DamageType::Radiant,
            element: DamageType::Fire,
            breath_element: DamageType::Fire,
            hex_ability: 0,
            hex_pending_ability: 0,
            force_mark_slot: false,
            metamagic: None,
            heightened_target: None,
            concentration_heightened_target: None,
            resolving_center: None,
            direction: 0,
            cube_offset: [0, 0],
            aid_hp: 0,
            ally_selection: 0,
            ally_recipients: 0,
            word: None,
            chosen_word: CommandWord::Halt,
            form_option: 0,
            suggestion: None,
            zone_truth: None,
            poisoned_turn: -1,
            magic_restricted_turn: -1,
            readied: None,
            ready_state: ready::ReadySpellState::default(),
            awaiting_rite: None,
            rite_actions: 0,
            longstrider_until: 0,
            jump_until: 0,
            spell_weapon_until: 0,
            spell_weapon_bonus: 0,
            spell_weapon: None,
            sanctuary_until: 0,
            sanctuary_source: None,
            roster_fields: Default::default(),
            mage_armor_until: 0,
            shocking_targets: 0,
            held: false,
            restrained: false,
            repelling_pending: 0,
            bane_recipients: 0,
            guidance_skill: super::origins::Skill::Acrobatics,
            enhanced_ability: 0,
            gust_direction: 0,
            gust_cast_turn: -1,
            alarm: alarm::AlarmState::default(),
            nondetection_until: [0; 4],
            true_strike_ranged: false,
            true_strike_thrown: false,
            true_strike_radiant: false,
            light_until: 0,
            light_target: 0,
            light_color: 0,
            aux_choice: 0,
            protected_type: DamageType::Fire,
            resistance_tick: -1,
            finished_rite: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommandWord {
    Approach,
    Drop,
    Flee,
    Grovel,
    Halt,
}

#[derive(Clone, Copy, Debug)]
enum Metamagic {
    Quickened,
    Heightened,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::arena2) enum SpellRider {
    MindSliver,
    ArmsOfHadar,
    None,
    Mockery(usize),
    Whispers(usize),
    Thunderwave(Point),
    LandHeal(usize, i32),
    SeaPush(usize),
}

#[derive(Clone, Copy, Debug)]
struct SpellDamage {
    source: usize,
    emitter: Option<usize>,
    amount: i32,
    kind: DamageType,
    recipients: u8,
    saved: u8,
    checked: u8,
    rider: SpellRider,
    objects: Option<(Point, Area)>,
}

#[derive(Clone, Copy, Debug)]
enum MagicChoice {
    Direction(Act),
    CubeFace(Act),
    Element(Act),
    Form(Act),
    Command(Act),
    CastSlot {
        action: Act,
        center: Option<Point>,
    },
    ShieldSlot,
    EnsnaringSlot,
    ShillelaghType(Act),
    DivineSparkType,
    CreateSlot,
    ConvertSlot,
    RecoverBard,
    RecoverDruid,
    Guidance(Act),
    EnhanceAbility(Act),
    AlarmMode { center: Point },
    AlarmExempt { center: Point, audible: bool },
    TrueStrikeWeapon(Act),
    TrueStrikeType(Act),
    WeaponEnchantment(Act),
    HexAbility,
    WildShape,
    Dispel(Act),
    Creature(Act),
    CureRoute(Act),
    GroupTargets(Act),
    HeightenedTarget {
        action: Act,
        candidates: u8,
        center: Option<Point>,
    },
    Allies(Act),
    SpiritualStrike,
    LightTarget(Act),
    LightColor {
        action: Act,
        target: usize,
    },
    AllyConsent {
        caster: usize,
        action: Act,
        subset: u8,
    },
    MarkCost,
    MetaMode,
    MetaSpell(Metamagic),
    CounterResponse {
        responder: usize,
    },
    Counterspell {
        source: usize,
        spell: Spell,
        tier: usize,
        heightened: bool,
        heightened_target: Option<usize>,
        center: Option<Point>,
        target: Option<usize>,
        cure_route: Option<bool>,
        targets: u8,
        old_slot_turn: i32,
    },
    Damage(SpellDamage),
}

impl MagicChoice {
    fn phase(self) -> i32 {
        match self {
            Self::Direction(_) | Self::CubeFace(_) => 34,
            Self::ShillelaghType(_) | Self::DivineSparkType | Self::LightColor { .. } => 35,
            Self::Element(_) | Self::AlarmMode { .. } | Self::AlarmExempt { .. } => 35,
            Self::Form(_) => 36,
            Self::Command(_) => 37,
            Self::CastSlot { .. }
            | Self::ShieldSlot
            | Self::EnsnaringSlot
            | Self::CreateSlot
            | Self::ConvertSlot
            | Self::RecoverBard
            | Self::RecoverDruid => 38,
            Self::Damage(_) => 40,
            Self::HexAbility => 41,
            Self::MarkCost => 42,
            Self::MetaMode => 43,
            Self::MetaSpell(_) => 44,
            Self::WildShape => 45,
            Self::Dispel(_) => 46,
            Self::Counterspell { .. } | Self::CounterResponse { .. } => 47,
            Self::Creature(_)
            | Self::CureRoute(_)
            | Self::GroupTargets(_)
            | Self::HeightenedTarget { .. }
            | Self::Allies(_)
            | Self::AllyConsent { .. }
            | Self::SpiritualStrike
            | Self::LightTarget(_) => 48,
            Self::Guidance(_)
            | Self::EnhanceAbility(_)
            | Self::TrueStrikeWeapon(_)
            | Self::TrueStrikeType(_)
            | Self::WeaponEnchantment(_) => 49,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::arena2) enum Spell {
    ViciousMockery,
    StarryWisp,
    SacredFlame,
    FireBolt,
    RayOfFrost,
    EldritchBlast,
    Shillelagh,
    DissonantWhispers,
    HealingWord,
    CureWounds,
    GuidingBolt,
    Bless,
    Thunderwave,
    BurningHands,
    ChromaticOrb,
    MagicMissile,
    Hex,
    DivineFavor,
    HunterMark,
    Shatter,
    SpiritualWeapon,
    Moonbeam,
    ScorchingRay,
    HypnoticPattern,
    SpiritGuardians,
    Fireball,
    Aid,
    LesserRestoration,
    MassHealingWord,
    Revivify,
    Blur,
    AlterSelf,
    Command,
    DragonBreath,
    Fear,
    Fly,
    Suggestion,
    StinkingCloud,
    Protection,
    ShieldOfFaith,
    ZoneOfTruth,
    Shield,
    HoldPerson,
    DispelMagic,
    Entangle,
    Longstrider,
    Jump,
    Guidance,
    Resistance,
    ProtectionFromEnergy,
    Light,
    Heroism,
    Sanctuary,
    EnsnaringStrike,
    MagicWeapon,
    ShockingGrasp,
    TrueStrike,
    PoisonSpray,
    Bane,
    ExpeditiousRetreat,
    Counterspell,
    MistyStep,
    MageArmor,
    Mending,
    HeldSpell,
    FiendishVigor,
    Elementalism,
    ArmsOfHadar,
    HideousLaughter,
    CalmEmotions,
    Sleep,
    DetectThoughts,
    SeeInvisibility,
    FogCloud,
    GustOfWind,
    LightningBolt,
    Invisibility,
    PassWithoutTrace,
    CrusadersMantle,
    EnhanceAbility,
    CharmPerson,
    MirrorImage,
    RopeTrick,
    DisguiseSelf,
    HungerOfHadar,
    FaerieFire,
    MindSliver,
    Blink,
    PlantGrowth,
    Alarm,
    Nondetection,
    PhantasmalForce,
}

impl Spell {
    pub(super) fn label(self) -> &'static str {
        use Spell::*;
        match self {
            Alarm => "Alarm",
            Nondetection => "Nondetection",
            ArmsOfHadar => "Arms of Hadar",
            HideousLaughter => "Tasha’s Hideous Laughter",
            CalmEmotions => "Calm Emotions",
            Sleep => "Sleep",
            DetectThoughts => "Detect Thoughts",
            PhantasmalForce => "Phantasmal Force",
            SeeInvisibility => "See Invisibility",
            ViciousMockery => "Vicious Mockery",
            FogCloud => "Fog Cloud",
            GustOfWind => "Gust of Wind",
            LightningBolt => "Lightning Bolt",
            Invisibility => "Invisibility",
            PassWithoutTrace => "Pass without Trace",
            CrusadersMantle => "Crusader's Mantle",
            EnhanceAbility => "Enhance Ability",
            StarryWisp => "Starry Wisp",
            SacredFlame => "Sacred Flame",
            FireBolt => "Fire Bolt",
            RayOfFrost => "Ray of Frost",
            EldritchBlast => "Eldritch Blast",
            Shillelagh => "Shillelagh",
            DissonantWhispers => "Dissonant Whispers",
            HealingWord => "Healing Word",
            CureWounds => "Cure Wounds",
            GuidingBolt => "Guiding Bolt",
            Bless => "Bless",
            Thunderwave => "Thunderwave",
            BurningHands => "Burning Hands",
            ChromaticOrb => "Chromatic Orb",
            MagicMissile => "Magic Missile",
            Hex => "Hex",
            DivineFavor => "Divine Favor",
            HunterMark => "Hunter's Mark",
            Shatter => "Shatter",
            SpiritualWeapon => "Spiritual Weapon",
            Moonbeam => "Moonbeam",
            ScorchingRay => "Scorching Ray",
            HypnoticPattern => "Hypnotic Pattern",
            SpiritGuardians => "Spirit Guardians",
            Fireball => "Fireball",
            Aid => "Aid",
            LesserRestoration => "Lesser Restoration",
            MassHealingWord => "Mass Healing Word",
            Revivify => "Revivify",
            Blur => "Blur",
            AlterSelf => "Alter Self",
            Command => "Command",
            DragonBreath => "Dragon's Breath",
            Fear => "Fear",
            Fly => "Fly",
            Suggestion => "Suggestion",
            StinkingCloud => "Stinking Cloud",
            Protection => "Protection from Evil and Good",
            ShieldOfFaith => "Shield of Faith",
            ZoneOfTruth => "Zone of Truth",
            Shield => "Shield",
            HoldPerson => "Hold Person",
            DispelMagic => "Dispel Magic",
            Entangle => "Entangle",
            Longstrider => "Longstrider",
            Jump => "Jump",
            Guidance => "Guidance",
            Resistance => "Resistance",
            ProtectionFromEnergy => "Protection from Energy",
            Light => "Light",
            Heroism => "Heroism",
            Sanctuary => "Sanctuary",
            EnsnaringStrike => "Ensnaring Strike",
            MagicWeapon => "Magic Weapon",
            ShockingGrasp => "Shocking Grasp",
            TrueStrike => "True Strike",
            PoisonSpray => "Poison Spray",
            Bane => "Bane",
            ExpeditiousRetreat => "Expeditious Retreat",
            Counterspell => "Counterspell",
            MistyStep => "Misty Step",
            CharmPerson => "Charm Person",
            MirrorImage => "Mirror Image",
            RopeTrick => "Rope Trick",
            DisguiseSelf => "Disguise Self",
            MageArmor => "Mage Armor",
            Mending => "Mending",
            HeldSpell => "readied spell",
            Blink => "Blink",
            PlantGrowth => "Plant Growth",
            FaerieFire => "Faerie Fire",
            MindSliver => "Mind Sliver",
            HungerOfHadar => "Hunger of Hadar",
            FiendishVigor => "Fiendish Vigor",
            Elementalism => "Elementalism",
        }
    }

    fn level(self) -> usize {
        use Spell::*;
        match self {
            GustOfWind | Invisibility | PassWithoutTrace | EnhanceAbility => 2,
            MindSliver | ViciousMockery | StarryWisp | SacredFlame | FireBolt | RayOfFrost | EldritchBlast
            | ShockingGrasp | PoisonSpray | TrueStrike | Light | Guidance | Resistance
            | Mending | Shillelagh | Elementalism => 0,
            DetectThoughts | SeeInvisibility | CalmEmotions | Shatter | SpiritualWeapon | Moonbeam | ScorchingRay | Aid | LesserRestoration
            | Blur | AlterSelf | DragonBreath | Suggestion | ZoneOfTruth | HoldPerson
            | MagicWeapon | MistyStep | MirrorImage | RopeTrick | PhantasmalForce => 2,
            HypnoticPattern | SpiritGuardians | Fireball | Revivify | MassHealingWord | Fear
            | Fly | Blink | PlantGrowth | HungerOfHadar | StinkingCloud | DispelMagic | ProtectionFromEnergy | Counterspell
            | LightningBolt | CrusadersMantle | Nondetection => 3,
            _ => 1,
        }
    }

    fn bonus(self) -> bool {
        use Spell::*;
        matches!(
            self,
            HealingWord
                | SpiritualWeapon
                | Hex
                | DivineFavor
                | HunterMark
                | Shillelagh
                | LesserRestoration
                | MassHealingWord
                | DragonBreath
                | ShieldOfFaith
                | Jump
                | MistyStep
                | Sanctuary
                | EnsnaringStrike
                | MagicWeapon
                | ExpeditiousRetreat
        )
    }

    fn range(self) -> Option<i32> {
        use Spell::*;
        match self {
            FogCloud | GustOfWind | LightningBolt | Invisibility | PassWithoutTrace
            | CrusadersMantle | EnhanceAbility | Alarm | Nondetection => None,
            Blink | PlantGrowth | FaerieFire | HungerOfHadar | HealingWord | CureWounds | Bless | DivineFavor | Shillelagh | SpiritGuardians
            | Shatter | Fireball | HypnoticPattern | Moonbeam | Aid | Blur | AlterSelf
            | DragonBreath | Fear | Fly | StinkingCloud | ZoneOfTruth | LesserRestoration
            | MassHealingWord | Protection | ShieldOfFaith | Revivify | Thunderwave
            | BurningHands | SpiritualWeapon => None,
            Suggestion | Bane | CharmPerson => Some(6),
            Entangle | Longstrider | Jump | Guidance | Resistance | ProtectionFromEnergy
            | Light | Heroism | Sanctuary | EnsnaringStrike | MagicWeapon | ExpeditiousRetreat
            | MistyStep | MageArmor | DispelMagic | Counterspell | MirrorImage | DisguiseSelf | RopeTrick => None,
            DetectThoughts | SeeInvisibility | ArmsOfHadar | CalmEmotions | Sleep => None,
            HideousLaughter => Some(6),
            PhantasmalForce => Some(12),
            ShockingGrasp => Some(1),
            PoisonSpray => Some(6),
            Hex | ChromaticOrb | HunterMark => Some(18),
            FireBolt | EldritchBlast | MagicMissile | ScorchingRay | GuidingBolt => Some(24),
            _ => Some(12),
        }
    }

    fn needs_healing(self) -> bool {
        matches!(
            self,
            Self::HealingWord | Self::CureWounds | Self::MassHealingWord
        )
    }
}

const SPELL_ACTIONS: [Act; 23] = [
    Act::Spell0,
    Act::Spell1,
    Act::Spell2,
    Act::Spell3,
    Act::Spell4,
    Act::Spell5,
    Act::Magic1,
    Act::Magic2,
    Act::Magic3,
    Act::Magic4,
    Act::Magic5,
    Act::Magic6,
    Act::Magic7,
    Act::Magic8,
    Act::Magic9,
    Act::Magic10,
    Act::Magic11,
    Act::Magic12,
    Act::Magic13,
    Act::Magic14,
    Act::Magic15,
    Act::Magic16,
    Act::Magic17,
];
const SPECIALS: [Act; 15] = [
    Act::Special1,
    Act::Special2,
    Act::Special3,
    Act::Special4,
    Act::Special5,
    Act::Special6,
    Act::Special7,
    Act::Special8,
    Act::Magic18,
    Act::Magic19,
    Act::Magic20,
    Act::Magic21,
    Act::Magic22,
    Act::Magic23,
    Act::Magic24,
];

fn spell(class: Class, action: Act) -> Option<Spell> {
    use Class::*;
    use Spell::*;
    match (class, action) {
        (Bard, Act::Special1) => return Some(StarryWisp),
        (Cleric, Act::Special4) | (Druid, Act::Special6) => return Some(CureWounds),
        (Druid, Act::Special4) => return Some(Shillelagh),
        (Warlock, Act::Special1) => return Some(FiendishVigor),
        (Bard, Act::Magic1) | (Wizard | Sorcerer, Act::Magic3) => return Some(Thunderwave),
        (Bard, Act::Magic2) | (Cleric | Paladin, Act::Magic8) => return Some(Command),
        (Bard, Act::Magic3) | (Sorcerer | Warlock, Act::Magic6) | (Wizard, Act::Magic4) => {
            if class == Warlock {
                return Some(MistyStep);
            }
            return Some(TrueStrike);
        }
        (Bard | Warlock, Act::Magic4)
        | (Cleric, Act::Magic9)
        | (Sorcerer, Act::Magic5)
        | (Druid, Act::Magic3) => return Some(HoldPerson),
        (Bard, Act::Magic5) | (Cleric, Act::Magic11) | (Druid, Act::Magic8) => {
            return Some(DispelMagic)
        }
        (Druid | Ranger, Act::Magic1) => return Some(Longstrider),
        (Druid | Ranger, Act::Magic2) => return Some(Entangle),
        (Druid, Act::Magic4) => return Some(ProtectionFromEnergy),
        (Druid, Act::Magic7) | (Cleric, Act::Magic6) => return Some(Resistance),
        (Cleric, Act::Magic5) => return Some(Guidance),
        (Cleric, Act::Magic7) | (Sorcerer, Act::Magic2) => return Some(Light),
        (Cleric, Act::Magic10) => return Some(Sanctuary),
        (Paladin, Act::Magic9) => return Some(Heroism),
        (Paladin, Act::Magic10) | (Ranger, Act::Magic5) => return Some(MagicWeapon),
        (Ranger, Act::Magic3) => return Some(Jump),
        (Ranger, Act::Magic4) => return Some(EnsnaringStrike),
        (Sorcerer | Wizard, Act::Magic1) => return Some(ShockingGrasp),
        (Sorcerer, Act::Magic4) => return Some(BurningHands),
        (Sorcerer, Act::Magic13) | (Wizard, Act::Magic5) => return Some(Shatter),
        (Sorcerer, Act::Magic14) | (Warlock, Act::Magic7) => return Some(Counterspell),
        (Warlock, Act::Magic1) => return Some(PoisonSpray),
        (Warlock, Act::Magic2) => return Some(Bane),
        (Warlock, Act::Magic3) => return Some(ExpeditiousRetreat),
        (Warlock, Act::Magic5) => return Some(TrueStrike),
        (Wizard, Act::Magic2) => return Some(MageArmor),
        (Wizard, Act::Magic6) => return Some(MistyStep),
        (Cleric | Paladin, Act::Magic1) => return Some(Aid),
        (Cleric, Act::Magic2) => return Some(LesserRestoration),
        (Cleric, Act::Magic3) => return Some(MassHealingWord),
        (Cleric, Act::Magic4) => return Some(Revivify),
        (Druid, Act::Magic5) => return Some(Blur),
        (Druid, Act::Magic6) => return Some(FireBolt),
        (Sorcerer, Act::Magic7) => return Some(AlterSelf),
        (Sorcerer | Warlock, Act::Magic8) => return Some(Command),
        (Sorcerer, Act::Magic9) => return Some(DragonBreath),
        (Sorcerer, Act::Magic11) => return Some(Fear),
        (Sorcerer, Act::Magic12) => return Some(Fly),
        (Warlock, Act::Magic13) => return Some(Suggestion),
        (Warlock, Act::Magic14) => return Some(StinkingCloud),
        (Paladin, Act::Magic15) => return Some(Protection),
        (Cleric | Paladin, Act::Magic16) => return Some(ShieldOfFaith),
        (Paladin, Act::Magic17) => return Some(ZoneOfTruth),
        _ => {}
    }
    let index = SPELL_ACTIONS
        .iter()
        .position(|&candidate| candidate == action)?;
    let equipped: &[Spell] = match class {
        Bard => &[
            ViciousMockery,
            DissonantWhispers,
            HealingWord,
            Shatter,
            HypnoticPattern,
            CureWounds,
        ],
        Cleric => &[
            SacredFlame,
            GuidingBolt,
            HealingWord,
            SpiritualWeapon,
            SpiritGuardians,
            Bless,
        ],
        Druid => &[
            StarryWisp,
            Thunderwave,
            HealingWord,
            Moonbeam,
            Fireball,
            BurningHands,
        ],
        Sorcerer => &[
            FireBolt,
            ChromaticOrb,
            MagicMissile,
            ScorchingRay,
            Fireball,
            RayOfFrost,
        ],
        Warlock => &[EldritchBlast, Hex, BurningHands, ScorchingRay, Fireball],
        Wizard => &[
            RayOfFrost,
            MagicMissile,
            BurningHands,
            ScorchingRay,
            Fireball,
            FireBolt,
        ],
        Paladin => &[Bless, DivineFavor, CureWounds],
        Ranger => &[HunterMark, CureWounds],
        _ => &[],
    };
    equipped.get(index).copied()
}

pub fn label(class: Class, action: Act) -> Option<&'static str> {
    use Act::*;
    use Class::*;
    if let Some(spell) = spell(class, action) {
        return Some(spell.label());
    }
    Some(match (class, action) {
        (Bard, Defend) => "Cutting Words",
        (Bard, Special2) => "Font of Inspiration",
        (Cleric, Special1) => "Divine Spark",
        (Cleric, Special2) => "Preserve Life",
        (Cleric, Special3) => "Strike with Spiritual Weapon",
        (Druid, Martial6) => "Black Bear Multiattack (Rend twice)",
        (Druid, Special1) => "Wild Shape",
        (Druid, Special2) => "Leave Wild Shape",
        (Druid, Special3) => "Land's Aid",
        (Druid | Paladin, Special5) => "Move Moonbeam",
        (Druid, Special7) => "Wild Resurgence",
        (Sorcerer, Special1) => "Innate Sorcery",
        (Sorcerer, Special2) => "Heightened Fireball",
        (Sorcerer, Special3) => "Quickened Fire Bolt",
        (Sorcerer, Special4) => "Create Spell Slot",
        (Sorcerer, Special5) => "Convert Slot to Sorcery Points",
        (Sorcerer | Wizard, Defend) => "Shield",
        (Warlock, Special1) => "Fiendish Vigor",
        (Warlock, Magic23) => "Dash with Expeditious Retreat",
        (Druid, Magic24) => "Choose Shillelagh ability and damage type",
        (Sorcerer, Magic10) => "Exhale Dragon's Breath",
        (Bard, Magic18) => "Bardic Inspiration",
        (Cleric, Magic19) => "Turn Undead",
        (Cleric, Magic20) => "Heal with Divine Spark",
        (Warlock, Magic21) => "Magical Cunning",
        (Sorcerer, Magic22) => "Cast with Metamagic",
        (Sorcerer, Magic23) => "Change Alter Self Form",
        (Warlock | Ranger, Magic24) => "Move Mark to a New Target",
        (_, EscapeAthletics) => "Break Free of Entanglement",
        (_, Entity11) => "Free a creature from Ensnaring Strike",
        (Bard | Cleric | Druid | Sorcerer | Wizard | Paladin | Ranger, Special8) => {
            "Select Higher Spell Slot"
        }
        _ => return None,
    })
}

impl Battle {
    pub(super) fn init_magic(&mut self, i: usize) {
        let fighter = &mut self.f[i];
        let (level, casting) = (fighter.build.level, fighter.build.casting);
        match fighter.build.class {
            Class::Bard => fighter.resource = casting.max(1),
            Class::Cleric | Class::Druid if level >= 2 => fighter.resource = 2,
            Class::Sorcerer => {
                fighter.resource = if level >= 2 { level } else { 0 };
                fighter.secondary = 2;
            }
            Class::Ranger => fighter.secondary = if level >= 5 { 3 } else { 2 },
            _ => {}
        }
    }

    fn magic_visible(&self, i: usize, range: i32) -> bool {
        self.rope_same_space(i, 1 - i)
            && self.divine_spell_origin(i).dist(self.fighter_point(1 - i)) <= range
            && self.can_see(i, 1 - i)
    }

    pub(super) fn prepared_spell(&self, i: usize, action: Act) -> Option<Spell> {
        let build = self.f[i].build;
        if build.is_subclass(Class::Monk, 2) && build.level >= 3 && action == Act::Magic9 {
            return Some(Spell::Elementalism);
        }
        if build.is_subclass(Class::Druid, 1) && matches!(action, Act::Magic9 | Act::Magic10)
            && !self.star_map_held(i)
        {
            return None;
        }
        divine_spells::prepared(build, action).unwrap_or_else(|| {
            super::bard_ranger::prepared_spell(build, action).or_else(|| {
                super::magic_arcane::prepared(build, action)
                    .unwrap_or_else(|| spell(build.class, action))
            })
        })
    }

    fn spell_slot(&self, i: usize, spell: Spell, floor: usize) -> Option<usize> {
        if spell.level() == 0
            || (spell == Spell::MistyStep && self.f[i].arcane.fey_steps > 0)
            || spell == Spell::FiendishVigor
            || (spell == Spell::HunterMark
                && self.f[i].secondary > 0
                && !self.f[i].magic.force_mark_slot)
        {
            return Some(0);
        }
        (spell.level().max(floor)..=3).find(|&tier| self.can_slot(i, tier))
    }

    pub(super) fn slot_for_action(&self, i: usize, action: Act) -> Option<usize> {
        if self.f[i].build.class == Class::Sorcerer && action == Act::Special2 {
            return self.can_slot(i, 3).then_some(3);
        }
        self.spell_slot(
            i,
            self.prepared_spell(i, action)?,
            self.f[i].magic.slot_floor,
        )
    }

    pub(super) fn magic_is_wolf(&self, i: usize) -> bool {
        self.f[i].magic.form
    }

    /// Read-only spellbook for a fixed viewer, including the first applicable
    /// preparation constraint. The decision mask remains authoritative.
    pub(super) fn spellbook(&self, i: usize) -> String {
        let f = &self.f[i];
        let mut entries = Vec::new();
        for &act in &ACTS {
            let shield = act == Act::Defend && matches!(f.build.class, Class::Sorcerer | Class::Wizard);
            let Some(s) = self.prepared_spell(i, act).or(shield.then_some(Spell::Shield)) else { continue };
            let unlock = self.spell_unlock_level(i, s);
            let timing = match s {
                Spell::Shield | Spell::Counterspell => "reaction",
                Spell::EnsnaringStrike => "on_hit",
                Spell::Alarm => "action",
                _ if s.bonus() => "bonus",
                _ => "action",
            };
            let reason = if f.build.level < unlock { "level" }
                else if timing == "reaction" || timing == "on_hit" { timing }
                else if f.magic.form || f.raging { "form" }
                else if !self.magic_components(i, s) { "components" }
                else if s.bonus() && !self.bonus { "bonus_spent" }
                else if !s.bonus() && !self.action { "action_spent" }
                else if self.spell_slot(i, s, f.magic.slot_floor).is_none() {
                    if f.slot_turn == self.turns { "slot_spent" } else { "slot" }
                } else { "conditions" };
            entries.push(format!("{}\t{}\t{}\t{}\t{}\t{}", act as usize, s.label(), s.level(), timing, unlock, reason));
        }
        for (name, rank, unlock, reason) in divine_spells::unavailable(f.build) {
            entries.push(format!("-1\t{name}\t{rank}\tutility\t{unlock}\t{reason}"));
        }
        if f.build.is_subclass(Class::Druid, 1) && !self.star_map_held(i) {
            entries.push("-1\tGuidance\t0\taction\t3\tstar_map_not_held".into());
            entries.push("-1\tGuiding Bolt\t1\taction\t3\tstar_map_not_held".into());
        }
        entries.join("\n")
    }

    pub(in crate::arena2) fn divine_free_bolt_ready(&self, i: usize) -> bool {
        self.star_map_held(i) && !self.f[i].raging && self.magic_components(i, Spell::GuidingBolt)
    }

    pub(in crate::arena2) fn divine_cast_free_bolt(&mut self, i: usize) {
        self.f[i].magic.cast_tier = Some(0);
        self.cast(i, Spell::GuidingBolt, false, false);
        self.f[i].magic.cast_tier = None;
    }

    pub(in crate::arena2) fn divine_archer_attack(&mut self, i: usize, target: usize) {
        self.f[i].magic.attack = Some(Spell::SacredFlame);
        self.f[i].magic.attack_target = Some(target);
        self.spell_attack_target(i, target, 8, 1, self.f[i].build.casting, true);
    }

    fn magic_spell_ready(&self, i: usize, spell: Spell) -> bool {
        self.magic_spell_ready_as(i, spell, spell.bonus())
    }

    fn magic_spell_ready_as(&self, i: usize, spell: Spell, as_bonus: bool) -> bool {
        use Spell::*;
        let f = &self.f[i];
        if f.magic.form
            || f.raging
            || !self.magic_components(i, spell)
            || as_bonus && !self.bonus
            || !as_bonus && !self.action
            || (spell.chooses_creature()
                || spell.is_group_targeted()
                || spell.needs_healing() && !spell.is_ally_support())
                && self.creature_choices(i, spell) == 0
            || !spell.chooses_creature()
                && !spell.is_group_targeted()
                && !spell.is_projectile()
                && spell
                    .range()
                    .is_some_and(|range| !self.magic_visible(i, range))
        {
            return false;
        }
        if (spell.is_ally_support() && self.magic_ally_candidates(i, spell) == 0)
            || f.build.level < self.spell_unlock_level(i, spell)
        {
            return false;
        }
        match spell {
            Counterspell | EnsnaringStrike | Shield => false,
            RopeTrick => self.rope_trick_castable(i),
            FiendishVigor if f.build.level < 2 => false,

            TrueStrike if self.true_strike_weapon_mask(i) == 0 => false,
            ShockingGrasp | PoisonSpray
                if self.direct_attack_spell_mask(i, spell, spell.range().unwrap_or(1)) == 0 =>
            {
                false
            }
            MagicWeapon if self.enchantment_choices(i) == 0 => false,
            Aid if f.build.class == Class::Paladin && f.build.level < 5 => false,
            Aid | LesserRestoration | Blur | AlterSelf | DragonBreath | Suggestion | Protection
                if f.build.level < 3 =>
            {
                false
            }
            FireBolt if f.build.class == Class::Druid && f.build.level < 3 => false,
            Revivify => false,
            MagicMissile if f.build.class == Class::Sorcerer && f.build.level == 1 => false,
            Bless | CureWounds if f.build.class == Class::Paladin && f.build.level == 1 => false,
            BurningHands
                if matches!(f.build.class, Class::Druid | Class::Warlock) && f.build.level < 3 =>
            {
                false
            }
            Fireball if f.build.class == Class::Druid && f.build.level < 5 => false,
            Shillelagh
                if self.gear_weapon_kind(i, false)
                    != Some(super::weapon::WeaponKind::Quarterstaff)
                    || !self.gear_wielded(i, false) =>
            {
                false
            }
            _ => true,
        }
    }

    pub(super) fn rope_cast_mask(&self, i: usize) -> u128 {
        use Spell::*;
        self.magic_mask(i) & SPELL_ACTIONS.iter().chain(&SPECIALS).copied()
            .filter(|&action| {
                self.target_spec(action).is_none()
                    && self.prepared_spell(i, action).is_some_and(|spell| {
                        spell.chooses_creature() || spell.is_group_targeted()
                            || spell.is_projectile() || spell.is_ally_support()
                            || matches!(spell, Shillelagh | DivineFavor | Blur | AlterSelf
                                | ExpeditiousRetreat | FiendishVigor | Light | SeeInvisibility
                                | MirrorImage | DisguiseSelf | Nondetection | MagicWeapon)
                    })
            })
            .fold(0, |mask, action| mask | action.bit())
    }

    fn field_can_reach(&self, i: usize, movement: i32, reach: i32) -> bool {
        self.f[i]
            .magic
            .field
            .is_some_and(|pos| pos.dist(self.fighter_point(1 - i)) <= movement + reach)
    }

    pub(super) fn magic_mask(&self, i: usize) -> u128 {
        use Act::*;
        use Class::*;
        let f = &self.f[i];
        let class = f.build.class;
        let (level, resource) = (f.build.level, f.resource);
        let floor = f.magic.slot_floor;
        let mut mask = self.arcane_action_mask(i);
        if self.phantasmal_damage_available(i) {
            mask |= PHANTASM_DAMAGE.bit();
        }
        for &act in SPELL_ACTIONS.iter().chain(&SPECIALS) {
            if self.prepared_spell(i, act).is_some_and(|spell| {
                (!spell.needs_point() || self.target_spec(act).is_some() || spell == Spell::Alarm && self.alarm_cast_in_progress(i))
                    && self.magic_spell_ready(i, spell)
                    && self.spell_slot(i, spell, floor).is_some()
            }) {
                mask |= act.bit();
            }
        }
        let has_slot = f.slots.iter().any(|&slots| slots > 0);
        let concentration = f.concentration;
        let mut allow = |condition, action: Act| {
            if condition {
                mask |= action.bit();
            }
        };
        match class {
            Bard => allow(
                level >= 5 && resource < f.build.casting.max(1) && has_slot,
                Special2,
            ),
            Cleric => {
                allow(
                    self.action && resource > 0 && self.magic_visible(i, 6),
                    Special1,
                );
                allow(
                    self.action && level >= 3 && resource > 0 && f.hp < f.build.hp / 2,
                    Special2,
                );
                allow(
                    self.bonus && concentration == Some(Concentration::SpiritualWeapon),
                    Special3,
                );
            }
            Paladin if f.build.variant == 1 => {
                allow(self.action && concentration == Some(Concentration::Moonbeam)
                    && self.field_can_reach(i, 12, 1), Special5);
            }
            Druid => {
                allow(self.bonus && concentration == Some(Concentration::GustOfWind)
                    && self.turns > f.magic.gust_cast_turn, Magic21);
                allow(
                    self.bonus && resource > 0 && !self.moonbeam_blocks_shape(i),
                    Special1,
                );
                allow(self.bonus && f.magic.form, Special2);
                allow(
                    self.wild_form(i).is_some_and(|w| w.name == "Black Bear")
                        && (self.action || f.magic.bear_next_rend)
                        && self.bear_target_mask(i) != 0,
                    Martial6,
                );
                allow(f.build.variant == 0 && self.action && level >= 3 && resource > 0, Special3);
                allow(
                    self.action
                        && concentration == Some(Concentration::Moonbeam)
                        && self.magic_visible(i, 24)
                        && self.field_can_reach(i, 12, 1),
                    Special5,
                );
                allow(
                    level >= 5
                        && ((resource == 0 && f.magic.resurgence_turn != self.turns && has_slot)
                            || (resource > 0 && !f.magic.resurgence_slot)),
                    Special7,
                );
            }
            Sorcerer => {
                allow(self.bonus && f.secondary > 0, Special1);
                allow(
                    resource >= 2
                        && self.can_slot(i, 3)
                        && self.magic_spell_ready(i, Spell::Fireball),
                    Special2,
                );
                allow(
                    resource >= 2
                        && self.bonus
                        && f.slot_turn != self.turns
                        && self.magic_spell_ready_as(i, Spell::FireBolt, true),
                    Special3,
                );
                allow(resource >= 2 && self.bonus, Special4);
                allow(level >= 2 && resource < level && has_slot, Special5);
                allow(
                    self.meta_spell_mask(i, Metamagic::Quickened) != 0
                        || self.meta_spell_mask(i, Metamagic::Heightened) != 0,
                    Magic22,
                );
            }
            Warlock => {
                allow(
                    self.bonus && concentration == Some(Concentration::ExpeditiousRetreat),
                    Magic23,
                );
                allow(
                    level >= 2
                        && self.action
                        && !f.magic.finished_rite
                        && f.slots.iter().sum::<i32>() < f.build.slots.iter().sum(),
                    Magic21,
                );
            }
            _ => {}
        }
        if class == Sorcerer {
            allow(self.action && self.area_breath_source(i).is_some(), Magic10);
            allow(
                self.action && concentration == Some(Concentration::AlterSelf),
                Magic23,
            );
        }
        allow(
            class == Bard
                && self.bonus
                && resource > 0
                && self.visible_ally(i, 12)
                && f.magic.ally_inspiration_until <= self.turns,
            Magic18,
        );
        allow(
            class == Cleric
                && self.action
                && resource > 0
                && self.visible_ally(i, 6)
                && self.entity(i + 2).is_some_and(|e| e.hp < e.kind.stats().hp),
            Magic20,
        );
        allow(
            matches!(class, Warlock | Ranger)
                && self.bonus
                && matches!(
                    concentration,
                    Some(Concentration::Hex | Concentration::HunterMark)
                )
                && f.magic.mark_defeat_turn.is_some_and(|turn| {
                    concentration == Some(Concentration::HunterMark) || turn < self.turns
                })
                && self.creature_choices(
                    i,
                    if class == Warlock {
                        Spell::Hex
                    } else {
                        Spell::HunterMark
                    },
                ) != 0,
            Magic24,
        );
        allow(
            class == Druid
                && !f.magic.form
                && self.magic_shillelagh_active(i)
                && (self.action || self.attacks > 0),
            Magic24,
        );
        allow(
            self.action && self.magic_spell_restrained(i) && self.ensnaring_escape_target(i) != Some(i),
            Act::EscapeAthletics,
        );
        allow(
            self.action && self.ensnaring_escape_target(i).is_some(),
            Act::Entity11,
        );
        let overridden = match class {
            Bard => Magic18.bit() | Special2.bit(),
            Cleric => Special1.bit() | Special2.bit() | Magic19.bit() | Magic20.bit(),
            _ => 0,
        };
        (mask & !overridden) | self.class_magic_mask(i)
            | if self.mantle_available(i) { super::bard_ranger::MANTLE.bit() } else { 0 }
            | if self.beguiling_restore_available(i) {
                super::bard_ranger::RESTORE_BEGUILING.bit()
            } else { 0 }
    }

    fn roll_many(&mut self, count: i32, die: i32) -> i32 {
        (0..count).map(|_| self.roll(die)).sum()
    }

    pub(super) fn spell_dc(&self, i: usize) -> i32 {
        let f = &self.f[i];
        8 + f.build.pb + f.build.casting + i32::from(f.magic.innate_until > self.turns)
    }

    fn spell_save_damage(
        &mut self,
        i: usize,
        stat: usize,
        dice: (i32, i32),
        bonus: i32,
        half: bool,
        kind: DamageType,
        rider: SpellRider,
        target: usize,
        ward: bool,
    ) {
        use super::battle::sanctuary::{TargetedDamage, WardRedirect};
        let range = 12;
        let damage = TargetedDamage { stat, dice, bonus, half, kind, rider, range };
        if !ward || !self.sanctuary_reject(i, target, WardRedirect::SaveSpell(damage), 0) {
            self.resolve_targeted_save_spell(i, target, damage);
        }
    }

    pub(super) fn resolve_targeted_save_spell(
        &mut self,
        i: usize,
        target: usize,
        attack: super::battle::sanctuary::TargetedDamage,
    ) {
        let super::battle::sanctuary::TargetedDamage { stat, dice, bonus, half, kind, rider, .. } = attack;
        let rider = match rider {
            SpellRider::Mockery(_) => SpellRider::Mockery(target),
            SpellRider::Whispers(_) => SpellRider::Whispers(target),
            other => other,
        };
        let dc = self.spell_dc(i);
        let saved = if target < 2 {
            self.save(target, stat, dc)
        } else {
            self.entity_save(target, stat, dc)
        };
        if saved && !half {
            return;
        }
        if !saved && matches!(rider, SpellRider::MindSliver) {
            self.mark_mind_sliver(i, target);
        }
        let damage = SpellDamage {
            source: i,
            emitter: None,
            amount: (self.roll_many(dice.0, dice.1) + bonus).max(0),
            kind,
            recipients: 1 << target,
            saved: if saved { 1 << target } else { 0 },
            checked: 0,
            rider: if saved { SpellRider::None } else { rider },
            objects: None,
        };
        self.offer_spell_damage(damage);
    }

    fn chosen_center(&self) -> Point {
        self.selected_point()
            .or_else(|| self.f.iter().find_map(|f| f.magic.resolving_center))
            .expect("area cast resolves after target selection")
    }

    pub(super) fn concentrate(
        &mut self,
        i: usize,
        effect: Concentration,
        rounds: i32,
        field: Option<Point>,
        tier: usize,
    ) {
        let f = &mut self.f[i];
        f.concentration = Some(effect);
        f.magic.concentration_heightened_target = f.magic.heightened_target;
        f.magic.expires = self.turns + rounds * 2;
        f.magic.field = field;
        f.magic.field_level = tier;
        f.magic.field_last_turn = [-1; 4];
        f.magic.field_blocks_shape = [false; 4];
        f.magic.ally_recipients = 0;
    }

    fn start_spell_attack(
        &mut self,
        i: usize,
        spell: Spell,
        die: i32,
        count: i32,
        bonus: i32,
        ranged: bool,
    ) {
        self.f[i].magic.attack = Some(spell);
        let target = self.f[i]
            .magic
            .cast_target
            .or(self.f[i].magic.attack_target)
            .unwrap_or(1 - i);
        self.f[i].magic.attack_target = Some(target);
        self.spell_attack_target(i, target, die, count, bonus, ranged);
    }

    fn burn_slot(&mut self, i: usize) -> Option<usize> {
        let f = &self.f[i];
        let tier = (f.magic.slot_floor..=3)
            .chain(1..f.magic.slot_floor)
            .find(|&tier| f.slots[tier - 1] > 0)?;
        self.f[i].slots[tier - 1] -= 1;
        self.f[i].magic.slot_floor = 1;
        Some(tier)
    }

    pub(super) fn cast_elementalism(&mut self, i: usize) {
        self.cast(i, Spell::Elementalism, false, false);
        self.notify_readied_cast(i);
    }

    fn cast(&mut self, i: usize, spell: Spell, quickened: bool, heightened: bool) {
        let slot_turn = self.f[i].slot_turn;
        let tier = self.f[i]
            .magic
            .cast_tier
            .or_else(|| self.spell_slot(i, spell, self.f[i].magic.slot_floor))
            .expect("legal spell");
        if tier == 0 && spell == Spell::MistyStep {
            self.f[i].arcane.fey_steps -= 1;
        }
        if tier > 0 {
            assert!(self.spend_slot(i, tier));
            self.f[i].magic.slot_floor = 1;
        }
        if spell.bonus() || quickened {
            self.bonus = false;
        } else {
            self.action = false;
        }
        if quickened {
            self.f[i].slot_turn = self.turns;
        }
        self.f[i].arcane.psychic = self.arcane_choose_psychic(i, spell);
        self.magic_offensive(i);
        self.magic_end_invisibility_for(i);
        let (verbal, somatic, material) = self.casting_components(i, spell);
        if verbal {
            self.reveal(i);
        }
        self.consume_material(i, material);
        if (verbal || somatic || material != super::equipment::Material::None)
            && self.magic_can_counterspell(1 - i, i)
        {
            self.magic_request(
                1 - i,
                MagicChoice::Counterspell {
                    source: i,
                    spell,
                    tier,
                    heightened,
                    heightened_target: self.f[i].magic.heightened_target,
                    center: self.selected_point().or(self.f[i].magic.resolving_center),
                    target: self.f[i].magic.cast_target,
                    cure_route: self.f[i].magic.cure_via_familiar,
                    targets: self.f[i].magic.cast_targets,
                    old_slot_turn: slot_turn,
                },
            );
        } else {
            self.resolve_spell(i, spell, if spell == Spell::GuidingBolt { tier.max(1) } else { tier }, heightened);
        }
    }

    fn resolve_spell(&mut self, i: usize, spell: Spell, tier: usize, heightened: bool) {
        self.mark_beguiling_spell(i, spell, tier);
        self.arcane_ward_cast(i, spell, tier);
        if self.magic_begin_projectile(i, spell, tier)
            || self.magic_apply_ally_spell(i, spell, tier)
        {
            return;
        }
        use Spell::*;
        let level = self.f[i].build.level;
        let casting = self.f[i].build.casting;
        let target = 1 - i;
        let tier_dice = tier as i32;
        let cantrip = if level >= 5 { 2 } else { 1 };
        match spell {
            ArmsOfHadar => self.directional_area_damage(
                i, 0, tier_dice + 1, 6, DamageType::Necrotic, heightened,
                Area::Sphere { radius: 2 }, SpellRider::ArmsOfHadar,
            ),
            HideousLaughter => self.cast_hideous_laughter(i, tier),
            PhantasmalForce => self.cast_phantasmal_force(i, tier, heightened),
            CalmEmotions => self.arcane_apply_calm(i, self.chosen_center(), tier, heightened),
            Sleep => self.cast_sleep(i, tier, heightened),
            DetectThoughts => {
                self.concentrate(i, Concentration::DetectThoughts, 20, None, tier);
                self.arcane_sense_thoughts(i);
            }
            SeeInvisibility => self.f[i].arcane.see_invisibility_until = self.turns + 1_200,
            MindSliver => {
                let target = self.f[i].magic.cast_target.unwrap_or(target);
                self.spell_save_damage(i, 3, (cantrip, 6), 0, false, DamageType::Psychic,
                    SpellRider::MindSliver, target, true);
            }
            Blink => self.cast_blink(i),
            PlantGrowth => self.cast_plant_growth(i, self.chosen_center()),
            FaerieFire => self.cast_faerie_fire(i, tier, heightened),
            HungerOfHadar => {
                self.f[i].arcane.psychic_hadar = self.f[i].arcane.psychic;
                self.concentrate(i, Concentration::HungerOfHadar, 20, Some(self.chosen_center()), tier);
            }
            ViciousMockery => {
                let target = self.f[i].magic.cast_target.unwrap_or(target);
                self.spell_save_damage(
                    i,
                    WIS,
                    (cantrip, 6),
                    0,
                    false,
                    DamageType::Psychic,
                    SpellRider::Mockery(target),
                    target,
                    true,
                );
            }
            SacredFlame => {
                let target = self.f[i].magic.cast_target.unwrap_or(target);
                self.spell_save_damage(
                    i,
                    DEX,
                    (cantrip, 8),
                    0,
                    false,
                    DamageType::Radiant,
                    SpellRider::None,
                    target,
                    true,
                );
            }
            StarryWisp | RayOfFrost | FireBolt => {
                let die = if spell == FireBolt { 10 } else { 8 };
                self.start_spell_attack(i, spell, die, cantrip, 0, true);
            }
            EldritchBlast => {
                self.f[i].magic.rays = cantrip - 1;
                self.start_spell_attack(
                    i,
                    spell,
                    10,
                    1,
                    if level >= 2 { casting } else { 0 },
                    true,
                );
            }
            DissonantWhispers => {
                let target = self.f[i].magic.cast_target.unwrap_or(target);
                self.spell_save_damage(
                    i,
                    WIS,
                    (tier_dice + 2, 6),
                    0,
                    true,
                    DamageType::Psychic,
                    SpellRider::Whispers(target),
                    target,
                    true,
                );
            }
            HealingWord | CureWounds | MassHealingWord => {
                let die = if spell == CureWounds { 8 } else { 4 };
                let life = if self.is_subclass(i, Class::Cleric, 0) && level >= 3 {
                    tier_dice + 2
                } else {
                    0
                };
                let dice = if spell == MassHealingWord {
                    tier_dice - 1
                } else {
                    2 * tier_dice
                };
                let rolled = self.roll_many(dice, die);
                let recipient = self.f[i].magic.cast_target.unwrap_or(i);
                if spell == CureWounds
                    && self.f[i]
                        .magic
                        .cure_via_familiar
                        .unwrap_or_else(|| !self.cure_paths(i, recipient).0)
                    && !self.entity_touch_reaction(i, recipient)
                {
                    return;
                }
                let restored = if recipient < 2 {
                    let before = self.f[recipient].hp;
                    self.magic_heal(recipient, rolled + casting + life);
                    self.f[recipient].hp > before
                } else {
                    let before = self.entity(recipient).map(|entity| entity.hp);
                    self.entity_heal(recipient, rolled + casting + life);
                    self.entity(recipient).map(|entity| entity.hp) > before
                };
                if spell == MassHealingWord {
                    if let Some(pos) = self.entity_position(i + 2) {
                        if self.f[i].pos.dist(pos) <= 12 && self.entity_visible(i, i + 2) {
                            self.entity_heal(i + 2, rolled + casting + life);
                        }
                    }
                }
                if restored { self.divine_chalice(i, tier); }
            }
            GuidingBolt => self.start_spell_attack(i, spell, 6, tier_dice + 3, 0, true),
            Bless => self.concentrate(i, Concentration::Bless, 10, None, tier),
            Thunderwave => {
                let area = Area::OffsetCube {
                    side: 3,
                    direction: self.f[i].magic.direction,
                    quarters: self.f[i].magic.cube_offset,
                };
                self.directional_area_damage(
                    i,
                    CON,
                    tier_dice + 1,
                    8,
                    DamageType::Thunder,
                    heightened,
                    area,
                    SpellRider::Thunderwave(self.fighter_point(i)),
                );
            }
            BurningHands => {
                let area = Area::Cone {
                    length: 3,
                    direction: self.f[i].magic.direction,
                };
                self.ignite_objects_in(self.fighter_point(i), area);
                self.directional_area_damage(
                    i,
                    DEX,
                    tier_dice + 2,
                    6,
                    DamageType::Fire,
                    heightened,
                    area,
                    SpellRider::None,
                );
            }
            ChromaticOrb => self.start_spell_attack(i, spell, 8, tier_dice + 2, 0, true),
            MagicMissile => {
                let darts = tier_dice + 2;
                let damage = self.roll_many(darts, 4) + darts;
                self.magic_missile(i, damage);
            }
            Hex => {
                self.f[i].arcane.psychic_hex = self.f[i].arcane.psychic;
                self.f[i].magic.hex_ability = self.f[i].magic.hex_pending_ability;
                let minutes = [0, 60, 240, 480][tier];
                self.f[i].magic.mark_target = Some(self.f[i].magic.cast_target.unwrap_or(target));
                self.f[i].magic.mark_defeat_turn = None;
                self.concentrate(i, Concentration::Hex, minutes * 10, None, tier);
            }
            HunterMark => {
                self.f[i].magic.mark_target = Some(self.f[i].magic.cast_target.unwrap_or(target));
                self.f[i].magic.mark_defeat_turn = None;
                if tier == 0 {
                    self.f[i].secondary -= 1;
                }
                self.concentrate(
                    i,
                    Concentration::HunterMark,
                    if tier >= 3 { 4800 } else { 600 },
                    None,
                    tier,
                );
            }
            DivineFavor => self.f[i].magic.favor_until = self.turns + 20,
            Shillelagh => {
                self.f[i].magic.shillelagh_weapon = Some(self.gear_weapon_owner(i, false));
                self.f[i].magic.shillelagh_until = self.turns + 20;
            }
            Shatter => self.spell_burst_damage(
                i,
                CON,
                (tier_dice + 1, 8),
                Area::Sphere { radius: 2 },
                heightened,
                DamageType::Thunder,
            ),
            SpiritualWeapon => {
                self.concentrate(
                    i,
                    Concentration::SpiritualWeapon,
                    10,
                    Some(self.chosen_center()),
                    tier,
                );
                self.offer_spiritual_weapon_attack(i);
            }
            Moonbeam => self.start_moonbeam(i, tier),
            ScorchingRay => {
                self.f[i].magic.rays = tier_dice;
                self.start_spell_attack(i, spell, 6, 2, 0, true);
            }
            HypnoticPattern => self.cast_hypnotic_pattern(i, tier, heightened),
            SpiritGuardians => self.start_spirit_guardians(i, tier),
            Fireball => self.spell_burst_damage(
                i,
                DEX,
                (tier_dice + 5, 6),
                Area::Sphere { radius: 4 },
                heightened,
                DamageType::Fire,
            ),
            Aid => {
                let extra = 5 * (tier_dice - 1);
                let gained = (extra - self.f[i].magic.aid_hp).max(0);
                self.f[i].magic.aid_hp += gained;
                self.f[i].hp += gained;
            }
            LesserRestoration => {
                self.f[i].magic.poisoned_turn = -1;
                self.area_cure_poison(i);
            }
            Revivify => unreachable!("no eligible dead target in the arena"),
            Blur => self.concentrate(i, Concentration::Blur, 10, None, tier),
            AlterSelf => self.concentrate(i, Concentration::AlterSelf, 600, None, tier),
            Command => {
                let dc = self.spell_dc(i);
                let word = self.f[i].magic.chosen_word;
                for target in 0..4 {
                    if self.f[i].magic.cast_targets & (1 << target) == 0 {
                        continue;
                    }
                    let saved = if target < 2 {
                        self.save_advantage(
                            target,
                            WIS,
                            dc,
                            heightened && self.magic_heightened_for(i, target),
                        )
                    } else {
                        self.entity_save(target, WIS, dc)
                    };
                    if !saved {
                        if target < 2 {
                            self.f[target].magic.word = Some((i, word));
                        } else {
                            self.issue_entity_command(i, target, compulsion_word(word));
                        }
                    }
                }
            }
            DragonBreath => {
                self.f[i].magic.breath_element = self.f[i].magic.element;
                self.concentrate(i, Concentration::DragonBreath, 10, None, tier);
                self.attach_breath(i, self.f[i].magic.cast_target.unwrap_or(i));
            }
            Fear => self.cast_fear(i, tier, heightened),
            Fly => self.concentrate(i, Concentration::Fly, 100, None, tier),
            Suggestion => {
                self.f[i].concentration = None;
                let target = self.f[i].magic.cast_target.unwrap_or(target);
                let plan = self.f[i]
                    .magic
                    .suggestion
                    .take()
                    .expect("selected suggestion");
                let dc = self.spell_dc(i);
                let saved = if target < 2 {
                    self.save_advantage(target, WIS, dc, heightened)
                } else {
                    self.entity_save(target, WIS, dc)
                };
                if !saved {
                    self.concentrate(i, Concentration::Suggestion, 4_800, None, tier);
                    self.activate_suggestion(i, target, plan);
                }
            }
            StinkingCloud => self.concentrate(
                i,
                Concentration::StinkingCloud,
                10,
                Some(self.chosen_center()),
                tier,
            ),
            Protection => self.concentrate(i, Concentration::Protection, 100, None, tier),
            ShieldOfFaith => self.concentrate(i, Concentration::ShieldOfFaith, 100, None, tier),
            ZoneOfTruth => self.place_truth_zone(i, self.chosen_center()),
            _ => self.resolve_added_spell(i, spell, tier, heightened),
        }
    }

    pub(super) fn magic_continues_cast(&self, i: usize, action: Act) -> bool {
        self.f[i].magic.metamagic.is_some_and(|(_, pending)| pending == action)
    }

    pub(super) fn magic_step(&mut self, i: usize, action: Act) {
        if action == PHANTASM_DAMAGE && self.phantasmal_damage_available(i) {
            self.phantasmal_force_damage(i);
            return;
        }
        if self.f[i].build.class == Class::Druid && action == Act::Martial6 {
            self.request(i, 102);
            return;
        }
        if action == super::bard_ranger::RESTORE_BEGUILING && self.beguiling_restore_available(i) {
            self.restore_beguiling(i);
            return;
        }
        if action == super::bard_ranger::MANTLE && self.mantle_available(i) {
            self.start_mantle(i);
            return;
        }
        if self.arcane_action_step(i, action) || self.class_magic_step(i, action) {
            return;
        }
        use Act::*;
        use Spell::*;
        if action == Entity11 && self.ensnaring_escape_targets(i).count_ones() > 1 {
            self.magic_request(i,MagicChoice::Creature(action));
            return;
        }
        let prepared = self.prepared_spell(i, action);
        if prepared == Some(Elementalism) {
            self.begin_elementalism(i, self.selected_point().expect("aimed Elementalism"), false);
            return;
        }
        let choice = match (prepared, action) {
            (None, Magic21) if self.is_subclass(i, Class::Druid, 2) => Some(MagicChoice::Direction(action)),
            (Some(Thunderwave | BurningHands | Fear | GustOfWind | LightningBolt), _) => Some(MagicChoice::Direction(action)),
            (None, Magic10) if self.f[i].build.class == Class::Sorcerer => {
                Some(MagicChoice::Direction(action))
            }
            (Some(ChromaticOrb | DragonBreath | Resistance | ProtectionFromEnergy), _) => {
                Some(MagicChoice::Element(action))
            }
            (Some(AlterSelf), _) => Some(MagicChoice::Form(action)),
            (_, Magic23) if self.f[i].build.class == Class::Sorcerer => {
                Some(MagicChoice::Form(action))
            }
            (Some(Command), _) => Some(MagicChoice::Command(action)),
            (Some(Guidance), _) => Some(MagicChoice::Guidance(action)),
            (Some(EnhanceAbility), _) => Some(MagicChoice::EnhanceAbility(action)),
            (Some(Light), _) => Some(MagicChoice::LightTarget(action)),
            (Some(TrueStrike), _) => Some(MagicChoice::TrueStrikeWeapon(action)),
            (Some(MagicWeapon), _) => Some(MagicChoice::WeaponEnchantment(action)),
            (Some(Shillelagh), _) | (_, Magic24) if self.f[i].build.class == Class::Druid => {
                Some(MagicChoice::ShillelaghType(action))
            }
            (_, Special1) if self.f[i].build.class == Class::Cleric => {
                Some(MagicChoice::DivineSparkType)
            }
            (_, Special2) if self.f[i].build.class == Class::Bard => Some(MagicChoice::RecoverBard),
            (_, Special7) if self.f[i].build.class == Class::Druid && self.f[i].resource == 0 => {
                Some(MagicChoice::RecoverDruid)
            }
            (Some(DispelMagic), _) => Some(MagicChoice::Dispel(action)),
            (Some(Hex), _) => Some(MagicChoice::HexAbility),
            (Some(HunterMark), _)
                if self.f[i].secondary > 0 && (1..=3).any(|tier| self.can_slot(i, tier)) =>
            {
                Some(MagicChoice::MarkCost)
            }
            (_, Magic22) if self.f[i].build.class == Class::Sorcerer => Some(MagicChoice::MetaMode),
            (_, Magic24) => Some(MagicChoice::Creature(action)),
            (_, Special1) if self.f[i].build.class == Class::Druid => Some(MagicChoice::WildShape),
            (_, Special4) if self.f[i].build.class == Class::Sorcerer => {
                Some(MagicChoice::CreateSlot)
            }
            (_, Special5) if self.f[i].build.class == Class::Sorcerer => {
                Some(MagicChoice::ConvertSlot)
            }
            _ => None,
        };
        if let Some(choice) = choice {
            self.magic_request(i, choice);
        } else {
            self.execute_magic_step(i, action);
        }
    }

    fn execute_magic_step(&mut self, i: usize, action: Act) {
        use Act::*;
        use Class::*;
        let class = self.f[i].build.class;
        if action == Act::Entity11 {
            self.action = false;
            if let Some(target) = self.ensnaring_escape_target(i) {
                self.area_escape_restraint_as(i, target);
            }
            return;
        }
        if action == Act::EscapeAthletics {
            self.action = false;
            let modifier = self.magic_save_modifier(i, 0)
                + super::origins::skill_bonus(self.f[i].build, super::origins::Skill::Athletics, 0);
            let disadvantage = self.magic_ability_disadvantage(i, 0)
                || self.armor_disadvantages_ability(i, 0);
            let roll = self.d20_for(i, false, disadvantage);
            if roll + modifier >= self.spell_dc(1 - i) {
                self.f[i].magic.restrained = false;
                self.area_release_restraint(i);
            }
            return;
        }
        if let Some(spell) = self.prepared_spell(i, action) {
            if spell == Spell::Alarm {
                self.alarm_work(i);
                return;
            }
            if spell.is_ally_support()
                && self.f[i].magic.cast_tier.is_none()
                && self.castable_slot_mask(i, spell).count_ones() > 1
            {
                self.magic_request(
                    i,
                    MagicChoice::CastSlot {
                        action,
                        center: None,
                    },
                );
                return;
            }
            if spell.is_ally_support() && self.f[i].magic.ally_selection == 0 {
                let candidates = self.magic_ally_candidates(i, spell);
                if candidates != 1 << i {
                    self.magic_request(i, MagicChoice::Allies(action));
                    return;
                }
            }
            if spell.chooses_creature() && self.f[i].magic.cast_target.is_none() {
                let mask = self.creature_choices(i, spell);
                let normal = if spell.needs_healing() { i } else { 1 - i };
                if mask != 0 && mask != ACTS[normal].bit() {
                    self.magic_request(i, MagicChoice::Creature(action));
                    return;
                }
            }
            if spell == Spell::CureWounds && self.f[i].magic.cure_via_familiar.is_none() {
                let (direct, familiar) =
                    self.cure_paths(i, self.f[i].magic.cast_target.unwrap_or(i));
                if direct && familiar {
                    self.magic_request(i, MagicChoice::CureRoute(action));
                    return;
                }
                self.f[i].magic.cure_via_familiar = Some(familiar && !direct);
            }
            if spell == Spell::Suggestion && self.f[i].magic.suggestion.is_none() {
                self.start_suggestion_cast(i, self.f[i].magic.cast_target.unwrap_or(1 - i));
                return;
            }
            let tier_mask = self.castable_slot_mask(i, spell);
            if self.f[i].magic.cast_tier.is_none() && tier_mask.count_ones() > 1 {
                self.magic_request(
                    i,
                    MagicChoice::CastSlot {
                        action,
                        center: self.selected_point().or(self.f[i].magic.resolving_center),
                    },
                );
                return;
            }
            if spell.is_group_targeted() && self.f[i].magic.cast_targets == 0 {
                self.magic_request(i, MagicChoice::GroupTargets(action));
                return;
            }
            if matches!(self.f[i].magic.metamagic,Some((Metamagic::Heightened,a)) if a==action)
                && self.f[i].magic.heightened_target.is_none()
            {
                let candidates = self.heightened_candidates(i, spell);
                if candidates.count_ones() > 1 {
                    self.magic_request(
                        i,
                        MagicChoice::HeightenedTarget {
                            action,
                            candidates,
                            center: self.selected_point().or(self.f[i].magic.resolving_center),
                        },
                    );
                    return;
                }
                if candidates == 0 {
                    self.f[i].magic.metamagic = None;
                } else {
                    self.f[i].magic.heightened_target = Some(candidates.trailing_zeros() as usize);
                }
            }
            let meta = self.f[i]
                .magic
                .metamagic
                .take()
                .filter(|&(_, a)| a == action)
                .map(|(mode, _)| mode);
            if meta.is_some() {
                self.f[i].resource -= 2;
            }
            self.cast(
                i,
                spell,
                matches!(meta, Some(Metamagic::Quickened)),
                matches!(meta, Some(Metamagic::Heightened)),
            );
            self.f[i].magic.cast_target = None;
            self.f[i].magic.cure_via_familiar = None;
            self.f[i].magic.cast_targets = 0;
            self.f[i].magic.force_mark_slot = false;
            self.f[i].magic.cast_tier = None;
            self.f[i].magic.heightened_target = None;
            return;
        }
        match (class, action) {
            (Bard, Special2) => {
                self.burn_slot(i)
                    .expect("Font of Inspiration requires a slot");
                self.f[i].resource += 1;
            }
            (Druid, Magic24) => {}
            (Warlock, Magic23) => {
                self.bonus = false;
                self.dash += self.speed(i);
            }
            (Cleric, Special1) => {
                self.action = false;
                self.f[i].resource -= 1;
                self.spell_save_damage(
                    i,
                    CON,
                    (1, 8),
                    self.f[i].build.casting,
                    true,
                    self.f[i].magic.divine_spark_kind,
                    SpellRider::None,
                    1 - i,
                    false,
                );
            }
            (Cleric, Special2) => {
                self.action = false;
                self.f[i].resource -= 1;
                self.heal(
                    i,
                    (self.f[i].build.hp / 2 - self.f[i].hp).min(5 * self.f[i].build.level),
                );
            }
            (Cleric, Special3) => {
                self.bonus = false;
                self.f[i].magic.field = Some(self.chosen_center());
                self.offer_spiritual_weapon_attack(i);
            }
            (Druid, Special1) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].magic.form = true;
                self.grant_temporary_hp(i, self.f[i].build.level, TempSource::WildShape);
            }
            (Druid, Special2) => {
                self.bonus = false;
                self.magic_end_wild_shape(i);
            }
            (Druid, Special3) if self.f[i].build.variant == 0 => {
                self.action = false;
                self.f[i].resource -= 1;
                self.start_lands_aid(i);
            }
            (Druid | Paladin, Special5) => {
                self.action = false;
                self.move_moonbeam(i);
            }
            (Druid, Special7) => {
                if self.f[i].resource == 0 {
                    self.burn_slot(i).expect("Wild Resurgence requires a slot");
                    self.f[i].resource = 1;
                    self.f[i].magic.resurgence_turn = self.turns;
                } else {
                    self.f[i].resource -= 1;
                    self.f[i].slots[0] += 1;
                    self.f[i].magic.resurgence_slot = true;
                }
            }
            (Sorcerer, Special1) => {
                self.bonus = false;
                self.f[i].secondary -= 1;
                self.f[i].magic.innate_until = self.turns + 20;
            }
            (Sorcerer, Special2) => {
                self.f[i].magic.metamagic = Some((Metamagic::Heightened, Spell4));
                self.execute_magic_step(i, Spell4);
            }
            (Sorcerer, Special3) => {
                self.f[i].resource -= 2;
                self.f[i].slot_turn = self.turns;
                self.cast(i, Spell::FireBolt, true, false);
            }
            (Sorcerer, Special4) => {
                self.bonus = false;
                self.f[i].resource -= 2;
                self.f[i].slots[0] += 1;
            }
            (Sorcerer, Special5) => {
                let tier = self.burn_slot(i).expect("Font of Magic requires a slot");
                self.f[i].resource = (self.f[i].resource + tier as i32).min(self.f[i].build.level);
            }
            (Bard, Magic18) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].magic.ally_inspiration_until = self.turns + 1_200;
            }
            (Cleric, Magic20) => {
                self.action = false;
                self.f[i].resource -= 1;
                let amount = self.roll(8) + self.f[i].build.casting;
                self.entity_heal(i + 2, amount);
            }
            (Warlock, Magic21) => self.perform_pact_rite(i),
            (Sorcerer, Magic23) => self.action = false,
            (Sorcerer, Magic10) => {
                self.action = false;
                self.area_exhale(i, self.f[i].magic.direction);
            }
            _ => unreachable!("unknown magic action {action:?} for {class:?}"),
        }
    }

    pub(super) fn magic_field(&self, i: usize) -> Option<Point> {
        self.f[i].concentration.and(self.f[i].magic.field)
    }

    fn spiritual_recipients(&self, i: usize) -> u128 {
        let Some(origin) = self.magic_field(i) else {
            return 0;
        };
        let observed = (0..4)
            .filter(|&id| {
                !self.suggestion_charmed_by(i, id)
                    && self.observed_footprint(i, id).is_some_and(|footprint| {
                        footprint
                            .squares()
                            .any(|point| origin.dist(point) <= 1 && self.map.los_at(origin, point))
                    })
            })
            .fold(0, |mask, id| mask | ACTS[id].bit());
        // Enable the same guesses when an opposing companion is unseen or
        // absent. The menu never confirms whether a guessed cell is occupied.
        if self.observed_pos(i, 1 - i).is_some() && self.observed_entity(i, 3 - i).is_some() {
            return observed;
        }
        (16..43)
            .filter(|&index| self.spiritual_guess(i, index).is_some())
            .fold(observed, |mask, index| mask | ACTS[index].bit())
    }

    fn spiritual_guess(&self, caster: usize, index: usize) -> Option<Point> {
        let offset = index.checked_sub(16).filter(|&n| n < 27)? as i8;
        let origin = self.magic_field(caster)?;
        let point = Point {
            cell: super::geometry::Pos {
                x: origin.cell.x + offset % 3 - 1,
                y: origin.cell.y + (offset / 3) % 3 - 1,
            },
            altitude: origin.altitude + offset / 9 - 1,
        };
        (self.map.can_aim_point(origin, point)
            && (0..4).all(|id| {
                self.observed_footprint(caster, id)
                    .is_none_or(|footprint| footprint.squares().all(|pos| pos != point))
            }))
        .then_some(point)
    }

    fn offer_spiritual_weapon_attack(&mut self, i: usize) {
        if self.spiritual_recipients(i) != 0 {
            self.magic_request(i, MagicChoice::SpiritualStrike);
        }
    }

    pub(super) fn magic_attack_source(&self, i: usize) -> Option<Point> {
        match self.f[i].magic.attack {
            Some(Spell::SpiritualWeapon) => self.magic_field(i),
            Some(_) => self.divine_cast_origin(i),
            None => None,
        }
    }

    fn magic_push(&mut self, i: usize, feet_cells: i32) {
        let target = 1 - i;
        let from = self.f[i].pos;
        let start = self.f[target].pos;
        let away = ((start.x - from.x).signum(), (start.y - from.y).signum());
        let directions = [
            (0, -1),
            (1, -1),
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
        ];
        let Some(dir) = directions.iter().position(|&offset| offset == away) else {
            return;
        };
        for _ in 0..feet_cells {
            let Some((next, _)) = self.map.step(self.f[target].pos, dir) else {
                break;
            };
            let previous = self.fighter_point(target);
            self.f[target].pos = next;
            self.martial_moved(target);
            self.magic_moved(target, previous);
            if self.f[target].hp == 0 {
                return;
            }
        }
    }

    pub(super) fn magic_defend(&self, i: usize, hit: Hit) -> bool {
        if !self.can_react(i) || self.f[i].magic.form {
            return false;
        }
        match self.f[i].build.class {
            Class::Sorcerer | Class::Wizard => {
                let new_bonus = self.f[i].shield == 0
                    && (1..=3).any(|slot| self.can_slot(i, slot))
                    && self.can_cast_components(i, true, true, super::equipment::Material::None);
                new_bonus && (!hit.weapon || hit.d20 != 0)
            }
            Class::Bard => {
                self.f[i].build.variant == 0
                    && self.f[i].build.level >= 3
                    && self.f[i].resource > 0
                    && !hit.crit
                    && hit.d20 != 0
                    && hit.total < hit.ac + if self.f[i].build.level >= 5 { 8 } else { 6 }
                    && self.magic_visible(i, 12)
            }
            _ => false,
        }
    }

    pub(super) fn magic_defense(&mut self, i: usize, hit: Hit) -> bool {
        self.f[i].reaction = false;
        if self.f[i].build.class == Class::Bard {
            self.f[i].resource -= 1;
            return hit.total - self.roll(if self.f[i].build.level >= 5 { 8 } else { 6 }) < hit.ac;
        }
        if let Some(slot) = (1..=3).find(|&slot| self.can_slot(i, slot)) {
            assert!(self.spend_slot(i, slot));
            self.arcane_ward_cast(i, Spell::Shield, slot);
            self.f[i].shield = 5;
            return hit.d20 == 0 || !hit.crit && hit.total < hit.ac + 5;
        }
        false
    }

    #[cfg(test)]
    pub(super) fn magic_damage(&mut self, i: usize, hit: Hit) -> i32 {
        let mut parts = self.magic_damage_parts(i, hit);
        self.damage_parts_after_resistance(hit.defender, &mut parts)
    }

    pub(super) fn magic_damage_parts(&mut self, i: usize, hit: Hit) -> [(i32, DamageType); 4] {
        let f = &self.f[i];
        let copies = if hit.crit { 2 } else { 1 };
        let mark = self.magic_mark_target(i) == Some(hit.defender)
            && matches!(
                f.concentration,
                Some(Concentration::Hex | Concentration::HunterMark)
            );

        let spider = hit.weapon && self.wild_form(i).is_some_and(|form| form.name == "Spider");
        if !hit.weapon && hit.defender < 2 {
            match f.magic.attack {
                _ => {}
            }
        }
        if !hit.weapon && self.f[i].magic.attack == Some(Spell::ShockingGrasp) && hit.defender < 4 {
            self.f[i].magic.shocking_targets |= 1 << hit.defender;
        }
        let mut parts = [(0, DamageType::Force); 4];
        if mark && hit.d20 != 0 {
            let amount = self.roll_many(copies, 6);
            let kind = if self.f[i].concentration == Some(Concentration::Hex) {
                if self.f[i].arcane.psychic_hex { DamageType::Psychic } else { DamageType::Necrotic }
            } else {
                DamageType::Force
            };
            parts[0] = (amount, kind);
        }
        let favor = self.magic_divine_favor_damage(i, hit.crit, hit.weapon && !hit.unarmed);
        parts[1] = (favor, DamageType::Radiant);
        if spider {
            let amount = self.roll_many(copies, 4);
            parts[2] = (amount, DamageType::Poison);
        }
        let amount = self.magic_true_strike_extra(i, hit.crit);
        if amount != 0 {
            let kind = self.arcane_damage_kind(i, DamageType::Radiant);
            parts[3] = (amount, kind);
        }
        parts
    }

    pub(super) fn arcane_weapon_spell(&self, i: usize) -> bool {
        self.f[i].magic.attack == Some(Spell::TrueStrike)
    }

    pub(super) fn magic_true_strike_extra(&mut self, i: usize, critical: bool) -> i32 {
        let f = &self.f[i];
        if f.magic.attack == Some(Spell::TrueStrike) && f.build.level >= 5 {
            self.roll_many(if critical { 2 } else { 1 }, 6)
        } else {
            0
        }
    }

    pub(in crate::arena2) fn magic_divine_favor_damage(
        &mut self,
        i: usize,
        critical: bool,
        weapon: bool,
    ) -> i32 {
        if weapon && self.f[i].magic.favor_until > self.turns {
            self.roll_many(1 + i32::from(critical), 4)
        } else {
            0
        }
    }

    pub(super) fn magic_roll_bonus(&mut self, i: usize) -> i32 {
        let bonus = if self.magic_blessed(i) {
            self.roll(4)
        } else {
            0
        };
        let penalty = self.magic_bane_penalty(i);
        bonus - penalty
    }

    pub(super) fn magic_speed(&self, i: usize) -> i32 {
        let f = &self.f[i];
        let mut speed = self.wild_form(i).map_or(f.build.speed, |form| form.speed);
        if f.magic.longstrider_until > self.turns {
            speed += 2;
        }
        if self.magic_restrained(i) || self.magic_paralyzed(i) {
            return 0;
        }
        self.magic_area_speed(i, speed)
    }

    pub(super) fn magic_ac(&self, i: usize) -> Option<i32> {
        self.wild_form(i).map(|form| form.ac).or_else(|| {
            (self.f[i].magic.mage_armor_until > self.turns && !self.armor_worn(i))
                .then(|| 13 + self.f[i].build.stats[DEX])
        })
    }

    pub(super) fn magic_save_modifier(&self, i: usize, stat: usize) -> i32 {
        let f = &self.f[i];
        if let Some(form) = self.wild_form(i).filter(|_| stat <= CON) {
            form.physical[stat]
        } else {
            f.build.stats[stat]
        }
    }

    pub(super) fn magic_weapon_attack_bonus(&self, i: usize, ranged: bool) -> Option<i32> {
        self.wild_form(i)
            .filter(|_| !ranged)
            .map(|form| form.attack_bonus)
    }

    pub(super) fn magic_weapon(&self, i: usize, ranged: bool) -> Option<(i32, i32)> {
        let f = &self.f[i];
        if ranged {
            return f.magic.form.then_some((0, 0));
        }
        if let Some(form) = self.wild_form(i) {
            return Some((form.die, form.damage_bonus));
        }
        if self.magic_shillelagh_active(i) {
            let ability = if f.magic.shillelagh_strength {
                f.build.stats[0]
            } else {
                f.build.casting
            };
            return Some((if f.build.level >= 5 { 10 } else { 8 }, ability));
        }
        None
    }

    pub(super) fn magic_attack_advantage(&mut self, i: usize, weapon: bool) -> bool {
        let state = &mut self.f[i].magic;
        let guided = state.guided_until > 0 && state.guided_until >= self.turns;
        state.guided_until = 0;
        guided || (!weapon || state.attack == Some(Spell::TrueStrike)) && state.innate_until > self.turns
    }

    pub(super) fn magic_incap(&self, i: usize) -> bool {
        self.magic_area_incapacitated(i) || self.magic_paralyzed(i)
    }

    pub(super) fn magic_hurt(&mut self, i: usize) {
        self.arcane_damage_ends_indifference(i);
        self.magic_area_hurt(i);
        if self.f[i].stunned {
            self.f[i].magic.awaiting_rite = None;
        }
        if self.f[i].stunned {
            self.magic_end_wild_shape(i);
        }
    }

    pub(super) fn magic_miss(&mut self, hit: Hit, die: i32, count: i32, bonus: i32) {
        let i = hit.attacker;
        let f = &self.f[i];
        if self.is_subclass(i, Class::Wizard, 0) && f.build.level >= 3
            && f.magic.attack.is_some_and(|spell| {
                spell.level() == 0 && (!hit.weapon || spell == Spell::TrueStrike)
            })
        {
            let kind = self
                .magic_attack_kind(i, hit.weapon)
                .unwrap_or(hit.damage_kind);
            let base = (self.roll_many(count, die) + bonus).max(0);
            let radiant = self.magic_true_strike_extra(i, false);
            if kind == DamageType::Radiant {
                self.apply_spell_damage(i, hit.defender, (base + radiant) / 2, kind);
            } else {
                self.apply_spell_damage(i, hit.defender, base / 2, kind);
                let radiant_half = (base + radiant) / 2 - base / 2;
                if radiant_half > 0 {
                    self.apply_spell_damage(i, hit.defender, radiant_half, DamageType::Radiant);
                }
            }
        }
    }

    pub(super) fn magic_continue(&mut self, i: usize) {
        if self.magic_continue_projectile(i) {
            return;
        }
        if self.f[1 - i].hp <= 0 || self.f[i].hp <= 0 {
            self.f[i].magic.attack = None;
            self.f[i].magic.attack_target = None;
            self.f[i].magic.rays = 0;
            return;
        }
        if self.f[i].magic.rays == 0 || !self.magic_visible(i, 24) {
            self.f[i].magic.rays = 0;
            self.f[i].magic.attack_target = None;
            self.f[i].magic.attack = None;
            return;
        }
        if self.f[i]
            .magic
            .attack_target
            .is_some_and(|target| !self.valid_mark_target(target))
        {
            self.f[i].magic.attack_target = Some(1 - i);
        }
        self.f[i].magic.rays -= 1;
        let spell = self.f[i].magic.attack.expect("additional spell ray");
        if spell == Spell::EldritchBlast {
            self.start_spell_attack(i, spell, 10, 1, self.f[i].build.casting, true);
        } else {
            self.start_spell_attack(i, spell, 6, 2, 0, true);
        }
    }

    pub(super) fn magic_end(&mut self, i: usize) {
        self.alarm_end_turn(i);
        self.f[i].magic.bear_next_rend = false;
        if self.f[i]
            .magic
            .awaiting_rite
            .is_some_and(|last| last != self.turns)
            || self.incap(i)
        {
            self.f[i].magic.awaiting_rite = None;
        }
        self.magic_area_end(i);
        if self.magic_paralyzed(i)
            && self.save_advantage(
                i,
                WIS,
                self.spell_dc(1 - i),
                self.magic_heightened_concentration_for(1 - i, i),
            )
        {
            self.f[1 - i].concentration = None;
            self.f[i].magic.held = false;
        }
        let f = &mut self.f[i];
        f.magic.slot_floor = 1;
        if f.magic.mocked_until == self.turns {
            f.attack_disadvantage = false;
        }
    }

    pub(super) fn magic_turn(&mut self, i: usize) {
        self.magic_reset_opportunity(i);
        let heroism = self.magic_heroism_source(i);
        if self.f[i].stunned {
            self.magic_end_wild_shape(i);
        }
        let f = &mut self.f[i];
        if self.turns >= f.magic.expires {
            f.concentration = None;
        }
        let word = f.magic.word.take();
        if let Some(source) = heroism {
            self.grant_temporary_hp(i, self.f[source].build.casting, TempSource::Heroism(source));
        }
        self.magic_area_start(i);
        if let Some((source, word)) = word {
            self.start_compelled_turn(source, i, compulsion_word(word));
        } else if self.start_fear_travel(i) {
        } else if self.f[1 - i].concentration == Some(Concentration::Suggestion) {
            self.start_suggested_turn(1 - i, i);
        }
    }

    fn magic_drop_gear(&mut self, i: usize) {
        self.drop_held(i);
    }

    pub(super) fn finish_suggestion_cast(&mut self, i: usize, plan: SuggestionPlan) {
        self.f[i].magic.suggestion = Some(plan);
        if self.f[i]
            .magic
            .readied
            .is_some_and(|(spell, _)| spell == Spell::Suggestion)
            && self.f[i].concentration != Some(Concentration::ReadiedSpell)
        {
            self.finish_readied_suggestion(i);
        } else {
            self.execute_magic_step(i, Act::Magic13);
        }
    }

    pub(super) fn magic_ac_bonus(&self, i: usize) -> i32 {
        if self.magic_ally_shielded(i) {
            2
        } else {
            0
        }
    }

    pub(super) fn magic_fly_speed(&self, i: usize) -> i32 {
        if self.magic_buff_source(i, Concentration::Fly).is_some() {
            12
        } else {
            0
        }
    }

    pub(in crate::arena2) fn magic_heal(&mut self, i: usize, amount: i32) {
        if amount > 0 {
            let hp = self.f[i].hp;
            self.heal(i, amount);
            self.entity_life_bond(i, self.f[i].hp - hp);
        }
    }

    pub(super) fn magic_attack_kind(&self, i: usize, weapon: bool) -> Option<DamageType> {
        use Spell::*;
        if weapon {
            if self.f[i].magic.attack == Some(TrueStrike) && self.f[i].magic.true_strike_radiant {
                return Some(DamageType::Radiant);
            }
            if let Some(form) = self.wild_form(i) {
                return Some(form.damage_type);
            }
            if self.magic_shillelagh_active(i) && self.f[i].magic.shillelagh_force {
                return Some(DamageType::Force);
            }
            return None;
        }
        Some(match self.f[i].magic.attack? {
            FireBolt | ScorchingRay => DamageType::Fire,
            ShockingGrasp => DamageType::Lightning,
            PoisonSpray => DamageType::Poison,
            EldritchBlast | MagicMissile | SpiritualWeapon => DamageType::Force,
            RayOfFrost => DamageType::Cold,
            StarryWisp | GuidingBolt | SacredFlame => DamageType::Radiant,
            ChromaticOrb => self.f[i].magic.element,
            _ => return None,
        })
    }
}

impl Battle {
    /// Concentration ID, remaining duration, slot floor, Innate Sorcery, Divine Favor,
    /// Shillelagh, Wild Shape, field x/y and tier, primary/secondary charges, Guiding
    /// Bolt, Mockery, slot spent this turn, and availability of Wild Resurgence recovery.
    /// Values are normalized to [0, 1]. Field coordinates are zero when inactive.
    pub fn magic_features(&self, i: usize) -> [f32; 16] {
        use Concentration::*;
        let f = &self.f[i];
        let m = &f.magic;
        let concentration = f.concentration.map_or(0.0, |effect| f32::from(study::concentration_identity(effect)));
        let duration =
            |deadline: i32, scale: f32| ((deadline - self.turns).max(0) as f32 / scale).min(1.0);
        let area = match f.concentration {
            Some(Moonbeam | SpiritualWeapon | StinkingCloud | HungerOfHadar | FogCloud | PhantasmalForce) => m.field,
            _ => None,
        };
        let active = |active| if active { 1.0 } else { 0.0 };
        [
            concentration / 27.0,
            if f.concentration.is_some() {
                duration(m.expires, 200.0)
            } else {
                0.0
            },
            (m.cast_tier.unwrap_or(m.slot_floor) as f32 - 1.0) / 2.0,
            duration(m.innate_until, 20.0),
            duration(m.favor_until, 20.0),
            duration(m.shillelagh_until, 20.0),
            active(m.form),
            area.map_or(0.0, |pos| {
                (f32::from(pos.cell.x) + 1.0) / f32::from(super::geometry::WIDTH)
            }),
            area.map_or(0.0, |pos| {
                (f32::from(pos.cell.y) + 1.0) / f32::from(super::geometry::HEIGHT)
            }),
            if f.concentration.is_some() {
                m.field_level as f32 / 3.0
            } else {
                0.0
            },
            (f.resource as f32 / 5.0).clamp(0.0, 1.0),
            (f.secondary as f32 / 5.0).clamp(0.0, 1.0),
            active(m.guided_until > 0 && m.guided_until >= self.turns),
            active(m.mocked_until > 0 && m.mocked_until >= self.turns && f.attack_disadvantage),
            active(f.slot_turn == self.turns),
            active(f.build.class == Class::Druid && f.build.level >= 5 && !m.resurgence_slot),
        ]
    }

    pub fn magic_summary(&self, i: usize) -> String {
        use Concentration::*;
        let f = &self.f[i];
        let m = &f.magic;
        let mut parts = self.arcane_status(i);
        parts.extend(self.alarm_summary(i, i));
        parts.extend(self.elementalism_summary(i, i));
        if self.magic_nondetection(i) {
            parts.push("Nondetection".into());
        }
        if self.phantasmal_force_dc(i).is_some() {
            parts.push("Phantasm (Study to examine)".into());
        }
        if let (n @ 1.., _) = self.warlock_rite_progress(i) {
            parts.push(format!("Magical Cunning: {n}/10 actions"));
        }
        if let Some(concentration) = f.concentration {
            let name = match concentration {
                Bless => "Bless",
                Hex => "Hex",
                HunterMark => "Hunter's Mark",
                HypnoticPattern => "Hypnotic Pattern",
                HideousLaughter => "Tasha’s Hideous Laughter",
                CalmEmotions => "Calm Emotions",
                Sleep => "Sleep",
                DetectThoughts => "Detect Thoughts",
                PhantasmalForce => "Phantasmal Force",
                Moonbeam => "Moonbeam",
                SpiritualWeapon => "Spiritual Weapon",
                SpiritGuardians => "Spirit Guardians",
                Blur => "Blur",
                AlterSelf => "Alter Self",
                DragonBreath => "Dragon's Breath",
                Fear => "Fear",
                Fly => "Fly",
                Suggestion => "Suggestion",
                StinkingCloud => "Stinking Cloud",
                Protection => "Protection from Evil and Good",
                ShieldOfFaith => "Shield of Faith",
                ReadiedSpell => "readied spell",
                HoldPerson => "Hold Person",
                Entangle => "Entangle",
                Heroism => "Heroism",
                EnsnaringStrike => "Ensnaring Strike",
                Bane => "Bane",
                ExpeditiousRetreat => "Expeditious Retreat",
                Guidance => "Guidance",
                Resistance => "Resistance",
                ProtectionFromEnergy => "Protection from Energy",
                Mending => "Mending",
                AlarmCasting => "casting Alarm",
                FogCloud => "Fog Cloud",
                GustOfWind => "Gust of Wind",
                Invisibility => "Invisibility",
                PassWithoutTrace => "Pass without Trace",
                CrusadersMantle => "Crusader's Mantle",
                EnhanceAbility => "Enhance Ability",
                HungerOfHadar => "Hunger of Hadar",
                FaerieFire => "Faerie Fire",
            };
            let mut item = format!("Concentrating on {name}");
            if matches!(concentration, Moonbeam | SpiritualWeapon | StinkingCloud | HungerOfHadar | FogCloud | PhantasmalForce) {
                if let Some(pos) = m.field {
                    item.push_str(&format!(
                        " at ({}, {}, elevation {})",
                        pos.cell.x, pos.cell.y, pos.altitude
                    ));
                }
            }
            parts.push(item);
        }
        for (applies, label) in [
            (m.innate_until > self.turns, "Innate Sorcery"),
            (m.favor_until > self.turns, "Divine Favor"),
            (m.shillelagh_until > self.turns, "Shillelagh"),
            (self.magic_incap(i) && !self.magic_laughter(i) && !self.sleep_affected(i), "Hypnotized"),
            (self.magic_laughter(i), "Laughing uncontrollably"),
            (self.sleep_affected(i) && !self.sleep_unconscious(i), "Drowsy"),
            (self.sleep_unconscious(i), "Unconscious"),
            (
                m.guided_until > 0 && m.guided_until >= self.turns,
                "Guiding Bolt advantage",
            ),
        ] {
            if applies {
                parts.push(label.to_owned());
            }
        }
        if let Some(form) = self.wild_form(i) {
            parts.push(format!("Wild Shape ({})", form.name));
        }
        if m.slot_floor > 1 {
            parts.push(format!("Selected spell slot level {}", m.slot_floor));
        }
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Act::*;
    use Class::*;

    fn encounter(seed: u64, caster: Class, target: Class, level: i32) -> Battle {
        let mut battle = Battle::new(seed, [caster, target], [level, level]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 2, y: 0 };
        battle
    }

    fn cast_at(battle: &mut Battle, action: Act, center: Pos) {
        use super::super::ACTS;
        battle.step(action).unwrap();
        assert_eq!(battle.target_request(), Some((action, None)));
        battle.step(ACTS[center.y as usize]).unwrap();
        assert_eq!(battle.target_request(), Some((action, Some(center.y))));
        battle.step(ACTS[center.x as usize]).unwrap();
        if let Some((pending, square)) = battle.target_altitude_request() {
            assert_eq!((pending, square), (action, center));
            battle.step(ACTS[0]).unwrap();
        }
        assert_eq!(battle.target_request(), None);
    }

    #[test]
    fn shatter_can_hit_caster_foe_or_neither_and_spends_the_same_slot() {
        for (x, outcomes) in [(0, [true, false]), (4, [false, false]), (8, [false, true])] {
            let mut battle = encounter(1, Bard, Fighter, 3);
            battle.f[1].pos.x = 8;
            let hp = [battle.f[0].hp, battle.f[1].hp];
            cast_at(&mut battle, Spell3, Pos { x, y: 0 });
            assert_eq!(battle.f[0].slots, [4, 1, 0]);
            assert!(!battle.action);
            assert_eq!([battle.f[0].hp < hp[0], battle.f[1].hp < hp[1]], outcomes);
        }
    }

    #[test]
    fn lands_aid_can_heal_without_an_enemy_in_the_radius() {
        let mut battle = encounter(1, Druid, Fighter, 3);
        battle.f[1].pos.x = 8;
        battle.f[0].hp -= 10;
        let [self_hp, foe_hp] = [battle.f[0].hp, battle.f[1].hp];
        cast_at(&mut battle, Special3, Pos { x: 0, y: 0 });
        battle.step(ACTS[32]).unwrap(); // Deal no damage.
        battle.step(N).unwrap(); // Heal the caster.
        assert_eq!(battle.f[0].resource, 1);
        assert!(battle.f[0].hp > self_hp);
        assert_eq!(battle.f[1].hp, foe_hp);
    }

    #[test]
    fn moonbeam_attacks_both_creatures_but_only_once_each_per_turn() {
        let mut battle = encounter(1, Druid, Fighter, 3);
        let open_hp = [battle.f[0].hp, battle.f[1].hp];
        cast_at(&mut battle, Spell3, Pos { x: 1, y: 0 });
        let reduced_hp = [battle.f[0].hp, battle.f[1].hp];
        assert!(reduced_hp[0] < open_hp[0]);
        assert!(reduced_hp[1] < open_hp[1]);
        assert_eq!(
            battle.f[0].magic.field_last_turn[..2],
            [battle.areas.epoch; 2]
        );
        battle.step(End).unwrap();
        assert_eq!([battle.f[0].hp, battle.f[1].hp], reduced_hp);
    }

    #[test]
    fn charm_excludes_its_source_from_direct_group_and_readied_spell_targets() {
        let mut battle = encounter(3, Warlock, Warlock, 5);
        battle.f[1].concentration = Some(Concentration::Suggestion);
        battle.activate_suggestion(1, 0, SuggestionPlan::EndTurn);
        for spell in [
            Spell::ViciousMockery,
            Spell::SacredFlame,
            Spell::DissonantWhispers,
            Spell::Suggestion,
            Spell::Hex,
            Spell::HunterMark,
            Spell::Bane,
            Spell::Command,
            Spell::HoldPerson,
        ] {
            assert_eq!(battle.creature_choices(0, spell) & ACTS[1].bit(), 0);
        }
        assert_ne!(battle.creature_choices(0, Spell::HealingWord) & ACTS[1].bit(), 0);
        assert!(!battle.magic_can_counterspell(0, 1));
        assert!(!battle.magic_readied_valid(0));

        let slots = battle.f[0].slots;
        let mut prepared = battle.clone();
        assert!(prepared.magic_prepare(0, Magic2)); // Ready Bane.
        prepared.magic_release(0, Magic2);
        assert_ne!(prepared.mask() & ACTS[0].bit(), 0);
        assert_eq!(prepared.mask() & (ACTS[1].bit() | ACTS[35].bit()), 0);

        battle.step(Magic2).unwrap(); // Cast upcast Bane.
        assert_ne!(battle.mask() & ACTS[0].bit(), 0);
        assert_eq!(battle.mask() & (ACTS[1].bit() | ACTS[35].bit()), 0);
        assert!(battle.step(ACTS[35]).is_err());
        assert_eq!(battle.f[0].slots, slots);
    }

    #[test]
    fn cancelling_bless_recipients_does_not_leak_rank_to_hold_person() {
        let mut battle = encounter(3, Cleric, Fighter, 5);
        battle.step(Spell5).unwrap();
        battle.step(N).unwrap(); // Rank one Bless.
        assert_eq!(battle.phase_id(), 48);
        battle.step(Pass).unwrap();
        assert!(battle.f[0].magic.cast_tier.is_none());

        battle.step(Magic9).unwrap();
        assert_eq!(battle.phase_id(), 38);
        battle.step(NE).unwrap(); // Rank two Hold Person.
        assert_eq!(battle.phase_id(), 48);
        assert_ne!(battle.mask() & ACTS[1].bit(), 0);
        battle.step(ACTS[1]).unwrap();
    }

    #[test]
    fn cancelling_shield_of_faith_does_not_charge_the_previous_rank_for_bless() {
        let mut battle = encounter(3, Paladin, Fighter, 5);
        battle.step(Gear2).unwrap();
        // Start a rank-two Shield of Faith and cancel while selecting recipients.
        battle.step(Magic16).unwrap();
        battle.step(NE).unwrap();
        assert_eq!(battle.phase_id(), 48);
        battle.step(Pass).unwrap();
        assert!(battle.f[0].magic.cast_tier.is_none());

        battle.f[0].slots[1] = 0;
        let rank_one_before = battle.f[0].slots[0];
        battle.step(Spell0).unwrap();
        battle.step(N).unwrap(); // Self only.
        assert_eq!(battle.f[0].slots[0], rank_one_before - 1);
        assert_eq!(battle.f[0].slots[1], 0);
    }

    #[test]
    fn quickened_shatter_finishes_ground_aim_after_action_was_spent() {
        let mut battle = encounter(3, Sorcerer, Fighter, 4);
        battle.step(Dodge).unwrap();
        battle.step(Magic22).unwrap();
        battle.step(N).unwrap(); // Quickened.
        cast_at(&mut battle, Magic13, Pos { x: 1, y: 0 });
        assert_eq!(battle.f[0].resource, 2);
        assert_eq!(battle.f[0].slots[1], 2);
        assert!(!battle.bonus);
    }

    #[test]
    fn spell_slots_upcast_and_block_a_second_slot_on_the_same_turn() {
        let mut battle = encounter(3, Cleric, Fighter, 5);
        battle.f[0].hp -= 5;
        assert_eq!(battle.slot_for_action(0, Spell2), Some(1));
        assert_eq!(battle.mask() & Special8.bit(), 0);
        battle.step(Spell2).unwrap();
        if battle.phase_id() == 48 {
            battle.step(N).unwrap();
        }
        assert_eq!(battle.phase_id(), 38);
        assert_eq!(battle.f[0].slots, [4, 3, 2]);
        assert_eq!(battle.mask() & Pass.bit(), 0);
        battle.step(E).unwrap();
        assert_eq!(battle.f[0].slots, [4, 3, 1]);
        assert_eq!(battle.mask() & Spell5.bit(), 0);
    }

    #[test]
    fn warlock_pact_slots_and_ranger_free_marks_use_distinct_economies() {
        let mut warlock = encounter(3, Warlock, Fighter, 3);
        assert_eq!(warlock.slot_for_action(0, Spell1), Some(2));
        warlock.step(Spell1).unwrap();
        warlock.step(N).unwrap();
        if warlock.phase_id() == 48 {
            warlock.step(NE).unwrap();
        }
        assert_eq!(warlock.f[0].slots, [0, 1, 0]);
        assert_eq!(warlock.f[0].concentration, Some(Concentration::Hex));
        assert_ne!(warlock.mask() & Spell0.bit(), 0);

        let mut ranger = encounter(3, Ranger, Fighter, 1);
        ranger.f[0].hp -= 5;
        ranger.step(Spell0).unwrap();
        ranger.step(N).unwrap();
        if ranger.phase_id() == 48 {
            ranger.step(NE).unwrap();
        }
        assert_eq!(ranger.f[0].secondary, 1);
        assert_eq!(ranger.f[0].slots, [2, 0, 0]);
        assert_ne!(ranger.mask() & Spell1.bit(), 0);
    }

    #[test]
    fn wild_shape_keeps_concentration_and_blocks_new_spells() {
        let mut battle = encounter(9, Druid, Fighter, 3);
        battle.f[1].hp = 100;
        cast_at(&mut battle, Spell3, Pos { x: 2, y: 0 });
        assert_eq!(battle.f[0].concentration, Some(Concentration::Moonbeam));
        battle.step(Special1).unwrap();
        battle.step(N).unwrap();
        assert_eq!(battle.phase_id(), 100);
        battle.step(N).unwrap();
        assert!(battle.magic_is_wolf(0));
        assert_eq!(battle.f[0].temp, 3);
        assert_eq!(battle.magic_speed(0), 8);
        assert_eq!(battle.magic_weapon(0, false), Some((6, 2)));
        assert_eq!(battle.f[0].concentration, Some(Concentration::Moonbeam));
        battle.step(End).unwrap();
        battle.step(End).unwrap();
        assert_eq!(battle.mask() & Spell0.bit(), 0);
        battle.step(Special2).unwrap();
        if battle.mask() & Gear2.bit() != 0 {
            battle.step(Gear2).unwrap();
        }
        assert_ne!(battle.mask() & Spell0.bit(), 0);
    }

    #[test]
    fn shield_reacts_to_magic_missile_and_expires_on_its_casters_turn() {
        let mut battle = encounter(3, Wizard, Sorcerer, 2);
        let hp = battle.f[1].hp;
        battle.step(Spell1).unwrap();
        assert_eq!(battle.phase_id(), 80);
        battle.step(ACTS[17]).unwrap();
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.phase_id(), 81);
        battle.step(NE).unwrap();
        assert_eq!(battle.f[1].hp, hp);
        assert_eq!(battle.f[1].slots, [2, 0, 0]);
        assert_eq!(battle.f[1].shield, 5);
        battle.step(End).unwrap();
        assert_eq!(battle.f[1].shield, 0);
    }

    #[test]
    fn font_of_magic_converts_without_actions_and_creates_only_eligible_ranks() {
        for (level, tier) in [(2, 1), (3, 2), (5, 3)] {
            let mut battle = encounter(5, Sorcerer, Fighter, level);
            let initial_slots = battle.f[0].slots;
            battle.step(Special4).unwrap();
            for index in 0..3 {
                assert_eq!(battle.mask() & ACTS[index].bit() != 0, index < tier);
            }
            battle.step(ACTS[tier - 1]).unwrap();
            assert_eq!(battle.f[0].slots[tier - 1], initial_slots[tier - 1] + 1);
            assert_eq!(battle.f[0].resource, 0);
            assert!(battle.action);
            assert!(!battle.bonus);
            assert_eq!(battle.f[0].slot_turn, -1);
            battle.step(Special5).unwrap();
            battle.step(ACTS[tier - 1]).unwrap();
            assert_eq!(battle.f[0].slots, initial_slots);
            assert_eq!(battle.f[0].resource, tier as i32);
            assert!(battle.action);
            assert!(!battle.bonus);
            assert_eq!(battle.f[0].slot_turn, -1);
        }
    }

    #[test]
    fn quickened_cantrip_preserves_action_and_excludes_slot_casting() {
        for shortcut in [false, true] {
            let mut battle = encounter(5, Sorcerer, Fighter, 2);
            battle.f[1].hp = 100;
            if shortcut {
                battle.step(Special3).unwrap();
            } else {
                battle.step(Magic22).unwrap();
                battle.step(N).unwrap();
                battle.step(Spell0).unwrap();
            }
            if battle.phase_id() == 80 {
                battle.step(NE).unwrap();
            }
            assert_eq!(battle.f[0].resource, 0);
            assert!(battle.action);
            assert!(!battle.bonus);
            assert_ne!(battle.mask() & Spell0.bit(), 0);
            assert_eq!(battle.mask() & Spell1.bit(), 0);
        }
    }

    #[test]
    fn chromatic_orb_element_choice_reserves_until_selection() {
        let mut battle = encounter(6, Sorcerer, Fighter, 2);
        battle.f[1].hp = 100;
        let slots = battle.f[0].slots;
        battle.step(Spell1).unwrap();
        assert_eq!(battle.phase_id(), 35);
        assert_eq!(battle.f[0].slots, slots);
        assert_eq!(battle.choice_label(0, S), "Poison");
        battle.step(S).unwrap();
        assert_eq!(battle.f[0].magic.element, DamageType::Poison);
        assert_eq!(battle.f[0].slots[0], slots[0] - 1);
    }

    #[test]
    fn counterspell_can_negate_precommitted_cast_and_refunds_the_casters_slot() {
        let interrupted = (0..100).any(|seed| {
            let mut battle = encounter(seed, Wizard, Warlock, 5);
            let slots = battle.f[0].slots;
            let hp = battle.f[1].hp;
            battle.step(Spell1).unwrap();
            assert_eq!(battle.phase_id(), 38);
            battle.step(N).unwrap();
            assert_eq!(battle.phase_id(), 47);
            assert!(!battle.action);
            battle.step(Defend).unwrap();
            assert_eq!(battle.f[1].slots, [0, 0, 1]);
            assert!(!battle.f[1].reaction);
            battle.f[0].slots == slots && battle.f[1].hp == hp
        });
        assert!(interrupted);
    }

    #[test]
    fn counterspell_can_be_countered_if_the_original_caster_has_a_legal_reaction() {
        let stopped_a_counter = (0..100).any(|seed| {
            let mut b = encounter(seed, Warlock, Warlock, 5);
            b.step(Spell0).unwrap();
            assert_eq!(b.actor(), Some(1));
            assert_eq!(b.phase_id(), 47);
            b.step(Defend).unwrap();
            assert_eq!(b.actor(), Some(0));
            assert_eq!(b.phase_id(), 47);
            b.step(Defend).unwrap();
            assert_eq!(b.f[0].slots[2], 1);
            assert!(!b.f[0].reaction && !b.f[1].reaction);
            b.phase_id() == 80 && b.f[1].slots[2] == 2
        });
        assert!(stopped_a_counter);
    }

    #[test]
    fn ordinary_shield_can_use_any_legal_slot_even_on_an_unpreventable_hit() {
        let observed = (0..100).any(|seed| {
            let mut b = encounter(seed, Fighter, Wizard, 5);
            b.f[1].pos.x = 1;
            let hp = b.f[1].hp;
            b.step(Melee).unwrap();
            if b.phase_id() != 2 {
                return false;
            }
            b.step(Defend).unwrap();
            assert_eq!(b.phase_id(), 38);
            assert!(b.mask() & E.bit() != 0);
            b.step(E).unwrap();
            assert_eq!(b.f[1].slots, [4, 3, 1]);
            assert_eq!(b.f[1].shield, 5);
            b.f[1].hp < hp
        });
        assert!(observed);
    }

    #[test]
    fn ordinary_shield_can_be_countered_and_keeps_its_slot_if_interrupted() {
        let observed = (0..100).any(|seed| {
            let mut b = encounter(seed, Warlock, Wizard, 5);
            b.step(Spell0).unwrap();
            b.step(NE).unwrap();
            if b.phase_id() != 2 {
                return false;
            }
            let slots = b.f[1].slots;
            let hp = b.f[1].hp;
            b.step(Defend).unwrap();
            b.step(N).unwrap();
            assert_eq!(b.phase_id(), 47);
            assert_eq!(b.actor(), Some(0));
            b.step(Defend).unwrap();
            assert_eq!(b.f[0].slots[2], 1);
            if b.phase_id() == 3 {
                b.step(Pass).unwrap();
            }
            b.f[1].slots == slots && b.f[1].shield == 0 && b.f[1].hp < hp
        });
        assert!(observed);
    }

    #[test]
    fn guidance_chooses_exact_skill_even_at_inherited_pass_action_index() {
        use super::super::origins::Skill;
        let mut b = encounter(6, Cleric, Fighter, 1);
        if b.mask() & Gear2.bit() != 0 {
            b.step(Gear2).unwrap();
        }
        b.step(Magic5).unwrap();
        assert_eq!(b.phase_id(), 49);
        assert_eq!(b.choice_label(0, Pass), "Stealth");
        b.step(Pass).unwrap();
        if b.phase_id() == 48 {
            b.step(NE).unwrap();
        }
        assert_eq!(b.f[0].concentration, Some(Concentration::Guidance));
        assert_eq!(b.magic_guidance(0, Skill::Acrobatics), 0);
        assert!((1..=4).contains(&b.magic_guidance(0, Skill::Stealth)));
    }

    #[test]
    fn burning_hands_uses_the_chosen_cone_direction() {
        for (direction, hit) in [(E, true), (W, false)] {
            let mut battle = encounter(3, Wizard, Fighter, 3);
            let hp = battle.f[1].hp;
            battle.step(Spell2).unwrap();
            assert_eq!(battle.phase_id(), 34);
            battle.step(direction).unwrap();
            assert_eq!(battle.phase_id(), 38);
            battle.step(N).unwrap();
            assert_eq!(battle.f[0].slots, [3, 2, 0]);
            assert_eq!(battle.f[1].hp < hp, hit);
        }
    }

    #[test]
    fn wild_shape_adds_two_more_known_forms_at_level_four() {
        let mut early = encounter(3, Druid, Fighter, 2);
        early.step(Special1).unwrap();
        assert_eq!(early.mask(), 0b1111);
        let mut late = encounter(3, Druid, Fighter, 4);
        late.step(Special1).unwrap();
        assert_eq!(late.mask(), 0b11_1111);
        late.step(SW).unwrap();
        late.step(N).unwrap();
        assert_eq!(late.magic_ac(0), Some(13));
        assert_eq!(late.magic_weapon_dice(0), 2);
        assert!(late.magic_resists_poison(0));
    }

    #[test]
    fn cutting_words_reduces_a_spell_damage_roll_as_a_distinct_reaction() {
        let observed = (0..24).any(|seed| {
            let mut battle = encounter(seed, Wizard, Bard, 5);
            battle.f[1].pos.x = 8;
            battle.f[1].hp = 100;
            cast_at(&mut battle, Spell4, Pos { x: 8, y: 0 });
            assert_eq!(battle.phase_id(), 40);
            let mut passed = battle.clone();
            let resource = battle.f[1].resource;
            passed.step(Pass).unwrap();
            battle.step(Defend).unwrap();
            assert_eq!(battle.f[1].resource, resource - 1);
            assert!(!battle.f[1].reaction);
            battle.f[1].hp > passed.f[1].hp
        });
        assert!(observed);
    }

    #[test]
    fn light_can_illuminate_even_a_lit_arena_without_spending_a_slot() {
        let mut battle = encounter(2, Sorcerer, Fighter, 1);
        let slots = battle.f[0].slots;
        battle.step(Magic2).unwrap();
        battle.step(N).unwrap();
        battle.step(NE).unwrap();
        assert_eq!(battle.f[0].magic.light_color, 1);
        assert!(battle.f[0].magic.light_until > battle.turns);
        assert_eq!(battle.f[0].slots, slots);
        assert!(!battle.action);
    }

    #[test]
    fn cure_wounds_can_choose_familiar_touch_without_spending_reaction_for_direct_touch() {
        use super::super::entities::{Entity, EntityKind, FamiliarForm, Spirit};
        let mut b = encounter(9, Druid, Fighter, 2);
        let prior = b.f[0].hp;
        b.f[0].hp -= 7;
        b.fixture_companion(
            0,
            Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Owl,
                    spirit: Spirit::Fey,
                },
                0,
                Pos { x: 1, y: 0 },
                1,
            ),
        );
        if b.mask() & Gear2.bit() != 0 {
            b.step(Gear2).unwrap();
        }
        b.step(Special6).unwrap();
        assert_eq!(b.phase_id(), 48);
        if b.choice_label(0, N) != "Touch target directly" {
            b.step(N).unwrap();
        }
        let mut direct = b.clone();
        direct.step(N).unwrap();
        b.step(NE).unwrap();
        assert!(!b.entity(2).unwrap().reaction);
        assert!(direct.entity(2).unwrap().reaction);
        assert_eq!(b.f[0].hp, direct.f[0].hp);
        assert!(b.f[0].hp > prior - 7);
    }

    #[test]
    fn mage_armor_sets_unarmored_base_ac_and_clears_on_wearing_armor() {
        let mut battle = encounter(7, Wizard, Fighter, 1);
        assert_eq!(battle.magic_ac(0), None);
        battle.step(Magic2).unwrap();
        assert_eq!(battle.magic_ac(0), Some(13 + battle.f[0].build.stats[DEX]));
        battle.magic_end_mage_armor(0);
        assert_eq!(battle.magic_ac(0), None);
    }

    #[test]
    fn sorcerer_selects_tier_for_both_directions_of_slot_conversion() {
        let mut battle = encounter(7, Sorcerer, Fighter, 5);
        battle.f[0].resource = 3;
        battle.step(Special5).unwrap();
        assert_eq!(battle.phase_id(), 38);
        battle.step(NE).unwrap();
        assert_eq!(battle.f[0].resource, 5);
        assert_eq!(battle.f[0].slots, [4, 2, 2]);
        battle.step(Special4).unwrap();
        battle.step(E).unwrap();
        assert_eq!(battle.f[0].resource, 0);
        assert_eq!(battle.f[0].slots, [4, 2, 3]);
        assert!(battle.action);
        assert!(!battle.bonus);
    }

    #[test]
    fn wild_resurgence_can_spend_a_chosen_slot_and_once_create_a_first_rank_slot() {
        let mut battle = encounter(7, Druid, Fighter, 5);
        battle.f[0].resource = 0;
        battle.step(Special7).unwrap();
        battle.step(E).unwrap();
        assert_eq!(battle.f[0].resource, 1);
        assert_eq!(battle.f[0].slots, [4, 3, 1]);
        battle.step(Special7).unwrap();
        assert_eq!(battle.f[0].resource, 0);
        assert_eq!(battle.f[0].slots, [5, 3, 1]);
        assert_eq!(battle.mask() & Special7.bit(), 0);
        assert!(battle.action && battle.bonus);
    }

    #[test]
    fn dragons_breath_retains_selected_element_for_repeat_cones() {
        let mut battle = encounter(9, Sorcerer, Fighter, 3);
        let hp = battle.f[1].hp;
        battle.step(Magic9).unwrap();
        battle.step(S).unwrap();
        battle.step(N).unwrap(); // Grant the spell to the caster.
        assert_eq!(battle.f[0].concentration, Some(Concentration::DragonBreath));
        assert_eq!(battle.f[0].magic.breath_element, DamageType::Poison);
        battle.step(Magic10).unwrap();
        battle.step(E).unwrap();
        assert!(battle.f[1].hp < hp);
        assert_eq!(battle.f[0].slots, [4, 1, 0]);
    }

    #[test]
    fn alter_self_changes_natural_weapon_without_changing_armed_weapon() {
        let mut battle = encounter(9, Sorcerer, Fighter, 3);
        battle.step(Magic7).unwrap();
        battle.step(E).unwrap();
        assert_eq!(
            battle.magic_unarmed_override(0),
            Some((6, battle.f[0].build.casting))
        );
        assert_eq!(battle.magic_unarmed_type(0), Some(DamageType::Slashing));
        assert_eq!(battle.magic_weapon(0, false), None);
        battle.step(End).unwrap();
        battle.step(End).unwrap();
        battle.step(Magic23).unwrap();
        battle.step(SE).unwrap();
        assert_eq!(battle.magic_unarmed_type(0), Some(DamageType::Piercing));
        assert_eq!(battle.f[0].slots, [4, 1, 0]);
    }

    #[test]
    fn fly_grants_a_real_sixty_foot_flight_speed_only_during_concentration() {
        let mut battle = encounter(9, Sorcerer, Fighter, 5);
        battle.step(Magic12).unwrap();
        assert_eq!(battle.magic_fly_speed(0), 12);
        battle.step(DropConcentration).unwrap();
        assert_eq!(battle.magic_fly_speed(0), 0);
        assert_eq!(battle.f[0].slots[2], 1);
    }

    #[test]
    fn resistance_cantrip_reduces_the_selected_type_once_per_turn() {
        let mut battle = encounter(9, Druid, Fighter, 4);
        if battle.mask() & Gear2.bit() != 0 {
            battle.step(Gear2).unwrap();
        }
        battle.step(Magic7).unwrap();
        battle.step(N).unwrap();
        assert_eq!(battle.magic_flat_protection(0, 9, DamageType::Fire), 9);
        assert!((5..9).contains(&battle.magic_flat_protection(0, 9, DamageType::Acid)));
        assert_eq!(battle.magic_flat_protection(0, 9, DamageType::Acid), 9);
    }

    #[test]
    fn expeditious_retreat_grants_bonus_dash_on_cast_and_later_turns() {
        let mut battle = encounter(2, Warlock, Fighter, 2);
        let speed = battle.magic_speed(0);
        battle.step(Magic3).unwrap();
        assert_eq!(
            battle.dash,
            speed,
            "phase={} concentration={:?}",
            battle.phase_id(),
            battle.f[0].concentration
        );

        assert!(battle.action && !battle.bonus);
        battle.step(End).unwrap();
        battle.step(End).unwrap();
        battle.step(Magic23).unwrap();
        assert_eq!(battle.dash, speed);
        assert!(battle.action && !battle.bonus);
        assert_eq!(battle.f[0].slots, [1, 0, 0]);
    }

    #[test]
    fn magical_cunning_recovers_pact_slots_after_one_minute_once() {
        let mut battle = encounter(9, Warlock, Fighter, 3);
        battle.f[0].slots[1] = 0;
        battle.step(Magic21).unwrap();
        assert_eq!(battle.f[0].slots, [0, 0, 0]);
        for _ in 1..10 {
            battle.step(End).unwrap();
            battle.step(End).unwrap();
            assert_eq!(battle.f[0].slots, [0, 0, 0]);
            battle.step(Magic21).unwrap();
        }
        assert_eq!(battle.f[0].slots, [0, 1, 0]);
        assert_eq!(battle.mask() & Magic21.bit(), 0);
    }

    #[test]
    fn true_strike_uses_weapon_and_damage_type_choices_without_a_slot() {
        let mut battle = encounter(13, Wizard, Fighter, 4);
        battle.f[1].pos.x = 1;
        let slots = battle.f[0].slots;
        battle.step(Magic4).unwrap();
        assert_eq!(battle.phase_id(), 49);
        battle.step(N).unwrap();
        assert_eq!(battle.phase_id(), 49);
        battle.step(NE).unwrap();
        assert_eq!(battle.phase_id(), 80);
        battle.step(NE).unwrap();
        assert!(!battle.action);
        assert_eq!(battle.f[0].slots, slots);
        assert!(battle.f[0].magic.true_strike_radiant);
    }

    #[test]
    fn potent_cantrip_applies_to_a_true_strike_weapon_miss_against_a_creature() {
        let missed = (0..64).any(|seed| {
            let mut battle = encounter(seed, Wizard, Fighter, 5);
            battle.f[1].pos.x = 1;
            battle.f[1].shield = 100;
            let hp = battle.f[1].hp;
            battle.step(Magic4).unwrap();
            battle.step(N).unwrap();
            battle.step(NE).unwrap();
            battle.step(NE).unwrap();
            (1..=8).contains(&(hp - battle.f[1].hp))
        });
        assert!(missed);
    }

    #[test]
    fn command_upcast_allows_additional_creatures_after_payment_selection() {
        let mut battle = encounter(9, Bard, Fighter, 3);
        let slots = battle.f[0].slots;
        battle.step(Magic2).unwrap();
        assert_eq!(battle.phase_id(), 37);
        battle.step(S).unwrap();
        assert_eq!(battle.phase_id(), 38);
        battle.step(NE).unwrap();
        assert_eq!(battle.phase_id(), 48);
        assert_eq!(battle.f[0].slots, slots);
        assert_ne!(battle.mask() & ACTS[35].bit(), 0);
        battle.step(ACTS[35]).unwrap();
        assert_eq!(battle.f[0].slots, [4, 1, 0]);
    }

    #[test]
    fn bane_penalizes_selected_creature_attacks_and_saves_until_concentration_ends() {
        let observed = (0..100).any(|seed| {
            let mut b = encounter(seed, Warlock, Fighter, 2);
            b.step(Magic2).unwrap();
            assert_eq!(b.phase_id(), 48);
            b.step(ACTS[1]).unwrap();
            if b.f[0].concentration != Some(Concentration::Bane) {
                return false;
            }
            assert!((-4..=-1).contains(&b.magic_roll_bonus(1)));
            assert_eq!(b.magic_roll_bonus(0), 0);
            b.step(DropConcentration).unwrap();
            assert_eq!(b.magic_roll_bonus(1), 0);
            true
        });
        assert!(observed);
    }

    #[test]
    fn hold_person_paralyzes_chosen_humanoid_and_allows_a_repeat_save() {
        let observed = (0..100).any(|seed| {
            let mut b = encounter(seed, Warlock, Fighter, 3);
            b.step(Magic4).unwrap();
            assert_eq!(b.phase_id(), 48);
            b.step(ACTS[1]).unwrap();
            if !b.magic_paralyzed(1) {
                return false;
            }
            b.step(End).unwrap();
            assert_eq!(b.movement_left(), 0);
            assert_eq!(b.mask() & Melee.bit(), 0);
            b.step(End).unwrap();
            !b.magic_paralyzed(1)
        });
        assert!(observed);
    }

    #[test]
    fn protection_from_energy_offers_thunder_not_poison_and_halves_that_type() {
        let mut b = encounter(6, Druid, Fighter, 5);
        if b.mask() & Gear2.bit() != 0 {
            b.step(Gear2).unwrap();
        }
        b.step(Magic4).unwrap();
        assert_eq!(b.phase_id(), 35);
        assert_eq!(b.choice_label(0, S), "Thunder");
        b.step(S).unwrap();
        assert!(b.magic_energy_resistance(0, DamageType::Thunder));
        let hp = b.f[0].hp;
        b.damage_typed(0, 4, DamageType::Thunder);
        assert_eq!(b.f[0].hp, hp - 2);
    }

    #[test]
    fn innate_sorcery_costs_one_use_and_boosts_spell_dc_and_attacks() {
        let mut b = encounter(9, Sorcerer, Fighter, 1);
        let dc = b.spell_dc(0);
        b.step(Special1).unwrap();
        assert_eq!(b.spell_dc(0), dc + 1);
        assert!(b.magic_attack_advantage(0, false));
        assert!(!b.magic_attack_advantage(0, true));
        b.f[0].magic.attack = Some(Spell::TrueStrike);
        assert!(b.magic_attack_advantage(0, true));
        b.f[0].magic.attack = None;
        assert_eq!(b.f[0].secondary, 1);
        assert!(b.action && !b.bonus);
        let first_expiry = b.f[0].magic.innate_until;
        b.step(End).unwrap();
        b.step(End).unwrap();
        b.step(Special1).unwrap();
        assert_eq!(b.f[0].secondary, 0);
        assert!(b.f[0].magic.innate_until > first_expiry);
    }

    #[test]
    fn heightened_hold_person_applies_to_its_chosen_target_until_concentration_ends() {
        let applied = (0..64).any(|seed| {
            let mut b = encounter(seed, Sorcerer, Fighter, 3);
            b.step(Magic22).unwrap();
            b.step(NE).unwrap();
            b.step(Magic5).unwrap();
            let targets = ACTS[1];
            assert_ne!(b.mask() & targets.bit(), 0);
            b.step(targets).unwrap();
            assert_eq!(b.f[0].resource, 1);
            if b.f[0].concentration != Some(Concentration::HoldPerson) {
                return false;
            }
            assert!(b.magic_paralyzed(1));
            assert!(b.magic_heightened_concentration_for(0, 1));
            assert!(!b.magic_heightened_concentration_for(0, 0));
            assert!(!b.magic_heightened_for(0, 1));
            b.f[0].concentration = None;
            assert!(!b.magic_paralyzed(1));
            true
        });
        assert!(applied);
    }

    #[test]
    fn damage_can_break_concentration_and_healing_does_not() {
        let interrupted = (0..100).any(|seed| {
            let mut battle = encounter(seed, Cleric, Fighter, 3);
            battle.step(Spell5).unwrap();
            if battle.phase_id() == 38 {
                battle.step(N).unwrap();
            }
            if battle.phase_id() == 48 {
                battle.step(N).unwrap();
            }
            battle.heal(0, 4);
            assert_eq!(battle.f[0].concentration, Some(Concentration::Bless));
            battle.damage(0, 1);
            battle.f[0].concentration.is_none() && battle.f[0].hp > 0
        });
        assert!(interrupted);
    }

    #[test]
    fn a_push_checks_caltrops_on_an_intermediate_square() {
        let checked = (0..100).any(|seed| {
            let mut battle = encounter(seed, Rogue, Bard, 3);
            battle.f[0].pos = Pos { x: 2, y: 0 };
            cast_at(&mut battle, Special4, Pos { x: 2, y: 0 });
            battle.f[0].pos = Pos { x: 1, y: 0 };
            battle.martial_moved(0);
            battle.f[1].pos = Pos { x: 0, y: 0 };
            battle.magic_push(1, 2);
            assert_eq!(battle.f[0].pos, Pos { x: 3, y: 0 });
            battle.f[0].hp == battle.f[0].build.hp - 1 && battle.f[0].speed_zero
        });
        assert!(checked);
    }

    #[test]
    fn mockery_is_consumed_or_expires_by_the_end_of_the_targets_turn() {
        let mut battle = (0..100)
            .find_map(|seed| {
                let mut battle = encounter(seed, Bard, Fighter, 1);
                battle.step(Spell0).unwrap();
                if battle.phase_id() == 48 {
                    battle.step(NE).unwrap();
                }
                battle.f[1].attack_disadvantage.then_some(battle)
            })
            .expect("some targets fail a Wisdom save");
        battle.step(End).unwrap();
        assert!(battle.f[1].attack_disadvantage);
        battle.step(End).unwrap();
        assert!(!battle.f[1].attack_disadvantage);
    }
}

const ELEMENTS: [(DamageType, &str); 6] = [
    (DamageType::Acid, "Acid"),
    (DamageType::Cold, "Cold"),
    (DamageType::Fire, "Fire"),
    (DamageType::Lightning, "Lightning"),
    (DamageType::Poison, "Poison"),
    (DamageType::Thunder, "Thunder"),
];
const ENERGY_TYPES: [(DamageType, &str); 5] = [
    ELEMENTS[0],
    ELEMENTS[1],
    ELEMENTS[2],
    ELEMENTS[3],
    ELEMENTS[5],
];
const RESISTANCE_TYPES: [(DamageType, &str); 11] = [
    (DamageType::Acid, "Acid"),
    (DamageType::Bludgeoning, "Bludgeoning"),
    (DamageType::Cold, "Cold"),
    (DamageType::Fire, "Fire"),
    (DamageType::Lightning, "Lightning"),
    (DamageType::Necrotic, "Necrotic"),
    (DamageType::Piercing, "Piercing"),
    (DamageType::Poison, "Poison"),
    (DamageType::Radiant, "Radiant"),
    (DamageType::Slashing, "Slashing"),
    (DamageType::Thunder, "Thunder"),
];
const GUIDANCE_SKILLS: [super::origins::Skill; 18] = {
    use super::origins::Skill::*;
    [
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
    ]
};
const FORMS: [&str; 6] = [
    "Aquatic Adaptation",
    "Change Appearance",
    "Claws",
    "Fangs",
    "Horns",
    "Hooves",
];
const WORDS: [CommandWord; 5] = [
    CommandWord::Approach,
    CommandWord::Drop,
    CommandWord::Flee,
    CommandWord::Grovel,
    CommandWord::Halt,
];

impl Battle {
    fn magic_request(&mut self, i: usize, choice: MagicChoice) {
        self.f[i].magic.choice = Some(choice);
        self.request(i, choice.phase());
    }

    pub(super) fn magic_decision_mask(&self, i: usize, phase: i32) -> u128 {
        let f = &self.f[i];
        let Some(choice) = f.magic.choice.filter(|c| c.phase() == phase) else {
            return 0;
        };
        let list_mask = |len| (1u128 << len) - 1;
        match choice {
            MagicChoice::Direction(_) => list_mask(26),
            MagicChoice::CubeFace(_) => list_mask(25),
            MagicChoice::ShillelaghType(_) => list_mask(4),
            MagicChoice::DivineSparkType => list_mask(2),
            MagicChoice::Element(action) => list_mask(
                self.prepared_spell(i, action)
                    .expect("elemental spell")
                    .element_choices()
                    .len(),
            ),
            MagicChoice::Form(_) => list_mask(FORMS.len()),
            MagicChoice::Command(_) => list_mask(WORDS.len()),
            MagicChoice::Guidance(_) => list_mask(GUIDANCE_SKILLS.len()),
            MagicChoice::EnhanceAbility(_) => list_mask(6),
            MagicChoice::AlarmMode { .. } => list_mask(2) | Act::Pass.bit(),
            MagicChoice::AlarmExempt { .. } => self.alarm_exemption_mask(i),
            MagicChoice::TrueStrikeWeapon(_) => self.true_strike_weapon_mask(i),
            MagicChoice::TrueStrikeType(_) => list_mask(2),
            MagicChoice::WeaponEnchantment(_) => self.enchantment_choices(i),
            MagicChoice::HexAbility => list_mask(6),
            MagicChoice::WildShape => list_mask(if f.build.level >= 4 { 6 } else { 4 }),
            MagicChoice::CureRoute(_) => list_mask(2),
            MagicChoice::SpiritualStrike => self.spiritual_recipients(i) | Act::Pass.bit(),
            MagicChoice::LightTarget(_) => self.magic_light_targets(i),
            MagicChoice::LightColor { .. } => list_mask(LIGHT_HUES.len()),

            MagicChoice::Allies(action) => self.magic_ally_choice_mask(i, action),
            MagicChoice::GroupTargets(action) => self.group_target_mask(i, action),
            MagicChoice::HeightenedTarget { candidates, .. } => candidates as u128,

            MagicChoice::AllyConsent { subset, .. } => self.magic_ally_consent_mask(i, subset),
            MagicChoice::Creature(Act::Entity11) => self.ensnaring_escape_targets(i),
            MagicChoice::Creature(action) => {
                self.creature_choices(i, self.creature_menu_spell(i, action))
            }
            MagicChoice::Dispel(_) => self.dispel_targets(i) | Act::Pass.bit(),
            MagicChoice::MarkCost => list_mask(2),
            MagicChoice::MetaMode => [Metamagic::Quickened, Metamagic::Heightened]
                .into_iter()
                .enumerate()
                .filter(|(_, mode)| self.meta_spell_mask(i, *mode) != 0)
                .fold(Act::Pass.bit(), |mask, (index, _)| mask | ACTS[index].bit()),
            MagicChoice::MetaSpell(mode) => self.meta_spell_mask(i, mode) | Act::Pass.bit(),
            MagicChoice::CastSlot { action, .. } => {
                self.castable_slot_mask(i, self.prepared_spell(i, action).expect("slotted spell"))
            }
            MagicChoice::EnsnaringSlot => self.castable_slot_mask(i, Spell::EnsnaringStrike),
            MagicChoice::ShieldSlot => (0..3)
                .filter(|&index| self.can_slot(i, index + 1))
                .fold(Act::Pass.bit(), |mask, index| mask | ACTS[index].bit()),
            MagicChoice::CreateSlot => [2, 3, 5]
                .into_iter()
                .zip([2, 3, 5])
                .enumerate()
                .filter(|(_, (cost, level))| f.resource >= *cost && f.build.level >= *level)
                .fold(Act::Pass.bit(), |mask, (index, _)| mask | ACTS[index].bit()),
            MagicChoice::ConvertSlot | MagicChoice::RecoverBard | MagicChoice::RecoverDruid => (0
                ..3)
                .filter(|&index| f.slots[index] > 0)
                .fold(Act::Pass.bit(), |mask, index| mask | ACTS[index].bit()),
            MagicChoice::Damage(_)
            | MagicChoice::Counterspell { .. }
            | MagicChoice::CounterResponse { .. } => Act::Defend.bit() | Act::Pass.bit(),
        }
    }

    pub(super) fn magic_decision_step(&mut self, i: usize, phase: i32, input: Act) {
        let choice = self.f[i]
            .magic
            .choice
            .take()
            .filter(|c| c.phase() == phase)
            .expect("pending magic choice");
        if input == Act::Pass && matches!(choice, MagicChoice::ShieldSlot) {
            self.f[i].magic.shield_hit = None;
            self.resume_magic_shield(false);
            return;
        }
        if input == Act::Pass
            && !matches!(
                choice,
                MagicChoice::Damage(_)
                    | MagicChoice::Counterspell { .. }
                    | MagicChoice::CounterResponse { .. }
                    | MagicChoice::Guidance(_) | MagicChoice::EnhanceAbility(_)
                    | MagicChoice::AllyConsent { .. }
            )
        {
            self.magic_clear_cast_choices(i);
            return;
        }
        let index = input as usize;
        match choice {
            MagicChoice::ShillelaghType(action) => {
                self.f[i].magic.shillelagh_force = index % 2 == 1;
                self.f[i].magic.shillelagh_strength = index >= 2;
                if action != Act::Magic24 {
                    self.execute_magic_step(i, action);
                }
            }
            MagicChoice::DivineSparkType => {
                self.f[i].magic.divine_spark_kind =
                    [DamageType::Radiant, DamageType::Necrotic][index];
                self.execute_magic_step(i, Act::Special1);
            }
            MagicChoice::Direction(action) => {
                self.f[i].magic.direction = index;
                if action == Act::Magic21 && self.is_subclass(i, Class::Druid, 2) {
                    self.f[i].magic.gust_direction = index;
                    self.bonus = false;
                    self.gust_clear_clouds(i);
                } else if self.prepared_spell(i, action) == Some(Spell::Thunderwave) {
                    self.magic_request(i, MagicChoice::CubeFace(action));
                } else {
                    self.execute_magic_step(i, action);
                }
            }
            MagicChoice::CubeFace(action) => {
                self.f[i].magic.cube_offset = super::targeting::cube_face_offset(index);
                self.execute_magic_step(i, action);
            }
            MagicChoice::Element(action) => {
                let choice_set = self.prepared_spell(i, action)
                    .expect("elemental spell")
                    .element_choices();
                self.f[i].magic.element = choice_set[index].0;
                if self.prepared_spell(i, action) == Some(Spell::DragonBreath) {
                    self.start_breath_touch(i, action);
                } else {
                    self.execute_magic_step(i, action);
                }
            }
            MagicChoice::Form(action) => {
                self.f[i].magic.form_option = index;
                self.execute_magic_step(i, action);
            }
            MagicChoice::Command(action) => {
                self.f[i].magic.chosen_word = WORDS[index];
                self.execute_magic_step(i, action);
            }
            MagicChoice::Guidance(action) => {
                self.f[i].magic.guidance_skill = GUIDANCE_SKILLS[index];
                self.execute_magic_step(i, action);
            }
            MagicChoice::AlarmMode { center } => {
                self.magic_request(i, MagicChoice::AlarmExempt { center, audible: index == 1 });
            }
            MagicChoice::AlarmExempt { center, audible } => {
                self.alarm_initialize(i, center, audible, if index == 31 { 15 } else { index as u8 });
            }
            MagicChoice::EnhanceAbility(action) => {
                self.f[i].magic.enhanced_ability = index;
                self.execute_magic_step(i, action);
            }
            MagicChoice::TrueStrikeWeapon(action) => {
                self.select_true_strike_weapon(i, index);
                self.magic_request(i, MagicChoice::TrueStrikeType(action));
            }
            MagicChoice::TrueStrikeType(action) => {
                self.f[i].magic.true_strike_radiant = index == 1;
                self.execute_magic_step(i, action);
            }
            MagicChoice::WeaponEnchantment(action) => {
                self.f[i].magic.aux_choice = index;
                self.execute_magic_step(i, action);
            }
            MagicChoice::RecoverBard | MagicChoice::RecoverDruid => {
                self.f[i].slots[index] -= 1;
                self.f[i].resource += 1;
                if matches!(choice, MagicChoice::RecoverDruid) {
                    self.f[i].magic.resurgence_turn = self.turns;
                }
            }

            MagicChoice::LightTarget(action) => self.magic_request(
                i,
                MagicChoice::LightColor {
                    action,
                    target: index,
                },
            ),
            MagicChoice::LightColor { action, target } => {
                self.f[i].magic.light_target = target;
                self.f[i].magic.light_color = index;
                self.execute_magic_step(i, action);
            }
            MagicChoice::SpiritualStrike => {
                let target = if index < 4 {
                    Some(index)
                } else {
                    self.spiritual_guess(i, index).and_then(|guess| {
                        [1 - i, 3 - i].into_iter().find(|&id| {
                            self.footprint(id).is_some_and(|footprint| {
                                footprint.squares().any(|point| point == guess)
                            })
                        })
                    })
                };
                let Some(target) = target else { return };
                self.f[i].magic.cast_target = Some(target);
                let dice = self.f[i].magic.field_level as i32 - 1;
                self.start_spell_attack(
                    i,
                    Spell::SpiritualWeapon,
                    8,
                    dice,
                    self.f[i].build.casting,
                    false,
                );
                self.f[i].magic.cast_target = None;
            }
            MagicChoice::Allies(action) => self.magic_commit_ally_choice(i, action, index as u8),
            MagicChoice::GroupTargets(action) => {
                self.f[i].magic.cast_targets = (index - 32) as u8;
                self.execute_magic_step(i, action);
            }
            MagicChoice::HeightenedTarget { action, center, .. } => {
                self.f[i].magic.heightened_target = Some(index);
                self.f[i].magic.resolving_center = center;
                self.execute_magic_step(i, action);
                self.f[i].magic.resolving_center = None;
            }

            MagicChoice::AllyConsent {
                caster,
                action,
                subset,
            } => {
                self.magic_complete_ally_consent(caster, action, subset, input);
            }
            MagicChoice::CureRoute(action) => {
                self.f[i].magic.cure_via_familiar = Some(index == 1);
                self.execute_magic_step(i, action);
            }
            MagicChoice::Creature(Act::Entity11) => {
                self.action = false;
                self.area_escape_restraint_as(i,index);
            }
            MagicChoice::Creature(action) => {
                if action == Act::Magic24 {
                    self.bonus = false;
                    self.f[i].magic.mark_target = Some(index);
                    self.f[i].magic.mark_defeat_turn = None;
                } else {
                    self.f[i].magic.cast_target = Some(index);
                    self.execute_magic_step(i, action);
                }
            }
            MagicChoice::Dispel(action) => {
                self.f[i].magic.choice = Some(choice);
                self.f[i].magic.aux_choice = index;
                self.execute_magic_step(i, action);
                self.f[i].magic.choice = None;
            }
            MagicChoice::WildShape => {
                self.f[i].magic.next_form = index;
                self.request(i, 100);
            }
            MagicChoice::HexAbility => {
                self.f[i].magic.hex_pending_ability = index;
                self.execute_magic_step(i, Act::Spell1);
            }
            MagicChoice::MarkCost => {
                self.f[i].magic.force_mark_slot = index == 1;
                self.execute_magic_step(i, Act::Spell0);
            }
            MagicChoice::MetaMode => {
                self.magic_request(
                    i,
                    MagicChoice::MetaSpell([Metamagic::Quickened, Metamagic::Heightened][index]),
                );
            }
            MagicChoice::MetaSpell(mode) => {
                self.f[i].magic.metamagic = Some((mode, input));
                if !self.begin_targeting(input) {
                    self.magic_step(i, input);
                }
            }
            MagicChoice::CastSlot { action, center } => {
                self.f[i].magic.cast_tier = Some(if input == Act::Rider { 0 } else { index + 1 });
                self.f[i].magic.resolving_center = center;
                self.execute_magic_step(i, action);
                self.f[i].magic.resolving_center = None;
            }

            MagicChoice::ShieldSlot => self.magic_pay_shield(i, index + 1),
            MagicChoice::EnsnaringSlot => {
                self.magic_pay_ensnaring(i, index + 1);
                if !self.magic_rider_pending(i) {
                    self.resume_magic_rider();
                }
            }
            MagicChoice::CreateSlot => {
                self.bonus = false;
                self.f[i].resource -= [2, 3, 5][index];
                self.f[i].slots[index] += 1;
            }
            MagicChoice::ConvertSlot => {
                let f = &mut self.f[i];
                f.slots[index] -= 1;
                f.resource = (f.resource + index as i32 + 1).min(f.build.level);
            }
            pending @ MagicChoice::Counterspell { source, .. } => {
                if input != Act::Defend {
                    self.finish_counterspell(pending, false);
                    return;
                }
                let responder_slot_turn = self.f[i].slot_turn;
                assert!(self.spend_slot(i, 3));
                self.f[i].reaction = false;
                self.magic_offensive(i);
                self.magic_end_invisibility_for(i);
                self.arcane_ward_cast(i, Spell::Counterspell, 3);
                if self.magic_can_counterspell(source, i) {
                    self.f[i].magic.counter_suspended = Some((pending, responder_slot_turn));
                    self.magic_request(source, MagicChoice::CounterResponse { responder: i });
                } else {
                    let negate = !self.save(source, CON, self.spell_dc(i));
                    self.finish_counterspell(pending, negate);
                }
            }
            MagicChoice::CounterResponse { responder } => {
                let (pending, prior) = self.f[responder]
                    .magic
                    .counter_suspended
                    .take()
                    .expect("suspended Counterspell");
                let first_counter_ends = if input == Act::Defend {
                    assert!(self.spend_slot(i, 3));
                    self.f[i].reaction = false;
                    self.magic_offensive(i);
                    self.arcane_ward_cast(i, Spell::Counterspell, 3);
                    !self.save(responder, CON, self.spell_dc(i))
                } else {
                    false
                };
                if first_counter_ends {
                    self.f[responder].slots[2] += 1;
                    self.f[responder].slot_turn = prior;
                    self.finish_counterspell(pending, false);
                } else {
                    let negate = !self.save(i, CON, self.spell_dc(responder));
                    self.finish_counterspell(pending, negate);
                }
            }
            MagicChoice::Damage(mut damage) => {
                if input == Act::Defend {
                    damage.amount = (damage.amount - self.magic_cut_damage(i)).max(0);
                }
                damage.checked |= 1 << i;
                self.offer_spell_damage(damage);
            }
        }
    }

    pub(super) fn magic_decision_label(&self, i: usize, phase: i32, a: Act) -> Option<String> {
        let f = &self.f[i];
        let index = a as usize;
        let choice = f.magic.choice.filter(|c| c.phase() == phase)?;
        if a == Act::Pass
            && !matches!(
                choice,
                MagicChoice::Direction(_) | MagicChoice::CubeFace(_) | MagicChoice::Guidance(_) | MagicChoice::EnhanceAbility(_)
            )
        {
            return Some("Pass".into());
        }
        Some(
            match choice {
                MagicChoice::CubeFace(_) => {
                    let [right, up] = super::targeting::cube_face_offset(index);
                    return Some(format!("Shift cube face right {:+.1} ft, up {:+.1} ft",
                        f32::from(right) * 1.25, f32::from(up) * 1.25));
                }
                MagicChoice::Direction(_) => {
                    const FLAT: [&str; 8] = [
                        "North",
                        "North-East",
                        "East",
                        "South-East",
                        "South",
                        "South-West",
                        "West",
                        "North-West",
                    ];
                    return Some(match index {
                        0..=7 => FLAT[index].to_owned(),
                        8 => "Up".into(),
                        9 => "Down".into(),
                        10..=17 => format!("{} and up", FLAT[index - 10]),
                        18..=25 => format!("{} and down", FLAT[index - 18]),
                        _ => return None,
                    });
                }
                MagicChoice::ShillelaghType(_) => [
                    "Spellcasting ability, weapon damage", "Spellcasting ability, Force damage",
                    "Strength, weapon damage", "Strength, Force damage",
                ].get(index)?,
                MagicChoice::DivineSparkType => ["Radiant damage", "Necrotic damage"].get(index)?,
                MagicChoice::Element(action) => spell(f.build.class,action)?.element_choices().get(index)?.1,
                MagicChoice::Form(_) => FORMS.get(index)?,
                MagicChoice::Command(_) => return WORDS.get(index).map(|word| format!("{word:?}")),
                MagicChoice::Guidance(_) => return GUIDANCE_SKILLS.get(index).map(|skill| format!("{skill:?}")),
                MagicChoice::AlarmMode { .. } => {
                    return ["Mental alarm", "Audible alarm"].get(index).map(|mode| (*mode).into());
                }
                MagicChoice::AlarmExempt { .. } => return Some(self.alarm_exemption_name(i, index)),
                MagicChoice::EnhanceAbility(_) => {
                    return ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"]
                        .get(index).map(|ability| format!("Enhance {ability}"));
                }
                MagicChoice::TrueStrikeWeapon(_) => ["Strike with melee weapon", "Strike with ranged weapon", "Throw held weapon"].get(index)?,
                MagicChoice::WeaponEnchantment(_) => {
                    let (issuer,ranged) = self.enchantment_item(i,index);
                    return Some(format!("Enchant {} (issued to seat {})", self.gear_weapon_kind_for(issuer,ranged)?.name(),issuer+1));
                }
                MagicChoice::TrueStrikeType(_) => ["Weapon's normal damage", "Radiant damage"].get(index)?,
                MagicChoice::RecoverBard | MagicChoice::RecoverDruid => {
                    return Some(format!("Spend level {} spell slot", index + 1))
                }
                MagicChoice::WildShape => WILD_FORMS.get(index)?.name,
                MagicChoice::LightTarget(_) => return self.magic_light_target_label(i,index),
                MagicChoice::LightColor { .. } => LIGHT_HUES.get(index)?,
                MagicChoice::SpiritualStrike => {
                    if index >= 16 {
                        let point = self.spiritual_guess(i, index)?;
                        return Some(format!(
                            "Guess Spiritual Weapon strike at {}{} ({} ft)",
                            char::from(b'A' + point.cell.x as u8), point.cell.y + 1, point.altitude * 5
                        ));
                    }
                    if index < 2 { if index == i { "Strike yourself" } else { "Strike opponent" } }
                    else if index == i+2 { "Strike your companion" } else { "Strike opposing companion" }
                }
                MagicChoice::Allies(action) => return self.magic_ally_choice_label(i, action, a),
                MagicChoice::HeightenedTarget { .. } => return Some(format!("Heighten against {}", self.group_target_label(i,1 << index).trim_start_matches("Target "))),
                MagicChoice::GroupTargets(_) => return Some(self.group_target_label(i,(index - 32) as u8)),

                MagicChoice::AllyConsent { .. } => return self.magic_ally_consent_label(i, a),

                MagicChoice::CureRoute(_) => ["Touch target directly", "Deliver through your familiar"].get(index)?,
                MagicChoice::Creature(Act::Entity11) => {
                    let target = if index < 2 {
                        format!("{:?}",self.f.get(index)?.build.class)
                    } else {
                        self.entity(index)?.kind.name().to_owned()
                    };
                    return Some(format!("Free {target}"));
                }
                MagicChoice::Creature(_) => {
                    return Some(match index {
                        0 | 1 if index == i => "Target self".into(),
                        0 | 1 => "Target opponent".into(),
                        2 | 3 if index == i + 2 => {
                        if matches!(choice, MagicChoice::Creature(action) if self.creature_menu_spell(i,action) == Spell::CureWounds)
                            && !self.visible_ally(i, 1)
                        {
                            "Target allied creature via familiar".into()
                        } else { "Target allied creature".into() }
                    },
                        2 | 3 => "Target opposing creature".into(),
                        _ => return None,
                    });
                }
                MagicChoice::Dispel(_) => return self.dispel_target_label(i, index),
                MagicChoice::HexAbility => [
                    "Strength",
                    "Dexterity",
                    "Constitution",
                    "Intelligence",
                    "Wisdom",
                    "Charisma",
                ]
                .get(index)?,
                MagicChoice::MarkCost => {
                    ["Spend Favored Enemy use", "Spend spell slot"].get(index)?
                }
                MagicChoice::MetaMode => {
                    ["Quickened Spell (2 points)", "Heightened Spell (2 points)"].get(index)?
                }
                MagicChoice::MetaSpell(_) => self.prepared_spell(i, a)?.label(),
                MagicChoice::EnsnaringSlot => return Some(format!("Cast Ensnaring Strike with level {} slot", index + 1)),
                MagicChoice::ShieldSlot => return Some(format!("Cast Shield with level {} slot", index + 1)),
                MagicChoice::CastSlot { action, .. } => {
                    let spell = self.prepared_spell(i, action)?.label();
                    return Some(if a == Act::Rider {
                        "Cast Misty Step using Steps of the Fey".into()
                    } else { format!("Cast {spell} with level {} slot", index + 1) });
                }
                MagicChoice::CreateSlot => {
                    return Some(format!(
                        "Create level {} spell slot ({} points)",
                        index + 1,
                        [2, 3, 5].get(index)?
                    ))
                }
                MagicChoice::ConvertSlot => {
                    return Some(format!(
                        "Convert level {} spell slot to {} Sorcery Points",
                        index + 1,
                        index + 1
                    ))
                }
                MagicChoice::CounterResponse { .. } => return Some("Counter the opposing Counterspell".into()),
                MagicChoice::Counterspell { spell, .. } => {
                    return Some(format!("Counterspell against {}", spell.label()))
                }
                MagicChoice::Damage(damage) => return Some(format!("Cutting Words against {} rolled damage",damage.amount)),
            }
            .to_owned(),
        )
    }

    pub(super) fn magic_can_cut_damage(&self, bard: usize, source: usize) -> bool {
        let f = &self.f[bard];
        f.build.class == Class::Bard
            && f.build.variant == 0
            && f.build.level >= 3
            && f.resource > 0
            && bard != source
            && self.can_react(bard)
            && self.magic_visible(bard, 12)
    }

    pub(super) fn magic_cut_damage(&mut self, i: usize) -> i32 {
        self.f[i].reaction = false;
        self.f[i].resource -= 1;
        self.roll(if self.f[i].build.level >= 5 { 8 } else { 6 })
    }

    pub(in crate::arena2) fn divine_spray_damage(&mut self, source: usize, target: usize, amount: i32) {
        self.offer_spell_damage(SpellDamage {
            source,
            emitter: None,
            amount,
            kind: DamageType::Cold,
            recipients: 1 << target,
            saved: 0,
            checked: 0,
            rider: SpellRider::SeaPush(target),
            objects: None,
        });
    }

    fn offer_spell_damage(&mut self, damage: SpellDamage) {
        let observer = 1 - damage.source;
        if damage.amount > 0
            && damage.checked & (1 << observer) == 0
            && self.area_can_cut_packet(observer, damage)
        {
            self.magic_request(observer, MagicChoice::Damage(damage));
            return;
        }
        let emitter = damage.emitter.unwrap_or(damage.source);
        if let Some((center, area)) = damage.objects {
            let tracked = self.magic_ward_source(emitter).is_some().then(|| {
                (super::objects::FIRST_OBJECT..super::objects::END_OBJECTS)
                    .filter_map(|id| self.object(id).map(|object| (id, object.hp)))
                    .collect::<Vec<_>>()
            });
            self.damage_objects_in(center, area, damage.amount, damage.kind);
            if tracked.is_some_and(|objects| {
                objects.into_iter().any(|(id, hp)| self.object(id).is_none_or(|object| object.hp < hp))
            }) {
                self.magic_offensive(emitter);
            }
        }
        for target in [1 - damage.source, damage.source, 2, 3] {
            if damage.recipients & (1 << target) == 0 {
                continue;
            }
            let amount = if damage.saved & (1 << target) != 0 {
                damage.amount / 2
            } else {
                damage.amount
            };
            if target < 2 || self.entity(target).is_some() {
                self.apply_spell_damage_emitted_by(
                    damage.source,
                    emitter,
                    target,
                    amount,
                    damage.kind,
                );
            }
        }
        match damage.rider {
            SpellRider::ArmsOfHadar => {
                for target in (0..4).filter(|&target| damage.recipients & !damage.saved & (1 << target) != 0) {
                    if target < 2 {
                        self.f[target].reaction = false;
                    } else if let Some(entity) = self.entity_mut(target) {
                        entity.reaction = false;
                    }
                }
            }
            SpellRider::Mockery(target) if target < 2 && self.f[target].hp > 0 => {
                self.f[target].attack_disadvantage = true;
                self.f[target].magic.mocked_until = self.turns + 1;
            }
            SpellRider::Whispers(target) if self.valid_mark_target(target) => {
                self.start_whispers_flee(damage.source, target);
            }
            SpellRider::Thunderwave(origin) => {
                self.area_thunderwave_push(origin, damage.recipients & !damage.saved);
            }
            SpellRider::SeaPush(target) => self.divine_offer_spray_push(damage.source, target),
            SpellRider::LandHeal(target, amount) if target < 2 && self.f[target].hp > 0 => {
                self.heal(target, amount);
            }
            SpellRider::LandHeal(target, amount) if target >= 2 => self.entity_heal(target, amount),
            _ => {}
        }
    }

    pub(super) fn magic_max_hp(&self, i: usize) -> i32 {
        self.f[i].build.hp + self.f[i].magic.aid_hp
    }

    pub(super) fn magic_attack_disadvantage(&self, i: usize) -> bool {
        self.magic_poisoned(i) || self.magic_frightened(i)
    }

    pub(in crate::arena2) fn blur_disadvantage(&self, attacker: usize, target: usize) -> bool {
        if target >= 2 || self.f[target].concentration != Some(Concentration::Blur) {
            return false;
        }
        let senses = if attacker < 2 {
            self.entity_shared_blindsight(attacker, target)
        } else {
            self.entity(attacker).is_some_and(|e| {
                e.kind.stats().blindsight > 0
                    && self
                        .combatant_distance(attacker, target)
                        .is_some_and(|d| d <= e.kind.stats().blindsight)
                    && self.combatant_los(attacker, target)
            })
        };
        !senses
    }

    fn magic_frightened(&self, i: usize) -> bool {
        !self.arcane_emotions_immune(i) && self.fear_source(i)
            .or_else(|| self.bard_ranger_fear_source(i))
            .is_some_and(|source| self.can_see(i, source))
    }

    pub(super) fn magic_poisoned(&self, i: usize) -> bool {
        self.f[i].magic.poisoned_turn == self.turns || self.magic_area_poisoned(i)
    }

    pub(super) fn magic_quiet_turn(&self, i: usize) -> bool {
        self.magic_poisoned(i) || self.f[i].magic.magic_restricted_turn == self.turns
    }

    fn apply_spell_damage(&mut self, i: usize, target: usize, amount: i32, kind: DamageType) {
        self.apply_spell_damage_emitted_by(i, i, target, amount, kind);
    }

    fn apply_spell_damage_emitted_by(
        &mut self,
        caster: usize,
        emitter: usize,
        target: usize,
        amount: i32,
        kind: DamageType,
    ) {
        if !self.rope_same_space(emitter, target) {
            return;
        }
        let before = self.footprint(target);
        let hp = |battle: &Self| {
            if target < 2 {
                battle.f[target].hp + battle.f[target].temp
            } else {
                battle.entity(target).map_or(0, |e| e.hp + e.temp_hp)
            }
        };
        let old_hp = hp(self);
        self.damage_typed(target, amount, kind);
        if hp(self) < old_hp {
            self.magic_offensive(emitter);
        }
        self.magic_hurt_from(emitter, target, amount);
        self.magic_blessing_at_zero(Some(caster), target, before);
    }

    fn meta_spell_mask(&self, i: usize, meta: Metamagic) -> u128 {
        let f = &self.f[i];
        if f.resource < 2 {
            return 0;
        }
        SPELL_ACTIONS
            .iter()
            .chain(&SPECIALS)
            .filter_map(|&action| {
                let spell = self.prepared_spell(i, action)?;
                if !self.spell_slot(i, spell, f.magic.slot_floor).is_some() {
                    return None;
                }
                let ready = match meta {
                    Metamagic::Quickened => {
                        !spell.bonus()
                            && f.slot_turn != self.turns
                            && self.magic_spell_ready_as(i, spell, true)
                    }
                    Metamagic::Heightened => {
                        matches!(
                            spell,
                            Spell::BurningHands
                                | Spell::Shatter
                                | Spell::HoldPerson
                                | Spell::Thunderwave
                                | Spell::Fireball
                                | Spell::Fear
                                | Spell::StinkingCloud
                                | Spell::Command
                        ) && self.magic_spell_ready(i, spell)
                    }
                };
                ready.then_some(action.bit())
            })
            .fold(0, |mask, bit| mask | bit)
    }

    pub(super) fn magic_ability_disadvantage(&self, i: usize, stat: usize) -> bool {
        self.magic_poisoned(i) || self.magic_frightened(i) || self.magic_hex_disadvantage(i, stat)
    }

    pub(in crate::arena2) fn magic_hex_disadvantage(&self, id: usize, stat: usize) -> bool {
        (0..2).any(|source| {
            self.f[source].concentration == Some(Concentration::Hex)
                && self.magic_mark_target(source) == Some(id)
                && self.f[source].magic.hex_ability == stat
        })
    }

    pub(super) fn magic_concentration_advantage(&self, i: usize) -> bool {
        self.f[i].build.class == Class::Warlock
    }
}

impl Battle {
    fn casting_components(&self, i: usize, spell: Spell) -> (bool, bool, super::equipment::Material) {
        use Spell::*;
        let (verbal, somatic, material) = spell.components();
        if self.f[i].build.level >= 3 && self.is_subclass(i, Class::Warlock, 2)
            && matches!(spell, DissonantWhispers | Hex | Bane | HoldPerson | Suggestion | HideousLaughter | PhantasmalForce)
        {
            (false, false, material)
        } else {
            (verbal, somatic, material)
        }
    }

    fn magic_components(&self, i: usize, spell: Spell) -> bool {
        let (v, s, m) = self.casting_components(i, spell);
        self.can_cast_components(i, v, s, m)
    }
}

impl Spell {
    fn components(self) -> (bool, bool, super::equipment::Material) {
        use super::equipment::Material;
        use Spell::*;
        match self {
            MindSliver | FaerieFire | ViciousMockery | DissonantWhispers | HealingWord | MassHealingWord | HunterMark
            | Command | MistyStep | EnsnaringStrike | Blur | CrusadersMantle => (true, false, Material::None),
            HypnoticPattern => (false, true, Material::Ordinary),
            Counterspell => (false, true, Material::None),
            FogCloud => (true, true, Material::None),
            Alarm => (true, true, Material::Ordinary),
            Nondetection => (true, true, Material::DiamondDust25),
            TrueStrike => (false, true, Material::Weapon1cp),
            Light => (true, false, Material::Ordinary),
            ChromaticOrb => (true, true, Material::Diamond50),
            Revivify => (true, true, Material::Diamond300),
            Bless => (true, true, Material::HolySymbol5),
            Suggestion => (true, false, Material::Ordinary),
            Protection => (true, true, Material::HolyWater25),
            HungerOfHadar | Shillelagh | Hex | Shatter | Moonbeam | SpiritGuardians | Fireball | Aid
            | DragonBreath | Fear | Fly | StinkingCloud | ShieldOfFaith | FiendishVigor
            | Longstrider | Jump | Bane | MageArmor | Sanctuary | HoldPerson | Mending | HideousLaughter
            | GustOfWind | LightningBolt | Invisibility | PassWithoutTrace | EnhanceAbility
            | PhantasmalForce => {
                (true, true, Material::Ordinary)
            }
            _ => (true, true, Material::None),
        }
    }
}

impl Battle {
    fn wild_form(&self, i: usize) -> Option<&'static WildForm> {
        self.f[i]
            .magic
            .form
            .then_some(&WILD_FORMS[self.f[i].magic.form_id])
    }

    pub(super) fn magic_weapon_dice(&self, i: usize) -> i32 {
        self.wild_form(i).map_or(1, |form| form.count)
    }

    pub(super) fn magic_extra_attack(&self, _i: usize) -> i32 {
        0
    }

    pub(super) fn magic_climb_speed(&self, i: usize) -> i32 {
        self.wild_form(i).map_or(0, |form| form.climb)
    }

    pub(super) fn magic_agile(&self, i: usize) -> bool {
        self.wild_form(i).is_some_and(|form| form.name == "Rat")
    }

    pub(super) fn magic_resists_poison(&self, i: usize) -> bool {
        self.wild_form(i)
            .is_some_and(|form| form.name == "Giant Badger")
    }
}

impl Battle {
    fn spell_unlock_level(&self, i: usize, spell: Spell) -> i32 {
        if let Some(level) = divine_spells::unlock(self.f[i].build, spell) {
            return level;
        }
        use Class::*;
        use Spell::*;
        if spell == MistyStep {
            if self.is_subclass(i, Warlock, 1) { return 3; }
            if self.is_subclass(i, Wizard, 1) { return 4; }
        }
        match (self.f[i].build.class, spell) {
            (Bard, Command)
            | (Druid | Ranger, Entangle)
            | (Wizard | Sorcerer, Thunderwave)
            | (Cleric, ShieldOfFaith)
            | (Sorcerer, MagicMissile)
            | (Paladin, Bless | CureWounds)
            | (Warlock, ExpeditiousRetreat | FiendishVigor) => 2,
            (Paladin, ShieldOfFaith)
            | (Cleric | Paladin | Sorcerer | Warlock, Command)
            | (Cleric | Sorcerer | Warlock, HoldPerson)
            | (Druid, Blur | FireBolt | BurningHands)
            | (Sorcerer, BurningHands | AlterSelf | DragonBreath)
            | (Warlock, BurningHands | ScorchingRay | Suggestion)
            | (Ranger, Jump) => 3,
            (Bard | Druid, HoldPerson)
            | (Cleric, Sanctuary | Light)
            | (Druid, Resistance)
            | (Warlock, MistyStep)
            | (_, TrueStrike)
            | (Wizard, MagicWeapon)
            | (Ranger, EnsnaringStrike)
            | (Sorcerer | Wizard, Shatter)
            | (Paladin, Heroism) => 4,
            (Wizard, MistyStep)
            | (Paladin, Aid | ZoneOfTruth | MagicWeapon)
            | (Ranger, MagicWeapon)
            | (
                _,
                Fireball | Fear | Fly | StinkingCloud | DispelMagic | ProtectionFromEnergy
                | Counterspell,
            ) => 5,
            _ => 1,
        }
    }

    pub(super) fn magic_restrained(&self, i: usize) -> bool {
        self.divine_restrained(i) || self.magic_spell_restrained(i)
    }

    fn magic_spell_restrained(&self, i: usize) -> bool {
        self.magic_spell_area_restrained(i)
            || self.f[i].magic.restrained
                && matches!(
                    self.f[1 - i].concentration,
                    Some(Concentration::Entangle | Concentration::EnsnaringStrike)
                )
    }

    pub(super) fn magic_paralyzed(&self, i: usize) -> bool {
        self.f[i].magic.held && self.f[1 - i].concentration == Some(Concentration::HoldPerson)
    }

    pub(super) fn magic_weapon_bonus(&self, i: usize, ranged: bool) -> i32 {
        let issuer = self.gear_weapon_owner(i, ranged);
        self.f
            .iter()
            .filter(|source| {
                source.magic.spell_weapon_until > self.turns
                    && source.magic.spell_weapon == Some((issuer, ranged))
            })
            .map(|source| source.magic.spell_weapon_bonus)
            .max()
            .unwrap_or(0)
    }

    fn enchantment_item(&self, i: usize, choice: usize) -> (usize, bool) {
        (if choice < 2 { i } else { 1 - i }, choice % 2 != 0)
    }

    fn enchantment_choices(&self, i: usize) -> u128 {
        (0..4)
            .filter(|&choice| {
                let (issuer, ranged) = self.enchantment_item(i, choice);
                if self.gear_weapon_kind_for(issuer, ranged).is_none()
                    || self.magic_enchanted_weapon(issuer, ranged)
                {
                    return false;
                }
                if self.gear_wielded_by(i, issuer, ranged) {
                    return true;
                }
                if self.gear_wielded_by(1 - i, issuer, ranged)
                    && self.can_see(i, 1 - i)
                    && self.dist() <= 1
                {
                    return true;
                }
                self.observed_object(i, 4 + issuer + 2 * usize::from(ranged))
                    .is_some_and(|object| self.fighter_point(i).dist(object.point) <= 1)
            })
            .fold(0, |mask, choice| mask | ACTS[choice].bit())
    }

    pub(super) fn magic_jump_active(&self, i: usize) -> bool {
        self.f[i].magic.jump_until > self.turns
    }

    pub(super) fn magic_guidance(&mut self, i: usize, skill: super::origins::Skill) -> i32 {
        if self
            .magic_buff_source(i, Concentration::Guidance)
            .is_some_and(|source| self.f[source].magic.guidance_skill == skill)
        {
            self.roll(4)
        } else {
            0
        }
    }

    pub(super) fn magic_ward_source(&self, target: usize) -> Option<usize> {
        if target < 2 {
            let magic = &self.f[target].magic;
            (magic.sanctuary_until > self.turns)
                .then_some(magic.sanctuary_source.unwrap_or(target))
        } else {
            let (source, expires) = self.entity(target)?.sanctuary?;
            (expires > self.turns).then_some(source)
        }
    }

    pub(super) fn magic_energy_resistance(&self, i: usize, kind: DamageType) -> bool {
        self.magic_buff_source(i, Concentration::ProtectionFromEnergy)
            .is_some_and(|source| self.f[source].magic.protected_type == kind)
    }

    pub(super) fn magic_flat_protection(&mut self, i: usize, amount: i32, kind: DamageType) -> i32 {
        let Some(source) = self.magic_buff_source(i, Concentration::Resistance) else {
            return amount;
        };
        let f = &mut self.f[source];
        if amount > 0 && f.magic.protected_type == kind && f.magic.resistance_tick != self.turns {
            f.magic.resistance_tick = self.turns;
            (amount - self.roll(4)).max(0)
        } else {
            amount
        }
    }

    pub(super) fn magic_no_reactions(&self, id: usize) -> bool {
        id < 4
            && self
                .f
                .iter()
                .any(|f| f.magic.shocking_targets & (1 << id) != 0)
    }

    pub(super) fn magic_reset_opportunity(&mut self, id: usize) {
        for source in &mut self.f {
            source.magic.shocking_targets &= !(1 << id);
        }
    }

    fn resolve_added_spell(&mut self, i: usize, spell: Spell, tier: usize, heightened: bool) {
        use Spell::*;
        let target = 1 - i;
        match spell {
            Alarm => self.finish_alarm(i),
            FogCloud => self.concentrate(i, Concentration::FogCloud, 600, Some(self.chosen_center()), tier),
            GustOfWind => self.start_gust_of_wind(i, tier),
            LightningBolt => {
                let area = Area::Line { length: 20, width: 1, direction: self.f[i].magic.direction };
                self.ignite_objects_in(self.fighter_point(i), area);
                self.directional_area_damage(i, DEX, 5 + tier as i32, 6, DamageType::Lightning,
                    heightened, area, SpellRider::None);
            }
            PassWithoutTrace => self.concentrate(i, Concentration::PassWithoutTrace, 600, None, tier),
            CrusadersMantle => self.concentrate(i, Concentration::CrusadersMantle, 10, None, tier),
            CharmPerson => {
                if !self.save_advantage(target, WIS, self.spell_dc(i), true) {
                    self.f[i].bard_ranger.person_charm[target] = self.turns + 1200;
                    self.f[i].bard_ranger.person_charm_tier[target] = tier;
                }
            }
            MirrorImage => {
                self.f[i].bard_ranger.mirrors = 3;
                self.f[i].bard_ranger.mirror_until = self.turns + 20;
                self.f[i].bard_ranger.mirror_tier = tier;
            }
            RopeTrick => self.cast_rope_trick(i, tier),
            DisguiseSelf => {
                self.f[i].bard_ranger.disguise_until = self.turns + 1200;
                self.f[i].bard_ranger.disguise_tier = tier;
            }
            Longstrider => self.f[i].magic.longstrider_until = self.turns + 1_200,
            Jump => self.f[i].magic.jump_until = self.turns + 20,
            EnsnaringStrike => unreachable!("requires a confirmed weapon hit"),
            Heroism => {
                self.concentrate(i, Concentration::Heroism, 10, None, tier);
            }
            Guidance => self.concentrate(i, Concentration::Guidance, 10, None, 0),
            Resistance => {
                self.f[i].magic.protected_type = self.f[i].magic.element;
                self.f[i].magic.resistance_tick = -1;
                self.concentrate(i, Concentration::Resistance, 10, None, 0);
            }
            ProtectionFromEnergy => {
                self.f[i].magic.protected_type = self.f[i].magic.element;
                self.concentrate(i, Concentration::ProtectionFromEnergy, 600, None, tier);
            }
            Sanctuary => self.f[i].magic.sanctuary_until = self.turns + 20,
            MageArmor => self.f[i].magic.mage_armor_until = self.turns + 9_600,
            MagicWeapon => {
                self.f[i].magic.spell_weapon_until = self.turns + 1_200;
                self.f[i].magic.spell_weapon_bonus = if tier >= 3 { 2 } else { 1 };
                self.f[i].magic.spell_weapon =
                    Some(self.enchantment_item(i, self.f[i].magic.aux_choice));
            }
            Bane => {
                self.f[i].concentration = None;
                let mut cursed = 0;
                let dc = self.spell_dc(i);
                for id in 0..4 {
                    if self.f[i].magic.cast_targets & (1 << id) != 0 {
                        let saved = if id < 2 {
                            self.save_advantage(
                                id,
                                CHA,
                                dc,
                                heightened && self.magic_heightened_for(i, id),
                            )
                        } else {
                            self.entity_save(id, CHA, dc)
                        };
                        if !saved {
                            cursed |= 1 << id;
                        }
                    }
                }
                if cursed != 0 {
                    self.concentrate(i, Concentration::Bane, 10, None, tier);
                    self.f[i].magic.bane_recipients = cursed;
                }
            }
            ExpeditiousRetreat => {
                self.concentrate(i, Concentration::ExpeditiousRetreat, 10, None, tier);
                self.dash += self.speed(i);
            }
            HoldPerson => {
                self.f[i].concentration = None;
                let dc = self.spell_dc(i);
                let recipients = self.f[i].magic.cast_targets;
                let held = [1 - i, i].map(|id| {
                    (recipients & (1 << id) != 0)
                        && !self.save_advantage(
                            id,
                            WIS,
                            dc,
                            heightened && self.magic_heightened_for(i, id),
                        )
                });
                if held[0] && !held[1] {
                    self.concentrate(i, Concentration::HoldPerson, 10, None, tier);
                    self.f[target].concentration = None;
                    self.f[target].magic.held = true;
                    self.magic_end_wild_shape(target);
                }
            }
            Entangle => self.cast_entangle(i, tier, heightened),
            DispelMagic => self.resolve_dispel_target(i, self.f[i].magic.aux_choice, tier),
            MistyStep => {
                let p = self.chosen_center();
                if (0..4).all(|id| {
                    self.footprint(id)
                        .is_none_or(|body| !body.squares().any(|square| square == p))
                }) {
                    let departure = self.fighter_point(i);
                    self.commit_forced_move(i, p);
                    self.arcane_fey_step(i, departure);
                }
            }
            Elementalism => self.resolve_elementalism(i),
            Light => self.f[i].magic.light_until = self.turns + 1_200,
            FiendishVigor => self.grant_temporary_hp(i, 12, TempSource::FiendishVigor),
            Counterspell | Shield => unreachable!("requires reaction"),
            _ => unreachable!("unexpected spell {spell:?}"),
        }
    }
}

impl Battle {
    /// Actor-private menu context. Indices 0–15 one-hot encode phase IDs 34–49.
    /// Indices 16–23 encode action ID, current tier, resource, target seat,
    /// pending damage, modifier mode, current form ID, and concentration tier.
    /// Outside a cast/damage menu every component is zero.
    pub fn magic_decision_features(&self, viewer: usize) -> [f32; 24] {
        let mut row = [0.0; 24];
        if self.actor() != Some(viewer) {
            return row;
        }
        let f = &self.f[viewer];
        let Some(choice) = f.magic.choice else {
            return row;
        };
        if self.phase_id() != choice.phase() {
            return row;
        }
        row[(choice.phase() - 34) as usize] = 1.0;
        let action = match choice {
            MagicChoice::Direction(action)
            | MagicChoice::CubeFace(action)
            | MagicChoice::Element(action)
            | MagicChoice::Form(action)
            | MagicChoice::Command(action)
            | MagicChoice::Dispel(action)
            | MagicChoice::CureRoute(action)
            | MagicChoice::TrueStrikeWeapon(action)
            | MagicChoice::TrueStrikeType(action)
            | MagicChoice::Creature(action)
            | MagicChoice::GroupTargets(action)
            | MagicChoice::HeightenedTarget { action, .. }
            | MagicChoice::LightTarget(action)
            | MagicChoice::LightColor { action, .. }
            | MagicChoice::Allies(action)
            | MagicChoice::AllyConsent { action, .. }
            | MagicChoice::CastSlot { action, .. } => Some(action),
            MagicChoice::HexAbility | MagicChoice::MarkCost => Some(Act::Spell1),
            MagicChoice::WildShape => Some(Act::Special1),
            _ => None,
        };
        row[16] = action.map_or(0.0, |a| (a as usize + 1) as f32 / ACTS.len() as f32);
        row[17] = f.magic.cast_tier.unwrap_or(f.magic.slot_floor) as f32 / 3.;
        row[18] = (f.resource as f32 / 5.0).clamp(0.0, 1.0);
        if let MagicChoice::Damage(damage) = choice {
            let source = crate::arena2::target_ids::relative_id(viewer, damage.source);
            row[19] = (source + 1) as f32 / 2.0;
            row[20] = (damage.amount as f32 / 50.0).min(1.0);
        }
        row[21] = match choice {
            MagicChoice::CubeFace(_) => (f.magic.direction + 1) as f32 / 26.0,
            MagicChoice::TrueStrikeType(_) => {
                if f.magic.true_strike_thrown {
                    1.0
                } else if f.magic.true_strike_ranged {
                    2.0 / 3.0
                } else {
                    1.0 / 3.0
                }
            }
            MagicChoice::CreateSlot | MagicChoice::MetaSpell(Metamagic::Quickened) => 0.5,
            MagicChoice::ConvertSlot | MagicChoice::MetaSpell(Metamagic::Heightened) => 1.0,
            _ => 0.0,
        };
        row[22] = if f.magic.form {
            (f.magic.form_id + 1) as f32 / WILD_FORMS.len() as f32
        } else {
            0.0
        };
        row[23] = f.magic.field_level as f32 / 3.0;
        row
    }
}

impl Battle {
    pub(super) fn magic_end_mage_armor(&mut self, i: usize) {
        self.f[i].magic.mage_armor_until = 0;
    }

    fn magic_can_counterspell(&self, caster: usize, source: usize) -> bool {
        let f = &self.f[caster];
        (f.build.level >= 5 && (matches!(f.build.class, Class::Sorcerer | Class::Warlock)
            || self.is_subclass(caster, Class::Wizard, 1)))
            && self.rope_same_space(caster, source)
            && !self.suggestion_charmed_by(caster, source)
            && self.can_react(caster)
            && self.can_slot(caster, 3)
            && self.dist() <= 12
            && self.can_see(caster, source)
            && self.can_cast_components(caster, false, true, super::equipment::Material::None)
    }
}

impl Spell {
    fn is_projectile(self) -> bool {
        matches!(
            self,
            Spell::MagicMissile
                | Spell::EldritchBlast
                | Spell::ScorchingRay
                | Spell::ChromaticOrb
                | Spell::StarryWisp
                | Spell::FireBolt
                | Spell::GuidingBolt
                | Spell::RayOfFrost
                | Spell::ShockingGrasp
                | Spell::PoisonSpray
                | Spell::TrueStrike
        )
    }

    fn chooses_creature(self) -> bool {
        use Spell::*;
        matches!(
            self,
            MindSliver | ViciousMockery
                | SacredFlame
                | DissonantWhispers
                | Suggestion
                | Hex
                | HunterMark
                | HealingWord
                | CureWounds
                | CharmPerson
                | PhantasmalForce
        )
    }

    fn harmful_creature_target(self) -> bool {
        use Spell::*;
        matches!(
            self,
            MindSliver | ViciousMockery | SacredFlame | DissonantWhispers | Suggestion | Hex | HunterMark
                | Bane | Command | HoldPerson | CharmPerson | HideousLaughter | PhantasmalForce
        )
    }
}

impl Battle {
    fn creature_menu_spell(&self, i: usize, action: Act) -> Spell {
        if action == Act::Magic24 {
            return if self.f[i].build.class == Class::Warlock {
                Spell::Hex
            } else {
                Spell::HunterMark
            };
        }
        self.prepared_spell(i, action).expect("selected creature spell")
    }

    fn visible_ally(&self, i: usize, range: i32) -> bool {
        self.entity_position(i + 2)
            .is_some_and(|p| self.f[i].pos.dist(p) <= range && self.entity_visible(i, i + 2))
    }

    fn creature_choices(&self, i: usize, spell: Spell) -> u128 {
        let range = if spell == Spell::CureWounds {
            1
        } else if spell.needs_healing() {
            12
        } else {
            spell.range().unwrap_or(1)
        };
        (0..4)
            .filter(|&id| {
                if spell.harmful_creature_target() && self.suggestion_charmed_by(i, id)
                {
                    return false;
                }
                if (matches!(spell, Spell::HoldPerson | Spell::CharmPerson) && id >= 2)
                    || (spell == Spell::CharmPerson && id == i)
                {
                    return false;
                }
                if spell == Spell::Suggestion
                    && (id == i
                        || id == i + 2
                        || id >= 2
                            && !self.entity(id).is_some_and(|e| {
                                matches!(e.kind, super::entities::EntityKind::Steed { .. })
                            }))
                {
                    return false;
                }
                if spell == Spell::CureWounds {
                    let (direct, via_familiar) = self.cure_paths(i, id);
                    direct || via_familiar
                } else {
                    self.magic_reaches_known_creature(i, id, range, true)
                }
            })
            .fold(0, |mask, id| mask | ACTS[id].bit())
    }

    fn valid_mark_target(&self, target: usize) -> bool {
        if target < 2 {
            self.f[target].hp > 0
        } else {
            self.entity(target).is_some()
        }
    }

    pub(super) fn magic_offensive(&mut self, i: usize) {
        if i < 2 {
            self.f[i].magic.sanctuary_until = 0;
        } else if let Some(entity) = self.entity_mut(i) {
            entity.sanctuary = None;
        }
    }
}

impl Battle {
    pub(in crate::arena2) fn magic_mark_target_downed(&mut self, target: usize) {
        self.area_end_ensnared_creature(target);
        for fighter in &mut self.f {
            if matches!(
                fighter.concentration,
                Some(Concentration::Hex | Concentration::HunterMark)
            ) && fighter.magic.mark_target == Some(target)
                && fighter.magic.mark_defeat_turn.is_none()
            {
                fighter.magic.mark_defeat_turn = Some(self.turns);
            }
        }
    }

    pub(in crate::arena2) fn magic_forget_replaced_mark(&mut self, id: usize) {
        for fighter in &mut self.f {
            if fighter.magic.mark_target == Some(id)
                && fighter.magic.mark_defeat_turn.is_none()
                && matches!(
                    fighter.concentration,
                    Some(Concentration::Hex | Concentration::HunterMark)
                )
            {
                fighter.concentration = None;
                fighter.magic.mark_target = None;
            }
        }
        self.area_end_ensnared_creature(id);
    }

    pub(super) fn magic_mark_target(&self, i: usize) -> Option<usize> {
        if matches!(
            self.f[i].concentration,
            Some(Concentration::Hex | Concentration::HunterMark)
        ) {
            self.f[i]
                .magic
                .mark_target
                .filter(|_| self.f[i].magic.mark_defeat_turn.is_none())
        } else {
            None
        }
    }
}

impl Battle {
    pub(super) fn magic_rider(&self, i: usize, hit: Hit) -> bool {
        let f = &self.f[i];
        ((f.build.class == Class::Ranger && f.build.level >= 4
            || f.build.is_subclass(Class::Paladin, 1) && f.build.level >= 3)
            && self.owner == i
            && self.bonus
            && hit.weapon
            && !hit.unarmed
            && hit.defender < 4
            && self.magic_components(i, Spell::EnsnaringStrike)
            && self
                .spell_slot(i, Spell::EnsnaringStrike, f.magic.slot_floor)
                .is_some())
            || self.magic_can_repel(hit)
    }

    pub(super) fn magic_followup(&mut self, i: usize, hit: Hit) {
        self.f[i].magic.ensnaring_hit = Some(hit);
        let options = self.castable_slot_mask(i, Spell::EnsnaringStrike);
        if options.count_ones() > 1 {
            self.magic_request(i, MagicChoice::EnsnaringSlot);
        } else {
            self.magic_pay_ensnaring(i, options.trailing_zeros() as usize + 1);
        }
    }

    pub(super) fn magic_rider_pending(&self, i: usize) -> bool {
        self.f[i].magic.ensnaring_hit.is_some()
    }

    fn magic_pay_ensnaring(&mut self, i: usize, tier: usize) {
        self.bonus = false;
        let old_slot_turn = self.f[i].slot_turn;
        assert!(self.spend_slot(i, tier));
        self.reveal(i);
        self.magic_offensive(i);
        if self.magic_can_counterspell(1 - i, i) {
            self.magic_request(
                1 - i,
                MagicChoice::Counterspell {
                    source: i,
                    spell: Spell::EnsnaringStrike,
                    tier,
                    heightened: false,
                    heightened_target: None,
                    center: None,
                    target: None,
                    cure_route: None,
                    targets: 0,
                    old_slot_turn,
                },
            );
        } else {
            self.resolve_ensnaring_hit(i, tier);
        }
    }

    fn resolve_ensnaring_hit(&mut self, i: usize, tier: usize) {
        let target = self.f[i]
            .magic
            .ensnaring_hit
            .take()
            .expect("confirmed Ensnaring Strike hit")
            .defender;
        self.f[i].concentration = None;
        let large = self.combatant_size(target) >= 3;
        let dc = self.spell_dc(i);
        let saved = if target >= 2 {
            self.entity_save_with_advantage(target, 0, dc, large)
        } else {
            self.save_with_cover(target, 0, dc, false, 0, large)
        };
        if !saved {
            self.concentrate(i, Concentration::EnsnaringStrike, 10, None, tier);
            self.area_set_ensnared(i, target);
        }
    }

    pub(super) fn magic_rider_label(&self, i: usize) -> &'static str {
        if matches!(self.f[i].build.class, Class::Ranger | Class::Paladin) {
            "Ensnaring Strike"
        } else {
            "Repelling Blast"
        }
    }
}

impl Battle {
    pub(super) fn magic_unarmed_override(&self, i: usize) -> Option<(i32, i32)> {
        let f = &self.f[i];
        (f.concentration == Some(Concentration::AlterSelf) && f.magic.form_option >= 2)
            .then_some((6, f.build.casting))
    }

    pub(super) fn magic_unarmed_type(&self, i: usize) -> Option<DamageType> {
        if self.f[i].concentration != Some(Concentration::AlterSelf) {
            return None;
        }
        match self.f[i].magic.form_option {
            2 => Some(DamageType::Slashing),
            3 | 4 => Some(DamageType::Piercing),
            5 => Some(DamageType::Bludgeoning),
            _ => None,
        }
    }

    pub(super) fn magic_start_shield(&mut self, i: usize, hit: Hit) -> bool {
        if !matches!(self.f[i].build.class, Class::Wizard | Class::Sorcerer)
            || !self.magic_defend(i, hit)
        {
            return false;
        }
        self.f[i].magic.shield_hit = Some(hit);
        self.magic_request(i, MagicChoice::ShieldSlot);
        true
    }

    fn magic_pay_shield(&mut self, i: usize, tier: usize) {
        let attacker = self.f[i]
            .magic
            .shield_hit
            .expect("shield hit before payment")
            .attacker;
        self.commit_magic_shield(i, tier, attacker);
    }

    pub(super) fn magic_pay_projectile_shield(&mut self, i: usize, tier: usize, attacker: usize) {
        self.f[i].magic.projectile_shield = true;
        self.commit_magic_shield(i, tier, attacker);
    }

    fn commit_magic_shield(&mut self, i: usize, tier: usize, attacker: usize) {
        let prior_slot_turn = self.f[i].slot_turn;
        assert!(self.spend_slot(i, tier));
        self.f[i].reaction = false;
        self.magic_offensive(i);
        self.magic_end_invisibility_for(i);
        self.reveal(i);
        if attacker < 2 && self.magic_can_counterspell(attacker, i) {
            self.magic_request(
                attacker,
                MagicChoice::Counterspell {
                    source: i,
                    spell: Spell::Shield,
                    tier,
                    heightened: false,
                    heightened_target: None,
                    center: None,
                    target: None,
                    cure_route: None,
                    targets: 0,
                    old_slot_turn: prior_slot_turn,
                },
            );
        } else {
            self.finish_magic_shield(i, tier, true);
        }
    }

    fn finish_magic_shield(&mut self, i: usize, tier: usize, applied: bool) {
        if applied {
            self.arcane_ward_cast(i, Spell::Shield, tier);
            self.f[i].shield = 5;
        }
        if std::mem::take(&mut self.f[i].magic.projectile_shield) {
            self.finish_projectile_shield(i, applied);
        } else {
            let hit = self.f[i]
                .magic
                .shield_hit
                .take()
                .expect("pending shield hit");
            self.resume_magic_shield(
                applied && (hit.d20 == 0 || !hit.crit && hit.total < hit.ac + 5),
            );
        }
    }
}

impl Battle {
    /// An observable field exposes geometry independently of the caster.
    /// A creature-bound emanation would expose the caster's exact location,
    /// so its center requires actual sight of that creature.
    pub(in crate::arena2) fn visible_spell_field(
        &self,
        viewer: usize,
        caster: usize,
    ) -> Option<(Point, f32, f32)> {
        let effect = self.f.get(caster)?.concentration?;
        let eye = self.f.get(viewer).map(|_| self.fighter_point(viewer))?;
        let (center, radius, height) = match effect {
            Concentration::Moonbeam => (self.magic_field(caster)?, 1.0, 8.0),
            Concentration::HungerOfHadar | Concentration::StinkingCloud => (self.magic_field(caster)?, 4.0, 0.0),
            Concentration::FogCloud => (self.magic_field(caster)?, self.fog_radius(caster) as f32, 0.0),
            Concentration::SpiritGuardians => {
                if viewer != caster && !self.can_see(viewer, caster) {
                    return None;
                }
                (self.fighter_point(caster), 3.0, 0.0)
            }
            Concentration::Entangle => (self.magic_field(caster)?, 2.0, 0.0),
            Concentration::SpiritualWeapon => (self.magic_field(caster)?, 0.0, 0.0),
            _ => return None,
        };
        (viewer == caster
            || self.map.los_at(eye, center)
                && (matches!(effect, Concentration::StinkingCloud | Concentration::HungerOfHadar | Concentration::FogCloud) || !self.magic_cloud_blocks(eye, center)))
        .then_some((center, radius, height))
    }

    /// Public ongoing spell state for an observed combatant and visible field.
    /// 0–2: concentration ID, time and tier. 3–8: active field, x, y, z,
    /// radius and height. 9–14: form, Aid, flight speed, longstrider, Magic
    /// Weapon and Mage Armor. 15–23: frightened, poisoned, paralyzed,
    /// restrained, no reactions, Sanctuary, truth bound, Jump and Light.
    pub fn magic_state_features(&self, viewer: usize, subject: usize) -> [f32; 24] {
        let mut public = [0.0; 24];
        if let Some((point, radius, height)) = self.visible_spell_field(viewer, subject) {
            public[3..9].copy_from_slice(&[
                1.0,
                point.cell.x as f32 / (super::geometry::WIDTH - 1) as f32,
                point.cell.y as f32 / (super::geometry::HEIGHT - 1) as f32,
                point.altitude as f32 / super::geometry::MAX_ELEVATION as f32,
                radius / 8.0,
                height / 8.0,
            ]);
        }
        if viewer >= 2 || subject >= 2 || viewer != subject && !self.can_see(viewer, subject) {
            return public;
        }
        let f = &self.f[subject];
        let m = &f.magic;
        let legacy = self.magic_features(subject);
        let own = viewer == subject;
        let identified = (!own)
            .then(|| self.study_candidates(viewer))
            .unwrap_or_default();
        let knows = |id| {
            own || identified
                .iter()
                .any(|effect| effect.id == id && self.knows_spell(viewer, *effect))
        };
        let knows_concentration = f
            .concentration
            .is_some_and(|effect| knows(study::concentration_identity(effect)));
        let active = |yes| f32::from(u8::from(yes));
        let remaining = |t: i32| ((t - self.turns).max(0) as f32 / 60.0).min(1.0);
        [
            if knows_concentration { legacy[0] } else { 0.0 },
            if own { legacy[1] } else { 0.0 },
            if own { legacy[9] } else { 0.0 },
            public[3],
            public[4],
            public[5],
            public[6],
            public[7],
            public[8],
            if m.form {
                (m.form_id + 1) as f32 / WILD_FORMS.len() as f32
            } else {
                0.0
            },
            if own {
                (m.aid_hp as f32 / 10.0).min(1.0)
            } else {
                0.0
            },
            if knows(study::concentration_identity(Concentration::Fly)) {
                self.magic_fly_speed(subject) as f32 / 12.0
            } else {
                0.0
            },
            if own {
                remaining(m.longstrider_until)
            } else {
                0.0
            },
            if own {
                remaining(m.spell_weapon_until)
            } else {
                0.0
            },
            if own {
                remaining(m.mage_armor_until)
            } else {
                0.0
            },
            active(self.magic_frightened(subject)),
            active(self.magic_poisoned(subject)),
            active(self.magic_paralyzed(subject)),
            active(self.magic_restrained(subject)),
            active(own && self.magic_no_reactions(subject)),
            if own {
                remaining(m.sanctuary_until)
            } else {
                0.0
            },
            active(own && self.magic_truth_bound(subject)),
            if own { remaining(m.jump_until) } else { 0.0 },
            if own {
                remaining(m.light_until)
            } else {
                active(knows(study::LIGHT))
            },
        ]
    }

    pub fn magic_public_summary(&self, viewer: usize, subject: usize) -> String {
        if viewer >= 2 || subject >= 2 {
            return String::new();
        }
        if viewer == subject {
            return self.magic_summary(subject);
        }
        let field = self.visible_spell_field(viewer, subject);
        let field_id = self.f[subject]
            .concentration
            .map(study::concentration_identity);
        self.study_candidates(viewer)
            .into_iter()
            .map(|effect| {
                let mut text = if self.knows_spell(viewer, effect) {
                    format!("Identified: {}", effect.identity)
                } else {
                    format!("Unidentified effect: {}", effect.clue)
                };
                if field_id == Some(effect.id) {
                    if let Some((point, radius, height)) = field {
                        text.push_str(&format!(
                            " at {}{} ({} ft elevation)",
                            char::from(b'A' + point.cell.x as u8),
                            point.cell.y + 1,
                            point.altitude * 5
                        ));
                        if radius > 0.0 {
                            let measure = if effect.identity == "Entangle" {
                                "half-width"
                            } else {
                                "radius"
                            };
                            text.push_str(&format!(", {} ft {measure}", radius * 5.0));
                        }
                        if height > 0.0 {
                            text.push_str(&format!(", {} ft high", height * 5.0));
                        }
                    }
                }
                text
            })
            .chain(self.alarm_summary(viewer, subject))
            .chain(self.elementalism_summary(viewer, subject))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(super) fn magic_enchanted_weapon(&self, issuer: usize, ranged: bool) -> bool {
        self.f.iter().any(|f| {
            f.magic.spell_weapon_until > self.turns
                && f.magic.spell_weapon == Some((issuer, ranged))
        })
    }
}

impl Battle {
    fn castable_slot_mask(&self, i: usize, spell: Spell) -> u128 {
        if spell.level() == 0
            || spell == Spell::FiendishVigor
            || spell == Spell::HunterMark
                && self.f[i].secondary > 0
                && !self.f[i].magic.force_mark_slot
        {
            return 0;
        }
        let free_fey_step = if spell == Spell::MistyStep && self.f[i].arcane.fey_steps > 0 {
            Act::Rider.bit()
        } else { 0 };
        (spell.level()..=3)
            .filter(|&tier| self.can_slot(i, tier))
            .fold(free_fey_step, |mask, tier| mask | ACTS[tier - 1].bit())
    }
}

impl Battle {
    fn true_strike_weapon_mask(&self, i: usize) -> u128 {
        let modes = [
            self.gear_weapon_kind(i, false).is_some_and(|kind|kind.proficient(self.f[i].build)) && self.gear_wielded(i, false),
            self.gear_weapon_kind(i, true).is_some_and(|kind|kind.proficient(self.f[i].build))
                && self.gear_wielded(i, true)
                && self.f[i].arrows > 0,
            self.true_strike_throw_range(i).is_some(),
        ];
        modes
            .into_iter()
            .enumerate()
            .filter(|&(_, available)| available)
            .fold(0, |mask, (index, _)| mask | ACTS[index].bit())
    }

    fn true_strike_selected_range(&self, i: usize) -> (i32, i32) {
        let magic = &self.f[i].magic;
        if magic.true_strike_thrown {
            self.true_strike_throw_range(i).unwrap_or((0, 0))
        } else {
            self.true_strike_range(i, magic.true_strike_ranged)
        }
    }

    fn select_true_strike_weapon(&mut self, i: usize, index: usize) {
        self.f[i].magic.true_strike_ranged = index == 1;
        self.f[i].magic.true_strike_thrown = index == 2;
    }

    fn magic_shillelagh_active(&self, i: usize) -> bool {
        let magic = &self.f[i].magic;
        magic.shillelagh_until > self.turns
            && magic.shillelagh_weapon == Some(self.gear_weapon_owner(i, false))
            && self.gear_weapon_kind(i, false) == Some(super::weapon::WeaponKind::Quarterstaff)
    }

    pub(super) fn magic_lose_weapon_contact(&mut self, i: usize) {
        self.f[i].magic.shillelagh_until = 0;
    }
}

impl Battle {
    pub(super) fn magic_hurt_from(&mut self, source: usize, target: usize, amount: i32) {
        if amount > 0 {
            self.f[source % 2].bard_ranger.person_charm[target] = 0;
        }
        self.suggestion_hurt_from(source, target, amount);
    }

    pub(super) fn magic_pack_advantage(&self, i: usize, target: usize) -> bool {
        self.wild_form(i).is_some_and(|form| form.name == "Wolf")
            && target != i + 2
            && self
                .entity(i + 2)
                .is_some_and(|ally| !ally.martial.stunned(self.turns))
            && !self.magic_area_incapacitated(i + 2)
            && self
                .combatant_distance(i + 2, target)
                .is_some_and(|d| d <= 1)
    }
}

#[path = "magic_study.rs"]
mod study;

fn compulsion_word(word: CommandWord) -> Compulsion {
    match word {
        CommandWord::Approach => Compulsion::Approach,
        CommandWord::Drop => Compulsion::Drop,
        CommandWord::Flee => Compulsion::Flee,
        CommandWord::Grovel => Compulsion::Grovel,
        CommandWord::Halt => Compulsion::Halt,
    }
}

impl Battle {
    pub(super) fn arcane_heightened_for(&self, caster: usize, target: usize) -> bool {
        self.magic_heightened_for(caster, target)
    }

    fn magic_clear_cast_choices(&mut self, i: usize) {
        let magic = &mut self.f[i].magic;
        magic.ally_selection = 0;
        magic.cast_target = None;
        magic.cast_targets = 0;
        magic.cast_tier = None;
        magic.cure_via_familiar = None;
        magic.metamagic = None;
        magic.force_mark_slot = false;
        magic.resolving_center = None;
        magic.heightened_target = None;
    }

    pub(super) fn magic_cancel_suggestion(&mut self, i: usize) {
        self.magic_clear_cast_choices(i);
        self.f[i].magic.suggestion = None;
        if self.f[i]
            .magic
            .readied
            .is_some_and(|(spell, _)| spell == Spell::Suggestion)
            && self.f[i].concentration != Some(Concentration::ReadiedSpell)
        {
            self.f[i].magic.readied = None;
        }
    }
}

impl Spell {
    fn needs_point(self) -> bool {
        use Spell::*;
        matches!(
            self,
            PlantGrowth | HungerOfHadar | FaerieFire | Shatter
                | Fireball
                | HypnoticPattern
                | Moonbeam
                | SpiritualWeapon
                | StinkingCloud
                | ZoneOfTruth
                | Entangle
                | MistyStep
                | FogCloud
                | CalmEmotions
                | Sleep
        )
    }

    fn is_group_targeted(self) -> bool {
        matches!(self, Spell::Bane | Spell::Command | Spell::HoldPerson | Spell::HideousLaughter)
    }
}

impl Battle {
    fn group_target_mask(&self, i: usize, action: Act) -> u128 {
        let spell = self.prepared_spell(i, action).expect("group-targeted spell");
        let eligible = self.creature_choices(i, spell) as u8;
        let tier = self.f[i]
            .magic
            .cast_tier
            .or_else(|| self.spell_slot(i, spell, 1))
            .unwrap_or(0) as u32;
        let max_targets = match spell {
            Spell::Bane => tier + 2,
            Spell::HoldPerson => tier.saturating_sub(1),
            _ => tier,
        };
        (1u8..16)
            .filter(|&group| group & !eligible == 0 && group.count_ones() <= max_targets)
            .fold(0, |mask, group| mask | ACTS[32 + group as usize].bit())
    }

    fn group_target_label(&self, i: usize, group: u8) -> String {
        let mut names = Vec::new();
        for id in 0..4 {
            if group & (1 << id) == 0 {
                continue;
            }
            names.push(match id {
                0 | 1 if id == i => "self",
                0 | 1 => "opponent",
                2 | 3 if id == i + 2 => "allied creature",
                _ => "opposing creature",
            });
        }
        format!("Target {}", names.join(", "))
    }

    pub(super) fn magic_bane_penalty(&mut self, target: usize) -> i32 {
        if target < 4
            && self.f.iter().any(|source| {
                source.concentration == Some(Concentration::Bane)
                    && source.magic.bane_recipients & (1 << target) != 0
            })
        {
            self.roll(4)
        } else {
            0
        }
    }
}

impl Battle {
    fn heightened_candidates(&self, i: usize, spell: Spell) -> u8 {
        if spell.is_group_targeted() {
            return self.f[i].magic.cast_targets;
        }
        let direction = self.f[i].magic.direction;
        let area = match spell {
            Spell::BurningHands => Some((
                self.fighter_point(i),
                Area::Cone {
                    length: 3,
                    direction,
                },
            )),
            Spell::Fear => Some((
                self.fighter_point(i),
                Area::Cone {
                    length: 6,
                    direction,
                },
            )),
            Spell::Thunderwave => Some((
                self.fighter_point(i),
                Area::OffsetCube {
                    side: 3,
                    direction,
                    quarters: self.f[i].magic.cube_offset,
                },
            )),
            Spell::Fireball | Spell::Shatter | Spell::StinkingCloud => {
                let radius = if spell == Spell::Shatter { 2 } else { 4 };
                Some((self.chosen_center(), Area::Sphere { radius }))
            }
            _ => None,
        };
        if let Some((origin, area)) = area {
            (0..4)
                .filter(|&id| {
                    (id != i || !matches!(spell, Spell::BurningHands | Spell::Fear | Spell::Thunderwave))
                        && self.footprint(id).is_some_and(|footprint| footprint.squares().any(|target| self.area_hits_at(origin, target, area)))
                })
                .fold(0, |mask, id| mask | (1 << id))
        } else {
            1 << self.f[i].magic.cast_target.unwrap_or(1 - i)
        }
    }

    pub(super) fn magic_heightened_for(&self, i: usize, target: usize) -> bool {
        self.f[i].magic.heightened_target == Some(target)
    }

    pub(super) fn magic_heightened_concentration_for(&self, i: usize, target: usize) -> bool {
        self.f[i].magic.concentration_heightened_target == Some(target)
    }
}

impl Battle {
    pub(super) fn magic_offer_mending_counterspell(&mut self, i: usize) -> bool {
        if self.magic_can_counterspell(1 - i, i) {
            self.magic_request(
                1 - i,
                MagicChoice::Counterspell {
                    source: i,
                    spell: Spell::Mending,
                    tier: 0,
                    heightened: false,
                    heightened_target: None,
                    center: None,
                    target: None,
                    cure_route: None,
                    targets: 0,
                    old_slot_turn: self.f[i].slot_turn,
                },
            );
            true
        } else {
            false
        }
    }
}

const LIGHT_HUES: [&str; 8] = [
    "White", "Red", "Amber", "Yellow", "Green", "Cyan", "Blue", "Violet",
];

impl Battle {
    fn magic_light_targets(&self, i: usize) -> u128 {
        let origin = self.fighter_point(i);
        let mut available = 1; // one carried focus or component pouch
        if self.f[i].gear.shield == super::equipment::Carried::Held {
            available |= 2;
        }
        for id in super::objects::FIRST_OBJECT..super::objects::END_OBJECTS {
            let held = id < 8 && self.gear_wielded_by(i, id % 2, id >= 6);
            let near = self
                .observed_object(i, id)
                .is_some_and(|object| origin.dist(object.point) <= 1);
            if held || near {
                available |= 1 << id;
            }
        }
        available
    }

    fn magic_light_target_label(&self, i: usize, id: usize) -> Option<String> {
        match id {
            0 => Some("Illuminate carried magic focus".into()),
            1 => Some("Illuminate your shield".into()),
            4..=7 if self.gear_wielded_by(i, id % 2, id >= 6) => Some(format!(
                "Illuminate held {}",
                self.gear_weapon_kind_for(id % 2, id >= 6)?.name()
            )),
            _ => Some(format!(
                "Illuminate {}",
                self.observed_object(i, id)?.label()
            )),
        }
    }

    pub(super) fn light_source_point(&self, caster: usize) -> Option<Point> {
        let magic = &self.f[caster].magic;
        if magic.light_until <= self.turns {
            return None;
        }
        match magic.light_target {
            0 => Some(self.fighter_point(caster)),
            1 => match self.f[caster].gear.shield {
                super::equipment::Carried::Ground(cell) => Some(Point::ground(cell)),
                super::equipment::Carried::Held => Some(self.fighter_point(caster)),
                super::equipment::Carried::ForeignHeld => Some(self.fighter_point(1 - caster)),
                _ => None,
            },
            item @ 4..=7 => {
                (0..2)
                    .find(|&holder| self.gear_wielded_by(holder, item % 2, item >= 6))
                    .map(|holder| self.fighter_point(holder))
                    .or_else(|| self.object(item).map(|object| object.point))
            }
            item => self.object(item).map(|object| object.point),
        }
    }

    pub fn magic_visible_light(
        &self,
        viewer: usize,
        caster: usize,
    ) -> Option<(Point, &'static str)> {
        if viewer >= 2 {
            return None;
        }
        let m = &self.f.get(caster)?.magic;
        if m.light_until <= self.turns {
            return None;
        }
        let point = match m.light_target {
            0 => self
                .can_see(viewer, caster)
                .then(|| self.fighter_point(caster)),
            1 if self.f[caster].gear.shield == super::equipment::Carried::Held => self
                .can_see(viewer, caster)
                .then(|| self.fighter_point(caster)),
            1 => {
                if let super::equipment::Carried::Ground(cell) = self.f[caster].gear.shield {
                    self.map
                        .los(self.f[viewer].pos, cell)
                        .then_some(Point::ground(cell))
                } else {
                    None
                }
            }
            id => self
                .observed_object(viewer, id)
                .map(|obj| obj.point)
                .or_else(|| {
                    (id < 8)
                        .then(|| {
                            (0..2).find(|&holder| {
                                self.can_see(viewer, holder)
                                    && self.gear_wielded_by(holder, id % 2, id >= 6)
                            })
                        })
                        .flatten()
                        .map(|holder| self.fighter_point(holder))
                }),
        }?;
        Some((point, LIGHT_HUES[m.light_color]))
    }
}

impl Battle {
    pub(super) fn magic_apply_willing_buff(
        &mut self,
        i: usize,
        spell: Spell,
        tier: usize,
        recipients: u8,
    ) {
        use Spell::*;
        let effect = match spell {
            Heroism => Some((Concentration::Heroism, 10)),
            Fly => Some((Concentration::Fly, 100)),
            Guidance => Some((Concentration::Guidance, 10)),
            Resistance => Some((Concentration::Resistance, 10)),
            ProtectionFromEnergy => Some((Concentration::ProtectionFromEnergy, 600)),
            Invisibility => Some((Concentration::Invisibility, 600)),
            EnhanceAbility => Some((Concentration::EnhanceAbility, 600)),
            Jump => {
                for target in 0..2 {
                    if recipients & (1 << target) != 0 {
                        self.f[target].magic.jump_until = self.turns + 20;
                    }
                }
                None
            }
            _ => unreachable!("not a willing-focus spell"),
        };
        if let Some((effect, rounds)) = effect {
            self.concentrate(i, effect, rounds, None, tier);
            self.f[i].magic.ally_recipients = recipients;
            self.f[i].magic.protected_type = self.f[i].magic.element;
            self.f[i].magic.resistance_tick = -1;
        }
    }

    pub(in crate::arena2) fn magic_nondetection(&self, target: usize) -> bool {
        self.f.iter().any(|f| f.magic.nondetection_until[target] > self.turns)
    }

    pub(in crate::arena2) fn magic_invisible(&self, target: usize) -> bool {
        self.magic_buff_source(target, Concentration::Invisibility).is_some()
            && !self.projectile_glowing(target)
    }

    pub(in crate::arena2) fn magic_end_invisibility_for(&mut self, target: usize) {
        for fighter in &mut self.f {
            if fighter.concentration == Some(Concentration::Invisibility) {
                fighter.magic.ally_recipients &= !(1 << target);
                if fighter.magic.ally_recipients == 0 {
                    fighter.concentration = None;
                }
            }
        }
    }

    pub(in crate::arena2) fn magic_enhances_ability(&self, target: usize, ability: usize) -> bool {
        self.magic_buff_source(target, Concentration::EnhanceAbility)
            .is_some_and(|source| self.f[source].magic.enhanced_ability == ability)
    }

    pub(in crate::arena2) fn magic_pass_without_trace(&self, target: usize) -> bool {
        let source = target % 2;
        self.f[source].concentration == Some(Concentration::PassWithoutTrace)
            && self.creature_point(target)
                .is_some_and(|recipient| self.fighter_point(source).dist(recipient) <= 6)
    }

    pub(in crate::arena2) fn magic_crusaders_mantle(&mut self, hit: Hit) -> i32 {
        let source = hit.attacker % 2;
        if hit.weapon && !hit.unarmed
            && self.f[source].concentration == Some(Concentration::CrusadersMantle)
            && self.creature_point(hit.attacker)
                .is_some_and(|attacker| self.fighter_point(source).dist(attacker) <= 6)
        {
            self.roll_many(1 + i32::from(hit.crit), 4)
        } else {
            0
        }
    }

    fn magic_buff_source(&self, target: usize, effect: Concentration) -> Option<usize> {
        self.f.iter().enumerate().find_map(|(source, f)| {
            (f.concentration == Some(effect)
                && ((f.magic.ally_recipients == 0 && target == source)
                    || f.magic.ally_recipients & (1 << target) != 0))
                .then_some(source)
        })
    }

    fn magic_heroism_source(&self, target: usize) -> Option<usize> {
        self.f
            .iter()
            .enumerate()
            .filter(|&(source, f)| {
                f.concentration == Some(Concentration::Heroism)
                    && f.magic.expires > self.turns
                    && ((f.magic.ally_recipients == 0 && target == source)
                        || f.magic.ally_recipients & (1 << target) != 0)
            })
            .max_by_key(|&(_, f)| (f.build.casting, f.magic.expires))
            .map(|(source, _)| source)
    }

    pub(super) fn magic_heroism_immune(&self, target: usize) -> bool {
        self.magic_heroism_source(target).is_some()
    }
}

impl Battle {
    fn magic_offer_readied_counterspell(&mut self, i: usize, tier: usize, old_slot_turn: i32) {
        if self.magic_can_counterspell(1 - i, i) {
            self.magic_request(
                1 - i,
                MagicChoice::Counterspell {
                    source: i,
                    spell: Spell::HeldSpell,
                    tier,
                    heightened: false,
                    heightened_target: None,
                    center: None,
                    target: None,
                    cure_route: None,
                    targets: 0,
                    old_slot_turn,
                },
            );
        }
    }
}

impl Battle {
    fn finish_counterspell(&mut self, pending: MagicChoice, negate: bool) {
        let MagicChoice::Counterspell {
            source,
            spell,
            tier,
            heightened,
            heightened_target,
            center,
            target,
            cure_route,
            targets,
            old_slot_turn,
        } = pending
        else {
            unreachable!("pending Counterspell payload")
        };
        if spell == Spell::HeldSpell {
            if negate {
                if tier != 0 {
                    self.f[source].slots[tier - 1] += 1;
                }
                self.f[source].slot_turn = old_slot_turn;
                self.f[source].concentration = None;
                self.f[source].magic.readied = None;
            }
            return;
        }
        if spell == Spell::Alarm && negate {
            self.alarm_cancel(source);
        }
        if spell == Spell::Mending {
            if negate && self.f[source].concentration == Some(Concentration::Mending) {
                self.f[source].concentration = None;
            }
            self.origin_finish_mend_if_ready(source);
            return;
        }
        if spell == Spell::EnsnaringStrike {
            if negate {
                self.f[source].slots[tier - 1] += 1;
                self.f[source].slot_turn = old_slot_turn;
                self.f[source].magic.ensnaring_hit = None;
            } else {
                self.resolve_ensnaring_hit(source, tier);
            }
            self.resume_magic_rider();
            return;
        }
        if spell == Spell::Shield {
            if negate {
                self.f[source].slots[tier - 1] += 1;
                self.f[source].slot_turn = old_slot_turn;
            }
            self.finish_magic_shield(source, tier, !negate);
            return;
        }
        if negate {
            self.f[source].magic.ally_selection = 0;
            if spell == Spell::Suggestion {
                self.f[source].magic.suggestion = None;
            }
            if tier > 0 {
                self.f[source].slots[tier - 1] += 1;
                self.f[source].slot_turn = old_slot_turn;
            }
        } else {
            self.f[source].magic.resolving_center = center;
            self.f[source].magic.heightened_target = heightened_target;
            self.f[source].magic.cast_target = target;
            self.f[source].magic.cure_via_familiar = cure_route;
            self.f[source].magic.cast_targets = targets;
            self.resolve_spell(source, spell, if spell == Spell::GuidingBolt { tier.max(1) } else { tier }, heightened);
            self.f[source].magic.resolving_center = None;
            self.f[source].magic.heightened_target = None;
            self.f[source].magic.cast_target = None;
            self.f[source].magic.cure_via_familiar = None;
            self.f[source].magic.cast_targets = 0;
        }
    }
}

#[path = "magic_warlock.rs"]
mod warlock;

impl Spell {
    fn element_choices(self) -> &'static [(DamageType, &'static str)] {
        match self {
            Self::Resistance => &RESISTANCE_TYPES,
            Self::ProtectionFromEnergy => &ENERGY_TYPES,
            Self::DragonBreath => &ELEMENTS[..5],
            _ => &ELEMENTS,
        }
    }
}

impl Battle {
    fn cure_paths(&self, i: usize, recipient: usize) -> (bool, bool) {
        (
            self.magic_reaches_known_creature(i, recipient, 1, false),
            self.entity_can_deliver_touch(i, recipient),
        )
    }

    pub(super) fn magic_reaches_known_creature(
        &self,
        caster: usize,
        recipient: usize,
        range: i32,
        require_sight: bool,
    ) -> bool {
        if !self.rope_same_space(caster, recipient) {
            return false;
        }

        if require_sight && !self.entity_visible(caster, recipient) {
            return false;
        }
        if let Some(origin) = self.divine_cast_origin(caster) {
            return self.observed_footprint(caster, recipient).is_some_and(|targets|
                targets.squares().any(|target| origin.dist(target) <= range && self.map.los_at(origin, target)));
        }
        self.footprint(caster)
            .zip(self.observed_footprint(caster, recipient))
            .is_some_and(|(origins, targets)| origins.squares().any(|origin| {
                targets.squares().any(|target| {
                    origin.dist(target) <= range && self.map.los_at(origin, target)
                })
            }))
    }
}

#[cfg(test)]
#[path = "magic_martial_tests.rs"]
mod martial_spell_tests;
