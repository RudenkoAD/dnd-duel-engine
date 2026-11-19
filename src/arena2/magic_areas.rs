//! Spatial spells and their recurring, per-creature area events.
use super::*;
use std::collections::VecDeque;

impl Battle {

    pub(super) fn cast_hideous_laughter(&mut self, caster: usize, tier: usize) {
        let recipients = self.f[caster].magic.cast_targets;
        let dc = self.spell_dc(caster);
        self.f[caster].concentration = None;
        let mut affected = 0;
        for id in 0..4 {
            if recipients & (1 << id) == 0 {
                continue;
            }
            let save = if id < 2 { self.save(id, WIS, dc) } else { self.entity_save(id, WIS, dc) };
            if !save {
                affected |= 1 << id;
                if id < 2 {
                    self.f[id].prone = true;
                } else {
                    self.entity_knock_prone(id);
                }
            }
        }
        if affected != 0 && affected & (1 << caster) == 0 {
            self.concentrate(caster, Concentration::HideousLaughter, 20, None, tier);
            self.areas.afflicted[caster] = affected;
        }
    }

    fn laugh_repeat_save(&mut self, id: usize, damaged: bool) {
        for caster in 0..2 {
            if self.f[caster].concentration != Some(Concentration::HideousLaughter)
                || self.areas.afflicted[caster] & (1 << id) == 0
                || self.creature_point(id).is_none()
            {
                continue;
            }
            let dc = self.spell_dc(caster);
            let saved = if id < 2 { self.save_advantage(id, WIS, dc, damaged) }
                else { self.entity_save_with_advantage(id, WIS, dc, damaged) };
            if saved {
                self.areas.afflicted[caster] &= !(1 << id);
                if self.areas.afflicted[caster] == 0 {
                    self.f[caster].concentration = None;
                }
            }
        }
    }

    pub(super) fn spell_burst_damage(
        &mut self,
        i: usize,
        stat: usize,
        dice: (i32, i32),
        area: Area,
        heightened: bool,
        kind: DamageType,
    ) {
        let center = self.chosen_center();
        let dc = self.spell_dc(i);
        let mut recipients = 0;
        let mut saved = 0;
        for id in [1 - i, i, 2, 3] {
            if self.area_contains_creature(center, id, area) {
                recipients |= 1 << id;
                if if id < 2 {
                    self.save_from_point(
                        id,
                        stat,
                        dc,
                        heightened && self.magic_heightened_for(i, id),
                        center,
                    )
                } else {
                    self.entity_save_from(
                        id,
                        stat,
                        dc,
                        center,
                        heightened && self.magic_heightened_for(i, id),
                    )
                } {
                    saved |= 1 << id;
                }
            }
        }
        if kind == DamageType::Fire {
            self.ignite_objects_in(center, area);
        }
        if kind == DamageType::Thunder || recipients != 0 {
            let amount = self.roll_many(dice.0, dice.1);
            self.offer_spell_damage(SpellDamage {
                source: i,
                emitter: None,
                amount,
                kind,
                recipients,
                saved,
                checked: 0,
                rider: SpellRider::None,
                objects: (kind == DamageType::Thunder).then_some((center, area)),
            });
        }
    }

    pub(super) fn directional_area_damage(
        &mut self,
        i: usize,
        stat: usize,
        count: i32,
        die: i32,
        kind: DamageType,
        heightened: bool,
        area: Area,
        rider: SpellRider,
    ) {
        let origin = self.fighter_point(i);
        if matches!(rider, SpellRider::Thunderwave(_)) {
            self.area_thunderwave_objects(origin, area);
        }
        let mut recipients = 0;
        let mut saved = 0;
        for id in [1 - i, 2, 3] {
            if self.area_contains_creature(origin, id, area) {
                recipients |= 1 << id;
                let dc = self.spell_dc(i);
                if if id < 2 {
                    self.save_from_point(
                        id,
                        stat,
                        dc,
                        heightened && self.magic_heightened_for(i, id),
                        origin,
                    )
                } else {
                    self.entity_save_from(
                        id,
                        stat,
                        dc,
                        origin,
                        heightened && self.magic_heightened_for(i, id),
                    )
                } {
                    saved |= 1 << id;
                }
            }
        }
        if recipients != 0 {
            let amount = self.roll_many(count, die);
            self.offer_spell_damage(SpellDamage {
                source: i,
                emitter: None,
                amount,
                kind,
                recipients,
                saved,
                checked: 0,
                rider,
                objects: None,
            });
        }
    }
}

const SPIRIT_DESIGNATION: i32 = 92;
const LAND_DAMAGE: i32 = 93;
const LAND_HEAL: i32 = 94;
const WAKE_TARGET: i32 = 95;
pub(super) const SLEEP_TARGETS: i32 = 177;
const AREA_REACTION: i32 = 96;
const BREATH_TOUCH: i32 = 104;
const COMPANION_DIRECTION: i32 = 105;
const CREATURES: [usize; 4] = [0, 1, 2, 3];

#[derive(Clone, Copy, Debug)]
enum AreaChoice {
    Guardians {
        tier: usize,
        eligible: u8,
    },
    LandDamage {
        center: Point,
        eligible: u8,
    },
    LandHeal {
        center: Point,
        eligible: u8,
        damage: u8,
    },
    Wake {
        eligible: u8,
    },
    Sleep { tier: usize, center: Point, eligible: u8, heightened: u8 },
    Recurring(SpellDamage),
    BreathTouch {
        action: Act,
        eligible: u8,
    },
    BreathConsent {
        caster: usize,
        action: Act,
        id: usize,
    },
    BreathDirection {
        id: usize,
    },
}

#[derive(Clone, Debug)]
pub(in crate::arena2) struct AreaState {
    pub(super) epoch: i32,
    faerie_marks: [u16; 2],
    mind_sliver_until: [[i32; 4]; 2],
    afflicted: [u8; 2],
    asleep: [u8; 2],
    exempt: [u8; 2],
    poison_epoch: [i32; 4],
    breath_target: [Option<usize>; 2],
    fear_last_seen: [Option<Point>; 4],
    truth_bound: [u8; 2],
    truth_epoch: [[i32; 4]; 2],
    choice: Option<AreaChoice>,
    pending_damage: VecDeque<SpellDamage>,
}

impl Default for AreaState {
    fn default() -> Self {
        Self {
            epoch: 0,
            faerie_marks: [0; 2],
            mind_sliver_until: [[-1; 4]; 2],
            afflicted: [0; 2],
            asleep: [0; 2],
            exempt: [0; 2],
            poison_epoch: [-1; 4],
            breath_target: [None; 2],
            fear_last_seen: [None; 4],
            truth_bound: [0; 2],
            truth_epoch: [[-1; 4]; 2],
            choice: None,
            pending_damage: VecDeque::new(),
        }
    }
}

impl Battle {
    pub(super) fn mark_mind_sliver(&mut self, source: usize, recipient: usize) {
        // Count through the end of the caster's next turn, including reaction casts.
        self.areas.mind_sliver_until[source][recipient] = self.turns + if self.owner == source { 2 } else { 1 };
    }

    pub(in crate::arena2) fn take_mind_sliver(&mut self, recipient: usize) -> i32 {
        let mut afflicted = false;
        for deadlines in &mut self.areas.mind_sliver_until {
            afflicted |= deadlines[recipient] >= self.turns;
            deadlines[recipient] = -1;
        }
        if afflicted { self.roll(4) } else { 0 }
    }

    pub(super) fn dispel_faerie_mark(&mut self, caster: usize, target: usize) {
        self.areas.faerie_marks[caster] &= !(1 << target);
        if self.areas.faerie_marks[caster] == 0 {
            self.f[caster].concentration = None;
        }
    }

    pub(in crate::arena2) fn faerie_outlined(&self, target: usize) -> bool {
        (0..2).any(|caster| self.f[caster].concentration == Some(Concentration::FaerieFire)
            && self.areas.faerie_marks[caster] & (1 << target) != 0)
    }

    pub(super) fn cast_faerie_fire(&mut self, caster: usize, tier: usize, heightened: bool) {
        let center = self.chosen_center();
        let shape = Area::Cube { half_side: 2 };
        self.concentrate(caster, Concentration::FaerieFire, 10, None, tier);
        let eligible = self.area_creature_mask(center, shape);
        let dis = if heightened { self.area_heightened_mask(caster) } else { 0 };
        let saved = self.area_save_mask(eligible, DEX, self.spell_dc(caster), center, dis);
        let mut marks = u16::from(eligible & !saved);
        for id in crate::arena2::objects::FIRST_OBJECT..crate::arena2::objects::END_OBJECTS {
            if self.object(id).is_some_and(|obj| self.area_hits_at(center, obj.point, shape)) {
                marks |= 1 << id;
            }
        }
        self.areas.faerie_marks[caster] = marks;
    }

    fn hadar_turn_damage(&mut self, caster: usize, target: usize, start: bool) {
        if self.f[caster].concentration != Some(Concentration::HungerOfHadar) {
            return;
        }
        let Some(center) = self.f[caster].magic.field else { return };
        if !self.area_contains_creature(center, target, Area::Sphere { radius: 4 }) {
            return;
        }
        if !start && self.save_in_area(target, DEX, self.spell_dc(caster), center,
            self.f[caster].magic.concentration_heightened_target == Some(target)) {
            return;
        }
        let amount = self.roll_many(2, 6);
        self.areas.pending_damage.push_back(SpellDamage {
            source: caster, emitter: None, amount,
            kind: if self.f[caster].arcane.psychic_hadar { DamageType::Psychic }
                else if start { DamageType::Cold } else { DamageType::Acid },
            recipients: 1 << target, saved: 0, checked: 0, rider: SpellRider::None, objects: None,
        });
    }

    fn area_contains_creature(&self, origin: Point, id: usize, area: Area) -> bool {
        self.present_on_arena_plane(id)
            && (id >= 2 || self.f[id].hp > 0)
            && self.footprint(id).is_some_and(|footprint| {
                footprint
                    .squares()
                    .any(|point| self.area_hits_at(origin, point, area))
            })
    }

    fn area_creature_mask(&self, center: Point, area: Area) -> u8 {
        CREATURES
            .into_iter()
            .filter(|&id| self.area_contains_creature(center, id, area))
            .fold(0, |mask, id| mask | 1 << id)
    }

    fn save_in_area(
        &mut self,
        id: usize,
        stat: usize,
        dc: i32,
        origin: Point,
        disadvantage: bool,
    ) -> bool {
        if id < 2 {
            self.save_from_point(id, stat, dc, disadvantage, origin)
        } else {
            self.entity_save_from(id, stat, dc, origin, disadvantage)
        }
    }

    fn area_save_mask(&mut self, eligible: u8, stat: usize, dc: i32, origin: Point, dis: u8) -> u8 {
        CREATURES
            .into_iter()
            .filter(|&id| {
                eligible & (1 << id) != 0
                    && self.save_in_area(id, stat, dc, origin, dis & (1 << id) != 0)
            })
            .fold(0, |mask, id| mask | 1 << id)
    }

