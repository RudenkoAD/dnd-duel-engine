use super::DamageType;

#[derive(Clone, Copy, Debug)]
pub(super) struct WildForm {
    pub name: &'static str,
    pub ac: i32,
    pub speed: i32,
    pub physical: [i32; 3],
    pub attack_bonus: i32,
    pub die: i32,
    pub count: i32,
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    pub climb: i32,
}

pub(super) const WILD_FORMS: [WildForm; 6] = [
    WildForm {
        name: "Wolf",
        ac: 12,
        speed: 8,
        physical: [2, 2, 1],
        attack_bonus: 4,
        die: 6,
        count: 1,
        damage_bonus: 2,
        damage_type: DamageType::Piercing,
        climb: 0,
    },
    WildForm {
        name: "Rat",
        ac: 10,
        speed: 4,
        physical: [-4, 0, -1],
        attack_bonus: 2,
        die: 1,
        count: 1,
        damage_bonus: 0,
        damage_type: DamageType::Piercing,
        climb: 4,
    },
    WildForm {
        name: "Riding Horse",
        ac: 11,
        speed: 12,
        physical: [3, 1, 1],
        attack_bonus: 5,
        die: 8,
        count: 1,
        damage_bonus: 3,
        damage_type: DamageType::Bludgeoning,
        climb: 0,
    },
    WildForm {
        name: "Spider",
        ac: 12,
        speed: 4,
        physical: [-4, 2, -1],
        attack_bonus: 4,
        die: 1,
        count: 1,
        damage_bonus: 0,
        damage_type: DamageType::Piercing,
        climb: 4,
    },
    WildForm {
        name: "Black Bear",
        ac: 11,
        speed: 6,
        physical: [2, 1, 2],
        attack_bonus: 4,
        die: 6,
        count: 1,
        damage_bonus: 2,
        damage_type: DamageType::Slashing,
        climb: 6,
    },
    WildForm {
        name: "Giant Badger",
        ac: 13,
        speed: 6,
        physical: [1, 0, 3],
        attack_bonus: 3,
        die: 4,
        count: 2,
        damage_bonus: 1,
        damage_type: DamageType::Piercing,
        climb: 0,
    },
];

use super::{Act, Battle};
use crate::arena2::{
    origins::{raw_scores, skill_bonus, Skill},
};

impl Battle {
    pub(in crate::arena2) fn magic_end_wild_shape(&mut self, i: usize) {
        self.f[i].magic.form = false;
        self.f[i].magic.bear_next_rend = false;
        self.footprint_squeezed[i] = false;
    }

    pub fn combatant_size(&self, id: usize) -> u8 {
        if id >= 2 {
            return self.entity(id).map_or(2, |e| e.kind.size_category());
        }
        match self.wild_form(id).map(|f| f.name) {
            Some("Rat" | "Spider") => 0,
            Some("Riding Horse") => 3,
            _ => 2,
        }
    }

    pub fn body_score(&self, i: usize, stat: usize) -> i32 {
        let b = self.f[i].build;
        if let Some(form) = self.wild_form(i).filter(|_| stat < 3) {
            let physical = match form.name {
                "Wolf" => [14, 15, 12],
                "Rat" => [2, 11, 9],
                "Riding Horse" => [16, 13, 12],
                "Spider" => [2, 14, 8],
                "Black Bear" => [15, 12, 14],
                _ => [13, 10, 17],
            };
            return physical[stat];
        }
        raw_scores(b.class, b.level)[stat]
    }

    pub fn shape_spider_climb(&self, i: usize) -> bool {
        self.wild_form(i).is_some_and(|f| f.name == "Spider")
    }

