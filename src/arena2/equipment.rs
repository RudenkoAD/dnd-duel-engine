//! Objects carried by the twelve fixed loadouts. Board coordinates are in five-foot cells.
use super::geometry::{Point, HEIGHT, MAX_ELEVATION, WIDTH};
use super::origins::{weapon_masteries, MasteryWeapon};
pub use super::weapon::WeaponKind;
use super::{
    battle::Battle,
    class::{Build, Class},
    geometry::Pos,
    Act,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Material {
    None,
    Ordinary,
    Diamond50,
    DiamondDust25,
    Diamond300,
    Incense10,
    GildedAcorn200,
    HolySymbol5,
    Weapon1cp,
    HolyWater25,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Carried {
    Absent,
    Packed,
    Held,
    Ground(Pos),
    ForeignHeld,
    ForeignPacked,
}

#[derive(Clone, Debug)]
pub struct GearState {
    pub melee: Carried,
    pub bow: Carried,
    /// Stars uses the secondary equipment slot for a nonweapon Star Map.
    pub(super) star_map: bool,
    pub shield: Carried,
    pub shield_loose: bool,
    shield_ground_height: i8,
    pub interaction: bool,
    pub after_attack: bool,
    pub versatile_two: bool,
    pub poisoners_kit: bool,
    pub diamond50: bool,
    pub diamonds300: u8,
    diamond_dust25: u8,
    pub incense10: u8,
    pub holy_water25: u8,
    preferred_foreign: [bool; 2],
    pub throw_cell: Pos,
    pub(super) weapon_options: super::weapon_options::WeaponOptions,
}

impl GearState {
    pub fn new(build: Build) -> Self {
        use Carried::*;
        use Class::*;
        let star_map = build.is_subclass(Druid, 1) && build.level >= 3;
        Self {
            melee: if build.class == Monk { Absent } else if star_map { Packed } else { Held },
            bow: if star_map { Held } else if build.ranged_die > 0 { Packed } else { Absent },
            star_map,
            shield: if star_map {
                Packed
            } else if matches!(build.class, Cleric | Druid | Fighter | Paladin) {
                Held
            } else {
                Absent
            },
            shield_loose: false,
            shield_ground_height: 0,
            interaction: true,
            after_attack: false,
            preferred_foreign: [false; 2],
            throw_cell: Pos { x: 0, y: 0 },
            weapon_options: super::weapon_options::WeaponOptions::default(),
            versatile_two: false,
            poisoners_kit: build.class == Rogue && build.level >= 5,
            diamond50: build.class == Sorcerer,
            diamonds300: 0,
            diamond_dust25: u8::from(build.is_subclass(Cleric, 2)),
            incense10: u8::from(build.class == Wizard),
            holy_water25: u8::from(build.class == Paladin && build.level >= 3
                || build.level >= 3 && (build.is_subclass(Wizard, 1) || build.is_subclass(Sorcerer, 2))),
        }
    }

    fn occupied(&self) -> usize {
        [self.melee, self.bow, self.shield]
            .iter()
            .filter(|&&s| s == Carried::Held)
            .count()
    }

    pub fn describe(&self) -> String {
        format!(
            "melee {}, {} {}, shield {}{}",
            self.melee.description(),
            if self.star_map { "Star Map" } else { "bow" },
            self.bow.description(),
            self.shield.description(),
            if self.versatile_two {
                ", versatile two-handed grip"
            } else {
                ""
            }
        )
    }
}

impl Carried {
    fn available(self, here: Point, landing: Point) -> bool {
        self == Self::Packed || self == Self::Ground(here.cell) && here == landing
    }
    fn description(self) -> &'static str {
        match self {
            Self::Absent => "none",
            Self::Packed => "stowed",
            Self::Held => "equipped",
            Self::Ground(_) => "on the ground",
            Self::ForeignHeld | Self::ForeignPacked => "taken",
        }
    }
}

impl Battle {
    pub(in crate::arena2) fn star_map_held(&self, i: usize) -> bool {
        let gear = &self.f[i].gear;
        gear.star_map && gear.bow == Carried::Held && !self.magic_is_wolf(i)
    }

    pub(in crate::arena2) fn gear_item_name_for(&self, issuer: usize, secondary: bool) -> Option<&'static str> {
        if secondary && self.f[issuer].gear.star_map {
            Some("Star Map")
        } else {
            self.gear_weapon_kind_for(issuer, secondary).map(WeaponKind::name)
        }
    }

    pub fn gear_free_hand(&self, i: usize) -> bool {
        !self.magic_is_wolf(i) && self.occupied_for_strike(i) < 2
    }

    pub(super) fn gear_two_free_hands(&self, i: usize) -> bool {
        !self.magic_is_wolf(i) && self.occupied_for_strike(i) == 0
    }

    fn occupied_for_strike(&self, i: usize) -> usize {
        let other = &self.f[1 - i].gear;
        self.f[i].gear.occupied()
            + self.martial_entity_grapple_count(i) as usize
            + usize::from(self.f[1 - i].grappled.is_some())
            + usize::from(other.melee == Carried::ForeignHeld)
            + usize::from(other.bow == Carried::ForeignHeld)
            + usize::from(other.shield == Carried::ForeignHeld)
    }

    pub(super) fn gear_telekinetic_held_dagger(&self, i: usize, id: usize) -> bool {
        (4..6).contains(&id) && self.wielded_by(i, id % 2, false)
            && self.gear_weapon_kind_for(id % 2, false) == Some(WeaponKind::Dagger)
    }

    pub(super) fn gear_can_telekinetic_pickup(&self, i: usize, id: usize) -> bool {
        (4..6).contains(&id) && self.gear_free_hand(i) && self.object(id).is_some_and(|item| {
            item.kind == super::objects::ObjectKind::Weapon(WeaponKind::Dagger)
        })
    }

    pub(super) fn gear_telekinetic_dagger_to_hand(&mut self, i: usize, id: usize) -> bool {
        if !self.gear_can_telekinetic_pickup(i, id) { return false; }
        let issuer = id % 2;
        let place = if i == issuer { Carried::Held } else { Carried::ForeignHeld };
        self.set_weapon_location(issuer, false, place);
        self.gear_choose_for_attack(i, issuer, false);
        true
    }

    pub(super) fn gear_telekinetic_dagger_from_hand(&mut self, i: usize, id: usize, point: Point) {
        debug_assert!(self.gear_telekinetic_held_dagger(i, id));
        let issuer = id % 2;
        self.martial_sacred_weapon_dropped(i, issuer);
        let cell = self.map.landing(point).cell;
        self.set_weapon_location(issuer, false, Carried::Ground(cell));
    }

    fn weapon_location(&self, issuer: usize, ranged: bool) -> Carried {
        if ranged {
            self.f[issuer].gear.bow
        } else {
            self.f[issuer].gear.melee
        }
    }
    fn set_weapon_location(&mut self, issuer: usize, ranged: bool, loc: Carried) {
        let old = self.weapon_location(issuer, ranged);
        if !ranged
            && old != loc
            && self.gear_weapon_kind_for(issuer, false) == Some(WeaponKind::Quarterstaff)
        {
            if old == Carried::Held {
                self.magic_lose_weapon_contact(issuer);
            }
            if old == Carried::ForeignHeld {
                self.magic_lose_weapon_contact(1 - issuer);
            }
        }
        let g = &mut self.f[issuer].gear;
        if ranged {
            g.bow = loc;
        } else {
            g.melee = loc;
        }
    }
    fn wielded_by(&self, i: usize, issuer: usize, ranged: bool) -> bool {
        self.weapon_location(issuer, ranged)
            == if i == issuer {
                Carried::Held
            } else {
                Carried::ForeignHeld
            }
    }
    fn selected_issuer(&self, i: usize, ranged: bool) -> usize {
        if self.wielded_by(i, 1 - i, ranged)
            && (!self.wielded_by(i, i, ranged) || self.f[i].gear.preferred_foreign[ranged as usize])
        {
            1 - i
        } else {
            i
        }
    }
    pub fn gear_weapon_owner(&self, i: usize, ranged: bool) -> usize {
        self.selected_issuer(i, ranged)
    }
    pub fn gear_carries_weapon(&self, i: usize, issuer: usize, ranged: bool) -> bool {
        let loc = self.weapon_location(issuer, ranged);
        if i == issuer {
            matches!(loc, Carried::Held | Carried::Packed)
        } else {
            matches!(loc, Carried::ForeignHeld | Carried::ForeignPacked)
        }
    }
    pub fn gear_weapon_kind(&self, i: usize, ranged: bool) -> Option<WeaponKind> {
        if self.magic_is_wolf(i) {
            return None;
        }
        self.gear_weapon_kind_for(self.selected_issuer(i, ranged), ranged)
    }
    pub(super) fn gear_weapon_kind_for(&self, issuer: usize, ranged: bool) -> Option<WeaponKind> {
        WeaponKind::issued(self.f[issuer].build.class, ranged)
    }
    pub(super) fn gear_wielded_by(&self, i: usize, issuer: usize, ranged: bool) -> bool {
        self.wielded_by(i, issuer, ranged)
    }
    pub(super) fn gear_choose_for_attack(&mut self, i: usize, issuer: usize, ranged: bool) {
        debug_assert!(self.wielded_by(i, issuer, ranged));
        self.f[i].gear.preferred_foreign[ranged as usize] = i != issuer;
    }
    pub(super) fn gear_drop_weapon_instance(
        &mut self,
        i: usize,
        issuer: usize,
        ranged: bool,
        cell: Pos,
    ) {
        if !ranged {
            self.martial_sacred_weapon_dropped(i, issuer);
        }
        self.set_weapon_location(issuer, ranged, Carried::Ground(cell));
    }
    pub fn gear_weapon_stats(&self, i: usize, ranged: bool) -> Option<(i32, i32)> {
        let kind = self.gear_weapon_kind(i, ranged)?;
        let stats = self.f[i].build.stats;
        let modifier = if kind.is_bow() {
            stats[1]
        } else if kind.finesse() {
            self.weapon_finesse_modifier(i)
        } else {
            stats[0]
        };
        Some((kind.die(self.gear_two_handed(i)), modifier))
    }
    pub fn gear_mastery(&self, i: usize, ranged: bool) -> Option<MasteryWeapon> {
        let mastery = self.gear_weapon_kind(i, ranged)?.mastery()?;
        let build = self.f[i].build;
        weapon_masteries(build.class, build.level)
            .contains(&mastery)
            .then_some(mastery)
    }
    pub fn gear_wielded(&self, i: usize, ranged: bool) -> bool {
        if !ranged && (self.f[i].build.class == Class::Monk || self.magic_is_wolf(i)) {
            return true;
        }
        let issuer = self.selected_issuer(i, ranged);
        self.wielded_by(i, issuer, ranged) && self.gear_allows_weapon(i, ranged)
    }
    pub fn gear_allows_weapon(&self, i: usize, ranged: bool) -> bool {
        let f = &self.f[i];
        if self.magic_is_wolf(i) {
            return !ranged;
        }
        if !ranged && f.build.class == Class::Monk && !self.wielded_by(i, 1 - i, false) {
            return true;
        }
        let Some(kind) = self.gear_weapon_kind(i, ranged) else {
            return false;
        };
        let issuer = self.selected_issuer(i, ranged);
        let held = self.wielded_by(i, issuer, ranged);
        if !held
            && !self.weapon_location(issuer, ranged).available(
                self.fighter_point(i),
                self.map.landing(self.fighter_point(i)),
            )
        {
            return false;
        }
        self.occupied_for_strike(i) + usize::from(!held) <= if kind.needs_two() { 1 } else { 2 }
    }
    pub fn gear_strike_equip(&mut self, i: usize, ranged: bool) {
        if self.magic_is_wolf(i) {
            return;
        }
        let issuer = self.selected_issuer(i, ranged);
        let draw = self.weapon_location(issuer, ranged).available(
            self.fighter_point(i),
            self.map.landing(self.fighter_point(i)),
        );
        self.f[i].gear.after_attack = !draw;
        if draw {
            self.set_weapon_location(issuer, ranged, Carried::Held);
        }
    }

    pub fn gear_turn_start(&mut self, i: usize) {
        self.f[i].gear.interaction = true;
        self.f[i].gear.after_attack = false;
        self.weapon_options_turn_start(i);
    }

    pub(super) fn shield_holder(&self, issuer: usize) -> Option<usize> {
        match self.f[issuer].gear.shield {
            Carried::Held | Carried::Packed => Some(issuer),
            Carried::ForeignHeld | Carried::ForeignPacked => Some(1 - issuer),
            _ => None,
        }
    }

    fn shield_in_hand(&self, issuer: usize) -> bool {
        matches!(
            self.f[issuer].gear.shield,
            Carried::Held | Carried::ForeignHeld
        )
    }

    fn worn_shield(&self, i: usize) -> Option<usize> {
        (0..2).find(|&issuer| {
            self.shield_holder(issuer) == Some(i)
                && self.shield_in_hand(issuer)
                && !self.f[issuer].gear.shield_loose
        })
    }

    pub fn gear_shield_worn(&self, i: usize) -> bool {
        !self.magic_is_wolf(i) && self.worn_shield(i).is_some()
    }

    pub fn gear_carries_shield(&self, i: usize) -> bool {
        (0..2).any(|issuer| self.shield_holder(issuer) == Some(i))
    }

    pub fn gear_holds_foreign_shield(&self, i: usize) -> bool {
        self.shield_holder(1 - i) == Some(i)
    }

    pub fn gear_loose_shields(&self, i: usize) -> u8 {
        (0..2)
            .filter(|&issuer| {
                self.shield_holder(issuer) == Some(i)
                    && self.shield_in_hand(issuer)
                    && self.f[issuer].gear.shield_loose
            })
            .fold(0, |mask, issuer| mask | 1 << issuer)
    }

    pub fn shield_ground(&self, issuer: usize) -> Option<Point> {
        if let Carried::Ground(cell) = self.f[issuer].gear.shield {
            Some(Point {
                cell,
                altitude: self.f[issuer].gear.shield_ground_height,
            })
        } else {
            None
        }
    }

    pub fn shield_present(&self, issuer: usize) -> bool {
        self.f[issuer].gear.shield != Carried::Absent
    }

    pub fn shield_destroy(&mut self, issuer: usize) {
        self.f[issuer].gear.shield = Carried::Absent;
        self.f[issuer].gear.shield_loose = false;
    }

    pub fn shield_move_ground(&mut self, issuer: usize, point: Point) {
        if self.shield_ground(issuer).is_some() {
            self.shield_on_ground(issuer, point);
        }
    }

    pub(super) fn shield_on_ground(&mut self, issuer: usize, point: Point) {
        let gear = &mut self.f[issuer].gear;
        gear.shield = Carried::Ground(point.cell);
        gear.shield_ground_height = point.altitude;
        gear.shield_loose = false;
    }

    pub(super) fn gear_drop_shields_for_shape(&mut self, i: usize) {
        let point = self.map.landing(self.fighter_point(i));
        for issuer in 0..2 {
            if self.shield_holder(issuer) == Some(i) {
                self.shield_on_ground(issuer, point);
            }
        }
    }

    fn set_shield_carried(&mut self, i: usize, issuer: usize, in_hand: bool, loose: bool) {
        let gear = &mut self.f[issuer].gear;
        gear.shield = match (issuer == i, in_hand) {
            (true, true) => Carried::Held,
            (true, false) => Carried::Packed,
            (false, true) => Carried::ForeignHeld,
            (false, false) => Carried::ForeignPacked,
        };
        gear.shield_loose = loose;
        self.f[i].gear.versatile_two = false;
    }

    fn can_don_shield(&self, i: usize, issuer: usize) -> bool {
        if self.worn_shield(i).is_some() {
            return false;
        }
        let already_holding = self.shield_holder(issuer) == Some(i);
        if already_holding && self.shield_in_hand(issuer) {
            return self.f[issuer].gear.shield_loose;
        }
        self.gear_free_hand(i)
            && (already_holding || self.shield_ground(issuer) == Some(self.fighter_point(i)))
    }

    fn shield_don_candidate(&self, i: usize) -> Option<usize> {
        [i, 1 - i]
            .into_iter()
            .find(|&issuer| self.can_don_shield(i, issuer))
    }

    pub(super) fn gear_ready_mask(&self, i: usize) -> u128 {
        if self.magic_is_wolf(i) {
            return 0;
        }
        if self.worn_shield(i).is_some() {
            Act::Gear7.bit()
        } else if self.shield_don_candidate(i).is_some() {
            Act::Gear8.bit()
        } else {
            0
        }
    }

    pub(super) fn gear_ready_release(&mut self, i: usize, action: Act) {
        let issuer = if action == Act::Gear7 {
            self.worn_shield(i)
        } else {
            self.shield_don_candidate(i)
        };
        if let Some(issuer) = issuer {
            self.set_shield_carried(i, issuer, action == Act::Gear8, false);
        }
    }

    pub fn gear_ac_adjust(&self, i: usize) -> i32 {
        use Class::*;
        let build = self.f[i].build;
        let class = build.class;
        let initial = matches!(class, Cleric | Druid | Fighter | Paladin);
        let trained = matches!(
            class,
            Barbarian | Cleric | Druid | Fighter | Paladin | Ranger
        ) || class == Bard && build.variant == 1;
        2 * (i32::from(trained && self.gear_shield_worn(i)) - i32::from(initial))
    }

    pub fn gear_two_handed(&self, i: usize) -> bool {
        let g = &self.f[i].gear;
        self.gear_weapon_kind(i, false)
            .is_some_and(|w| w.versatile())
            && g.versatile_two
            && self.occupied_for_strike(i) == 1
    }

    pub fn gear_mask(&self, i: usize) -> u128 {
        use Act::*;
        use Carried::*;
        let f = &self.f[i];
        let g = &f.gear;
        let mut mask = 0;
        if self.magic_is_wolf(i) {
            return 0;
        }
        let hand = self.gear_free_hand(i);
        if g.interaction || g.after_attack || self.action {
            for (slot, draw, stow, drop) in
                [(g.melee, Gear1, Gear2, Gear3), (g.bow, Gear4, Gear5, Gear6)]
            {
                if g.star_map && draw == Gear4 && !g.interaction && !self.action {
                    continue;
                }
                if slot.available(
                    self.fighter_point(i),
                    self.map.landing(self.fighter_point(i)),
                ) && hand
                {
                    mask |= draw.bit();
                }
                if slot == Held {
                    mask |= stow.bit() | drop.bit();
                }
            }
        }
        if self.action && self.worn_shield(i).is_some() {
            mask |= Gear7.bit();
        }
        if self.action && self.shield_don_candidate(i).is_some() {
            mask |= Gear8.bit();
        }
        if self
            .gear_weapon_kind(i, false)
            .is_some_and(|w| w.versatile())
            && self.wielded_by(i, self.selected_issuer(i, false), false)
            && hand
        {
            mask |= if g.versatile_two { Gear10 } else { Gear9 }.bit();
        }
        if (self.action || self.attacks > 0)
            && self.gear_weapon_kind(i, false) == Some(WeaponKind::Dagger)
            && self.gear_allows_weapon(i, false)
            && self.gear_decision_mask(i, 66) != 0
        {
            mask |= Gear11.bit();
        }
        if self.gear_decision_mask(i, 71) != 0 {
            mask |= Gear12.bit();
        }
        mask | self.weapon_options_mask(i)
    }

    pub fn gear_label(&self, i: usize, a: Act) -> String {
        use Act::*;
        let g = &self.f[i].gear;
        let map_action = g.star_map && matches!(a, Gear4 | Gear5 | Gear6);
        let cost = if g.after_attack && !map_action {
            "attack interaction"
        } else if g.interaction {
            "free interaction"
        } else {
            "Utilize action"
        };
        if let Some(label) = Self::weapon_options_label(a) {
            return label.into();
        }
        match a {
            Gear1 | Gear2 | Gear3 | Gear4 | Gear5 | Gear6 => {
                let slot = if (a as u8) <= Gear3 as u8 {
                    "melee weapon"
                } else if map_action {
                    "Star Map"
                } else {
                    "bow"
                };
                let verb = match a {
                    Gear1 | Gear4 => "Draw or pick up",
                    Gear2 | Gear5 => "Stow",
                    _ => "Drop",
                };
                format!("{verb} {slot} ({cost})")
            }
            Gear7 => "Doff shield (Utilize action)".into(),
            Gear8 => "Don shield (Utilize action)".into(),
            Gear11 => "Throw dagger (Attack)".into(),
            Gear12 if self.f[1 - i].gear.star_map && self.foreign_mask(i) & 0xF0 != 0 => {
                "Handle another duelist's equipment".into()
            }
            Gear12 => "Handle a borrowed weapon or shield".into(),
            Gear9 => "Use versatile weapon two-handed".into(),
            Gear10 => "Use versatile weapon one-handed".into(),
            _ => String::new(),
        }
    }

    pub fn gear_step(&mut self, i: usize, a: Act) {
        use Act::*;
        if Self::weapon_options_label(a).is_some() {
            self.weapon_options_step(i, a);
            return;
        }
        if a == Gear11 || a == Gear12 {
            self.request(i, if a == Gear11 { 66 } else { 71 });
            return;
        }
        if matches!(a, Gear2 | Gear3)
            && self.gear_weapon_kind_for(i, false) == Some(WeaponKind::Quarterstaff)
        {
            self.magic_lose_weapon_contact(i);
        }
        if matches!(a, Gear7 | Gear8) {
            let issuer = if a == Gear7 {
                self.worn_shield(i)
            } else {
                self.shield_don_candidate(i)
            }
            .expect("validated shield choice");
            self.set_shield_carried(i, issuer, a == Gear8, false);
            self.action = false;
            return;
        }
        if a == Gear3 {
            self.martial_sacred_weapon_dropped(i, i);
        }
        let f = &mut self.f[i];
        let g = &mut f.gear;
        match a {
            Gear1 | Gear2 | Gear3 | Gear4 | Gear5 | Gear6 => {
                let map_action = g.star_map && matches!(a, Gear4 | Gear5 | Gear6);
                if g.after_attack && !map_action {
                    g.after_attack = false;
                } else if g.interaction {
                    g.interaction = false;
                } else {
                    self.action = false;
                }
                let item = if a as u8 <= Gear3 as u8 {
                    &mut g.melee
                } else {
                    &mut g.bow
                };
                *item = match a {
                    Gear1 | Gear4 => Carried::Held,
                    Gear2 | Gear5 => Carried::Packed,
                    _ => Carried::Ground(f.pos),
                };
                g.versatile_two = false;
            }
            Gear9 | Gear10 => g.versatile_two = a == Gear9,
            _ => unreachable!("equipment mask validated"),
        }
    }

    pub fn can_cast_components(
        &self,
        i: usize,
        _verbal: bool,
        somatic: bool,
        material: Material,
    ) -> bool {
        if self.armor_untrained(i) {
            return false;
        }
        let f = &self.f[i];
        let g = &f.gear;
        let free = self.gear_free_hand(i);
        let emblem = matches!(f.build.class,Class::Paladin|Class::Cleric)
            && (0..2).any(|issuer|matches!(self.f[issuer].build.class,Class::Paladin|Class::Cleric)
                && self.shield_holder(issuer)==Some(i) && self.shield_in_hand(issuer));
        let staff = f.build.class == Class::Druid && g.melee == Carried::Held;
        let valor_focus = f.build.class == Class::Bard
            && f.build.variant == 1
            && (0..2).any(|issuer| {
                [false, true].into_iter().any(|ranged| {
                    self.wielded_by(i, issuer, ranged)
                        && self.gear_weapon_kind_for(issuer, ranged).is_some()
                })
            });
        match material {
            Material::None => !somatic || free,
            Material::Ordinary => free || emblem || staff || valor_focus || self.star_map_held(i),
            Material::Diamond50 => free && (g.diamond50 || g.diamonds300 > 0),
            Material::Diamond300 => free && g.diamonds300 > 0,
            Material::DiamondDust25 => free && g.diamond_dust25 > 0,
            Material::Incense10 => free && g.incense10 > 0,
            Material::GildedAcorn200 => false,
            Material::HolyWater25 => free && g.holy_water25 > 0,
            Material::HolySymbol5 => emblem,
            Material::Weapon1cp => (0..2).any(|issuer|[false,true].into_iter().any(|ranged|
                self.wielded_by(i,issuer,ranged) && self.gear_weapon_kind_for(issuer,ranged).is_some_and(|w|w.proficient(f.build)))) ,
        }
    }

    pub fn drop_held(&mut self, i: usize) {
        let loose = self.gear_loose_shields(i);
        let point = self.map.landing(self.fighter_point(i));
        for issuer in 0..2 {
            if loose & (1 << issuer) != 0 {
                self.shield_on_ground(issuer, point);
            }
        }
        self.magic_lose_weapon_contact(i);
        for issuer in [i, 1 - i] {
            if self.wielded_by(i, issuer, false) {
                self.martial_sacred_weapon_dropped(i, issuer);
            }
        }
        let pos = self.f[i].pos;
        for ranged in [false, true] {
            if self.wielded_by(i, 1 - i, ranged) {
                self.set_weapon_location(1 - i, ranged, Carried::Ground(pos));
            }
        }
        let f = &mut self.f[i];
        for item in [&mut f.gear.melee, &mut f.gear.bow] {
            if *item == Carried::Held {
                *item = Carried::Ground(f.pos);
            }
        }
        f.gear.versatile_two = false;
    }

    pub fn consume_material(&mut self, i: usize, material: Material) {
        match material {
            Material::Diamond300 => self.f[i].gear.diamonds300 -= 1,
            Material::DiamondDust25 => self.f[i].gear.diamond_dust25 -= 1,
            Material::Incense10 => self.f[i].gear.incense10 -= 1,
            Material::HolyWater25 => self.f[i].gear.holy_water25 -= 1,
            _ => {}
        }
    }
}

