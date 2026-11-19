//! Level-three martial subclass features from Player's Handbook (2024).

use super::{Act, Battle, Class, Hit};
use crate::arena2::{equipment::DamageType, geometry::Point, temp_hp::TempSource, ACTS};

pub(in crate::arena2) const DIVINE_FURY: i32 = 141;
pub(in crate::arena2) const WARRIOR_OF_GODS: i32 = 142;
pub(in crate::arena2) const LIFE_GIVING_FORCE: i32 = 143;
const ELEMENTAL_ATTUNEMENT: i32 = 144;
const ELEMENTAL_DAMAGE: i32 = 145;
const ELEMENTAL_MOVE: i32 = 146;
const HAND_OF_HARM: i32 = 147;
pub(super) const HAND_OF_HEALING: i32 = 148;
pub(in crate::arena2) const ELEMENTAL_SAVE: i32 = 149;
pub(in crate::arena2) const ELEMENTALISM_MODE: i32 = 150;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementalismMode {
    Breeze,
    Dust,
    Embers,
    Mist,
    Cup,
    Shape,
}

impl ElementalismMode {
    const CHOICES: [Self; 6] = [
        Self::Breeze,
        Self::Dust,
        Self::Embers,
        Self::Mist,
        Self::Cup,
        Self::Shape,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Breeze => "harmless breeze",
            Self::Dust => "thin dust on surfaces",
            Self::Embers => "amber sparks and cinnamon-scented smoke",
            Self::Mist => "damp mist",
            Self::Cup => "a cup of water",
            Self::Shape => "a small sculpted figure",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Breeze => "Beckon Air: stir a breeze",
            Self::Dust => "Beckon Earth: dust the surfaces",
            Self::Embers => "Beckon Fire: amber sparks and cinnamon scent",
            Self::Mist => "Beckon Water: dampen with mist",
            Self::Cup => "Beckon Water: form a cup of water on the surface",
            Self::Shape => "Sculpt Element: form a small figure from present material",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct ElementalismEffect {
    mode: ElementalismMode,
    point: Point,
    expires: i32,
}

const ELEMENTS: [DamageType; 5] = [
    DamageType::Acid,
    DamageType::Cold,
    DamageType::Fire,
    DamageType::Lightning,
    DamageType::Thunder,
];

impl Battle {
    pub(in crate::arena2) fn has_martial_subclass(
        &self,
        i: usize,
        class: Class,
        variant: u8,
    ) -> bool {
        self.f.get(i).is_some_and(|f| {
            f.build.class == class && f.build.variant == variant && f.build.level >= 3
        })
    }

    pub(in crate::arena2) fn martial_subclass_label(
        &self,
        i: usize,
        action: Act,
    ) -> Option<&'static str> {
        match action {
            Act::Martial1 if self.has_martial_subclass(i, Class::Barbarian, 1) => {
                Some("Warrior of the Gods")
            }
            Act::Martial1 if self.has_martial_subclass(i, Class::Monk, 1) => {
                Some("Hand of Healing")
            }
            _ => None,
        }
    }

    pub(super) fn subclass_bonus_mask(&self, i: usize) -> u128 {
        let f = &self.f[i];
        if self.has_martial_subclass(i, Class::Barbarian, 1)
            && self.bonus
            && f.martial.subclass_dice > 0
            && f.hp < self.magic_max_hp(i)
        {
            Act::Martial1.bit()
        } else {
            0
        }
    }

    pub(super) fn subclass_rage_start(&mut self, i: usize) {
        if self.has_martial_subclass(i, Class::Barbarian, 2) {
            self.grant_temporary_hp(i, self.f[i].build.level, TempSource::VitalitySurge);
        }
    }

    pub(super) fn subclass_rage_end(&mut self, i: usize) {
        if self.has_martial_subclass(i, Class::Barbarian, 2) {
            self.clear_temporary_hp_by_source(TempSource::LifeGivingForce(i));
        }
    }

    pub(in crate::arena2) fn subclass_start_turn(&mut self, i: usize) {
        if self.has_martial_subclass(i, Class::Monk, 2)
            && !self.incap(i)
            && !self.elemental_attuned(i)
            && self.f[i].resource > 0
        {
            self.request(i, ELEMENTAL_ATTUNEMENT);
        } else if self.has_martial_subclass(i, Class::Barbarian, 2)
            && self.f[i].raging
            && self.subclass_decision_mask(i, LIFE_GIVING_FORCE) != Act::End.bit()
        {
            self.request(i, LIFE_GIVING_FORCE);
        }
    }

    pub(in crate::arena2) fn subclass_divine_fury(&mut self, hit: Hit) -> bool {
        let i = hit.attacker;
        if self.has_martial_subclass(i, Class::Barbarian, 1)
            && self.owner == i
            && self.f[i].raging
            && hit.weapon
            && self.f[i].martial.frenzy_turn != self.turns
        {
            self.f[i].martial.frenzy_turn = self.turns;
            self.f[i].martial.divine_fury = (if hit.crit { 2 } else { 1 }, None);
            self.request(i, DIVINE_FURY);
            true
        } else {
            false
        }
    }

    pub(in crate::arena2) fn take_divine_fury(&mut self, hit: Hit) -> (i32, DamageType) {
        let Some(f) = self.f.get_mut(hit.attacker) else {
            return (0, DamageType::Radiant);
        };
        let (count, kind) = std::mem::take(&mut f.martial.divine_fury);
        let Some(kind) = kind else {
            return (0, DamageType::Radiant);
        };
        let damage = f.build.level / 2 + (0..count).map(|_| self.roll(6)).sum::<i32>();
        (damage, kind)
    }

    pub(in crate::arena2) fn subclass_decision_mask(&self, i: usize, phase: i32) -> u128 {
        match phase {
            ELEMENTAL_ATTUNEMENT | HAND_OF_HARM => Act::Rider.bit() | Act::End.bit(),
            ELEMENTAL_DAMAGE => 0b11111 | Act::End.bit(),
            ELEMENTAL_MOVE => 0b1111 | Act::End.bit(),
            ELEMENTAL_SAVE => Act::Defend.bit() | Act::Pass.bit(),
            ELEMENTALISM_MODE => self.elementalism_mask(i),
            HAND_OF_HEALING => self.mercy_heal_targets(i) | Act::End.bit(),
            DIVINE_FURY => Act::N.bit() | Act::NE.bit(),
            WARRIOR_OF_GODS => ACTS[..usize::from(self.f[i].martial.subclass_dice)]
                .iter()
                .fold(Act::End.bit(), |mask, act| mask | act.bit()),
            LIFE_GIVING_FORCE => (0..4)
                .filter(|&target| {
                    target != i
                        && self.observed_footprint(i, target).is_some()
                        && self
                            .combatant_distance(i, target)
                            .is_some_and(|distance| distance <= 2)
                })
                .fold(Act::End.bit(), |mask, target| mask | ACTS[target].bit()),
            _ => 0,
        }
    }

    pub(in crate::arena2) fn subclass_decision_step(&mut self, i: usize, phase: i32, action: Act) {
        match phase {
            ELEMENTAL_ATTUNEMENT if action != Act::End => {
                self.f[i].resource -= 1;
                self.f[i].martial.attunement_until = self.turns + 200;
            }
            ELEMENTAL_DAMAGE => self
                .resume_elemental_damage((action != Act::End).then(|| ELEMENTS[action as usize])),
            ELEMENTAL_MOVE => self.choose_elemental_move(i, action),
            ELEMENTAL_SAVE => self.resolve_elemental_move(action == Act::Defend),
            ELEMENTALISM_MODE if action != Act::End => {
                self.f[i]
                    .martial
                    .elementalism_target
                    .as_mut()
                    .expect("aimed Elementalism")
                    .1 = action;
                if self.f[i].martial.elementalism_readied {
                    self.resolve_elementalism(i);
                } else {
                    self.cast_elementalism(i);
                }
            }
            ELEMENTALISM_MODE => self.f[i].martial.elementalism_target = None,
            HAND_OF_HARM => self.resume_hand_of_harm(action != Act::End),
            HAND_OF_HEALING => self.resolve_mercy_heal(i, action),
            DIVINE_FURY => {
                self.f[i].martial.divine_fury.1 = Some(match action {
                    Act::N => DamageType::Radiant,
                    _ => DamageType::Necrotic,
                });
                self.resume_magic_rider();
            }
            WARRIOR_OF_GODS if action != Act::End => {
                let dice = action as u8 + 1;
                self.f[i].martial.subclass_dice -= dice;
                self.bonus = false;
                let healing = (0..dice).map(|_| self.roll(12)).sum();
                self.heal(i, healing);
            }
            LIFE_GIVING_FORCE if action != Act::End => {
                let temp = self.roll(6) + self.roll(6);
                self.grant_temporary_hp(action as usize, temp, TempSource::LifeGivingForce(i));
            }
            _ => {}
        }
    }

    pub(in crate::arena2) fn subclass_decision_label(
        &self,
        i: usize,
        phase: i32,
        action: Act,
    ) -> String {
        match (phase, action) {
            (DIVINE_FURY, Act::N) => "Divine Fury: Radiant".into(),
            (DIVINE_FURY, _) => "Divine Fury: Necrotic".into(),
            (_, Act::End) => "Decline".into(),
            (ELEMENTAL_ATTUNEMENT, _) => "Spend 1 Focus Point for Elemental Attunement".into(),
            (ELEMENTAL_DAMAGE, _) => {
                format!("Elemental Strikes: {:?} damage", ELEMENTS[action as usize])
            }
            (ELEMENTALISM_MODE, _) => ElementalismMode::CHOICES[action as usize]
                .description()
                .into(),
            (ELEMENTAL_SAVE, Act::Defend) => "Roll Strength saving throw".into(),
            (ELEMENTAL_SAVE, _) => "Voluntarily fail saving throw".into(),
            (ELEMENTAL_MOVE, _) => format!(
                "{} {} ft",
                if (action as usize) < 2 {
                    "Push"
                } else {
                    "Pull"
                },
                5 * (action as usize % 2 + 1)
            ),
            (HAND_OF_HARM, _) => "Spend 1 Focus Point for Hand of Harm".into(),
            (HAND_OF_HEALING, _) => format!(
                "Hand of Healing: {}",
                if action as usize == i {
                    "yourself".into()
                } else {
                    self.martial_target_label(action as usize)
                }
            ),
            (WARRIOR_OF_GODS, _) => format!("Spend {} healing dice", action as usize + 1),
            (LIFE_GIVING_FORCE, _) => format!(
                "Life-Giving Force: {}",
                if action as usize == 1 - i {
                    "opponent"
                } else if action as usize == 2 + i {
                    "your companion"
                } else {
                    "opponent's companion"
                }
            ),
            _ => String::new(),
        }
    }

    pub(in crate::arena2) fn subclass_pending_label(&self, phase: i32) -> &'static str {
        match phase {
            DIVINE_FURY => "Divine Fury: choose damage type",
            WARRIOR_OF_GODS => "Warrior of the Gods: choose how many healing dice to spend",
            LIFE_GIVING_FORCE => "Life-Giving Force: choose another creature or decline",
            ELEMENTAL_ATTUNEMENT => "Start of turn: use Elemental Attunement?",
            ELEMENTAL_DAMAGE => {
                "Elemental Strikes: choose the damage type or keep the original type"
            }
            ELEMENTAL_MOVE => "Elemental Strikes: try to move the target?",
            ELEMENTAL_SAVE => "Elemental Strikes: Strength saving throw",
            ELEMENTALISM_MODE => "Elementalism: choose an effect for the selected area",
            HAND_OF_HARM => "Spend a Focus Point on Hand of Harm?",
            HAND_OF_HEALING => "Hand of Healing: choose a creature to heal",
            _ => "",
        }
    }

