//! A viewer projection shared by the entity observation channels and terminal.
//
// Slot zero belongs to the viewer and slot one to the opposing side. An absent
// or unobserved entity has an all-zero row, unless a rule reveals limited
// information to that viewer (for example, Divine Sense).
use super::Batch;
use crate::arena2::{
    battle::Battle,
    entities::{Entity, EntityKind, EntityPhase, Spirit},
    geometry::{Pos, HEIGHT, MAX_ELEVATION, WIDTH},
};
use std::{ffi::c_char, slice};

pub(super) const ENTITY_FEATURES: usize = 64;
const ENTITY_FIELDS: usize = 23;

pub(super) fn phase_row(battle: &Battle, viewer: usize) -> Option<i8> {
    if battle.actor() != Some(viewer) {
        return None;
    }
    match battle.entity_phase()? {
        EntityPhase::Column { row, .. } | EntityPhase::PowerColumn { row, .. } => Some(row),
        _ => None,
    }
}

fn visible(battle: &Battle, viewer: usize, id: usize) -> Option<&Entity> {
    battle.observed_entity(viewer, id)
}

fn kind_code(kind: EntityKind) -> usize {
    match kind {
        EntityKind::Familiar { form, .. } => {
            let index = form as usize;
            if index < 3 {
                index
            } else {
                index + 3
            }
        }
        EntityKind::Steed { spirit, .. } => 3 + spirit_code(spirit),
    }
}

fn spirit_code(spirit: Spirit) -> usize {
    match spirit {
        Spirit::Celestial => 0,
        Spirit::Fey => 1,
        Spirit::Fiend => 2,
    }
}

fn projected(battle: &Battle, viewer: usize, id: usize, entity: &Entity) -> [i32; ENTITY_FIELDS] {
    let stats = entity.kind.stats();
    let spirit = match entity.kind {
        EntityKind::Familiar { spirit, .. } | EntityKind::Steed { spirit, .. } => spirit,
    };
    let active = battle.active_entity_id() == Some(id);
    let owns_turn = active && viewer == entity.side;
    [
        1,
        id as i32,
        entity.side as i32,
        kind_code(entity.kind) as i32,
        spirit_code(spirit) as i32,
        entity.position.x.into(),
        entity.position.y.into(),
        entity.altitude.into(),
        entity.hp,
        entity.max_hp(),
        battle.ac(id),
        entity.initiative,
        i32::from(entity.reaction),
        i32::from(entity.dodging),
        i32::from(entity.spirit_power),
        i32::from(battle.entities.mounted[entity.side]),
        i32::from(active),
        if owns_turn {
            battle.entity_movement_left(id)
        } else {
            -1
        },
        if owns_turn {
            i32::from(entity.turn.action)
        } else {
            -1
        },
        if owns_turn {
            i32::from(entity.turn.bonus)
        } else {
            -1
        },
        i32::from(stats.flyby),
        i32::from(entity.exhaustion),
        entity.temp_hp,
    ]
}

fn sensed(id: usize, contacts: &[(usize, Spirit, Pos)]) -> Option<(Spirit, Pos)> {
    contacts
        .iter()
        .find_map(|&(contact, spirit, position)| (contact == id).then_some((spirit, position)))
}

fn sensed_row(id: usize, spirit: Spirit, position: Pos) -> [i32; ENTITY_FIELDS] {
    let mut row = [-1; ENTITY_FIELDS];
    row[..7].copy_from_slice(&[
        2,
        id as i32,
        (id - 2) as i32,
        -1,
        spirit_code(spirit) as i32,
        position.x.into(),
        position.y.into(),
    ]);
    row
}

