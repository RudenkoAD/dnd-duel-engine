//! Spell projectiles with independent targets, simultaneous darts and die-dependent leaps.
use super::{Battle, DamageType, Hit, Spell};
use crate::arena2::{
    class::Class,
    equipment::Material,
    geometry::{Point, Pos, HEIGHT, MAX_ELEVATION, WIDTH},
    Act,
};

const AIM: i32 = 80;
const SHIELD: i32 = 81;
const ROW: i32 = 82;
const COLUMN: i32 = 83;
const ALTITUDE: i32 = 84;
const CUT: i32 = 85;
const GUESS: usize = 126;
const ALL_DARTS: usize = 16;

#[derive(Clone, Debug, Default)]
pub(super) struct ProjectileState {
    spell: Option<Spell>,
    tier: usize,
    remaining: usize,
    targets: [usize; 5],
    assigned: usize,
    damage: [i32; 5],
    shield_answered: u8,
    sanctuary_checked: u8,
    sanctuary_blocked: u8,
    cut_index: usize,
    previous: Option<Point>,
    used: u128,
    d8_faces: u16,
    repeated_face: bool,
    guess_x: i8,
    guess_y: i8,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ProjectileMarks {
    glowing_until: [i32; crate::arena2::objects::END_OBJECTS],
    slow_until: [i32; 4],
    guiding_until: [i32; 4],
}

impl Battle {
    fn direct_spell_target(
        &self,
        i: usize,
        spell: Spell,
        target: usize,
        origin: Point,
        range: i32,
    ) -> Option<Point> {
        if !self.rope_same_space(i, target) || self.suggestion_charmed_by(i, target) {
            return None;
        }
        let reaches = |point: Point| origin.dist(point) <= range && self.map.los_at(origin, point);
        if target >= 4 {
            let point = self.observed_object(i, target)?.point;
            return (spell == Spell::TrueStrike && reaches(point)).then_some(point);
        }
        self.observed_footprint(i, target)?
            .squares()
            .find(|&point| reaches(point))
    }

    pub(super) fn direct_attack_spell_mask(&self, i: usize, spell: Spell, range: i32) -> u128 {
        let origin = self.divine_spell_origin(i);
        (0..crate::arena2::objects::END_OBJECTS)
            .filter(|&id| {
                self.direct_spell_target(i, spell, id, origin, range)
                    .is_some()
            })
            .fold(0, |mask, id| mask | (1 << id))
    }

    fn launch_true_strike(&mut self, i: usize, target: Option<usize>, point: Point) {
        let (normal, _) = self.true_strike_selected_range(i);
        let long_range = self.fighter_point(i).dist(point) > normal;
        let ranged = self.f[i].magic.true_strike_ranged;
        self.f[i].magic.attack = Some(Spell::TrueStrike);
        self.f[i].magic.cast_target = target;
        self.f[i].magic.attack_target = target;
        if self.f[i].magic.true_strike_thrown {
            self.true_strike_throw(i, target, point, long_range);
        } else if let Some(target) = target {
            self.true_strike_attack(i, target, ranged, long_range);
        } else {
            self.true_strike_miss(i, ranged);
        }
    }

    pub(in crate::arena2) fn projectile_hit(&mut self, source: usize, target: usize) {
        if source >= 2 {
            return;
        }
        let start = self.turns + if self.owner == source { 2 } else { 1 };
        let spell = self.f[source].magic.projectiles.spell;
        let marks = &mut self.f[source].magic.projectile_marks;
        match (spell, target) {
            (Some(Spell::StarryWisp), _) if target < crate::arena2::objects::END_OBJECTS => {
                marks.glowing_until[target] = start + 1
            }
            (Some(Spell::RayOfFrost), 0..=3) => marks.slow_until[target] = start,
            (Some(Spell::GuidingBolt), 0..=3) => marks.guiding_until[target] = start + 1,
            (Some(Spell::FireBolt), 4..) => {
                self.ignite_object(target);
            }
            _ => {}
        }
    }
    pub(in crate::arena2) fn frost_slow(&self, target: usize) -> i32 {
        if self.f.iter().any(|f| {
            f.magic
                .projectile_marks
                .slow_until
                .get(target)
                .is_some_and(|&end| end > self.turns)
        }) {
            2
        } else {
            0
        }
    }
    pub(in crate::arena2) fn projectile_glowing(&self, target: usize) -> bool {
        self.faerie_outlined(target) || self.f.iter().any(|f| {
            f.magic
                .projectile_marks
                .glowing_until
                .get(target)
                .is_some_and(|&end| end > self.turns)
        })
    }
    pub(in crate::arena2) fn guiding_advantage(&self, target: usize) -> bool {
        self.f.iter().any(|f| {
            f.magic
                .projectile_marks
                .guiding_until
                .get(target)
                .is_some_and(|&end| end > self.turns)
        })
    }
    pub(in crate::arena2) fn take_guiding_advantage(&mut self, target: usize) -> bool {
        let active = self.guiding_advantage(target);
        for f in &mut self.f {
            if let Some(end) = f.magic.projectile_marks.guiding_until.get_mut(target) {
                *end = 0;
            }
        }
        active
    }
    pub(in crate::arena2) fn magic_begin_projectile(
        &mut self,
        i: usize,
        spell: Spell,
        tier: usize,
    ) -> bool {
        let remaining = match spell {
            Spell::EldritchBlast => {
                if self.f[i].build.level >= 5 {
                    2
                } else {
                    1
                }
            }
            Spell::ScorchingRay => tier + 1,
            Spell::StarryWisp
            | Spell::FireBolt
            | Spell::RayOfFrost
            | Spell::GuidingBolt
            | Spell::ShockingGrasp
            | Spell::PoisonSpray
            | Spell::TrueStrike => 1,
            Spell::ChromaticOrb => tier + 1,
            Spell::MagicMissile => tier + 2,
            _ => return false,
        };
        self.f[i].magic.projectiles = ProjectileState {
            spell: Some(spell),
            tier,
            remaining,
            ..Default::default()
        };
        self.f[i].magic.rays = 0;
        self.request(i, AIM);
        true
    }