    fn area_heightened_mask(&self, caster: usize) -> u8 {
        CREATURES
            .into_iter()
            .filter(|&id| self.magic_heightened_for(caster, id))
            .fold(0, |mask, id| mask | 1 << id)
    }

    pub(super) fn start_breath_touch(&mut self, caster: usize, action: Act) {
        let mut eligible = 1 << caster;
        for id in CREATURES.into_iter().filter(|&id| id != caster) {
            let known = if id < 2 {
                self.observed_pos(caster, id).is_some()
            } else {
                self.observed_entity(caster, id).is_some()
            };
            if known
                && self.combatant_distance(caster, id).is_some_and(|d| d <= 1)
                && self.combatant_los(caster, id)
            {
                eligible |= 1 << id;
            }
        }
        self.areas.choice = Some(AreaChoice::BreathTouch { action, eligible });
        self.request(caster, BREATH_TOUCH);
    }

    pub(super) fn attach_breath(&mut self, caster: usize, id: usize) {
        self.areas.breath_target[caster] = Some(id);
    }

    pub(super) fn area_breath_source(&self, id: usize) -> Option<usize> {
        (0..2).find(|&caster| {
            self.f[caster].concentration == Some(Concentration::DragonBreath)
                && self.areas.breath_target[caster] == Some(id)
        })
    }

    pub(in crate::arena2) fn companion_can_exhale(&self, id: usize) -> bool {
        id >= 2 && self.creature_point(id).is_some() && self.area_breath_source(id).is_some()
    }

    pub(in crate::arena2) fn begin_companion_exhale(&mut self, id: usize) {
        self.areas.choice = Some(AreaChoice::BreathDirection { id });
        self.request(id - 2, COMPANION_DIRECTION);
    }

    pub(super) fn area_exhale(&mut self, id: usize, direction: usize) {
        let Some(source) = self.area_breath_source(id) else {
            return;
        };
        let Some(origin) = self.creature_point(id) else {
            return;
        };
        let cone = Area::Cone {
            length: 3,
            direction,
        };
        let victims = self.area_creature_mask(origin, cone) & !(1 << id);
        if victims == 0 {
            return;
        }
        let saved = self.area_save_mask(
            victims,
            DEX,
            self.spell_dc(source),
            origin,
            self.area_heightened_mask(source),
        );
        let amount = self.roll_many(self.f[source].magic.field_level as i32 + 1, 6);
        self.areas.pending_damage.push_back(SpellDamage {
            source,
            emitter: Some(id),
            amount,
            kind: self.f[source].magic.breath_element,
            recipients: victims,
            saved,
            checked: 0,
            rider: SpellRider::None,
            objects: None,
        });
    }

    pub(super) fn cast_sleep(&mut self, caster: usize, tier: usize, heightened: bool) {
        let center = self.chosen_center();
        let eligible = self.area_creature_mask(center, Area::Sphere { radius: 1 });
        let heightened = if heightened { self.area_heightened_mask(caster) } else { 0 };
        let eligible = CREATURES.into_iter().filter(|&id| {
            eligible & (1 << id) != 0 && (id == caster
                || if id < 2 { self.observed_pos(caster, id).is_some() }
                    else { self.observed_entity(caster, id).is_some() })
        }).fold(0, |mask, id| mask | 1 << id);
        self.f[caster].concentration = None;
        if eligible != 0 {
            self.areas.choice = Some(AreaChoice::Sleep { tier, center, eligible, heightened });
            self.request(caster, SLEEP_TARGETS);
        }
    }

    fn sleep_repeat_save(&mut self, id: usize) {
        for caster in 0..2 {
            if self.f[caster].concentration != Some(Concentration::Sleep)
                || self.areas.afflicted[caster] & (1 << id) == 0
                || self.areas.asleep[caster] & (1 << id) != 0
                || self.creature_point(id).is_none()
            {
                continue;
            }
            let dc = self.spell_dc(caster);
            let disadvantage = self.f[caster].magic.concentration_heightened_target == Some(id);
            let saved = if id < 2 { self.save_advantage(id, WIS, dc, disadvantage) }
                else if let Some(origin) = self.creature_point(id) {
                    self.entity_save_from(id, WIS, dc, origin, disadvantage)
                } else { false };
            if saved {
                self.dispel_area_affliction(caster, id, Concentration::Sleep);
            } else {
                self.areas.asleep[caster] |= 1 << id;
                if id < 2 {
                    self.knock_prone(id);
                    self.drop_held(id);
                } else {
                    self.entity_knock_prone(id);
                }
            }
        }
    }

    pub(in crate::arena2) fn sleep_affected(&self, id: usize) -> bool {
        (0..2).any(|caster| self.f[caster].concentration == Some(Concentration::Sleep)
            && self.areas.afflicted[caster] & (1 << id) != 0)
    }

    pub(in crate::arena2) fn sleep_unconscious(&self, id: usize) -> bool {
        (0..2).any(|caster| self.f[caster].concentration == Some(Concentration::Sleep)
            && self.areas.afflicted[caster] & self.areas.asleep[caster] & (1 << id) != 0)
    }

    pub(in crate::arena2) fn magic_area_speed_zero(&self, id: usize) -> bool {
        self.sleep_unconscious(id) || (0..2).any(|caster| {
            self.f[caster].concentration == Some(Concentration::HypnoticPattern)
                && self.areas.afflicted[caster] & (1 << id) != 0
        })
    }

    pub(in crate::arena2) fn magic_laughter(&self, id: usize) -> bool {
        (0..2).any(|caster| self.f[caster].concentration == Some(Concentration::HideousLaughter)
            && self.areas.afflicted[caster] & (1 << id) != 0)
    }

    pub(super) fn cast_hypnotic_pattern(&mut self, caster: usize, tier: usize, heightened: bool) {
        let center = self.chosen_center();
        let shape = Area::Cube { half_side: 3 };
        let victims = CREATURES
            .into_iter()
            .filter(|&id| {
                (id >= 2 || self.f[id].hp > 0)
                    && self.footprint(id).is_some_and(|footprint| {
                        footprint.squares().any(|point| {
                            self.area_hits_at(center, point, shape)
                                && !self.magic_cloud_blocks(point, center)
                        })
                    })
            })
            .fold(0, |mask, id| mask | 1 << id);
        let saves = self.area_save_mask(
            victims,
            WIS,
            self.spell_dc(caster),
            center,
            if heightened {
                self.area_heightened_mask(caster)
            } else {
                0
            },
        );
        let failed = victims & !saves;
        self.f[caster].concentration = None;
        if failed & (1 << caster) != 0 || failed == 0 {
            return;
        }
        self.concentrate(caster, Concentration::HypnoticPattern, 10, None, tier);
        self.areas.afflicted[caster] = failed;
        for seat in 0..2 {
            if failed & (1 << seat) != 0 {
                self.magic_end_wild_shape(seat);
                self.f[seat].concentration = None;
            }
        }
    }

    pub(super) fn cast_entangle(&mut self, caster: usize, tier: usize, heightened: bool) {
        let center = self.chosen_center();
        self.concentrate(caster, Concentration::Entangle, 10, Some(center), tier);
        let victims =
            self.area_creature_mask(center, Area::GroundSquare { half_side: 2 }) & !(1 << caster);
        let saves = self.area_save_mask(
            victims,
            0,
            self.spell_dc(caster),
            center,
            if heightened {
                self.area_heightened_mask(caster)
            } else {
                0
            },
        );
        self.areas.afflicted[caster] = victims & !saves;
    }

    pub(super) fn start_spirit_guardians(&mut self, caster: usize, tier: usize) {
        let eligible = CREATURES
            .into_iter()
            .filter(|&id| {
                id != caster
                    && if id < 2 {
                        self.f[id].hp > 0
                    } else {
                        self.observed_entity(caster, id).is_some()
                    }
            })
            .fold(0, |bits, id| bits | 1 << id);
        self.areas.choice = Some(AreaChoice::Guardians { tier, eligible });
        self.request(caster, SPIRIT_DESIGNATION);
    }

    pub(super) fn start_lands_aid(&mut self, caster: usize) {
        self.start_lands_aid_at(caster, self.chosen_center());
    }

    pub(in crate::arena2) fn start_lands_aid_at(&mut self, caster: usize, center: Point) {
        let eligible = CREATURES
            .into_iter()
            .filter(|&id| {
                if id < 2 && self.f[id].hp > 0 && self.observed_pos(caster, id).is_none() {
                    // A player can choose the opposing combatant without knowing
                    // whether the selected area actually catches that creature.
                    return true;
                }
                if id >= 2 && self.observed_entity(caster, id).is_none() {
                    return false;
                }
                self.observed_footprint(caster, id).is_some_and(|f| {
                    f.squares()
                        .any(|p| self.area_hits_at(center, p, Area::Sphere { radius: 2 }))
                })
            })
            .fold(0, |bits, id| bits | 1 << id);
        self.areas.choice = Some(AreaChoice::LandDamage { center, eligible });
        self.request(caster, LAND_DAMAGE);
    }

    pub(super) fn start_moonbeam(&mut self, caster: usize, tier: usize) {
        self.concentrate(
            caster,
            Concentration::Moonbeam,
            10,
            Some(self.chosen_center()),
            tier,
        );
        for id in [1 - caster, caster, 2, 3] {
            self.magic_field_hit(caster, id);
        }
    }

    pub(super) fn move_moonbeam(&mut self, caster: usize) {
        self.move_moonbeam_at(caster, self.chosen_center());
    }

    pub(in crate::arena2) fn move_moonbeam_at(&mut self, caster: usize, center: Point) {
        let inside = CREATURES.map(|id| {
            self.creature_point(id)
                .is_some_and(|p| self.field_contains(caster, id, p))
        });
        self.f[caster].magic.field = Some(center);
        for id in [1 - caster, caster, 2, 3] {
            self.field_transition(caster, id, inside[id]);
        }
    }

    fn field_contains(&self, caster: usize, target: usize, pos: Point) -> bool {
        let (center, area) = match self.f[caster].concentration {
            Some(Concentration::Moonbeam) => (
                self.f[caster].magic.field,
                Area::Cylinder {
                    radius: 1,
                    height: 8,
                },
            ),
            Some(Concentration::SpiritGuardians)
                if target != caster && self.areas.exempt[caster] & (1 << target) == 0 =>
            {
                (Some(self.fighter_point(caster)), Area::Sphere { radius: 3 })
            }
            _ => return false,
        };
        center.is_some_and(|center| {
            self.footprint(target).is_some_and(|mut footprint| {
                footprint.anchor = pos;
                footprint
                    .squares()
                    .any(|point| self.area_hits_at(center, point, area))
            })
        })
    }

    pub(super) fn moonbeam_blocks_shape(&self, i: usize) -> bool {
        self.f.iter().enumerate().any(|(caster, f)| {
            f.concentration == Some(Concentration::Moonbeam)
                && f.magic.field_blocks_shape[i]
                && self.field_contains(caster, i, self.fighter_point(i))
        })
    }