pub(super) fn collect_observation(
    battle: &Battle,
    viewer: usize,
    out: &mut [f32],
    temporary_hp: &mut [f32],
) {
    debug_assert_eq!(out.len(), ENTITY_FEATURES);
    debug_assert_eq!(temporary_hp.len(), 2);
    out.fill(0.0);
    temporary_hp.fill(0.0);
    let contacts = battle.divine_sense_types(viewer);
    for (slot, (side, row)) in [viewer, 1 - viewer]
        .into_iter()
        .zip(out.chunks_exact_mut(32))
        .enumerate()
    {
        let id = side + 2;
        let Some(entity) = visible(battle, viewer, id) else {
            if let Some((spirit, position)) = sensed(id, &contacts) {
                row[7 + spirit_code(spirit)] = 1.0;
                row[10] = f32::from(position.x) / f32::from(WIDTH - 1);
                row[11] = f32::from(position.y) / f32::from(HEIGHT - 1);
                row[29] = 1.0;
            }
            continue;
        };
        let raw = projected(battle, viewer, id, entity);
        temporary_hp[slot] = raw[22] as f32 / 40.0;
        let stats = entity.kind.stats();
        row[0] = 1.0;
        if raw[3] < 6 {
            row[1 + raw[3] as usize] = 1.0;
        }
        row[7 + raw[4] as usize] = 1.0;
        row[10] = raw[5] as f32 / f32::from(WIDTH - 1);
        row[11] = raw[6] as f32 / f32::from(HEIGHT - 1);
        row[12] = raw[7] as f32 / f32::from(MAX_ELEVATION);
        row[13] = raw[8] as f32 / raw[9] as f32;
        row[14] = raw[9] as f32 / 40.0;
        row[15] = raw[10] as f32 / 20.0;
        row[16] = raw[11] as f32 / 40.0;
        row[17] = stats.ground_speed as f32 / 20.0;
        row[18] = stats.fly_speed as f32 / 20.0;
        row[19] = stats.climb_speed as f32 / 20.0;
        for (feature, value) in row[20..25].iter_mut().zip(&raw[12..17]) {
            *feature = *value as f32;
        }
        row[25] = raw[17].max(0) as f32 / 20.0;
        row[26] = i32::from(raw[18] > 0) as f32;
        row[27] = i32::from(raw[19] > 0) as f32;
        row[28] = raw[20] as f32;
        row[30] = raw[3] as f32 / 27.0;
        row[31] = raw[21] as f32 / 6.0;
    }
}

#[no_mangle]
unsafe extern "C" fn arena_entities(
    batch: &Batch,
    lane: usize,
    viewer: usize,
    out: *mut i32,
    out_len: usize,
) -> bool {
    let Some(battle) = batch.0.get(lane) else {
        return false;
    };
    if viewer > 1 || out.is_null() || out_len != 2 * ENTITY_FIELDS {
        return false;
    }
    let out = slice::from_raw_parts_mut(out, out_len);
    out.fill(0);
    let contacts = battle.divine_sense_types(viewer);
    for (side, row) in [viewer, 1 - viewer]
        .into_iter()
        .zip(out.chunks_exact_mut(ENTITY_FIELDS))
    {
        let id = side + 2;
        if let Some(entity) = visible(battle, viewer, id) {
            row.copy_from_slice(&projected(battle, viewer, id, entity));
        } else if let Some((spirit, position)) = sensed(id, &contacts) {
            row.copy_from_slice(&sensed_row(id, spirit, position));
        }
    }
    true
}

#[no_mangle]
unsafe extern "C" fn arena_entity_name(
    batch: &Batch,
    lane: usize,
    viewer: usize,
    id: usize,
    out: *mut c_char,
    out_len: usize,
) -> bool {
    let Some(battle) = batch.0.get(lane) else {
        return false;
    };
    if viewer > 1 || out.is_null() || !(2..4).contains(&id) {
        return false;
    }
    let name = if let Some(entity) = visible(battle, viewer, id) {
        entity.kind.name()
    } else {
        match sensed(id, &battle.divine_sense_types(viewer)) {
            Some((Spirit::Celestial, _)) => "Celestial presence",
            Some((Spirit::Fiend, _)) => "Fiend presence",
            _ => return false,
        }
    };
    if name.len() >= out_len {
        return false;
    }
    std::ptr::copy_nonoverlapping(name.as_ptr(), out.cast(), name.len());
    *out.add(name.len()) = 0;
    true
}
