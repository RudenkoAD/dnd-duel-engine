//! Extended casting and entry alerts for the Clockwork Sorcerer's Alarm.
use super::{Act, Area, Battle, Concentration, MagicChoice, Point, Spell};

const CAST_ACTIONS: u8 = 10;

#[derive(Clone, Copy, Debug)]
struct Ward {
    center: Point,
    audible: bool,
    exempt: u8,
    expires: i32,
    last_trigger: Option<i32>,
}

#[derive(Clone, Copy, Debug)]
struct AlarmCast {
    center: Point,
    audible: bool,
    exempt: u8,
    actions: u8,
    last_action: i32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct AlarmState {
    wards: Vec<Ward>,
    casting: Option<AlarmCast>,
}

impl Battle {
    pub(in crate::arena2) fn alarm_cast_in_progress(&self, i: usize) -> bool {
        self.f[i].concentration == Some(Concentration::AlarmCasting)
            && self.f[i].magic.alarm.casting.is_some()
    }

    pub(super) fn alarm_initialize(&mut self, i: usize, center: Point, audible: bool, exempt: u8) {
        self.concentrate(i, Concentration::AlarmCasting, 10, None, 1);
        self.f[i].magic.alarm.casting = Some(AlarmCast {
            center,
            audible,
            exempt,
            actions: 1,
            last_action: self.turns,
        });
        self.action = false;
        self.reveal(i);
        self.magic_end_invisibility_for(i);
    }

    pub(super) fn alarm_work(&mut self, i: usize) {
        if self.alarm_cast_in_progress(i) {
            let progress = self.f[i]
                .magic
                .alarm
                .casting
                .as_mut()
                .expect("extended cast");
            progress.actions += 1;
            progress.last_action = self.turns;
            if progress.actions == CAST_ACTIONS {
                self.cast(i, Spell::Alarm, false, false);
            } else {
                self.action = false;
                self.reveal(i);
            }
        } else {
            let center = self.chosen_center();
            self.magic_request(i, MagicChoice::AlarmMode { center });
        }
    }

    pub(super) fn alarm_end_turn(&mut self, i: usize) {
        if self.alarm_cast_in_progress(i)
            && (self.incap(i)
                || self.f[i]
                    .magic
                    .alarm
                    .casting
                    .is_some_and(|cast| cast.last_action != self.turns))
        {
            self.alarm_cancel(i);
        } else if !self.alarm_cast_in_progress(i) {
            self.f[i].magic.alarm.casting = None;
        }
    }

    pub(super) fn alarm_cancel(&mut self, i: usize) {
        self.f[i].magic.alarm.casting = None;
        if self.f[i].concentration == Some(Concentration::AlarmCasting) {
            self.f[i].concentration = None;
        }
    }

    pub(super) fn finish_alarm(&mut self, i: usize) {
        let Some(cast) = self.f[i].magic.alarm.casting else {
            return;
        };
        self.alarm_cancel(i);
        self.f[i].magic.alarm.wards.push(Ward {
            center: cast.center,
            audible: cast.audible,
            exempt: cast.exempt,
            expires: self.turns + 9_600,
            last_trigger: None,
        });
    }

    pub(super) fn alarm_exemption_mask(&self, caster: usize) -> u128 {
        let mut exists = 0b0011;
        for subject in 2..4 {
            if self.observed_entity(caster, subject).is_some() {
                exists |= 1 << subject;
            }
        }
        (0..16)
            .filter(|&subset| subset & !exists == 0)
            .fold(Act::Pass.bit(), |mask, subset| {
                mask | (1 << if subset == 15 { 31 } else { subset })
            })
    }

    pub(super) fn alarm_exemption_name(&self, caster: usize, index: usize) -> String {
        let subset = if index == 31 { 15 } else { index };
        if subset == 0 {
            "Exempt nobody".into()
        } else {
            let names = (0..4)
                .filter(|&id| subset & (1 << id) != 0)
                .map(|id| {
                    if id == caster {
                        "yourself".into()
                    } else if id == 1 - caster {
                        "your opponent".into()
                    } else {
                        self.creature_label(id)
                    }
                })
                .collect::<Vec<_>>()
                .join(" and ");
            format!("Exempt {names}")
        }
    }

    pub(in crate::arena2) fn alarm_moved(&mut self, subject: usize, previous: Point) {
        let Some(current) = self.footprint(subject) else {
            return;
        };
        let mut prior = current;
        prior.anchor = previous;
        let area = Area::Cube { half_side: 2 };
        for fighter in &mut self.f {
            for ward in &mut fighter.magic.alarm.wards {
                let touches = |body: crate::arena2::footprint::Footprint| {
                    body.squares()
                        .any(|point| area.contains_at(ward.center, point))
                };
                if ward.expires > self.turns
                    && ward.exempt & (1 << subject) == 0
                    && touches(current)
                    && !touches(prior)
                {
                    ward.last_trigger = Some(self.turns);
                }
            }
        }
    }