    fn aim_origin_range(&self, i: usize) -> (Point, i32) {
        let p = &self.f[i].magic.projectiles;
        if let Some(previous) = p.previous.filter(|_| p.spell == Some(Spell::ChromaticOrb)) {
            return (previous, 6);
        }
        (
            self.divine_spell_origin(i),
            match p.spell {
                Some(Spell::ChromaticOrb) => 18,
                Some(Spell::StarryWisp | Spell::RayOfFrost) => 12,
                Some(Spell::ShockingGrasp) => 1,
                Some(Spell::PoisonSpray) => 6,
                Some(Spell::TrueStrike) => self.true_strike_selected_range(i).1,
                Some(Spell::EldritchBlast) if self.f[i].build.level >= 5 => {
                    24 + 6 * self.f[i].build.level
                }
                _ => 24,
            },
        )
    }

    fn projectile_reaches(&self, i: usize, target: Point) -> bool {
        let (origin, range) = self.aim_origin_range(i);
        self.map.can_occupy(target)
            && origin.dist(target) <= range
            && self.map.los_at(origin, target)
    }

    pub(in crate::arena2) fn projectile_eligible(&self, i: usize, target: usize) -> Option<Point> {
        let p = &self.f[i].magic.projectiles;
        if let Some(spell @ (Spell::ShockingGrasp | Spell::PoisonSpray | Spell::TrueStrike)) =
            p.spell
        {
            let (origin, range) = self.aim_origin_range(i);
            return self.direct_spell_target(i, spell, target, origin, range);
        }
        if self.suggestion_charmed_by(i, target) {
            return None;
        }
        if !self.rope_same_space(i, target) {
            return None;
        }
        if p.spell == Some(Spell::ChromaticOrb) && p.used & (1 << target) != 0 {
            return None;
        }
        if target >= 4 {
            if matches!(
                p.spell,
                Some(Spell::MagicMissile | Spell::GuidingBolt | Spell::RayOfFrost)
            ) {
                return None;
            }
            let obj = self.observed_object(i, target)?;
            return self.projectile_reaches(i, obj.point).then_some(obj.point);
        }
        if p.spell == Some(Spell::MagicMissile) {
            if p.sanctuary_blocked & (1 << target) != 0 {
                return None;
            }
            let visible = if target < 2 {
                self.can_see(i, target)
            } else {
                self.entity_visible(i, target)
            };
            if !visible {
                return None;
            }
        }
        self.observed_footprint(i, target)?
            .squares()
            .find(|&point| self.projectile_reaches(i, point))
    }

    fn projectile_aim_mask(&self, i: usize) -> u128 {
        let p = &self.f[i].magic.projectiles;
        let mut mask = 0;
        for id in 0..crate::arena2::objects::END_OBJECTS {
            if self.projectile_eligible(i, id).is_some() {
                mask |= 1 << id;
                if p.spell == Some(Spell::MagicMissile) && p.remaining > 1 {
                    mask |= 1 << (ALL_DARTS + id);
                }
            }
        }
        if p.spell != Some(Spell::MagicMissile)
            && self.rope_space(i).is_none()
            && (self.observed_pos(i, 1 - i).is_none() || self.observed_entity(i, 3 - i).is_none())
        {
            mask |= 1 << GUESS;
        }
        if p.spell == Some(Spell::ChromaticOrb) && p.previous.is_some() {
            mask |= Act::Pass.bit();
        }
        mask
    }

    fn projectile_altitudes(&self, i: usize, cell: Pos) -> u128 {
        (0..=MAX_ELEVATION)
            .filter(|&altitude| self.projectile_reaches(i, Point { cell, altitude }))
            .fold(0, |mask, h| mask | (1 << h))
    }

    fn missile_owner(&self) -> usize {
        (0..2)
            .find(|&s| self.f[s].magic.projectiles.spell == Some(Spell::MagicMissile))
            .expect("Missile reaction is attached to an active cast")
    }

    pub(in crate::arena2) fn projectile_mask(&self, i: usize, phase: i32) -> u128 {
        let p = &self.f[i].magic.projectiles;
        match phase {
            AIM => self.projectile_aim_mask(i),
            SHIELD => (1..=3)
                .filter(|&s| self.can_slot(i, s))
                .fold(Act::Pass.bit(), |m, s| m | (1 << s)),
            CUT => Act::Pass.bit() | Act::Defend.bit(),
            ROW => (0..HEIGHT)
                .filter(|&y| (0..WIDTH).any(|x| self.projectile_altitudes(i, Pos { x, y }) != 0))
                .fold(0, |m, y| m | (1 << y)),
            COLUMN => (0..WIDTH)
                .filter(|&x| self.projectile_altitudes(i, Pos { x, y: p.guess_y }) != 0)
                .fold(0, |m, x| m | (1 << x)),
            ALTITUDE => self.projectile_altitudes(
                i,
                Pos {
                    x: p.guess_x,
                    y: p.guess_y,
                },
            ),
            _ => 0,
        }
    }

