//! Combat resources and attack features for the six martial presets.
use super::{
    battle::{Battle, Hit},
    class::{Build, Class},
    common::{Check, Skill},
    equipment::WeaponKind,
    geometry::{Pos, HEIGHT, WIDTH},
    origins::MasteryWeapon,
    Act,
};

#[path = "martial_choices.rs"]
mod choices;
#[path = "martial_entities.rs"]
mod entity_conditions;
#[path = "martial_hits.rs"]
mod hits;
pub use entity_conditions::EntityMartial;
#[path = "martial_unarmed.rs"]
mod unarmed;
#[path = "fighter_rogue.rs"]
pub(super) mod fighter_rogue;
#[path = "martial_subclasses.rs"]
pub(super) mod subclasses;

pub(super) fn paired_martial_choices(phase: i32) -> std::ops::Range<usize> {
    match phase {
        choices::LAY_TARGET | hits::REDIRECT | hits::CLEAVE | unarmed::UNARMED_TARGET
        | subclasses::LIFE_GIVING_FORCE | subclasses::HAND_OF_HEALING => 0..4,
        choices::DISARM_TRAP => 8..10,
        _ => 0..0,
    }
}

#[derive(Clone, Debug, Default)]
pub struct MartialState {
    pub(super) fighter_rogue: fighter_rogue::SubclassState,
    attack_rolled: bool,
    rage_expires: i32,
    rage_started: i32,
    frenzy_turn: i32,
    frenzy_enabled: bool,
    sneak_turn: i32,
    colossus_turn: i32,
    stunning_turn: i32,
    sacred_expires: i32,
    sacred_issuer: Option<usize>,
    free_smite: bool,
    vex_expires: [i32; 4],
    caltrops: Option<Pos>,
    caltrops_triggered: [i32; 4],
    caltrops_inside: [bool; 4],
    lay_target: Option<usize>,
    sense_until: i32,
    poison_until: [i32; 4],
    hit: Option<hits::MartialHit>,
    cleave_turn: i32,
    shifted: bool,
    longer_jump: bool,
    primal_check: Option<Check>,
    bonus_unarmed: Option<unarmed::BonusUnarmed>,
    disarm_bonus: bool,
    subclass_dice: u8,
    divine_fury: (i32, Option<super::equipment::DamageType>),
    harm_turn: i32,
    attunement_until: i32,
    elemental_hit: Option<(Hit, Option<Act>)>,
    mercy_heal_flurry: bool,
    elementalism_target: Option<(super::geometry::Point, Act)>,
    elementalism_readied: bool,
    elementalism_effects: Vec<subclasses::ElementalismEffect>,
}

pub fn label(class: Class, action: Act) -> Option<&'static str> {
    use Act::*;
    use Class::*;
    Some(match (class, action) {
        (Barbarian, Special1) => "Rage",
        (Barbarian, Special2) => "Reckless Attack",
        (Barbarian, Special3) => "Extend Rage",
        (Fighter, Special1) => "Second Wind",
        (Fighter, Special2) => "Action Surge",
        (Monk, Special1) => "Bonus Unarmed Strike",
        (Monk, Special2) => "Flurry of Blows",
        (Monk, Special3) => "Patient Defense",
        (Monk, Special4) => "Step of the Wind",
        (Monk, Special5) => "Patient Defense: Disengage",
        (Monk, Special6) => "Step of the Wind: Disengage",
        (Monk, Martial3) => "Flurry of Blows: second strike",
        (Monk, Martial4) => "Bonus Unarmed Strike: choose mode",
        (Monk, Martial5) => "Flurry of Blows: choose modes",
        (Monk, Defend) => "Deflect Attacks",
        (Monk, Rider) => "Stunning Strike",
        (Paladin, Special1) => "Lay On Hands",
        (Paladin, Special2) => "Sacred Weapon",
        (Paladin, Martial1) => "Divine Sense",
        (Paladin, Martial2) => "End Sacred Weapon",
        (Paladin, Rider) => "Divine Smite",
        (Rogue, Special1) => "Steady Aim",
        (Rogue, Special2) => "Cunning Dash",
        (Rogue, Special3) => "Cunning Disengage",
        (Rogue, Special4) => "Fast Hands: Caltrops",
        (Rogue, Utilize) => "Utilize Caltrops",
        (Rogue, Martial4) => "Fast Hands: Doff shield",
        (Rogue, Martial5) => "Fast Hands: Don shield",
        (Rogue, Martial6) => "Fast Hands: Disarm caltrops",
        (Rogue, Martial7) => "Thieves’ Tools: Disarm caltrops",
        (Rogue, Martial1) => "Cunning Action: Hide",
        (Rogue, Defend) => "Uncanny Dodge",
        _ => return None,
    })
}

pub(super) fn attacks_per_action(build: Build) -> i32 {
    use Class::*;
    let extra_attack = matches!(build.class, Barbarian | Fighter | Monk | Paladin | Ranger);
    1 + i32::from(extra_attack && build.level >= 5)
}

pub(super) fn critical_threshold(build: Build) -> i32 {
    if build.class == Class::Fighter && build.variant == 0 && build.level >= 3 {
        19
    } else {
        20
    }
}

fn sneak_attack_dice(level: i32) -> i32 {
    (level + 1) / 2
}

impl Battle {
    /// Additional observations in the order Sacred Weapon, Vex, Frenzy,
    /// Sneak Attack, Stunning Strike, free Smite, Rage extension, first roll.
    /// For Rogue, entries 0 and 2 encode Caltrops as (coordinate + 1) / board size.
    /// A zero coordinate feature indicates no deployed bag.
    /// For Fighter, entry 4 encodes remaining Tactical Shift as a share of base Speed.
    pub fn martial_features(&self, i: usize) -> [f32; 8] {
        let fighter = &self.f[i];
        let state = &fighter.martial;
        let class = fighter.build.class;
        let level = fighter.build.level;
        let flag = |set| f32::from(u8::from(set));
        let mut features = [
            flag(self.martial_sacred_active(i)),
            flag(self.martial_has_vex(i)),
            flag(
                class == Class::Barbarian
                    && level >= 3
                    && fighter.raging
                    && fighter.build.variant == 0
                    && state.frenzy_turn != self.turns,
            ),
            flag(class == Class::Rogue && state.sneak_turn != self.turns),
            flag(
                class == Class::Monk
                    && level >= 5
                    && fighter.resource > 0
                    && state.stunning_turn != self.turns,
            ),
            flag(state.free_smite),
            flag(fighter.raging && state.rage_expires == self.turns),
            flag(class == Class::Barbarian && level >= 2 && !state.attack_rolled),
        ];
        if let Some(pos) = self
            .observed_object(i, 8 + i)
            .map(|object| object.point.cell)
        {
            features[0] = f32::from(pos.x + 1) / f32::from(WIDTH);
            features[2] = f32::from(pos.y + 1) / f32::from(HEIGHT);
        }
        if class == Class::Fighter && self.owner == i {
            features[4] = self.tactical_steps as f32 / fighter.build.speed as f32;
        }
        features
    }

    pub fn caltrops(&self, i: usize) -> Option<Pos> {
        self.f[i].martial.caltrops
    }

    pub(super) fn move_caltrops(&mut self, i: usize, cell: Pos) {
        if self.f[i].martial.caltrops.is_some() {
            self.place_caltrops(i, cell);
        }
    }

    pub(super) fn disable_caltrops(&mut self, i: usize) {
        self.f[i].martial.caltrops = None;
    }

    pub fn martial_summary(&self, i: usize) -> String {
        let fighter = &self.f[i];
        let [sacred, vex, _, _, _, free_smite, _, _] = self.martial_features(i);
        let summary = match fighter.build.class {
            Class::Barbarian => {
                let active = if fighter.raging { " (active)" } else { "" };
                let uses = format!("Rage uses {}{active}", fighter.resource);
                if self.has_martial_subclass(i, Class::Barbarian, 1) {
                    format!("{uses} | Warrior of the Gods dice {}", fighter.martial.subclass_dice)
                } else {
                    uses
                }
            }
            Class::Fighter => {
                let uses = format!(
                    "Second Wind {} | Action Surge {}",
                    fighter.resource, fighter.secondary
                );
                if self.owner == i && self.tactical_steps > 0 {
                    format!("{uses} | Tactical Shift {} ft", 5 * self.tactical_steps)
                } else {
                    uses
                }
            }
            Class::Monk => {
                let focus = format!("Focus {}", fighter.resource);
                if self.unarmed_reach(i) > 1 {
                    format!("{focus} | Elemental Attunement active")
                } else {
                    focus
                }
            }
            Class::Paladin => format!(
                "Lay On Hands {} HP | Channel Divinity {} | Free Smite {}{}",
                fighter.resource,
                fighter.secondary,
                free_smite as u8,
                if sacred > 0.0 {
                    " | Sacred Weapon active"
                } else {
                    ""
                }
            ),
            Class::Rogue => {
                let state = match self.caltrops(i) {
                    Some(pos) => format!("Caltrops at ({}, {})", pos.x, pos.y),
                    None if fighter.secondary > 0 => format!("Caltrops bags {}", fighter.secondary),
                    None => String::new(),
                };
                match (state.is_empty(), vex > 0.0) {
                    (true, true) => "Vex ready".into(),
                    (false, true) => format!("{state} | Vex ready"),
                    (_, false) => state,
                }
            }
            Class::Ranger => {
                let lore = self.hunters_lore(i).map(|traits| {
                    if traits.is_empty() {
                        "Hunter's Lore: no damage traits".into()
                    } else {
                        format!("Hunter's Lore: resists {traits:?}")
                    }
                });
                match (vex > 0.0, lore) {
                    (true, Some(lore)) => format!("Vex ready | {lore}"),
                    (true, None) => "Vex ready".into(),
                    (false, Some(lore)) => lore,
                    (false, None) => String::new(),
                }
            }
            _ => String::new(),
        };
        match self.fighter_rogue_resource_summary(i) {
            Some(extra) if summary.is_empty() => extra,
            Some(extra) => format!("{summary} | {extra}"),
            None => summary,
        }
    }

