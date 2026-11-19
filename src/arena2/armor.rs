//! Timed donning and doffing, armor training, and transferable body armor.

use super::{
    battle::Battle,
    class::{Build, Class},
    geometry::Point,
    origins::raw_scores,
    Act, ACTS,
};

const MENU_PHASE: i32 = 112;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
enum ArmorWeight {
    Unarmored,
    Light,
    Medium,
    Heavy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArmorKind {
    None,
    Leather,
    Studded,
    Scale,
    Chain,
}

impl ArmorKind {
    fn issued(class: Class) -> Self {
        use Class::*;
        match class {
            Bard | Druid | Warlock => Self::Leather,
            Ranger | Rogue => Self::Studded,
            Cleric => Self::Scale,
            Fighter | Paladin => Self::Chain,
            _ => Self::None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Leather => "Leather",
            Self::Studded => "Studded leather",
            Self::Scale => "Scale mail",
            Self::Chain => "Chain mail",
        }
    }

    fn weight(self) -> ArmorWeight {
        match self {
            Self::None => ArmorWeight::Unarmored,
            Self::Leather | Self::Studded => ArmorWeight::Light,
            Self::Scale => ArmorWeight::Medium,
            Self::Chain => ArmorWeight::Heavy,
        }
    }

    fn time(self, don: bool) -> i32 {
        match (self.weight(), don) {
            (ArmorWeight::Unarmored, _) => 0,
            (ArmorWeight::Medium, true) | (ArmorWeight::Heavy, false) => 50,
            (ArmorWeight::Heavy, true) => 100,
            _ => 10,
        }
    }