    pub(in crate::arena2) fn projectile_label(
        &self,
        i: usize,
        phase: i32,
        a: Act,
    ) -> Option<String> {
        let id = a as usize;
        Some(match phase {
            AIM if a == Act::Pass => "End Chromatic Orb without another leap".into(),
            AIM if id == GUESS => "Aim at a guessed location".into(),
            AIM => {
                let target = id % ALL_DARTS;
                let who = if target == i {
                    "yourself".to_string()
                } else if target < 2 {
                    "the opposing adventurer".into()
                } else if target < 4 {
                    self.entity(target)?.kind.name().into()
                } else {
                    self.object(target)?.label().into()
                };
                if matches!(
                    self.f[i].magic.projectiles.spell,
                    Some(Spell::ShockingGrasp | Spell::PoisonSpray | Spell::TrueStrike)
                ) {
                    return Some(format!("Attack {who}"));
                }
                let count = if id >= ALL_DARTS {
                    "all remaining darts"
                } else {
                    "one projectile"
                };
                format!("Aim {count} at {who}")
            }
            SHIELD if a != Act::Pass => format!("Cast Shield using a level {id} slot"),
            SHIELD => "Let the darts strike".into(),
            CUT if a == Act::Defend => "Use Cutting Words on this dart's damage roll".into(),
            CUT => "Accept this damage roll".into(),
            ROW => format!("Row {}", id + 1),
            COLUMN => format!(
                "Cell {}{}",
                char::from(b'A' + id as u8),
                self.f[i].magic.projectiles.guess_y + 1
            ),
            ALTITUDE => format!("{} ft above the floor", id * 5),
            _ => return None,
        })
    }

    pub(in crate::arena2) fn projectile_step(&mut self, i: usize, phase: i32, a: Act) {
        let id = a as usize;
        match phase {
            AIM if a == Act::Pass => {
                self.f[i].magic.projectiles.remaining = 0;
                self.magic_continue_projectile(i);
            }
            AIM if id == GUESS => self.request(i, ROW),
            AIM if self.f[i].magic.projectiles.spell == Some(Spell::MagicMissile) => {
                let count = if id >= ALL_DARTS {
                    self.f[i].magic.projectiles.remaining
                } else {
                    1
                };
                let target = id % ALL_DARTS;
                if self.sanctuary_missile_reject(i, target, count, 0) {
                    return;
                }
                self.sanctuary_assign_darts(i, Some(target), count);
            }
            AIM => self.launch_spell_ray(
                i,
                Some(id),
                self.projectile_eligible(i, id).expect("legal ray target"),
            ),
            ROW => {
                self.f[i].magic.projectiles.guess_y = id as i8;
                self.request(i, COLUMN);
            }
            COLUMN => {
                self.f[i].magic.projectiles.guess_x = id as i8;
                self.request(i, ALTITUDE);
            }
            ALTITUDE => {
                let p = &self.f[i].magic.projectiles;
                let point = Point {
                    cell: Pos {
                        x: p.guess_x,
                        y: p.guess_y,
                    },
                    altitude: id as i8,
                };
                let target = (0..4)
                    .find(|&id| {
                        !self.suggestion_charmed_by(i, id)
                            && self
                                .footprint(id)
                                .is_some_and(|f| f.squares().any(|p| p == point))
                            && (p.spell != Some(Spell::ChromaticOrb) || p.used & (1 << id) == 0)
                    })
                    .or_else(|| {
                        if p.spell == Some(Spell::TrueStrike) {
                            (4..crate::arena2::objects::END_OBJECTS).find(|&id| {
                                self.object(id).is_some_and(|object| object.point == point)
                            })
                        } else {
                            None
                        }
                    });
                self.launch_spell_ray(i, target, point);
            }
            SHIELD => {
                let caster = self.missile_owner();
                if a == Act::Pass {
                    self.finish_projectile_shield(i, false);
                } else {
                    self.magic_pay_projectile_shield(i, id, caster);
                }
            }
            CUT => {
                let caster = self.missile_owner();
                let k = self.f[caster].magic.projectiles.cut_index;
                if a == Act::Defend {
                    let cut = self.magic_cut_damage(i);
                    let damage = &mut self.f[caster].magic.projectiles.damage[k];
                    *damage = (*damage - cut).max(0);
                }
                self.f[caster].magic.projectiles.cut_index += 1;
                self.resolve_darts(caster);
            }
            _ => unreachable!("projectile phase in validated dispatch"),
        }
    }

    pub(super) fn finish_projectile_shield(&mut self, defender: usize, _applied: bool) {
        let caster = self.missile_owner();
        self.f[caster].magic.projectiles.shield_answered |= 1 << defender;
        self.resolve_darts(caster);
    }

    fn resolve_darts(&mut self, i: usize) {
        let p = &self.f[i].magic.projectiles;
        for target in 0..2 {
            let eligible = p.targets[..p.assigned].contains(&target)
                && p.shield_answered & (1 << target) == 0
                && self.f[target].shield == 0
                && self.can_react(target)
                && matches!(self.f[target].build.class, Class::Sorcerer | Class::Wizard)
                && (1..=3).any(|s| self.can_slot(target, s))
                && self.can_cast_components(target, true, true, Material::None);
            if eligible {
                self.request(target, SHIELD);
                return;
            }
        }
        let bard = 1 - i;
        if self.magic_can_cut_damage(bard, i) {
            while self.f[i].magic.projectiles.cut_index < self.f[i].magic.projectiles.assigned {
                let p = &self.f[i].magic.projectiles;
                let target = p.targets[p.cut_index];
                if target >= 2 || self.f[target].shield == 0 {
                    self.request(bard, CUT);
                    return;
                }
                self.f[i].magic.projectiles.cut_index += 1;
            }
        }
        let p = std::mem::take(&mut self.f[i].magic.projectiles);
        for dart in 0..p.assigned {
            let id = p.targets[dart];
            if id >= 2 || self.f[id].shield == 0 {
                self.apply_spell_damage(i, id, p.damage[dart], DamageType::Force);
            }
        }
    }