    pub fn shape_skill_modifier(&self, i: usize, skill: Skill, stat: usize) -> i32 {
        let b = self.f[i].build;
        let own = skill_bonus(b, skill, stat);
        let Some(form) = self.wild_form(i) else {
            return own;
        };
        let stat_bonus = (self.body_score(i, stat) - 10).div_euclid(2);
        let retained = own - b.stats[stat] + stat_bonus;
        let beast = match (form.name, skill) {
            ("Wolf" | "Black Bear", Skill::Perception) => 5,
            ("Rat", Skill::Perception) => 2,
            ("Giant Badger", Skill::Perception) => 3,
            ("Wolf" | "Spider", Skill::Stealth) => 4,
            _ => return retained,
        };
        // The form can grant a proficiency the Druid lacks. A changed check ability
        // still uses that proficiency, alongside the higher printed stat-block bonus.
        retained
            .max(stat_bonus + b.pb)
            .max(if skill.ability() == stat {
                beast
            } else {
                i32::MIN
            })
    }

    /// Bit flags in an equipment disposition choice. Each weapon retains its issuer.
    fn form_equipment(&self, i: usize) -> u8 {
        let mut inventory = 0;
        for issuer in 0..2 {
            for ranged in [false, true] {
                if self.gear_carries_weapon(i, issuer, ranged) {
                    inventory |= 1 << (2 * issuer + usize::from(ranged));
                }
            }
        }
        if self.shield_holder(i) == Some(i) {
            inventory |= 1 << 4;
        }
        if self.armor_holder(i) == Some(i) {
            inventory |= 1 << 5;
        }
        inventory
    }

    pub(in crate::arena2) fn shape_mask(&self, i: usize, _phase: i32) -> u128 {
        if _phase == 102 {
            return self.bear_target_mask(i);
        }
        let inventory = if _phase == 115 {
            self.shape_foreign_gear(i)
        } else {
            self.form_equipment(i)
        };
        (0..64)
            .filter(|&opt| opt & !inventory == 0)
            .fold(0, |mask, opt| mask | 1 << opt)
    }

    pub(in crate::arena2) fn shape_label(&self, i: usize, _phase: i32, a: Act) -> Option<String> {
        if _phase == 102 {
            let id = a as usize;
            return Some(if id < 2 {
                if i == id {
                    "Rend yourself".into()
                } else {
                    "Rend opposing adventurer".into()
                }
            } else if id < 4 {
                format!("Rend {}", self.entity(id)?.kind.name())
            } else if id < crate::arena2::objects::END_OBJECTS {
                format!("Rend {}", self.object(id)?.label())
            } else {
                let p = self.bear_guess(i, id)?;
                format!(
                    "Rend at {}{}, {} ft high",
                    char::from(b'A' + p.cell.x as u8),
                    p.cell.y + 1,
                    p.altitude * 5
                )
            });
        }
        let choice = a as u8;
        if _phase == 115 {
            let mut drops = Vec::new();
            if choice & 1 != 0 {
                drops.push("the borrowed shield");
            }
            if choice & 2 != 0 {
                drops.push(self.armor_ground_name(1 - i));
            }
            return Some(if drops.is_empty() {
                "Merge all borrowed armor into the form".into()
            } else {
                format!("Drop {}; merge other borrowed armor", drops.join(" and "))
            });
        }
        if choice == 0 {
            return Some(
                if self.shape_foreign_gear(i) == 0 {
                    "Merge all carried equipment into the new form"
                } else {
                    "Merge listed equipment and choose borrowed armor next"
                }
                .into(),
            );
        }
        let mut dropped = Vec::new();
        for issuer in 0..2 {
            for ranged in [false, true] {
                if choice & (1 << (2 * issuer + usize::from(ranged))) != 0 {
                    let item = self.gear_item_name_for(issuer, ranged)?;
                    dropped.push(format!(
                        "{} ({})",
                        item,
                        if issuer == i { "issued" } else { "borrowed" }
                    ));
                }
            }
        }
        if choice & 16 != 0 {
            dropped.push("shield".into());
        }
        if choice & 32 != 0 {
            dropped.push("fitted leather armor".into());
        }
        Some(format!("Drop {}; merge remaining gear", dropped.join(", ")))
    }

