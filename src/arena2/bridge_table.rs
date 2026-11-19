//! Public geometry for a human-operated tabletop. No simulated rolls are previewed.
use super::Battle;
use crate::arena2::{
    geometry::{Point, Pos, HEIGHT, WALL_HEIGHT, WIDTH},
    ACTS,
};

pub(super) fn cover_map(b: &Battle, viewer: usize) -> String {
    let foe = 1 - viewer;
    let Some(cell) = b.observed_pos(viewer, foe) else {
        return String::new();
    };
    let source = Point {
        cell,
        altitude: b.f[foe].elevation,
    };
    let mut out = String::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let cell = Pos { x, y };
            let altitude = b.f[viewer].elevation.max(if b.map.tile(cell) == 1 {
                WALL_HEIGHT
            } else {
                0
            });
            out.push(char::from(
                b'0' + b.map.cover_at(source, Point { cell, altitude }) as u8,
            ));
        }
    }
    out
}

pub(super) fn target_columns(b: &Battle) -> String {
    if !matches!(b.phase_id(), 5 | 51 | 56 | 67 | 75 | 82 | 109) {
        return String::new();
    }
    let legal = b.mask();
    (0..HEIGHT as usize)
        .map(|row| {
            let mut col = 0;
            if legal & ACTS[row].bit() != 0 {
                // These phases only store the row. Inspect its column mask without
                // committing either choice or advancing the live random stream.
                let mut copy = b.clone();
                if copy.step(ACTS[row]).is_ok() && copy.phase_id() == b.phase_id() + 1 {
                    col = copy.mask() & 0x1ff;
                }
            }
            col.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn area_slices(b: &Battle) -> String {
    let Some((action, cell)) = b.target_altitude_request() else {
        return String::new();
    };
    let Some(area) = b.target_area(action, 0) else {
        return String::new();
    };
    let mask = b.mask();
    (0..7)
        .map(|altitude| {
            let origin = Point { cell, altitude };
            let mut tiles = 0u64;
            if mask & ACTS[altitude as usize].bit() != 0 {
                for y in 0..HEIGHT {
                    for x in 0..WIDTH {
                        let cell = Pos { x, y };
                        let floor = if b.map.tile(cell) == 1 {
                            WALL_HEIGHT
                        } else {
                            0
                        };
                        if b.area_hits_at(
                            origin,
                            Point {
                                cell,
                                altitude: floor,
                            },
                            area,
                        ) {
                            tiles |= 1 << (y * WIDTH + x);
                        }
                    }
                }
            }
            format!("{tiles:x}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Terrain-only edge masks for route planning. Occupancy is added from the
/// human projection, and every real step is revalidated against the live mask.
pub(super) fn ground_edges(b: &Battle) -> String {
    (0..HEIGHT)
        .flat_map(|y| {
            (0..WIDTH).map(move |x| {
                let bits = (0..8)
                    .filter(|&dir| b.map.step(Pos { x, y }, dir).is_some())
                    .fold(0u8, |v, dir| v | 1 << dir);
                format!("{bits:02x}")
            })
        })
        .collect()
}