    fn launch_spell_ray(&mut self, i: usize, id: Option<usize>, point: Point) {
        use crate::arena2::battle::sanctuary::WardRedirect;
        if id.is_some_and(|target| self.sanctuary_reject(i, target, WardRedirect::Ray(point), 0)) {
            return;
        }
        self.sanctuary_finish_ray(i, id, point);
    }

    pub(in crate::arena2) fn sanctuary_missile_reject(
        &mut self,
        actor: usize,
        target: usize,
        count: usize,
        rejected: u128,
    ) -> bool {
        let projectile = &mut self.f[actor].magic.projectiles;
        if projectile.sanctuary_checked & (1 << target) != 0 {
            return false;
        }
        projectile.sanctuary_checked |= 1 << target;
        let rejected = self.sanctuary_reject(
            actor,
            target,
            crate::arena2::battle::sanctuary::WardRedirect::Missile(count),
            rejected,
        );
        if rejected {
            self.f[actor].magic.projectiles.sanctuary_blocked |= 1 << target;
        }
        rejected
    }

    pub(in crate::arena2) fn sanctuary_assign_darts(
        &mut self,
        i: usize,
        target: Option<usize>,
        count: usize,
    ) {
        let p = &mut self.f[i].magic.projectiles;
        if let Some(target) = target {
            p.targets[p.assigned..p.assigned + count].fill(target);
            p.assigned += count;
        }
        p.remaining -= count;
        if p.remaining > 0 {
            self.request(i, AIM);
        } else {
            for dart in 0..p.assigned {
                self.f[i].magic.projectiles.damage[dart] = 1 + self.roll(4);
            }
            self.resolve_darts(i);
        }
    }

    pub(in crate::arena2) fn sanctuary_finish_ray(
        &mut self,
        i: usize,
        id: Option<usize>,
        point: Point,
    ) {
        self.ward_preapproved_ray = id.map(|target| (i, target));
        let p = &mut self.f[i].magic.projectiles;
        p.remaining -= 1;
        p.repeated_face = false;
        p.d8_faces = 0;
        p.previous = Some(point);
        let spell = p.spell.expect("projectile cast");
        let tier = p.tier as i32;
        if spell == Spell::TrueStrike {
            self.launch_true_strike(i, id, point);
            return;
        }
        let Some(target) = id else {
            self.magic_continue_projectile(i);
            return;
        };
        p.used |= 1 << target;
        self.f[i].magic.cast_target = Some(target);
        let (die, count, bonus) = match spell {
            Spell::EldritchBlast => (
                10,
                1,
                if self.f[i].build.level >= 2 {
                    self.f[i].build.casting
                } else {
                    0
                },
            ),
            Spell::ChromaticOrb => (8, tier + 2, 0),
            Spell::ScorchingRay => (6, 2, 0),
            Spell::GuidingBolt => (6, tier + 3, 0),
            Spell::ShockingGrasp | Spell::PoisonSpray => (
                if spell == Spell::ShockingGrasp { 8 } else { 12 },
                if self.f[i].build.level >= 5 { 2 } else { 1 },
                0,
            ),
            Spell::StarryWisp | Spell::RayOfFrost | Spell::FireBolt => (
                if spell == Spell::FireBolt { 10 } else { 8 },
                if self.f[i].build.level >= 5 { 2 } else { 1 },
                0,
            ),
            _ => unreachable!(),
        };
        self.start_spell_attack(i, spell, die, count, bonus, spell != Spell::ShockingGrasp);
    }

    pub(in crate::arena2) fn projectile_die(&mut self, hit: Hit, face: i32) {
        if hit.attacker >= 2 || hit.weapon {
            return;
        }
        self.projectile_object_die(hit.attacker, face);
    }

    pub(in crate::arena2) fn projectile_object_die(&mut self, actor: usize, face: i32) {
        let p = &mut self.f[actor].magic.projectiles;
        if p.spell == Some(Spell::ChromaticOrb) && (1..=8).contains(&face) {
            let bit = 1 << face;
            p.repeated_face |= p.d8_faces & bit != 0;
            p.d8_faces |= bit;
        }
    }

    pub(in crate::arena2) fn magic_continue_projectile(&mut self, i: usize) -> bool {
        let p = &self.f[i].magic.projectiles;
        if p.spell.is_none() || p.spell == Some(Spell::MagicMissile) {
            return false;
        }
        let continues = p.remaining > 0
            && (p.spell != Some(Spell::ChromaticOrb) || p.repeated_face)
            && self.projectile_aim_mask(i) & !Act::Pass.bit() != 0;
        if continues {
            self.request(i, AIM);
        } else {
            self.f[i].magic.projectiles = Default::default();
            self.f[i].magic.attack = None;
            self.f[i].magic.cast_target = None;
            self.f[i].magic.attack_target = None;
        }
        true
    }