    fn field_transition(&mut self, caster: usize, target: usize, was_inside: bool) {
        let inside = self
            .creature_point(target)
            .is_some_and(|p| self.field_contains(caster, target, p));
        if inside && !was_inside {
            self.magic_field_hit(caster, target);
        } else if !inside {
            self.f[caster].magic.field_blocks_shape[target] = false;
        }
    }

    pub(super) fn magic_field_hit(&mut self, caster: usize, target: usize) {
        let state = &self.f[caster].magic;
        if (target < 2 && self.f[target].hp <= 0)
            || self.f[caster].hp <= 0
            || state.field_last_turn[target] == self.areas.epoch
            || !self
                .creature_point(target)
                .is_some_and(|p| self.field_contains(caster, target, p))
        {
            return;
        }
        let tier = state.field_level as i32;
        let moonbeam = self.f[caster].concentration == Some(Concentration::Moonbeam);
        let (stat, die, origin) = if moonbeam {
            (CON, 10, state.field.expect("Moonbeam center"))
        } else {
            (WIS, 8, self.fighter_point(caster))
        };
        self.f[caster].magic.field_last_turn[target] = self.areas.epoch;
        let saved = self.save_in_area(
            target,
            stat,
            self.spell_dc(caster),
            origin,
            self.magic_heightened_for(caster, target),
        );
        let damage = self.roll_many(tier, die);
        if target < 2 && !saved && moonbeam && self.f[target].magic.form {
            self.magic_end_wild_shape(target);
            self.f[caster].magic.field_blocks_shape[target] = true;
        }
        self.areas.pending_damage.push_back(SpellDamage {
            source: caster,
            emitter: None,
            amount: damage,
            kind: DamageType::Radiant,
            recipients: 1 << target,
            saved: if saved { 1 << target } else { 0 },
            checked: 0,
            rider: SpellRider::None,
            objects: None,
        });
    }

    pub(super) fn place_truth_zone(&mut self, caster: usize, center: Point) {
        self.f[caster].magic.zone_truth = Some((center, self.turns + 200));
        self.areas.truth_bound[caster] = 0;
        self.areas.truth_epoch[caster] = [-1; 4];
    }

    fn truth_zone_contains(&self, caster: usize, id: usize, at: Point) -> bool {
        self.f[caster]
            .magic
            .zone_truth
            .is_some_and(|(center, expiry)| {
                self.turns < expiry
                    && self.footprint(id).is_some_and(|mut footprint| {
                        footprint.anchor = at;
                        footprint.squares().any(|point| {
                            self.area_hits_at(center, point, Area::Sphere { radius: 3 })
                        })
                    })
            })
    }

    fn truth_zone_check(&mut self, caster: usize, id: usize) {
        let Some(at) = self.creature_point(id) else {
            return;
        };
        if self.areas.truth_bound[caster] & (1 << id) != 0
            || self.areas.truth_epoch[caster][id] == self.areas.epoch
            || !self.truth_zone_contains(caster, id, at)
        {
            return;
        }
        self.areas.truth_epoch[caster][id] = self.areas.epoch;
        let dc = self.spell_dc(caster);
        if !self.save_in_area(id, CHA, dc, at, false) {
            self.areas.truth_bound[caster] |= 1 << id;
        }
    }

    pub(super) fn magic_truth_bound(&self, id: usize) -> bool {
        self.creature_point(id).is_some_and(|at| {
            (0..2).any(|caster| {
                self.areas.truth_bound[caster] & (1 << id) != 0
                    && self.truth_zone_contains(caster, id, at)
            })
        })
    }

    pub(in crate::arena2) fn magic_area_moved(&mut self, id: usize, previous: Point) {
        self.alarm_moved(id, previous);
        for caster in [0, 1] {
            if !self.truth_zone_contains(caster, id, previous) {
                self.truth_zone_check(caster, id);
            }
            let was_inside = self.field_contains(caster, id, previous);
            self.field_transition(caster, id, was_inside);
        }
        if id < 2 && self.f[id].concentration == Some(Concentration::SpiritGuardians) {
            for target in CREATURES.into_iter().filter(|&seat| seat != id) {
                let was_inside =
                    self.area_contains_creature(previous, target, Area::Sphere { radius: 3 });
                self.field_transition(id, target, was_inside);
            }
        }
    }

    pub(in crate::arena2) fn magic_moved(&mut self, id: usize, previous: Point) {
        self.magic_area_moved(id, previous);
    }

    pub(in crate::arena2) fn magic_area_start(&mut self, id: usize) {
        self.areas.epoch += 1;
        if self.creature_point(id).is_none() {
            return;
        }
        for caster in 0..2 {
            self.truth_zone_check(caster, id);
            self.hadar_turn_damage(caster, id, true);
            if self.f[caster].concentration == Some(Concentration::EnsnaringStrike)
                && self.areas.afflicted[caster] & (1 << id) != 0
            {
                let amount = self.roll_many(self.f[caster].magic.field_level as i32, 6);
                self.areas.pending_damage.push_back(SpellDamage {
                    source: caster,
                    emitter: None,
                    amount,
                    kind: DamageType::Piercing,
                    recipients: 1 << id,
                    saved: 0,
                    checked: 0,
                    rider: SpellRider::None,
                    objects: None,
                });
            }
            if self.f[caster].concentration != Some(Concentration::StinkingCloud) {
                continue;
            }
            let inside = self.f[caster].magic.field.is_some_and(|center| {
                self.area_contains_creature(center, id, Area::Sphere { radius: 4 })
            });
            if inside {
                let dc = self.spell_dc(caster);
                let saved = if id < 2 {
                    self.save_poison_condition(id, dc)
                } else {
                    self.entity_save(id, CON, dc)
                };
                if !saved {
                    self.areas.poison_epoch[id] = self.areas.epoch;
                }
            }
        }
    }

    pub(super) fn cast_fear(&mut self, caster: usize, tier: usize, heightened: bool) {
        let origin = self.fighter_point(caster);
        let area = Area::Cone {
            length: 6,
            direction: self.f[caster].magic.direction,
        };
        let creatures = self.area_creature_mask(origin, area) & !(1 << caster);
        let immune = CREATURES
            .into_iter()
            .filter(|id| self.magic_heroism_immune(*id))
            .fold(0, |mask, id| mask | 1 << id);
        let eligible = creatures & !immune;
        self.concentrate(caster, Concentration::Fear, 10, None, tier);
        let disadvantage = if heightened {
            self.area_heightened_mask(caster)
        } else {
            0
        };
        self.areas.afflicted[caster] = eligible
            & !self.area_save_mask(eligible, WIS, self.spell_dc(caster), origin, disadvantage);
        for target in CREATURES {
            if self.areas.afflicted[caster] & (1 << target) != 0 {
                self.areas.fear_last_seen[target] =
                    self.entity_visible(target, caster).then_some(origin);
                if target < 2 {
                    self.magic_drop_gear(target);
                }
            }
        }
    }

    pub(in crate::arena2) fn fear_source(&self, target: usize) -> Option<usize> {
        (!self.magic_heroism_immune(target) && !self.arcane_emotions_immune(target))
            .then(|| {
                (0..2).find(|&caster| {
                    self.f[caster].concentration == Some(Concentration::Fear)
                        && self.areas.afflicted[caster] & (1 << target) != 0
                })
            })
            .flatten()
    }

    pub(in crate::arena2) fn fear_anchor(&mut self, target: usize, caster: usize) -> Option<Point> {
        if self.entity_visible(target, caster) {
            self.areas.fear_last_seen[target] = Some(self.fighter_point(caster));
        }
        self.areas.fear_last_seen[target]
    }

    fn fear_end(&mut self, target: usize) {
        let Some(caster) = self.fear_source(target) else {
            return;
        };
        let line_of_sight = self
            .footprint(target)
            .zip(self.footprint(caster))
            .is_some_and(|(a, b)| {
                a.squares().any(|from| {
                    b.squares()
                        .any(|to| self.map.los_at(from, to) && !self.magic_cloud_blocks(from, to))
                })
            });
        if !line_of_sight
            && self.save_in_area(
                target,
                WIS,
                self.spell_dc(caster),
                self.fighter_point(caster),
                self.magic_heightened_concentration_for(caster, target),
            )
        {
            self.areas.afflicted[caster] &= !(1 << target);
        }
    }

    pub(in crate::arena2) fn magic_area_end(&mut self, id: usize) {
        for caster in 0..2 {
            self.gust_save(caster, id);
            self.hadar_turn_damage(caster, id, false);
            self.magic_field_hit(caster, id);
        }
        self.areas.poison_epoch[id] = -1;
        self.fear_end(id);
        self.laugh_repeat_save(id, false);
        self.sleep_repeat_save(id);
    }

    pub(in crate::arena2) fn magic_area_hurt(&mut self, id: usize) {
        self.laugh_repeat_save(id, true);
        self.wake_creature(id);
    }

    fn wake_creature(&mut self, id: usize) {
        for caster in 0..2 {
            match self.f[caster].concentration {
                Some(Concentration::Sleep) => {
                    self.dispel_area_affliction(caster, id, Concentration::Sleep);
                }
                Some(Concentration::HypnoticPattern) => self.areas.afflicted[caster] &= !(1 << id),
                _ => {}
            }
        }
    }

    pub(in crate::arena2) fn magic_area_incapacitated(&self, id: usize) -> bool {
        (0..2).any(|caster| {
            matches!(self.f[caster].concentration, Some(Concentration::HypnoticPattern | Concentration::HideousLaughter | Concentration::Sleep))
                && self.areas.afflicted[caster] & (1 << id) != 0
        })
    }

    pub(in crate::arena2) fn magic_area_restrained(&self, id: usize) -> bool {
        self.divine_restrained(id) || self.magic_spell_area_restrained(id)
    }

    pub(super) fn magic_spell_area_restrained(&self, id: usize) -> bool {
        (0..2).any(|caster| {
            matches!(
                self.f[caster].concentration,
                Some(Concentration::Entangle | Concentration::EnsnaringStrike)
            ) && self.areas.afflicted[caster] & (1 << id) != 0
        })
    }

    pub(super) fn area_set_ensnared(&mut self, caster: usize, id: usize) {
        self.areas.afflicted[caster] = 1 << id;
    }

    pub(in crate::arena2) fn ensnaring_escape_targets(&self, helper: usize) -> u128 {
        (0..4)
            .filter(|&id| {
                (0..2).any(|caster| {
                    self.f[caster].concentration == Some(Concentration::EnsnaringStrike)
                        && self.areas.afflicted[caster] & (1 << id) != 0
                }) && self
                    .footprint(helper)
                    .zip(self.observed_footprint(helper % 2, id))
                    .is_some_and(|(origin, target)| origin.distance(target) <= 1)
            })
            .fold(0, |bits, id| bits | 1 << id)
    }

