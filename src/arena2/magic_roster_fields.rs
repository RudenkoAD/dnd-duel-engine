//! Spatial and turn effects of the Archfey's always-prepared spells.
use super::*;
use crate::arena2::geometry::{HEIGHT, MAX_ELEVATION, WIDTH};

pub(in crate::arena2) const BLINK_CELL: i32 = 200;
pub(in crate::arena2) const BLINK_HEIGHT: i32 = 201;
pub(in crate::arena2) const GROWTH_CELLS: i32 = 202;

#[derive(Clone, Default, Debug)]
pub(super) struct RosterFieldState {
    pub(super) blink_until: i32,
    ethereal: bool,
    return_cell: Option<Pos>,
    growth: u64,
    pending_plants: u64,
    excluded: u64,
}

fn index(cell: Pos) -> usize {
    (cell.y * WIDTH + cell.x) as usize
}

fn cell(key: usize) -> Pos {
    Pos {
        x: (key % WIDTH as usize) as i8,
        y: (key / WIDTH as usize) as i8,
    }
}

impl Battle {
    pub(in crate::arena2) fn blink_ethereal(&self, id: usize) -> bool {
        self.f
            .get(id)
            .is_some_and(|f| f.magic.roster_fields.ethereal)
    }

    pub(super) fn cast_blink(&mut self, i: usize) {
        self.f[i].magic.roster_fields.blink_until = self.turns + 20;
    }

    pub(in crate::arena2) fn blink_end_turn(&mut self, i: usize) {
        if self.f[i].magic.roster_fields.blink_until > self.turns
            && self.rope_space(i).is_none()
            && self.roll(6) >= 4
        {
            self.f[i].magic.roster_fields.ethereal = true;
            self.f[i].grappled = None;
            self.f[1 - i].grappled = None;
        }
    }

    pub(in crate::arena2) fn blink_start_turn(&mut self, i: usize) -> bool {
        if self.blink_ethereal(i) {
            self.request(i, BLINK_CELL);
            true
        } else {
            false
        }
    }

