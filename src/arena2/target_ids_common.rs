use super::{sanctuary, Battle, Phase};
use crate::arena2::{
    entities::EntityPhase, martial, objects::END_OBJECTS, origins::MEND_TARGET,
    target_ids::TargetPermutation, Act,
};

impl Battle {
    pub(in crate::arena2) fn common_target_id_permutation(
        &self,
        actor: usize,
    ) -> TargetPermutation {
        let mut permutation = TargetPermutation::identity();
        if actor != 1 {
            return permutation;
        }

        // Each pair stores the seat-zero entry first internally. On the public
        // side the first entry belongs to the actor, including for issued items
        // that are now carried by another creature.
        let paired_choices = match self.phase {
            Phase::CreatureTarget(_) => 0..END_OBJECTS,
            Phase::HelpTarget | Phase::FlurryTarget { .. } => 0..4,
            Phase::ReadyTarget {
                action: Act::Melee | Act::Ranged | Act::Unarmed,
                ..
            } => 0..END_OBJECTS,
            Phase::ReadyTarget { action: Act::Special3 | Act::Special4, .. }
                if self.f[actor].build.class == crate::arena2::class::Class::Rogue => 0..END_OBJECTS,
            Phase::ReadyTarget {
                action: Act::Grapple | Act::ShovePush | Act::ShoveProne | Act::Help,
                ..
            } => 0..4,
            Phase::ReadyTarget {
                action: Act::Martial7,
                ..
            } => 8..10,
            Phase::Entity(
                EntityPhase::PowerTarget { .. }
                | EntityPhase::Strike(_)
                | EntityPhase::Help(_)
                | EntityPhase::SteedOpportunityTarget { .. },
            ) => 0..4,
            Phase::Decision { id, .. } => match id {
                66 | sanctuary::RETARGET => 0..END_OBJECTS,
                71 => 8..16,
                72 => self.weapon_option_paired_choices(actor),
                MEND_TARGET => 4..8,
                112 => 0..4,
                martial::fighter_rogue::TELEKINETIC_SUBJECT
                | martial::fighter_rogue::PSYCHIC_BLADE_TARGET => 0..END_OBJECTS,
                _ => martial::paired_martial_choices(id),
            },
            _ => 0..0,
        };
        for raw in paired_choices.step_by(2) {
            permutation.swap(raw, raw + 1);
        }
        permutation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::class::Class;

    #[test]
    fn ready_attack_reorders_targets_but_leaves_skip_and_ready_locations_alone() {
        let mut battle = Battle::new(0, [Class::Fighter; 2], [1; 2]);
        battle.phase = Phase::ReadyTarget {
            actor: 1,
            action: Act::Melee,
        };
        let mapping = battle.common_target_id_permutation(1);
        assert_eq!(mapping.encode(0), 1);
        assert_eq!(mapping.encode(2), 3);
        assert_eq!(mapping.encode(4), 5);
        assert_eq!(mapping.encode(Act::Other8 as usize), Act::Other8 as usize);

        battle.phase = Phase::ReadyTarget {
            actor: 1,
            action: Act::Utilize,
        };
        assert_eq!(battle.common_target_id_permutation(1).encode(4), 4);
    }

    #[test]
    fn shared_entity_phase_number_does_not_reorder_jump_choices() {
        let mut battle = Battle::new(0, [Class::Paladin; 2], [5; 2]);
        let power = EntityPhase::PowerTarget {
            side: 1,
            resume: false,
        };
        let jump = EntityPhase::Jump {
            side: 1,
            high: false,
        };
        assert_eq!(power.id(), jump.id());

        battle.set_entity_phase(Some(power));
        assert_eq!(battle.common_target_id_permutation(1).encode(2), 3);
        battle.set_entity_phase(Some(jump));
        assert_eq!(battle.common_target_id_permutation(1).encode(2), 2);
    }
}