    pub(super) fn init_martial(&mut self, i: usize) {
        let fighter = &mut self.f[i];
        fighter.martial = MartialState {
            rage_expires: -1,
            frenzy_turn: -1,
            sneak_turn: -1,
            colossus_turn: -1,
            cleave_turn: -1,
            stunning_turn: -1,
            harm_turn: -1,
            attunement_until: -1,
            sacred_expires: -1,
            sense_until: -1,
            vex_expires: [-1; 4],
            caltrops_triggered: [-1; 4],
            fighter_rogue: fighter_rogue::SubclassState::new(fighter.build),
            ..MartialState::default()
        };
        let level = fighter.build.level;
        if fighter.build.class == Class::Barbarian && fighter.build.variant == 1 && level >= 3 {
            fighter.martial.subclass_dice = 4;
        }
        match fighter.build.class {
            Class::Barbarian => fighter.resource = if level >= 3 { 3 } else { 2 },
            Class::Fighter => {
                fighter.resource = if level >= 4 { 3 } else { 2 };
                fighter.secondary = i32::from(level >= 2);
            }
            Class::Monk if level >= 2 => fighter.resource = level,
            Class::Paladin => {
                fighter.resource = 5 * level;
                fighter.secondary = if level >= 3 { 2 } else { 0 };
                fighter.martial.free_smite = level >= 2;
            }
            Class::Rogue if level >= 3 => fighter.secondary = 1,
            _ => {}
        }
    }

    pub(super) fn martial_mask(&self, i: usize) -> u128 {
        use Act::*;
        use Class::*;
        let fighter = &self.f[i];
        let level = fighter.build.level;
        let melee =
            self.observed_pos(i, 1 - i).is_none() || self.creature_targets(Act::Special2) != 0;
        let can_attack = melee && (self.action || self.attacks > 0);
        let option = |action: Act, legal: bool| if legal { action.bit() } else { 0 };
        match fighter.build.class {
            Barbarian => {
                option(
                    Special1,
                    self.bonus && fighter.resource > 0 && !fighter.raging && !self.armor_heavy(i),
                ) | option(
                    Special2,
                    can_attack
                        && level >= 2
                        && !fighter.martial.attack_rolled
                        && self.gear_allows_weapon(i, false),
                ) | option(
                    Special3,
                    self.bonus && fighter.raging && fighter.martial.rage_expires == self.turns,
                ) | self.subclass_bonus_mask(i)
            }
            Fighter => {
                option(Special1, self.bonus && fighter.resource > 0)
                    | option(
                        Special2,
                        !self.action && self.attacks == 0 && fighter.secondary > 0,
                    )
            }
            Monk => {
                let subclass = self.subclass_monk_mask(i);
                if !self.bonus {
                    return subclass;
                }
                let has_focus = fighter.resource > 0;
                let can_dash = self.speed(i) > 0;
                option(Special1, melee && self.martial_arts_active(i))
                    | option(Special2, melee && has_focus)
                    | option(Special3, has_focus && !fighter.dodge)
                    | option(Special4, level >= 2 && can_dash)
                    | option(Special5, level >= 2 && !self.disengaged)
                    | option(Special6, has_focus && can_dash && !self.disengaged)
                    | option(Martial4, self.martial_unarmed_can_start(i, false))
                    | option(Martial5, self.martial_unarmed_can_start(i, true))
                    | subclass
            }
            Paladin => {
                option(
                    Special1,
                    self.bonus && fighter.resource > 0 && self.lay_targets(i) != 0,
                ) | option(
                    Special2,
                    fighter.build.variant == 0 && self.action && fighter.secondary > 0 && self.gear_wielded(i, false),
                ) | option(Martial1, self.bonus && fighter.secondary > 0)
                    | option(Martial2, fighter.martial.sacred_expires >= self.turns)
            }
            Rogue => {
                let can_disarm = self.martial_disarm_targets(i) != 0;
                let objects = option(Utilize, self.action && fighter.secondary > 0)
                    | option(Martial7, self.action && can_disarm);
                if !self.bonus {
                    return objects;
                }
                let shield = self.gear_ready_mask(i);
                let thief = level >= 3 && fighter.build.variant == 0;
                objects
                    | option(Martial4, thief && shield & Gear7.bit() != 0)
                    | option(Martial5, thief && shield & Gear8.bit() != 0)
                    | option(
                        Special1,
                        level >= 3
                            && self.moved == 0
                            && !fighter.martial.shifted
                            && (self.action || self.attacks > 0),
                    )
                    | option(Special2, level >= 2 && self.speed(i) > 0)
                    | option(Special3, level >= 2 && !self.disengaged)
                    | option(Special4, thief && fighter.secondary > 0)
                    | option(Martial1, level >= 2 && self.can_hide(i))
                    | option(Martial6, thief && can_disarm)
            }
            _ => 0,
        }
    }

