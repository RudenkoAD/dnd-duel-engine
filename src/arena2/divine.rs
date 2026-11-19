//! Level 3–5 domain, circle, and oath abilities.
use super::*;
use crate::arena2::{geometry, temp_hp::TempSource};

const MENU: i32 = 160;
const TARGET: i32 = 161;
const PLACE: i32 = 162;
const PUSH: i32 = 163;
const GUIDED: i32 = 164;
const SMITE_TARGET: i32 = 165;
const SMITE_AMOUNT: i32 = 166;
const CHALICE: i32 = 167;
const PRIEST_MODE: i32 = 168;
const NATURE_TARGETS: i32 = 169;
const COMMIT: Act = Act::Other7;
const PRIEST_MODES: [Act; 6] = [
    Act::Unarmed,
    Act::Melee,
    Act::Ranged,
    Act::Grapple,
    Act::ShovePush,
    Act::ShoveProne,
];
pub(super) const ACTION: Act = Act::Martial8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Power {
    Priest,
    Blessing,
    Duplicate,
    MoveDuplicate,
    Archer,
    Chalice,
    Dragon,
    FireArrow,
    Sea,
    Spray,
    DismissSea,
    Nature,
    Athlete,
    FreeBolt,
    CastViaDuplicate,
    EndStars,
    DismissDuplicate,
    SwapDuplicate,
}

const POWERS: [Power; 17] = [
    Power::Priest,
    Power::Blessing,
    Power::Duplicate,
    Power::MoveDuplicate,
    Power::Archer,
    Power::Chalice,
    Power::Dragon,
    Power::FireArrow,
    Power::Sea,
    Power::Spray,
    Power::DismissSea,
    Power::Nature,
    Power::Athlete,
    Power::FreeBolt,
    Power::CastViaDuplicate,
    Power::EndStars,
    Power::DismissDuplicate,
];

#[derive(Clone, Default, Debug)]
pub(in crate::arena2) struct DivineState {
    selection: Option<Power>,
    readied_magic: bool,
    priest_spent: i32,
    duplicate: Option<(Point, i32)>,
    duplicate_cell: Option<Pos>,
    cast_via_duplicate: bool,
    nature_targets: u8,
    priest_mode: usize,
    free_bolts_spent: i32,
    trickster: Option<usize>,
    blessing_pending: Option<usize>,
    stars: Option<(Power, i32)>,
    sea_until: i32,
    athlete_until: i32,
    restrained_until: [i32; 4],
    guided: Option<(Hit, Attack, u8)>,
    spray_target: Option<usize>,
    smite_hp: i32,
    smite_target: usize,
    smite_used: u8,
    chalice_hp: i32,
}

impl Battle {
    fn divine_ready(&self, i: usize, class: Class, variant: u8) -> bool {
        let build = self.f[i].build;
        build.class == class && build.variant == variant && build.level >= 3
    }

    fn divine_targets(&self, i: usize, range: i32, own: bool) -> u128 {
        (0..4)
            .filter(|&id| {
                (id != i || own)
                    && (id == i
                        || self
                            .combatant_distance(i, id)
                            .is_some_and(|distance| distance <= range))
                    && self.rope_same_space(i, id)
                    && (id == i || self.entity_visible(i, id))
            })
            .fold(0, |mask, id| mask | ACTS[id].bit())
    }

    fn divine_harmful_targets(&self, i: usize, range: i32, own: bool) -> u128 {
        (0..4)
            .filter(|&id| self.suggestion_charmed_by(i, id))
            .fold(self.divine_targets(i, range, own), |mask, id| {
                mask & !ACTS[id].bit()
            })
    }

    fn divine_cells(&self, i: usize, power: Power) -> u128 {
        (0..geometry::WIDTH * geometry::HEIGHT)
            .filter(|&index| {
                self.divine_duplicate_heights(i, power, Self::divine_point(index as usize).cell)
                    != 0
            })
            .fold(0, |mask, index| mask | ACTS[index as usize].bit())
    }

    fn divine_duplicate_heights(&self, i: usize, power: Power, cell: Pos) -> u128 {
        if !self.present_on_arena_plane(i) {
            return 0;
        }
        let origin = self.fighter_point(i);
        let from = if power == Power::Duplicate {
            origin
        } else {
            self.divine_duplicate(i).unwrap_or(origin)
        };
        (0..=geometry::MAX_ELEVATION)
            .filter(|&altitude| {
                let point = Point { cell, altitude };
                from.dist(point) <= 6
                    && origin.dist(point) <= 24
                    && self.map.can_occupy(point)
                    && self.map.los_at(origin, point)
                    && !self.magic_cloud_blocks(origin, point)
                    && (0..4).all(|id| {
                        self.observed_footprint(i, id)
                            .is_none_or(|body| !body.squares().any(|square| square == point))
                    })
            })
            .fold(0, |mask, altitude| mask | ACTS[altitude as usize].bit())
    }

    fn divine_point(index: usize) -> Point {
        let width = geometry::WIDTH as usize;
        Point::ground(Pos {
            x: (index % width) as i8,
            y: (index / width) as i8,
        })
    }

    fn divine_priest_targets(&self, i: usize, action: Act) -> u128 {
        if action == Act::Melee && !self.gear_allows_weapon(i, false)
            || action == Act::Ranged && (self.f[i].arrows == 0 || !self.gear_allows_weapon(i, true))
        {
            return 0;
        }
        self.creature_targets(action) & 0b1111
    }

    fn divine_priest_modes(&self, i: usize) -> u128 {
        PRIEST_MODES
            .into_iter()
            .enumerate()
            .filter(|&(_, action)| self.divine_priest_targets(i, action) != 0)
            .fold(0, |mask, (index, _)| mask | ACTS[index].bit())
    }