    pub(in crate::arena2) fn projectile_pending(&self, viewer: usize) -> String {
        if self.actor() != Some(viewer) {
            return String::new();
        }
        match self.phase_id() {
            AIM => {
                let p = &self.f[viewer].magic.projectiles;
                let name = p.spell.expect("pending projectile").label();
                if matches!(
                    p.spell,
                    Some(Spell::ShockingGrasp | Spell::PoisonSpray | Spell::TrueStrike)
                ) {
                    return format!("{name}: select the attack target");
                }
                format!("{name}: select a target ({} remaining)", p.remaining)
            }
            SHIELD => {
                let p = &self.f[self.missile_owner()].magic.projectiles;
                let darts = p.targets[..p.assigned]
                    .iter()
                    .filter(|&&id| id == viewer)
                    .count();
                format!("Magic Missile: {darts} incoming darts. Choose a Shield slot or pass")
            }
            CUT => {
                let p = &self.f[self.missile_owner()].magic.projectiles;
                format!(
                    "Magic Missile: dart damage {}. Use Cutting Words or pass",
                    p.damage[p.cut_index]
                )
            }
            ROW | COLUMN => "Select a guessed projectile location".into(),
            ALTITUDE => {
                let p = &self.f[viewer].magic.projectiles;
                format!(
                    "Select height of guessed cell {}{}",
                    char::from(b'A' + p.guess_x as u8),
                    p.guess_y + 1
                )
            }
            _ => String::new(),
        }
    }

    pub(in crate::arena2) fn projectile_target_row(&self, viewer: usize) -> Option<i8> {
        (self.actor() == Some(viewer) && matches!(self.phase_id(), COLUMN | ALTITUDE))
            .then_some(self.f[viewer].magic.projectiles.guess_y)
    }

    pub(in crate::arena2) fn projectile_features(&self, viewer: usize) -> [f32; 7] {
        use crate::arena2::target_ids::relative_id;

        let mut f = [0.; 7];
        if self.actor() != Some(viewer) || !(AIM..=CUT).contains(&self.phase_id()) {
            return f;
        }
        if self.phase_id() == SHIELD || self.phase_id() == CUT {
            let p = &self.f[self.missile_owner()].magic.projectiles;
            f[0] = 1.;
            f[1] = p.targets[..p.assigned]
                .iter()
                .filter(|&&id| id == viewer)
                .count() as f32
                / 5.;
            if self.phase_id() == CUT {
                f[2] = p.damage[p.cut_index] as f32 / 5.;
                f[3] = (1 + relative_id(viewer, p.targets[p.cut_index])) as f32 / 4.;
            }
            return f;
        }
        let p = &self.f[viewer].magic.projectiles;
        f[0] = match p.spell {
            Some(Spell::MagicMissile) => 1.,
            Some(Spell::ChromaticOrb) => 0.75,
            Some(Spell::EldritchBlast) => 0.5,
            Some(Spell::ShockingGrasp) => 0.125,
            Some(Spell::PoisonSpray) => 0.375,
            Some(Spell::TrueStrike) if self.f[viewer].magic.true_strike_thrown => 0.9375,
            Some(Spell::TrueStrike) if self.f[viewer].magic.true_strike_ranged => 0.875,
            Some(Spell::TrueStrike) => 0.625,
            _ => 0.25,
        };
        f[1] = p.remaining as f32 / 5.;
        f[2] = p.tier as f32 / 3.;
        let used = (0..crate::arena2::objects::END_OBJECTS).fold(0, |mask, id| {
            let bit = (p.used >> relative_id(viewer, id)) & 1;
            mask | (bit << id)
        });
        f[3] = used as f32 / 16383.;
        if let Some(previous) = p.previous {
            f[4] = (previous.cell.x + 1) as f32 / 9.;
            f[5] = (previous.cell.y + 1) as f32 / 7.;
            f[6] = (previous.altitude + 1) as f32 / 7.;
        }
        if self.phase_id() == COLUMN || self.phase_id() == ALTITUDE {
            f[5] = (p.guess_y + 1) as f32 / 7.;
        }
        if self.phase_id() == ALTITUDE {
            f[4] = (p.guess_x + 1) as f32 / 9.;
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{
        entities::{Entity, EntityKind, Spirit},
        equipment::Carried,
        ACTS,
    };

    fn battle(seed: u64, classes: [Class; 2]) -> Battle {
        let mut b = Battle::new(seed, classes, [5, 5]);
        b.owner = 0;
        b.f[0].pos = Pos { x: 0, y: 0 };
        b.f[1].pos = Pos { x: 2, y: 0 };
        b
    }
    fn choose_first_rank_if_prompted(b: &mut Battle) {
        if b.phase_id() == 38 && b.mask() & ACTS[0].bit() != 0 {
            b.step(ACTS[0]).unwrap();
        }
    }
    fn fixture_cast(b: &mut Battle, spell: Spell, tier: usize) {
        assert!(b.magic_begin_projectile(0, spell, tier));
        assert_eq!(b.phase_id(), AIM);
    }

    #[test]
    fn shocking_grasp_reaches_the_selected_steed_and_only_suppresses_its_opportunities() {
        for seed in 0..128 {
            let mut b = battle(seed, [Class::Wizard, Class::Paladin]);
            let adjacent = Pos { x: 1, y: 0 };
            if !b.map.los_at(b.fighter_point(0), Point::ground(adjacent)) {
                continue;
            }
            b.f[1].pos.x = 4;
            b.fixture_companion(
                1,
                Entity::new(
                    EntityKind::Steed {
                        spirit: Spirit::Fey,
                        spell_level: 2,
                    },
                    1,
                    adjacent,
                    3,
                ),
            );
            b.f[1].gear.melee = Carried::Ground(adjacent);
            let hp = b.f[1].hp;
            b.step(Act::Magic1).unwrap();
            assert_eq!(b.phase_id(), AIM);
            assert_eq!(b.mask() & (ACTS[1].bit() | ACTS[5].bit()), 0);
            assert_ne!(b.mask() & ACTS[3].bit(), 0);
            b.step(ACTS[3]).unwrap();
            if !b.magic_no_reactions(3) {
                continue;
            }
            assert_eq!(b.f[1].hp, hp);
            assert!(!b.magic_no_reactions(1));
            assert!(b.entity(3).unwrap().reaction);
            b.magic_reset_opportunity(3);
            assert!(!b.magic_no_reactions(3));
            return;
        }
        panic!("expected Shocking Grasp to hit the chosen steed");
    }

    #[test]
    fn poison_spray_guess_menu_ignores_hidden_occupancy_and_can_miss() {
        let mut near = battle(2, [Class::Warlock, Class::Fighter]);
        near.f[1].hidden_dc = Some(22);
        near.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 0 });
        let mut farther = near.clone();
        farther.f[1].pos.x = 3;
        let hp = near.f[1].hp;
        for choice in [Act::Magic1, ACTS[GUESS], ACTS[0], ACTS[1], ACTS[0]] {
            assert_eq!(near.mask(), farther.mask());
            assert_eq!(near.projectile_features(0), farther.projectile_features(0));
            if near.phase_id() == AIM {
                assert_eq!(near.mask() & (ACTS[1].bit() | ACTS[5].bit()), 0);
            }
            near.step(choice).unwrap();
            farther.step(choice).unwrap();
        }
        assert_eq!((near.f[1].hp, farther.f[1].hp), (hp, hp));
        assert_eq!((near.phase_id(), farther.phase_id()), (0, 0));
    }

