//! Phantasmal Force uses a stationary illusion visible only to its target.
use super::{Act, Battle, Concentration, DamageType, MagicChoice, Point, Pos, Spell, ACTS};
use crate::arena2::geometry::{HEIGHT, MAX_ELEVATION, WIDTH};

pub(in crate::arena2) const PHANTASM_CELL: i32 = 153;
pub(in crate::arena2) const PHANTASM_HEIGHT: i32 = 154;
pub(in crate::arena2) const PHANTASM_DAMAGE: Act = Act::Magic19;

fn cell(action: Act) -> Pos {
    let index = action as i8;
    Pos {
        x: index % WIDTH,
        y: index / WIDTH,
    }
}

fn contains_or_adjacent(origin: Point, point: Point) -> bool {
    [
        point.cell.x - origin.cell.x,
        point.cell.y - origin.cell.y,
        point.altitude - origin.altitude,
    ]
    .into_iter()
    .all(|offset| (-1..=2).contains(&offset))
}

impl Battle {
    pub(super) fn cast_phantasmal_force(&mut self, caster: usize, tier: usize, heightened: bool) {
        let target = self.f[caster].magic.cast_target.unwrap_or(1 - caster);
        let dc = self.spell_dc(caster);
        let disadvantage = heightened && self.magic_heightened_for(caster, target);
        self.f[caster].concentration = None;
        let saved = if target < 2 {
            self.save_advantage(target, 3, dc, disadvantage)
        } else {
            self.entity_save_with_advantage(target, 3, dc, disadvantage)
        };
        if saved {
            return;
        }
        self.concentrate(caster, Concentration::PhantasmalForce, 10, None, tier);
        self.f[caster].magic.phantasm_target = Some(target);
        self.f[caster].magic.phantasm_cell = None;
        self.f[caster].magic.phantasm_damage_turn = -1;
        self.request(caster, PHANTASM_CELL);
    }

    fn phantasm_heights(&self, caster: usize, cell: Pos) -> u128 {
        let eye = self.fighter_point(caster);
        (0..MAX_ELEVATION)
            .filter(|&altitude| {
                let point = Point { cell, altitude };
                eye.dist(point) <= 12
                    && self.map.los_at(eye, point)
                    && !self.magic_cloud_blocks(eye, point)
            })
            .fold(0, |mask, altitude| mask | (1 << altitude))
    }

    pub(in crate::arena2) fn phantasm_placement_mask(&self, caster: usize, phase: i32) -> u128 {
        if phase == PHANTASM_HEIGHT {
            return self.f[caster]
                .magic
                .phantasm_cell
                .map_or(0, |pos| self.phantasm_heights(caster, pos));
        }
        (0..HEIGHT - 1)
            .flat_map(|y| (0..WIDTH - 1).map(move |x| Pos { x, y }))
            .filter(|&pos| self.phantasm_heights(caster, pos) != 0)
            .fold(Act::Other8.bit(), |mask, pos| {
                mask | (1 << (pos.y * WIDTH + pos.x))
            })
    }

    pub(in crate::arena2) fn phantasm_placement_label(&self, phase: i32, action: Act) -> String {
        if action == Act::Other8 {
            return "End Phantasmal Force".into();
        }
        if phase == PHANTASM_HEIGHT {
            return format!("Place at {} feet above the floor", 5 * action as usize);
        }
        let pos = cell(action);
        format!(
            "Place cube at {}{}",
            char::from(b'A' + pos.x as u8),
            pos.y + 1
        )
    }

    pub(in crate::arena2) fn phantasm_pending_cell(&self, caster: usize) -> Option<Pos> {
        self.f[caster].magic.phantasm_cell
    }

    pub(in crate::arena2) fn phantasm_placement_step(
        &mut self,
        caster: usize,
        phase: i32,
        action: Act,
    ) {
        if action == Act::Other8 {
            self.f[caster].concentration = None;
        } else if phase == PHANTASM_CELL {
            self.f[caster].magic.phantasm_cell = Some(cell(action));
            self.request(caster, PHANTASM_HEIGHT);
        } else if let Some(cell) = self.f[caster].magic.phantasm_cell.take() {
            self.f[caster].magic.field = Some(Point {
                cell,
                altitude: action as i8,
            });
        }
    }

    fn phantasm_known_target(&self, caster: usize, target: usize) -> Option<Point> {
        if target < 2 {
            self.observed_pos(caster, target)?;
        } else {
            self.observed_entity(caster, target)?;
        }
        self.creature_point(target)
    }

    pub(in crate::arena2) fn phantasmal_damage_available(&self, caster: usize) -> bool {
        let spell = &self.f[caster];
        self.owner == caster
            && spell.concentration == Some(Concentration::PhantasmalForce)
            && spell.magic.phantasm_damage_turn != self.turns
            && spell
                .magic
                .phantasm_target
                .zip(spell.magic.field)
                .is_some_and(|(target, origin)| {
                    self.rope_same_space(caster, target)
                        && self
                            .phantasm_known_target(caster, target)
                            .is_some_and(|point| contains_or_adjacent(origin, point))
                })
    }