impl Battle {
    fn dagger_target(&self, i: usize, target: usize) -> Option<Point> {
        if target >= 4 {
            return self.observed_object(i, target).map(|o| o.point);
        }
        self.observed_footprint(i, target)?
            .squares()
            .find(|&p| self.dagger_reaches(i, p))
    }

    fn dagger_reaches(&self, i: usize, p: Point) -> bool {
        let source = self.fighter_point(i);
        self.map.can_occupy(p) && source.dist(p) <= 12 && self.map.los_at(source, p)
    }

    fn dagger_altitudes(&self, i: usize, cell: Pos) -> u128 {
        (0..=MAX_ELEVATION)
            .filter(|&altitude| self.dagger_reaches(i, Point { cell, altitude }))
            .fold(0, |mask, h| mask | (1 << h))
    }

    fn shield_option_mask(&self, i: usize) -> u128 {
        let interact = self.f[i].gear.interaction || self.action;
        (0..2).fold(0, |mask, issuer| {
            let held = self.shield_holder(issuer) == Some(i);
            let worn = self.worn_shield(i) == Some(issuer);
            let mut choices = 0;
            if held && !worn && interact {
                choices |= 1 << (8 + issuer);
            }
            if interact
                && self.gear_free_hand(i)
                && self.shield_ground(issuer) == Some(self.fighter_point(i))
            {
                choices |= 1 << (10 + issuer);
            }
            if self.action && worn {
                choices |= 1 << (12 + issuer);
            }
            if self.action && self.can_don_shield(i, issuer) {
                choices |= 1 << (14 + issuer);
            }
            mask | choices
        })
    }