    pub(in crate::arena2) fn ensnaring_escape_target(&self, helper: usize) -> Option<usize> {
        let targets = self.ensnaring_escape_targets(helper);
        if targets & (1 << helper) != 0 {
            Some(helper)
        } else {
            (targets != 0).then(|| targets.trailing_zeros() as usize)
        }
    }

    pub(in crate::arena2) fn area_end_ensnared_creature(&mut self, id: usize) {
        for caster in 0..2 {
            if self.f[caster].concentration == Some(Concentration::EnsnaringStrike)
                && self.areas.afflicted[caster] & (1 << id) != 0
            {
                self.areas.afflicted[caster] &= !(1 << id);
                self.f[caster].concentration = None;
            }
        }
    }

    pub(in crate::arena2) fn area_escape_restraint_as(&mut self, helper: usize, target: usize) {
        use crate::arena2::origins::Skill;
        for caster in 0..2 {
            let concentration = self.f[caster].concentration;
            if !matches!(
                concentration,
                Some(Concentration::Entangle | Concentration::EnsnaringStrike)
            ) || self.areas.afflicted[caster] & (1 << target) == 0
                || helper != target && concentration != Some(Concentration::EnsnaringStrike)
            {
                continue;
            }
            let (modifier, advantage, disadvantage) = if helper < 2 {
                let (bonus, advantage) = self.martial_skill_mod(helper, Skill::Athletics);
                (
                    bonus,
                    advantage,
                    self.magic_ability_disadvantage(helper, 0)
                        || self.armor_disadvantages_ability(helper, 0)
                        || self.martial_poisoned(helper),
                )
            } else if let Some(entity) = self.entity(helper) {
                (
                    entity.kind.stats().save_modifiers[0] - 2 * entity.exhaustion,
                    false,
                    self.magic_hex_disadvantage(helper, 0)
                        || self.martial_poisoned(helper)
                        || self.magic_area_poisoned(helper)
                        || self.entity_frightened(helper),
                )
            } else {
                return;
            };
            let guidance = self.magic_guidance(helper, Skill::Athletics);
            if self.d20_for(helper, advantage, disadvantage) + modifier + guidance >= self.spell_dc(caster) {
                self.areas.afflicted[caster] &= !(1 << target);
                if concentration == Some(Concentration::EnsnaringStrike) {
                    self.f[caster].concentration = None;
                }
            }
            break;
        }
    }

    pub(super) fn area_release_restraint(&mut self, id: usize) {
        for caster in 0..2 {
            if self.f[caster].concentration == Some(Concentration::Entangle) {
                self.areas.afflicted[caster] &= !(1 << id);
            }
        }
    }

    pub(in crate::arena2) fn magic_area_poisoned(&self, id: usize) -> bool {
        self.areas.poison_epoch[id] == self.areas.epoch
    }

    pub(super) fn area_cure_poison(&mut self, id: usize) {
        self.areas.poison_epoch[id] = -1;
    }

    pub(in crate::arena2) fn magic_area_speed(&self, id: usize, speed: i32) -> i32 {
        if self.magic_area_speed_zero(id) || self.magic_area_restrained(id) {
            return 0;
        }
        if self.creature_point(id).is_some_and(|p| {
            (0..2).any(|caster| {
                self.f[caster].concentration == Some(Concentration::SpiritGuardians)
                    && self.field_contains(caster, id, p)
            })
        }) {
            speed / 2
        } else {
            speed
        }
    }

    /// Persistent effects a controller can reasonably route around. Do not
    /// use an unseen creature's true footprint when forecasting movement.
    pub(in crate::arena2) fn visible_movement_spell_hazard(
        &self,
        viewer: usize,
        subject: usize,
        point: Point,
    ) -> bool {
        if !self.f.iter().any(|f| {
            matches!(
                f.concentration,
                Some(
                    Concentration::Moonbeam
                        | Concentration::SpiritGuardians
                        | Concentration::HungerOfHadar | Concentration::StinkingCloud | Concentration::FogCloud
                )
            )
        }) {
            return false;
        }
        let mut footprint = self
            .observed_footprint(viewer, subject)
            .unwrap_or_else(|| crate::arena2::footprint::Footprint::point(point));
        footprint.anchor = point;
        let eye = self.fighter_point(viewer);
        let visible = |at| self.map.los_at(eye, at) && !self.magic_cloud_blocks(eye, at);

        (0..2).any(|caster| {
            let (origin, area, cloud) = match self.f[caster].concentration {
                Some(Concentration::Moonbeam) => (
                    self.f[caster].magic.field,
                    Area::Cylinder {
                        radius: 1,
                        height: 8,
                    },
                    false,
                ),
                Some(Concentration::SpiritGuardians)
                    if subject != caster && self.areas.exempt[caster] & (1 << subject) == 0 =>
                {
                    (
                        Some(self.fighter_point(caster)),
                        Area::Sphere { radius: 3 },
                        false,
                    )
                }
                Some(Concentration::FogCloud) => (
                    self.f[caster].magic.field,
                    Area::Sphere { radius: self.fog_radius(caster) },
                    true,
                ),
                Some(Concentration::StinkingCloud | Concentration::HungerOfHadar) => {
                    (self.f[caster].magic.field, Area::Sphere { radius: 4 }, true)
                }
                _ => return false,
            };
            origin.is_some_and(|origin| {
                let origin_known = viewer == caster || visible(origin);
                footprint.squares().any(|occupied| {
                    self.area_hits_at(origin, occupied, area)
                        && (origin_known
                            || if cloud {
                                // The obscurity itself is visible where the
                                // viewer's geometric line of sight first meets it.
                                self.map.los_at(eye, occupied)
                            } else {
                                visible(occupied)
                            })
                })
            })
        })
    }

    pub(in crate::arena2) fn hadar_dark_at(&self, point: Point) -> bool {
        self.f.iter().any(|f| f.concentration == Some(Concentration::HungerOfHadar)
            && f.magic.field.is_some_and(|center| self.area_hits_at(center, point, Area::Sphere { radius: 4 })))
    }

    pub(in crate::arena2) fn magic_obscures_at(&self, point: Point) -> bool {
        self.f.iter().enumerate().any(|(caster, f)| {
            let radius = match f.concentration {
                Some(Concentration::FogCloud) => self.fog_radius(caster),
                Some(Concentration::StinkingCloud) => 4,
                _ => return false,
            };
            f.magic.field.is_some_and(|origin| self.area_hits_at(origin, point, Area::Sphere { radius }))
        })
    }

    /// Heavy obscurement can occlude sight through the cloud even when
    /// neither endpoint is inside it. Sample at quarter-cell intervals.
    pub(in crate::arena2) fn magic_cloud_blocks(&self, from: Point, to: Point) -> bool {
        // Eyes inside Hadar are blinded. Outside it the sight line can cross darkness.
        if self.hadar_dark_at(from) { return true; }
        if !self
            .f
            .iter()
            .any(|f| matches!(f.concentration, Some(Concentration::StinkingCloud | Concentration::FogCloud)))
        {
            return false;
        }
        let steps = (from.dist(to) * 4).max(1);
        let interpolate = |start: i8, end: i8, step: i32| {
            ((i32::from(start) * (steps - step) + i32::from(end) * step + steps / 2) / steps) as i8
        };
        (0..=steps).any(|step| {
            self.magic_obscures_at(Point {
                cell: Pos {
                    x: interpolate(from.cell.x, to.cell.x, step),
                    y: interpolate(from.cell.y, to.cell.y, step),
                },
                altitude: interpolate(from.altitude, to.altitude, step),
            })
        })
    }

    pub(in crate::arena2) fn magic_difficult(&self, point: Point) -> bool {
        self.f.iter().any(|f| {
            f.magic.field.is_some_and(|origin| match f.concentration {
                Some(Concentration::Entangle) => self.area_hits_at(origin, point, Area::GroundSquare { half_side: 2 }),
                Some(Concentration::HungerOfHadar) => self.area_hits_at(origin, point, Area::Sphere { radius: 4 }),
                _ => false,
            })
        })
    }

    fn away_step(&self, origin: Point, from: Point) -> Option<Point> {
        let delta = (
            from.cell.x - origin.cell.x,
            from.cell.y - origin.cell.y,
            from.altitude - origin.altitude,
        );
        if delta == (0, 0, 0) {
            return None;
        }
        let to = Point {
            cell: Pos {
                x: from.cell.x + delta.0.signum(),
                y: from.cell.y + delta.1.signum(),
            },
            altitude: from.altitude + delta.2.signum(),
        };
        (self.map.can_occupy(to) && self.map.can_aim_point(from, to)).then_some(to)
    }

    pub(super) fn area_thunderwave_push(&mut self, origin: Point, victims: u8) {
        self.area_push_distance(origin, victims, 2);
    }

    pub(in crate::arena2) fn area_push_distance(&mut self, origin: Point, victims: u8, distance: i32) {
        for id in CREATURES.into_iter().filter(|&id| victims & (1 << id) != 0) {
            for _ in 0..distance {
                let Some(from) = self
                    .creature_point(id)
                    .filter(|_| id >= 2 || self.f[id].hp > 0)
                else {
                    break;
                };
                let Some(to) = self.away_step(origin, from) else {
                    break;
                };
                if id < 2 {
                    self.commit_forced_move(id, to);
                    if self.fighter_point(id) != to {
                        break;
                    }
                } else if !self.entity_force_move(id, to) {
                    break;
                }
            }
        }
    }

    fn area_thunderwave_objects(&mut self, origin: Point, area: Area) {
        use crate::arena2::objects::{END_OBJECTS, FIRST_OBJECT};
        for id in FIRST_OBJECT..END_OBJECTS {
            let Some(object) = self.object(id) else {
                continue;
            };
            if !self.area_hits_at(origin, object.point, area)
                || !area.contains_entire_object(origin, object.point, object.kind.area_extent())
            {
                continue;
            }
            let mut current = object.point;
            for _ in 0..2 {
                let Some(next) = self.away_step(origin, current) else {
                    break;
                };
                current = next;
            }
            if current != object.point {
                self.move_ground_object(id, current);
            }
        }
    }

    pub(in crate::arena2) fn area_request_damage(&mut self) -> bool {
        while let Some(mut damage) = self.areas.pending_damage.pop_front() {
            let observer = 1 - damage.source;
            if damage.amount > 0 && self.area_can_cut_packet(observer, damage) {
                self.areas.choice = Some(AreaChoice::Recurring(damage));
                self.request(observer, AREA_REACTION);
                return true;
            }
            damage.checked = u8::MAX;
            self.offer_spell_damage(damage);
        }
        false
    }

    pub(super) fn area_can_cut_packet(&self, bard: usize, damage: SpellDamage) -> bool {
        let source = damage.emitter.unwrap_or(damage.source);
        if source < 2 {
            return self.magic_can_cut_damage(bard, source);
        }
        let f = &self.f[bard];
        f.build.class == Class::Bard
            && f.build.variant == 0
            && f.build.level >= 3
            && f.resource > 0
            && self.can_react(bard)
            && self.entity_visible(bard, source)
            && self
                .combatant_distance(bard, source)
                .is_some_and(|distance| distance <= 12)
    }

