//! Weapon table entries used by the fixed loadouts.
use super::{
    class::{Build, Class},
    equipment::DamageType,
    origins::MasteryWeapon,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponKind {
    Greataxe,
    Longsword,
    Rapier,
    Shortsword,
    Longbow,
    Shortbow,
    Dagger,
    Mace,
    Quarterstaff,
    PsychicBlade,
}

impl WeaponKind {
    pub fn issued(class: Class, bow: bool) -> Option<Self> {
        use WeaponKind::*;
        if bow {
            return match class {
                Class::Ranger => Some(Longbow),
                Class::Bard | Class::Rogue => Some(Shortbow),
                _ => None,
            };
        }
        Some(match class {
            Class::Barbarian => Greataxe,
            Class::Fighter | Class::Paladin => Longsword,
            Class::Ranger => Shortsword,
            Class::Rogue => Rapier,
            Class::Druid => Quarterstaff,
            Class::Cleric => Mace,
            Class::Monk => return None,
            _ => Dagger,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Greataxe => "Greataxe",
            Self::Longsword => "Longsword",
            Self::Rapier => "Rapier",
            Self::Shortsword => "Shortsword",
            Self::Longbow => "Longbow",
            Self::Shortbow => "Shortbow",
            Self::Dagger => "Dagger",
            Self::Mace => "Mace",
            Self::Quarterstaff => "Quarterstaff",
            Self::PsychicBlade => "Psychic Blade",
        }
    }
    pub fn damage_type(self) -> DamageType {
        match self {
            Self::Mace | Self::Quarterstaff => DamageType::Bludgeoning,
            Self::PsychicBlade => DamageType::Psychic,
            Self::Greataxe | Self::Longsword => DamageType::Slashing,
            _ => DamageType::Piercing,
        }
    }
    pub fn finesse(self) -> bool {
        matches!(self, Self::Dagger | Self::Shortsword | Self::Rapier | Self::PsychicBlade)
    }
    pub fn is_bow(self) -> bool {
        matches!(self, Self::Longbow | Self::Shortbow)
    }
    pub fn versatile(self) -> bool {
        matches!(self, Self::Quarterstaff | Self::Longsword)
    }
    pub fn needs_two(self) -> bool {
        self.is_bow() || self == Self::Greataxe
    }
    pub fn proficient(self, build: Build) -> bool {
        use Class::*;
        let simple = matches!(
            self,
            Self::Dagger | Self::Shortbow | Self::Mace | Self::Quarterstaff | Self::PsychicBlade
        );
        simple
            || match build.class {
                Fighter | Barbarian | Paladin | Ranger | Cleric | Druid => true, // Protector / Warden
                Bard if build.variant == 1 => true, // Valor Combat Training
                Rogue => self.finesse(), // plus martial Light weapons, represented here by Shortsword
                Monk => self == Self::Shortsword,
                _ => false,
            }
    }
    pub fn die(self, two_hands: bool) -> i32 {
        match self {
            Self::Greataxe => 12,
            Self::Longsword if two_hands => 10,
            Self::Quarterstaff if two_hands => 8,
            Self::Longbow | Self::Rapier | Self::Longsword => 8,
            Self::Dagger => 4,
            _ => 6,
        }
    }
    pub fn mastery(self) -> Option<MasteryWeapon> {
        Some(match self {
            Self::Greataxe => MasteryWeapon::Greataxe,
            Self::Longsword => MasteryWeapon::Longsword,
            Self::Rapier => MasteryWeapon::Rapier,
            Self::Shortsword => MasteryWeapon::Shortsword,
            Self::Shortbow => MasteryWeapon::Shortbow,
            Self::Longbow => MasteryWeapon::Longbow,
            Self::PsychicBlade => MasteryWeapon::PsychicBlade,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::class::{build,CLASSES};

    #[test]
    fn proficiencies_follow_the_current_wielder_for_every_issued_weapon_and_level() {
        use Class::*;
        use WeaponKind::*;
        for class in CLASSES {
            for level in 1..=5 {
                let wield=build(class,level);
                for simple in [Dagger,Mace,Quarterstaff,Shortbow] { assert!(simple.proficient(wield)); }
                for martial in [Greataxe,Longsword,Longbow,Rapier,Shortsword] {
                    let expected=matches!(class,Barbarian|Fighter|Paladin|Ranger|Cleric|Druid)
                        || class==Rogue && matches!(martial,Rapier|Shortsword)
                        || class==Monk && martial==Shortsword;
                    assert_eq!(martial.proficient(wield),expected,"{class:?} {level} {martial:?}");
                }
                for ranged in [false,true] {
                    if let Some(issued)=WeaponKind::issued(class,ranged) { assert!(issued.proficient(wield)); }
                }
            }
        }
    }
}