    fn divine_menu(&self, i: usize) -> u128 {
        let f = &self.f[i];
        let state = &f.divine;
        let available = |power| match power {
            Power::Priest => {
                self.divine_ready(i, Class::Cleric, 1)
                    && self.bonus
                    && state.priest_spent < f.build.casting.max(1)
                    && self.divine_priest_modes(i) != 0
            }
            Power::Blessing => {
                self.divine_ready(i, Class::Cleric, 2)
                    && self.action
                    && self.divine_targets(i, 6, true) != 0
            }
            Power::Duplicate => {
                self.divine_ready(i, Class::Cleric, 2)
                    && self.bonus
                    && f.resource > 0
                    && self.divine_cells(i, power) != 0
            }
            Power::MoveDuplicate => {
                self.divine_ready(i, Class::Cleric, 2)
                    && self.bonus
                    && self.divine_duplicate(i).is_some()
                    && self.divine_cells(i, power) != 0
            }
            Power::Archer | Power::Chalice | Power::Dragon => {
                self.divine_ready(i, Class::Druid, 1) && self.bonus && f.resource > 0
            }
            Power::FireArrow => {
                self.divine_stars(i, Power::Archer)
                    && self.bonus
                    && self.divine_harmful_targets(i, 12, false) != 0
            }
            Power::Sea => self.divine_ready(i, Class::Druid, 2) && self.bonus && f.resource > 0,
            Power::Spray => {
                state.sea_until > self.turns
                    && self.bonus
                    && self.divine_harmful_targets(i, 1, false) != 0
            }
            Power::DismissSea => state.sea_until > self.turns,
            Power::Nature => {
                self.divine_ready(i, Class::Paladin, 1)
                    && self.action
                    && f.secondary > 0
                    && self.divine_harmful_targets(i, 3, true) != 0
            }
            Power::Athlete => {
                self.divine_ready(i, Class::Paladin, 2)
                    && self.bonus
                    && f.secondary > 0
                    && !self.divine_athlete(i)
            }
            Power::FreeBolt => {
                self.divine_ready(i, Class::Druid, 1)
                    && self.action
                    && state.free_bolts_spent < f.build.casting.max(1)
                    && self.divine_free_bolt_ready(i)
            }
            Power::CastViaDuplicate | Power::DismissDuplicate => self.divine_duplicate(i).is_some(),
            Power::SwapDuplicate => false,
            Power::EndStars => state.stars.is_some_and(|(_, until)| until > self.turns),
        };
        POWERS
            .into_iter()
            .enumerate()
            .filter(|&(_, power)| available(power))
            .fold(0, |mask, (index, _)| mask | ACTS[index].bit())
    }

    pub(super) fn divine_mask(&self, i: usize) -> u128 {
        if self.divine_menu(i) != 0 {
            ACTION.bit()
        } else {
            0
        }
    }

    pub(super) fn divine_main_step(&mut self, i: usize, action: Act) -> bool {
        if action != ACTION || self.divine_menu(i) == 0 {
            return false;
        }
        self.f[i].divine.readied_magic = false;
        self.request(i, MENU);
        true
    }

    fn divine_readied_power(&self, i: usize) -> Option<Power> {
        if self.divine_ready(i, Class::Cleric, 2) {
            Some(Power::Blessing)
        } else if self.divine_ready(i, Class::Paladin, 1) && self.f[i].secondary > 0 {
            Some(Power::Nature)
        } else {
            None
        }
    }

    pub(super) fn divine_ready_mask(&self, i: usize) -> u128 {
        if self.divine_readied_power(i).is_some() {
            ACTION.bit()
        } else {
            0
        }
    }

    pub(super) fn divine_ready_valid(&self, i: usize) -> bool {
        match self.divine_readied_power(i) {
            Some(Power::Blessing) => self.divine_targets(i, 6, true) != 0,
            Some(Power::Nature) => self.divine_harmful_targets(i, 3, true) != 0,
            _ => false,
        }
    }

    pub(super) fn divine_ready_release(&mut self, i: usize) {
        let power = self
            .divine_readied_power(i)
            .expect("readied divine feature");
        self.f[i].divine.readied_magic = true;
        self.divine_choose(i, power);
    }

    pub(super) fn divine_ready_label(&self, i: usize) -> Option<String> {
        Some(
            match self.divine_readied_power(i)? {
                Power::Blessing => "Blessing of the Trickster",
                Power::Nature => "Nature's Wrath",
                _ => unreachable!("readied divine feature"),
            }
            .into(),
        )
    }

    fn divine_spend_magic_action(&mut self, i: usize) {
        if !std::mem::take(&mut self.f[i].divine.readied_magic) {
            self.action = false;
        }
    }

    pub(super) fn divine_main_label(&self, i: usize, action: Act) -> Option<String> {
        (action == ACTION && self.divine_menu(i) != 0).then(|| {
            match (self.f[i].build.class, self.f[i].build.variant) {
                (Class::Cleric, 1) => "War Priest",
                (Class::Cleric, 2) => "Trickery Domain features",
                (Class::Druid, 1) => "Starry Form",
                (Class::Druid, 2) => "Wrath of the Sea",
                (Class::Paladin, 1) => "Nature's Wrath",
                (Class::Paladin, 2) => "Peerless Athlete",
                _ => unreachable!(),
            }
            .to_owned()
        })
    }

    pub(super) fn divine_decision_mask(&self, i: usize, phase: i32) -> u128 {
        let state = &self.f[i].divine;
        let pass = Act::Pass.bit();
        match phase {
            MENU => self.divine_menu(i) | Act::Other8.bit(),
            TARGET if self.f[1 - i].divine.blessing_pending.is_some() => Act::Defend.bit() | pass,
            TARGET => {
                let power = state.selection.expect("divine target power");
                if power == Power::Priest {
                    return self.divine_priest_targets(i, PRIEST_MODES[state.priest_mode]) | pass;
                }
                let (range, own) = match power {
                    Power::Blessing => (6, true),
                    Power::Nature => (3, false),
                    Power::Archer | Power::FireArrow => (12, false),
                    _ => (1, false),
                };
                let targets = if power == Power::Blessing {
                    self.divine_targets(i, range, own)
                } else {
                    self.divine_harmful_targets(i, range, own)
                };
                targets | pass
            }
            PLACE if state.selection == Some(Power::SwapDuplicate) => Act::Defend.bit() | pass,
            PLACE => {
                let power = state.selection.expect("duplicity placement");
                state.duplicate_cell.map_or_else(
                    || self.divine_cells(i, power),
                    |cell| self.divine_duplicate_heights(i, power, cell),
                ) | Act::Other8.bit()
            }
            PUSH => 0b1111,
            PRIEST_MODE => self.divine_priest_modes(i) | pass,
            NATURE_TARGETS => {
                self.divine_harmful_targets(i, 3, true) & !u128::from(state.nature_targets)
                    | if state.nature_targets != 0 {
                        COMMIT.bit()
                    } else {
                        0
                    }
                    | pass
            }
            GUIDED => Act::Defend.bit() | pass,
            SMITE_TARGET => self.divine_targets(i, 6, true) & !u128::from(state.smite_used) | pass,
            SMITE_AMOUNT => (1_u128 << state.smite_hp) - 1,
            CHALICE => self.divine_targets(i, 6, true) | pass,
            _ => 0,
        }
    }

