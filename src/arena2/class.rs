//! Arena classes, level progression, and level-five roster presets.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Barbarian,
    Bard,
    Cleric,
    Druid,
    Fighter,
    Monk,
    Paladin,
    Ranger,
    Rogue,
    Sorcerer,
    Warlock,
    Wizard,
}
pub const CLASSES: [Class; 12] = [
    Class::Barbarian,
    Class::Bard,
    Class::Cleric,
    Class::Druid,
    Class::Fighter,
    Class::Monk,
    Class::Paladin,
    Class::Ranger,
    Class::Rogue,
    Class::Sorcerer,
    Class::Warlock,
    Class::Wizard,
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Build {
    pub class: Class,
    /// The subclass index within the class roster. Zero is the original preset.
    pub variant: u8,
    pub level: i32,
    pub hp: i32,
    pub ac: i32,
    pub speed: i32,
    pub stats: [i32; 6],
    pub pb: i32,
    pub casting: i32,
    pub saves: [bool; 6],
    pub slots: [i32; 3],
    pub melee_die: i32,
    pub melee_mod: i32,
    pub ranged_die: i32,
    pub ranged_mod: i32,
}

pub const NAMES: [&str; 12] = [
    "Barbarian",
    "Bard",
    "Cleric",
    "Druid",
    "Fighter",
    "Monk",
    "Paladin",
    "Ranger",
    "Rogue",
    "Sorcerer",
    "Warlock",
    "Wizard",
];
pub const EQUIPMENT: [&str; 12] = [
    "Greataxe",
    "Dagger / shortbow / leather",
    "Mace / shield / scale mail",
    "Quarterstaff / shield / leather",
    "Longsword / shield / chain mail",
    "Unarmed strikes",
    "Longsword / shield / chain mail",
    "Shortsword / longbow / studded leather",
    "Rapier / shortbow / studded leather",
    "Dagger",
    "Dagger / leather",
    "Dagger",
];

pub const DUEL_LEVEL: i32 = 5;
pub const VARIANTS_PER_CLASS: usize = 3;
pub const ROSTER_SIZE: usize = CLASSES.len() * VARIANTS_PER_CLASS;

#[derive(Clone, Copy, Debug)]
pub struct RosterPreset {
    pub name: &'static str,
    pub description: &'static str,
}

const fn preset(name: &'static str, description: &'static str) -> RosterPreset {
    RosterPreset { name, description }
}

pub const ROSTER: [RosterPreset; ROSTER_SIZE] = [
    preset("Berserker", "Frenzy drives extra damage on reckless strikes."),
    preset("Zealot", "Divine Fury adds damage to the first strike that lands while raging."),
    preset("World Tree", "Raging grants additional temporary hit points through Vitality of the Tree."),
    preset("Lore", "Cutting Words disrupts opposing rolls."),
    preset("Valor", "Martial weapons, medium armor and shields support a weapon-focused bard."),
    preset("Glamour", "Fey magic charms enemies and aids allies."),
    preset("Life", "Channel Divinity and restorative spells sustain the Life cleric."),
    preset("War", "War Priest supports additional weapon attacks and Guided Strike hones accuracy."),
    preset("Trickery", "Invoke Duplicity creates an illusion to support misdirection in battle."),
    preset("Land", "Arid circle magic adds Fireball alongside Wild Shape."),
    preset("Stars", "Starry Form selects a constellation with a distinct combat benefit."),
    preset("Sea", "Wrath of the Sea harms and pushes a creature near the druid."),
    preset("Champion", "Improved Critical and initiative advantage favor direct combat."),
    preset("Battle Master", "Combat Superiority dice fuel weapon maneuvers."),
    preset("Psi Warrior", "Psionic Energy dice power protection, force damage and telekinetic movement."),
    preset("Open Hand", "Flurry of Blows can push, topple, or prevent opportunity attacks."),
    preset("Mercy", "Hand of Healing restores hit points and Hand of Harm adds necrotic damage."),
    preset("Elements", "Elemental Attunement extends unarmed reach and changes damage types."),
    preset("Devotion", "Sacred Weapon empowers attacks and can deal radiant damage."),
    preset("Ancients", "Nature magic restrains creatures and the oath grants protective spells."),
    preset("Glory", "Channel Divinity supports physical feats and turns a smite into protection."),
    preset("Hunter", "Colossus Slayer adds weapon damage against injured targets."),
    preset("Fey Wanderer", "Dreadful Strikes add psychic damage and fey magic broadens spell choices."),
    preset("Gloom Stalker", "Dread Ambusher improves opening initiative and fuels psychic strikes."),
    preset("Thief", "Fast Hands opens bonus-action item and object interactions."),
    preset("Assassin", "Assassinate rewards acting before the enemy and striking on the first turn."),
    preset("Soulknife", "Psychic Blades and psionic dice give the rogue weapon and skill options."),
    preset("Draconic", "Draconic Resilience adds hit points and unarmored AC."),
    preset("Aberrant", "Aberrant spells and telepathy support psychic tactics."),
    preset("Clockwork", "Restore Balance cancels advantage or disadvantage on a roll."),
    preset("Fiend", "Dark One's Blessing grants temporary hit points on nearby defeats."),
    preset("Archfey", "Steps of the Fey supplies bonus Misty Step uses with an additional effect."),
    preset("Great Old One", "Psychic Spells and telepathic features favor mental magic."),
    preset("Evoker", "Potent Cantrip deals half damage when a creature avoids a damaging cantrip."),
    preset("Abjurer", "Arcane Ward absorbs damage and recharges when casting abjurations."),
    preset("Diviner", "Portent lets the wizard substitute pre-rolled d20 results."),
];

/// A fixed spellbook entry learned from a Wizard's Savant subclass feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavantSpell {
    pub learned_at: i32,
    pub rank: i32,
    pub school: &'static str,
    pub name: &'static str,
}