    fn step_shield_option(&mut self, i: usize, id: usize) {
        let issuer = id % 2;
        match id {
            8..=9 => self.shield_on_ground(issuer, self.map.landing(self.fighter_point(i))),
            10..=11 => self.set_shield_carried(i, issuer, false, false),
            12..=13 => self.set_shield_carried(i, issuer, true, true),
            14..=15 => self.set_shield_carried(i, issuer, true, false),
            _ => unreachable!("masked shield option"),
        }
        if id < 12 && self.f[i].gear.interaction {
            self.f[i].gear.interaction = false;
        } else {
            self.action = false;
        }
    }

    fn foreign_mask(&self, i: usize) -> u128 {
        let g = &self.f[i].gear;
        let can_interact = g.interaction || g.after_attack || self.action;
        let mut mask = 0;
        for ranged in [false, true] {
            let peer = 1 - i;
            let map_slot = ranged && self.f[peer].gear.star_map;
            let can_interact = can_interact && (!map_slot || g.interaction || self.action);
            let base = if ranged { 4 } else { 0 };
            match self.weapon_location(peer, ranged) {
                Carried::Ground(p)
                    if p == self.f[i].pos
                        && self.map.supported(self.fighter_point(i))
                        && can_interact
                        && self.gear_free_hand(i) =>
                {
                    mask |= 1 << base
                }
                Carried::ForeignPacked if can_interact && self.gear_free_hand(i) => {
                    mask |= 1 << base
                }
                Carried::ForeignHeld => {
                    if can_interact {
                        mask |= 3 << (base + 1);
                    }
                    if !map_slot && self.wielded_by(i, i, ranged) {
                        mask |= 1 << (base + 3);
                    }
                }
                _ => {}
            }
        }
        mask
    }

