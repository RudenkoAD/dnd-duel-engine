//! Attackable unattended gear and hazards. Structural stone is permanent.
use super::battle::Battle;
use super::equipment::{Carried, DamageType, WeaponKind};
use super::geometry::{Cover, Point};
use super::targeting::Area;

pub const FIRST_OBJECT: usize = 4;
pub const END_OBJECTS: usize = 14;

/// IDs 4–5 identify each build's melee weapon, 6–7 its bow, and 8–9
/// a deployed caltrops cluster. IDs 10–11 identify body armor and 12–13
/// identify shields. The ID follows the object if ownership changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Weapon(WeaponKind),
    Caltrops,
    Armor { leather: bool },
    Shield,
    StarMap,
}

impl ObjectKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Weapon(weapon) => weapon.name(),
            Self::Caltrops => "Deployed caltrops",
            Self::Armor { leather: true } => "Leather armor",
            Self::Armor { leather: false } => "Metal armor",
            Self::Shield => "Wooden shield",
            Self::StarMap => "Star Map (stone tablet)",
        }
    }

    pub fn flammable(self) -> bool {
        matches!(
            self,
            Self::Weapon(WeaponKind::Shortbow | WeaponKind::Longbow | WeaponKind::Quarterstaff)
                | Self::Armor { leather: true }
                | Self::Shield
        )
    }

    pub(super) fn area_extent(self) -> [i32; 3] {
        match self {
            Self::Caltrops => [2, 2, 0], // one five-foot square of scattered points
            Self::Armor { .. } => [2, 2, 2],
            Self::Weapon(_) | Self::Shield | Self::StarMap => [1, 1, 1], // centered half-square volume
        }
    }

    fn ac(self) -> i32 {
        match self {
            Self::Armor { leather: true } => 11,
            Self::StarMap => 17,
            _ if self.flammable() => 15,
            _ => 19,
        }
    }

    fn max_hp(self) -> i32 {
        match self {
            Self::Weapon(WeaponKind::Dagger) | Self::Caltrops | Self::StarMap => 5,
            Self::Armor { .. } => 18,
            Self::Weapon(_) | Self::Shield => 10,
        }
    }

    fn damage_received(self, amount: i32, kind: DamageType) -> i32 {
        let amount = amount.max(0);
        match kind {
            DamageType::Poison | DamageType::Psychic => 0,
            DamageType::Fire if self.flammable() => amount.saturating_mul(2),
            DamageType::Piercing if !self.flammable() => amount / 2,
            _ => amount,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectTarget {
    pub id: usize,
    pub point: Point,
    pub kind: ObjectKind,
    pub ac: i32,
    pub hp: i32,
    pub burning: bool,
    pub display_name: &'static str,
}

impl ObjectTarget {
    pub fn can_ignite(self) -> bool {
        self.kind.flammable() && !self.burning
    }

    pub fn label(self) -> &'static str {
        self.display_name
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Wear {
    lost_hp: i32,
    burning: bool,
    scorched: bool,
    fracture: Option<(Point, i32)>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ObjectState([Wear; END_OBJECTS - FIRST_OBJECT]);

impl Battle {
    fn object_kind(&self, id: usize) -> Option<ObjectKind> {
        let seat = id % 2;
        match id {
            4..=7 => {
                let bow = id >= 6;
                let gear = &self.f[seat].gear;
                let place = if bow { gear.bow } else { gear.melee };
                if bow && gear.star_map {
                    return (place != Carried::Absent).then_some(ObjectKind::StarMap);
                }
                (place != Carried::Absent)
                    .then(|| self.gear_weapon_kind_for(seat, bow))
                    .flatten()
                    .map(ObjectKind::Weapon)
            }
            8..=9 => self.caltrops(seat).map(|_| ObjectKind::Caltrops),
            10..=11 => self.armor_present(seat).then_some(ObjectKind::Armor {
                leather: self.armor_ground_flammable(seat),
            }),
            12..=13 => self.shield_present(seat).then_some(ObjectKind::Shield),
            _ => None,
        }
    }

    pub fn object(&self, id: usize) -> Option<ObjectTarget> {
        let kind = self.object_kind(id)?;
        let seat = id % 2;
        let point = match id {
            4..=7 => {
                let gear = &self.f[seat].gear;
                let Carried::Ground(cell) = (if id >= 6 { gear.bow } else { gear.melee }) else {
                    return None;
                };
                self.map.landing(Point::ground(cell))
            }
            8..=9 => Point::ground(self.caltrops(seat)?),
            10..=11 => self.armor_ground(seat)?,
            12..=13 => self.shield_ground(seat)?,
            _ => return None,
        };
        let wear = self.objects.0[id - FIRST_OBJECT];
        let hp = kind.max_hp() - wear.lost_hp;
        (hp > 0).then_some(ObjectTarget {
            id,
            point,
            kind,
            ac: kind.ac(),
            hp,
            burning: wear.burning,
            display_name: if (10..=11).contains(&id) {
                self.armor_ground_name(seat)
            } else {
                kind.name()
            },
        })
    }

    pub fn observed_object(&self, viewer: usize, id: usize) -> Option<ObjectTarget> {
        if self.rope_space(viewer).is_some() {
            return None;
        }
        let object = self.object(id)?;
        let observer = self.fighter_point(viewer);
        (self.map.los_at(observer, object.point)
            && !self.magic_cloud_blocks(observer, object.point))
        .then_some(object)
    }

    pub fn object_mask(&self, viewer: usize, range: i32) -> u128 {
        let observer = self.fighter_point(viewer);
        (FIRST_OBJECT..END_OBJECTS)
            .filter(|&id| {
                self.observed_object(viewer, id)
                    .is_some_and(|object| observer.dist(object.point) <= range)
            })
            .fold(0, |bits, id| bits | 1 << id)
    }

    /// Rolls only the hit check; the caller determines resource cost and rolls
    /// damage, including multi-die followups such as Chromatic Orb ricochets.
    pub fn roll_object_attack(
        &mut self,
        attacker: usize,
        origin: Point,
        id: usize,
        to_hit: i32,
        advantage: bool,
        disadvantage: bool,
        critical_min: i32,
    ) -> Option<bool> {
        let (d20, total, ac) =
            self.object_attack_roll(attacker, origin, id, to_hit, advantage, disadvantage)?;
        let critical = d20 >= critical_min;
        (d20 != 1 && (critical || total >= ac)).then_some(critical)
    }

    pub(super) fn object_attack_roll(
        &mut self,
        attacker: usize,
        origin: Point,
        id: usize,
        to_hit: i32,
        advantage: bool,
        disadvantage: bool,
    ) -> Option<(i32, i32, i32)> {
        let object = self.object(id)?;
        let cover = self.map.cover_with_creatures(
            origin,
            object.point,
            (0..4).filter_map(|seat| self.creature_point(seat)),
        );
        if cover == Cover::Total {
            return None;
        }
        let outlined = self.faerie_outlined(id) && self.map.los_at(origin, object.point) && !self.magic_cloud_blocks(origin, object.point);
        let d20 = self.d20_for(attacker, advantage || outlined, disadvantage);
        Some((d20, d20 + to_hit, object.ac + cover.bonus()))
    }

    pub fn damage_object(&mut self, id: usize, amount: i32, damage_type: DamageType) -> i32 {
        if let Some(object) = self.object(id) {
            self.apply_object_damage(id, object.kind, amount, damage_type)
        } else {
            0
        }
    }

    fn apply_object_damage(
        &mut self,
        id: usize,
        kind: ObjectKind,
        amount: i32,
        damage_type: DamageType,
    ) -> i32 {
        let fracture_point = if (kind == ObjectKind::StarMap
            || matches!(kind, ObjectKind::Weapon(_)) && kind.flammable())
            && matches!(damage_type, DamageType::Slashing | DamageType::Bludgeoning)
        {
            self.object(id).map(|object| object.point)
        } else {
            None
        };
        let wear = &mut self.objects.0[id - FIRST_OBJECT];
        let damage = kind
            .damage_received(amount, damage_type)
            .min((kind.max_hp() - wear.lost_hp).max(0));
        let previous_wear = wear.lost_hp;
        wear.lost_hp += damage;
        wear.scorched |= damage_type == DamageType::Fire && damage > 0;
        if wear.lost_hp == kind.max_hp() {
            if !wear.burning && !wear.scorched {
                wear.fracture = fracture_point.map(|point| (point, previous_wear));
            }
            wear.burning = false;
            let seat = id % 2;
            match id {
                4..=5 => self.f[seat].gear.melee = Carried::Absent,
                6..=7 => self.f[seat].gear.bow = Carried::Absent,
                8..=9 => self.disable_caltrops(seat),
                10..=11 => self.armor_destroy(seat),
                12..=13 => self.shield_destroy(seat),
                _ => unreachable!("not an object id"),
            }
        }
        damage
    }

    /// The point where a single mendable fracture left the object's pieces.
    pub fn mendable_object(&self, id: usize) -> Option<Point> {
        if !(FIRST_OBJECT..8).contains(&id) || self.object_kind(id).is_some() {
            return None;
        }
        self.objects.0[id - FIRST_OBJECT]
            .fracture
            .map(|(point, _)| point)
    }

    pub fn mend_object(&mut self, id: usize) -> bool {
        if self.mendable_object(id).is_none() {
            return false;
        }
        let wear = &mut self.objects.0[id - FIRST_OBJECT];
        let (point, previous_wear) = wear.fracture.take().expect("mendable object");
        wear.lost_hp = previous_wear;
        let gear = &mut self.f[id % 2].gear;
        if id >= 6 {
            gear.bow = Carried::Ground(point.cell);
        } else {
            gear.melee = Carried::Ground(point.cell);
        }
        true
    }

    pub fn ignite_object(&mut self, id: usize) -> bool {
        if !self.object(id).is_some_and(ObjectTarget::can_ignite) {
            return false;
        }
        self.objects.0[id - FIRST_OBJECT].burning = true;
        true
    }

    pub fn ignite_objects_in(&mut self, origin: Point, area: Area) {
        for id in FIRST_OBJECT..END_OBJECTS {
            if self.object(id).is_some_and(|target| {
                target.can_ignite() && self.area_hits_at(origin, target.point, area)
            }) {
                self.ignite_object(id);
            }
        }
    }

    pub fn damage_objects_in(&mut self, origin: Point, area: Area, damage: i32, kind: DamageType) {
        for id in FIRST_OBJECT..END_OBJECTS {
            if self.object(id).is_some_and(|target| {
                self.area_hits_at(origin, target.point, area) && !self.object_is_magical(id)
            }) {
                self.damage_object(id, damage, kind);
            }
        }
    }

    pub(super) fn move_ground_object(&mut self, id: usize, destination: Point) -> bool {
        if self.object(id).is_none() || !self.map.can_occupy(destination) {
            return false;
        }
        let landing = self.map.landing(destination);
        let seat = id % 2;
        match id {
            4..=5 => self.f[seat].gear.melee = Carried::Ground(landing.cell),
            6..=7 => self.f[seat].gear.bow = Carried::Ground(landing.cell),
            8..=9 if landing.altitude == 0 && self.caltrops(1 - seat) != Some(landing.cell) => {
                self.move_caltrops(seat, landing.cell);
            }
            10..=11 => {
                self.armor_move_ground(seat, landing);
            }
            12..=13 => {
                self.shield_move_ground(seat, landing);
            }
            _ => return false,
        }
        true
    }

    fn object_is_magical(&self, id: usize) -> bool {
        // No issued loadout begins magical. Magic Weapon's instance identity
        // is supplied by the spell subsystem where active.
        id < 8 && self.magic_enchanted_weapon(id % 2, id >= 6)
    }

    /// An object has no separate initiative. It burns once per round at the
    /// start of its original issue holder's turn, even if picked up meanwhile.
    pub(super) fn object_begin_turn(&mut self, seat: usize) {
        for id in (FIRST_OBJECT + seat..END_OBJECTS).step_by(2) {
            if !self.objects.0[id - FIRST_OBJECT].burning {
                continue;
            }
            if let Some(kind) = self.object_kind(id) {
                let fire = self.roll(4);
                self.apply_object_damage(id, kind, fire, DamageType::Fire);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        class::Class,
        geometry::{Map, Pos},
        Act, ACTS,
    };
    use super::*;

    fn battle(first: Class, second: Class) -> Battle {
        let mut battle = Battle::new(0, [first, second], [5, 5]);
        battle.owner = 0;
        battle.map = Map::new(0);
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 3, y: 0 };
        battle
    }

    #[test]
    fn steel_dagger_uses_table_ac_and_hp_and_destruction_removes_gear() {
        let mut b = battle(Class::Wizard, Class::Fighter);
        b.step(Act::Gear3).unwrap();
        let dagger = b.object(4).unwrap();
        assert_eq!((dagger.ac, dagger.hp), (19, 5));
        assert!(b.object_mask(1, 24) & (1 << 4) != 0);
        assert!(b.object_mask(1, 1) & (1 << 4) == 0);
        assert_eq!(b.damage_object(4, 50, DamageType::Poison), 0);
        assert_eq!(b.damage_object(4, 50, DamageType::Psychic), 0);
        assert_eq!(b.damage_object(4, 7, DamageType::Piercing), 3);
        assert_eq!(b.object(4).unwrap().hp, 2);
        assert!(!b.ignite_object(4));
        assert_eq!(b.damage_object(4, 4, DamageType::Force), 2);
        assert!(b.object(4).is_none());
        assert_eq!(b.f[0].gear.melee, Carried::Absent);
    }

    #[test]
    fn wooden_ground_object_ignites_and_burns_after_being_picked_up() {
        let mut b = battle(Class::Bard, Class::Wizard);
        b.f[0].gear.bow = Carried::Ground(b.f[0].pos);
        let bow = b.object(6).unwrap();
        assert_eq!((bow.ac, bow.hp), (15, 10));
        assert_eq!(b.damage_object(6, 1, DamageType::Fire), 2);
        assert!(b.ignite_object(6));
        assert!(b.object(6).unwrap().burning);
        b.f[0].gear.bow = Carried::Held;
        assert!(b.object(6).is_none());
        for _ in 0..4 {
            b.object_begin_turn(0);
        }
        assert_eq!(b.f[0].gear.bow, Carried::Absent);
    }

    #[test]
    fn ranged_objects_behind_a_wall_are_not_exposed_by_the_target_menu() {
        let mut b = battle(Class::Wizard, Class::Bard);
        b.f[0].pos = Pos { x: 0, y: 3 };
        b.f[1].gear.bow = Carried::Ground(Pos { x: 4, y: 3 });
        assert!(b.object(7).is_some());
        assert!(b.observed_object(0, 7).is_none());
        assert_eq!(b.object_mask(0, 24) & (1 << 7), 0);
    }

    #[test]
    fn destructing_caltrops_removes_the_hazard_without_refunding_a_bag() {
        let mut b = battle(Class::Rogue, Class::Fighter);
        b.step(Act::Special4).unwrap();
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[0]).unwrap();
        assert!(b.caltrops(0).is_some());
        assert_eq!(b.object(8).unwrap().hp, 5);
        assert_eq!(b.damage_object(8, 5, DamageType::Thunder), 5);
        assert!(b.caltrops(0).is_none());
        assert_eq!(b.f[0].secondary, 0);
    }

    #[test]
    fn mundane_attack_targets_gear_instead_of_the_issuing_fighter() {
        let mut damaged_object = false;
        for seed in 0..32 {
            let mut b = Battle::new(seed, [Class::Fighter, Class::Wizard], [5, 5]);
            b.owner = 0;
            b.map = Map::new(0);
            b.f[0].pos = Pos { x: 0, y: 0 };
            b.f[1].pos = Pos { x: 3, y: 0 };
            b.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 0 });
            let initial_hp = b.f[1].hp;
            b.step(Act::Melee).unwrap();
            assert_eq!(b.phase_id(), 14);
            assert_eq!(b.mask(), 1 << 5);
            b.step(ACTS[5]).unwrap();
            assert_eq!(b.f[1].hp, initial_hp);
            assert!(!b.action);
            damaged_object |= b.object(5).is_none_or(|target| target.hp < 5);
        }
        assert!(damaged_object);
    }

    #[test]
    fn mending_repairs_wooden_fractures_without_erasing_earlier_damage() {
        let mut b = battle(Class::Bard, Class::Wizard);
        let cell = b.f[0].pos;
        b.f[0].gear.bow = Carried::Ground(cell);
        assert_eq!(b.damage_object(6, 3, DamageType::Piercing), 3);
        assert!(!b.mend_object(6));
        assert_eq!(b.damage_object(6, 10, DamageType::Bludgeoning), 7);
        assert_eq!(b.mendable_object(6), Some(Point::ground(cell)));
        assert!(b.mend_object(6));
        assert_eq!(b.object(6).unwrap().hp, 7);
        assert!(!b.mend_object(6));
        b.damage_object(6, 1, DamageType::Fire);
        b.damage_object(6, 10, DamageType::Bludgeoning);
        assert!(b.mendable_object(6).is_none());

        b.f[1].gear.melee = Carried::Ground(cell);
        b.damage_object(5, 5, DamageType::Slashing);
        assert!(b.mendable_object(5).is_none());
    }

    #[test]
    fn fireball_ignites_wood_and_shatter_damages_unattended_gear() {
        let point = Pos { x: 5, y: 0 };
        let cast_at = |b: &mut Battle, spell: Act| {
            b.f[1].pos = Pos { x: 8, y: 6 };
            b.step(spell).unwrap();
            b.step(ACTS[point.y as usize]).unwrap();
            b.step(ACTS[point.x as usize]).unwrap();
            b.step(ACTS[0]).unwrap();
            if b.phase_id() == 38 {
                b.step(ACTS[b.mask().trailing_zeros() as usize]).unwrap();
            }
        };
        let mut fireball = battle(Class::Wizard, Class::Bard);
        fireball.f[1].gear.bow = Carried::Ground(point);
        cast_at(&mut fireball, Act::Spell4);
        let burning = fireball.object(7).unwrap();
        assert!(burning.burning);
        assert_eq!(burning.hp, 10);
        assert_eq!(fireball.f[1].gear.melee, Carried::Held);

        let mut shatter = battle(Class::Bard, Class::Fighter);
        shatter.f[1].gear.melee = Carried::Ground(point);
        cast_at(&mut shatter, Act::Spell3);
        assert!(shatter.object(5).is_none_or(|weapon| weapon.hp < 10));
    }

    #[test]
    fn dropped_armor_and_shield_are_attacked_by_issued_id_and_destroyed() {
        let mut b = battle(Class::Druid, Class::Fighter);
        b.armor_drop_for_wild_shape(0);
        b.gear_drop_shields_for_shape(0);
        let armor = b.object(10).unwrap();
        let shield = b.object(12).unwrap();
        assert_eq!((armor.ac, armor.hp, armor.label()), (11, 18, "Leather"));
        assert_eq!((shield.ac, shield.hp), (15, 10));
        assert!(armor.can_ignite() && shield.can_ignite());
        assert_eq!(b.damage_object(10, 20, DamageType::Fire), 18);
        assert_eq!(b.damage_object(12, 20, DamageType::Force), 10);
        assert!(!b.armor_present(0));
        assert!(!b.shield_present(0));
        assert!(b.object(10).is_none() && b.object(12).is_none());
    }
}
