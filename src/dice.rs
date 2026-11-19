//! Per-duel SplitMix64. Sampling rejects the incomplete interval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dice(pub u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    pub fn roll(&mut self, sides: u64) -> i32 {
        let threshold = sides.wrapping_neg() % sides;
        loop {
            let value = self.next();
            if value >= threshold {
                return (value % sides + 1) as i32;
            }
        }
    }
}