    pub(in crate::arena2) fn begin_elementalism(&mut self, i: usize, point: Point, readied: bool) {
        self.f[i].martial.elementalism_target = Some((point, Act::End));
        self.f[i].martial.elementalism_readied = readied;
        self.request(i, ELEMENTALISM_MODE);
    }

    fn elementalism_material(&self, viewer: usize, point: Point) -> Option<(usize, usize, i32)> {
        self.f.iter().enumerate().find_map(|(seat, fighter)| {
            fighter
                .martial
                .elementalism_effects
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, effect)| {
                    (effect.point == point
                        && effect.expires > self.turns
                        && (effect.mode != ElementalismMode::Embers
                            || effect.expires - self.turns > 18)
                        && (seat == viewer
                            || !self.magic_cloud_blocks(self.fighter_point(viewer), point))
                        && matches!(
                            effect.mode,
                            ElementalismMode::Dust
                                | ElementalismMode::Cup
                                | ElementalismMode::Mist
                                | ElementalismMode::Embers
                        ))
                    .then_some((seat, index, effect.expires))
                })
        })
    }

    fn elementalism_object_flame(&self, i: usize, point: Point) -> bool {
        (crate::arena2::objects::FIRST_OBJECT..crate::arena2::objects::END_OBJECTS).any(|id| {
            self.observed_object(i, id)
                .is_some_and(|object| object.burning && object.point == point)
        })
    }

    fn elementalism_mask(&self, i: usize) -> u128 {
        let Some((point, _)) = self.f[i].martial.elementalism_target else {
            return Act::End.bit();
        };
        let mut mask = Act::N.bit() | Act::E.bit() | Act::SE.bit() | Act::End.bit();
        if self.map.supported(point) {
            mask |= Act::NE.bit() | Act::S.bit();
        }
        if self.elementalism_material(i, point).is_some()
            || self.elementalism_object_flame(i, point)
        {
            mask |= Act::SW.bit();
        }
        mask
    }

    pub(in crate::arena2) fn resolve_elementalism(&mut self, i: usize) {
        let (point, choice) = self.f[i]
            .martial
            .elementalism_target
            .take()
            .expect("Elementalism choices");
        let mode = ElementalismMode::CHOICES[choice as usize];
        let expires = match mode {
            ElementalismMode::Breeze | ElementalismMode::Mist => self.turns + 2,
            ElementalismMode::Embers | ElementalismMode::Cup => self.turns + 20,
            ElementalismMode::Dust => i32::MAX,
            ElementalismMode::Shape => {
                if let Some((seat, index, expires)) = self.elementalism_material(i, point) {
                    self.f[seat].martial.elementalism_effects.remove(index);
                    expires.min(self.turns + 1_200)
                } else if self.elementalism_object_flame(i, point) {
                    self.turns + 1_200
                } else {
                    return;
                }
            }
        };
        let effects = &mut self.f[i].martial.elementalism_effects;
        effects.retain(|effect| {
            effect.expires > self.turns && !(effect.point == point && effect.mode == mode)
        });
        effects.push(ElementalismEffect {
            mode,
            point,
            expires,
        });
    }

    pub(in crate::arena2) fn elementalism_summary(
        &self,
        viewer: usize,
        subject: usize,
    ) -> Vec<String> {
        self.f[subject]
            .martial
            .elementalism_effects
            .iter()
            .filter(|effect| {
                effect.expires > self.turns
                    && (viewer == subject
                        || self.map.los_at(self.fighter_point(viewer), effect.point)
                            && !self.magic_cloud_blocks(self.fighter_point(viewer), effect.point))
            })
            .map(|effect| {
                format!(
                    "{} at {}{} ({} ft elevation)",
                    if effect.mode == ElementalismMode::Embers && effect.expires - self.turns <= 18
                    {
                        "lingering cinnamon scent"
                    } else {
                        effect.mode.name()
                    },
                    char::from(b'A' + effect.point.cell.x as u8),
                    effect.point.cell.y + 1,
                    effect.point.altitude * 5
                )
            })
            .collect()
    }

    pub(in crate::arena2) fn martial_subclass_features(&self, i: usize) -> [f32; 8] {
        let mut values = [0.; 8];
        values[0] = f32::from(self.f[i].martial.subclass_dice) / 4.;
        values[1] = u8::from(self.elemental_attuned(i)).into();
        values[2] = u8::from(
            self.has_martial_subclass(i, Class::Monk, 1)
                && self.f[i].martial.harm_turn != self.turns,
        )
        .into();
        values[3] = u8::from(
            self.has_martial_subclass(i, Class::Barbarian, 1)
                && self.f[i].raging
                && self.f[i].martial.frenzy_turn != self.turns,
        )
        .into();
        values
    }

    pub(in crate::arena2) fn martial_subclass_policy(
        &self,
        i: usize,
        mask: u128,
        _expert: bool,
    ) -> Option<Act> {
        let missing_hp = self.magic_max_hp(i) - self.f[i].hp;
        let choice = match self.phase_id() {
            DIVINE_FURY => Act::N,
            LIFE_GIVING_FORCE => {
                if mask & ACTS[2 + i].bit() != 0 {
                    ACTS[2 + i]
                } else {
                    Act::End
                }
            }
            WARRIOR_OF_GODS => {
                let dice =
                    ((missing_hp + 2) / 7).clamp(1, i32::from(self.f[i].martial.subclass_dice));
                ACTS[(dice - 1) as usize]
            }
            ELEMENTAL_ATTUNEMENT => Act::Rider,
            ELEMENTAL_DAMAGE => ACTS[3],
            ELEMENTAL_MOVE => Act::NE,
            ELEMENTAL_SAVE => Act::Defend,
            HAND_OF_HARM => Act::Rider,
            HAND_OF_HEALING => {
                if mask & ACTS[i].bit() != 0 {
                    ACTS[i]
                } else {
                    Act::End
                }
            }
            0 if missing_hp >= 12 && self.has_martial_subclass(i, Class::Barbarian, 1) => {
                Act::Martial1
            }
            0 if missing_hp >= 10 && self.has_martial_subclass(i, Class::Monk, 1) => {
                if mask & Act::Martial5.bit() != 0 {
                    Act::Martial5
                } else {
                    Act::Martial1
                }
            }
            90 if missing_hp >= 7 && mask & Act::Martial1.bit() != 0 => Act::Martial1,
            _ => return None,
        };
        (mask & choice.bit() != 0).then_some(choice)
    }

    pub(in crate::arena2) fn clear_martial_subclass_on_incapacitation(&mut self, i: usize) {
        self.subclass_rage_end(i);
        self.f[i].martial.attunement_until = -1;
    }

    fn elemental_attuned(&self, i: usize) -> bool {
        self.has_martial_subclass(i, Class::Monk, 2)
            && self.f[i].martial.attunement_until > self.turns
            && !self.incap(i)
    }

    pub(in crate::arena2) fn unarmed_reach(&self, i: usize) -> i32 {
        if self.elemental_attuned(i) {
            3
        } else {
            1
        }
    }

    pub(in crate::arena2) fn offer_elemental_damage(&mut self, i: usize, unarmed: bool) -> bool {
        if unarmed && self.elemental_attuned(i) {
            self.request(i, ELEMENTAL_DAMAGE);
            true
        } else {
            false
        }
    }

    pub(in crate::arena2) fn offer_elemental_movement(&mut self, hit: Hit, damage: i32) -> bool {
        if damage > 0
            && hit.unarmed
            && self.elemental_attuned(hit.attacker)
            && ELEMENTS.contains(&hit.damage_kind)
            && (hit.defender >= 2 || self.f[hit.defender].hp > 0)
            && self.footprint(hit.defender).is_some()
        {
            self.f[hit.attacker].martial.elemental_hit = Some((hit, None));
            self.request(hit.attacker, ELEMENTAL_MOVE);
            true
        } else {
            false
        }
    }

    fn choose_elemental_move(&mut self, i: usize, action: Act) {
        if action == Act::End {
            self.f[i].martial.elemental_hit = None;
            self.complete_attack(i);
        } else {
            let (hit, direction) = self.f[i]
                .martial
                .elemental_hit
                .as_mut()
                .expect("Elemental Strike target");
            *direction = Some(action);
            let defender = hit.defender;
            self.request(defender % 2, ELEMENTAL_SAVE);
        }
    }

    fn resolve_elemental_move(&mut self, roll_save: bool) {
        use crate::arena2::geometry::{Point, Pos};
        let (hit, action) = self
            .f
            .iter_mut()
            .find_map(|fighter| fighter.martial.elemental_hit.take())
            .expect("Elemental Strike target");
        let i = hit.attacker;
        let action = action.expect("Elemental Strike direction");
        let dc = 8 + self.f[i].build.pb + self.f[i].build.stats[4];
        if !roll_save || !self.martial_save(hit.defender, 0, dc) {
            let from = self.fighter_point(i);
            let mut target = self.creature_point(hit.defender).expect("living target");
            let direction = if (action as usize) < 2 { 1 } else { -1 };
            for _ in 0..(action as usize % 2 + 1) {
                let dx = (target.cell.x - from.cell.x).signum() * direction;
                let dy = (target.cell.y - from.cell.y).signum() * direction;
                let dz = (target.altitude - from.altitude).signum() * direction;
                let next = Point {
                    cell: Pos {
                        x: target.cell.x + dx,
                        y: target.cell.y + dy,
                    },
                    altitude: target.altitude + dz,
                };
                if !self.map.can_aim_point(target, next)
                    || self
                        .footprint_fit_kind(
                            hit.defender,
                            next,
                            self.combatant_size(hit.defender),
                            false,
                            true,
                        )
                        .is_none()
                {
                    break;
                }
                if hit.defender < 2 {
                    self.commit_forced_move(hit.defender, next);
                } else if !self.entity_force_move(hit.defender, next) {
                    break;
                }
                let Some(actual) = self.creature_point(hit.defender) else {
                    break;
                };
                if actual != next {
                    break;
                }
                target = next;
            }
        }
        self.complete_attack(i);
    }

    pub(in crate::arena2) fn offer_hand_of_harm(&mut self, hit: Hit, damage: i32) -> bool {
        let i = hit.attacker;
        if hit.unarmed
            && damage > 0
            && self.has_martial_subclass(i, Class::Monk, 1)
            && self.f[i].resource > 0
            && self.f[i].martial.harm_turn != self.turns
        {
            self.request(i, HAND_OF_HARM);
            true
        } else {
            false
        }
    }

    pub(in crate::arena2) fn hand_of_harm_damage(&mut self, hit: Hit) -> i32 {
        let i = hit.attacker;
        self.f[i].resource -= 1;
        self.f[i].martial.harm_turn = self.turns;
        let die = self.martial_arts_die(i);
        let mut extra = self.roll(die) + self.f[i].build.stats[4];
        if hit.crit {
            extra += self.roll(die);
        }
        self.damage_after_resistance(hit.defender, extra, DamageType::Necrotic)
    }

    fn mercy_heal_targets(&self, i: usize) -> u128 {
        (0..4)
            .filter(|&target| {
                self.observed_footprint(i, target).is_some()
                    && self
                        .combatant_distance(i, target)
                        .is_some_and(|distance| distance <= 1)
            })
            .fold(0, |mask, target| mask | ACTS[target].bit())
    }

    pub(super) fn subclass_monk_mask(&self, i: usize) -> u128 {
        if self.has_martial_subclass(i, Class::Monk, 1)
            && self.action
            && self.f[i].resource > 0
            && self.mercy_heal_targets(i) != 0
        {
            Act::Martial1.bit()
        } else {
            0
        }
    }

    pub(super) fn mercy_flurry_heal_available(&self, i: usize) -> bool {
        self.has_martial_subclass(i, Class::Monk, 1)
            && !self.f[i].martial.mercy_heal_flurry
            && self.f[i]
                .martial
                .bonus_unarmed
                .is_some_and(|sequence| sequence.flurry)
            && self.mercy_heal_targets(i) != 0
    }

    pub(super) fn start_mercy_heal(&mut self, i: usize, flurry: bool) {
        self.f[i].martial.mercy_heal_flurry = flurry;
        self.request(i, HAND_OF_HEALING);
    }

    fn resolve_mercy_heal(&mut self, i: usize, action: Act) {
        let flurry = self.f[i].martial.mercy_heal_flurry;
        if action == Act::End {
            if flurry {
                self.f[i].martial.mercy_heal_flurry = false;
                self.request(i, super::unarmed::UNARMED_MODE);
            }
            return;
        }
        if flurry {
            self.pay_mercy_flurry_strike(i);
        } else {
            self.action = false;
            self.f[i].resource -= 1;
        }
        let target = action as usize;
        let healing = self.roll(self.martial_arts_die(i)) + self.f[i].build.stats[4];
        if target < 2 {
            self.heal(target, healing);
        } else {
            self.entity_heal(target, healing);
        }
        if flurry {
            if !self.martial_continue_flurry(i) {
                self.finish_martial_unarmed(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{class::VARIANTS_PER_CLASS, geometry::Pos};

    fn duel(seed: u64, class: Class, variant: u8, distance: i8) -> Option<Battle> {
        let id = |class: Class, variant: u8| (class as usize * VARIANTS_PER_CLASS) as u8 + variant;
        let mut battle =
            Battle::new_roster_in(seed, [id(class, variant), id(Class::Fighter, 0)], 0);
        if battle.owner != 0 {
            return None;
        }
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: distance, y: 0 };
        Some(battle)
    }

    #[test]
    fn zealot_spends_healing_dice_and_chooses_divine_fury_type_on_first_hit() {
        let mut duel = (0..100)
            .filter_map(|seed| duel(seed, Class::Barbarian, 1, 1))
            .find_map(|mut battle| {
                battle.step(Act::Special1).unwrap();
                battle.step(Act::Melee).unwrap();
                if battle.phase_id() == super::super::hits::RECKLESS {
                    battle.step(Act::Pass).unwrap();
                }
                (battle.phase_id() == DIVINE_FURY).then_some(battle)
            })
            .expect("a raging Zealot hits");
        assert_eq!(
            duel.mask() & (Act::N.bit() | Act::NE.bit()),
            Act::N.bit() | Act::NE.bit()
        );
        let before = duel.f[1].hp;
        duel.step(Act::NE).unwrap();
        assert!(duel.f[1].hp < before);
        assert_eq!(duel.f[0].martial.frenzy_turn, duel.turns);
        duel.step(Act::Melee).unwrap();
        assert_ne!(duel.phase_id(), DIVINE_FURY);

        let mut injury = (0..100)
            .find_map(|seed| self::duel(seed, Class::Barbarian, 1, 1))
            .unwrap();
        let maximum = injury.f[0].hp;
        injury.f[0].hp -= 25;
        injury.step(Act::Martial1).unwrap();
        assert_eq!(injury.phase_id(), WARRIOR_OF_GODS);
        injury.step(Act::NE).unwrap();
        assert_eq!(injury.f[0].martial.subclass_dice, 2);
        assert!((maximum - 23..maximum).contains(&injury.f[0].hp));
        assert_eq!(injury.mask() & Act::Martial1.bit(), 0);
    }

    #[test]
    fn world_tree_grants_self_temp_hp_and_expires_its_other_creatures_grants() {
        let mut battle = (0..100)
            .find_map(|seed| duel(seed, Class::Barbarian, 2, 2))
            .unwrap();
        battle.step(Act::Special1).unwrap();
        assert_eq!(battle.f[0].temp, 5);
        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap();
        assert_eq!(battle.phase_id(), LIFE_GIVING_FORCE);
        battle.step(Act::NE).unwrap();
        assert!((2..=12).contains(&battle.f[1].temp));
        battle.step(Act::End).unwrap();
        assert_eq!(battle.f[1].temp, 0);
        assert_eq!(battle.f[0].temp, 5);
    }

    #[test]
    fn elemental_attunement_reaches_fifteen_feet_and_offers_typed_damage() {
        let mut battle = (0..100)
            .filter_map(|seed| duel(seed, Class::Monk, 2, 3))
            .find_map(|mut battle| {
                assert_eq!(battle.phase_id(), ELEMENTAL_ATTUNEMENT);
                battle.step(Act::Rider).unwrap();
                assert_eq!(battle.unarmed_reach(0), 3);
                assert_eq!(battle.f[0].resource, 4);
                battle.step(Act::Unarmed).unwrap();
                (battle.phase_id() == ELEMENTAL_DAMAGE).then_some(battle)
            })
            .expect("an attuned Monk lands a 15-foot Unarmed Strike");
        battle.step(Act::E).unwrap();
        if battle.phase_id() == super::super::hits::STUN {
            battle.step(Act::Pass).unwrap();
        }
        assert_eq!(battle.phase_id(), ELEMENTAL_MOVE);
        battle.step(Act::NE).unwrap();
        assert_eq!(battle.phase_id(), ELEMENTAL_SAVE);
        assert_eq!(battle.actor(), Some(1));
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.phase_id(), 0);
        assert_eq!(battle.dist(), 5);
    }

    #[test]
    fn elemental_reach_triggers_an_opportunity_at_its_outer_edge() {
        let mut battle = (0..100)
            .find_map(|seed| duel(seed, Class::Monk, 2, 3))
            .unwrap();
        battle.step(Act::Rider).unwrap();
        battle.step(Act::End).unwrap();
        assert_eq!(battle.actor(), Some(1));
        battle.step(Act::E).unwrap();
        assert_eq!(battle.actor(), Some(0));
        assert_ne!(battle.mask() & Act::Opportunity.bit(), 0);
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.dist(), 4);
    }

    #[test]
    fn mercy_can_replace_one_flurry_strike_with_healing() {
        let mut battle = (0..100)
            .find_map(|seed| duel(seed, Class::Monk, 1, 1))
            .unwrap();
        battle.f[0].hp -= 20;
        let injury = battle.f[0].hp;
        battle.step(Act::Martial5).unwrap();
        battle.step(Act::Martial1).unwrap();
        assert_eq!(battle.phase_id(), HAND_OF_HEALING);
        battle.step(Act::N).unwrap();
        assert!(battle.f[0].hp > injury);
        assert_eq!(battle.f[0].resource, 4);
        assert_eq!(battle.phase_id(), super::super::unarmed::UNARMED_MODE);
        assert_eq!(battle.mask() & Act::Martial1.bit(), 0);
        battle.step(Act::Pass).unwrap();
        assert_eq!(battle.mask() & Act::Martial5.bit(), 0);
    }

    #[test]
    fn mercy_spends_focus_for_harm_on_only_one_damaging_hit_per_turn() {
        let mut battle = (0..100)
            .filter_map(|seed| duel(seed, Class::Monk, 1, 1))
            .find_map(|mut battle| {
                battle.step(Act::Unarmed).unwrap();
                if battle.phase_id() == super::super::hits::STUN {
                    battle.step(Act::Pass).unwrap();
                }
                (battle.phase_id() == HAND_OF_HARM).then_some(battle)
            })
            .expect("a Mercy Monk lands a damaging strike");
        let initial_hp = battle.f[1].hp;
        battle.step(Act::Rider).unwrap();
        assert!(battle.f[1].hp < initial_hp);
        assert_eq!(battle.f[0].resource, 4);
        assert_eq!(battle.f[0].martial.harm_turn, battle.turns);
        battle.step(Act::Unarmed).unwrap();
        if battle.phase_id() == super::super::hits::STUN {
            battle.step(Act::Pass).unwrap();
        }
        assert_ne!(battle.phase_id(), HAND_OF_HARM);
        assert_eq!(battle.f[0].resource, 4);
    }

    #[test]
    fn elementalism_targets_a_cube_without_spending_focus_or_changing_combat() {
        let mut battle = (0..100)
            .find_map(|seed| duel(seed, Class::Monk, 2, 3))
            .unwrap();
        battle.step(Act::End).unwrap(); // Decline the opening Elemental Attunement.
        let aim = |battle: &mut Battle| {
            battle.step(Act::Magic9).unwrap();
            battle.step(Act::N).unwrap();
            battle.step(Act::N).unwrap();
            if battle.phase_id() != ELEMENTALISM_MODE {
                battle.step(Act::N).unwrap();
            }
            assert_eq!(battle.phase_id(), ELEMENTALISM_MODE);
        };
        let hp = [battle.f[0].hp, battle.f[1].hp];
        let before = battle.surface_tile(Pos { x: 0, y: 0 });
        aim(&mut battle);
        assert_eq!(battle.mask() & Act::SW.bit(), 0);
        battle.step(Act::NE).unwrap();
        assert_eq!([battle.f[0].hp, battle.f[1].hp], hp);
        assert_eq!(battle.surface_tile(Pos { x: 0, y: 0 }), before);
        assert_eq!(battle.f[0].resource, 5);
        assert!(!battle.action);
        assert!(battle
            .magic_public_summary(1, 0)
            .contains("thin dust on surfaces at A1"));

        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap(); // Decline Elemental Attunement on the new turn.
        aim(&mut battle);
        assert_ne!(battle.mask() & Act::SW.bit(), 0);
        battle.step(Act::SW).unwrap();
        let summary = battle.magic_public_summary(1, 0);
        assert!(summary.contains("a small sculpted figure at A1"));
        assert!(!summary.contains("thin dust"));
    }

    #[test]
    fn readied_elementalism_selects_the_effect_after_the_reaction_trigger() {
        use crate::arena2::common::{ReadyTrigger, READY_TRIGGERS};
        let mut battle = (0..100)
            .find_map(|seed| duel(seed, Class::Monk, 2, 3))
            .unwrap();
        battle.step(Act::End).unwrap(); // Decline Elemental Attunement.
        let move_trigger = READY_TRIGGERS
            .iter()
            .position(|&trigger| trigger == ReadyTrigger::Move)
            .unwrap();
        battle.step(Act::Ready).unwrap();
        battle.step(ACTS[move_trigger]).unwrap();
        battle.step(Act::Magic9).unwrap();
        assert_eq!(
            battle.f[0].concentration,
            Some(crate::arena2::magic::Concentration::ReadiedSpell)
        );
        assert!(!battle.action);
        battle.step(Act::End).unwrap();

        battle.step(Act::E).unwrap();
        assert_eq!(battle.actor(), Some(0));
        let reaction = if battle.mask() & Act::Ready.bit() != 0 {
            Act::Ready
        } else {
            Act::Rider
        };
        battle.step(reaction).unwrap();
        battle.step(Act::N).unwrap();
        battle.step(Act::N).unwrap();
        if battle.phase_id() != ELEMENTALISM_MODE {
            battle.step(Act::N).unwrap();
        }
        assert_eq!(battle.phase_id(), ELEMENTALISM_MODE);
        battle.step(Act::SE).unwrap();
        assert_eq!(battle.actor(), Some(1));
        assert!(!battle.f[0].reaction);
        assert!(battle.action);
        assert!(battle.f[0].concentration.is_none());
        assert!(battle
            .magic_public_summary(1, 0)
            .contains("damp mist at A1"));
    }
}