    fn area_wake_targets(&self, actor: usize) -> u8 {
        let Some(origin) = self.creature_point(actor) else {
            return 0;
        };
        CREATURES
            .into_iter()
            .filter(|&target| {
                target != actor
                    && (0..2).any(|caster| {
                        matches!(self.f[caster].concentration, Some(Concentration::HypnoticPattern | Concentration::Sleep))
                            && self.areas.afflicted[caster] & (1 << target) != 0
                    })
                    && if target < 2 {
                        self.observed_pos(actor, target).is_some()
                    } else {
                        self.observed_entity(actor, target).is_some()
                    }
                    && self
                        .creature_point(target)
                        .is_some_and(|p| p.dist(origin) <= 1 && self.map.los_at(origin, p))
            })
            .fold(0, |bits, id| bits | 1 << id)
    }

    pub(in crate::arena2) fn area_action_mask(&self, actor: usize) -> u128 {
        if !self.action || self.incap(actor) || self.magic_quiet_turn(actor) {
            return 0;
        }
        let mut mask = 0;
        if self.area_wake_targets(actor) != 0 {
            mask |= Act::OtherCommon3.bit();
        }
        if self.f[actor].build.class != Class::Sorcerer && self.area_breath_source(actor).is_some()
        {
            mask |= Act::OtherCommon4.bit();
        }
        mask
    }

    pub(in crate::arena2) fn area_action_step(&mut self, actor: usize, choice: Act) -> bool {
        match choice {
            Act::OtherCommon3 => {
                let eligible = self.area_wake_targets(actor);
                self.areas.choice = Some(AreaChoice::Wake { eligible });
                self.request(actor, WAKE_TARGET);
            }
            Act::OtherCommon4 => {
                self.areas.choice = Some(AreaChoice::BreathDirection { id: actor });
                self.request(actor, COMPANION_DIRECTION);
            }
            _ => return false,
        }
        true
    }

    pub(in crate::arena2) fn area_action_label(&self, action: Act) -> Option<String> {
        match action {
            Act::OtherCommon3 => Some("Shake a creature awake".into()),
            Act::OtherCommon4 => Some("Exhale Dragon's Breath".into()),
            _ => None,
        }
    }

    pub(in crate::arena2) fn area_decision_mask(&self, _actor: usize, _phase: i32) -> u128 {
        match self.areas.choice {
            Some(
                AreaChoice::Guardians { eligible, .. } | AreaChoice::LandDamage { eligible, .. }
                    | AreaChoice::Sleep { eligible, .. },
            ) => (0..16)
                .filter(|&subset| subset & !eligible == 0)
                .fold(0, |bits, subset| bits | 1 << subset),
            Some(AreaChoice::LandHeal { eligible: 0, .. }) => Act::Pass.bit(),
            Some(AreaChoice::LandHeal { eligible, .. } | AreaChoice::Wake { eligible }) => {
                u128::from(eligible)
            }
            Some(AreaChoice::Recurring(_)) => Act::Defend.bit() | Act::Pass.bit(),
            Some(AreaChoice::BreathConsent { .. }) => Act::Defend.bit() | Act::Pass.bit(),
            Some(AreaChoice::BreathTouch { eligible, .. }) => u128::from(eligible),
            Some(AreaChoice::BreathDirection { .. }) => (1 << 26) - 1,
            None => 0,
        }
    }

    pub(in crate::arena2) fn area_decision_step(&mut self, actor: usize, _phase: i32, choice: Act) {
        match self.areas.choice.take().expect("area decision") {
            AreaChoice::Sleep { tier, center, heightened, .. } => {
                let targets = choice as u8;
                let saved = self.area_save_mask(targets, WIS, self.spell_dc(actor), center, heightened);
                let failed = targets & !saved;
                if failed != 0 && failed & (1 << actor) == 0 {
                    self.concentrate(actor, Concentration::Sleep, 20, None, tier);
                    self.f[actor].magic.concentration_heightened_target = (heightened != 0)
                        .then(|| heightened.trailing_zeros() as usize);
                    self.areas.afflicted[actor] = failed;
                    self.areas.asleep[actor] = 0;
                }
            }
            AreaChoice::Guardians { tier, .. } => {
                self.concentrate(actor, Concentration::SpiritGuardians, 100, None, tier);
                self.areas.exempt[actor] = choice as u8;
                for id in CREATURES {
                    self.magic_field_hit(actor, id);
                }
            }
            AreaChoice::LandDamage { center, eligible } => {
                self.areas.choice = Some(AreaChoice::LandHeal {
                    center,
                    eligible,
                    damage: choice as u8,
                });
                self.request(actor, LAND_HEAL);
            }
            AreaChoice::LandHeal { center, damage, .. } => {
                let victims = self.area_creature_mask(center, Area::Sphere { radius: 2 });
                let damage = damage & victims;
                let saves = self.area_save_mask(damage, CON, self.spell_dc(actor), center, 0);
                let amount = if damage != 0 { self.roll_many(2, 6) } else { 0 };
                let rider = if choice == Act::Pass || victims & (1 << choice as usize) == 0 {
                    SpellRider::None
                } else {
                    SpellRider::LandHeal(choice as usize, self.roll_many(2, 6))
                };
                self.offer_spell_damage(SpellDamage {
                    source: actor,
                    emitter: None,
                    amount,
                    kind: DamageType::Necrotic,
                    recipients: damage,
                    saved: saves,
                    checked: 0,
                    rider,
                    objects: None,
                });
            }
            AreaChoice::Wake { .. } => {
                self.action = false;
                self.wake_creature(choice as usize);
            }
            AreaChoice::BreathTouch { action, .. } => {
                let id = choice as usize;
                if id % 2 == actor {
                    self.f[actor].magic.cast_target = Some(id);
                    self.execute_magic_step(actor, action);
                } else {
                    self.areas.choice = Some(AreaChoice::BreathConsent {
                        caster: actor,
                        action,
                        id,
                    });
                    self.request(id % 2, BREATH_TOUCH);
                }
            }
            AreaChoice::BreathConsent { caster, action, id } => {
                if choice == Act::Defend {
                    self.f[caster].magic.cast_target = Some(id);
                    self.execute_magic_step(caster, action);
                } else {
                    self.f[caster].magic.cast_target = None;
                }
            }
            AreaChoice::BreathDirection { id } => {
                if id >= 2 {
                    self.set_entity_phase(Some(super::super::entities::EntityPhase::Turn(id - 2)));
                } else {
                    self.action = false;
                }
                self.area_exhale(id, choice as usize);
            }
            AreaChoice::Recurring(mut damage) => {
                if choice == Act::Defend {
                    damage.amount = (damage.amount - self.magic_cut_damage(actor)).max(0);
                }
                damage.checked = u8::MAX;
                self.offer_spell_damage(damage);
            }
        }
    }

    pub(in crate::arena2) fn area_decision_label(
        &self,
        _actor: usize,
        phase: i32,
        choice: Act,
    ) -> Option<String> {
        let label = |id: usize| {
            if id < 2 {
                format!("Seat {} {:?}", id + 1, self.f[id].build.class)
            } else {
                self.entity(id)
                    .map(|e| e.kind.name().into())
                    .unwrap_or_else(|| format!("Companion {}", id - 1))
            }
        };
        Some(match phase {
            SPIRIT_DESIGNATION | LAND_DAMAGE | SLEEP_TARGETS => {
                let names = CREATURES
                    .into_iter()
                    .filter(|&id| choice as u8 & (1 << id) != 0)
                    .map(label)
                    .collect::<Vec<_>>()
                    .join(", ");
                if phase == SPIRIT_DESIGNATION {
                    if names.is_empty() {
                        "Designate nobody".into()
                    } else {
                        format!("Exempt {names}")
                    }
                } else if phase == SLEEP_TARGETS {
                    if names.is_empty() { "Target nobody".into() } else { format!("Target {names}") }
                } else if names.is_empty() {
                    "Deal no damage".into()
                } else {
                    format!("Damage {names}")
                }
            }
            LAND_HEAL if choice == Act::Pass => "No creature can be healed".into(),
            LAND_HEAL => format!("Heal {}", label(choice as usize)),
            WAKE_TARGET => format!("Wake {}", label(choice as usize)),
            AREA_REACTION if choice == Act::Defend => "Cutting Words on area damage".into(),
            AREA_REACTION => "Pass on area damage".into(),
            BREATH_TOUCH if matches!(self.areas.choice, Some(AreaChoice::BreathConsent { .. })) => {
                if choice == Act::Defend {
                    "Accept Dragon's Breath".into()
                } else {
                    "Decline Dragon's Breath".into()
                }
            }
            BREATH_TOUCH => format!("Grant Dragon's Breath to {}", label(choice as usize)),
            COMPANION_DIRECTION => format!(
                "Exhale {}",
                super::super::targeting::DIRECTION_NAMES[choice as usize]
            ),
            _ => return None,
        })
    }

    pub fn area_choice_features(&self, viewer: usize) -> [f32; 7] {
        if viewer >= 2
            || self.actor() != Some(viewer)
            || !matches!(self.phase_id(), 92..=96 | 104..=105 | SLEEP_TARGETS)
        {
            return [0.0; 7];
        }
        let (eligible, amount) = match self.areas.choice {
            Some(
                AreaChoice::Guardians { eligible, .. }
                | AreaChoice::LandDamage { eligible, .. }
                | AreaChoice::LandHeal { eligible, .. }
                | AreaChoice::Wake { eligible }
                | AreaChoice::Sleep { eligible, .. },
            ) => (eligible, 0.0),
            Some(AreaChoice::Recurring(damage)) => (damage.recipients, damage.amount as f32 / 32.0),
            Some(AreaChoice::BreathTouch { eligible, .. }) => (eligible, 0.0),
            Some(AreaChoice::BreathDirection { id } | AreaChoice::BreathConsent { id, .. }) => {
                (1 << id, 0.0)
            }
            None => (0, 0.0),
        };
        [
            f32::from(u8::from(eligible & (1 << viewer) != 0)),
            f32::from(u8::from(eligible & (1 << (1 - viewer)) != 0)),
            f32::from(u8::from(eligible & (1 << (2 + viewer)) != 0)),
            f32::from(u8::from(eligible & (1 << (3 - viewer)) != 0)),
            amount,
            (self.phase_id() - 92) as f32 / 13.0,
            self.areas.epoch as f32 / 120.0,
        ]
    }
}

#[cfg(test)]
mod sleep_tests {
    use super::*;

