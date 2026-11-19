//! Readied spells commit their slot and components now, then choose a legal
//! aim from the caster's position when the trigger resolves.
use super::*;
use crate::arena2::geometry::{HEIGHT, MAX_ELEVATION, WIDTH};

const RANK: i32 = 107;
const OPTION: i32 = 108;
const ROW: i32 = 109;
const COLUMN: i32 = 110;
const ALTITUDE: i32 = 111;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OptionKind {
    Direction,
    CubeFace,
    Element,
    Form,
    Word,
    Skill,
    Ability,
    Creatures,
    CureRoute,
    Group,
    Allies,
    Consent { caster: usize, recipients: u8 },
    Dispel,
    LightObject,
    LightColor,
    WeaponType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadyMode {
    Rank(Act),
    TrueStrikeWeapon,
    Options(OptionKind),
    Row,
    Column(i8),
    Altitude(Pos),
}

#[derive(Clone, Default, Debug)]
pub(super) struct ReadySpellState {
    pub(super) mode: Option<ReadyMode>,
    true_strike_weapon: Option<(usize, usize)>,
    psychic: bool,
}

impl Spell {
    fn readied_point(self) -> Option<(i32, bool)> {
        use Spell::*;
        match self {
            Fireball | HungerOfHadar | PlantGrowth => Some((30, false)),
            FaerieFire => Some((12, false)),
            Elementalism => Some((6, false)),
            Shatter | ZoneOfTruth | CalmEmotions | Sleep => Some((12, false)),
            Moonbeam | HypnoticPattern | FogCloud => Some((24, false)),
            Entangle | StinkingCloud => Some((18, self == Entangle)),
            _ => None,
        }
    }
}

impl Battle {
    pub(in crate::arena2) fn magic_prepare_mask(&self, i: usize) -> u128 {
        if self.f[i].magic.form || self.f[i].raging || !self.action {
            return 0;
        }
        SPELL_ACTIONS
            .iter()
            .chain(&SPECIALS)
            .filter_map(|&a| {
                let s = self.prepared_spell(i, a)?;
                (!s.bonus()
                    && !matches!(s, Spell::Revivify | Spell::Counterspell | Spell::Shield | Spell::Alarm)
                    && self.f[i].build.level >= self.spell_unlock_level(i, s)
                    && self.magic_components(i, s)
                    && (s != Spell::TrueStrike || self.true_strike_weapon_mask(i) != 0)
                    && self.spell_slot(i, s, 1).is_some())
                .then_some(a.bit())
            })
            .fold(0, |mask, bit| mask | bit)
    }

    pub(in crate::arena2) fn magic_prepare(&mut self, i: usize, a: Act) -> bool {
        if self.magic_prepare_mask(i) & a.bit() == 0 {
            return false;
        }
        let s = self.prepared_spell(i, a).expect("ready spell");
        if s == Spell::TrueStrike {
            self.ready_spell_request(i, ReadyMode::TrueStrikeWeapon, OPTION);
            return true;
        }
        let slots = self.castable_slot_mask(i, s);
        if slots.count_ones() > 1 {
            self.ready_spell_request(i, ReadyMode::Rank(a), RANK);
        } else {
            let tier = self.spell_slot(i, s, 1).expect("available ready spell");
            self.ready_spell_commit(i, s, tier);
        }
        true
    }

    fn ready_spell_commit(&mut self, i: usize, s: Spell, tier: usize) {
        let prior = self.f[i].slot_turn;
        if tier > 0 {
            assert!(self.spend_slot(i, tier));
        }
        self.action = false;
        self.magic_offensive(i);
        self.magic_end_invisibility_for(i);
        self.f[i].magic.ready_state.psychic = self.arcane_choose_psychic(i, s);
        let (v, so, m) = self.casting_components(i, s);
        self.consume_material(i, m);
        if v {
            self.reveal(i);
        }
        self.concentrate(i, Concentration::ReadiedSpell, 1, None, tier);
        self.f[i].magic.readied = Some((s, tier));
        if v || so || m != crate::arena2::equipment::Material::None {
            self.notify_readied_cast(i);
            self.magic_offer_readied_counterspell(i, tier, prior);
        }
    }

    pub(in crate::arena2) fn magic_readied_valid(&self, i: usize) -> bool {
        self.f[i].concentration == Some(Concentration::ReadiedSpell)
            && self.f[i].magic.readied.is_some_and(|(s, _)| {
                if s == Spell::DispelMagic {
                    self.dispel_targets(i) != 0
                } else if s.is_ally_support() {
                    self.magic_ally_candidates(i, s) != 0
                } else if matches!(s, Spell::ShockingGrasp | Spell::PoisonSpray) {
                    self.direct_attack_spell_mask(i, s, s.range().unwrap_or(1)) != 0
                } else if s == Spell::TrueStrike {
                    self.f[i]
                        .magic
                        .ready_state
                        .true_strike_weapon
                        .is_some_and(|(issuer, mode)| {
                            self.gear_weapon_owner(i, mode == 1) == issuer
                                && self.true_strike_weapon_mask(i) & ACTS[mode].bit() != 0
                        })
                } else if s.chooses_creature() || s.is_group_targeted() {
                    self.creature_choices(i, s) != 0
                } else {
                    true
                }
            })
    }

    pub(in crate::arena2) fn magic_release(&mut self, i: usize, _choice: Act) {
        if !self.magic_readied_valid(i) {
            return;
        }
        self.f[i].concentration = None;
        let (s, tier) = self.f[i].magic.readied.expect("ready spell");
        if s.readied_point().is_some() {
            self.ready_spell_request(i, ReadyMode::Row, ROW);
        } else {
            use OptionKind::*;
            use Spell::*;
            let options = match s {
                Thunderwave | BurningHands | Fear | GustOfWind | LightningBolt => Some(Direction),
                ChromaticOrb | Resistance | ProtectionFromEnergy => Some(Element),
                AlterSelf => Some(Form),
                Command => Some(Word),
                Guidance => Some(Skill),
                EnhanceAbility => Some(Ability),
                TrueStrike => Some(WeaponType),
                Light => Some(LightObject),
                DispelMagic => Some(Dispel),
                s if s.is_group_targeted() => Some(Group),
                s if s.is_ally_support() => Some(Allies),
                s if s.chooses_creature() => Some(Creatures),
                _ => None,
            };
            if let Some(option) = options {
                self.ready_option_or_resolve(i, option);
            } else {
                self.finish_readied_spell(i, s, tier, None);
            }
        }
    }

    fn ready_spell_request(&mut self, i: usize, mode: ReadyMode, id: i32) {
        self.f[i].magic.ready_state.mode = Some(mode);
        self.request(i, id);
    }

    fn ready_option_or_resolve(&mut self, i: usize, kind: OptionKind) {
        self.ready_spell_request(i, ReadyMode::Options(kind), OPTION);
    }

    fn legal_readied_point(&self, i: usize, s: Spell, p: Point) -> bool {
        let Some((range, floor)) = s.readied_point() else {
            return false;
        };
        let from = self.divine_spell_origin(i);
        (!floor || p.altitude == 0) && from.dist(p) <= range && self.map.can_aim_point(from, p)
    }

    fn legal_readied_altitudes(&self, i: usize, s: Spell, cell: Pos) -> u128 {
        (0..=MAX_ELEVATION)
            .filter(|&altitude| self.legal_readied_point(i, s, Point { cell, altitude }))
            .fold(0, |mask, h| mask | (1 << h))
    }

    pub(in crate::arena2) fn ready_spell_mask(&self, i: usize, _phase: i32) -> u128 {
        let Some(mode) = self.f[i].magic.ready_state.mode else {
            return 0;
        };
        if mode == ReadyMode::TrueStrikeWeapon {
            return self.true_strike_weapon_mask(i);
        }
        if let ReadyMode::Rank(a) = mode {
            return self
                .castable_slot_mask(i, self.prepared_spell(i, a).expect("slotted spell"));
        }
        if let ReadyMode::Options(OptionKind::Consent { recipients, .. }) = mode {
            return self.magic_ally_consent_mask(i, recipients);
        }
        let Some((s, tier)) = self.f[i].magic.readied else {
            return 0;
        };
        let bits = |n: usize| (1u128 << n) - 1;
        use OptionKind::*;
        match mode {
            ReadyMode::Options(Direction) => bits(26),
            ReadyMode::Options(CubeFace) => bits(25),
            ReadyMode::Options(Element) => bits(s.element_choices().len()),
            ReadyMode::Options(Form) => bits(FORMS.len()),
            ReadyMode::Options(Word) => bits(WORDS.len()),
            ReadyMode::Options(Skill) => bits(GUIDANCE_SKILLS.len()),
            ReadyMode::Options(Ability) => bits(6),
            ReadyMode::Options(WeaponType) => bits(2),
            ReadyMode::Options(LightObject) => self.magic_light_targets(i),
            ReadyMode::Options(LightColor) => bits(LIGHT_HUES.len()),
            ReadyMode::Options(Dispel) => self.dispel_targets(i),
            ReadyMode::Options(CureRoute) => bits(2),
            ReadyMode::Options(Creatures) => self.creature_choices(i, s),
            ReadyMode::Options(Group) => {
                let available = self.creature_choices(i, s) as u8;
                let count = match s {
                    Spell::Bane => tier + 2,
                    Spell::HoldPerson => tier.saturating_sub(1),
                    _ => tier,
                };
                (1u8..16)
                    .filter(|&set| set & !available == 0 && set.count_ones() as usize <= count)
                    .fold(0, |mask, set| mask | (1u128 << (32 + set)))
            }
            ReadyMode::Options(Allies) => {
                let available = self.magic_ally_candidates(i, s);
                (1u8..16)
                    .filter(|&set| {
                        set & !available == 0
                            && set.count_ones() as usize <= ready_support_limit(s, tier)
                    })
                    .fold(0, |mask, set| mask | (1u128 << (32 + set)))
            }
            ReadyMode::Row => (0..HEIGHT)
                .filter(|&y| {
                    (0..WIDTH).any(|x| self.legal_readied_altitudes(i, s, Pos { x, y }) != 0)
                })
                .fold(0, |mask, y| mask | (1 << y)),
            ReadyMode::Column(y) => (0..WIDTH)
                .filter(|&x| self.legal_readied_altitudes(i, s, Pos { x, y }) != 0)
                .fold(0, |mask, x| mask | (1 << x)),
            ReadyMode::Altitude(cell) => self.legal_readied_altitudes(i, s, cell),
            _ => 0,
        }
    }

    pub(in crate::arena2) fn ready_spell_step(&mut self, i: usize, _phase: i32, a: Act) {
        let mode = self.f[i]
            .magic
            .ready_state
            .mode
            .take()
            .expect("pending ready spell choice");
        let index = a as usize;
        if mode == ReadyMode::TrueStrikeWeapon {
            let issuer = self.gear_weapon_owner(i, index == 1);
            self.f[i].magic.ready_state.true_strike_weapon = Some((issuer, index));
            return self.ready_spell_commit(i, Spell::TrueStrike, 0);
        }
        if let ReadyMode::Rank(action) = mode {
            return self.ready_spell_commit(
                i,
                self.prepared_spell(i, action).expect("slotted spell"),
                index + 1,
            );
        }
        if let ReadyMode::Options(OptionKind::Consent { caster, recipients }) = mode {
            let (s, tier) = self.f[caster]
                .magic
                .readied
                .expect("released support spell");
            let allowed = Self::accepted_magic_ally_subset(i, recipients, a);
            if allowed == 0 {
                self.f[caster].magic.readied = None;
                return;
            }
            self.f[caster].magic.ally_selection = allowed;
            return self.finish_readied_spell(caster, s, tier, None);
        }
        let (s, tier) = self.f[i].magic.readied.expect("readied spell");
        use OptionKind::*;
        use ReadyMode::*;
        match mode {
            Row => return self.ready_spell_request(i, Column(index as i8), COLUMN),
            Column(y) => {
                return self.ready_spell_request(i, Altitude(Pos { x: index as i8, y }), ALTITUDE)
            }
            Altitude(cell) => {
                return self.finish_readied_spell(
                    i,
                    s,
                    tier,
                    Some(Point {
                        cell,
                        altitude: index as i8,
                    }),
                )
            }
            Options(Direction) => {
                self.f[i].magic.direction = index;
                if s == Spell::Thunderwave {
                    return self.ready_option_or_resolve(i, CubeFace);
                }
            }
            Options(CubeFace) => {
                self.f[i].magic.cube_offset = crate::arena2::targeting::cube_face_offset(index)
            }

            Options(Element) => self.f[i].magic.element = s.element_choices()[index].0,
            Options(Form) => self.f[i].magic.form_option = index,
            Options(Word) => {
                self.f[i].magic.chosen_word = WORDS[index];
                return self.ready_option_or_resolve(i, Group);
            }
            Options(Skill) => {
                self.f[i].magic.guidance_skill = GUIDANCE_SKILLS[index];
                return self.ready_option_or_resolve(i, Allies);
            }
            Options(Ability) => {
                self.f[i].magic.enhanced_ability = index;
                return self.ready_option_or_resolve(i, Allies);
            }
            Options(WeaponType) => {
                let (_, weapon_mode) = self.f[i]
                    .magic
                    .ready_state
                    .true_strike_weapon
                    .take()
                    .expect("committed True Strike weapon");
                self.select_true_strike_weapon(i, weapon_mode);
                self.f[i].magic.true_strike_radiant = index == 1;
            }
            Options(Dispel) => self.f[i].magic.aux_choice = index,
            Options(CureRoute) => self.f[i].magic.cure_via_familiar = Some(index == 1),
            Options(Creatures) => {
                self.f[i].magic.cast_target = Some(index);
                if s == Spell::CureWounds {
                    let (direct, familiar) = self.cure_paths(i, index);
                    if direct && familiar {
                        return self.ready_option_or_resolve(i, CureRoute);
                    }
                    self.f[i].magic.cure_via_familiar = Some(familiar && !direct);
                }

                if s == Spell::Suggestion {
                    return self.start_suggestion_cast(i, index);
                }
            }
            Options(Group) => self.f[i].magic.cast_targets = (index - 32) as u8,
            Options(Allies) => {
                let recipients = (index - 32) as u8;
                let other = 1 - i;
                if ready_support_consent(s) && recipients & ((1 << other) | (1 << (other + 2))) != 0
                {
                    return self.ready_spell_request(
                        other,
                        Options(Consent {
                            caster: i,
                            recipients,
                        }),
                        OPTION,
                    );
                }
                self.f[i].magic.ally_selection = recipients;
            }
            Options(LightObject) => {
                self.f[i].magic.light_target = index;
                return self.ready_option_or_resolve(i, LightColor);
            }
            Options(LightColor) => self.f[i].magic.light_color = index,
            _ => unreachable!("resolved ready spell choices"),
        }
        if matches!(mode, Options(Element | Form)) && s.is_ally_support() {
            return self.ready_option_or_resolve(i, Allies);
        }
        self.finish_readied_spell(i, s, tier, None);
    }

    pub(super) fn finish_readied_suggestion(&mut self, i: usize) {
        let (s, tier) = self.f[i].magic.readied.expect("readied Suggestion");
        self.finish_readied_spell(i, s, tier, None);
    }

    fn finish_readied_spell(&mut self, i: usize, s: Spell, tier: usize, center: Option<Point>) {
        self.f[i].magic.readied = None;
        if s == Spell::Elementalism {
            self.begin_elementalism(i, center.expect("aimed readied Elementalism"), true);
            return;
        }
        self.f[i].magic.resolving_center = center;
        self.f[i].arcane.psychic = self.f[i].magic.ready_state.psychic;
        self.resolve_spell(i, s, tier, false);
        self.f[i].magic.resolving_center = None;
        self.f[i].magic.cast_target = None;
        self.f[i].magic.cure_via_familiar = None;
        self.f[i].magic.cast_targets = 0;
    }

    pub(in crate::arena2) fn ready_spell_label(
        &self,
        i: usize,
        _phase: i32,
        a: Act,
    ) -> Option<String> {
        let mode = self.f[i].magic.ready_state.mode?;
        let index = a as usize;
        use OptionKind::*;
        Some(match mode {
            ReadyMode::TrueStrikeWeapon => [
                "Ready with melee weapon",
                "Ready with ranged weapon",
                "Ready to throw held weapon",
            ]
            .get(index)?
            .to_string(),
            ReadyMode::Rank(action) => format!(
                "Ready {} using level {} slot",
                self.prepared_spell(i, action)?.label(),
                index + 1
            ),
            ReadyMode::Row => format!("Aim at row {}", index + 1),
            ReadyMode::Column(y) => format!("Aim at {}{}", char::from(b'A' + index as u8), y + 1),
            ReadyMode::Altitude(_) => format!("Aim at {} ft elevation", index * 5),
            ReadyMode::Options(direction) => match direction {
                Direction => ready_direction_label(index)?,
                CubeFace => {
                    let [right, up] = crate::arena2::targeting::cube_face_offset(index);
                    format!(
                        "Shift cube face right {:+.1} ft, up {:+.1} ft",
                        right as f32 * 1.25,
                        up as f32 * 1.25
                    )
                }
                Element => self.f[i]
                    .magic
                    .readied?
                    .0
                    .element_choices()
                    .get(index)?
                    .1
                    .into(),
                Form => FORMS.get(index)?.to_string(),
                Word => format!("{:?}", WORDS.get(index)?),
                Skill => format!("{:?}", GUIDANCE_SKILLS.get(index)?),
                Ability => ["Strength", "Dexterity", "Constitution", "Intelligence", "Wisdom", "Charisma"]
                    .get(index)?.to_string(),
                WeaponType => {
                    if index == 0 {
                        "Weapon's normal damage".into()
                    } else {
                        "Radiant damage".into()
                    }
                }
                CureRoute => ["Touch target directly", "Deliver through your familiar"]
                    .get(index)?
                    .to_string(),
                Creatures => self.group_target_label(i, 1 << index),
                Dispel => self.dispel_target_label(i, index)?,
                Group | Allies => self.group_target_label(i, (index - 32) as u8),
                Consent { .. } => self.magic_ally_consent_label(i, a)?,
                LightObject => self.magic_light_target_label(i, index)?,
                LightColor => LIGHT_HUES.get(index)?.to_string(),
            },
        })
    }
}

fn ready_support_consent(spell: Spell) -> bool {
    use Spell::*;
    matches!(
        spell,
        Protection
            | MageArmor
            | Heroism
            | Fly
            | Guidance
            | Invisibility
            | Nondetection
            | EnhanceAbility
            | Resistance
            | ProtectionFromEnergy
            | Jump
    )
}
fn ready_support_limit(spell: Spell, tier: usize) -> usize {
    use Spell::*;
    match spell {
        Bless => tier + 2,
        Aid => 3,
        Heroism | Jump | Longstrider => tier,
        Fly => tier.saturating_sub(2),
        Invisibility | EnhanceAbility => tier.saturating_sub(1),
        _ => 1,
    }
}

impl Battle {
    /// Current readied spell, casting rank and private aim-selection context.
    pub fn ready_spell_features(&self, viewer: usize) -> [f32; 8] {
        let Some(f) = self.f.get(viewer) else {
            return [0.0; 8];
        };
        let mut row = [0.; 8];
        if let Some((spell, tier)) = f.magic.readied {
            row[0] = (spell as usize + 1) as f32 / 128.;
            row[1] = tier as f32 / 3.;
            row[2] = u8::from(f.concentration == Some(Concentration::ReadiedSpell)).into();
            if spell == Spell::TrueStrike {
                row[6] = f
                    .magic
                    .ready_state
                    .true_strike_weapon
                    .map_or(0., |(_, mode)| (mode + 1) as f32 / 3.);
            }
        }
        if self.actor() == Some(viewer) {
            if let Some(mode) = f.magic.ready_state.mode {
                let (kind, y, x, other) = match mode {
                    ReadyMode::Rank(a) => (1, 0, 0, (a as usize + 1) as f32 / 128.),
                    ReadyMode::Options(OptionKind::Consent { caster, recipients }) => {
                        use crate::arena2::target_ids::relative_id;

                        let caster = relative_id(viewer, caster);
                        let recipients = (0..4).fold(0, |mask, id| {
                            let bit = (recipients >> relative_id(viewer, id)) & 1;
                            mask | (bit << id)
                        });
                        (2, 0, 0, (caster + 1) as f32 / 2. + recipients as f32 / 32.)
                    }
                    ReadyMode::TrueStrikeWeapon | ReadyMode::Options(_) => (2, 0, 0, 0.),
                    ReadyMode::Row => (3, 0, 0, 0.),
                    ReadyMode::Column(y) => (4, y + 1, 0, 0.),
                    ReadyMode::Altitude(cell) => (5, cell.y + 1, cell.x + 1, 0.),
                };
                row[3] = kind as f32 / 5.;
                row[4] = y as f32 / HEIGHT as f32;
                row[5] = x as f32 / WIDTH as f32;
                if other != 0. {
                    row[6] = other;
                }
                row[7] = 1.;
            }
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readied_true_strike_commits_the_held_weapon_before_aiming() {
        let mut b = Battle::new(3, [Class::Wizard, Class::Fighter], [4, 4]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 1, y: 0 };
        let slots = b.f[0].slots;
        assert!(b.magic_prepare(0, Act::Magic4));
        assert_eq!(b.phase_id(), OPTION);
        assert!(b.action);
        assert!(b.f[0].magic.readied.is_none());
        b.step(Act::N).unwrap();
        assert!(!b.action);
        assert_eq!(b.f[0].slots, slots);
        assert!(b.magic_readied_valid(0));

        let mut stowed = b.clone();
        stowed.step(Act::Gear2).unwrap();
        assert!(!stowed.magic_readied_valid(0));

        b.owner = 1;
        b.magic_release(0, Act::Magic4);
        assert_eq!(b.phase_id(), OPTION);
        assert_eq!(b.choice_label(0, Act::NE), "Radiant damage");
        b.step(Act::NE).unwrap();
        assert_eq!(b.phase_id(), 80);
        assert!(b.f[0].magic.true_strike_radiant);
        assert_eq!(b.f[0].slots, slots);
    }

    #[test]
    fn readied_area_points_use_the_same_geometry_as_direct_casts() {
        let mut battle = Battle::new(0, [Class::Druid, Class::Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        let elevated = Point {
            cell: Pos { x: 1, y: 1 },
            altitude: 3,
        };
        assert!(battle.legal_readied_point(0, Spell::Moonbeam, elevated));
        assert!(!battle.legal_readied_point(0, Spell::Entangle, elevated));
        assert!(battle.legal_readied_point(0, Spell::Entangle, Point::ground(elevated.cell)));
    }

    #[test]
    fn ready_selects_rank_now_and_area_at_releasing_casters_location() {
        let mut b = Battle::new(3, [Class::Wizard, Class::Fighter], [5, 5]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 8, y: 0 };
        let hp = [b.f[0].hp, b.f[1].hp];
        let slots = b.f[0].slots;
        assert!(b.magic_prepare(0, Act::Magic5));
        assert_eq!(b.phase_id(), 107);
        assert_eq!(b.f[0].slots, slots);
        b.step(Act::NE).unwrap();
        assert_eq!(b.f[0].slots, [slots[0], slots[1] - 1, slots[2]]);
        assert!(b.magic_readied_valid(0));
        b.owner = 1;
        b.magic_release(0, Act::Magic5);
        assert_eq!(b.actor(), Some(0));
        assert_eq!(b.phase_id(), 109);
        b.step(Act::N).unwrap();
        b.step(ACTS[8]).unwrap();
        b.step(Act::N).unwrap();
        assert_eq!(b.f[0].hp, hp[0]);
        assert!(b.f[1].hp < hp[1]);
        assert_eq!(b.f[0].slots, [slots[0], slots[1] - 1, slots[2]]);
        assert!(!b.magic_readied_valid(0));
    }
}

fn ready_direction_label(index: usize) -> Option<String> {
    const FLAT: [&str; 8] = [
        "North",
        "North-East",
        "East",
        "South-East",
        "South",
        "South-West",
        "West",
        "North-West",
    ];
    Some(match index {
        0..=7 => FLAT[index].into(),
        8 => "Up".into(),
        9 => "Down".into(),
        10..=17 => format!("{} and up", FLAT[index - 10]),
        18..=25 => format!("{} and down", FLAT[index - 18]),
        _ => return None,
    })
}