    pub(in crate::arena2) fn shape_step(&mut self, i: usize, _phase: i32, a: Act) {
        if _phase == 102 {
            let id = a as usize;
            let target = if id < crate::arena2::objects::END_OBJECTS {
                Some(id)
            } else {
                self.bear_guess(i, id).and_then(|p| {
                    (0..4).find(|&t| {
                        self.footprint(t)
                            .is_some_and(|body| body.squares().any(|q| q == p))
                    })
                })
            };
            if self.f[i].magic.bear_next_rend {
                self.f[i].magic.bear_next_rend = false;
            } else {
                self.action = false;
                self.f[i].magic.bear_next_rend = true;
            }
            self.form_attack_target(i, target);
            return;
        }
        if _phase == 100 && self.shape_foreign_gear(i) != 0 {
            self.f[i].magic.shape_drop = a as u8;
            self.request(i, 115);
            return;
        }
        let choice = if _phase == 115 {
            self.f[i].magic.shape_drop
        } else {
            a as u8
        };
        let borrowed = if _phase == 115 { a as u8 } else { 0 };
        let cell = self.f[i].pos;
        for issuer in 0..2 {
            for ranged in [false, true] {
                if choice & (1 << (2 * issuer + usize::from(ranged))) != 0 {
                    self.gear_drop_weapon_instance(i, issuer, ranged, cell);
                }
            }
        }
        let ground = self.map.landing(self.fighter_point(i));
        if choice & 16 != 0 {
            self.shield_on_ground(i, ground);
        }
        if borrowed & 1 != 0 {
            self.shield_on_ground(1 - i, ground);
        }
        if borrowed & 2 != 0 {
            self.drop_form_armor(1 - i, ground);
        }
        if choice & 32 != 0 {
            self.drop_form_armor(i, ground);
        }
        self.armor_interrupt(i);
        self.magic_lose_weapon_contact(i);
        self.f[i].magic.form_id = self.f[i].magic.next_form;
        self.f[i].magic.bear_next_rend = false;
        self.execute_magic_step(i, Act::Special1);
        self.footprint_transform(i);
    }
}

impl Battle {
    pub(in crate::arena2) fn shape_bite_prone(&mut self, hit: super::Hit) {
        if hit.attacker >= 2
            || !hit.weapon
            || hit.ranged
            || hit.unarmed
            || hit.improvised
            || hit.defender >= 4
            || !self
                .wild_form(hit.attacker)
                .is_some_and(|f| f.name == "Wolf")
            || self.combatant_size(hit.defender) > 2
        {
            return;
        }
        if hit.defender < 2 {
            self.knock_prone(hit.defender);
        } else {
            self.entity_knock_prone(hit.defender);
        }
    }

    fn shape_foreign_gear(&self, i: usize) -> u8 {
        u8::from(self.shield_holder(1 - i) == Some(i))
            | (u8::from(self.armor_holder(1 - i) == Some(i)) << 1)
    }

