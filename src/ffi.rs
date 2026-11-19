//! Each handle has one owner. The owner serializes access and frees it once.
use crate::{Duel, Phase, ACTIONS};

#[no_mangle]
extern "C" fn duel_abi_version() -> u32 {
    1
}
#[no_mangle]
extern "C" fn duel_new(seed: u64) -> *mut Duel {
    Box::into_raw(Box::new(Duel::new(seed)))
}
#[no_mangle]
unsafe extern "C" fn duel_free(handle: *mut Duel) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}
#[no_mangle]
unsafe extern "C" fn duel_step(handle: *mut Duel, action: u32) -> bool {
    let (Some(d), Some(&choice)) = (handle.as_mut(), ACTIONS.get(action as usize)) else {
        return false;
    };
    d.step(choice).is_ok()
}
#[no_mangle]
unsafe extern "C" fn duel_bot(handle: *const Duel) -> i32 {
    handle
        .as_ref()
        .and_then(|d| d.bot())
        .map_or(-1, |a| a as i32)
}
// STATE_FIELDS in native.py names these indices. State and mask describe the same decision.
#[no_mangle]
unsafe extern "C" fn duel_read(handle: *const Duel, output: *mut i32, length: usize) -> i32 {
    let Some(d) = handle.as_ref() else { return -1 };
    if output.is_null() || length != 24 {
        return -1;
    }
    let buffer = std::slice::from_raw_parts_mut(output, length);
    let (phase, winner) = match d.phase() {
        Phase::Main => (0, -1),
        Phase::Reaction { .. } => (1, -1),
        Phase::Finished { winner } => (2, winner as i32),
    };
    let t = d.turn();
    buffer[..10].copy_from_slice(&[
        d.actor().map_or(-1, |p| p as i32),
        t.owner as i32,
        phase,
        winner,
        d.turns_started().min(i32::MAX as u32) as i32,
        d.distance(),
        d.movement_left(),
        t.action as i32,
        t.bonus as i32,
        t.movement_spent,
    ]);
    for (slot, f) in buffer[10..].chunks_exact_mut(7).zip(d.fighters()) {
        slot.copy_from_slice(&[
            f.hp,
            f.position,
            f.winds,
            f.arrows,
            f.reaction as i32,
            f.sapped as i32,
            f.slowed as i32,
        ]);
    }
    d.legal_mask() as i32
}
