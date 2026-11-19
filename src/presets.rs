//! Hand-authored combat loadouts. IDs belong to this project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mastery {
    Sap,
    Slow,
}

#[derive(Clone, Copy, Debug)]
pub struct Weapon {
    pub id: &'static str,
    pub die: i32,
    pub mastery: Mastery,
    pub ranged: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Build {
    pub id: &'static str,
    pub book: &'static str,
    pub features: &'static [&'static str],
    pub level: i32,
    pub hp: i32,
    pub ac: i32,
    pub dex: i32,
    pub strength: i32,
    pub speed: i32,
    pub proficiency: i32,
    pub style_attack_bonus: i32,
    pub arrows: i32,
    pub winds: i32,
    pub weapon: Weapon,
}

const FIGHTER: Build = Build {
    id: "shield",
    book: "phb-2024",
    features: &[
        "fighter.second-wind",
        "fighter.weapon-mastery",
        "style.defense",
    ],
    level: 1,
    hp: 12,
    ac: 19,
    dex: 1,
    strength: 3,
    speed: 30,
    proficiency: 2,
    style_attack_bonus: 0,
    arrows: 0,
    winds: 2,
    weapon: Weapon {
        id: "longsword",
        die: 8,
        mastery: Mastery::Sap,
        ranged: false,
    },
};

pub const BUILDS: [Build; 2] = [
    FIGHTER,
    Build {
        id: "archer",
        ac: 15,
        dex: 3,
        strength: 0,
        features: &[
            "fighter.second-wind",
            "fighter.weapon-mastery",
            "style.archery",
        ],
        style_attack_bonus: 2,
        arrows: 20,
        weapon: Weapon {
            id: "longbow",
            die: 8,
            mastery: Mastery::Slow,
            ranged: true,
        },
        ..FIGHTER
    },
];