    fn blink_returns(&self, i: usize) -> Vec<Point> {
        let origin = self.fighter_point(i);
        let mut near = Vec::new();
        let mut fallback = Vec::new();
        let mut closest = i32::MAX;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                for altitude in 0..=MAX_ELEVATION {
                    let p = Point {
                        cell: Pos { x, y },
                        altitude,
                    };
                    let distance = origin.dist(p);
                    if !self.map.can_occupy(p)
                        || (0..4).any(|other| {
                            other != i
                                && !self.blink_ethereal(other)
                                && self.rope_space(other).is_none()
                                && self
                                    .footprint(other)
                                    .is_some_and(|f| f.squares().any(|q| q == p))
                        })
                    {
                        continue;
                    }
                    if distance <= 2
                        && self.map.los_at(origin, p)
                        && !self.magic_cloud_blocks(origin, p)
                    {
                        near.push(p);
                    }
                    if distance <= closest {
                        if distance < closest {
                            fallback.clear();
                            closest = distance;
                        }
                        fallback.push(p);
                    }
                }
            }
        }
        if near.is_empty() {
            fallback
        } else {
            near
        }
    }

    pub(in crate::arena2) fn roster_field_mask(&self, i: usize, phase: i32) -> u128 {
        let state = &self.f[i].magic.roster_fields;
        match phase {
            GROWTH_CELLS => u128::from(state.pending_plants & !state.excluded) | (1 << 63),
            BLINK_CELL => self
                .blink_returns(i)
                .iter()
                .fold(0, |m, p| m | (1 << index(p.cell))),
            BLINK_HEIGHT => self
                .blink_returns(i)
                .iter()
                .filter(|p| Some(p.cell) == state.return_cell)
                .fold(0, |m, p| m | (1 << p.altitude)),
            _ => 0,
        }
    }

    pub(in crate::arena2) fn roster_field_label(&self, phase: i32, action: Act) -> String {
        let key = action as usize;
        if phase == BLINK_HEIGHT {
            return format!("Return at {} feet above the floor", 5 * key);
        }
        if phase == GROWTH_CELLS && key == 63 {
            return "Grow all remaining plants".into();
        }
        let p = cell(key);
        let verb = if phase == BLINK_CELL {
            "Return at"
        } else {
            "Exclude plants at"
        };
        format!("{verb} {}{}", char::from(b'A' + p.x as u8), p.y + 1)
    }

    pub(in crate::arena2) fn roster_field_step(&mut self, i: usize, phase: i32, action: Act) {
        let index = action as usize;
        let state = &mut self.f[i].magic.roster_fields;
        match phase {
            BLINK_CELL => {
                state.return_cell = Some(cell(index));
                self.request(i, BLINK_HEIGHT);
            }
            BLINK_HEIGHT => {
                let pos = state.return_cell.take().expect("Blink return cell chosen");
                state.ethereal = false;
                self.f[i].pos = pos;
                self.f[i].elevation = index as i8;
                self.finish_start_turn(i);
            }
            GROWTH_CELLS if index == 63 => {
                state.growth |= state.pending_plants & !state.excluded;
                state.pending_plants = 0;
                state.excluded = 0;
            }
            GROWTH_CELLS => {
                state.excluded |= 1 << index;
                self.request(i, GROWTH_CELLS);
            }
            _ => unreachable!(),
        }
    }

    pub(in crate::arena2) fn cast_plant_growth(&mut self, i: usize, center: Point) {
        // The arena maps mark their low patches of vegetation as difficult terrain.
        let mut plants = 0;
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let pos = Pos { x, y };
                if self.map.tile(pos) == 2
                    && self.area_hits_at(center, Point::ground(pos), Area::Sphere { radius: 20 })
                {
                    plants |= 1 << index(pos);
                }
            }
        }
        if plants != 0 {
            self.f[i].magic.roster_fields.pending_plants = plants;
            self.f[i].magic.roster_fields.excluded = 0;
            self.request(i, GROWTH_CELLS);
        }
    }

    pub(in crate::arena2) fn surface_tile(&self, cell: Pos) -> u8 {
        if self.overgrowth_cost(Point::ground(cell), 1) == 4 {
            3
        } else {
            self.map.tile(cell)
        }
    }

    pub(in crate::arena2) fn overgrowth_cost(&self, p: Point, cost: i32) -> i32 {
        if p.altitude == 0
            && self
                .f
                .iter()
                .any(|f| f.magic.roster_fields.growth & (1 << index(p.cell)) != 0)
        {
            cost.max(4)
        } else {
            cost
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::geometry::Map;

    #[test]
    fn blink_removes_the_caster_and_returns_them_to_the_chosen_cell() {
        let mut b = Battle::new_roster_in(17, [31, 12], 0);
        b.owner = 0;
        b.cast_blink(0);
        for _ in 0..100 {
            b.blink_end_turn(0);
            if b.blink_ethereal(0) {
                break;
            }
        }
        assert!(b.blink_ethereal(0));
        assert_eq!(b.observed_pos(1, 0), None);
        let original = b.fighter_point(0);
        assert!(b.blink_start_turn(0));
        let choices = b.blink_returns(0);
        assert!(choices.iter().all(|p| original.dist(*p) <= 2));
        let p = choices
            .into_iter()
            .find(|p| *p != original && p.altitude == 0)
            .unwrap();
        b.roster_field_step(0, BLINK_CELL, ACTS[index(p.cell)]);
        b.roster_field_step(0, BLINK_HEIGHT, ACTS[p.altitude as usize]);
        assert_eq!(b.fighter_point(0), p);
        assert!(!b.blink_ethereal(0));
    }

    #[test]
    fn overgrowth_excludes_selected_plants_and_affects_ground_movement() {
        let mut b = Battle::new_roster_in(22, [31, 12], 0);
        b.map = Map::from_layout(0);
        let plant = Pos { x: 3, y: 1 };
        let other = Pos { x: 3, y: 5 };
        b.cast_plant_growth(0, Point::ground(plant));
        assert_ne!(
            b.roster_field_mask(0, GROWTH_CELLS) & (1 << index(plant)),
            0
        );
        b.roster_field_step(0, GROWTH_CELLS, ACTS[index(other)]);
        b.roster_field_step(0, GROWTH_CELLS, ACTS[63]);
        assert_eq!(b.overgrowth_cost(Point::ground(plant), 2), 4);
        assert_eq!(b.overgrowth_cost(Point::ground(other), 2), 2);
        assert_eq!(
            b.overgrowth_cost(
                Point {
                    cell: plant,
                    altitude: 1
                },
                1
            ),
            1
        );
    }
}