    #[test]
    fn sleep_allows_movement_before_the_second_save_and_ends_on_damage() {
        let mut saw_unconscious = false;
        for seed in 0..48 {
            let mut duel = Battle::new_roster(seed, [31, 12]);
            duel.concentrate(0, Concentration::Sleep, 20, None, 3);
            duel.areas.afflicted[0] = 1 << 1;
            assert!(duel.magic_area_incapacitated(1));
            assert!(!duel.sleep_unconscious(1));
            assert!(duel.speed(1) > 0);
            duel.sleep_repeat_save(1);
            if duel.sleep_unconscious(1) {
                assert_eq!(duel.speed(1), 0);
                assert!(duel.f[1].prone);
                duel.damage(1, 1);
                assert!(!duel.sleep_unconscious(1));
                assert!(!duel.magic_area_incapacitated(1));
                saw_unconscious = true;
                break;
            }
        }
        assert!(saw_unconscious);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{
        class::Class,
        entities::{Entity, EntityKind, Spirit},
        equipment::Carried,
        geometry::Map,
        ACTS,
    };

    fn point(x: i8, y: i8) -> Point {
        Point::ground(Pos { x, y })
    }

    fn field_battle(seed: u64, first: Class, second: Class) -> Battle {
        let mut battle = Battle::new(seed, [first, second], [5, 5]);
        battle.owner = 0;
        battle.map = Map::new(0);
        battle.f[0].pos = point(0, 0).cell;
        battle.f[1].pos = point(3, 0).cell;
        battle
    }

    #[test]
    fn hadar_creates_difficult_space_and_damage_at_the_creatures_turn_boundaries() {
        let mut battle = field_battle(0xdea, Class::Warlock, Class::Fighter);
        let center = point(3, 0);
        battle.concentrate(0, Concentration::HungerOfHadar, 10, Some(center), 3);
        assert!(battle.magic_difficult(center));
        assert!(battle.hadar_dark_at(center));
        assert!(battle.magic_cloud_blocks(center, point(8, 0)));
        assert!(!battle.magic_cloud_blocks(point(8, 0), center));
        battle.magic_area_start(1);
        let packet = battle.areas.pending_damage.pop_front().unwrap();
        assert_eq!((packet.kind, packet.recipients), (DamageType::Cold, 2));
        assert!((2..=12).contains(&packet.amount));
        battle.f[1].stunned = true;
        battle.magic_area_end(1);
        assert_eq!(battle.areas.pending_damage.pop_front().unwrap().kind, DamageType::Acid);
        battle.f[0].concentration = None;
        assert!(!battle.magic_difficult(center));
        assert!(!battle.magic_cloud_blocks(point(0, 0), center));
    }

    #[test]
    fn faerie_fire_follows_the_creature_and_ends_with_concentration() {
        let mut battle = field_battle(5, Class::Warlock, Class::Fighter);
        battle.f[1].stunned = true;
        battle.f[0].magic.resolving_center = Some(battle.fighter_point(1));
        battle.cast_faerie_fire(0, 1, false);
        assert!(battle.faerie_outlined(1));
        battle.f[1].pos = Pos { x: 8, y: 6 };
        assert!(battle.projectile_glowing(1));
        battle.f[0].concentration = None;
        assert!(!battle.projectile_glowing(1));
    }

    #[test]
    fn mind_sliver_penalty_works_once_and_expires_after_casters_next_turn() {
        let mut battle = field_battle(21, Class::Sorcerer, Class::Cleric);
        battle.mark_mind_sliver(0, 1);
        battle.turns += 2;
        assert!((1..=4).contains(&battle.take_mind_sliver(1)));
        assert_eq!(battle.take_mind_sliver(1), 0);
        battle.mark_mind_sliver(0, 1);
        battle.turns += 3;
        assert_eq!(battle.take_mind_sliver(1), 0);
    }

    fn steed(battle: &mut Battle, side: usize, at: Point) {
        battle.fixture_companion(
            side,
            Entity::new(
                EntityKind::Steed {
                    spirit: Spirit::Celestial,
                    spell_level: 2,
                },
                side,
                at.cell,
                15,
            ),
        );
    }

    fn choose_center(battle: &mut Battle, action: Act, center: Point) {
        battle.step(action).unwrap();
        assert_eq!(battle.phase_id(), 5);
        battle.step(ACTS[center.cell.y as usize]).unwrap();
        battle.step(ACTS[center.cell.x as usize]).unwrap();
        if battle.target_altitude_request().is_some() {
            battle.step(ACTS[center.altitude as usize]).unwrap();
        }
    }

    #[test]
    fn thunderwave_pushes_failed_companion_and_unsecured_weapon() {
        let mut battle = field_battle(0, Class::Druid, Class::Fighter);
        steed(&mut battle, 1, point(1, 1));
        battle.f[1].gear.melee = Carried::Ground(point(1, 0).cell);
        battle.area_thunderwave_objects(
            point(0, 0),
            Area::ForwardCube {
                side: 3,
                direction: 2,
            },
        );
        assert_eq!(battle.object(5).unwrap().point, point(3, 0));
        battle.area_thunderwave_objects(
            point(0, 0),
            Area::ForwardCube {
                side: 3,
                direction: 2,
            },
        );
        // The object's center is inside, but its far edge is outside the cube.
        assert_eq!(battle.object(5).unwrap().point, point(3, 0));

        battle.f[1].pos = point(8, 6).cell;
        battle.area_thunderwave_push(point(0, 0), 1 << 3);
        // The diagonal push is stopped by the solid 2,2 wall.
        assert_eq!(battle.entity(3).unwrap().point(), point(1, 1));
        battle.entity_mut(3).unwrap().position = point(1, 0).cell;
        battle.area_thunderwave_push(point(0, 0), 1 << 3);
        assert_eq!(battle.entity(3).unwrap().point(), point(3, 0));
    }

    #[test]
    fn thunderwave_allows_shifting_the_caster_origin_along_its_rear_face() {
        for face in [0, 13] {
            let mut battle = field_battle(2, Class::Druid, Class::Fighter);
            battle.f[1].pos = point(1, 2).cell;
            let opening = battle.f[1].hp;
            battle.step(Act::Gear2).unwrap();
            battle.step(Act::Spell1).unwrap();
            battle.step(Act::E).unwrap();
            assert_eq!(battle.mask().count_ones(), 25);
            battle.step(ACTS[face]).unwrap();
            if battle.phase_id() == 38 {
                battle
                    .step(ACTS[battle.mask().trailing_zeros() as usize])
                    .unwrap();
            }
            assert_eq!(battle.f[1].hp < opening, face == 13);
        }
    }

    #[test]
    fn burning_hands_ignites_only_unattended_gear_in_its_selected_cone() {
        for direction in [Act::E, Act::W] {
            let mut battle = field_battle(0, Class::Wizard, Class::Ranger);
            battle.f[1].pos = point(8, 6).cell;
            battle.f[1].gear.bow = Carried::Ground(point(2, 0).cell);
            battle.step(Act::Spell2).unwrap();
            assert_eq!(battle.mask().count_ones(), 26);
            battle.step(direction).unwrap();
            if battle.phase_id() == 38 {
                battle
                    .step(ACTS[battle.mask().trailing_zeros() as usize])
                    .unwrap();
            }
            assert_eq!(battle.object(7).unwrap().burning, direction == Act::E);
            assert_eq!(battle.object(7).unwrap().hp, 10);
        }
    }

    #[test]
    fn moonbeam_ticks_a_companion_once_per_actual_turn_including_its_own() {
        let mut battle = field_battle(0, Class::Druid, Class::Fighter);
        steed(&mut battle, 1, point(4, 0));
        battle.concentrate(0, Concentration::Moonbeam, 10, Some(point(5, 1)), 2);
        let opening = battle.entity(3).unwrap().hp;
        battle.magic_field_hit(0, 3);
        assert!(!battle.area_request_damage());
        let first = battle.entity(3).unwrap().hp;
        assert!(first < opening);
        battle.magic_field_hit(0, 3);
        assert!(!battle.area_request_damage());
        assert_eq!(battle.entity(3).unwrap().hp, first);
        let before_turn = battle.turns;
        battle.magic_area_start(3);
        battle.magic_area_end(3);
        assert!(!battle.area_request_damage());
        assert_eq!(battle.turns, before_turn);
        assert!(battle.entity(3).is_none_or(|entity| entity.hp < first));
    }

    #[test]
    fn recurring_damage_offers_cutting_words_before_damage_resolves() {
        let mut initial = field_battle(1, Class::Druid, Class::Bard);
        initial.concentrate(0, Concentration::Moonbeam, 10, Some(point(3, 0)), 2);
        let hp = initial.f[1].hp;
        let charges = initial.f[1].resource;
        initial.magic_field_hit(0, 1);
        assert!(initial.area_request_damage());
        assert_eq!(initial.phase_id(), AREA_REACTION);
        assert_eq!(initial.actor(), Some(1));
        assert_eq!(initial.f[1].hp, hp);
        let mut pass = initial.clone();
        pass.step(Act::Pass).unwrap();
        initial.step(Act::Defend).unwrap();
        assert_eq!(initial.f[1].resource, charges - 1);
        assert_eq!(pass.f[1].resource, charges);
        assert!(initial.f[1].hp > pass.f[1].hp);
    }

    #[test]
    fn guardian_designation_exempts_ally_but_damages_and_slows_foe() {
        for actor in 0..2 {
            let foe = 1 - actor;
            let mut classes = [Class::Fighter; 2];
            classes[actor] = Class::Cleric;
            let mut battle = field_battle(7, classes[0], classes[1]);
            battle.owner = actor;
            battle.f[actor].pos = point(0, 0).cell;
            battle.f[foe].pos = point(3, 0).cell;
            let companion = 2 + actor;
            steed(&mut battle, actor, point(0, 1));
            let ally_hp = battle.entity(companion).unwrap().hp;
            let foe_hp = battle.f[foe].hp;

            battle.start_spirit_guardians(actor, 3);
            assert_eq!(battle.phase_id(), SPIRIT_DESIGNATION);
            assert_eq!(&battle.area_choice_features(actor)[..4], &[0., 1., 1., 0.]);
            assert_eq!(battle.area_choice_features(foe), [0.; 7]);
            let expected = ACTS[1].bit() | ACTS[2].bit() | ACTS[32].bit() | ACTS[38].bit();
            assert_eq!(battle.mask(), expected);
            battle.step(ACTS[2]).unwrap();

            assert_eq!(battle.entity(companion).unwrap().hp, ally_hp);
            assert!(battle.f[foe].hp < foe_hp);
            assert_eq!(battle.magic_area_speed(companion, 8), 8);
            assert_eq!(battle.magic_area_speed(foe, 6), 3);
        }
    }

    #[test]
    fn ready_shatter_uses_the_shared_move_trigger_and_spends_slot_only_at_preparation() {
        use crate::arena2::common::ReadyTrigger;

        let mut battle = field_battle(0, Class::Wizard, Class::Fighter);
        battle.f[1].pos = point(8, 0).cell;
        let [rank_one, rank_two, rank_three] = battle.f[0].slots;
        let victim_hp = battle.f[1].hp;
        battle.step(Act::Ready).unwrap();
        battle.step(ACTS[ReadyTrigger::Move as usize]).unwrap();
        battle.step(Act::Magic5).unwrap(); // Shatter.
        if battle.phase_id() == 107 {
            battle.step(Act::NE).unwrap(); // Rank 2.
        }
        let paid = [rank_one, rank_two - 1, rank_three];
        assert_eq!(battle.f[0].slots, paid);
        assert_eq!(battle.f[0].concentration, Some(Concentration::ReadiedSpell));
        battle.step(Act::End).unwrap();
        battle.step(Act::W).unwrap();
        assert_eq!((battle.phase_id(), battle.actor()), (9, Some(0)));
        battle.step(Act::Ready).unwrap();
        assert_eq!(battle.phase_id(), 109);
        battle.step(Act::N).unwrap();
        battle.step(ACTS[7]).unwrap();
        if battle.phase_id() == 111 {
            battle.step(Act::N).unwrap();
        }
        assert!(battle.f[1].hp < victim_hp);
        assert_eq!(battle.f[0].slots, paid);
        assert!(!battle.f[0].reaction);
        assert_eq!(battle.f[0].concentration, None);
    }

    #[test]
    fn public_spell_geometry_decouples_fixed_fields_from_hidden_casters() {
        let mut battle = field_battle(0, Class::Fighter, Class::Druid);
        battle.f[1].pos = point(3, 3).cell;
        assert!(!battle.can_see(0, 1));
        let moonbeam = Point {
            altitude: 3,
            ..point(1, 0)
        };
        battle.concentrate(1, Concentration::Moonbeam, 10, Some(moonbeam), 2);
        let seen = battle.magic_state_features(0, 1);
        assert_eq!(&seen[..3], &[0.0; 3]);
        assert_eq!(&seen[3..9], &[1.0, 0.125, 0.0, 0.5, 0.125, 1.0]);
        assert_eq!(&seen[9..], &[0.0; 15]);
        let summary = battle.magic_public_summary(0, 1);
        assert!(summary
            .contains("silver column of light at B1 (15 ft elevation), 5 ft radius, 40 ft high"));
        assert!(!summary.contains("Identified:"));

        battle.concentrate(1, Concentration::StinkingCloud, 10, Some(point(3, 0)), 3);
        assert!(!battle.can_see(0, 1));
        assert_eq!(battle.magic_state_features(0, 1)[7], 0.5);
        assert!(battle
            .magic_public_summary(0, 1)
            .contains("cloud at D1 (0 ft elevation), 20 ft radius"));

        battle.concentrate(1, Concentration::SpiritGuardians, 10, None, 3);
        assert_eq!(battle.magic_state_features(0, 1), [0.0; 24]);
        assert!(!battle
            .magic_public_summary(0, 1)
            .contains("orbiting shapes"));
    }

    #[test]
    fn movement_hazards_require_observable_fields_and_respect_exemptions() {
        let mut battle = field_battle(0, Class::Cleric, Class::Druid);
        battle.f[0].pos = point(0, 0).cell;
        battle.f[1].pos = point(3, 3).cell;
        let concealed = battle.fighter_point(1);
        assert!(!battle.map.los_at(battle.fighter_point(0), concealed));
        battle.concentrate(1, Concentration::Moonbeam, 10, Some(concealed), 2);
        assert!(!battle.visible_movement_spell_hazard(0, 0, concealed));
        assert!(battle.visible_movement_spell_hazard(1, 0, concealed));
        assert!(!battle.visible_movement_spell_hazard(1, 0, point(8, 6)));

        battle.concentrate(0, Concentration::SpiritGuardians, 10, None, 3);
        let near = point(1, 0);
        assert!(battle.visible_movement_spell_hazard(0, 1, near));
        assert!(!battle.visible_movement_spell_hazard(0, 0, near));
        battle.areas.exempt[0] = 1 << 1;
        assert!(!battle.visible_movement_spell_hazard(0, 1, near));

        battle.concentrate(1, Concentration::StinkingCloud, 10, Some(point(3, 0)), 3);
        assert!(battle.magic_cloud_blocks(battle.fighter_point(0), near));
        assert!(battle.visible_movement_spell_hazard(0, 0, near));
        assert!(!battle.visible_movement_spell_hazard(0, 0, point(8, 6)));
    }

    #[test]
    fn stinking_cloud_occludes_a_ray_even_if_both_endpoints_are_clear() {
        let mut battle = field_battle(0, Class::Warlock, Class::Fighter);
        battle.concentrate(0, Concentration::StinkingCloud, 10, Some(point(4, 1)), 3);
        let west = point(0, 0);
        let east = point(8, 0);
        assert!(!battle.magic_obscures_at(west));
        assert!(!battle.magic_obscures_at(east));
        assert!(battle.magic_cloud_blocks(west, east));
        let above = |p: Point| Point { altitude: 6, ..p };
        assert!(!battle.magic_cloud_blocks(above(west), above(east)));
    }

    #[test]
    fn stinking_cloud_poison_applies_only_on_start_and_ends_on_that_turn() {
        let mut saw_failure = false;
        for seed in 0..32 {
            let mut battle = field_battle(seed, Class::Warlock, Class::Fighter);
            steed(&mut battle, 1, point(1, 0));
            battle.concentrate(0, Concentration::StinkingCloud, 10, Some(point(0, 0)), 3);
            assert!(!battle.magic_area_poisoned(3));
            battle.magic_area_start(3);
            saw_failure |= battle.magic_area_poisoned(3);
            battle.magic_area_end(3);
            assert!(!battle.magic_area_poisoned(3));
        }
        assert!(saw_failure);
    }

    #[test]
    fn entangle_contains_the_ground_only_and_checks_the_entire_target_footprint() {
        let mut battle = field_battle(1, Class::Druid, Class::Fighter);
        steed(&mut battle, 1, point(1, 0));
        let center = point(4, 0);
        let footprint = battle.footprint(3).expect("large creature");
        assert_eq!(footprint.edge, 2);
        let area = Area::GroundSquare { half_side: 2 };
        assert!(!area.contains_at(center, footprint.anchor));
        assert!(battle.area_contains_creature(center, 3, area));
        battle.entity_mut(3).unwrap().altitude = 1;
        assert!(!battle.area_contains_creature(center, 3, area));
        battle.concentrate(0, Concentration::Entangle, 10, Some(center), 1);
        assert!(battle.magic_difficult(point(4, 0)));
        assert!(!battle.magic_difficult(Point {
            altitude: 1,
            ..center
        }));
    }

    #[test]
    fn breaking_pattern_for_one_target_does_not_wake_other_targets() {
        let mut battle = field_battle(0, Class::Bard, Class::Fighter);
        steed(&mut battle, 1, point(1, 0));
        battle.concentrate(0, Concentration::HypnoticPattern, 10, None, 3);
        battle.areas.afflicted[0] = (1 << 1) | (1 << 3);
        assert!(battle.magic_area_incapacitated(1));
        assert!(battle.magic_area_incapacitated(3));
        battle.magic_area_hurt(1);
        assert!(!battle.magic_area_incapacitated(1));
        assert!(battle.magic_area_incapacitated(3));
        battle.step(Act::OtherCommon3).unwrap();
        assert_eq!(battle.phase_id(), WAKE_TARGET);
        battle.step(ACTS[3]).unwrap();
        assert!(!battle.magic_area_incapacitated(3));
        assert!(!battle.action);
    }

    #[test]
    fn lands_aid_selects_damage_subset_and_a_distinct_healing_recipient() {
        let mut battle = field_battle(3, Class::Druid, Class::Fighter);
        battle.f[1].pos = point(1, 0).cell;
        steed(&mut battle, 0, point(0, 1));
        battle.entity_mut(2).unwrap().hp = 5;
        let initial = battle.f.each_ref().map(|f| f.hp);
        choose_center(&mut battle, Act::Special3, point(1, 1));
        assert_eq!(battle.phase_id(), LAND_DAMAGE);
        battle.step(ACTS[1]).unwrap();
        assert_eq!(battle.phase_id(), LAND_HEAL);
        battle.step(ACTS[2]).unwrap();
        assert_eq!(battle.f[0].hp, initial[0]);
        assert!(battle.f[1].hp < initial[1]);
        assert!(battle.entity(2).unwrap().hp > 5);
    }

    #[test]
    fn dragons_breath_can_be_accepted_by_a_companion_and_uses_its_cone_origin() {
        let mut battle = field_battle(9, Class::Sorcerer, Class::Fighter);
        steed(&mut battle, 1, point(1, 0));
        let opening = battle.f[0].hp;
        battle.step(Act::Magic9).unwrap();
        battle.step(ACTS[2]).unwrap(); // Fire.
        assert_eq!(battle.phase_id(), BREATH_TOUCH);
        assert_ne!(battle.mask() & (1 << 3), 0);
        battle.step(ACTS[3]).unwrap();
        assert_eq!(battle.actor(), Some(1));
        assert_eq!(battle.mask(), Act::Defend.bit() | Act::Pass.bit());
        battle.step(Act::Defend).unwrap();
        if battle.phase_id() == 38 {
            battle
                .step(ACTS[battle.mask().trailing_zeros() as usize])
                .unwrap();
        }
        assert!(battle.companion_can_exhale(3));
        assert!(!battle.companion_can_exhale(2));
        assert!(battle.area_breath_source(0).is_none());
        battle.begin_companion_exhale(3);
        battle.step(ACTS[6]).unwrap(); // West, towards the original caster.
        assert!(battle.f[0].hp < opening);
    }

    #[test]
    fn fear_affects_companion_footprints_dashes_and_ends_per_creature() {
        for seed in 0..64 {
            let mut battle = field_battle(seed, Class::Sorcerer, Class::Fighter);
            battle.f[1].pos = point(3, 0).cell;
            steed(&mut battle, 1, point(5, 0));
            battle.f[0].magic.direction = Act::E as usize;
            battle.cast_fear(0, 3, false);
            if battle.fear_source(1) != Some(0) || battle.fear_source(3) != Some(0) {
                continue;
            }
            assert_ne!(battle.f[1].gear.melee, Carried::Held);
            battle.step(Act::End).unwrap();
            assert_eq!(battle.phase_id(), 73);
            assert!(!battle.entity(3).unwrap().turn.action);
            assert!(battle.entity(3).unwrap().turn.dash > 0);
            for _ in 0..48 {
                if battle.owner == 1 && battle.phase_id() == 73 {
                    break;
                }
                let mask = battle.mask();
                assert_ne!(mask, 0);
                let next = if mask & Act::End.bit() != 0 {
                    Act::End
                } else if mask & Act::Pass.bit() != 0 {
                    Act::Pass
                } else {
                    ACTS[mask.trailing_zeros() as usize]
                };
                battle.step(next).unwrap();
            }
            assert_eq!(battle.owner, 1);
            assert_eq!(battle.phase_id(), 73);
            assert!(!battle.action);
            assert!(battle.dash > 0);
            let elevated = point(3, 3);
            let source = battle.fighter_point(0);
            if !battle.map.los_at(source, elevated) {
                battle.f[1].pos = elevated.cell;
                for _ in 0..32 {
                    battle.fear_end(1);
                    if battle.fear_source(1).is_none() {
                        break;
                    }
                }
                assert!(battle.fear_source(1).is_none());
                assert_eq!(battle.fear_source(3), Some(0));
                return;
            }
        }
        panic!("fixture needs two failed initial saves and blocked line of sight");
    }

    #[test]
    fn fear_uses_vertical_flight_for_a_fighter_and_independent_familiar() {
        use crate::arena2::entities::FamiliarForm;

        for subject in [1, 3] {
            let mut battle = field_battle(0, Class::Sorcerer, Class::Sorcerer);
            battle.f[0].pos = point(4, 3).cell;
            battle.f[1].pos = if subject == 1 {
                point(0, 0)
            } else {
                point(8, 6)
            }
            .cell;
            if subject == 1 {
                battle.owner = 1;
                battle.concentrate(1, Concentration::Fly, 100, None, 3);
            } else {
                battle.fixture_companion(
                    1,
                    Entity::new(
                        EntityKind::Familiar {
                            form: FamiliarForm::Owl,
                            spirit: Spirit::Fey,
                        },
                        1,
                        point(0, 0).cell,
                        5,
                    ),
                );
            }
            battle.concentrate(0, Concentration::Fear, 10, None, 3);
            battle.areas.afflicted[0] = 1 << subject;
            battle.areas.fear_last_seen[subject] = Some(battle.fighter_point(0));
            assert!(battle.start_fear_travel(subject));
            let ascend = ACTS[126];
            assert_ne!(battle.mask() & ascend.bit(), 0);
            battle.step(ascend).unwrap();
            assert_eq!(battle.creature_point(subject).unwrap().altitude, 1);
        }
    }

    #[test]
    fn spiritual_weapon_uses_companion_footprint_and_persists_its_paid_rank() {
        for seed in 0..64 {
            let mut battle = field_battle(seed, Class::Cleric, Class::Fighter);
            battle.f[1].pos = point(8, 6).cell;
            steed(&mut battle, 1, point(2, 0));
            let initial = point(5, 0);
            battle.concentrate(0, Concentration::SpiritualWeapon, 10, Some(initial), 3);
            let slots = battle.f[0].slots;
            let before = battle.entity(3).unwrap().hp;
            choose_center(&mut battle, Act::Special3, point(4, 1));
            assert_eq!(battle.magic_field(0), Some(point(4, 1)));
            assert!(!battle.bonus);
            assert_eq!(battle.f[0].slots, slots);
            assert_eq!(battle.f[0].magic.field_level, 3);
            assert_ne!(battle.mask() & ACTS[3].bit(), 0);
            assert_ne!(battle.mask() & Act::Pass.bit(), 0);
            battle.step(ACTS[3]).unwrap();
            if battle.entity(3).is_none_or(|steed| steed.hp < before) {
                return;
            }
        }
        panic!("fixture must land a Spiritual Weapon attack");
    }

    #[test]
    fn spiritual_weapon_guesses_without_revealing_unseen_positions() {
        for seed in 0..64 {
            let mut occupied = field_battle(seed, Class::Cleric, Class::Warlock);
            let center = point(2, 0);
            occupied.f[1].pos = center.cell;
            occupied.f[1].hidden_dc = Some(99);
            occupied.concentrate(1, Concentration::StinkingCloud, 10, Some(center), 3);
            occupied.concentrate(0, Concentration::SpiritualWeapon, 10, Some(center), 2);
            let mut vacant = occupied.clone();
            vacant.f[1].pos = point(3, 0).cell;
            for battle in [&mut occupied, &mut vacant] {
                assert!(battle.observed_pos(0, 1).is_none());
                battle.offer_spiritual_weapon_attack(0);
            }
            assert_eq!(occupied.mask(), vacant.mask());
            assert_eq!(occupied.phase_id(), vacant.phase_id());
            let guess = ACTS[29];
            assert_ne!(occupied.mask() & guess.bit(), 0);
            let hp = vacant.f[1].hp;
            vacant.step(guess).unwrap();
            occupied.step(guess).unwrap();
            assert_eq!(vacant.f[1].hp, hp);
            if occupied.f[1].hp < hp {
                return;
            }
        }
        panic!("one guess must land on the actual target");
    }

    #[test]
    fn truth_zone_checks_entry_and_companion_turns_but_binds_only_inside() {
        let mut battle = field_battle(0, Class::Paladin, Class::Fighter);
        battle.f[1].pos = point(3, 0).cell;
        steed(&mut battle, 1, point(5, 0));
        battle.place_truth_zone(0, point(0, 0));
        assert_eq!(battle.areas.truth_epoch[0], [-1; 4]);
        battle.magic_area_moved(1, point(4, 0));
        let first_epoch = battle.areas.epoch;
        assert_eq!(battle.areas.truth_epoch[0][1], first_epoch);
        battle.magic_area_moved(1, point(4, 0));
        assert_eq!(battle.areas.truth_epoch[0][1], first_epoch);
        battle.place_truth_zone(0, point(5, 0));
        battle.magic_area_start(3);
        assert_eq!(battle.areas.truth_epoch[0][3], first_epoch + 1);
        battle.areas.truth_bound[0] |= 1 << 3;
        assert!(battle.magic_truth_bound(3));
        battle.turns += 200;
        assert!(!battle.magic_truth_bound(3));
    }

    #[test]
    fn spider_shape_can_climb_from_the_perimeter_onto_the_ceiling() {
        let mut battle = field_battle(0, Class::Druid, Class::Fighter);
        battle.f[0].pos = point(0, 1).cell;
        let uses = battle.f[0].resource;
        battle.step(Act::Special1).unwrap();
        battle.step(ACTS[3]).unwrap(); // Spider.
        battle.step(ACTS[0]).unwrap(); // Merge equipment.
        assert_eq!(battle.f[0].resource, uses - 1);
        assert!(battle.shape_spider_climb(0));
        assert_eq!(battle.body_score(0, 0), 2);
        assert_eq!(battle.body_score(0, 1), 14);
        assert_eq!(battle.speed(0), 4);
        assert_eq!(battle.magic_climb_speed(0), 4);
        for height in 1..=6 {
            if height == 5 {
                assert_eq!(battle.moved, 4);
                battle.step(Act::End).unwrap();
                battle.step(Act::End).unwrap();
            }
            battle.step(Act::Climb).unwrap();
            battle.step(Act::N).unwrap();
            assert_eq!(battle.fighter_point(0).altitude, height);
        }
        battle.step(Act::E).unwrap();
        assert_eq!(battle.moved, 3);
        assert_eq!(
            battle.fighter_point(0),
            Point {
                altitude: 6,
                ..point(1, 1)
            }
        );
    }
}

impl Battle {
    pub(in crate::arena2) fn dispellable_area_affliction(
        &self,
        source: usize,
        recipient: usize,
        spell: Concentration,
    ) -> bool {
        match spell {
            Concentration::HypnoticPattern
            | Concentration::Sleep
            | Concentration::HideousLaughter
            | Concentration::Fear
            | Concentration::EnsnaringStrike => {
                self.areas.afflicted[source] & (1 << recipient) != 0
            }
            Concentration::DragonBreath => self.areas.breath_target[source] == Some(recipient),
            _ => false,
        }
    }

