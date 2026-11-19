//! Action IDs at the player boundary. Inside the simulator a creature keeps its
//! physical seat ID across turns, interrupts and replayed decisions.
use super::{battle::Battle, Act, ACTS};

/// Fighters, their companions, and each pair of issued objects store the west
/// seat first. Swapping the low bit converts in either direction for east.
pub(super) fn relative_id(viewer: usize, id: usize) -> usize {
    id ^ viewer
}

const IDENTITY: [u8; 128] = {
    let mut table = [0; 128];
    let mut i = 0;
    while i < table.len() {
        table[i] = i as u8;
        i += 1;
    }
    table
};

/// The public numbering can change the representation as well as the order:
/// for example, an internal one-bit recipient set is a public creature ID.
pub(super) struct TargetPermutation {
    public: [u8; 128],
    raw: [u8; 128],
    changed: bool,
}

impl TargetPermutation {
    pub(super) fn identity() -> Self {
        Self {
            public: IDENTITY,
            raw: IDENTITY,
            changed: false,
        }
    }

    pub(super) fn swap(&mut self, a: usize, b: usize) {
        let (public_a, public_b) = (self.encode(a), self.encode(b));
        self.raw.swap(public_a, public_b);
        self.public.swap(a, b);
        self.changed |= a != b;
    }

    pub(super) fn set(&mut self, raw: usize, public: usize) {
        self.swap(raw, self.decode(public));
    }

    pub(super) fn encode(&self, raw: usize) -> usize {
        self.public[raw] as usize
    }

    pub(super) fn decode(&self, public: usize) -> usize {
        self.raw[public] as usize
    }

    fn mask(&self, mut raw: u128) -> u128 {
        if !self.changed {
            return raw;
        }
        let mut public = 0;
        while raw != 0 {
            let index = raw.trailing_zeros() as usize;
            public |= 1 << self.encode(index);
            raw &= raw - 1;
        }
        public
    }
}

impl Battle {
    pub(super) fn target_permutation(&self) -> TargetPermutation {
        // The responder in an off-turn window can be different from the turn owner.
        // Replayed save/temp-HP prompts and concentration menus have their own
        // answer codes even when suspended on top of a target-picking phase.
        let Some(actor) = self.actor() else {
            return TargetPermutation::identity();
        };
        if self.phase_id() == 0
            || self.save_choice_actor().is_some()
            || self.save_choice.concentration_menu
        {
            return TargetPermutation::identity();
        }
        if matches!(self.phase_id(), 161 | 165 | 167 | 169 | 183 | 185) {
            let mut ids = TargetPermutation::identity();
            if actor == 1 {
                ids.swap(0, 1);
                ids.swap(2, 3);
            }
            return ids;
        }
        let common = self.common_target_id_permutation(actor);
        if common.changed {
            return common;
        }
        let mut ids = self.magic_target_id_permutation(actor);
        // Wild Shape gear subsets can overlap the free Drop Concentration action.
        // Keep that command at its normal ID when the public gear mask allows it.
        if self.phase_id() == 100
            && ids.encode(Act::DropConcentration as usize) != Act::DropConcentration as usize
            && self.optional_concentrator().is_some()
        {
            let gear = ids.mask(self.shape_mask(actor, 100));
            let drop = Act::DropConcentration as usize;
            let public = if gear & (1 << drop) == 0 { drop } else { 127 };
            ids.set(drop, public);
        }
        ids
    }

    /// The mask, labels and step method all use the current actor's target IDs.
    pub fn mask(&self) -> u128 {
        self.target_permutation().mask(self.raw_action_mask())
    }

    pub fn step(&mut self, choice: Act) -> Result<(), &'static str> {
        let raw = self.target_permutation().decode(choice as usize);
        self.step_raw_action(ACTS[raw])
    }

    pub fn choice_label(&self, viewer: usize, choice: Act) -> String {
        let raw = self.target_permutation().decode(choice as usize);
        self.raw_choice_label(viewer, ACTS[raw])
    }
}