    pub(super) fn phantasmal_force_damage(&mut self, caster: usize) {
        if !self.phantasmal_damage_available(caster) {
            return;
        }
        let target = self.f[caster]
            .magic
            .phantasm_target
            .expect("valid phantasm target");
        self.f[caster].magic.phantasm_damage_turn = self.turns;
        let damage = self.roll_many(2, 8);
        self.apply_spell_damage(caster, target, damage, DamageType::Psychic);
    }

    fn phantasmal_force_source(&self, target: usize) -> Option<usize> {
        self.f.iter().position(|fighter| {
            fighter.concentration == Some(Concentration::PhantasmalForce)
                && fighter.magic.phantasm_target == Some(target)
        })
    }

    pub(in crate::arena2) fn phantasmal_force_dc(&self, target: usize) -> Option<i32> {
        self.phantasmal_force_source(target)
            .map(|caster| self.spell_dc(caster))
    }

    pub(in crate::arena2) fn end_phantasmal_force(&mut self, target: usize) {
        if let Some(caster) = self.phantasmal_force_source(target) {
            self.f[caster].concentration = None;
        }
    }

    pub(in crate::arena2) fn phantasmal_force_policy(
        &self,
        caster: usize,
        mask: u128,
    ) -> Option<Act> {
        let phase = self.phase_id();
        if matches!(phase, PHANTASM_CELL | PHANTASM_HEIGHT) {
            let placements = self.phantasm_placement_mask(caster, phase) & mask;
            let target = self.f[caster]
                .magic
                .phantasm_target
                .and_then(|target| self.phantasm_known_target(caster, target))
                .unwrap_or_else(|| self.fighter_point(caster));
            return ACTS
                .iter()
                .copied()
                .filter(|&action| action != Act::Other8 && placements & action.bit() != 0)
                .min_by_key(|&action| {
                    if phase == PHANTASM_CELL {
                        let pos = cell(action);
                        (pos.dist(target.cell), action as usize)
                    } else {
                        (
                            (i32::from(action as i8) - i32::from(target.altitude)).abs(),
                            action as usize,
                        )
                    }
                })
                .or_else(|| (mask & Act::Other8.bit() != 0).then_some(Act::Other8));
        }
        if phase == 0
            && mask & PHANTASM_DAMAGE.bit() != 0
            && self.f[caster]
                .magic
                .phantasm_target
                .is_some_and(|target| target % 2 != caster)
        {
            return Some(PHANTASM_DAMAGE);
        }
        let MagicChoice::Creature(action) = self.f[caster].magic.choice? else {
            return None;
        };
        if self.prepared_spell(caster, action) != Some(Spell::PhantasmalForce) {
            return None;
        }
        let foe = 1 - caster;
        [ACTS[foe], ACTS[foe + 2], Act::Pass]
            .into_iter()
            .find(|&action| mask & action.bit() != 0)
    }

    pub(in crate::arena2) fn phantasmal_enemy_target(&self, caster: usize) -> bool {
        let foe = 1 - caster;
        self.creature_choices(caster, Spell::PhantasmalForce)
            & (ACTS[foe].bit() | ACTS[foe + 2].bit())
            != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{common::Check, geometry::Pos};

    #[test]
    fn phantasm_is_stationary_and_an_investigation_can_end_it() {
        let mut battle = Battle::new_roster_in(48, [31, 12], 0);
        battle.owner = 0;
        battle.f[1].pos = Pos { x: 1, y: 3 };
        battle.f[1].build.stats[3] = -30;
        let initial_hp = battle.f[1].hp;
        battle.cast_phantasmal_force(0, 2, false);
        assert_eq!(battle.phase_id(), PHANTASM_CELL);
        battle
            .step_raw_action(ACTS[(2 * WIDTH + 1) as usize])
            .unwrap();
        assert_eq!(battle.phase_id(), PHANTASM_HEIGHT);
        battle.step_raw_action(Act::N).unwrap();
        assert_eq!(battle.f[1].hp, initial_hp);
        assert!(battle.phantasmal_damage_available(0));
        battle.action = false;
        battle.bonus = false;
        let budget = (battle.action, battle.bonus);
        battle.step_raw_action(PHANTASM_DAMAGE).unwrap();
        let damaged_hp = battle.f[1].hp;
        assert!(initial_hp - damaged_hp >= 2);
        assert_eq!((battle.action, battle.bonus), budget);
        assert!(!battle.phantasmal_damage_available(0));
        battle.turns += 2;
        battle.magic_turn(0);
        assert_eq!(battle.f[1].hp, damaged_hp);
        assert!(battle.phantasmal_damage_available(0));
        battle.f[1].pos = Pos { x: 4, y: 3 };
        assert!(!battle.phantasmal_damage_available(0));
        assert_eq!(battle.f[1].hp, damaged_hp);
        assert!(battle.phantasmal_force_dc(1).is_some());
        assert!(battle.phantasmal_force_dc(0).is_none());
        battle.action = true;
        assert_eq!(battle.study_main_mask(1), Act::Study.bit());
        assert_eq!(
            battle.study_label(1, Act::Rider).as_deref(),
            Some("Examine the phantasm")
        );
        battle.f[1].build.stats[3] = 30;
        assert!(battle.try_check(1, Check::ExaminePhantasm).is_none());
        assert!(battle.phantasmal_force_dc(1).is_none());
    }
}