    #[test]
    fn true_strike_can_throw_its_component_at_an_object() {
        let mut b = battle(2, [Class::Wizard, Class::Fighter]);
        let point = Pos { x: 1, y: 0 };
        b.f[1].gear.melee = Carried::Ground(point);
        let slots = b.f[0].slots;
        b.step(Act::Magic4).unwrap();
        b.step(ACTS[2]).unwrap(); // throw the component dagger instead of striking with it
        b.step(ACTS[1]).unwrap(); // Radiant base damage
        assert_eq!(b.phase_id(), AIM);
        assert_ne!(b.mask() & ACTS[5].bit(), 0);
        b.step(ACTS[5]).unwrap();
        assert_eq!(b.f[0].gear.melee, Carried::Ground(point));
        assert_eq!(b.f[0].slots, slots);
        assert!(!b.action);
        assert_eq!(b.phase_id(), 0);
    }

    #[test]
    fn charmed_subject_cannot_target_the_source_with_cantrips() {
        use crate::arena2::magic::Concentration;
        use crate::arena2::spell_movement::Suggestion;
        let mut b = battle(2, [Class::Wizard, Class::Warlock]);
        b.f[1].pos = Pos { x: 1, y: 0 };
        b.f[1].concentration = Some(Concentration::Suggestion);
        b.f[1].reaction = false;
        b.activate_suggestion(1, 0, Suggestion::EndTurn);
        b.step(Act::Magic1).unwrap();
        assert_eq!(b.phase_id(), AIM);
        assert_ne!(b.mask() & ACTS[0].bit(), 0);
        assert_eq!(b.mask() & ACTS[1].bit(), 0);
        assert!(b.step(ACTS[1]).is_err());
    }

    #[test]
    fn potent_cantrip_deals_half_on_a_miss_without_shocking_grasp_rider() {
        let found = (0..100).any(|seed| {
            let mut b = battle(seed, [Class::Wizard, Class::Fighter]);
            b.f[1].pos.x = 1;
            b.f[1].shield = 100;
            let hp = b.f[1].hp;
            b.step(Act::Magic1).unwrap();
            if b.mask() & ACTS[1].bit() == 0 {
                return false;
            }
            b.step(ACTS[1]).unwrap();
            b.f[1].hp < hp && (1..=8).contains(&(hp - b.f[1].hp)) && !b.magic_no_reactions(1)
        });
        assert!(found);
    }

    #[test]
    fn bard_bot_only_reduces_darts_hitting_its_own_side() {
        for caster in 0..2 {
            let bard = 1 - caster;
            let mut classes = [Class::Bard; 2];
            classes[caster] = Class::Wizard;
            for (target, expected) in [(0, Act::Pass), (1, Act::Defend)] {
                let mut b = battle(2, classes);
                b.owner = caster;
                b.f[caster].reaction = false;
                b.step(Act::Spell1).unwrap();
                choose_first_rank_if_prompted(&mut b);
                b.step(ACTS[ALL_DARTS + target]).unwrap();
                assert_eq!((b.phase_id(), b.actor()), (CUT, Some(bard)));
                assert_eq!(crate::arena2::bot::choose(&b, false), expected);
            }
        }
    }