    pub(super) fn gear_decision_mask(&self, i: usize, phase: i32) -> u128 {
        match phase {
            66 => {
                let mut mask = 0;
                for t in 0..crate::arena2::objects::END_OBJECTS {
                    if self
                        .dagger_target(i, t)
                        .is_some_and(|p| self.dagger_reaches(i, p))
                    {
                        mask |= 1 << t;
                    }
                }
                if self.observed_pos(i, 1 - i).is_none() || self.observed_entity(i, 3 - i).is_none()
                {
                    mask |= 1 << 126;
                }
                mask
            }
            67 => (0..HEIGHT)
                .filter(|&y| (0..WIDTH).any(|x| self.dagger_altitudes(i, Pos { x, y }) != 0))
                .fold(0, |mask, y| mask | (1 << y)),
            68 => (0..WIDTH)
                .filter(|&x| {
                    self.dagger_altitudes(
                        i,
                        Pos {
                            x,
                            y: self.f[i].gear.throw_cell.y,
                        },
                    ) != 0
                })
                .fold(0, |mask, x| mask | (1 << x)),
            69 => self.dagger_altitudes(i, self.f[i].gear.throw_cell),
            71 => self.foreign_mask(i) | self.shield_option_mask(i),
            72 => self.weapon_option_choice_mask(i),
            _ => 0,
        }
    }

