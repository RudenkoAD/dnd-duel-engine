use dnd_duel_engine::arena2::{battle::Battle, bot, class::CLASSES};
use std::time::Instant;

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "50000".into())
        .parse()
        .expect("positive duel count");
    assert!(n > 0);
    let mut decisions = 0u64;
    let mut wins = [0u64; 3];
    let start = Instant::now();
    for idx in 0..n {
        let k = idx as usize;
        let classes = [CLASSES[k % 12], CLASSES[k / 12 % 12]];
        let level = (k / 144 % 5 + 1) as i32;
        let mut b = Battle::new(idx, classes, [level; 2]);
        while b.actor().is_some() {
            let action = bot::choose(&b, false);
            b.step(action).unwrap();
            decisions += 1;
        }
        wins[b.winner().unwrap_or(2)] += 1;
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!("duels={n} wins={wins:?} decisions={decisions} seconds={elapsed:.3} decisions/s={:.0} duels/s={:.0}",
        decisions as f64/elapsed, n as f64/elapsed);
}
