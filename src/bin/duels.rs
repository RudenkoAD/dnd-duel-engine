use dnd_duel_engine::{Duel, Phase};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let count: u64 = args
        .next()
        .as_deref()
        .unwrap_or("10000")
        .parse()
        .expect("positive duel count");
    let seed: u64 = args
        .next()
        .as_deref()
        .unwrap_or("0")
        .parse()
        .expect("unsigned base seed");
    assert!(
        count > 0 && args.next().is_none(),
        "usage: duels [positive-count] [base-seed]"
    );
    let mut wins = [0u64; 2];
    let mut decisions = 0u64;
    let start = Instant::now();
    for index in 0..count {
        let mut duel = Duel::new(seed.wrapping_add(index));
        for _ in 0..2048 {
            let Some(action) = duel.bot() else { break };
            duel.step(action).unwrap();
            decisions += 1;
        }
        if let Phase::Finished { winner } = duel.phase() {
            wins[winner] += 1;
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    println!("duels={count} seed={seed} shield={} archer={} capped={} decisions={decisions} seconds={seconds:.6} decisions/s={:.0} duels/s={:.0}",
        wins[0], wins[1], count-wins[0]-wins[1], decisions as f64/seconds, count as f64/seconds);
}