    #[test]
    fn darts_split_then_resolve_together_with_one_ranked_shield_decision() {
        let mut b = battle(2, [Class::Wizard, Class::Wizard]);
        let hp = [b.f[0].hp, b.f[1].hp];
        let slots = b.f[0].slots;
        b.step(Act::Spell1).unwrap();
        choose_first_rank_if_prompted(&mut b); // actual level-one Magic Missile
        assert_eq!(b.phase_id(), AIM);
        b.step(ACTS[0]).unwrap();
        assert_eq!(hp, [b.f[0].hp, b.f[1].hp]);
        b.step(ACTS[ALL_DARTS + 1]).unwrap();
        assert_eq!((b.phase_id(), b.actor()), (SHIELD, Some(1)));
        assert_eq!(hp, [b.f[0].hp, b.f[1].hp]);
        assert_eq!(b.f[0].slots[0], slots[0] - 1);
        assert!(b.mask() & ACTS[2].bit() != 0);
        let defense_slots = b.f[1].slots;
        b.step(ACTS[2]).unwrap();
        assert_eq!(b.f[1].slots[1], defense_slots[1] - 1);
        assert!(!b.f[1].reaction);
        assert_eq!(b.f[1].hp, hp[1]);
        assert!((2..=5).contains(&(hp[0] - b.f[0].hp)));
        assert_eq!(b.phase_id(), 0);
    }

    #[test]
    fn missile_shield_can_be_countered_on_a_readied_release_without_losing_the_dart_assignments() {
        let mut observed = [false, false];
        for seed in 0..64 {
            let mut b = battle(seed, [Class::Sorcerer, Class::Wizard]);
            let hp = b.f[1].hp;
            // At a release the original projectile caster has not spent a slot on this turn.
            fixture_cast(&mut b, Spell::MagicMissile, 1);
            b.step(ACTS[ALL_DARTS + 1]).unwrap();
            assert_eq!(b.phase_id(), SHIELD);
            b.step(ACTS[1]).unwrap();
            assert_eq!(b.actor(), Some(0));
            assert!(b.choice_label(0, Act::Defend).contains("Counterspell"));
            b.step(Act::Defend).unwrap();
            assert_eq!(b.phase_id(), 0);
            let blocked = b.f[1].hp == hp;
            observed[usize::from(blocked)] = true;
            assert_eq!(b.f[1].shield > 0, blocked);
            assert_eq!(b.f[1].slots[0], if blocked { 3 } else { 4 });
            assert!(!b.f[1].reaction && !b.f[0].reaction);
            if observed == [true, true] {
                break;
            }
        }
        assert_eq!(observed, [true, true]);
    }

    #[test]
    fn simultaneous_darts_can_knock_out_both_seats() {
        let mut b = battle(91, [Class::Wizard, Class::Fighter]);
        b.f[0].hp = 1;
        b.f[1].hp = 1;
        b.step(Act::Spell1).unwrap();
        choose_first_rank_if_prompted(&mut b);
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[ALL_DARTS + 1]).unwrap();
        assert_eq!([b.f[0].hp, b.f[1].hp], [0, 0]);
        assert!(b.finished());
        assert_eq!(b.winner(), None);
    }