    pub(super) fn gear_decision_label(&self, i: usize, phase: i32, a: Act) -> Option<String> {
        let id = a as usize;
        if phase == 72 {
            return self.weapon_option_choice_label(i, a);
        }
        Some(match phase {
            66 if id < 2 => {
                if id == i {
                    "Aim at yourself".into()
                } else {
                    "Aim at opponent".into()
                }
            }
            66 if id < 4 => format!("Aim at {}", self.entity(id)?.kind.name()),
            66 if id < crate::arena2::objects::END_OBJECTS => {
                format!("Aim at {}", self.object(id)?.label())
            }
            66 => "Guess the target's location".into(),
            67 => format!("Row {}", id + 1),
            68 => format!(
                "Cell {}{}",
                char::from(b'A' + id as u8),
                self.f[i].gear.throw_cell.y + 1
            ),
            69 => format!("{} ft above the floor", id * 5),
            71 if id >= 8 => {
                let who = if i == id % 2 { "issued" } else { "borrowed" };
                match id {
                    8..=9 => format!("Drop {who} shield (interaction or Utilize)"),
                    10..=11 => format!("Pick up {who} shield (interaction or Utilize)"),
                    12..=13 => format!("Doff {who} shield into hand (Utilize)"),
                    14..=15 => format!("Don {who} shield (Utilize)"),
                    _ => return None,
                }
            }
            71 => {
                let ranged = id >= 4;
                let name = self.gear_item_name_for(1 - i, ranged)?;
                let verb = match id % 4 {
                    0 => "Pick up or draw",
                    1 => "Stow",
                    2 => "Drop",
                    _ if self.selected_issuer(i, ranged) == i => "Prefer this weapon for attacks",
                    _ => "Prefer your issued weapon for attacks",
                };
                format!("{verb} ({name})")
            }
            _ => return None,
        })
    }

