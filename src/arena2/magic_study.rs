//! Viewer-side identification of non-instantaneous effects.
use super::super::battle::Battle;
use super::super::origins::StudyTarget;
use super::Concentration;

pub(super) const DIVINE_FAVOR: u8 = 28;
pub(super) const SHILLELAGH: u8 = 29;
pub(super) const LIGHT: u8 = 36;

pub(super) fn concentration_identity(concentration: Concentration) -> u8 {
    match concentration {
        Concentration::HungerOfHadar => 40,
        Concentration::FaerieFire => 41,
        Concentration::AlarmCasting => 48,
        Concentration::FogCloud => 42,
        Concentration::GustOfWind => 43,
        Concentration::Invisibility => 44,
        Concentration::PassWithoutTrace => 45,
        Concentration::CrusadersMantle => 46,
        Concentration::EnhanceAbility => 47,
        Concentration::HideousLaughter => 49,
        Concentration::CalmEmotions => 50,
        Concentration::Sleep => 51,
        Concentration::DetectThoughts => 52,
        Concentration::PhantasmalForce => 53,
        _ => concentration as u8 + 1,
    }
}

impl Battle {
    /// Observable concentration, plus private own state. A subtle ongoing
    /// spell has no dedicated flag in the opponent's view.
    pub fn public_concentration(&self, viewer: usize, subject: usize) -> bool {
        if viewer == subject && self.concentration_dropped_in_prompt(subject) {
            return false;
        }
        let Some(effect) = self.f[subject].concentration else {
            return false;
        };
        viewer == subject
            || self
                .study_candidates(viewer)
                .iter()
                .any(|target| target.id == concentration_identity(effect))
    }

    pub fn public_identified_spells(&self, viewer: usize) -> u64 {
        self.study_candidates(viewer)
            .iter()
            .filter(|&&target| self.knows_spell(viewer, target))
            .fold(0, |mask, target| mask | 1 << target.id)
    }

    pub(in crate::arena2) fn study_candidates(&self, viewer: usize) -> Vec<StudyTarget> {
        let subject = 1 - viewer;
        let fighter = &self.f[subject];
        let magic = &fighter.magic;
        let sees_subject = self.can_see(viewer, subject);
        let mut effects = Vec::new();

        if let Some(effect) = fighter.concentration {
            use Concentration::*;
            // A subtle buff or the process of casting is not by itself an
            // observable manifestation of a spell whose duration is ongoing.
            let centered = |name, clue| {
                self.visible_spell_field(viewer, subject).map(|_| (name, clue))
            };
            let on_body = |name, clue| sees_subject.then_some((name, clue));
            let observable = match effect {
                Moonbeam => centered("Moonbeam", "silver column of light"),
                SpiritualWeapon => centered("Spiritual Weapon", "floating spectral shape"),
                HungerOfHadar => centered("Hunger of Hadar", "sphere of darkness"),
                FaerieFire if self.faerie_outlined(viewer) => Some(("Faerie Fire", "outlines of light")),
                StinkingCloud => centered("Stinking Cloud", "lingering yellow cloud"),
                FogCloud => centered("Fog Cloud", "obscuring fog"),
                Entangle => centered("Entangle", "ground covered by animated growth"),
                SpiritGuardians => centered("Spirit Guardians", "orbiting shapes"),
                ShieldOfFaith => {
                    on_body("Shield of Faith", "shimmering field surrounding a creature")
                }
                Blur => on_body("Blur", "shimmering silhouette"),
                AlterSelf => on_body("Alter Self", "visibly transformed body"),
                Fly if fighter.elevation > 0
                    && !self.map.supported(self.fighter_point(subject)) =>
                {
                    on_body("Fly", "combatant suspended in the air")
                }
                HypnoticPattern if self.magic_incap(viewer) => {
                    Some(("Hypnotic Pattern", "persistent dazed effect"))
                }
                HideousLaughter if self.magic_area_incapacitated(viewer) => {
                    Some(("Tasha’s Hideous Laughter", "uncontrollable laughter"))
                }
                HoldPerson if self.magic_paralyzed(viewer) => {
                    Some(("Hold Person", "body immobilized without bindings"))
                }
                Fear if self.magic_frightened(viewer) => Some(("Fear", "persistent dread")),
                _ => None,
            };
            if let Some((identity, clue)) = observable {
                effects.push(StudyTarget {
                    id: concentration_identity(effect),
                    stamp: magic.expires,
                    identity,
                    clue,
                });
            }
        }

        if sees_subject {
            for (id, deadline, identity, clue) in [
                (
                    DIVINE_FAVOR,
                    magic.favor_until,
                    "Divine Favor",
                    "radiant coating on a weapon",
                ),
                (
                    SHILLELAGH,
                    magic.shillelagh_until,
                    "Shillelagh",
                    "enchanted wooden weapon",
                ),
                (LIGHT, magic.light_until, "Light", "glowing held object"),
            ] {
                if deadline > self.turns {
                    effects.push(StudyTarget {
                        id,
                        stamp: deadline,
                        identity,
                        clue,
                    });
                }
            }
        }
        if sees_subject && fighter.bard_ranger.disguise_until > self.turns {
            effects.push(StudyTarget {
                id: crate::arena2::bard_ranger::DISGUISE_STUDY_ID,
                stamp: fighter.bard_ranger.disguise_until,
                identity: "Disguise Self",
                clue: "apparent clothing and features",
            });
        }
        // Other active effects remain mechanically active. The spell text
        // establishes no persistent sensory cue for their numeric bonuses.
        effects
    }
}
