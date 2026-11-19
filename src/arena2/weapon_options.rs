//! Weapon properties and their explicit actor choices.

use super::{
    battle::Battle,
    geometry::{Point, Pos, HEIGHT, MAX_ELEVATION, WIDTH},
    origins::raw_scores,
    weapon::WeaponKind,
    Act,
};

const PHASE: i32 = 72;
const GUESS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Light,
    LightThrow,
    Bash,
    Throw,
}

impl Mode {
    fn is_ranged(self) -> bool {
        matches!(self, Self::Throw | Self::LightThrow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Strike {
    issuer: usize,
    from_bow: bool,
    mode: Mode,
}

#[derive(Clone, Copy, Debug)]
enum Choice {
    Finesse,
    LightWeapon,
    ImprovisedWeapon,
    Aim(Strike),
    Row(Strike),
    Column(Strike, i8),
    Altitude(Strike, Pos),
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct WeaponOptions {
    finesse: Option<usize>,
    attacked_with_light: u8,
    light_spent: bool,
    choice: Option<Choice>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct OptionAttack {
    pub kind: Option<WeaponKind>,
    pub die: i32,
    pub damage_bonus: i32,
    pub to_hit: i32,
    pub ranged: bool,
    pub strength: bool,
    pub long_range: bool,
    pub bonus_action: bool,
    pub improvised: bool,
}

impl WeaponKind {
    pub fn light(self) -> bool {
        matches!(self, Self::Dagger | Self::Shortsword)
    }

    pub fn heavy(self) -> bool {
        matches!(self, Self::Greataxe | Self::Longbow)
    }
}

impl Battle {
    fn weapon_options(&self, i: usize) -> WeaponOptions {
        self.f[i].gear.weapon_options
    }

    pub(super) fn weapon_options_turn_start(&mut self, i: usize) {
        let options = &mut self.f[i].gear.weapon_options;
        options.attacked_with_light = 0;
        options.light_spent = false;
        options.choice = None;
    }

    pub(super) fn weapon_finesse_ability(&self, i: usize) -> usize {
        let stats = self.f[i].build.stats;
        self.weapon_options(i)
            .finesse
            .unwrap_or_else(|| usize::from(stats[1] > stats[0]))
    }

    pub(super) fn weapon_finesse_modifier(&self, i: usize) -> i32 {
        self.f[i].build.stats[self.weapon_finesse_ability(i)]
    }

    pub(super) fn weapon_heavy_disadvantage(&self, i: usize, kind: Option<WeaponKind>) -> bool {
        let Some(kind) = kind.filter(|kind| kind.heavy()) else {
            return false;
        };
        let build = self.f[i].build;
        raw_scores(build.class, build.level)[usize::from(kind.is_bow())] < 13
    }

    pub(super) fn weapon_note_light(&mut self, i: usize, issuer: usize, kind: Option<WeaponKind>) {
        if kind.is_some_and(WeaponKind::light) && !self.magic_is_wolf(i) {
            self.f[i].gear.weapon_options.attacked_with_light |= 1 << issuer;
        }
    }

    pub(super) fn weapon_options_mask(&self, i: usize) -> u128 {
        if self.magic_is_wolf(i) {
            return 0;
        }
        let mut mask = 0;
        if self.martial_arts_active(i)
            || (0..2).any(|issuer| {
                self.gear_carries_weapon(i, issuer, false)
                    && self
                        .gear_weapon_kind_for(issuer, false)
                        .is_some_and(WeaponKind::finesse)
            })
        {
            mask |= Act::Other6.bit();
        }
        if self.bonus && !self.weapon_options(i).light_spent && self.light_weapons(i) != 0 {
            mask |= Act::Other7.bit();
        }
        if (self.action || self.attacks > 0) && self.improvised_weapons(i) != 0 {
            mask |= Act::Other8.bit();
        }
        mask
    }

    pub(super) fn weapon_options_label(action: Act) -> Option<&'static str> {
        match action {
            Act::Other6 => Some("Choose ability for Finesse or Martial Arts"),
            Act::Other7 => Some("Light extra attack (Bonus Action)"),
            Act::Other8 => Some("Improvised weapon attack (Attack)"),
            _ => None,
        }
    }

    pub(super) fn weapon_options_step(&mut self, i: usize, action: Act) {
        let choice = match action {
            Act::Other6 => Choice::Finesse,
            Act::Other7 => Choice::LightWeapon,
            Act::Other8 => Choice::ImprovisedWeapon,
            _ => unreachable!("validated weapon option"),
        };
        self.weapon_request(i, choice);
    }

    fn weapon_request(&mut self, i: usize, choice: Choice) {
        self.f[i].gear.weapon_options.choice = Some(choice);
        self.request(i, PHASE);
    }

    fn light_weapons(&self, i: usize) -> u128 {
        let attacked_with = self.weapon_options(i).attacked_with_light;
        (0..4)
            .filter(|&option| {
                let issuer = option % 2;
                let throwing = option >= 2;
                let mode = if throwing {
                    Mode::LightThrow
                } else {
                    Mode::Light
                };
                attacked_with & !(1 << issuer) != 0
                    && self.gear_wielded_by(i, issuer, false)
                    && self
                        .gear_weapon_kind_for(issuer, false)
                        .is_some_and(|kind| {
                            if throwing {
                                kind == WeaponKind::Dagger
                            } else {
                                kind.light()
                            }
                        })
                    && self.aim_mask(i, mode) != 0
            })
            .fold(0, |mask, option| mask | 1 << option)
    }

    fn improvised(id: usize) -> Strike {
        Strike {
            issuer: id % 2,
            from_bow: id >= 2,
            mode: if (2..4).contains(&id) {
                Mode::Bash
            } else {
                Mode::Throw
            },
        }
    }

    fn improvised_weapons(&self, i: usize) -> u128 {
        (0..6)
            .filter(|&id| {
                let strike = Self::improvised(id);
                self.gear_wielded_by(i, strike.issuer, strike.from_bow)
                    && self
                        .gear_weapon_kind_for(strike.issuer, strike.from_bow)
                        .is_some_and(|weapon| weapon != WeaponKind::Dagger)
                    && self.aim_mask(i, strike.mode) != 0
            })
            .fold(0, |mask, id| mask | 1 << id)
    }

    fn weapon_target(&self, viewer: usize, mode: Mode, target: usize) -> Option<Point> {
        if self.suggestion_charmed_by(viewer, target) {
            return None;
        }
        self.observed_footprint(viewer, target)?
            .squares()
            .find(|&point| self.reaches(viewer, mode, point))
    }

    fn reaches(&self, i: usize, mode: Mode, point: Point) -> bool {
        let from = self.fighter_point(i);
        let distance = from.dist(point);
        let range = if mode.is_ranged() { 12 } else { 1 };
        distance <= range && self.map.can_occupy(point) && self.map.los_at(from, point)
    }

    fn aim_mask(&self, i: usize, mode: Mode) -> u128 {
        let visible = (0..4)
            .filter(|&target| target % 2 != i && self.weapon_target(i, mode, target).is_some())
            .fold(0, |mask, target| mask | 1 << target);
        if self.observed_pos(i, 1 - i).is_none() && self.weapon_rows(i, mode) != 0 {
            visible | 1 << GUESS
        } else {
            visible
        }
    }

    fn weapon_altitudes(&self, i: usize, mode: Mode, cell: Pos) -> u128 {
        (0..=MAX_ELEVATION)
            .filter(|&altitude| self.reaches(i, mode, Point { cell, altitude }))
            .fold(0, |mask, z| mask | 1 << z)
    }

    fn weapon_columns(&self, i: usize, mode: Mode, y: i8) -> u128 {
        (0..WIDTH)
            .filter(|&x| self.weapon_altitudes(i, mode, Pos { x, y }) != 0)
            .fold(0, |mask, x| mask | 1 << x)
    }

    fn weapon_rows(&self, i: usize, mode: Mode) -> u128 {
        (0..HEIGHT)
            .filter(|&y| self.weapon_columns(i, mode, y) != 0)
            .fold(0, |mask, y| mask | 1 << y)
    }

    pub(super) fn weapon_option_choice_mask(&self, i: usize) -> u128 {
        let Some(choice) = self.weapon_options(i).choice else {
            return 0;
        };
        let offered = match choice {
            Choice::Finesse => 0b111 & !(1 << self.weapon_options(i).finesse.unwrap_or(2)),
            Choice::LightWeapon => self.light_weapons(i),
            Choice::ImprovisedWeapon => self.improvised_weapons(i),
            Choice::Aim(strike) => self.aim_mask(i, strike.mode),
            Choice::Row(strike) => self.weapon_rows(i, strike.mode),
            Choice::Column(strike, row) => self.weapon_columns(i, strike.mode, row),
            Choice::Altitude(strike, cell) => self.weapon_altitudes(i, strike.mode, cell),
        };
        offered | Act::Pass.bit()
    }

    pub(super) fn weapon_option_paired_choices(&self, i: usize) -> std::ops::Range<usize> {
        match self.weapon_options(i).choice {
            Some(Choice::LightWeapon | Choice::Aim(_)) => 0..4,
            Some(Choice::ImprovisedWeapon) => 0..6,
            _ => 0..0,
        }
    }

    pub(super) fn weapon_option_choice_label(&self, i: usize, action: Act) -> Option<String> {
        if action == Act::Pass && self.weapon_options(i).choice.is_some() {
            return Some("Cancel weapon choice".into());
        }
        let id = action as usize;
        Some(match self.weapon_options(i).choice? {
            Choice::Finesse => {
                ["Use Strength", "Use Dexterity", "Prefer higher modifier"][id].into()
            }
            Choice::LightWeapon => {
                let issuer = id % 2;
                let weapon = self.gear_weapon_kind_for(issuer, false)?;
                let owner = if issuer == i { "issued" } else { "borrowed" };
                let verb = if id >= 2 { "Throw" } else { "Strike with" };
                format!("{verb} {owner} {}", weapon.name())
            }
            Choice::ImprovisedWeapon => {
                let strike = Self::improvised(id);
                let weapon = self.gear_weapon_kind_for(strike.issuer, strike.from_bow)?;
                let owner = if strike.issuer == i {
                    "issued"
                } else {
                    "borrowed"
                };
                if strike.mode == Mode::Bash {
                    format!("Use {owner} {} as a club", weapon.name())
                } else {
                    format!("Throw {owner} {} as an improvised weapon", weapon.name())
                }
            }
            Choice::Aim(_) if id == GUESS => "Guess target location".into(),
            Choice::Aim(_) if id < 2 => "Attack opposing duelist".into(),
            Choice::Aim(_) => format!("Attack {}", self.observed_entity(i, id)?.kind.name()),
            Choice::Row(_) => format!("Row {}", id + 1),
            Choice::Column(_, row) => format!("Cell {}{}", char::from(b'A' + id as u8), row + 1),
            Choice::Altitude(..) => format!("{} ft above the floor", id * 5),
        })
    }

    pub(super) fn weapon_option_choice_step(&mut self, i: usize, action: Act) {
        let id = action as usize;
        let choice = self.f[i]
            .gear
            .weapon_options
            .choice
            .take()
            .expect("weapon choice");
        if action == Act::Pass {
            return;
        }
        match choice {
            Choice::Finesse => {
                self.f[i].gear.weapon_options.finesse = (id < 2).then_some(id);
            }
            Choice::LightWeapon => self.weapon_request(
                i,
                Choice::Aim(Strike {
                    issuer: id % 2,
                    from_bow: false,
                    mode: if id >= 2 {
                        Mode::LightThrow
                    } else {
                        Mode::Light
                    },
                }),
            ),
            Choice::ImprovisedWeapon => self.weapon_request(i, Choice::Aim(Self::improvised(id))),
            Choice::Aim(strike) if id == GUESS => self.weapon_request(i, Choice::Row(strike)),
            Choice::Aim(strike) => {
                let p = self
                    .weapon_target(i, strike.mode, id)
                    .expect("masked weapon target");
                self.finish_weapon_option(i, strike, p, Some(id));
            }
            Choice::Row(strike) => self.weapon_request(i, Choice::Column(strike, id as i8)),
            Choice::Column(strike, y) => {
                self.weapon_request(i, Choice::Altitude(strike, Pos { x: id as i8, y }));
            }
            Choice::Altitude(strike, cell) => {
                let p = Point {
                    cell,
                    altitude: id as i8,
                };
                let target = (0..4).find(|&target| {
                    !self.suggestion_charmed_by(i, target)
                        && self
                            .footprint(target)
                            .is_some_and(|body| body.squares().any(|point| point == p))
                });
                self.finish_weapon_option(i, strike, p, target);
            }
        }
    }

    fn finish_weapon_option(
        &mut self,
        i: usize,
        strike: Strike,
        target: Point,
        defender: Option<usize>,
    ) {
        let spec = match strike.mode {
            Mode::Light | Mode::LightThrow => {
                let ranged = strike.mode.is_ranged();
                self.gear_choose_for_attack(i, strike.issuer, false);
                self.f[i].gear.weapon_options.light_spent = true;
                let kind = self
                    .gear_weapon_kind_for(strike.issuer, false)
                    .expect("light weapon");
                let modifier = self.weapon_finesse_modifier(i);
                let build = self.f[i].build;
                let proficiency = if kind.proficient(build) { build.pb } else { 0 };
                let enhancement = self.magic_weapon_bonus(i, false);
                OptionAttack {
                    kind: Some(kind),
                    die: kind.die(false),
                    damage_bonus: modifier.min(0) + enhancement,
                    to_hit: modifier
                        + proficiency
                        + enhancement
                        + self.martial_attack_bonus(i, false),
                    ranged,
                    strength: self.weapon_finesse_ability(i) == 0,
                    long_range: ranged && self.fighter_point(i).dist(target) > 4,
                    bonus_action: true,
                    improvised: false,
                }
            }
            Mode::Bash | Mode::Throw => {
                let ranged = strike.mode == Mode::Throw;
                let modifier = self.f[i].build.stats[usize::from(ranged)];
                OptionAttack {
                    kind: None,
                    die: 4,
                    damage_bonus: modifier,
                    to_hit: modifier,
                    ranged,
                    strength: !ranged,
                    long_range: ranged && self.fighter_point(i).dist(target) > 4,
                    bonus_action: false,
                    improvised: true,
                }
            }
        };
        self.weapon_option_attack(i, defender, spec);
        if strike.mode.is_ranged() && !self.sanctuary_defer_throw(strike.issuer, strike.from_bow) {
            self.gear_drop_weapon_instance(i, strike.issuer, strike.from_bow, target.cell);
        }
    }

    /// Option-specific public information. No guessed target is resolved here.
    pub fn weapon_option_candidate(&self, i: usize, id: usize) -> [f32; 8] {
        let mut out = [0.; 8];
        if id >= 128 || self.weapon_option_choice_mask(i) & (1 << id) == 0 {
            return out;
        }
        out[0] = 1.;
        if id == Act::Pass as usize {
            out[7] = 1.;
            return out;
        }
        let choice = self.weapon_options(i).choice.expect("masked weapon option");
        match choice {
            Choice::Finesse => {
                let stats = self.f[i].build.stats;
                out[1] = id as f32 / 2.;
                out[2] = if id == 2 {
                    stats[0].max(stats[1])
                } else {
                    stats[id]
                } as f32
                    / 5.;
            }
            Choice::LightWeapon | Choice::ImprovisedWeapon => {
                let strike = if matches!(choice, Choice::LightWeapon) {
                    Strike {
                        issuer: id % 2,
                        from_bow: false,
                        mode: if id >= 2 {
                            Mode::LightThrow
                        } else {
                            Mode::Light
                        },
                    }
                } else {
                    Self::improvised(id)
                };
                let kind = self
                    .gear_weapon_kind_for(strike.issuer, strike.from_bow)
                    .expect("masked weapon");
                out[1] = (kind as usize + 1) as f32 / 9.;
                out[2] = u8::from(i != strike.issuer).into();
                out[3] = u8::from(strike.mode.is_ranged()).into();
                out[4] = u8::from(strike.from_bow).into();
            }
            Choice::Aim(_) if id == GUESS => out[7] = 1.,
            Choice::Aim(strike) => {
                let position = self.weapon_target(i, strike.mode, id).expect("masked target");
                out[1] = (position.cell.x + 1) as f32 / f32::from(WIDTH);
                out[2] = (position.cell.y + 1) as f32 / f32::from(HEIGHT);
                out[3] = f32::from(position.altitude) / f32::from(MAX_ELEVATION);
                out[4] = self.fighter_point(i).dist(position) as f32 / 12.;
                out[5] = u8::from(id >= 2).into();
            }
            Choice::Row(_) => out[2] = (id + 1) as f32 / f32::from(HEIGHT),
            Choice::Column(_, y) => {
                out[1] = (id + 1) as f32 / f32::from(WIDTH);
                out[2] = f32::from(y + 1) / f32::from(HEIGHT);
            }
            Choice::Altitude(_, point) => {
                out[1] = f32::from(point.x + 1) / f32::from(WIDTH);
                out[2] = f32::from(point.y + 1) / f32::from(HEIGHT);
                out[3] = id as f32 / f32::from(MAX_ELEVATION);
            }
        }
        out
    }

    /// Public features for the actor's equipment menu; [0; 8] outside it.
    pub fn weapon_option_features(&self, i: usize) -> [f32; 8] {
        let options = self.weapon_options(i);
        let (phase, strike, row) = match options.choice {
            None => (0, None, None),
            Some(Choice::Finesse) => (1, None, None),
            Some(Choice::LightWeapon) => (2, None, None),
            Some(Choice::ImprovisedWeapon) => (3, None, None),
            Some(Choice::Aim(strike)) => (4, Some(strike), None),
            Some(Choice::Row(strike)) => (5, Some(strike), None),
            Some(Choice::Column(strike, row)) => (6, Some(strike), Some(row)),
            Some(Choice::Altitude(strike, cell)) => (7, Some(strike), Some(cell.y)),
        };
        [
            phase as f32 / 7.,
            options
                .finesse
                .map_or(0., |ability| (ability + 1) as f32 / 2.),
            (options.attacked_with_light.count_ones() as f32) / 2.,
            u8::from(options.light_spent).into(),
            strike.map_or(0., |value| match value.mode {
                Mode::Light => 0.25,
                Mode::LightThrow => 0.50,
                Mode::Bash => 0.75,
                Mode::Throw => 1.,
            }),
            strike.map_or(0., |value| u8::from(value.issuer != i).into()),
            strike.map_or(0., |value| u8::from(value.from_bow).into()),
            row.map_or(0., |value| f32::from(value + 1) / f32::from(HEIGHT)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{class::Class, equipment::Carried, ACTS};

    fn adjacent(classes: [Class; 2]) -> Battle {
        let mut b = Battle::new(1, classes, [1; 2]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 1, y: 0 };
        b.f[1].hp = 100;
        b
    }

    #[test]
    fn charmed_condition_excludes_the_source_from_equipment_attack_modes() {
        use crate::arena2::{magic::Concentration, spell_movement::Suggestion};
        let mut battle = adjacent([Class::Fighter, Class::Warlock]);
        for mode in [Mode::Light, Mode::LightThrow, Mode::Bash, Mode::Throw] {
            assert_ne!(battle.aim_mask(0, mode) & ACTS[1].bit(), 0);
        }
        battle.f[1].concentration = Some(Concentration::Suggestion);
        battle.activate_suggestion(1, 0, Suggestion::RefrainAttack);
        for mode in [Mode::Light, Mode::LightThrow, Mode::Bash, Mode::Throw] {
            assert_eq!(battle.aim_mask(0, mode) & ACTS[1].bit(), 0);
        }
    }

    fn finish_reactions(b: &mut Battle) {
        for _ in 0..12 {
            if b.phase_id() == 0 {
                return;
            }
            let mask = b.mask();
            let action = if mask & Act::Pass.bit() != 0 {
                Act::Pass
            } else {
                ACTS[mask.trailing_zeros() as usize]
            };
            b.step(action).unwrap();
        }
        panic!("attack did not finish");
    }

    #[test]
    fn finesse_menu_changes_the_same_modifier_for_attack_and_damage() {
        let mut b = adjacent([Class::Bard, Class::Warlock]);
        let strength = b.f[0].build.stats[0];
        let dexterity = b.f[0].build.stats[1];
        assert_ne!(strength, dexterity);
        b.step(Act::Other6).unwrap();
        assert_eq!(b.phase_id(), PHASE);
        assert_eq!(b.choice_label(0, ACTS[0]), "Use Strength");
        assert_eq!(b.weapon_option_candidate(0, 0)[0], 1.);
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.gear_weapon_stats(0, false).unwrap().1, strength);
        b.step(Act::Other6).unwrap();
        assert_eq!(b.mask() & ACTS[0].bit(), 0);
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.gear_weapon_stats(0, false).unwrap().1, dexterity);
        assert!(b.action && b.bonus);
    }

    #[test]
    fn heavy_uses_the_exact_relevant_score_for_a_borrowed_item() {
        let b = adjacent([Class::Wizard, Class::Cleric]);
        assert!(b.weapon_heavy_disadvantage(0, Some(WeaponKind::Greataxe)));
        assert!(!b.weapon_heavy_disadvantage(0, Some(WeaponKind::Longbow)));
        assert!(b.weapon_heavy_disadvantage(1, Some(WeaponKind::Longbow)));
        assert!(!b.weapon_heavy_disadvantage(1, Some(WeaponKind::Greataxe)));
        assert!(!b.weapon_heavy_disadvantage(0, Some(WeaponKind::Rapier)));
    }

    #[test]
    fn light_bonus_attack_requires_another_actual_weapon_instance() {
        let mut b = adjacent([Class::Ranger, Class::Warlock]);
        b.f[1].gear.melee = Carried::ForeignHeld;
        assert_eq!(b.mask() & Act::Other7.bit(), 0);
        b.step(Act::Melee).unwrap();
        finish_reactions(&mut b);
        assert_ne!(b.mask() & Act::Other7.bit(), 0);
        b.step(Act::Other7).unwrap();
        assert_eq!(b.mask() & ACTS[0].bit(), 0);
        assert_ne!(b.mask() & ACTS[1].bit(), 0);
        assert_ne!(b.mask() & ACTS[3].bit(), 0, "borrowed dagger can be thrown");
        b.step(ACTS[1]).unwrap();
        b.step(ACTS[1]).unwrap();
        finish_reactions(&mut b);
        assert!(!b.bonus);
        assert!(b.weapon_options(0).light_spent);
        assert_eq!(b.gear_weapon_owner(0, false), 1);
    }

    #[test]
    fn cancelling_an_aim_choice_preserves_weapon_and_action() {
        let mut b = adjacent([Class::Cleric, Class::Warlock]);
        b.step(Act::Other8).unwrap();
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.choice_label(0, Act::Pass), "Cancel weapon choice");
        b.step(Act::Pass).unwrap();
        assert_eq!(b.phase_id(), 0);
        assert!(b.action && b.bonus);
        assert_eq!(b.f[0].gear.melee, Carried::Held);
    }

    #[test]
    fn improvised_throw_commits_attack_and_drops_exact_item() {
        let mut b = adjacent([Class::Cleric, Class::Warlock]);
        b.step(Act::Other8).unwrap();
        assert!(b.choice_label(0, ACTS[0]).contains("Throw issued Mace"));
        assert!(b.action);
        b.step(ACTS[0]).unwrap();
        assert!(b.action);
        b.step(ACTS[1]).unwrap();
        finish_reactions(&mut b);
        assert!(!b.action);
        assert_eq!(b.f[0].gear.melee, Carried::Ground(b.f[1].pos));
    }

    #[test]
    fn improvised_bow_bash_leaves_bow_held_and_spends_no_arrows() {
        let mut b = adjacent([Class::Bard, Class::Warlock]);
        b.f[0].gear.bow = Carried::Held;
        let arrows = b.f[0].arrows;
        b.step(Act::Other8).unwrap();
        assert!(b.choice_label(0, ACTS[2]).contains("Shortbow as a club"));
        b.step(ACTS[2]).unwrap();
        b.step(ACTS[1]).unwrap();
        finish_reactions(&mut b);
        assert!(!b.action);
        assert_eq!(b.f[0].gear.bow, Carried::Held);
        assert_eq!(b.f[0].arrows, arrows);
    }
}