    pub(super) fn martial_step(&mut self, i: usize, action: Act) {
        use Act::*;
        use Class::*;
        let class = self.f[i].build.class;
        match (class, action) {
            (Barbarian, Special1) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].raging = true;
                self.f[i].concentration = None;
                self.f[i].martial.rage_started = self.turns;
                self.f[i].martial.rage_expires = self.turns + 2;
                self.subclass_rage_start(i);
            }
            (Barbarian, Martial1) => self.request(i, subclasses::WARRIOR_OF_GODS),
            (Barbarian, Special2) => {
                self.martial_enable_reckless(i);
                self.weapon_attack(i, false);
            }
            (Barbarian, Special3) => {
                self.bonus = false;
                self.f[i].martial.rage_expires = self.turns + 2;
            }
            (Fighter, Special1) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                let healing = self.roll(10) + self.f[i].build.level;
                self.heal(i, healing);
                if self.f[i].build.level >= 5 {
                    self.tactical_steps = self.speed(i) / 2;
                }
            }
            (Fighter, Special2) => {
                self.f[i].secondary -= 1;
                self.fighter_surge_used(i);
                self.action = true;
            }
            (Monk, Martial1) => self.start_mercy_heal(i, false),
            (Monk, Special1 | Special2) => {
                if self.matches_target_guess(i, false) {
                    self.martial_targeted_step(i, action, 1 - i);
                } else {
                    self.bonus = false;
                    self.f[i].resource -= i32::from(action == Special2);
                }
            }
            (Monk, Martial4 | Martial5) => self.martial_unarmed_begin(i, action == Martial5),
            (Monk, Special3) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].dodge = true;
                self.disengaged = true;
            }
            (Monk, Special4) | (Rogue, Special2) => {
                self.bonus = false;
                self.dash += self.speed(i);
            }
            (Monk, Special5) | (Rogue, Special3) => {
                self.bonus = false;
                self.disengaged = true;
            }
            (Monk, Special6) => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].martial.longer_jump = true;
                self.dash += self.speed(i);
                self.disengaged = true;
            }
            (Paladin, Special1) => self.request(i, choices::LAY_TARGET),
            (Paladin, Special2) => {
                self.f[i].secondary -= 1;
                self.f[i].martial.sacred_expires = self.turns + 199;
                self.f[i].martial.sacred_issuer = Some(self.gear_weapon_owner(i, false));
                self.action = false;
                self.attacks = attacks_per_action(self.f[i].build);
            }
            (Paladin, Martial1) => {
                self.bonus = false;
                self.f[i].secondary -= 1;
                self.f[i].martial.sense_until = self.turns + 199;
            }
            (Paladin, Martial2) => {
                self.f[i].martial.sacred_expires = -1;
                self.f[i].martial.sacred_issuer = None;
            }
            (Rogue, Special1) => {
                self.bonus = false;
                self.f[i].attack_advantage = true;
                self.f[i].speed_zero = true;
            }
            (Rogue, Martial1) => {
                self.bonus = false;
                self.try_check(i, Check::Hide);
            }
            (Rogue, Martial4 | Martial5) => {
                self.bonus = false;
                self.gear_ready_release(i, if action == Martial4 { Gear7 } else { Gear8 });
            }
            (Rogue, Martial6 | Martial7) => {
                self.f[i].martial.disarm_bonus = action == Martial6;
                self.request(i, choices::DISARM_TRAP);
            }
            (Rogue, Special4 | Utilize) => {
                let target = self
                    .selected_target
                    .expect("Caltrops require a target square");
                if action == Utilize {
                    self.action = false;
                } else {
                    self.bonus = false;
                }
                self.f[i].secondary -= 1;
                self.place_caltrops(i, target);
            }
            _ => unreachable!("illegal martial action"),
        }
    }

    pub(in crate::arena2) fn martial_targeted_step(
        &mut self,
        i: usize,
        action: Act,
        target: usize,
    ) {
        match self.f[i].build.class {
            Class::Barbarian => {
                self.martial_enable_reckless(i);
                self.weapon_attack_against(i, target, false);
            }
            Class::Monk => {
                self.bonus = false;
                let flurry = action == Act::Special2;
                self.f[i].resource -= i32::from(flurry);
                self.bonus_unarmed_attack_against(i, target, 1 + i32::from(flurry));
            }
            _ => unreachable!("martial creature target requires an attack"),
        }
    }

    pub(super) fn martial_turn(&mut self, i: usize) {
        let fighter = &mut self.f[i];
        fighter.martial.attack_rolled = false;
        fighter.martial.frenzy_enabled = false;
        fighter.martial.shifted = false;
        fighter.martial.longer_jump = false;
        if fighter.raging && (self.turns >= fighter.martial.rage_started + 200 || self.incap(i)) {
            self.f[i].raging = false;
            self.subclass_rage_end(i);
        }
    }

    pub(super) fn martial_jump_multiplier(&self, i: usize) -> i32 {
        1 + i32::from(self.f[i].martial.longer_jump && i == self.owner)
    }

    pub(super) fn martial_end_turn(&mut self, i: usize) {
        if self.f[i].raging && self.f[i].martial.rage_expires <= self.turns {
            self.f[i].raging = false;
            self.subclass_rage_end(i);
        }
    }

    fn touches_caltrops(&self, i: usize, trap: Pos) -> bool {
        !(i < 2 && self.entities.mounted[i])
            && self.footprint(i).is_some_and(|body| {
                body.squares().any(|point| {
                    point.cell == trap && point.altitude == 0 && self.map.supported(point)
                })
            })
    }

    pub(super) fn place_readied_caltrops(&mut self, i: usize, target: Pos) {
        self.f[i].secondary -= 1;
        self.place_caltrops(i, target);
    }

    fn place_caltrops(&mut self, i: usize, target: Pos) {
        self.f[i].martial.caltrops = Some(target);
        self.f[i].martial.caltrops_inside =
            std::array::from_fn(|id| self.touches_caltrops(id, target));
    }

    pub(super) fn martial_moved(&mut self, i: usize) {
        for source in 0..2 {
            let Some(trap) = self.f[source].martial.caltrops else {
                continue;
            };
            let inside = self.touches_caltrops(i, trap);
            let state = &mut self.f[source].martial;
            let entered = inside && !state.caltrops_inside[i];
            state.caltrops_inside[i] = inside;
            if !entered || state.caltrops_triggered[i] == self.turns {
                continue;
            }
            state.caltrops_triggered[i] = self.turns;
            if !self.martial_save(i, 1, 15) {
                if let Some(fighter) = self.f.get_mut(i) {
                    fighter.speed_zero = true;
                } else if let Some(entity) = self.entity_mut(i) {
                    entity.martial.caltrops_stopped = true;
                }
                let before = self.footprint(i);
                self.damage_typed(i, 1, super::equipment::DamageType::Piercing);
                self.magic_blessing_at_zero(Some(source), i, before);
            }
        }
    }

    pub(super) fn martial_marked_search(&self, i: usize) -> bool {
        self.f[i].concentration == Some(super::magic::Concentration::HunterMark)
            && self.magic_mark_target(i) == Some(1 - i)
    }

    pub(super) fn martial_skill_mod(&self, i: usize, skill: Skill) -> (i32, bool) {
        self.martial_skill_mod_as(i, skill, self.origin_can_use_primal_strength(i, skill))
    }

    pub(in crate::arena2) fn martial_skill_mod_as(
        &self,
        i: usize,
        skill: Skill,
        use_primal: bool,
    ) -> (i32, bool) {
        let fighter = &self.f[i];
        let build = fighter.build;
        let stat = if use_primal && self.origin_can_use_primal_strength(i, skill) {
            0
        } else {
            skill.ability()
        };
        let advantage = self.divine_skill_advantage(i, skill) || (fighter.raging && stat == 0)
            || (build.class == Class::Fighter && build.variant == 0 && build.level >= 3 && skill == Skill::Athletics);
        let bonus = self.shape_skill_modifier(i, skill, stat);
        (bonus, advantage)
    }

    fn martial_enable_reckless(&mut self, i: usize) {
        self.f[i].reckless = true;
        self.f[i].martial.frenzy_enabled = self.f[i].raging;
    }

    pub(super) fn martial_force_save(&mut self, i: usize) {
        if i == self.owner && self.f[i].raging {
            self.f[i].martial.rage_expires = self.turns + 2;
        }
    }

    pub(super) fn martial_attack_rolled(&mut self, i: usize, target: usize) {
        if let Some(vex) = self.f[i].martial.vex_expires.get_mut(target) {
            *vex = -1;
        }
        if i == self.owner {
            self.f[i].martial.attack_rolled = true;
            if target < 4 && target % 2 != i {
                self.martial_force_save(i);
            }
        }
    }

    fn martial_sacred_active(&self, i: usize) -> bool {
        let state = &self.f[i].martial;
        state.sacred_expires >= self.turns
            && state
                .sacred_issuer
                .is_some_and(|issuer| self.gear_carries_weapon(i, issuer, false))
    }

    fn martial_sacred_strike(&self, i: usize) -> bool {
        self.martial_sacred_active(i)
            && self.f[i].martial.sacred_issuer == Some(self.gear_weapon_owner(i, false))
    }

    pub(super) fn martial_sacred_weapon_dropped(&mut self, i: usize, issuer: usize) {
        let state = &mut self.f[i].martial;
        if state.sacred_issuer == Some(issuer) {
            state.sacred_expires = -1;
            state.sacred_issuer = None;
        }
    }

    pub(super) fn martial_unarmored_active(&self, i: usize) -> bool {
        !self.armor_worn(i) && !self.gear_shield_worn(i)
    }

    pub(super) fn martial_arts_active(&self, i: usize) -> bool {
        self.f[i].build.class == Class::Monk
            && self.martial_unarmored_active(i)
            && (0..2).all(|issuer| {
                [false, true].into_iter().all(|ranged| {
                    !self.gear_wielded_by(i, issuer, ranged)
                        || self
                            .gear_weapon_kind_for(issuer, ranged)
                            .is_none_or(Self::is_monk_weapon)
                })
            })
    }

    fn is_monk_weapon(kind: WeaponKind) -> bool {
        matches!(
            kind,
            WeaponKind::Dagger
                | WeaponKind::Mace
                | WeaponKind::Quarterstaff
                | WeaponKind::Shortsword
        )
    }

    pub(super) fn martial_unarmed_ability(&self, i: usize) -> usize {
        if self.martial_arts_active(i) {
            self.weapon_finesse_ability(i)
        } else {
            0
        }
    }

    pub(super) fn martial_monk_weapon_stats(
        &self,
        i: usize,
        kind: WeaponKind,
    ) -> Option<(i32, i32)> {
        (Self::is_monk_weapon(kind) && self.martial_arts_active(i))
            .then(|| (self.martial_arts_die(i), self.weapon_finesse_modifier(i)))
    }

    pub(super) fn martial_arts_die(&self, i: usize) -> i32 {
        if self.f[i].build.level >= 5 {
            8
        } else {
            6
        }
    }

    pub(super) fn martial_attack_bonus(&self, i: usize, ranged: bool) -> i32 {
        let fighter = &self.f[i];
        match fighter.build.class {
            Class::Paladin if !ranged && self.martial_sacred_strike(i) => {
                fighter.build.stats[5].max(1)
            }
            Class::Ranger
                if ranged
                    && fighter.build.level >= 2
                    && self
                        .gear_weapon_kind(i, true)
                        .is_some_and(WeaponKind::is_bow) =>
            {
                2
            }
            _ => 0,
        }
    }

    pub(super) fn martial_has_vex(&self, i: usize) -> bool {
        self.f[i]
            .martial
            .vex_expires
            .iter()
            .any(|&turn| turn >= self.turns)
    }

    pub(super) fn martial_vex_against(&self, i: usize, target: usize) -> bool {
        self.f[i]
            .martial
            .vex_expires
            .get(target)
            .is_some_and(|&turn| turn >= self.turns)
    }

    fn hit_mastery(&self, hit: Hit) -> Option<MasteryWeapon> {
        if hit.attacker >= 2 || !hit.weapon || hit.unarmed || hit.improvised {
            return None;
        }
        let mastery = hit.weapon_kind?.mastery()?;
        let build = self.f[hit.attacker].build;
        (mastery == MasteryWeapon::PsychicBlade
            || super::origins::weapon_masteries(build.class, build.level).contains(&mastery))
            .then_some(mastery)
    }

    pub(super) fn martial_after_hit(&mut self, hit: Hit, dealt: i32) {
        let i = hit.attacker;
        match self.hit_mastery(hit) {
            Some(
                MasteryWeapon::Rapier | MasteryWeapon::Shortsword
                | MasteryWeapon::Shortbow | MasteryWeapon::PsychicBlade,
            )
                if dealt > 0 =>
            {
                self.f[i].martial.vex_expires[hit.defender] =
                    self.turns + if i == self.owner { 2 } else { 1 };
            }
            Some(MasteryWeapon::Longsword) => {
                if let Some(target) = self.f.get_mut(hit.defender) {
                    target.sap = true;
                } else {
                    self.entity_sap(i, hit.defender);
                }
            }
            _ => {}
        }
    }

    pub(super) fn martial_damage(&mut self, i: usize, hit: Hit) -> i32 {
        let Some(fighter) = self.f.get_mut(i) else {
            return 0;
        };
        if !hit.weapon
            || !hit.strength
            || fighter.build.class != Class::Barbarian
            || !fighter.raging
        {
            return 0;
        }
        let frenzy = fighter.build.variant == 0 && fighter.martial.frenzy_enabled
            && fighter.build.level >= 3
            && i == self.owner
            && fighter.martial.frenzy_turn != self.turns;
        let count = if frenzy {
            fighter.martial.frenzy_turn = self.turns;
            if hit.crit {
                4
            } else {
                2
            }
        } else {
            0
        };
        2 + (0..count).map(|_| self.roll(6)).sum::<i32>()
    }

    pub(super) fn martial_defend(&self, i: usize, hit: Hit) -> bool {
        let build = self.f[i].build;
        self.can_react(i)
            && match build.class {
                Class::Monk => build.level >= 3 && hit.physical,
                Class::Fighter => self.fighter_can_parry(i, hit),
                Class::Rogue => {
                    build.level >= 5
                        && hit.d20 > 0
                        && if hit.attacker < 2 {
                            self.can_see(i, hit.attacker)
                        } else {
                            self.entity_visible(i, hit.attacker)
                        }
                }
                _ => false,
            }
    }

    pub(super) fn martial_defense(&mut self, i: usize, _hit: Hit, damage: i32) -> i32 {
        let build = self.f[i].build;
        self.f[i].reaction = false;
        match build.class {
            Class::Monk => (damage - self.roll(10) - build.stats[1] - build.level).max(0),
            Class::Rogue => damage / 2,
            _ => unreachable!("martial reaction without applicable feature"),
        }
    }

    pub(super) fn martial_rider(&self, i: usize, hit: Hit) -> bool {
        let Some(fighter) = self.f.get(i) else {
            return false;
        };
        hit.weapon
            && match fighter.build.class {
                Class::Paladin => {
                    i == self.owner
                        && self.bonus
                        && (hit.unarmed || hit.weapon_kind.is_some_and(|kind| !kind.is_bow()))
                        && (fighter.martial.free_smite
                            || self.can_slot(i, 1)
                            || self.can_slot(i, 2))
                }
                Class::Monk => {
                    fighter.build.level >= 5
                        && fighter.resource > 0
                        && (hit.unarmed || hit.weapon_kind.is_some_and(Self::is_monk_weapon))
                        && fighter.martial.stunning_turn != self.turns
                }
                _ => false,
            }
    }

    pub(super) fn martial_followup(&mut self, i: usize, hit: Hit) -> i32 {
        match self.f[i].build.class {
            Class::Paladin => {
                self.bonus = false;
                let dice = if self.f[i].martial.free_smite {
                    self.f[i].martial.free_smite = false;
                    2
                } else if self.spend_slot(i, 2) {
                    3
                } else {
                    self.spend_slot(i, 1);
                    2
                };
                let count = if hit.crit { dice * 2 } else { dice };
                (0..count).map(|_| self.roll(8)).sum()
            }
            Class::Monk => {
                let build = self.f[i].build;
                self.f[i].resource -= 1;
                self.f[i].martial.stunning_turn = self.turns;
                let target = hit.defender;
                if target >= 2 {
                    self.martial_stun_entity(i, target, 8 + build.pb + build.stats[4]);
                    return 0;
                }
                if self.save(target, 2, 8 + build.pb + build.stats[4]) {
                    self.f[target].stun_slow = true;
                    self.f[target].stun_exposed = true;
                } else {
                    self.f[target].stunned = true;
                    self.f[target].raging = false;
                    self.f[target].concentration = None;
                }
                0
            }
            _ => unreachable!("martial rider without applicable feature"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{geometry::Pos, ACTS};

    fn starts(first: Class, second: Class, levels: [i32; 2]) -> impl Iterator<Item = Battle> {
        (0..256).filter_map(move |seed| {
            let mut battle = Battle::new(seed, [first, second], levels);
            if battle.owner != 0 {
                return None;
            }
            battle.f[0].pos = Pos { x: 0, y: 0 };
            battle.f[1].pos = Pos { x: 1, y: 0 };
            Some(battle)
        })
    }

    fn can(battle: &Battle, action: Act) -> bool {
        battle.mask() & action.bit() != 0
    }

    fn melee_hit(advantage: bool) -> Hit {
        Hit {
            attacker: 0,
            defender: 1,
            damage_kind: crate::arena2::equipment::DamageType::Slashing,
            unarmed: false,
            thrown: false,
            strength: true,
            improvised: false,
            weapon_kind: Some(crate::arena2::equipment::WeaponKind::Rapier),
            d20: 15,
            total: 20,
            ac: 12,
            ranged: false,
            weapon: true,
            physical: true,
            crit: false,
            advantage,
            disadvantage: false,
            flurry: false,
        }
    }

    #[test]
    fn rage_expires_after_the_next_turn_without_extension() {
        let mut battle = starts(Class::Barbarian, Class::Fighter, [3, 3])
            .next()
            .unwrap();
        assert_eq!(battle.f[0].resource, 3);
        battle.step(Act::Special1).unwrap();
        assert!(battle.f[0].raging);
        assert_eq!(battle.f[0].resource, 2);
        assert!(!battle.bonus);
        battle.step(Act::End).unwrap();
        assert!(battle.f[0].raging);
        battle.step(Act::End).unwrap();
        assert!(can(&battle, Act::Special3));
        battle.step(Act::End).unwrap();
        assert!(!battle.f[0].raging);
    }

    #[test]
    fn rage_extends_when_forcing_a_grapple_save() {
        let mut battle = starts(Class::Barbarian, Class::Fighter, [3, 3])
            .next()
            .unwrap();
        battle.step(Act::Special1).unwrap();
        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap();
        assert!(can(&battle, Act::Special3));
        battle.step(Act::Grapple).unwrap();
        battle.step(ACTS[0]).unwrap();
        assert!(!can(&battle, Act::Special3));
        battle.step(Act::End).unwrap();
        assert!(battle.f[0].raging);
    }

    #[test]
    fn danger_sense_changes_dexterity_saves_without_rage() {
        let changed = starts(Class::Barbarian, Class::Fighter, [2, 3]).any(|mut battle| {
            let mut normal = battle.clone();
            normal.f[0].build.level = 1;
            let regular = normal.save(0, 1, 15);
            let danger_sense = battle.save(0, 1, 15);
            assert!(!regular || danger_sense);
            danger_sense && !regular
        });
        assert!(changed);
    }

    #[test]
    fn rage_gives_strength_save_advantage_and_clears_concentration() {
        let improved = starts(Class::Barbarian, Class::Rogue, [1, 1]).any(|mut raging| {
            raging.f[0].concentration = Some(crate::arena2::magic::Concentration::Bless);
            raging.step(Act::Special1).unwrap();
            assert!(raging.f[0].concentration.is_none());
            let mut ordinary = raging.clone();
            ordinary.f[0].raging = false;
            let straight = ordinary.save(0, 0, 17);
            let advantage = raging.save(0, 0, 17);
            assert!(!straight || advantage);
            advantage && !straight
        });
        assert!(improved);
        let mut caster = starts(Class::Paladin, Class::Rogue, [1, 1]).next().unwrap();
        caster.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        assert!(can(&caster, Act::Spell1));
        // The synthetic flag verifies the shared spell casting restriction.
        caster.f[0].raging = true;
        assert!(!can(&caster, Act::Spell1));
    }

    #[test]
    fn reckless_is_optional_on_the_first_unarmed_attack_roll() {
        let mut battle = starts(Class::Barbarian, Class::Fighter, [2, 5])
            .next()
            .unwrap();
        battle.step(Act::Unarmed).unwrap();
        assert_eq!(battle.phase_id(), hits::RECKLESS);
        let mut decline = battle.clone();
        decline.step(Act::Pass).unwrap();
        assert!(!decline.f[0].reckless);
        battle.step(Act::Rider).unwrap();
        assert!(battle.f[0].reckless);
        assert!(battle.f[0].martial.attack_rolled);
        battle.step(Act::End).unwrap();
        assert!(battle.f[0].reckless);
        battle.step(Act::End).unwrap();
        assert!(!battle.f[0].reckless);
    }

    #[test]
    fn primal_knowledge_offers_strength_or_the_normal_ability() {
        let mut battle = starts(Class::Barbarian, Class::Rogue, [3, 3])
            .next()
            .unwrap();
        battle.step(Act::Special1).unwrap();
        battle.f[0].grappled = Some(14);
        battle.step(Act::EscapeAcrobatics).unwrap();
        assert_eq!(battle.phase_id(), choices::PRIMAL);
        assert!(can(&battle, Act::Rider));
        assert!(can(&battle, Act::Pass));
        let (primal, advantage) = battle.martial_skill_mod_as(0, Skill::Acrobatics, true);
        let (normal, other_advantage) = battle.martial_skill_mod_as(0, Skill::Acrobatics, false);
        assert!(primal > normal);
        assert!(advantage);
        assert!(!other_advantage);
        battle.step(Act::Rider).unwrap();
        assert_ne!(battle.phase_id(), choices::PRIMAL);
    }

    #[test]
    fn rage_damage_uses_the_actual_attack_ability() {
        let mut battle = starts(Class::Barbarian, Class::Fighter, [1, 1])
            .next()
            .unwrap();
        let strength_hit = melee_hit(false);
        assert_eq!(battle.martial_damage(0, strength_hit), 0);
        battle.step(Act::Special1).unwrap();
        let dexterity_hit = Hit {
            strength: false,
            ..strength_hit
        };
        let unarmed_hit = Hit {
            unarmed: true,
            ..strength_hit
        };
        assert_eq!(battle.martial_damage(0, dexterity_hit), 0);
        assert_eq!(battle.martial_damage(0, strength_hit), 2);
        assert_eq!(battle.martial_damage(0, unarmed_hit), 2);
    }

    #[test]
    fn frenzy_adds_two_dice_only_to_the_first_hit() {
        let mut battle = starts(Class::Barbarian, Class::Fighter, [5, 5])
            .next()
            .unwrap();
        battle.step(Act::Special1).unwrap();
        battle.f[0].reckless = true;
        battle.f[0].martial.frenzy_enabled = true;
        let hit = melee_hit(true);
        assert!((4..=14).contains(&battle.martial_damage(0, hit)));
        assert_eq!(battle.martial_damage(0, hit), 2);
    }

    #[test]
    fn longsword_sap_expires_at_the_sources_next_turn() {
        for class in [Class::Fighter, Class::Paladin] {
            let mut battle = starts(class, Class::Barbarian, [1, 3])
                .find_map(|mut battle| {
                    battle.step(Act::Melee).unwrap();
                    if battle.phase_id() == hits::SMITE {
                        battle.step(Act::Pass).unwrap();
                    }
                    battle.f[1].sap.then_some(battle)
                })
                .expect("a damaging longsword hit grants Sap");
            battle.step(Act::End).unwrap();
            assert!(battle.f[1].sap);
            battle.step(Act::End).unwrap();
            assert!(!battle.f[1].sap);
        }
    }

    #[test]
    fn fixed_unarmored_and_defense_bonuses_use_the_expected_stats() {
        let battle = starts(Class::Barbarian, Class::Monk, [5, 2])
            .next()
            .unwrap();
        let barbarian = battle.f[0].build;
        let monk = battle.f[1].build;
        assert_eq!(battle.ac(0), 10 + barbarian.stats[1] + barbarian.stats[2]);
        assert_eq!(battle.ac(1), 10 + monk.stats[1] + monk.stats[4]);
        assert_eq!(battle.speed(0), 8);
        assert_eq!(battle.speed(1), 8);
        let fighter = starts(Class::Fighter, Class::Paladin, [1, 2])
            .next()
            .unwrap();
        assert_eq!(fighter.ac(0), 16 + 2 + 1);
        assert_eq!(fighter.ac(1), 16 + 2 + 1);
        let first_level = starts(Class::Paladin, Class::Fighter, [1, 1])
            .next()
            .unwrap();
        assert_eq!(first_level.ac(0), 16 + 2);
    }

    #[test]
    fn tactical_mind_spends_wind_only_when_the_added_die_succeeds() {
        use crate::arena2::common::FailedCheck;
        let mut fighter = starts(Class::Fighter, Class::Barbarian, [2, 2])
            .next()
            .unwrap();
        let wind = fighter.f[0].resource;
        fighter.f[0].grappled = Some(15);
        let mut impossible = fighter.clone();
        let check = Check::Escape(Skill::Athletics);
        impossible.tactical_mind(
            0,
            FailedCheck {
                check,
                total: 4,
                dc: 15,
            },
        );
        assert_eq!(impossible.f[0].resource, wind);
        assert!(impossible.f[0].grappled.is_some());
        fighter.tactical_mind(
            0,
            FailedCheck {
                check,
                total: 14,
                dc: 15,
            },
        );
        assert_eq!(fighter.f[0].resource, wind - 1);
        assert!(fighter.f[0].grappled.is_none());
    }

    #[test]
    fn second_wind_uses_a_bonus_and_surge_restores_a_spent_action() {
        let mut battle = starts(Class::Fighter, Class::Barbarian, [3, 5])
            .next()
            .unwrap();
        assert_eq!(critical_threshold(battle.f[0].build), 19);
        assert_eq!(battle.f[0].resource, 2);
        assert!(can(&battle, Act::Special1));
        battle.f[0].hp = 1;
        battle.step(Act::Special1).unwrap();
        assert!((5..=14).contains(&battle.f[0].hp));
        assert_eq!(battle.f[0].resource, 1);
        assert!(!battle.bonus);
        assert!(battle.action);
        battle.step(Act::Dash).unwrap();
        battle.step(Act::Special2).unwrap();
        assert!(battle.action);
        assert_eq!(battle.f[0].secondary, 0);
    }

    #[test]
    fn champion_critical_offers_free_movement_without_opportunity() {
        let mut battle = starts(Class::Fighter, Class::Barbarian, [3, 5])
            .find_map(|mut battle| {
                battle.f[0].pos = Pos { x: 1, y: 0 };
                battle.f[1].pos = Pos { x: 0, y: 0 };
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SAFE_MOVE).then_some(battle)
            })
            .expect("a Champion critical grants safe movement");
        battle.step(Act::E).unwrap();
        assert_eq!(battle.f[0].pos, Pos { x: 2, y: 0 });
        assert!(battle.f[1].reaction);
    }

    #[test]
    fn champion_uses_initiative_advantage_and_strength_athletics() {
        use crate::dice::Dice;
        for seed in 0..64 {
            let battle = Battle::new(seed, [Class::Fighter, Class::Rogue], [3, 3]);
            let mut dice = Dice(seed);
            let left = dice.roll(20).max(dice.roll(20)) + battle.f[0].build.stats[1];
            let right = dice.roll(20) + battle.f[1].build.stats[1];
            let expected = if left == right {
                (dice.roll(2) - 1) as usize
            } else {
                usize::from(right > left)
            };
            assert_eq!(battle.owner, expected);
            assert!(battle.martial_skill_mod(0, Skill::Athletics).1);
            assert!(!battle.martial_skill_mod(0, Skill::Acrobatics).1);
        }
        let second_level = Battle::new(0, [Class::Fighter, Class::Rogue], [2, 3]);
        assert!(!second_level.martial_skill_mod(0, Skill::Athletics).1);
    }

    #[test]
    fn champion_critical_can_climb_using_only_the_special_movement_budget() {
        use crate::arena2::geometry::Point;
        let checked = starts(Class::Fighter, Class::Barbarian, [3, 5]).any(|mut battle| {
            let found = (0..crate::arena2::geometry::HEIGHT).find_map(|y| {
                (0..crate::arena2::geometry::WIDTH).find_map(|x| {
                    let point = Point::ground(Pos { x, y });
                    if !battle.map.can_occupy(point) || !battle.map.touches_wall(point) {
                        return None;
                    }
                    (0..8)
                        .filter_map(|dir| battle.map.step(point.cell, dir))
                        .next()
                        .map(|(foe, _)| (point.cell, foe))
                })
            });
            let Some((next, foe)) = found else {
                return false;
            };
            battle.f[0].pos = next;
            battle.f[1].pos = foe;
            battle.moved = battle.speed(0);
            if !can(&battle, Act::Melee) {
                return false;
            }
            battle.step(Act::Melee).unwrap();
            if battle.phase_id() != hits::SAFE_MOVE || !can(&battle, ACTS[8]) {
                return false;
            }
            let moved = battle.moved;
            battle.step(ACTS[8]).unwrap();
            assert_eq!(battle.f[0].elevation, 1);
            assert_eq!(battle.moved, moved);
            assert!(battle.f[1].reaction);
            true
        });
        assert!(checked);
    }

    #[test]
    fn champion_critical_can_jump_without_spending_regular_movement() {
        let mut battle = starts(Class::Fighter, Class::Barbarian, [3, 5])
            .find_map(|mut battle| {
                battle.f[0].pos = Pos { x: 1, y: 0 };
                battle.f[1].pos = Pos { x: 0, y: 0 };
                battle.moved = battle.speed(0);
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SAFE_MOVE && can(&battle, Act::JumpLong))
                    .then_some(battle)
            })
            .expect("critical grants movement including a standing jump");
        let moved = battle.moved;
        battle.step(Act::JumpLong).unwrap();
        assert_eq!(battle.phase_id(), hits::SAFE_JUMP);
        battle.step(ACTS[12]).unwrap(); // one square east
        assert_eq!(battle.f[0].pos, Pos { x: 2, y: 0 });
        assert_eq!(battle.moved, moved);
        assert!(battle.f[1].reaction);
    }

    #[test]
    fn flurry_is_available_before_an_action_and_spends_one_focus() {
        let mut battle = starts(Class::Monk, Class::Fighter, [2, 5]).next().unwrap();
        assert!(can(&battle, Act::Special1));
        assert!(can(&battle, Act::Special2));
        battle.step(Act::Special2).unwrap();
        assert!(battle.action);
        assert!(!battle.bonus);
        assert_eq!(battle.f[0].resource, 1);
        assert!(!can(&battle, Act::Special1));
    }

    #[test]
    fn focus_only_spends_on_enhanced_patient_defense_and_step_of_the_wind() {
        let start = starts(Class::Monk, Class::Fighter, [2, 1]).next().unwrap();

        let mut patient = start.clone();
        patient.step(Act::Special3).unwrap();
        assert!(patient.disengaged);
        assert!(patient.f[0].dodge);
        assert_eq!(patient.f[0].resource, 1);

        let mut dash = start.clone();
        dash.step(Act::Special4).unwrap();
        assert_eq!(dash.dash, dash.speed(0));
        assert!(!dash.disengaged);
        assert_eq!(dash.f[0].resource, 2);

        let mut disengage = start.clone();
        disengage.step(Act::Special5).unwrap();
        assert!(disengage.disengaged);
        assert!(!disengage.f[0].dodge);
        assert_eq!(disengage.f[0].resource, 2);

        let mut step = start;
        step.step(Act::Special6).unwrap();
        assert!(step.disengaged);
        assert_eq!(step.dash, step.speed(0));
        assert_eq!(step.martial_jump_multiplier(0), 2);
        assert_eq!(step.f[0].resource, 1);
    }

    #[test]
    fn monk_martial_arts_ability_changes_bonus_grapple_dc() {
        for ability in 0..2 {
            let grappled = starts(Class::Monk, Class::Rogue, [1, 1]).any(|mut battle| {
                let expected = 8 + battle.f[0].build.pb + battle.f[0].build.stats[ability];
                battle.step(Act::Other6).unwrap();
                battle.step(ACTS[ability]).unwrap();
                battle.step(Act::Martial4).unwrap();
                battle.step(Act::Grapple).unwrap();
                battle.step(ACTS[1]).unwrap();
                battle.step(ACTS[0]).unwrap();
                match battle.f[1].grappled {
                    Some(dc) => {
                        assert_eq!(dc, expected);
                        assert!(battle.action);
                        assert!(!battle.bonus);
                        true
                    }
                    None => false,
                }
            });
            assert!(grappled);
        }
    }

    #[test]
    fn fast_hands_can_don_and_doff_a_borrowed_shield_as_a_bonus_action() {
        for level in [2, 3] {
            let mut battle = starts(Class::Rogue, Class::Fighter, [level, 1])
                .next()
                .unwrap();
            let square = battle.fighter_point(0);
            battle.shield_on_ground(1, square);
            assert!(can(&battle, Act::Gear8));
            assert_eq!(can(&battle, Act::Martial5), level >= 3);
            if level < 3 {
                continue;
            }
            battle.step(Act::Martial5).unwrap();
            assert!(battle.gear_shield_worn(0));
            assert_eq!(battle.shield_holder(1), Some(0));
            assert!(battle.action);
            assert!(!battle.bonus);
            assert!(can(&battle, Act::Gear7));
            assert!(!can(&battle, Act::Martial4));

            battle.bonus = true;
            battle.step(Act::Martial4).unwrap();
            assert!(!battle.gear_shield_worn(0));
            assert!(battle.action);
            assert!(!battle.bonus);
        }
    }

    #[test]
    fn martial_arts_bonus_unarmed_can_shove_while_preserving_the_action() {
        let shoved = starts(Class::Monk, Class::Rogue, [1, 1]).any(|mut battle| {
            battle.step(Act::Martial4).unwrap();
            assert_eq!(battle.phase_id(), unarmed::UNARMED_MODE);
            assert!(battle.bonus);
            battle.step(Act::ShoveProne).unwrap();
            assert_eq!(battle.phase_id(), unarmed::UNARMED_TARGET);
            battle.step(ACTS[1]).unwrap();
            assert_eq!(battle.phase_id(), 12);
            assert_eq!(battle.actor(), Some(1));
            battle.step(ACTS[0]).unwrap();
            assert!(battle.action);
            assert!(!battle.bonus);
            battle.f[1].prone
        });
        assert!(shoved);
    }

    #[test]
    fn flurry_can_grapple_and_shove_with_one_focus_point() {
        let checked = starts(Class::Monk, Class::Rogue, [2, 3]).any(|mut battle| {
            battle.step(Act::Martial5).unwrap();
            battle.step(Act::Grapple).unwrap();
            battle.step(ACTS[1]).unwrap();
            battle.step(ACTS[0]).unwrap();
            assert_eq!(battle.f[0].resource, 1);
            assert_eq!(battle.phase_id(), unarmed::UNARMED_MODE);
            let grappled = battle.f[1].grappled.is_some();
            battle.step(Act::ShoveProne).unwrap();
            battle.step(ACTS[1]).unwrap();
            battle.step(ACTS[0]).unwrap();
            assert_eq!(battle.f[0].resource, 1);
            assert!(battle.action);
            assert!(!battle.bonus);
            grappled && battle.f[1].prone
        });
        assert!(checked);
    }

    #[test]
    fn defensive_reactions_consume_their_reaction_at_the_expected_level() {
        let mut monk = starts(Class::Fighter, Class::Monk, [3, 3]).next().unwrap();
        let hit = melee_hit(false);
        let mut same_roll = monk.clone();
        let expected = 25 - same_roll.roll(10) - monk.f[1].build.stats[1] - 3;
        assert!(monk.martial_defend(1, hit));
        assert_eq!(monk.martial_defense(1, hit, 25), expected);
        assert!(!monk.martial_defend(1, hit));

        let low_rogue = starts(Class::Fighter, Class::Rogue, [3, 4]).next().unwrap();
        let mut rogue = starts(Class::Fighter, Class::Rogue, [3, 5]).next().unwrap();
        assert!(!low_rogue.martial_defend(1, hit));
        assert!(rogue.martial_defend(1, hit));
        assert_eq!(rogue.martial_defense(1, hit, 25), 12);
        assert!(!rogue.martial_defend(1, hit));
    }

    #[test]
    fn lay_on_hands_spends_only_restored_hit_points() {
        let mut battle = starts(Class::Paladin, Class::Rogue, [1, 1]).next().unwrap();
        assert_eq!(battle.f[0].resource, 5);
        battle.f[0].hp -= 3;
        battle.step(Act::Special1).unwrap();
        assert_eq!(battle.phase_id(), choices::LAY_TARGET);
        battle.step(ACTS[0]).unwrap();
        assert_eq!(battle.phase_id(), choices::LAY_AMOUNT);
        battle.step(ACTS[2]).unwrap();
        assert_eq!(battle.f[0].hp, battle.f[0].build.hp - 1);
        assert_eq!(battle.f[0].resource, 3);
        assert!(!battle.bonus);
        assert!(battle.action);
    }

    #[test]
    fn divine_smite_resolves_only_after_a_hit_and_first_use_preserves_slots() {
        let mut battle = starts(Class::Paladin, Class::Barbarian, [2, 5])
            .find_map(|mut battle| {
                assert_ne!(battle.phase_id(), hits::SMITE);
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SMITE).then_some(battle)
            })
            .expect("a melee hit grants a rider choice");
        let slots = battle.f[0].slots;
        let hp = battle.f[1].hp;
        let mut declined = battle.clone();
        declined.step(Act::Pass).unwrap();
        assert!(declined.f[0].martial.free_smite);
        assert!(declined.bonus);
        battle.step(ACTS[0]).unwrap();
        assert!(!battle.f[0].martial.free_smite);
        assert_eq!(battle.f[0].slots, slots);
        assert!(!battle.bonus);
        assert!(battle.f[1].hp < hp);
    }

    #[test]
    fn sacred_weapon_buffs_the_attack_without_using_a_bonus_action() {
        let mut battle = starts(Class::Paladin, Class::Barbarian, [3, 5])
            .next()
            .unwrap();
        let channels = battle.f[0].secondary;
        assert_eq!(battle.martial_attack_bonus(0, false), 0);
        battle.step(Act::Special2).unwrap();
        assert!(!battle.action);
        assert!(battle.bonus);
        assert_eq!(battle.f[0].secondary, channels - 1);
        assert_eq!(
            battle.martial_attack_bonus(0, false),
            battle.f[0].build.stats[5].max(1)
        );
    }

    #[test]
    fn colossus_slayer_checks_existing_wounds_and_is_once_per_turn() {
        let mut battle = starts(Class::Ranger, Class::Fighter, [5, 5])
            .find_map(|mut battle| {
                assert_eq!(battle.martial_attack_bonus(0, true), 2);
                battle.f[1].hp -= 1;
                battle.bonus = false;
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::COLOSSUS).then_some(battle)
            })
            .expect("a wounded target qualifies for Colossus Slayer");
        let mut declined = battle.clone();
        declined.step(Act::Pass).unwrap();
        assert_ne!(declined.f[0].martial.colossus_turn, declined.turns);
        battle.step(Act::Rider).unwrap();
        assert_eq!(battle.f[0].martial.colossus_turn, battle.turns);
        assert!(battle.f[1].hp < declined.f[1].hp);
        battle.step(Act::Melee).unwrap();
        assert_ne!(battle.phase_id(), hits::COLOSSUS);
    }

    #[test]
    fn fixed_paladin_and_ranger_prepared_spells_unlock_at_their_prescribed_levels() {
        for level in 1..=5 {
            for class in [Class::Paladin, Class::Ranger] {
                let mut battle = starts(class, Class::Barbarian, [level, 1]).next().unwrap();
                battle.f[0].hp -= 2;
                let slots = match level {
                    1 | 2 => [2, 0, 0],
                    3 | 4 => [3, 0, 0],
                    _ => [4, 2, 0],
                };
                assert_eq!(battle.f[0].slots, slots);
                let gates: &[(Act, i32)] = if class == Class::Paladin {
                    &[
                        (Act::Spell1, 1),
                        (Act::Spell0, 2),
                        (Act::Spell2, 2),
                        (Act::Magic8, 3),
                        (Act::Magic15, 3),
                        (Act::Magic16, 3),
                        (Act::Magic9, 4),
                        (Act::Magic1, 5),
                        (Act::Magic10, 5),
                        (Act::Magic17, 5),
                    ]
                } else {
                    &[
                        (Act::Spell0, 1),
                        (Act::Spell1, 1),
                        (Act::Magic1, 1),
                        (Act::Magic2, 2),
                        (Act::Magic3, 3),
                        (Act::Magic5, 5),
                    ]
                };
                for &(action, unlock) in gates {
                    let mut ready = battle.clone();
                    if class == Class::Paladin {
                        use crate::arena2::equipment::Carried;
                        if action == Act::Magic10 {
                            ready.f[0].gear.shield = Carried::Packed;
                        } else {
                            ready.f[0].gear.melee = Carried::Packed;
                        }
                    }
                    assert_eq!(
                        can(&ready, action),
                        level >= unlock,
                        "{class:?} level {level}, {action:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn hunters_mark_transfers_from_a_defeated_companion_without_another_cast() {
        let mut battle = starts(Class::Ranger, Class::Paladin, [5, 5])
            .next()
            .unwrap();
        battle.owner = 1;
        place_fiend_steed(&mut battle, Pos { x: 0, y: 1 });
        battle.owner = 0;
        let slots = battle.f[0].slots;
        battle.step(Act::Spell0).unwrap();
        for _ in 0..4 {
            if battle.magic_mark_target(0).is_some() {
                break;
            }
            let action = if can(&battle, ACTS[3]) {
                ACTS[3]
            } else {
                ACTS[battle.mask().trailing_zeros() as usize]
            };
            battle.step(action).unwrap();
        }
        assert_eq!(battle.magic_mark_target(0), Some(3));
        assert_eq!(battle.f[0].secondary, 2);
        assert_eq!(battle.f[0].slots, slots);
        assert!(!battle.martial_marked_search(0));
        battle.entity_damage(3, 100);
        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap();
        battle.step(Act::Magic24).unwrap();
        battle.step(ACTS[1]).unwrap();
        assert_eq!(battle.magic_mark_target(0), Some(1));
        assert_eq!(battle.f[0].secondary, 2);
        assert_eq!(battle.f[0].slots, slots);
        assert!(battle.martial_marked_search(0));
        let extra = battle.magic_damage(0, melee_hit(false));
        assert!((1..=6).contains(&extra));
    }

    #[test]
    fn hunter_lore_reports_the_current_marked_creatures_resistances() {
        use crate::arena2::equipment::DamageType;
        let mut battle = starts(Class::Ranger, Class::Barbarian, [3, 3])
            .next()
            .unwrap();
        battle.f[1].raging = true;
        assert!(battle.hunters_lore(0).is_none());
        battle.step(Act::Spell0).unwrap();
        for _ in 0..3 {
            if battle.hunters_lore(0).is_some() {
                break;
            }
            assert_ne!(battle.phase_id(), 0);
            let choice = if can(&battle, ACTS[1]) {
                ACTS[1]
            } else {
                ACTS[battle.mask().trailing_zeros() as usize]
            };
            battle.step(choice).unwrap();
        }
        let lore = battle.hunters_lore(0).unwrap();
        assert!(lore.contains(&DamageType::Bludgeoning));
        assert!(lore.contains(&DamageType::Slashing));
        assert!(lore.contains(&DamageType::Poison));
        for variant in [1, 2] {
            let mut other_ranger = battle.clone();
            other_ranger.f[0].build.variant = variant;
            assert_eq!(other_ranger.magic_mark_target(0), Some(1));
            assert!(other_ranger.hunters_lore(0).is_none());
        }
        battle.f[1].raging = false;
        assert_eq!(battle.hunters_lore(0), Some(vec![DamageType::Poison]));
        battle.step(Act::DropConcentration).unwrap();
        assert!(battle.hunters_lore(0).is_none());
    }

    #[test]
    fn ranger_and_rogue_apply_their_selected_expertise() {
        let battle = starts(Class::Ranger, Class::Rogue, [2, 3]).next().unwrap();
        let r = battle.f[0].build;
        let q = battle.f[1].build;
        assert_eq!(
            battle.martial_skill_mod(0, Skill::Perception).0,
            r.stats[4] + 2 * r.pb
        );
        assert_eq!(
            battle.martial_skill_mod(1, Skill::Stealth).0,
            q.stats[1] + 2 * q.pb
        );
    }

    #[test]
    fn longbow_slow_can_be_accepted_or_declined_after_damage() {
        use crate::arena2::equipment::Carried;
        let mut hit = starts(Class::Ranger, Class::Barbarian, [2, 5])
            .find_map(|mut battle| {
                battle.f[0].gear.melee = Carried::Packed;
                battle.f[0].gear.bow = Carried::Held;
                battle.f[1].pos = Pos { x: 3, y: 0 };
                battle.step(Act::Ranged).unwrap();
                (battle.phase_id() == hits::SLOW).then_some(battle)
            })
            .expect("a longbow hit deals damage that qualifies for Slow");
        let mut declined = hit.clone();
        declined.step(Act::Pass).unwrap();
        hit.step(Act::Rider).unwrap();
        assert!(hit.f[1].slow);
        assert!(!declined.f[1].slow);
        assert_eq!(hit.speed(1), declined.speed(1) - 2);
    }

    #[test]
    fn lay_on_hands_can_cure_poison_and_restore_a_selected_amount() {
        let mut battle = starts(Class::Paladin, Class::Rogue, [2, 5]).next().unwrap();
        battle.f[0].hp -= 5;
        battle.f[1].martial.poison_until[0] = battle.turns + 20;
        battle.step(Act::Special1).unwrap();
        battle.step(ACTS[0]).unwrap();
        battle.step(ACTS[34]).unwrap();
        assert!(!battle.martial_poisoned(0));
        assert_eq!(battle.f[0].hp, battle.f[0].build.hp - 3);
        assert_eq!(battle.f[0].resource, 3);
    }

    #[test]
    fn open_hand_allows_a_choice_and_declining_does_not_addle() {
        for mut novice in starts(Class::Monk, Class::Fighter, [2, 1]).take(32) {
            novice.step(Act::Special2).unwrap();
            assert_ne!(novice.phase_id(), hits::OPEN_HAND);
        }
        let mut choice = starts(Class::Monk, Class::Fighter, [3, 1])
            .find_map(|mut battle| {
                battle.step(Act::Special2).unwrap();
                (battle.phase_id() == hits::OPEN_HAND).then_some(battle)
            })
            .expect("a Flurry hit offers an Open Hand option");
        for option in &ACTS[..5] {
            assert!(can(&choice, *option));
        }
        assert!(can(&choice, Act::Pass));
        let mut decline = choice.clone();
        decline.step(Act::Pass).unwrap();
        assert!(!decline.f[1].addled);
        choice.step(ACTS[0]).unwrap();
        assert!(choice.f[1].addled);
    }

    #[test]
    fn open_hand_push_and_topple_apply_their_own_saves() {
        let successful = |option, applied: fn(&Battle) -> bool| {
            starts(Class::Monk, Class::Fighter, [3, 3]).any(|mut battle| {
                battle.step(Act::Special2).unwrap();
                if battle.phase_id() != hits::OPEN_HAND {
                    return false;
                }
                battle.step(ACTS[option]).unwrap();
                applied(&battle)
            })
        };
        assert!(successful(2, |battle| battle.f[1].pos == Pos { x: 3, y: 0 }));
        assert!(successful(4, |battle| battle.f[1].prone));
    }

    #[test]
    fn stunning_strike_and_open_hand_have_separate_choices() {
        let checked = starts(Class::Monk, Class::Fighter, [5, 5]).any(|mut battle| {
            battle.step(Act::Special2).unwrap();
            if battle.phase_id() != hits::STUN {
                return false;
            }
            let focus = battle.f[0].resource;
            battle.step(Act::Rider).unwrap();
            assert_eq!(battle.f[0].resource, focus - 1);
            assert!(battle.f[1].stunned || battle.f[1].stun_slow);
            assert_eq!(battle.phase_id(), hits::OPEN_HAND);
            battle.step(Act::Pass).unwrap();
            assert_ne!(battle.phase_id(), hits::STUN);
            true
        });
        assert!(checked);
    }

    fn place_fiend_steed(battle: &mut Battle, destination: Pos) {
        use crate::arena2::entities::{Entity, EntityKind, Spirit};

        let side = battle.owner;
        let kind = EntityKind::Steed {
            spirit: Spirit::Fiend,
            spell_level: 2,
        };
        battle.fixture_companion(side, Entity::new(kind, side, destination, 10));
    }

    #[test]
    fn divine_sense_reveals_a_fiend_steed_and_spends_shared_channel() {
        let mut battle = starts(Class::Paladin, Class::Barbarian, [5, 1])
            .next()
            .unwrap();
        let destination = Pos { x: 0, y: 1 };
        place_fiend_steed(&mut battle, destination);
        assert!(battle.divine_sense_types(0).is_empty());
        battle.step(Act::Martial1).unwrap();
        assert_eq!(battle.f[0].secondary, 1);
        assert_eq!(
            battle.divine_sense_types(0),
            vec![(2, crate::arena2::entities::Spirit::Fiend, destination)]
        );
    }

    #[test]
    fn a_bonus_grapple_can_target_an_enemy_steed() {
        let checked = starts(Class::Monk, Class::Paladin, [2, 5]).any(|mut battle| {
            battle.owner = 1;
            place_fiend_steed(&mut battle, Pos { x: 0, y: 1 });
            battle.owner = 0;
            battle.action = true;
            battle.step(Act::Martial4).unwrap();
            battle.step(Act::Grapple).unwrap();
            battle.step(ACTS[3]).unwrap();
            assert_eq!(battle.actor(), Some(1));
            battle.step(ACTS[0]).unwrap();
            battle
                .entity(3)
                .unwrap()
                .martial
                .grappled
                .is_some_and(|(source, _)| source == 0)
        });
        assert!(checked);
    }

    #[test]
    fn making_an_airborne_companion_prone_causes_a_fall() {
        let mut battle = starts(Class::Paladin, Class::Monk, [5, 3]).next().unwrap();
        place_fiend_steed(&mut battle, Pos { x: 0, y: 1 });
        let steed = battle.entity_mut(2).unwrap();
        steed.altitude = 2;
        let hp_before = steed.hp;
        battle.entity_knock_prone(2);
        let landed = battle.entity(2).unwrap();
        assert_eq!(landed.altitude, 0);
        assert!(landed.hp < hp_before);
        assert!(landed.martial.prone);
    }

    #[test]
    fn cleave_offers_only_the_second_reachable_creature_and_can_be_declined() {
        let mut battle = starts(Class::Barbarian, Class::Paladin, [1, 5])
            .find_map(|mut battle| {
                battle.owner = 1;
                place_fiend_steed(&mut battle, Pos { x: 0, y: 1 });
                battle.owner = 0;
                battle.action = true;
                battle.step(Act::Melee).unwrap();
                if can(&battle, ACTS[1]) && battle.phase_id() != 0 {
                    battle.step(ACTS[1]).unwrap();
                }
                (battle.phase_id() == hits::CLEAVE).then_some(battle)
            })
            .expect("a Greataxe hit next to a steed grants a Cleave option");
        let mut declined = battle.clone();
        assert_eq!(battle.mask(), Act::Pass.bit() | ACTS[3].bit());
        declined.step(Act::Pass).unwrap();
        assert_ne!(declined.f[0].martial.cleave_turn, declined.turns);
        battle.step(ACTS[3]).unwrap();
        assert_eq!(battle.f[0].martial.cleave_turn, battle.turns);
    }

    #[test]
    fn deflect_redirect_requires_zero_damage_and_spends_one_focus() {
        let mut choice = starts(Class::Fighter, Class::Monk, [1, 3])
            .find_map(|mut battle| {
                battle.step(Act::Melee).unwrap();
                if battle.phase_id() != 2 {
                    return None;
                }
                battle.step(Act::Defend).unwrap();
                (battle.phase_id() == hits::REDIRECT).then_some(battle)
            })
            .expect("a Monk can fully deflect a weapon hit");
        assert!(!choice.f[1].reaction);
        let focus = choice.f[1].resource;
        let mut decline = choice.clone();
        decline.step(Act::Pass).unwrap();
        assert_eq!(decline.f[1].resource, focus);
        choice.step(ACTS[1]).unwrap();
        assert_eq!(choice.f[1].resource, focus - 1);
    }

    #[test]
    fn second_level_smite_spends_the_slot_and_preserves_the_free_use() {
        let mut choice = starts(Class::Paladin, Class::Barbarian, [5, 5])
            .find_map(|mut battle| {
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SMITE).then_some(battle)
            })
            .expect("a melee hit offers all available Smite costs");
        assert!(can(&choice, ACTS[0]));
        assert!(can(&choice, ACTS[1]));
        assert!(can(&choice, ACTS[2]));
        let slots = choice.f[0].slots;
        choice.step(ACTS[2]).unwrap();
        assert!(choice.f[0].martial.free_smite);
        assert_eq!(choice.f[0].slots, [slots[0], slots[1] - 1, slots[2]]);
    }

    #[test]
    fn sacred_weapon_can_choose_radiant_to_bypass_rage_resistance() {
        let mut choice = starts(Class::Paladin, Class::Barbarian, [3, 5])
            .find_map(|mut battle| {
                battle.f[1].raging = true;
                battle.bonus = false;
                battle.step(Act::Special2).unwrap();
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SACRED).then_some(battle)
            })
            .expect("a Sacred Weapon hit offers a damage type");
        let mut physical = choice.clone();
        physical.step(Act::Pass).unwrap();
        choice.step(Act::Rider).unwrap();
        assert!(choice.f[1].hp < physical.f[1].hp);
    }

    #[test]
    fn cunning_poison_repeats_a_save_at_the_targets_turn_end() {
        let cured = starts(Class::Rogue, Class::Fighter, [5, 5]).any(|mut battle| {
            battle.f[0].martial.poison_until[1] = battle.turns + 20;
            battle.step(Act::End).unwrap();
            assert!(battle.martial_poisoned(1));
            battle.step(Act::End).unwrap();
            !battle.martial_poisoned(1)
        });
        assert!(cured);
    }

    #[test]
    fn cunning_trip_can_knock_prone_and_sneak_can_be_declined() {
        let saw_trip = starts(Class::Rogue, Class::Fighter, [5, 5]).any(|mut battle| {
            battle.step(Act::Special1).unwrap();
            battle.step(Act::Melee).unwrap();
            if battle.phase_id() != hits::SNEAK {
                return false;
            }
            let mut declined = battle.clone();
            declined.step(Act::Pass).unwrap();
            assert_ne!(declined.f[0].martial.sneak_turn, declined.turns);
            assert!(!declined.f[1].prone);
            battle.step(ACTS[2]).unwrap();
            assert_eq!(battle.f[0].martial.sneak_turn, battle.turns);
            battle.f[1].prone
        });
        assert!(saw_trip);
    }

    #[test]
    fn cunning_withdraw_allows_safe_steps_after_a_hit() {
        let mut choice = starts(Class::Rogue, Class::Fighter, [5, 5])
            .find_map(|mut battle| {
                battle.f[0].pos = Pos { x: 1, y: 0 };
                battle.f[1].pos = Pos { x: 0, y: 0 };
                battle.f[0].martial.vex_expires[1] = battle.turns;
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SNEAK).then_some(battle)
            })
            .expect("a Vex hit offers Cunning Strike");
        choice.step(ACTS[3]).unwrap();
        assert_eq!(choice.phase_id(), hits::SAFE_MOVE);
        choice.step(Act::E).unwrap();
        assert_eq!(choice.f[0].pos, Pos { x: 2, y: 0 });
        assert!(choice.f[1].reaction);
    }

    #[test]
    fn cunning_strike_poison_requires_tools_and_sacrifices_damage_dice() {
        let mut choice = starts(Class::Rogue, Class::Fighter, [5, 5])
            .find_map(|mut battle| {
                battle.step(Act::Special1).unwrap();
                battle.step(Act::Melee).unwrap();
                (battle.phase_id() == hits::SNEAK).then_some(battle)
            })
            .expect("a Steady Aim hit qualifies for Sneak Attack");
        let mut unequipped = choice.clone();
        unequipped.f[0].gear.poisoners_kit = false;
        assert!(!can(&unequipped, ACTS[1]));
        assert!(can(&unequipped, ACTS[2]));
        assert!(can(&unequipped, ACTS[3]));
        let mut full = choice.clone();
        full.step(ACTS[0]).unwrap();
        choice.step(ACTS[1]).unwrap();
        assert!(choice.f[1].hp > full.f[1].hp);
        assert_eq!(choice.f[0].martial.sneak_turn, choice.turns);
    }

    #[test]
    fn fast_hands_sets_caltrops_that_stop_an_entering_enemy_on_failed_save() {
        let low = starts(Class::Rogue, Class::Fighter, [2, 1]).next().unwrap();
        assert!(!can(&low, Act::Special4));
        let mut failed = starts(Class::Rogue, Class::Fighter, [3, 1])
            .find_map(|mut battle| {
                let trap = Pos { x: 1, y: 1 };
                battle.step(Act::Special4).unwrap();
                assert_eq!(battle.target_request(), Some((Act::Special4, None)));
                battle.step(ACTS[trap.y as usize]).unwrap();
                assert_eq!(battle.target_request(), Some((Act::Special4, Some(trap.y))));
                assert_eq!(battle.f[0].secondary, 1);
                assert!(battle.bonus);
                battle.step(ACTS[trap.x as usize]).unwrap();
                assert_eq!(battle.caltrops(0), Some(trap));
                assert_eq!(battle.f[0].secondary, 0);
                assert!(!battle.bonus);
                assert!(battle.action);
                battle.step(Act::End).unwrap();
                battle.step(Act::S).unwrap();
                assert_eq!(battle.f[1].pos, trap);
                battle.f[1].speed_zero.then_some(battle)
            })
            .expect("an entering fighter fails at least one Dexterity save");
        assert_eq!(failed.f[1].hp, failed.f[1].build.hp - 1);
        assert_eq!(failed.movement_left(), 0);
        failed.step(Act::End).unwrap();
        failed.step(Act::End).unwrap();
        assert!(!failed.f[1].speed_zero);
        assert!(failed.movement_left() > 0);
    }

    #[test]
    fn caltrops_trigger_when_a_large_companions_footprint_enters() {
        use crate::arena2::geometry::Point;
        let checked = starts(Class::Rogue, Class::Paladin, [3, 5]).any(|mut battle| {
            battle.owner = 1;
            place_fiend_steed(&mut battle, Pos { x: 2, y: 1 });
            battle.owner = 0;
            battle.place_caltrops(0, Pos { x: 1, y: 1 });
            let hp = battle.entity(3).unwrap().hp;
            assert!(battle.entity_force_move(3, Point::ground(Pos { x: 0, y: 1 })));
            let entered = battle.entity(3).unwrap();
            if entered.hp == hp {
                return false;
            }
            assert_eq!(entered.position, Pos { x: 0, y: 1 });
            assert!(entered.martial.caltrops_stopped);
            let after_hit = entered.hp;
            battle.turns += 1;
            battle.martial_moved(3);
            assert_eq!(battle.entity(3).unwrap().hp, after_hit);
            true
        });
        assert!(checked);
    }

    #[test]
    fn a_thief_can_disarm_an_adjacent_caltrops_trap_with_either_action_type() {
        let checked = starts(Class::Rogue, Class::Rogue, [3, 3]).any(|mut battle| {
            battle.place_caltrops(1, Pos { x: 1, y: 1 });
            let mut ordinary = battle.clone();
            battle.step(Act::Martial6).unwrap();
            assert_eq!(battle.phase_id(), choices::DISARM_TRAP);
            battle.step(ACTS[9]).unwrap();
            assert!(battle.action);
            assert!(!battle.bonus);
            ordinary.step(Act::Martial7).unwrap();
            ordinary.step(ACTS[9]).unwrap();
            assert!(!ordinary.action);
            assert!(ordinary.bonus);
            assert_eq!(battle.caltrops(1), ordinary.caltrops(1));
            battle.caltrops(1).is_none()
        });
        assert!(checked);
    }

    #[test]
    fn a_thief_can_spread_caltrops_with_the_ordinary_utilize_action() {
        let mut battle = starts(Class::Rogue, Class::Fighter, [3, 1]).next().unwrap();
        battle.step(Act::Utilize).unwrap();
        battle.step(ACTS[1]).unwrap();
        battle.step(ACTS[1]).unwrap();
        assert!(!battle.action);
        assert!(battle.bonus);
        assert_eq!(battle.caltrops(0), Some(Pos { x: 1, y: 1 }));
        assert_eq!(battle.f[0].secondary, 0);
        assert!(!can(&battle, Act::Special4));
    }

    #[test]
    fn tactical_shift_moves_after_spent_speed_without_provoking() {
        let mut battle = starts(Class::Fighter, Class::Rogue, [5, 1]).next().unwrap();
        battle.f[0].pos = Pos { x: 1, y: 0 };
        battle.f[1].pos = Pos { x: 0, y: 0 };
        battle.moved = battle.f[0].build.speed;
        assert!(!can(&battle, Act::E));
        battle.step(Act::Special1).unwrap();
        assert_eq!(battle.tactical_steps, 3);
        battle.step(Act::E).unwrap();
        assert_eq!(battle.f[0].pos, Pos { x: 2, y: 0 });
        assert!(battle.f[1].reaction);
        assert_eq!(battle.phase_id(), 0);
        assert_eq!(battle.tactical_steps, 2);
    }

    #[test]
    fn tactical_shift_can_jump_then_walk_with_one_shared_half_speed_budget() {
        let mut battle = starts(Class::Fighter, Class::Rogue, [5, 1])
            .find_map(|mut battle| {
                battle.f[0].pos = Pos { x: 1, y: 0 };
                battle.f[1].pos = Pos { x: 0, y: 0 };
                battle.moved = battle.speed(0);
                assert_eq!(battle.movement_left(), 0);
                assert!(!can(&battle, Act::JumpLong));
                battle.step(Act::Special1).unwrap();
                battle.step(Act::JumpLong).ok()?;
                can(&battle, ACTS[12]).then_some(battle)
            })
            .unwrap();
        let moved = battle.moved;
        assert_eq!(battle.tactical_steps, 3);
        battle.step(ACTS[12]).unwrap(); // one square east, out of the Rogue's reach
        assert_eq!(battle.phase_id(), 0);
        assert_eq!(battle.f[0].pos, Pos { x: 2, y: 0 });
        assert_eq!(battle.tactical_steps, 2);
        battle.step(Act::E).unwrap();
        battle.step(Act::E).unwrap();
        assert_eq!(battle.f[0].pos, Pos { x: 4, y: 0 });
        assert_eq!(battle.moved, moved);
        assert!(battle.f[1].reaction);
        assert_eq!(battle.movement_left(), 0);
        assert!(!can(&battle, Act::E));
        assert!(!can(&battle, Act::JumpLong));
    }

    #[test]
    fn tactical_shift_can_climb_after_ordinary_movement_is_spent() {
        use crate::arena2::geometry::{Point, HEIGHT, WIDTH};

        let mut battle = starts(Class::Fighter, Class::Rogue, [5, 1]).next().unwrap();
        let (climber, foe) = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| Pos { x, y }))
            .find_map(|cell| {
                let origin = Point::ground(cell);
                let elevated = Point {
                    altitude: 1,
                    ..origin
                };
                if !battle.map.can_occupy(origin)
                    || !battle.map.touches_wall(origin)
                    || !battle.map.touches_wall(elevated)
                    || !battle.map.can_occupy(elevated)
                {
                    return None;
                }
                let foe = (0..8).find_map(|dir| battle.map.step(cell, dir))?.0;
                Some((cell, foe))
            })
            .unwrap();
        battle.f[0].pos = climber;
        battle.f[1].pos = foe;
        battle.moved = battle.speed(0);
        let moved = battle.moved;
        assert!(!can(&battle, Act::Climb));
        battle.step(Act::Special1).unwrap();
        battle.step(Act::Climb).unwrap();
        battle.step(ACTS[0]).unwrap(); // up 5 feet, costs 10 feet without a climb Speed
        assert_eq!(battle.phase_id(), 0);
        assert_eq!(battle.f[0].elevation, 1);
        assert_eq!(battle.tactical_steps, 1);
        assert_eq!(battle.moved, moved);
        assert!(battle.f[1].reaction);
        assert!(!can(&battle, Act::Climb));
    }

    #[test]
    fn cunning_hide_uses_only_the_bonus_action() {
        let hidden = (0..128).any(|seed| {
            let mut battle = Battle::new(seed, [Class::Rogue, Class::Fighter], [2, 2]);
            battle.owner = 0;
            if !can(&battle, Act::Martial1) {
                return false;
            }
            battle.step(Act::Martial1).unwrap();
            assert!(battle.action);
            assert!(!battle.bonus);
            battle.f[0].hidden_dc.is_some()
        });
        assert!(hidden);
    }

    #[test]
    fn ranger_shortsword_vex_is_consumed_only_against_its_target() {
        let mut battle = starts(Class::Ranger, Class::Fighter, [1, 1])
            .find_map(|mut battle| {
                battle.step(Act::Melee).unwrap();
                battle.martial_vex_against(0, 1).then_some(battle)
            })
            .expect("a damaging shortsword hit grants Vex");
        battle.martial_attack_rolled(0, 3);
        assert!(battle.martial_vex_against(0, 1));
        battle.martial_attack_rolled(0, 1);
        assert!(!battle.martial_vex_against(0, 1));
    }

    #[test]
    fn sneak_attack_dice_scale_and_an_adjacent_ally_substitutes_for_advantage() {
        use crate::arena2::entities::{Entity, EntityKind, FamiliarForm, Spirit};
        assert_eq!(
            (1..=5).map(sneak_attack_dice).collect::<Vec<_>>(),
            vec![1, 1, 2, 2, 3]
        );
        let checked = starts(Class::Rogue, Class::Fighter, [1, 5]).any(|mut battle| {
            let cat = EntityKind::Familiar {
                form: FamiliarForm::Cat,
                spirit: Spirit::Fey,
            };
            battle.fixture_companion(0, Entity::new(cat, 0, Pos { x: 1, y: 1 }, 10));
            battle.step(Act::Melee).unwrap();
            battle.phase_id() == hits::SNEAK
        });
        assert!(checked);
    }

    #[test]
    fn steady_aim_locks_movement_and_vex_tracks_each_target() {
        let mut battle = starts(Class::Rogue, Class::Fighter, [3, 5]).next().unwrap();
        battle.step(Act::Special1).unwrap();
        assert!(battle.f[0].attack_advantage);
        assert_eq!(battle.movement_left(), 0);

        battle.martial_after_hit(melee_hit(true), 1);
        assert!(battle.martial_vex_against(0, 1));
        assert!(!battle.martial_vex_against(0, 2));
        battle.martial_attack_rolled(0, 2);
        assert!(battle.martial_has_vex(0));
        battle.martial_attack_rolled(0, 1);
        assert!(!battle.martial_has_vex(0));
    }
}