    pub(super) fn gear_decision_step(&mut self, i: usize, phase: i32, a: Act) {
        let id = a as usize;
        match phase {
            72 => self.weapon_option_choice_step(i, a),
            66 if id == 126 => self.request(i, 67),
            66 => {
                let p = self.dagger_target(i, id).expect("masked dagger target");
                self.throw_at(i, p, Some(id));
            }
            67 | 68 => {
                if phase == 67 {
                    self.f[i].gear.throw_cell.y = id as i8;
                } else {
                    self.f[i].gear.throw_cell.x = id as i8;
                }
                self.request(i, phase + 1);
            }
            69 => {
                let p = Point {
                    cell: self.f[i].gear.throw_cell,
                    altitude: id as i8,
                };
                let target = (0..4).find(|&id| {
                    self.footprint(id)
                        .is_some_and(|f| f.squares().any(|square| square == p))
                });
                self.throw_at(i, p, target);
            }
            71 if id >= 8 => self.step_shield_option(i, id),
            71 => {
                let ranged = id >= 4;
                let op = id % 4;
                if op == 3 {
                    self.f[i].gear.preferred_foreign[ranged as usize] =
                        self.selected_issuer(i, ranged) == i;
                    return;
                }
                if op == 2 && !ranged {
                    self.martial_sacred_weapon_dropped(i, 1 - i);
                }
                let loc = match op {
                    0 => Carried::ForeignHeld,
                    1 => Carried::ForeignPacked,
                    _ => Carried::Ground(self.f[i].pos),
                };
                self.set_weapon_location(1 - i, ranged, loc);
                let map_slot = ranged && self.f[1 - i].gear.star_map;
                let g = &mut self.f[i].gear;
                if !map_slot {
                    g.preferred_foreign[ranged as usize] = op == 0;
                }
                if g.after_attack && !map_slot {
                    g.after_attack = false;
                } else if g.interaction {
                    g.interaction = false;
                } else {
                    self.action = false;
                }
            }
            _ => unreachable!("validated equipment phase"),
        }
    }

