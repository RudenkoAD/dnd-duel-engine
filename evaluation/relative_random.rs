// Appended to a temporary copy of bridge.rs for the held-out evaluator.
#[no_mangle]
unsafe extern "C" fn arena_evaluation_random(
    batch: &Batch,
    ranks: *const i32,
    out: *mut i32,
) {
    for (i, battle) in batch.0.iter().enumerate() {
        let rank = *ranks.add(i);
        if rank < 0 {
            continue;
        }
        let mut legal = RAW_MASK;
        for _ in 0..rank {
            legal &= legal - 1;
        }
        assert!(legal != 0, "random rank outside the physical legal mask");
        let raw_id = legal.trailing_zeros() as usize;
        *out.add(i) = PUBLIC_ID as i32;
    }
}