    pub(in crate::arena2) fn alarm_features(&self, viewer: usize) -> [f32; 8] {
        let state = &self.f[viewer].magic.alarm;
        let progress = if self.alarm_cast_in_progress(viewer) {
            state.casting.map_or(0, |cast| cast.actions)
        } else {
            0
        };
        let triggered = |ward: &Ward| ward.last_trigger.is_some_and(|turn| turn + 4 > self.turns);
        let ward = state
            .wards
            .iter()
            .filter(|ward| ward.expires > self.turns)
            .max_by_key(|ward| (triggered(ward), ward.last_trigger, ward.expires));
        [
            f32::from(progress) / f32::from(CAST_ACTIONS),
            f32::from(u8::from(ward.is_some())),
            ward.map_or(0.0, |ward| f32::from(ward.center.cell.x) / 8.0),
            ward.map_or(0.0, |ward| f32::from(ward.center.cell.y) / 6.0),
            ward.map_or(0.0, |ward| f32::from(ward.center.altitude) / 6.0),
            self.f[viewer].arcane.balance as f32 / 5.0,
            f32::from(u8::from(ward.is_some_and(triggered))),
            f32::from(u8::from(ward.is_some_and(|ward| ward.audible))),
        ]
    }

    pub(super) fn alarm_summary(&self, viewer: usize, source: usize) -> Vec<String> {
        let state = &self.f[source].magic.alarm;
        let mut lines = Vec::new();
        if viewer == source && self.alarm_cast_in_progress(source) {
            if let Some(cast) = state.casting {
                lines.push(format!(
                    "Casting Alarm: {}/{} actions",
                    cast.actions, CAST_ACTIONS
                ));
            }
        }
        for ward in state.wards.iter().filter(|w| w.expires > self.turns) {
            let point = ward.center;
            let coordinate = format!(
                "{}{} at {} ft",
                char::from(b'A' + point.cell.x as u8),
                point.cell.y + 1,
                point.altitude * 5
            );
            let triggered = ward.last_trigger.is_some_and(|turn| turn + 4 > self.turns);
            if viewer == source {
                lines.push(if triggered {
                    format!("Alarm triggered near {coordinate}")
                } else {
                    format!("Alarm ward at {coordinate}")
                });
            } else if ward.audible && triggered && self.fighter_point(viewer).dist(point) <= 12 {
                lines.push(format!("Alarm bell near {coordinate}"));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::{class::Class, geometry::Pos};

    fn duel() -> Battle {
        let mut duel = Battle::new_roster_in(
            11,
            [Class::Sorcerer as u8 * 3 + 2, Class::Fighter as u8 * 3],
            0,
        );
        duel.owner = 0;
        duel.f[0].pos = Pos { x: 0, y: 0 };
        duel.f[1].pos = Pos { x: 7, y: 0 };
        duel
    }

    #[test]
    fn alarm_uses_ten_actions_and_only_reports_entry_without_revealing_the_creature() {
        let mut duel = duel();
        let initial = duel.f[0].slots;
        for command in [Act::Magic19, Act::N, Act::SE, Act::N, Act::N, Act::NE] {
            duel.step(command).unwrap();
        }
        assert_eq!(duel.alarm_features(0)[0], 0.1);
        for _ in 1..10 {
            assert_eq!(duel.f[0].slots, initial);
            duel.step(Act::End).unwrap();
            duel.step(Act::End).unwrap();
            assert_ne!(duel.mask() & Act::Magic19.bit(), 0);
            duel.step(Act::Magic19).unwrap();
        }
        assert_eq!(duel.f[0].slots[0], initial[0] - 1);
        assert!(!duel.alarm_cast_in_progress(0));
        assert!(!duel.magic_summary(0).contains("Alarm triggered"));

        let previous = duel.fighter_point(1);
        duel.f[1].pos = Pos { x: 5, y: 0 };
        duel.f[1].hidden_dc = Some(20);
        duel.alarm_moved(1, previous);
        assert!(duel.magic_summary(0).contains("Alarm triggered"));
        assert_eq!(duel.alarm_features(0)[6], 1.0);
        assert!(!duel.magic_public_summary(1, 0).contains("Alarm"));
        assert_eq!(duel.observed_pos(0, 1), None);
    }

    #[test]
    fn alarm_ends_without_spending_a_slot_when_the_caster_interrupts_work() {
        let mut duel = duel();
        let initial = duel.f[0].slots;
        let center = Point::ground(Pos { x: 3, y: 0 });
        duel.alarm_initialize(0, center, true, 0);
        duel.step(Act::End).unwrap();
        duel.step(Act::End).unwrap();
        duel.step(Act::End).unwrap();
        assert_eq!(duel.f[0].slots, initial);
        assert!(!duel.alarm_cast_in_progress(0));
        assert!(!duel.magic_summary(0).contains("Alarm ward"));
    }
}