    fn throw_at(&mut self, i: usize, p: Point, target: Option<usize>) {
        let issuer = self.selected_issuer(i, false);
        let ready = self.wielded_by(i, issuer, false);
        self.f[i].gear.after_attack = ready;
        self.launch_dagger(i, target, self.fighter_point(i).dist(p) > 4);
        if !self.sanctuary_defer_throw(issuer, false) {
            self.set_weapon_location(issuer, false, Carried::Ground(p.cell));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ACTS;
    use super::*;

    fn arena(classes: [Class; 2]) -> Battle {
        let mut b = Battle::new(1, classes, [5; 2]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 1, y: 0 };
        b
    }

    #[test]
    fn shield_emblem_and_staff_share_material_and_somatic_hands() {
        let mut b = arena([Class::Cleric, Class::Druid]);
        for i in 0..2 {
            assert!(!b.can_cast_components(i, true, true, Material::None));
            assert!(b.can_cast_components(i, true, true, Material::Ordinary));
        }
        assert!(b.can_cast_components(0, true, true, Material::HolySymbol5));
        assert!(!b.can_cast_components(1, true, true, Material::HolySymbol5));
        b.step(Act::Gear2).unwrap();
        assert!(b.f[0].gear.interaction == false && b.action);
        assert!(b.gear_free_hand(0));
        assert!(b.can_cast_components(0, true, true, Material::None));
        assert!(!b.gear_wielded(0, false));
        assert!(b.gear_allows_weapon(0, false));
        b.step(Act::Gear1).unwrap();
        assert!(!b.action);
    }

    #[test]
    fn shield_changes_ac_and_versatile_damage_through_turn_choices() {
        let mut b = arena([Class::Fighter, Class::Druid]);
        let old = b.ac(0);
        assert_eq!(b.gear_weapon_stats(0, false).unwrap().0, 8);
        assert_eq!(b.mask() & Act::Gear9.bit(), 0);
        b.step(Act::Gear7).unwrap();
        assert!(!b.action);
        assert_eq!(b.ac(0), old - 2);
        b.step(Act::Gear9).unwrap();
        assert_eq!(b.gear_weapon_stats(0, false).unwrap().0, 10);
        b.owner = 1;
        b.action = true;
        b.step(Act::Gear7).unwrap();
        b.step(Act::Gear9).unwrap();
        assert_eq!(b.gear_weapon_stats(1, false).unwrap().0, 8);
    }

    #[test]
    fn looting_and_stowing_use_actual_weapon_and_trained_mastery() {
        let mut b = arena([Class::Rogue, Class::Barbarian]);
        b.step(Act::Gear3).unwrap();
        let p = b.f[0].pos;
        b.f[0].pos = Pos { x: 0, y: 1 };
        b.f[1].pos = p;
        b.owner = 1;
        b.step(Act::Gear12).unwrap();
        assert_eq!(b.phase_id(), 71);
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.gear_weapon_kind(1, false), Some(WeaponKind::Rapier));
        assert_eq!(b.gear_mastery(1, false), Some(MasteryWeapon::Rapier));
        assert_eq!(
            b.gear_weapon_stats(1, false),
            Some((8, b.f[1].build.stats[0]))
        );
        assert_eq!(b.f[0].gear.melee, Carried::ForeignHeld);
        b.step(Act::Gear12).unwrap();
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.f[0].gear.melee, Carried::ForeignPacked);
        assert_eq!(b.gear_weapon_kind(1, false), Some(WeaponKind::Greataxe));
    }