const ABJURATION_SAVANT: [SavantSpell; 3] = [
    SavantSpell { learned_at: 3, rank: 1, school: "Abjuration", name: "Alarm" },
    SavantSpell { learned_at: 3, rank: 1, school: "Abjuration", name: "Protection from Evil and Good" },
    SavantSpell { learned_at: 5, rank: 3, school: "Abjuration", name: "Counterspell" },
];

const DIVINATION_SAVANT: [SavantSpell; 3] = [
    SavantSpell { learned_at: 3, rank: 2, school: "Divination", name: "Detect Thoughts" },
    SavantSpell { learned_at: 3, rank: 2, school: "Divination", name: "See Invisibility" },
    SavantSpell { learned_at: 5, rank: 3, school: "Divination", name: "Clairvoyance" },
];

impl Build {
    pub fn savant_spellbook(self) -> &'static [SavantSpell] {
        let count = match self.level {
            3 | 4 => 2,
            5.. => 3,
            _ => 0,
        };
        match (self.class, self.variant) {
            (Class::Wizard, 1) => &ABJURATION_SAVANT[..count],
            (Class::Wizard, 2) => &DIVINATION_SAVANT[..count],
            _ => &[],
        }
    }

    pub fn is_subclass(self, class: Class, variant: u8) -> bool {
        self.class == class && self.variant == variant
    }

    pub fn roster_id(self) -> usize {
        self.class as usize * VARIANTS_PER_CLASS + usize::from(self.variant)
    }

    pub fn subclass(self) -> &'static str {
        ROSTER[self.roster_id()].name
    }
}

pub fn roster_build(id: u8) -> Option<Build> {
    let (class, variant) = (
        usize::from(id) / VARIANTS_PER_CLASS,
        usize::from(id) % VARIANTS_PER_CLASS,
    );
    Some(build_with_variant(*CLASSES.get(class)?, DUEL_LEVEL, variant as u8))
}

pub fn build(class: Class, level: i32) -> Build {
    build_with_variant(class, level, 0)
}