    pub(in crate::arena2) fn dispel_area_affliction(
        &mut self,
        source: usize,
        recipient: usize,
        spell: Concentration,
    ) {
        if spell == Concentration::DragonBreath {
            self.areas.breath_target[source] = None;
            self.f[source].concentration = None;
        } else {
            self.areas.afflicted[source] &= !(1 << recipient);
            if self.areas.afflicted[source] == 0 {
                self.f[source].concentration = None;
            }
        }
    }
}

impl Battle {
    pub(super) fn fog_radius(&self, caster: usize) -> i32 {
        4 * self.f[caster].magic.field_level as i32
    }

    pub(super) fn start_gust_of_wind(&mut self, caster: usize, tier: usize) {
        self.concentrate(caster, Concentration::GustOfWind, 10, None, tier);
        self.f[caster].magic.gust_direction = self.f[caster].magic.direction;
        self.f[caster].magic.gust_cast_turn = self.turns;
        self.gust_clear_clouds(caster);
        for target in CREATURES {
            self.gust_save(caster, target);
        }
    }

    fn gust_area(&self, caster: usize) -> Area {
        Area::Line { length: 12, width: 2, direction: self.f[caster].magic.gust_direction }
    }

    fn gust_save(&mut self, caster: usize, target: usize) {
        if self.f[caster].concentration != Some(Concentration::GustOfWind) {
            return;
        }
        let origin = self.fighter_point(caster);
        if !self.area_contains_creature(origin, target, self.gust_area(caster))
            || self.save_in_area(target, 0, self.spell_dc(caster), origin, false)
        {
            return;
        }
        let axis = super::super::targeting::aim_vector(self.f[caster].magic.gust_direction)
            .expect("legal gust direction");
        let [dx, dy, dz] = axis.map(|v| v as i8);
        for _ in 0..3 {
            let Some(from) = self.creature_point(target) else { break };
            let to = Point {
                cell: Pos { x: from.cell.x + dx, y: from.cell.y + dy },
                altitude: from.altitude + dz,
            };
            if target < 2 {
                self.commit_forced_move(target, to);
                if self.fighter_point(target) != to {
                    break;
                }
            } else if !self.entity_force_move(target, to) {
                break;
            }
        }
    }