    #[test]
    fn thrown_dagger_lands_at_target_and_never_consumes_an_arrow() {
        let mut b = arena([Class::Bard, Class::Fighter]);
        let arrows = b.f[0].arrows;
        let destination = b.f[1].pos;
        b.step(Act::Gear11).unwrap();
        assert_eq!(b.phase_id(), 66);
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.f[0].arrows, arrows);
        assert_eq!(b.f[0].gear.melee, Carried::Ground(destination));
        assert!(!b.action);
        assert!(!b.gear_allows_weapon(0, false));
    }

    #[test]
    fn thrown_dagger_accepts_visible_objects_and_empty_height_guesses() {
        let mut b = arena([Class::Bard, Class::Fighter]);
        b.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 0 });
        b.step(Act::Gear11).unwrap();
        assert_ne!(b.mask() & (1 << 5), 0);
        let mut guess = b.clone();
        b.step(ACTS[5]).unwrap();
        assert_eq!(b.f[0].gear.melee, Carried::Ground(Pos { x: 1, y: 0 }));
        guess.step(ACTS[126]).unwrap();
        for choice in [0, 1, 3] {
            guess.step(ACTS[choice]).unwrap();
        }
        assert_eq!(guess.f[0].gear.melee, Carried::Ground(Pos { x: 1, y: 0 }));
        assert!(!guess.action);
        guess.f[0].pos = Pos { x: 1, y: 0 };
        guess.f[0].elevation = 3;
        assert_eq!(guess.gear_mask(0) & Act::Gear1.bit(), 0);
        guess.f[0].elevation = 0;
        assert_ne!(guess.gear_mask(0) & Act::Gear1.bit(), 0);
    }

    #[test]
    fn expensive_components_use_matching_payment_and_consumption() {
        let mut b = arena([Class::Sorcerer, Class::Paladin]);
        assert!(b.can_cast_components(0, true, true, Material::Diamond50));
        b.consume_material(0, Material::Diamond50);
        assert!(b.f[0].gear.diamond50);
        assert!(!b.can_cast_components(0, true, true, Material::Diamond300));
        assert!(!b.can_cast_components(1, true, true, Material::HolyWater25));
        b.f[1].gear.melee = Carried::Packed;
        assert!(b.can_cast_components(1, true, true, Material::HolyWater25));
        b.consume_material(1, Material::HolyWater25);
        assert!(!b.can_cast_components(1, true, true, Material::HolyWater25));
    }

    #[test]
    fn bow_requires_two_hands_and_drawing_during_attack_costs_no_free_interaction() {
        let mut b = arena([Class::Ranger, Class::Fighter]);
        assert!(!b.gear_allows_weapon(0, true));
        b.step(Act::Gear2).unwrap();
        assert!(b.gear_allows_weapon(0, true));
        assert!(b.mask() & Act::Ranged.bit() != 0);
        b.step(Act::Ranged).unwrap();
        assert_eq!(b.f[0].gear.bow, Carried::Held);
        assert!(!b.f[0].gear.after_attack);
        assert_eq!(b.f[0].arrows, 19);
    }

    #[test]
    fn stars_requires_held_star_map_for_guidance_and_guiding_bolt() {
        let mut battle = (0..100)
            .map(|seed| Battle::new_roster_in(seed, [10, 12], 0))
            .find(|battle| battle.owner == 0)
            .unwrap();
        assert_eq!(battle.f[0].build.subclass(), "Stars");
        assert!(battle.star_map_held(0));
        assert_eq!(battle.ac(0), 13);
        assert!(battle.gear_free_hand(0));
        assert!(battle.divine_free_bolt_ready(0));
        assert_eq!(battle.prepared_spell(0, Act::Magic9).unwrap().label(), "Guidance");
        assert_eq!(battle.prepared_spell(0, Act::Magic10).unwrap().label(), "Guiding Bolt");
        assert!(!battle.gear_allows_weapon(0, true));
        battle.step(Act::Gear6).unwrap();
        let chart = battle.object(6).expect("dropped Star Map");
        assert_eq!(chart.kind, crate::arena2::objects::ObjectKind::StarMap);
        assert_eq!((chart.ac, chart.hp), (17, 5));
        assert!(!battle.star_map_held(0));
        assert!(!battle.divine_free_bolt_ready(0));
        assert!(battle.prepared_spell(0, Act::Magic9).is_none());
        assert!(battle.prepared_spell(0, Act::Magic10).is_none());
        battle.step(Act::Gear4).unwrap();
        assert!(battle.star_map_held(0));
        assert!(battle.divine_free_bolt_ready(0));
    }

    #[test]
    fn held_star_map_is_a_focus_but_somatic_only_spells_need_a_free_hand() {
        let mut battle = (0..100)
            .map(|seed| Battle::new_roster_in(seed, [10, 12], 0))
            .find(|battle| battle.owner == 0)
            .unwrap();
        battle.step(Act::Gear8).unwrap();
        assert!(battle.star_map_held(0));
        assert_eq!(battle.ac(0), 15);
        assert!(!battle.gear_free_hand(0));
        assert!(battle.can_cast_components(0, true, true, Material::Ordinary));
        assert!(!battle.can_cast_components(0, true, true, Material::None));
        assert!(!battle.divine_free_bolt_ready(0));
    }
}