    pub(super) fn divine_decision_label(
        &self,
        i: usize,
        phase: i32,
        action: Act,
    ) -> Option<String> {
        if (action == Act::Pass && !matches!(phase, MENU | PLACE | SMITE_AMOUNT))
            || phase == MENU && action == Act::Other8
        {
            return Some("Decline".into());
        }
        let id = action as usize;
        Some(match phase {
            MENU => match POWERS[id] {
                Power::Priest => "War Priest attack",
                Power::Blessing => "Blessing of the Trickster",
                Power::Duplicate => "Invoke Duplicity",
                Power::MoveDuplicate => "Move duplicate",
                Power::Archer => "Starry Form (Archer)",
                Power::Chalice => "Starry Form (Chalice)",
                Power::Dragon => "Starry Form (Dragon)",
                Power::FireArrow => "Fire Archer arrow",
                Power::Sea => "Manifest Wrath of the Sea",
                Power::Spray => "Strike with Wrath of the Sea",
                Power::DismissSea => "Dismiss Wrath of the Sea",
                Power::Nature => "Nature's Wrath",
                Power::Athlete => "Peerless Athlete",
                Power::FreeBolt => "Cast Guiding Bolt from Star Map (free use)",
                Power::CastViaDuplicate => {
                    if self.f[i].divine.cast_via_duplicate {
                        "Cast from yourself"
                    } else {
                        "Cast from duplicate"
                    }
                }
                Power::EndStars => "End Starry Form",
                Power::DismissDuplicate => "Dismiss duplicate",
                Power::SwapDuplicate => unreachable!("swap after placement"),
            }
            .to_owned(),
            PLACE if self.f[i].divine.selection == Some(Power::SwapDuplicate) => {
                if action == Act::Defend {
                    "Swap places with the duplicate"
                } else {
                    "Stay in your space"
                }
                .into()
            }
            TARGET if self.f[1 - i].divine.blessing_pending.is_some() => {
                "Accept Blessing of the Trickster".into()
            }
            PLACE if action == Act::Other8 => "Cancel placement".into(),
            PLACE if self.f[i].divine.duplicate_cell.is_some() => {
                format!("Place duplicate at {} ft elevation", id * 5)
            }
            PLACE => {
                let p = Self::divine_point(id);
                format!(
                    "Place duplicate at {}{}",
                    char::from(b'A' + p.cell.x as u8),
                    p.cell.y + 1
                )
            }
            PUSH => format!("Push {} feet", id * 5),
            GUIDED => "Guided Strike (+10)".into(),
            SMITE_AMOUNT => format!("Grant {} temporary HP", id + 1),
            PRIEST_MODE => match PRIEST_MODES[id] {
                Act::Unarmed => "Unarmed Strike (damage)",
                Act::Melee => "Melee weapon attack",
                Act::Ranged => "Ranged weapon attack",
                Act::Grapple => "Unarmed Strike (grapple)",
                Act::ShovePush => "Unarmed Strike (push)",
                Act::ShoveProne => "Unarmed Strike (knock prone)",
                _ => unreachable!(),
            }
            .into(),
            NATURE_TARGETS if action == COMMIT => {
                "Use Channel Divinity against selected creatures".into()
            }
            TARGET | SMITE_TARGET | CHALICE | NATURE_TARGETS => self.creature_label(id),
            _ => return None,
        })
    }

    pub(super) fn divine_decision_step(&mut self, i: usize, phase: i32, action: Act) {
        let id = action as usize;
        match phase {
            MENU if action != Act::Other8 => self.divine_choose(i, POWERS[id]),
            PRIEST_MODE if action != Act::Pass => {
                self.f[i].divine.priest_mode = id;
                self.request(i, TARGET);
            }
            NATURE_TARGETS => match action {
                Act::Pass => self.f[i].divine.nature_targets = 0,
                COMMIT => {
                    let targets = std::mem::take(&mut self.f[i].divine.nature_targets);
                    self.divine_spend_magic_action(i);
                    self.f[i].secondary -= 1;
                    for id in (0..4).filter(|&id| targets & (1 << id) != 0) {
                        self.divine_restrain(i, id);
                    }
                }
                _ => {
                    self.f[i].divine.nature_targets |= 1 << id;
                    self.request(i, NATURE_TARGETS);
                }
            },
            TARGET if self.f[1 - i].divine.blessing_pending.is_some() => {
                let caster = 1 - i;
                let recipient = self.f[caster].divine.blessing_pending.take();
                if action == Act::Defend {
                    self.f[caster].divine.trickster = recipient;
                }
            }
            TARGET => {
                let power = self.f[i].divine.selection.take().expect("target power");
                if action != Act::Pass {
                    self.divine_apply(i, id, power);
                }
            }
            PLACE => {
                let power = self.f[i].divine.selection.take().expect("placement power");
                if power == Power::SwapDuplicate {
                    if action == Act::Defend {
                        self.divine_swap_duplicate(i);
                    }
                    return;
                }
                let cell = self.f[i].divine.duplicate_cell.take();
                if action == Act::Other8 {
                    return;
                }
                let p = if let Some(cell) = cell {
                    Point {
                        cell,
                        altitude: id as i8,
                    }
                } else {
                    let cell = Self::divine_point(id).cell;
                    self.f[i].divine.selection = Some(power);
                    self.f[i].divine.duplicate_cell = Some(cell);
                    self.request(i, PLACE);
                    return;
                };
                if self.try_point_collision(i, p) {
                    return;
                }
                self.bonus = false;
                let until = if power == Power::Duplicate {
                    self.f[i].resource -= 1;
                    self.f[i].divine.cast_via_duplicate = false;
                    self.turns + 20
                } else {
                    self.f[i].divine.duplicate.expect("active duplicate").1
                };
                self.f[i].divine.duplicate = Some((p, until));
                self.f[i].divine.selection = Some(Power::SwapDuplicate);
                self.request(i, PLACE);
            }
            PUSH => {
                let target = self.f[i].divine.spray_target.take().expect("spray victim");
                self.area_push_distance(self.fighter_point(i), 1 << target, id as i32);
            }
            GUIDED => self.divine_resume_guided(i, action == Act::Defend),
            SMITE_TARGET if action == Act::Pass => self.divine_finish_smite(i),
            SMITE_TARGET => {
                if self.f[i].divine.smite_hp == -1 {
                    self.f[i].secondary -= 1;
                    self.f[i].divine.smite_hp = self.roll(8) + self.roll(8) + self.f[i].build.level;
                }
                self.f[i].divine.smite_target = id;
                self.request(i, SMITE_AMOUNT);
            }
            SMITE_AMOUNT => self.divine_allocate_smite(i, id as i32 + 1),
            CHALICE => {
                let hp = std::mem::take(&mut self.f[i].divine.chalice_hp);
                if action != Act::Pass {
                    if id < 2 {
                        self.magic_heal(id, hp);
                    } else {
                        self.entity_heal(id, hp);
                    }
                }
            }
            _ => {}
        }
    }