    fn ac(self, build: Build) -> i32 {
        let dex = build.stats[1];
        let defense = i32::from(
            build.class == Class::Fighter || (build.class == Class::Paladin && build.level >= 2),
        );
        match self {
            Self::Leather => 11 + dex + defense,
            Self::Studded => 12 + dex + defense,
            Self::Scale => 14 + dex.min(2) + defense,
            Self::Chain => 16 + defense,
            Self::None => unarmored_ac(build, false),
        }
    }
}

fn unarmored_ac(build: Build, shield: bool) -> i32 {
    10 + build.stats[1]
        + match build.class {
            Class::Barbarian => build.stats[2],
            Class::Monk if !shield => build.stats[4],
            Class::Sorcerer if build.variant == 0 && build.level >= 3 => build.stats[5],
            _ => 0,
        }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Location {
    Absent,
    Worn,
    Packed,
    ForeignWorn,
    ForeignPacked,
    Ground(Point),
}

#[derive(Clone, Copy, Debug)]
struct ArmorWork {
    issuer: usize,
    don: bool,
    remaining: i32,
    worked_this_turn: bool,
}

#[derive(Clone, Copy, Debug)]
enum Menu {
    Don,
    Handle,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ArmorState {
    kit: ArmorKind,
    location: Location,
    work: Option<ArmorWork>,
    menu: Option<Menu>,
}

impl ArmorState {
    pub(super) fn new(build: Build) -> Self {
        let kit = ArmorKind::issued(build.class);
        Self {
            kit,
            location: if kit == ArmorKind::None {
                Location::Absent
            } else {
                Location::Worn
            },
            work: None,
            menu: None,
        }
    }
}

impl Battle {
    fn armor_wearer(&self, issuer: usize) -> Option<usize> {
        match self.f[issuer].armor.location {
            Location::Worn => Some(issuer),
            Location::ForeignWorn => Some(1 - issuer),
            _ => None,
        }
    }

    pub(super) fn armor_holder(&self, issuer: usize) -> Option<usize> {
        match self.f[issuer].armor.location {
            Location::Packed | Location::Worn => Some(issuer),
            Location::ForeignPacked | Location::ForeignWorn => Some(1 - issuer),
            _ => None,
        }
    }

    fn worn_suit(&self, i: usize) -> Option<(usize, ArmorKind)> {
        (0..2)
            .find(|&issuer| self.armor_wearer(issuer) == Some(i))
            .map(|issuer| (issuer, self.f[issuer].armor.kit))
    }

    pub(in crate::arena2) fn armor_worn(&self, i: usize) -> bool {
        self.worn_suit(i).is_some() && !self.magic_is_wolf(i)
    }

    pub(in crate::arena2) fn armor_heavy(&self, i: usize) -> bool {
        !self.magic_is_wolf(i)
            && self
                .worn_suit(i)
                .is_some_and(|(_, kit)| kit.weight() == ArmorWeight::Heavy)
    }

    pub(in crate::arena2) fn armor_untrained(&self, i: usize) -> bool {
        if self.magic_is_wolf(i) {
            return false;
        }
        let Some((_, kit)) = self.worn_suit(i) else {
            return false;
        };
        use Class::*;
        let build = self.f[i].build;
        let tolerated = match build.class {
            Cleric | Fighter | Paladin => ArmorWeight::Heavy,
            Barbarian | Druid | Ranger => ArmorWeight::Medium,
            Bard if build.variant == 1 => ArmorWeight::Medium,
            Bard | Rogue | Warlock => ArmorWeight::Light,
            Monk | Sorcerer | Wizard => ArmorWeight::Unarmored,
        };
        kit.weight() as i32 > tolerated as i32
    }

    pub(in crate::arena2) fn armor_disadvantages_ability(&self, i: usize, modifier: usize) -> bool {
        modifier < 2 && self.armor_untrained(i)
    }

    pub(super) fn armor_speed_penalty(&self, i: usize) -> i32 {
        let build = self.f[i].build;
        if self.magic_is_wolf(i) {
            return 0;
        }
        let heavy = self.armor_heavy(i);
        let strength = raw_scores(build.class, build.level)[0];
        let slow = heavy && strength < 13;
        let movement_bonus_lost = (build.class == Class::Monk
            && build.level >= 2
            && (self.armor_worn(i) || self.gear_shield_worn(i)))
            || (build.class == Class::Barbarian && build.level >= 5 && heavy);
        2 * (i32::from(slow) + i32::from(movement_bonus_lost))
    }

    pub fn armor_adjust(&self, i: usize) -> i32 {
        let build = self.f[i].build;
        let issued = ArmorKind::issued(build.class);
        let initial = issued.ac(build);
        let current = self.worn_suit(i).map_or_else(
            || unarmored_ac(build, self.gear_shield_worn(i)),
            |(_, kit)| kit.ac(build),
        );
        current - initial
    }

    pub(super) fn armor_stealth_disadvantage(&self, i: usize) -> bool {
        !self.magic_is_wolf(i)
            && self.worn_suit(i).is_some_and(|(_, kit)| {
                matches!(kit.weight(), ArmorWeight::Medium | ArmorWeight::Heavy)
            })
    }

    pub fn armor_projection(&self, i: usize) -> [i32; 4] {
        let state = self.f[i].armor;
        let worn = self.worn_suit(i);
        let (task, remaining) = state.work.map_or((0, 0), |work| {
            (if work.don { 1 } else { 2 }, work.remaining)
        });
        [
            worn.map_or(state.kit, |(_, kit)| kit).weight() as i32,
            i32::from(worn.is_some()),
            task,
            remaining,
        ]
    }

    pub fn armor_status(&self, i: usize) -> String {
        let state = self.f[i].armor;
        let worn = self.worn_suit(i);
        let mut description = if let Some((issuer, kit)) = worn {
            let borrowed = if issuer == i { "" } else { " (borrowed)" };
            let training = if self.armor_untrained(i) {
                ", untrained"
            } else {
                ""
            };
            format!("{} worn{borrowed}{training}", kit.name())
        } else if let Some(issuer) = (0..2).find(|&issuer| self.armor_holder(issuer) == Some(i)) {
            format!("{} packed", self.f[issuer].armor.kit.name())
        } else if matches!(state.location, Location::Ground(_)) {
            format!("{} on the ground", state.kit.name())
        } else {
            "Body armor none".into()
        };
        if let Some(work) = state.work {
            let verb = if work.don { "donning" } else { "doffing" };
            description.push_str(&format!(" · {verb}, {} turns remaining", work.remaining));
        }
        description
    }

    pub fn armor_ground(&self, issuer: usize) -> Option<Point> {
        match self.f[issuer].armor.location {
            Location::Ground(point) => Some(point),
            _ => None,
        }
    }

    pub fn armor_ground_name(&self, issuer: usize) -> &'static str {
        self.f[issuer].armor.kit.name()
    }

    pub fn armor_ground_flammable(&self, issuer: usize) -> bool {
        matches!(
            self.f[issuer].armor.kit,
            ArmorKind::Leather | ArmorKind::Studded
        )
    }

    pub fn armor_present(&self, issuer: usize) -> bool {
        self.f[issuer].armor.location != Location::Absent
    }

    pub fn armor_destroy(&mut self, issuer: usize) {
        self.f[issuer].armor.location = Location::Absent;
        for fighter in &mut self.f {
            if fighter.armor.work.is_some_and(|work| work.issuer == issuer) {
                fighter.armor.work = None;
            }
        }
    }

    pub fn armor_move_ground(&mut self, issuer: usize, point: Point) {
        if self.armor_ground(issuer).is_some() {
            self.f[issuer].armor.location = Location::Ground(point);
        }
    }

    pub fn armor_carried(&self, actor: usize) -> bool {
        (0..2).any(|issuer| self.armor_holder(issuer) == Some(actor))
    }

    pub fn armor_holds_foreign(&self, actor: usize) -> bool {
        self.armor_holder(1 - actor) == Some(actor)
    }

    fn armor_retrievable(&self, i: usize, issuer: usize) -> bool {
        self.armor_ground(issuer)
            .is_some_and(|point| self.fighter_point(i) == point)
    }

    fn don_candidates(&self, i: usize) -> u128 {
        (0..2)
            .filter(|&issuer| self.armor_holder(issuer) == Some(i))
            .fold(0, |mask, issuer| mask | 1 << issuer)
    }

    fn handle_candidates(&self, i: usize) -> u128 {
        if !(self.action || self.f[i].gear.interaction) {
            return 0;
        }
        (0..2).fold(0, |mask, issuer| {
            let pickup = self.armor_retrievable(i, issuer);
            let packed =
                self.armor_holder(issuer) == Some(i) && self.armor_wearer(issuer).is_none();
            mask | if pickup {
                1 << issuer
            } else if packed {
                1 << (issuer + 2)
            } else {
                0
            }
        })
    }

    pub(super) fn armor_mask(&self, i: usize) -> u128 {
        if self.magic_is_wolf(i) {
            return 0;
        }
        let work = self.f[i].armor.work;
        let handle = work.is_some() || self.handle_candidates(i) != 0;
        let mut mask = if handle { Act::Other5.bit() } else { 0 };
        if self.action && self.moved == 0 && !work.is_some_and(|project| project.worked_this_turn) {
            if self.worn_suit(i).is_some() {
                mask |= Act::Other4.bit();
            } else if self.don_candidates(i) != 0 {
                mask |= Act::Other3.bit();
            }
        }
        mask
    }

    pub(super) fn armor_step(&mut self, i: usize, action: Act) {
        let work = self.f[i].armor.work;
        let don = action == Act::Other3;
        match action {
            Act::Other5 if work.is_some() => self.armor_interrupt(i),
            Act::Other5 => {
                let candidates = self.handle_candidates(i);
                if candidates.count_ones() == 1 {
                    self.handle_armor(i, candidates.trailing_zeros() as usize);
                } else {
                    self.request_armor(i, Menu::Handle);
                }
            }
            Act::Other3 if work.is_none() && self.don_candidates(i).count_ones() > 1 => {
                self.request_armor(i, Menu::Don)
            }
            Act::Other3 | Act::Other4 => {
                let issuer = work.map(|project| project.issuer).unwrap_or_else(|| {
                    if don {
                        self.don_candidates(i).trailing_zeros() as usize
                    } else {
                        self.worn_suit(i).expect("masked doffing").0
                    }
                });
                self.work_on_armor(i, issuer, don);
            }
            _ => unreachable!("validated armor action"),
        }
    }

    fn request_armor(&mut self, i: usize, menu: Menu) {
        self.f[i].armor.menu = Some(menu);
        self.request(i, MENU_PHASE);
    }

    fn work_on_armor(&mut self, i: usize, issuer: usize, don: bool) {
        let initial = self.f[issuer].armor.kit.time(don);
        let state = &mut self.f[i].armor;
        let remaining = state.work.map_or(initial, |work| work.remaining);
        state.work = Some(ArmorWork {
            issuer,
            don,
            remaining,
            worked_this_turn: true,
        });
        self.action = false;
    }

    fn handle_armor(&mut self, i: usize, id: usize) {
        let issuer = id % 2;
        self.f[issuer].armor.location = if id < 2 {
            if issuer == i {
                Location::Packed
            } else {
                Location::ForeignPacked
            }
        } else {
            Location::Ground(self.map.landing(self.fighter_point(i)))
        };
        if self.f[i].gear.interaction {
            self.f[i].gear.interaction = false;
        } else {
            self.action = false;
        }
    }

    pub(super) fn armor_decision_mask(&self, i: usize) -> u128 {
        match self.f[i].armor.menu {
            Some(Menu::Don) => self.don_candidates(i) | Act::Pass.bit(),
            Some(Menu::Handle) => self.handle_candidates(i) | Act::Pass.bit(),
            None => 0,
        }
    }

    pub(super) fn armor_decision_step(&mut self, i: usize, action: Act) {
        let mode = self.f[i].armor.menu.take().expect("armor menu");
        if action == Act::Pass {
            return;
        }
        let choice = action as usize;
        match mode {
            Menu::Don => self.work_on_armor(i, choice, true),
            Menu::Handle => self.handle_armor(i, choice),
        }
    }

    pub(super) fn armor_decision_label(&self, i: usize, action: Act) -> Option<String> {
        let menu = self.f[i].armor.menu?;
        if action == Act::Pass {
            return Some("Cancel armor choice".into());
        }
        let id = action as usize;
        let issuer = id % 2;
        let suit = self.f[issuer].armor.kit.name();
        let who = if issuer == i {
            "issued"
        } else {
            "other duelist's"
        };
        Some(match menu {
            Menu::Don => format!(
                "Don {who} {suit} ({} turns)",
                self.f[issuer].armor.kit.time(true)
            ),
            Menu::Handle if id < 2 => format!("Pick up {who} {suit}"),
            Menu::Handle => format!("Drop {who} {suit}"),
        })
    }

    pub fn armor_candidate(&self, i: usize, id: usize) -> [f32; 8] {
        if id >= 4 || self.armor_decision_mask(i) & ACTS[id].bit() == 0 {
            return [0.; 8];
        }
        let issuer = id % 2;
        let mut x = [0.; 8];
        x[0] = 1.;
        x[1] = u8::from(i != issuer).into();
        x[2] = self.f[issuer].armor.kit.weight() as i32 as f32 / 3.;
        x[3] = u8::from(id >= 2).into();
        x[4] = self.f[issuer].armor.kit.time(true) as f32 / 100.;
        x
    }

    pub(super) fn drop_form_armor(&mut self,issuer:usize,point:Point) {self.f[issuer].armor.location=Location::Ground(point);}

    pub(super) fn armor_drop_for_wild_shape(&mut self, i: usize) {
        let at = self.map.landing(self.fighter_point(i));
        for issuer in 0..2 {
            if self.armor_holder(issuer) == Some(i) {
                self.f[issuer].armor.location = Location::Ground(at);
            }
        }
        self.armor_interrupt(i);
    }

    pub(super) fn armor_interrupt(&mut self, i: usize) {
        self.f[i].armor.work = None;
    }

    pub(super) fn armor_end(&mut self, i: usize) {
        let Some(mut work) = self.f[i].armor.work.take() else {
            return;
        };
        if !work.worked_this_turn || self.incap(i) {
            return;
        }
        work.remaining -= 1;
        if work.remaining == 0 {
            self.f[work.issuer].armor.location = match (work.don, work.issuer == i) {
                (true, true) => Location::Worn,
                (true, false) => Location::ForeignWorn,
                (false, true) => Location::Packed,
                (false, false) => Location::ForeignPacked,
            };
            if work.don {
                self.magic_end_mage_armor(i);
                if self.armor_heavy(i) {
                    self.f[i].raging = false;
                }
            }
        } else {
            work.worked_this_turn = false;
            self.f[i].armor.work = Some(work);
        }
    }

    pub fn armor_label(&self, i: usize, action: Act) -> Option<String> {
        let state = self.f[i].armor;
        match action {
            Act::Other3 | Act::Other4 => {
                let don = action == Act::Other3;
                let verb = if don { "don" } else { "doff" };
                if don && state.work.is_none() && self.don_candidates(i).count_ones() > 1 {
                    return Some("Choose body armor to don".into());
                }
                let issuer = state.work.map(|work| work.issuer).or_else(|| {
                    if don {
                        (0..2).find(|&issuer| self.armor_holder(issuer) == Some(i))
                    } else {
                        self.worn_suit(i).map(|(issuer, _)| issuer)
                    }
                })?;
                let kit = self.f[issuer].armor.kit;
                let (prefix, remaining) = state.work.map_or_else(
                    || ("Start", kit.time(don)),
                    |work| ("Continue", work.remaining),
                );
                Some(format!(
                    "{prefix} to {verb} {} (action, {remaining} turns remaining)",
                    kit.name()
                ))
            }
            Act::Other5 if state.work.is_some() => Some("Cancel work on body armor".into()),
            Act::Other5 => {
                let options = self.handle_candidates(i);
                if options.count_ones() > 1 {
                    return Some("Handle body armor".into());
                }
                let id = options.trailing_zeros() as usize;
                if id >= 4 {
                    return None;
                }
                let verb = if id < 2 { "Pick up" } else { "Drop" };
                let cost = if self.f[i].gear.interaction {
                    "free interaction"
                } else {
                    "Utilize action"
                };
                Some(format!("{verb} body armor ({cost})"))
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::class::{build, CLASSES};

    fn starts(first: Class) -> Battle {
        (0..100)
            .map(|seed| Battle::new(seed, [first, Class::Barbarian], [3, 3]))
            .find(|battle| battle.owner == 0)
            .expect("first seat wins initiative at some seed")
    }

    fn round(battle: &mut Battle, action: Act) {
        assert_eq!(battle.actor(), Some(0));
        battle.step(action).unwrap();
        battle.step(Act::End).unwrap();
        assert_eq!(battle.actor(), Some(1));
        battle.step(Act::End).unwrap();
    }

    #[test]
    fn preset_armor_timing_matches_weight_and_does_not_change_starting_ac() {
        for class in CLASSES {
            for level in 1..=5 {
                let state = ArmorState::new(build(class, level));
                let expected = match state.kit.weight() {
                    ArmorWeight::Unarmored => (0, 0),
                    ArmorWeight::Light => (10, 10),
                    ArmorWeight::Medium => (50, 10),
                    ArmorWeight::Heavy => (100, 50),
                };
                assert_eq!((state.kit.time(true), state.kit.time(false)), expected);
                let battle = Battle::new(17, [class; 2], [level; 2]);
                assert_eq!(battle.armor_adjust(0), 0);
            }
        }
    }

    #[test]
    fn heavy_armor_loses_its_ac_and_stealth_penalty_only_after_full_doff_time() {
        let mut battle = starts(Class::Fighter);
        assert!(battle.armor_stealth_disadvantage(0));
        // Simulate 50 uninterrupted own turns at the state-machine level.
        // The actual arena's round limit allows only 30 such turns.
        for remaining in (1..=49).rev() {
            battle.action = true;
            battle.armor_step(0, Act::Other4);
            battle.armor_end(0);
            assert_eq!(battle.armor_projection(0), [3, 1, 2, remaining]);
            assert_eq!(battle.armor_adjust(0), 0);
            assert!(battle.armor_stealth_disadvantage(0));
        }
        battle.action = true;
        battle.armor_step(0, Act::Other4);
        battle.armor_end(0);
        assert_eq!(battle.armor_projection(0), [3, 0, 0, 0]);
        assert_eq!(battle.f[0].build.ac + battle.armor_adjust(0), 14);
        assert_eq!(battle.gear_ac_adjust(0), 0);
        assert!(!battle.armor_stealth_disadvantage(0));
        assert!(battle
            .armor_label(0, Act::Other3)
            .unwrap()
            .contains("100 turns"));
    }

    #[test]
    fn heavy_armor_work_can_start_but_cannot_finish_within_arena_limit() {
        let mut battle = starts(Class::Fighter);
        while !battle.finished() {
            round(&mut battle, Act::Other4);
        }
        assert_eq!(battle.armor_projection(0), [3, 1, 2, 20]);
        assert_eq!(battle.armor_adjust(0), 0);
    }

    #[test]
    fn leather_can_be_removed_and_equipped_again() {
        let mut battle = starts(Class::Bard);
        for _ in 0..10 {
            round(&mut battle, Act::Other4);
        }
        assert_eq!(battle.armor_adjust(0), -1);
        for _ in 0..10 {
            round(&mut battle, Act::Other3);
        }
        assert_eq!(battle.armor_projection(0), [1, 1, 0, 0]);
        assert_eq!(battle.armor_adjust(0), 0);
    }

    #[test]
    fn taking_damage_or_skipping_progress_cancels_work_without_removing_armor() {
        let mut battle = starts(Class::Cleric);
        round(&mut battle, Act::Other4);
        assert_eq!(battle.armor_projection(0), [2, 1, 2, 9]);
        battle.damage(0, 1);
        assert_eq!(battle.armor_projection(0), [2, 1, 0, 0]);
        round(&mut battle, Act::Other4);
        battle.step(Act::End).unwrap();
        assert_eq!(battle.armor_projection(0), [2, 1, 0, 0]);
        assert_eq!(battle.armor_adjust(0), 0);
    }

    #[test]
    fn wild_shape_drop_requires_retrieving_body_armor_at_the_drop_location() {
        let mut battle = starts(Class::Druid);
        let drop_location = battle.f[0].pos;
        battle.armor_drop_for_wild_shape(0);
        assert!(!battle.armor_worn(0));
        assert_eq!(battle.mask() & Act::Other3.bit(), 0);
        assert!(battle
            .armor_label(0, Act::Other5)
            .unwrap()
            .contains("Pick up"));
        battle.f[0].pos.x = 1;
        assert_eq!(battle.mask() & Act::Other5.bit(), 0);
        battle.f[0].pos = drop_location;
        battle.step(Act::Other5).unwrap();
        assert!(battle.action);
        assert!(!battle.f[0].gear.interaction);
        assert_ne!(battle.mask() & Act::Other3.bit(), 0);
        assert!(battle.armor_status(0).contains("packed"));
    }

    #[test]
    fn borrowing_druid_leather_applies_untrained_body_armor_rules_after_donning() {
        let mut battle = Battle::new(1, [Class::Wizard, Class::Druid], [3; 2]);
        battle.owner = 0;
        battle.f[1].pos = crate::arena2::geometry::Pos { x: 0, y: 0 };
        battle.armor_drop_for_wild_shape(1);
        let dropped = battle.armor_ground(1).unwrap();
        battle.f[0].pos = dropped.cell;
        battle.f[1].pos.x = 1;
        battle.step(Act::Other5).unwrap();
        assert!(battle.armor_holds_foreign(0));
        assert!(battle.armor_ground(1).is_none());
        assert!(battle.can_cast_components(
            0,
            true,
            false,
            crate::arena2::equipment::Material::None
        ));
        for _ in 0..10 {
            round(&mut battle, Act::Other3);
        }
        assert!(battle.armor_untrained(0));
        assert_eq!(battle.armor_adjust(0), 1);
        assert!(battle.armor_disadvantages_ability(0, 0));
        assert!(battle.armor_disadvantages_ability(0, 1));
        assert!(!battle.armor_disadvantages_ability(0, 2));
        assert!(!battle.can_cast_components(
            0,
            true,
            false,
            crate::arena2::equipment::Material::None
        ));
        assert!(battle.armor_status(0).contains("borrowed"));
    }

    #[test]
    fn loose_shield_drops_and_borrowed_don_only_grants_ac_when_trained() {
        for (borrower, expected_bonus) in [(Class::Wizard, 0), (Class::Ranger, 2)] {
            let mut battle = Battle::new(1, [borrower, Class::Fighter], [3; 2]);
            battle.owner = 1;
            battle.f[1].pos = crate::arena2::geometry::Pos { x: 1, y: 0 };
            battle.step(Act::Gear12).unwrap();
            assert_eq!(
                battle.choice_label(1, ACTS[12]),
                "Doff issued shield into hand (Utilize)"
            );
            battle.step(ACTS[12]).unwrap();
            assert_eq!(battle.gear_loose_shields(1), 2);
            assert!(!battle.gear_shield_worn(1));
            battle.drop_held(1);
            let dropped = battle.shield_ground(1).unwrap();
            battle.owner = 0;
            battle.action = true;
            battle.f[0].pos = dropped.cell;
            battle.f[1].pos.x = 2;
            battle.step(Act::Gear12).unwrap();
            battle.step(ACTS[11]).unwrap();
            assert!(battle.gear_holds_foreign_shield(0));
            battle.step(Act::Gear8).unwrap();
            assert!(battle.gear_shield_worn(0));
            assert!(!battle.armor_untrained(0));
            assert_eq!(battle.gear_ac_adjust(0), expected_bonus);
            assert_eq!(battle.gear_ac_adjust(1), -2);
        }
    }

    #[test]
    fn readied_shield_utilize_changes_shield_without_spending_owners_action() {
        let mut b = Battle::new(1, [Class::Wizard, Class::Fighter], [3; 2]);
        b.owner = 0;
        assert_ne!(b.gear_ready_mask(1) & Act::Gear7.bit(), 0);
        b.gear_ready_release(1, Act::Gear7);
        assert!(!b.gear_shield_worn(1));
        assert!(b.action);
        b.action = false;
        assert_ne!(b.gear_ready_mask(1) & Act::Gear8.bit(), 0);
        b.gear_ready_release(1, Act::Gear8);
        assert!(b.gear_shield_worn(1));
        assert!(!b.action);
    }

    #[test]
    fn mage_armor_rejects_a_worn_suit_and_ends_on_donning() {
        let mut battle = starts(Class::Wizard);
        // Inject a carried suit to cover the interaction with a build
        // that knows Mage Armor but normally has no body armor in its kit.
        battle.f[0].armor = ArmorState::new(build(Class::Bard, 3));
        assert_eq!(battle.magic_mask(0) & Act::Magic2.bit(), 0);
        battle.f[0].armor.location = Location::Packed;
        assert_ne!(battle.magic_mask(0) & Act::Magic2.bit(), 0);
        battle.step(Act::Magic2).unwrap();
        for _ in 0..8 {
            if battle.phase_id() == 0 {
                break;
            }
            let action = crate::arena2::ACTS[battle.mask().trailing_zeros() as usize];
            battle.step(action).unwrap();
        }
        assert!(battle.magic_ac(0).is_some());
        battle.step(Act::End).unwrap();
        battle.step(Act::End).unwrap();
        for _ in 0..10 {
            round(&mut battle, Act::Other3);
        }
        assert!(battle.armor_worn(0));
        assert!(battle.magic_ac(0).is_none());
        battle.f[0].armor.location = Location::Packed;
        assert!(
            battle.magic_ac(0).is_none(),
            "Mage Armor cannot resume later"
        );
    }
}