    #[test]
    fn blasting_beams_retarget_each_roll_and_missile_excludes_objects() {
        let mut b = battle(3, [Class::Warlock, Class::Fighter]);
        b.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 0 });
        let object_id = 5;
        b.step(Act::Spell0).unwrap();
        assert_eq!(b.phase_id(), AIM);
        assert_ne!(b.mask() & ACTS[object_id].bit(), 0);
        b.step(ACTS[object_id]).unwrap();
        assert_eq!(b.phase_id(), AIM);
        assert_eq!(b.f[0].magic.projectiles.remaining, 1);
        assert_ne!(b.mask() & ACTS[1].bit(), 0);
        b.step(ACTS[1]).unwrap();
        if b.phase_id() == 3 {
            b.step(Act::Pass).unwrap();
        }
        assert_eq!(b.phase_id(), 0);
        fixture_cast(&mut b, Spell::MagicMissile, 3);
        assert_eq!(b.mask() & ACTS[object_id].bit(), 0);
    }

    #[test]
    fn orb_leaps_on_real_duplicate_damage_faces_only_to_fresh_targets() {
        let mut duplicate = false;
        let mut single = false;
        for seed in 0..512 {
            let mut b = battle(seed, [Class::Sorcerer, Class::Fighter]);
            b.f[1].gear.melee = Carried::Ground(Pos { x: 1, y: 0 });
            let hp = b.f[1].hp;
            fixture_cast(&mut b, Spell::ChromaticOrb, 2);
            b.step(ACTS[1]).unwrap();
            if b.phase_id() == AIM {
                assert!(b.f[1].hp < hp);
                assert!(b.f[0].magic.projectiles.repeated_face);
                assert_eq!(b.mask() & ACTS[1].bit(), 0);
                assert_ne!(b.mask() & ACTS[5].bit(), 0);
                b.step(ACTS[5]).unwrap();
                if b.phase_id() == AIM {
                    assert_eq!(b.f[0].magic.projectiles.remaining, 1);
                    assert_eq!(b.mask() & (ACTS[1].bit() | ACTS[5].bit()), 0);
                }
                duplicate = true;
            } else if hp > b.f[1].hp {
                single = true;
            }
            if duplicate && single {
                return;
            }
        }
        panic!("expected hits with and without repeated orb damage dice");
    }

    #[test]
    fn four_projectile_spells_offer_only_their_legal_objects_and_apply_to_the_chosen_target() {
        for spell in [
            Spell::FireBolt,
            Spell::StarryWisp,
            Spell::RayOfFrost,
            Spell::GuidingBolt,
        ] {
            let mut witness = false;
            for seed in 0..128 {
                let mut b = battle(seed, [Class::Druid, Class::Ranger]);
                b.f[1].gear.bow = Carried::Ground(Pos { x: 1, y: 0 });
                let object = 7;
                fixture_cast(&mut b, spell, 1);
                assert_eq!(
                    b.mask() & ACTS[object].bit() != 0,
                    matches!(spell, Spell::FireBolt | Spell::StarryWisp)
                );
                let target = if matches!(spell, Spell::FireBolt | Spell::StarryWisp) {
                    object
                } else {
                    0
                };
                let hp = b.f[0].hp;
                b.step(ACTS[target]).unwrap();
                witness = match spell {
                    Spell::FireBolt => b.object(object).is_some_and(|o| o.burning),
                    Spell::StarryWisp => b.projectile_glowing(object) && !b.projectile_glowing(1),
                    Spell::RayOfFrost => {
                        b.f[0].hp < hp && b.frost_slow(0) == 2 && b.frost_slow(1) == 0
                    }
                    Spell::GuidingBolt => {
                        b.f[0].hp < hp
                            && !b.guiding_advantage(1)
                            && b.take_guiding_advantage(0)
                            && !b.take_guiding_advantage(0)
                    }
                    _ => unreachable!(),
                };
                if witness {
                    break;
                }
            }
            assert!(witness, "no legal hit witnessed for {spell:?}");
        }
    }

    #[test]
    fn orb_leap_is_optional_and_scorching_rays_have_independent_targets() {
        assert!((0..128).any(|seed| {
            let mut b = battle(seed, [Class::Sorcerer, Class::Fighter]);
            fixture_cast(&mut b, Spell::ChromaticOrb, 1);
            b.step(ACTS[1]).unwrap();
            if b.phase_id() != AIM {
                return false;
            }
            assert!(b.mask() & Act::Pass.bit() != 0);
            let hp = b.f[0].hp;
            b.step(Act::Pass).unwrap();
            b.phase_id() == 0 && b.f[0].hp == hp
        }));
        let mut b = battle(3, [Class::Sorcerer, Class::Fighter]);
        b.f[0].hp = 100;
        fixture_cast(&mut b, Spell::ScorchingRay, 3);
        for (index, target) in [0, 1, 1, 0].into_iter().enumerate() {
            assert_eq!(b.phase_id(), AIM);
            assert_eq!(b.f[0].magic.projectiles.remaining, 4 - index);
            b.step(ACTS[target]).unwrap();
            if b.phase_id() == 3 {
                b.step(Act::Pass).unwrap();
            }
        }
        assert_eq!(b.phase_id(), 0);
    }

    #[test]
    fn warlock_level_unlocks_apply_only_to_the_learned_blast() {
        let mut hits = 0;
        for seed in 0..32 {
            let mut damage = [0; 2];
            for (j, level) in [1, 2].into_iter().enumerate() {
                let mut b = Battle::new(seed, [Class::Warlock, Class::Fighter], [level, 5]);
                b.owner = 0;
                b.f[0].pos = Pos { x: 0, y: 0 };
                b.f[1].pos = Pos { x: 3, y: 0 };
                assert!(b.magic_concentration_advantage(0));
                assert!(!b.magic_concentration_advantage(1));
                b.f[1].hp = 100;
                b.step(Act::Spell0).unwrap();
                assert_eq!(b.aim_origin_range(0).1, 24);
                b.step(ACTS[1]).unwrap();
                damage[j] = 100 - b.f[1].hp;
            }
            if damage[0] > 0 {
                assert_eq!(damage[1] - damage[0], 3);
                hits += 1;
            }
        }
        assert!(hits > 0);
        let mut b = battle(11, [Class::Warlock, Class::Fighter]);
        b.step(Act::Spell0).unwrap();
        assert_eq!(b.aim_origin_range(0).1, 24 + 6 * 5);
        assert_eq!(b.f[0].magic.projectiles.remaining, 2);
    }

    #[test]
    fn prepared_projectiles_reach_the_native_target_menu_for_each_actual_build() {
        for (class, action) in [
            (Class::Bard, Act::Special1),
            (Class::Druid, Act::Spell0),
            (Class::Wizard, Act::Spell0),
            (Class::Sorcerer, Act::Spell0),
            (Class::Cleric, Act::Spell1),
        ] {
            let mut b = battle(12, [class, Class::Fighter]);
            if b.mask() & action.bit() == 0 {
                b.step(Act::Gear2).unwrap();
            }
            b.step(action).unwrap();
            if b.phase_id() == 38 {
                b.step(ACTS[0]).unwrap();
            }
            assert_eq!(b.phase_id(), AIM, "{class:?} {action:?}");
            assert_ne!(b.mask() & ACTS[1].bit(), 0);
        }
    }

    #[test]
    fn unseen_target_can_be_guessed_by_rays_but_not_selected_for_missiles() {
        let mut b = battle(4, [Class::Warlock, Class::Fighter]);
        b.f[1].hidden_dc = Some(22);
        fixture_cast(&mut b, Spell::EldritchBlast, 0);
        assert_ne!(b.mask() & ACTS[GUESS].bit(), 0);
        assert_eq!(b.mask() & ACTS[1].bit(), 0);
        let hp = b.f[1].hp;
        b.step(ACTS[GUESS]).unwrap();
        b.step(ACTS[0]).unwrap();
        b.step(ACTS[4]).unwrap();
        b.step(ACTS[0]).unwrap();
        assert_eq!(hp, b.f[1].hp);
        assert_eq!(b.phase_id(), AIM);
        fixture_cast(&mut b, Spell::MagicMissile, 1);
        assert_eq!(b.mask() & (ACTS[GUESS].bit() | ACTS[1].bit()), 0);
    }
}