    fn divine_choose(&mut self, i: usize, power: Power) {
        self.f[i].divine.selection = Some(power);
        let phase = match power {
            Power::Duplicate | Power::MoveDuplicate => PLACE,
            Power::Priest => PRIEST_MODE,
            Power::Nature => {
                self.f[i].divine.nature_targets = 0;
                NATURE_TARGETS
            }
            Power::FreeBolt => {
                self.f[i].divine.free_bolts_spent += 1;
                self.divine_cast_free_bolt(i);
                return;
            }
            Power::CastViaDuplicate => {
                self.f[i].divine.cast_via_duplicate ^= true;
                return;
            }
            Power::DismissDuplicate => {
                self.f[i].divine.duplicate = None;
                self.f[i].divine.cast_via_duplicate = false;
                return;
            }
            Power::EndStars => {
                self.f[i].divine.stars = None;
                return;
            }
            Power::Archer | Power::Chalice | Power::Dragon => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].divine.stars = Some((power, self.turns + 200));
                if power != Power::Archer || self.divine_harmful_targets(i, 12, false) == 0 {
                    self.f[i].divine.selection = None;
                    return;
                }
                TARGET
            }
            Power::Sea => {
                self.bonus = false;
                self.f[i].resource -= 1;
                self.f[i].divine.sea_until = self.turns + 200;
                self.f[i].divine.selection = Some(Power::Spray);
                if self.divine_harmful_targets(i, 1, false) == 0 {
                    self.f[i].divine.selection = None;
                    return;
                }
                TARGET
            }
            Power::DismissSea => {
                self.f[i].divine.sea_until = 0;
                self.f[i].divine.selection = None;
                return;
            }
            Power::Athlete => {
                self.bonus = false;
                self.f[i].secondary -= 1;
                self.f[i].divine.athlete_until = self.turns + 1200;
                self.f[i].divine.selection = None;
                return;
            }
            _ => TARGET,
        };
        self.request(i, phase);
    }

    fn divine_apply(&mut self, i: usize, target: usize, power: Power) {
        match power {
            Power::Priest => {
                self.bonus = false;
                self.f[i].divine.priest_spent += 1;
                let attack = match PRIEST_MODES[self.f[i].divine.priest_mode] {
                    Act::Unarmed => self.unarmed_spec(i),
                    action @ (Act::Melee | Act::Ranged) => {
                        let ranged = action == Act::Ranged;
                        self.gear_strike_equip(i, ranged);
                        self.note_light_equipped(i, ranged);
                        self.weapon_spec(i, ranged, false)
                    }
                    action => {
                        let maneuver = match action {
                            Act::Grapple => UnarmedManeuver::Grapple,
                            Act::ShovePush => UnarmedManeuver::ShovePush,
                            Act::ShoveProne => UnarmedManeuver::ShoveProne,
                            _ => unreachable!(),
                        };
                        self.bonus_unarmed_maneuver(i, target, maneuver);
                        return;
                    }
                };
                self.start_attack_against(i, target, attack);
            }
            Power::Blessing => {
                self.divine_spend_magic_action(i);
                if target % 2 == i {
                    self.f[i].divine.trickster = Some(target);
                } else {
                    self.f[i].divine.blessing_pending = Some(target);
                    self.request(1 - i, TARGET);
                }
            }
            Power::Archer | Power::FireArrow => {
                self.bonus = false;
                self.divine_archer_attack(i, target);
            }
            Power::Spray => {
                self.bonus = false;
                let dc = self.spell_dc(i);
                if self.divine_save(target, 2, dc) {
                    return;
                }
                let damage = (0..self.f[i].build.casting.max(1))
                    .map(|_| self.roll(6))
                    .sum();
                self.divine_spray_damage(i, target, damage);
            }
            _ => unreachable!("placement or immediate power"),
        }
    }

    pub(in crate::arena2) fn divine_offer_spray_push(&mut self, i: usize, target: usize) {
        let alive = if target < 2 {
            self.f[target].hp > 0
        } else {
            self.entity(target).is_some()
        };
        if alive && self.combatant_size(target) <= 3 {
            self.f[i].divine.spray_target = Some(target);
            self.request(i, PUSH);
        }
    }

    fn divine_save(&mut self, target: usize, stat: usize, dc: i32) -> bool {
        if target < 2 {
            self.save(target, stat, dc)
        } else {
            self.entity_save(target, stat, dc)
        }
    }

    fn divine_restrain(&mut self, i: usize, target: usize) {
        let dc = self.spell_dc(i);
        if !self.divine_save(target, 0, dc) {
            self.f[i].divine.restrained_until[target] = self.turns + 20;
        }
    }

    pub(in crate::arena2) fn divine_forget_companion(&mut self, id: usize) {
        for f in &mut self.f {
            f.divine.restrained_until[id] = 0;
            if f.divine.trickster == Some(id) {
                f.divine.trickster = None;
            }
        }
    }

    pub(in crate::arena2) fn divine_restrained(&self, id: usize) -> bool {
        self.f
            .iter()
            .any(|f| f.divine.restrained_until[id] > self.turns)
    }

    pub(in crate::arena2) fn divine_turn_end(&mut self, id: usize) {
        for caster in 0..2 {
            if self.f[caster].divine.restrained_until[id] > self.turns
                && self.creature_point(id).is_some()
            {
                let dc = self.spell_dc(caster);
                if self.divine_save(id, 0, dc) {
                    self.f[caster].divine.restrained_until[id] = 0;
                }
            }
        }
    }

    pub(super) fn divine_end_on_incapacitation(&mut self, i: usize) {
        self.f[i].divine.duplicate = None;
        self.f[i].divine.cast_via_duplicate = false;
        self.f[i].divine.stars = None;
        self.f[i].divine.sea_until = 0;
    }

    pub(in crate::arena2) fn divine_spell_origin(&self, i: usize) -> Point {
        self.divine_cast_origin(i)
            .unwrap_or_else(|| self.fighter_point(i))
    }

    pub(in crate::arena2) fn divine_cast_origin(&self, i: usize) -> Option<Point> {
        (self.f[i].divine.cast_via_duplicate && self.present_on_arena_plane(i))
            .then(|| self.divine_duplicate(i))
            .flatten()
    }

    pub(super) fn divine_moved(&mut self, i: usize) {
        if self.f[i]
            .divine
            .duplicate
            .is_some_and(|(p, _)| p.dist(self.fighter_point(i)) > 24)
        {
            self.f[i].divine.duplicate = None;
            self.f[i].divine.cast_via_duplicate = false;
        }
    }

    fn divine_swap_duplicate(&mut self, i: usize) {
        let Some((destination, until)) = self.f[i].divine.duplicate else {
            return;
        };
        let origin = self.fighter_point(i);
        self.commit_forced_move(i, destination);
        if self.fighter_point(i) == destination {
            self.f[i].divine.duplicate = Some((origin, until));
        }
    }

    pub(in crate::arena2) fn divine_duplicate(&self, i: usize) -> Option<Point> {
        self.f[i]
            .divine
            .duplicate
            .filter(|&(p, until)| {
                until > self.turns && !self.incap(i) && p.dist(self.fighter_point(i)) <= 24
            })
            .map(|(p, _)| p)
    }

    pub(super) fn divine_duplicate_advantage(&self, i: usize, target: usize) -> bool {
        if !self.present_on_arena_plane(i) || !self.present_on_arena_plane(target) {
            return false;
        }
        self.divine_duplicate(i).is_some_and(|p| {
            self.creature_point(target).is_some_and(|t| {
                t.dist(p) <= 1 && t.dist(self.fighter_point(i)) <= 1 && self.map.los_at(t, p)
            })
        })
    }

    pub(in crate::arena2) fn divine_skill_advantage(&self, i: usize, skill: Skill) -> bool {
        match skill {
            Skill::Stealth => self.f.iter().any(|f| f.divine.trickster == Some(i)),
            Skill::Athletics | Skill::Acrobatics => self.divine_athlete(i),
            _ => false,
        }
    }

    pub(in crate::arena2) fn divine_starlight(&self, i: usize) -> bool {
        self.f[i]
            .divine
            .stars
            .is_some_and(|(_, until)| until > self.turns)
    }

    pub(in crate::arena2) fn divine_dragon(&self, i: usize) -> bool {
        self.divine_stars(i, Power::Dragon)
    }

    fn divine_stars(&self, i: usize, expected: Power) -> bool {
        self.f[i]
            .divine
            .stars
            .is_some_and(|(power, until)| power == expected && until > self.turns && !self.incap(i))
    }

    pub(in crate::arena2) fn divine_athlete(&self, i: usize) -> bool {
        self.f[i].divine.athlete_until > self.turns
    }

    pub(in crate::arena2) fn divine_chalice(&mut self, i: usize, tier: usize) {
        if tier > 0 && self.divine_stars(i, Power::Chalice) {
            self.f[i].divine.chalice_hp = self.roll(8) + self.f[i].build.casting;
            self.request(i, CHALICE);
        }
    }

    pub(super) fn divine_offer_guided(&mut self, hit: Hit, attack: Attack) -> bool {
        self.divine_offer_guided_after(hit, attack, 0)
    }

    fn divine_offer_guided_after(&mut self, hit: Hit, attack: Attack, declined: u8) -> bool {
        if hit.d20 == 1 || hit.total + 10 < hit.ac {
            return false;
        }
        let Some(i) = [hit.attacker % 2, 1 - hit.attacker % 2]
            .into_iter()
            .find(|&i| {
                declined & (1 << i) == 0
                    && self.divine_ready(i, Class::Cleric, 1)
                    && self.f[i].resource > 0
                    && (i == hit.attacker
                        || self.can_react(i)
                            && self.rope_same_space(i, hit.attacker)
                            && self
                                .combatant_distance(i, hit.attacker)
                                .is_some_and(|distance| distance <= 6))
            })
        else {
            return false;
        };
        self.f[i].divine.guided = Some((hit, attack, declined));
        self.request(i, GUIDED);
        true
    }

    fn divine_resume_guided(&mut self, i: usize, accepted: bool) {
        let (mut hit, attack, declined) = self.f[i].divine.guided.take().expect("missed attack");
        if accepted {
            if hit.attacker != i {
                self.f[i].reaction = false;
            }
            self.f[i].resource -= 1;
            hit.total += 10;
            hit.crit |= hit.defender < 2
                && self.magic_paralyzed(hit.defender)
                && self
                    .combatant_distance(hit.attacker, hit.defender)
                    .is_some_and(|distance| distance <= 1);
            self.confirm_roll_hit(hit, attack);
        } else if !self.divine_offer_guided_after(hit, attack, declined | (1 << i)) {
            self.offer_inspiration_after_miss(hit, attack);
        }
    }

    pub(in crate::arena2) fn divine_offer_smite(&mut self, i: usize) -> bool {
        if !self.divine_ready(i, Class::Paladin, 2) || self.f[i].secondary <= 0 {
            return false;
        }
        self.f[i].divine.smite_hp = -1;
        self.f[i].divine.smite_used = 0;
        self.request(i, SMITE_TARGET);
        true
    }

    fn divine_allocate_smite(&mut self, i: usize, amount: i32) {
        let id = self.f[i].divine.smite_target;
        let state = &mut self.f[i].divine;
        state.smite_hp -= amount;
        state.smite_used |= 1 << id;
        self.grant_temporary_hp(id, amount, TempSource::InspiringSmite);
        if self.f[i].divine.smite_hp <= 0
            || self.divine_decision_mask(i, SMITE_TARGET) == Act::Pass.bit()
        {
            self.divine_finish_smite(i);
        } else {
            self.request(i, SMITE_TARGET);
        }
    }

    fn divine_finish_smite(&mut self, i: usize) {
        self.f[i].divine.smite_hp = 0;
        self.martial_resume_inspiring_smite(i);
    }

    pub(in crate::arena2) fn divine_subclass_features(&self, viewer: usize) -> [f32; 8] {
        let f = &self.f[viewer];
        if f.build.variant == 0
            || !matches!(f.build.class, Class::Cleric | Class::Druid | Class::Paladin)
        {
            return [0.0; 8];
        }
        let state = &f.divine;
        let phase = self.phase_id();
        let own_decision = self.actor() == Some(viewer);
        let mode = state.stars.map_or(0.0, |(form, until)| {
            if until <= self.turns {
                return 0.0;
            }
            match form {
                Power::Archer => 1.0 / 3.0,
                Power::Chalice => 2.0 / 3.0,
                _ => 1.0,
            }
        });
        let point = self.divine_duplicate(viewer);
        [
            mode,
            f32::from(state.sea_until > self.turns || self.divine_athlete(viewer)),
            point.map_or(0.0, |p| {
                (f32::from(p.cell.x) + 1.0) / f32::from(geometry::WIDTH)
            }),
            point.map_or(0.0, |p| {
                (f32::from(p.cell.y) + 1.0) / f32::from(geometry::HEIGHT)
            }),
            f32::from(self.divine_cast_origin(viewer).is_some()),
            (f.build.casting.max(1) - state.free_bolts_spent.max(state.priest_spent)) as f32 / 5.0,
            if own_decision && matches!(phase, SMITE_TARGET | SMITE_AMOUNT) {
                state.smite_hp.max(0) as f32 / 21.0
            } else {
                0.0
            },
            if own_decision && phase == TARGET {
                state.selection.map_or(0.0, |power| {
                    (power as usize + 1) as f32 / POWERS.len() as f32
                })
            } else {
                0.0
            },
        ]
    }

    pub(in crate::arena2) fn divine_subclass_policy(
        &self,
        i: usize,
        mask: u128,
        _expert: bool,
    ) -> Option<Act> {
        let foe = 1 - i;
        let target = ACTS[foe];
        let has = |act: Act| mask & act.bit() != 0;
        let state = &self.f[i].divine;
        let menu = self.divine_menu(i);
        let hostile = ACTS[foe].bit() | ACTS[foe + 2].bit();
        let nature = self.divine_harmful_targets(i, 3, false) & hostile;
        let desired = if !self.divine_restrained(foe)
            && nature != 0
            && menu & ACTS[Power::Nature as usize].bit() != 0
        {
            Some(Power::Nature)
        } else {
            [
                Power::Priest,
                Power::Spray,
                Power::FireArrow,
                Power::Sea,
                Power::Archer,
                Power::Duplicate,
                Power::FreeBolt,
            ]
            .into_iter()
            .find(|&power| {
                if menu & ACTS[power as usize].bit() == 0 {
                    return false;
                }
                match power {
                    Power::Sea => {
                        state.sea_until <= self.turns
                            && self.divine_targets(i, 1, false) & target.bit() != 0
                    }
                    Power::Archer => {
                        state.stars.is_none_or(|(_, until)| until <= self.turns)
                            && self.divine_targets(i, 12, false) & target.bit() != 0
                    }
                    Power::Duplicate => {
                        self.divine_duplicate(i).is_none()
                            && self.divine_targets(i, 1, false) & target.bit() != 0
                    }
                    Power::FreeBolt => self.entity_visible(i, foe),
                    Power::Spray => self.divine_harmful_targets(i, 1, false) & hostile != 0,
                    Power::FireArrow => self.divine_harmful_targets(i, 12, false) & hostile != 0,
                    _ => true,
                }
            })
        };
        Some(match self.phase_id() {
            0 if has(ACTION) && desired.is_some() => ACTION,
            MENU => desired.map_or(Act::Other8, |power| ACTS[power as usize]),
            PRIEST_MODE => [1, 0, 2, 4, 5, 3]
                .into_iter()
                .map(|index| ACTS[index])
                .find(|&act| has(act))
                .unwrap_or(Act::Pass),
            TARGET if self.f[1 - i].divine.blessing_pending.is_some() => Act::Pass,
            TARGET if state.selection == Some(Power::Blessing) => ACTS[i],
            TARGET if has(target) => target,
            TARGET => (0..4)
                .find(|&id| id % 2 != i && has(ACTS[id]))
                .map_or(Act::Pass, |id| ACTS[id]),
            NATURE_TARGETS if has(target) => target,
            NATURE_TARGETS if has(ACTS[foe + 2]) => ACTS[foe + 2],
            NATURE_TARGETS if has(COMMIT) => COMMIT,
            NATURE_TARGETS => Act::Pass,
            PLACE if state.selection == Some(Power::SwapDuplicate) => Act::Pass,
            PLACE if state.duplicate_cell.is_some() => ACTS[mask.trailing_zeros() as usize],
            PLACE => {
                if let Some(foe) = self.observed_pos(i, foe) {
                    (0..63)
                        .filter(|&index| has(ACTS[index]))
                        .min_by_key(|&index| Self::divine_point(index).cell.dist(foe))
                        .map_or(Act::Other8, |index| ACTS[index])
                } else {
                    Act::Other8
                }
            }
            PUSH => ACTS[3],
            GUIDED => Act::Defend,
            SMITE_TARGET if has(ACTS[i]) => ACTS[i],
            SMITE_TARGET => Act::Pass,
            SMITE_AMOUNT => ACTS[127 - mask.leading_zeros() as usize],
            CHALICE if has(ACTS[i]) => ACTS[i],
            CHALICE => Act::Pass,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encounter(seed: u64, roster: u8) -> Battle {
        let mut b = Battle::new_roster_in(seed, [roster, 12], 0);
        b.owner = 0;
        b.f[0].pos = Pos { x: 1, y: 0 };
        b.f[1].pos = Pos { x: 2, y: 0 };
        b
    }

    fn choose(b: &mut Battle, power: Power) {
        b.step(ACTION).unwrap();
        assert_eq!(b.phase_id(), MENU);
        b.step(ACTS[power as usize]).unwrap();
    }

    #[test]
    fn war_priest_offers_bonus_unarmed_attack_and_uses_separate_pool() {
        let mut b = encounter(1, 7);
        let charges = b.f[0].resource;
        choose(&mut b, Power::Priest);
        assert_eq!(b.phase_id(), PRIEST_MODE);
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.f[0].divine.priest_spent, 1);
        assert_eq!(b.f[0].resource, charges);
        assert!(b.action);
        assert!(!b.bonus);
        assert_eq!(b.class_ready_mask(0) & Act::Special2.bit(), 0);
    }

    #[test]
    fn guided_strike_repairs_own_miss_without_taking_reaction() {
        let mut b = (1..100)
            .map(|seed| {
                let mut b = encounter(seed, 7);
                b.step(Act::Melee).unwrap();
                b
            })
            .find(|b| b.phase_id() == GUIDED)
            .expect("repairable attack roll");
        let charges = b.f[0].resource;
        let reaction = b.f[0].reaction;
        assert_eq!(b.choice_label(0, Act::Defend), "Guided Strike (+10)");
        b.step(Act::Defend).unwrap();
        assert_eq!(b.f[0].resource, charges - 1);
        assert_eq!(b.f[0].reaction, reaction);
    }

    #[test]
    fn trickery_blessing_and_duplicate_have_distinct_costs() {
        let mut b = encounter(1, 8);
        let charges = b.f[0].resource;
        choose(&mut b, Power::Blessing);
        b.step(ACTS[0]).unwrap();
        assert!(b.divine_skill_advantage(0, Skill::Stealth));
        assert!(!b.action);
        assert_eq!(b.f[0].resource, charges);
        choose(&mut b, Power::Duplicate);
        assert_eq!(b.phase_id(), PLACE);
        b.step(ACTS[2]).unwrap();
        assert_eq!(b.mask() & ACTS[0].bit(), 0, "ground occupied by rival");
        b.step(Act::Other8).unwrap();
        choose(&mut b, Power::Duplicate);
        let placement = geometry::WIDTH as usize + 2;
        b.step(ACTS[placement]).unwrap();
        b.step(ACTS[0]).unwrap();
        b.step(Act::Pass).unwrap();
        assert_eq!(b.f[0].resource, charges - 1);
        assert!(b.divine_duplicate_advantage(0, 1));
        choose(&mut b, Power::CastViaDuplicate);
        assert_eq!(b.divine_spell_origin(0), Battle::divine_point(placement));
        assert_eq!(b.fighter_point(0).cell, Pos { x: 1, y: 0 });
    }

    #[test]
    fn stars_uses_dragon_floor_and_star_map_does_not_spend_rank() {
        let mut b = encounter(1, 10);
        b.f[0].gear.melee = crate::arena2::equipment::Carried::Packed;
        let slots = b.f[0].slots;
        let charges = b.f[0].resource;
        choose(&mut b, Power::Dragon);
        assert!(b.divine_dragon(0));
        assert_eq!(b.f[0].resource, charges - 1);
        assert!(b.save_concentration(0, 10));
        choose(&mut b, Power::FreeBolt);
        assert_eq!(b.f[0].slots, slots);
        assert_eq!(b.f[0].divine.free_bolts_spent, 1);
        assert!(!b.action);
        assert_ne!(b.phase_id(), MENU);
    }

    #[test]
    fn sea_manifests_once_and_controls_push_after_failed_save() {
        let (mut b, hp) = (1..100)
            .map(|seed| {
                let mut b = encounter(seed, 11);
                let hp = b.f[1].hp;
                choose(&mut b, Power::Sea);
                b.step(ACTS[1]).unwrap();
                (b, hp)
            })
            .find(|(b, _)| b.phase_id() == PUSH)
            .expect("failed Constitution saving throw");
        assert!(b.f[1].hp < hp);
        assert_eq!(b.f[0].resource, 1);
        b.step(ACTS[3]).unwrap();
        assert_eq!(b.f[1].pos, Pos { x: 5, y: 0 });
        assert!(b.f[0].divine.sea_until > b.turns);
    }

    #[test]
    fn nature_wrath_spends_channel_on_commit_and_restrains_without_concentration() {
        let mut b = (1..100)
            .map(|seed| {
                let mut b = encounter(seed, 19);
                let charges = b.f[0].secondary;
                assert_eq!(b.mask() & Act::Special2.bit(), 0);
                choose(&mut b, Power::Nature);
                b.step(ACTS[1]).unwrap();
                assert_eq!(b.f[0].secondary, charges);
                assert!(b.action);
                b.step(COMMIT).unwrap();
                assert_eq!(b.f[0].secondary, charges - 1);
                b
            })
            .find(|b| b.divine_restrained(1))
            .expect("failed Strength saving throw");
        assert_eq!(b.speed(1), 0);
        assert_eq!(b.f[0].concentration, None);
        for _ in 0..20 {
            b.divine_turn_end(1);
        }
        assert!(!b.divine_restrained(1));
    }

    #[test]
    fn glory_can_allocate_inspiring_smite_pool_after_divine_smite() {
        let mut b = (1..100)
            .map(|seed| {
                let mut b = encounter(seed, 20);
                b.step(Act::Melee).unwrap();
                b
            })
            .find(|b| b.phase_id() == 23)
            .expect("melee strike hit with smite available");
        let charges = b.f[0].secondary;
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.phase_id(), SMITE_TARGET);
        assert_eq!(b.f[0].secondary, charges);
        b.step(ACTS[0]).unwrap();
        assert_eq!(b.phase_id(), SMITE_AMOUNT);
        let pool = b.f[0].divine.smite_hp;
        assert!((7..=21).contains(&pool));
        assert_eq!(b.f[0].secondary, charges - 1);
        b.step(ACTS[pool as usize - 1]).unwrap();
        assert_eq!(b.f[0].temp, pool);
        assert!(!matches!(b.phase_id(), SMITE_TARGET | SMITE_AMOUNT));
    }

    #[test]
    fn peerless_athlete_installs_athletics_and_acrobatics_advantage() {
        let mut b = encounter(1, 20);
        choose(&mut b, Power::Athlete);
        assert!(b.divine_athlete(0));
        assert!(b.divine_skill_advantage(0, Skill::Athletics));
        assert!(b.divine_skill_advantage(0, Skill::Acrobatics));
        assert_eq!(b.f[0].secondary, 1);
        assert!(b.action);
    }

    #[test]
    fn duplicate_selects_an_elevation_and_can_swap_to_its_position() {
        let mut b = encounter(1, 8);
        b.f[0].pos = Pos { x: 1, y: 1 };
        let previous = b.fighter_point(0);
        choose(&mut b, Power::Duplicate);
        // B2 at five feet is directly over the Cleric.
        b.step(ACTS[geometry::WIDTH as usize + 1]).unwrap();
        b.step(ACTS[1]).unwrap();
        assert_eq!(
            b.divine_duplicate(0),
            Some(Point {
                altitude: 1,
                ..previous
            })
        );
        b.step(Act::Pass).unwrap();
        assert_eq!(b.fighter_point(0), previous);
        assert_eq!(b.divine_duplicate(0).unwrap().altitude, 1);

        b.bonus = true;
        choose(&mut b, Power::MoveDuplicate);
        let cell = geometry::WIDTH as usize + 2;
        b.step(ACTS[cell]).unwrap();
        b.step(ACTS[0]).unwrap();
        assert!(b.choice_label(0, Act::Defend).contains("Swap places"));
        b.step(Act::Defend).unwrap();
        assert_eq!(b.fighter_point(0), Battle::divine_point(cell));
        assert_eq!(b.divine_duplicate(0), Some(previous));
    }

    #[test]
    fn trickster_blessing_acceptance_is_decided_by_other_player() {
        let mut b = encounter(1, 8);
        choose(&mut b, Power::Blessing);
        b.step(ACTS[1]).unwrap();
        assert_eq!(b.actor(), Some(1));
        let mut declined = b.clone();
        declined.step(Act::Pass).unwrap();
        assert!(!declined.divine_skill_advantage(1, Skill::Stealth));
        b.step(Act::Defend).unwrap();
        assert!(b.divine_skill_advantage(1, Skill::Stealth));
        assert_eq!(b.actor(), Some(0));
    }

    #[test]
    fn archer_attacks_on_activation_and_chalice_can_heal_a_second_creature() {
        let mut archer = (1..100)
            .map(|seed| {
                let mut b = encounter(seed, 10);
                b.f[0].pos = Pos { x: 0, y: 0 };
                choose(&mut b, Power::Archer);
                b.step(ACTS[1]).unwrap();
                b
            })
            .find(|b| b.f[1].hp < b.f[1].build.hp)
            .expect("an Archer attack hits");
        assert!(archer.action && !archer.bonus);
        assert_eq!(archer.f[0].resource, 1);
        archer.bonus = true;
        choose(&mut archer, Power::FireArrow);
        archer.step(ACTS[1]).unwrap();
        assert_eq!(archer.f[0].resource, 1);
        assert!(!archer.bonus);

        let mut chalice = encounter(2, 10);
        chalice.f[0].hp = 1;
        choose(&mut chalice, Power::Chalice);
        chalice.bonus = true;
        let enemy_hp = chalice.f[1].hp - 15;
        chalice.f[1].hp = enemy_hp;
        chalice.step(Act::Spell2).unwrap();
        if chalice.phase_id() == 48 {
            chalice.step(ACTS[0]).unwrap();
        }
        if chalice.phase_id() == 38 {
            chalice.step(ACTS[0]).unwrap();
        }
        assert_eq!(chalice.phase_id(), CHALICE);
        chalice.step(ACTS[1]).unwrap();
        assert!(chalice.f[1].hp > enemy_hp);
        assert_eq!(chalice.f[0].resource, 1);
    }

    #[test]
    fn guiding_another_creatures_attack_uses_the_clerics_reaction() {
        let mut b = (1..100)
            .map(|seed| {
                let mut b = encounter(seed, 7);
                b.owner = 1;
                b.step(Act::Melee).unwrap();
                b
            })
            .find(|b| b.phase_id() == GUIDED)
            .expect("other actor's repairable attack roll");
        assert_eq!(b.actor(), Some(0));
        let shots = b.f[0].resource;
        b.step(Act::Defend).unwrap();
        assert!(!b.f[0].reaction);
        assert_eq!(b.f[0].resource, shots - 1);
    }

    #[test]
    fn ancients_policy_progresses_when_no_hostile_is_in_range() {
        let mut b = Battle::new_roster_in(5_923_729_370_248_086_095, [19, 0], 0);
        while !b.finished() {
            let action = crate::arena2::bot::choose(&b, true);
            b.step(action).unwrap();
        }
        assert!(b.decisions < DECISION_LIMIT);
        assert!(b.turns > 1);
    }

    #[test]
    fn releasing_readied_nature_wrath_preserves_the_current_turn_action() {
        let mut b = encounter(2, 19);
        b.owner = 1;
        assert!(b.divine_ready_valid(0));
        b.divine_ready_release(0);
        assert_eq!(b.actor(), Some(0));
        b.step(ACTS[1]).unwrap();
        b.step(COMMIT).unwrap();
        assert!(b.action);
        assert_eq!(b.f[0].secondary, 1);
        assert_eq!(b.owner, 1);
    }

    #[test]
    fn subclasses_keep_their_own_level_three_features() {
        for id in [7, 8, 10, 11, 19, 20] {
            let mut b = encounter(1, id);
            assert!(
                b.mask() & ACTION.bit() != 0,
                "variant {id} has a subclass action"
            );
            match b.f[0].build.class {
                Class::Cleric => assert_eq!(b.mask() & Act::Special2.bit(), 0),
                Class::Druid => assert_eq!(b.mask() & Act::Special3.bit(), 0),
                Class::Paladin => assert_eq!(b.mask() & Act::Special2.bit(), 0),
                _ => unreachable!(),
            }
            b.f[0].build.level = 2;
            assert_eq!(b.mask() & ACTION.bit(), 0, "level two variant {id}");
        }
        let b = encounter(1, 9);
        assert!(
            b.mask() & Act::Special3.bit() != 0,
            "Land's Aid remains available"
        );
    }

    #[test]
    fn starry_form_illuminates_a_gloom_stalker_in_the_dark_arena() {
        let mut b = Battle::new_roster_in(1, [10, 23], geometry::DARK_LAYOUT);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 2, y: 0 };
        assert!(!b.can_see(0, 1));
        choose(&mut b, Power::Dragon);
        assert!(b.can_see(0, 1));
        choose(&mut b, Power::EndStars);
        assert!(!b.can_see(0, 1));
    }

    #[test]
    fn duplicate_origin_changes_cast_aim_around_total_cover() {
        let mut b = encounter(1, 8);
        b.f[0].pos = Pos { x: 0, y: 3 };
        let target = Pos { x: 4, y: 3 };
        let height_zero = |battle: &Battle| battle.target_altitude_mask(Act::Magic15, target) & 1;
        assert_eq!(height_zero(&b), 0);
        let duplicate = Point::ground(Pos { x: 1, y: 0 });
        b.f[0].divine.duplicate = Some((duplicate, b.turns + 20));
        choose(&mut b, Power::CastViaDuplicate);
        assert_eq!(height_zero(&b), 1);
        assert_eq!(b.f[0].pos, Pos { x: 0, y: 3 });
    }
}