fn build_with_variant(class: Class, level: i32, variant: u8) -> Build {
    use Class::*;
    assert!((1..=5).contains(&level));
    let stats = super::origins::raw_scores(class, level).map(|s| (s - 10).div_euclid(2));
    let [str_mod, dex, con, _, wis, cha] = stats;
    let pb = if level == 5 { 3 } else { 2 };
    let hit_die = match class {
        Barbarian => 12,
        Fighter | Paladin | Ranger => 10,
        Wizard | Sorcerer => 6,
        _ => 8,
    };
    // Dwarven Toughness adds a hit point at every level.
    let hp = hit_die
        + con
        + (level - 1) * (hit_die / 2 + 1 + con)
        + level
        + if class == Sorcerer && variant == 0 && level >= 3 {
            level
        } else {
            0
        };
    let ac = match class {
        Barbarian => 10 + dex + con,
        Bard | Warlock => 11 + dex,
        Cleric => 14 + dex.min(2) + 2,
        Druid => 11 + dex + 2,
        Fighter => 19,
        Paladin => 18 + (level >= 2) as i32,
        Ranger | Rogue => 12 + dex,
        Monk => 10 + dex + wis,
        Sorcerer if variant == 0 && level >= 3 => 10 + dex + cha,
        _ => 10 + dex,
    };
    let speed = 6 + if (class == Monk && level >= 2) || (class == Barbarian && level >= 5) {
        2
    } else {
        0
    };
    let (s1, s2) = match class {
        Barbarian | Fighter => (0, 2),
        Bard => (1, 5),
        Cleric | Paladin | Warlock => (4, 5),
        Druid | Wizard => (3, 4),
        Monk | Ranger => (0, 1),
        Rogue => (1, 3),
        Sorcerer => (2, 5),
    };
    let mut saves = [false; 6];
    saves[s1] = true;
    saves[s2] = true;
    let slots = match class {
        Paladin | Ranger => {
            [[2, 0, 0], [2, 0, 0], [3, 0, 0], [3, 0, 0], [4, 2, 0]][level as usize - 1]
        }
        Warlock => [[1, 0, 0], [2, 0, 0], [0, 2, 0], [0, 2, 0], [0, 0, 2]][level as usize - 1],
        Bard | Cleric | Druid | Sorcerer | Wizard => {
            [[2, 0, 0], [3, 0, 0], [4, 2, 0], [4, 3, 0], [4, 3, 2]][level as usize - 1]
        }
        _ => [0; 3],
    };
    let (melee_die, melee_mod) = match class {
        Barbarian => (12, str_mod),
        Fighter | Paladin => (8, str_mod),
        Rogue => (8, dex),
        Ranger => (6, dex),
        Monk => (if level >= 5 { 8 } else { 6 }, dex),
        Cleric | Druid => (6, str_mod),
        _ => (4, str_mod.max(dex)),
    };
    let ranged_die = match class {
        Ranger => 8,
        Rogue | Bard => 6,
        _ => 0,
    };
    let casting = match class {
        Bard | Sorcerer | Paladin | Warlock => cha,
        Cleric | Druid | Monk | Ranger => wis,
        Wizard => stats[3],
        _ => 0,
    };
    Build {
        class,
        variant,
        level,
        hp,
        ac,
        speed,
        stats,
        pb,
        casting,
        saves,
        slots,
        melee_die,
        melee_mod,
        ranged_die,
        ranged_mod: dex,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roster_preserves_original_level_five_builds_and_has_three_subclasses_per_class() {
        for class in CLASSES {
            let first = (class as u8) * VARIANTS_PER_CLASS as u8;
            assert_eq!(roster_build(first), Some(build(class, DUEL_LEVEL)));
            let subclasses = (first..first + VARIANTS_PER_CLASS as u8)
                .map(|id| {
                    let b = roster_build(id).unwrap();
                    assert_eq!(b.roster_id(), usize::from(id));
                    assert_eq!(b.level, DUEL_LEVEL);
                    assert_eq!(b.pb, 3);
                    b.subclass()
                })
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(subclasses.len(), VARIANTS_PER_CLASS);
        }
        assert!(roster_build(ROSTER_SIZE as u8).is_none());
    }

    #[test]
    fn all_sixty_progress_from_valid_standard_arrays() {
        for class in CLASSES {
            let scores = super::super::origins::raw_scores(class, 1);
            assert_eq!(scores.iter().sum::<i32>(), 75);
            for l in 1..=5 {
                let b = build(class, l);
                assert!(b.ac >= 12 && b.hp > 0);
                assert_eq!(b.saves.iter().filter(|s| **s).count(), 2);
                if l > 1 {
                    assert!(b.hp > build(class, l - 1).hp);
                }
            }
        }
        assert_eq!(build(Class::Warlock, 5).slots, [0, 0, 2]);
        assert_eq!(build(Class::Druid, 5).slots, [4, 3, 2]);
        assert_eq!(build(Class::Paladin, 5).slots, [4, 2, 0]);
    }
}