    pub(super) fn gust_clear_clouds(&mut self, caster: usize) {
        let eye = self.fighter_point(caster);
        let gust = self.gust_area(caster);
        for source in 0..2 {
            let f = &self.f[source];
            let radius = match f.concentration {
                Some(Concentration::FogCloud) => self.fog_radius(source),
                Some(Concentration::StinkingCloud) => 4,
                _ => continue,
            };
            let Some(center) = f.magic.field else { continue };
            let cloud = Area::Sphere { radius };
            use super::super::geometry::{HEIGHT, MAX_ELEVATION, WIDTH};
            let intersects = (0..=MAX_ELEVATION).any(|altitude| {
                (0..HEIGHT).any(|y| (0..WIDTH).any(|x| {
                    let point = Point { cell: Pos { x, y }, altitude };
                    self.map.can_occupy(point)
                        && self.area_hits_at(center, point, cloud)
                        && self.area_hits_at(eye, point, gust)
                }))
            });
            if intersects {
                self.f[source].concentration = None;
            }
        }
    }

    pub(in crate::arena2) fn gust_hinders(&self, from: Point, to: Point) -> bool {
        (0..2).any(|source| {
            let eye = self.fighter_point(source);
            self.f[source].concentration == Some(Concentration::GustOfWind)
                && eye.dist(to) < eye.dist(from)
                && self.area_hits_at(eye, from, self.gust_area(source))
        })
    }
}