    fn bear_guess(&self, i: usize, id: usize) -> Option<crate::arena2::geometry::Point> {
        let n = id.checked_sub(16)?;
        if n >= 27 {
            return None;
        }
        let mut p = self.fighter_point(i);
        p.cell.x += (n % 3) as i8 - 1;
        p.cell.y += ((n / 3) % 3) as i8 - 1;
        p.altitude += (n / 9) as i8 - 1;
        (self.map.can_occupy(p) && self.map.los_at(self.fighter_point(i), p)).then_some(p)
    }
    pub(super) fn bear_target_mask(&self, i: usize) -> u128 {
        let mut bits = self.object_mask(i, 1);
        for target in 0..4 {
            let known = if target < 2 {
                self.observed_pos(i, target).is_some()
            } else {
                self.observed_entity(i, target).is_some()
            };
            if known
                && self.combatant_distance(i, target).is_some_and(|d| d <= 1)
                && self.combatant_los(i, target)
            {
                bits |= 1 << target;
            }
        }
        if self.observed_pos(i, 1 - i).is_none() {
            for id in 16..43 {
                if self.bear_guess(i, id).is_some() {
                    bits |= 1 << id;
                }
            }
        }
        bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::equipment::Carried;
    use crate::arena2::{class::Class, geometry::Pos, ACTS};

    fn druid(seed: u64, level: i32) -> Battle {
        let mut b = Battle::new(seed, [Class::Druid, Class::Fighter], [level, level]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 1, y: 0 };
        b
    }

    fn transform(b: &mut Battle, form: usize, drop: usize) {
        b.step(Act::Special1).unwrap();
        b.step(ACTS[form]).unwrap();
        assert_eq!(b.phase_id(), 100);
        b.step(ACTS[drop]).unwrap();
        if b.phase_id() == 103 {
            let placement = b.mask().trailing_zeros() as usize;
            b.step(ACTS[placement]).unwrap();
        }
    }

    #[test]
    fn wild_shape_dispositions_follow_actual_carried_items_and_keep_their_locations() {
        let mut b = druid(19, 4);
        let resources = b.f[0].resource;
        b.step(Act::Special1).unwrap();
        b.step(ACTS[1]).unwrap();
        let inventory = 1 | 16 | 32; // Staff, shield and fitted armor.
        assert_eq!(b.mask().count_ones(), 8);
        for choice in 0..64 {
            assert_eq!(b.mask() & (1 << choice) != 0, choice & !inventory == 0);
        }
        b.step(ACTS[1 | 32]).unwrap();
        assert_eq!(b.object(4).unwrap().point.cell, b.f[0].pos);
        assert!(!b.armor_carried(0));
        assert!(!b.gear_free_hand(0));
        assert_eq!(b.gear_mask(0), 0);
        assert_eq!(b.combatant_size(0), 0);
        assert_eq!(b.body_score(0, 0), 2);
        assert_eq!(b.f[0].resource, resources - 1);
        b.step(Act::End).unwrap();
        b.step(Act::End).unwrap();
        b.step(Act::Special2).unwrap();
        assert!(!b.magic_is_wolf(0));
        assert!(b.object(4).is_some());
        assert!(!b.armor_carried(0));
        assert_eq!(b.f[0].gear.shield, Carried::Held);
    }

    #[test]
    fn wild_shape_selects_borrowed_shield_and_armor_independently_of_issued_gear() {
        for seat in 0..2 {
            let mut classes = [Class::Fighter; 2];
            classes[seat] = Class::Druid;
            let mut setup = Battle::new(19, classes, [4; 2]);
            setup.owner = seat;
            setup.f[seat].pos = Pos { x: 0, y: 0 };
            setup.f[1 - seat].pos = Pos { x: 1, y: 0 };
            let floor = setup.fighter_point(seat);
            setup.drop_form_armor(1 - seat, floor);
            setup.step(Act::Other5).unwrap();
            assert!(setup.armor_holds_foreign(seat));
            setup.f[1 - seat].gear.shield = Carried::ForeignPacked;

            setup.step(Act::Special1).unwrap();
            setup.step(ACTS[1]).unwrap();
            assert_eq!(setup.phase_id(), 100);
            setup.step(ACTS[16]).unwrap(); // Drop the Druid's own shield, merge its own leather.
            assert_eq!(setup.phase_id(), 115);
            assert_eq!(setup.mask(), 0b1111);
            assert!(setup
                .choice_label(seat, ACTS[1])
                .contains("borrowed shield"));
            for drop in 0..4 {
                let mut battle = setup.clone();
                battle.step(ACTS[drop]).unwrap();
                assert!(battle.magic_is_wolf(seat));
                assert_eq!(battle.shield_ground(seat), Some(floor));
                assert_eq!(battle.armor_holder(seat), Some(seat));
                assert_eq!(
                    battle.shield_ground(1 - seat),
                    (drop & 1 != 0).then_some(floor)
                );
                assert_eq!(
                    battle.armor_ground(1 - seat),
                    (drop & 2 != 0).then_some(floor)
                );
            }
        }
    }

    #[test]
    fn black_bear_multiattack_offers_two_separate_rends_with_movement_between() {
        let mut b = druid(1, 4);
        b.f[0].hp = 100;
        b.f[1].hp = 100;
        transform(&mut b, 4, 0);
        b.step(Act::Martial6).unwrap();
        assert_eq!(b.phase_id(), 102);
        b.step(ACTS[0]).unwrap();
        assert!(!b.action);
        assert_eq!(b.attacks, 0);
        let steps = b.movement_left();
        b.step(Act::S).unwrap();
        assert!(b.movement_left() < steps);
        b.step(Act::Martial6).unwrap();
        b.step(ACTS[1]).unwrap();
        assert!(!b.f[0].magic.bear_next_rend);
        assert_eq!(b.mask() & Act::Martial6.bit(), 0);
    }

    #[test]
    fn six_forms_use_their_real_bodies_and_retain_the_druids_mental_abilities() {
        let b = druid(7, 5);
        let starting = b.f[0].build.stats;
        for (id, (size, speed, climb, scores)) in [
            (2, 8, 0, [14, 15, 12]),
            (0, 4, 4, [2, 11, 9]),
            (3, 12, 0, [16, 13, 12]),
            (0, 4, 4, [2, 14, 8]),
            (2, 6, 6, [15, 12, 14]),
            (2, 6, 0, [13, 10, 17]),
        ]
        .into_iter()
        .enumerate()
        {
            let mut form = b.clone();
            transform(&mut form, id, 0);
            assert_eq!(
                (
                    form.combatant_size(0),
                    form.magic_speed(0),
                    form.magic_climb_speed(0)
                ),
                (size, speed, climb)
            );
            for stat in 0..3 {
                assert_eq!(form.body_score(0, stat), scores[stat]);
            }
            for stat in 3..6 {
                assert_eq!(form.magic_save_modifier(0, stat), starting[stat]);
            }
            assert_eq!(form.magic_agile(0), id == 1);
            assert_eq!(form.shape_spider_climb(0), id == 3);
            assert_eq!(form.magic_resists_poison(0), id == 5);
            assert_eq!(
                form.shape_skill_modifier(0, Skill::Perception, 4)
                    >= starting[4] + form.f[0].build.pb,
                true
            );
        }
    }

    #[test]
    fn pack_tactics_needs_an_active_ally_adjacent_to_the_target() {
        use crate::arena2::entities::{Entity, EntityKind, FamiliarForm, Spirit};
        let mut b = druid(1, 3);
        transform(&mut b, 0, 0);
        assert!(!b.magic_pack_advantage(0, 1));
        b.fixture_companion(
            0,
            Entity::new(
                EntityKind::Familiar {
                    form: FamiliarForm::Rat,
                    spirit: Spirit::Fey,
                },
                0,
                Pos { x: 1, y: 1 },
                0,
            ),
        );
        assert!(b.magic_pack_advantage(0, 1));
        b.martial_stun_entity(0, 2, 99);
        assert!(!b.magic_pack_advantage(0, 1));
    }

    #[test]
    fn wolf_bite_prones_medium_creatures_and_rat_uses_a_fixed_one_point_bite() {
        let mut bitten = false;
        let mut rat_hit = false;
        for seed in 0..64 {
            for form in [0, 1] {
                let mut b = druid(seed, 3);
                transform(&mut b, form, 0);
                let hp = b.f[1].hp;
                b.form_attack_target(0, Some(1));
                if b.f[1].hp < hp {
                    if form == 0 {
                        assert!(b.f[1].prone);
                        bitten = true;
                    } else {
                        assert_eq!(hp - b.f[1].hp, 1);
                        rat_hit = true;
                    }
                }
            }
        }
        assert!(bitten && rat_hit);
    }
}
