//! Public-state baseline and tactical expert policies.
use super::battle::{Battle, Concentration, Fighter};
use super::class::{
    Build,
    Class::{self, *},
};
use super::geometry::{Map, Point, Pos, HEIGHT, WIDTH};
use super::targeting::{TargetKind, TargetSpec};
use super::{Act, ACTS};

/// Selects one legal action. A finished battle returns End as a sentinel.
pub fn choose(battle: &Battle, expert: bool) -> Act {
    let raw = choose_raw(battle, expert);
    ACTS[battle.target_permutation().encode(raw as usize)]
}

fn choose_raw(battle: &Battle, expert: bool) -> Act {
    let Some(actor) = battle.actor() else {
        return Act::End;
    };
    let mask = battle.raw_action_mask();
    if battle.phase_id() == super::martial::subclasses::ELEMENTAL_SAVE {
        return Act::Defend;
    }
    let build = battle.f[actor].build;
    if build.variant != 0 {
        let choice = match build.class {
            Barbarian | Monk => battle.martial_subclass_policy(actor, mask, expert),
            Bard | Ranger => battle.bard_ranger_subclass_policy(actor, mask, expert),
            Cleric | Druid | Paladin => battle.divine_subclass_policy(actor, mask, expert),
            Fighter | Rogue => battle.fighter_rogue_subclass_policy(actor, mask, expert),
            Sorcerer | Warlock | Wizard => battle.arcane_subclass_policy(actor, mask, expert),
        };
        if let Some(choice) = choice.filter(|choice| mask & choice.bit() != 0) {
            return choice;
        }
    }
    // Transactional responses can suspend an entity or target-selection phase.
    // Score the active response before inspecting any of those underlying menus.
    if battle.save_choice_actor().is_some() {
        if battle.phase_id() == super::temp_hp::TEMP_CHOICE_PHASE {
            let pending = battle.pending_features();
            return if pending[1] > pending[0] {
                Act::Defend
            } else {
                Act::Pass
            };
        }
        return ACTS[mask.trailing_zeros() as usize];
    }
    if let Some(action) = battle.magic_ally_policy(actor, mask) {
        return action;
    }
    if let Some(action) = battle.phantasmal_force_policy(actor, mask) {
        return action;
    }
    if let Some(action) = projectile_policy(battle, actor, mask, expert) {
        return action;
    }
    if let Some(action) = battle.entity_policy() {
        return action;
    }
    if let Some((action, cell)) = battle.target_altitude_request() {
        let spec = battle
            .target_spec(action)
            .expect("targeting a known ability");
        let altitude = (0..ACTS.len())
            .filter(|&axis| mask & ACTS[axis].bit() != 0)
            .max_by(|&left, &right| {
                let score = |altitude| target_score(battle, action, spec, Point { cell, altitude });
                score(left as i8)
                    .total_cmp(&score(right as i8))
                    .then(right.cmp(&left))
            })
            .expect("a prepared point has a valid altitude");
        return ACTS[altitude];
    }
    if let Some((action, row)) = battle.target_request() {
        let (center, _) = best_target(battle, action, row)
            .expect("an active targeting phase has at least one legal center");
        return ACTS[row.map_or(center.cell.y, |_| center.cell.x) as usize];
    }
    let me = &battle.f[actor];
    let enemy = Opponent::new(battle, actor);
    match battle.phase_id() {
        1 if mask & Act::Opportunity.bit() != 0 => {
            let shield_caster = matches!(me.build.class, Wizard | Sorcerer)
                && me.slots.iter().any(|&slots| slots > 0);
            if expert
                && shield_caster
                && me.hp * 2 <= me.build.hp
                && enemy.hp + enemy.temp > me.build.melee_mod + me.build.melee_die
            {
                return Act::Pass;
            }
            return Act::Opportunity;
        }
        2 if mask & Act::Defend.bit() != 0 => return Act::Defend,
        2 | 182 if mask & Act::Rider.bit() != 0 => return Act::Rider,
        3 if mask & Act::Rider.bit() != 0 => return Act::Rider,
        87 if mask & Act::Rider.bit() != 0 => return Act::Rider,
        196 => return Act::Pass,
        1..=3 => return Act::Pass,
        super::battle::sanctuary::RETARGET => {
            let foe = 1 - actor;
            return [ACTS[foe], ACTS[foe + 2], Act::Pass]
                .into_iter()
                .find(|action| mask & action.bit() != 0)
                .expect("Sanctuary always permits losing the attempt");
        }
        7.. => return ACTS[mask.trailing_zeros() as usize],
        _ => {}
    }
    if battle.turns <= 2 && battle.action && battle.entity(actor + 2).is_none() {
        let before_melee = enemy.pos.dist(me.pos) > battle.movement_left() + 1;
        let without_shot = !enemy.position_known || !battle.can_see(actor, 1 - actor);
        let summoner = (me.build.class == Paladin && me.build.level == 5 && before_melee)
            || (expert
                && me.build.class == Druid
                && me.build.level >= 2
                && me.resource > 0
                && without_shot);
        if summoner && mask & Act::Entity1.bit() != 0 {
            return Act::Entity1;
        }
        if summoner
            && me.gear.melee == super::equipment::Carried::Held
            && mask & Act::Gear2.bit() != 0
        {
            return Act::Gear2;
        }
    }
    if !enemy.position_known {
        if mask & Act::Search.bit() != 0 {
            return Act::Search;
        }
        let likely_region = Map::spawn(1 - actor);
        let distance = battle.map.path(me.pos, likely_region);
        return ACTS[..8]
            .iter()
            .enumerate()
            .filter_map(|(direction, &action)| {
                if mask & action.bit() == 0 {
                    return None;
                }
                let (destination, cost) = battle.map.step(me.pos, direction)?;
                let gain = distance - battle.map.path(destination, likely_region);
                (gain > 0).then_some((gain, -cost, action as usize))
            })
            .max()
            .map_or(Act::End, |(_, _, index)| ACTS[index]);
    }
    let position = Position::new(battle, me, &enemy);
    // Reflect east/west tie-breaking for the opposite seat.
    let direction_order = [[0, 1, 2, 3, 4, 5, 6, 7], [0, 7, 6, 5, 4, 3, 2, 1]];
    let mut best = Act::End;
    let mut best_score = 0.0;
    for index in direction_order[actor].into_iter().chain(8..ACTS.len()) {
        let action = ACTS[index];
        if mask & action.bit() == 0 {
            continue;
        }
        let score = if index < 8 {
            let penalty = if expert && position.entry_has_caltrops(index) {
                2.0
            } else {
                0.0
            };
            position.move_score(index, expert) - penalty
        } else {
            action_score(&position, action, expert)
        };
        if score > best_score
            && (battle.target_spec(action).is_none()
                || best_target(battle, action, None).is_some_and(|(_, score)| score > 0.0))
        {
            best = action;
            best_score = score;
        }
    }
    best
}

fn projectile_policy(battle: &Battle, actor: usize, mask: u128, expert: bool) -> Option<Act> {
    let foe = 1 - actor;
    let first = || ACTS[mask.trailing_zeros() as usize];
    let allowed = |index: usize| mask & ACTS[index].bit() != 0;
    let chosen = match battle.phase_id() {
        80 => [16 + foe, foe, 18 + foe, 2 + foe, Act::Pass as usize, 126]
            .into_iter()
            .find(|&index| allowed(index))
            .map_or_else(first, |index| ACTS[index]),
        81 => {
            let darts = (battle.projectile_features(actor)[1] * 5.0).round() as i32;
            if expert && darts == 1 && battle.f[actor].hp > 10 {
                Act::Pass
            } else {
                (1..=3)
                    .find(|&rank| allowed(rank))
                    .map_or(Act::Pass, |rank| ACTS[rank])
            }
        }
        82 => {
            let guess = Map::spawn(foe).y as usize;
            (0..HEIGHT as usize)
                .filter(|&row| allowed(row))
                .min_by_key(|&row| row.abs_diff(guess))
                .map_or_else(first, |row| ACTS[row])
        }
        83 => {
            let guess = Map::spawn(foe).x as usize;
            (0..WIDTH as usize)
                .filter(|&column| allowed(column))
                .min_by_key(|&column| column.abs_diff(guess))
                .map_or_else(first, |column| ACTS[column])
        }
        84 => first(),
        85 => {
            let public = battle.projectile_features(actor);
            let damage = (public[2] * 5.0).round() as i32;
            let target = ((public[3] * 4.0).round() as usize).checked_sub(1);
            if allowed(Act::Defend as usize)
                && target.is_some_and(|relative_id| relative_id % 2 == 0)
                && (!expert || damage >= 3 || battle.f[actor].hp <= damage)
            {
                Act::Defend
            } else {
                Act::Pass
            }
        }
        _ => return None,
    };
    Some(chosen)
}

fn best_target(battle: &Battle, action: Act, row: Option<i8>) -> Option<(Point, f32)> {
    let spec = battle.target_spec(action)?;
    let rows = row.map_or(0..HEIGHT, |y| y..y + 1);
    let mut best = None;
    for y in rows {
        let mask = battle.target_mask(action, Some(y));
        for column in 0..WIDTH {
            let x = if battle.owner == 0 {
                column
            } else {
                WIDTH - 1 - column
            };
            if mask & (1 << x) == 0 {
                continue;
            }
            let cell = Pos { x, y };
            let mut altitudes = battle.target_altitude_mask(action, cell);
            while altitudes != 0 {
                let altitude = altitudes.trailing_zeros() as i8;
                altitudes &= altitudes - 1;
                let center = Point { cell, altitude };
                let score = target_score(battle, action, spec, center);
                if best.is_none_or(|(_, best_score)| score > best_score) {
                    best = Some((center, score));
                }
            }
        }
    }
    best
}

fn target_score(battle: &Battle, action: Act, spec: TargetSpec, origin: Point) -> f32 {
    let center = origin.cell;
    let owner = battle.owner;
    let foe = 1 - owner;
    let me = &battle.f[owner];
    let enemy = Opponent::new(battle, owner);
    if spec.kind == TargetKind::Caltrops {
        // Favor a reachable tile in the opponent's approach lane. An occupied
        // enemy square cannot trigger the hazard until the opponent re-enters.
        let to_center = battle.map.path(enemy.pos, center);
        let to_me = battle.map.path(center, me.pos);
        let detour = to_center + to_me - battle.map.path(enemy.pos, me.pos);
        if to_center == 0 || to_center > enemy.build.speed {
            return -1.0;
        }
        return 10.0 - 2.0 * detour as f32 - 0.25 * to_center as f32;
    }
    let Some(area) = battle.target_area(action, 0) else {
        return -1.0;
    };
    let launch = origin;
    let hits_enemy = enemy.position_known
        && battle.area_hits_at(
            launch,
            Point {
                cell: enemy.pos,
                altitude: enemy.elevation,
            },
            area,
        );
    let hits_self = battle.area_hits_at(launch, battle.fighter_point(owner), area);
    if matches!(spec.kind, TargetKind::SpiritualWeapon) {
        return if hits_enemy {
            25.0 - center.dist(enemy.pos) as f32
        } else {
            -1.0
        };
    }
    let accuracy_margin = center.dist(enemy.pos) as f32;
    if spec.kind == TargetKind::LandAid {
        let healing = if hits_self {
            (me.build.hp - me.hp).min(11) as f32
        } else {
            0.0
        };
        return if hits_enemy {
            30.0 - accuracy_margin + healing
        } else {
            healing - 4.0
        };
    }
    if !hits_enemy {
        let hits_companion = battle
            .observed_entity(owner, foe + 2)
            .is_some_and(|entity| battle.area_hits_at(launch, entity.point(), area));
        return if hits_self {
            -30.0
        } else if hits_companion {
            8.0
        } else {
            -1.0
        };
    }
    if !hits_self {
        return 30.0 - accuracy_margin;
    }
    // An overlapping self-hit is occasionally viable against a nearly
    // defeated opponent, provided the caster can weather an ordinary roll.
    let nominal_damage = match (me.build.class, action) {
        (Bard, Act::Spell4) => return -20.0, // Hypnotic Pattern
        (Bard, Act::Spell3) => 14,
        (Druid, Act::Spell3) => 11,
        _ => 28,
    };
    if enemy.hp + enemy.temp <= nominal_damage / 2 && me.hp + me.temp > nominal_damage {
        7.0 - accuracy_margin
    } else {
        -10.0 - accuracy_margin
    }
}

/// Current combat details are available only from direct sight. Build traits
/// and the last audible position are public irrespective of solid cover.
#[derive(Clone, Copy)]
struct Opponent {
    build: Build,
    pos: Pos,
    elevation: i8,
    position_known: bool,
    hp: i32,
    temp: i32,
    ac: i32,
    shield: i32,
    dodge: bool,
    reckless: bool,
    stun_exposed: bool,
    stunned: bool,
    wolf: bool,
}

impl Opponent {
    fn new(battle: &Battle, viewer: usize) -> Self {
        let side = 1 - viewer;
        let fighter = &battle.f[side];
        let position = battle.observed_pos(viewer, side);
        let visible = battle.can_see(viewer, side) || battle.familiar_sight(viewer, side);
        Self {
            build: fighter.build,
            pos: position.unwrap_or_else(|| Map::spawn(side)),
            elevation: if visible {
                battle.fighter_point(side).altitude
            } else {
                0
            },
            position_known: position.is_some(),
            hp: if visible {
                fighter.hp
            } else {
                fighter.build.hp
            },
            temp: if visible { fighter.temp } else { 0 },
            ac: if visible {
                battle.ac(side)
            } else {
                fighter.build.ac
            },
            shield: if visible { fighter.shield } else { 0 },
            dodge: visible && fighter.dodge,
            reckless: visible && fighter.reckless,
            stun_exposed: visible && fighter.stun_exposed,
            stunned: visible && fighter.stunned,
            wolf: visible && battle.magic_is_wolf(side),
        }
    }
}

struct Position<'a> {
    battle: &'a Battle,
    me: &'a Fighter,
    enemy: &'a Opponent,
    attack_distance: i32,
    enemy_path: i32,
    ranged: bool,
    kiting: bool,
}

impl<'a> Position<'a> {
    fn new(battle: &'a Battle, me: &'a Fighter, enemy: &'a Opponent) -> Self {
        let ranged = ranged_role(me.build.class) && !battle.magic_is_wolf(battle.owner);
        Self {
            battle,
            me,
            enemy,
            attack_distance: attack_distance(battle.map, me.pos, enemy.pos, ranged),
            enemy_path: attack_distance(battle.map, enemy.pos, me.pos, false),
            ranged,
            kiting: ranged && (!ranged_role(enemy.build.class) || enemy.wolf),
        }
    }

    fn attack_available(&self) -> bool {
        self.battle.action || self.battle.attacks > 0
    }

    fn enemy_engaged(&self) -> bool {
        self.enemy_path == 0
    }

    fn opportunity_on_exit(&self, destination: Pos, cost: i32) -> bool {
        cost > self.battle.tactical_steps
            && !self.battle.disengaged
            && !self.enemy.stunned
            && self.enemy_engaged()
            && destination.dist(self.enemy.pos) > 1
    }

    fn entry_has_caltrops(&self, direction: usize) -> bool {
        let Some((destination, _)) = self.battle.map.step(self.me.pos, direction) else {
            return false;
        };
        (8..10).any(|id| self.battle.observed_object(self.battle.owner, id)
            .is_some_and(|object| object.point.cell == destination))
    }

    fn move_score(&self, direction: usize, expert: bool) -> f32 {
        let map = self.battle.map;
        let Some((destination, cost)) = map.step(self.me.pos, direction) else {
            return 0.0;
        };
        let target = self.enemy.pos;
        if self.opportunity_on_exit(destination, cost) {
            return 0.0;
        }

        let attack_distance = attack_distance(map, destination, target, self.ranged);
        let approach = self.attack_distance - attack_distance;
        if approach > 0 {
            let urgency = if self.attack_available() { 4.0 } else { 1.0 };
            return urgency + approach as f32 - 0.45 * (cost - 1) as f32;
        }

        if expert && self.me.concentration == Some(Concentration::SpiritGuardians) {
            let dist = self.me.pos.dist(target);
            if dist > 3 && destination.dist(target) < dist {
                return 2.0;
            }
            if dist <= 3 && destination.dist(target) > 3 {
                return 0.0;
            }
        }
        if !expert || !self.ranged || self.attack_distance != 0 || attack_distance != 0 {
            return 0.0;
        }
        let enemy_path = attack_distance_to_melee(map, target, destination);
        // Adjacent hostile creatures give ranged attack rolls disadvantage.
        if self.enemy_engaged() && enemy_path > 0 && self.attack_available() {
            return 9.0;
        }
        if self.kiting && enemy_path > self.enemy_path {
            return 1.6 + 0.25 * (enemy_path - self.enemy_path) as f32;
        }
        0.0
    }
}

fn ranged_role(class: Class) -> bool {
    matches!(
        class,
        Bard | Cleric | Druid | Ranger | Rogue | Sorcerer | Warlock | Wizard
    )
}

fn attack_distance_to_melee(map: Map, from: Pos, target: Pos) -> i32 {
    if from.dist(target) == 1 && map.los(from, target) {
        return 0;
    }
    let mut shortest = 255;
    for y in (target.y - 1)..=(target.y + 1) {
        for x in (target.x - 1)..=(target.x + 1) {
            let candidate = Pos { x, y };
            if candidate != target && map.los(candidate, target) {
                shortest = shortest.min(map.path(from, candidate));
            }
        }
    }
    shortest
}

fn attack_distance(map: Map, from: Pos, target: Pos, ranged: bool) -> i32 {
    if ranged {
        map.path_to_los(from, target)
    } else {
        attack_distance_to_melee(map, from, target)
    }
}

fn action_score(position: &Position<'_>, action: Act, expert: bool) -> f32 {
    use Act::*;
    let battle = position.battle;
    let me = position.me;
    let enemy = position.enemy;
    let class = me.build.class;
    let missing = me.build.hp - me.hp;
    let needs_healing = missing as f32 / me.build.hp as f32;
    let engaged = position.enemy_engaged();
    let can_reach = position.attack_distance <= battle.movement_left();
    let heal = |minimum: i32, expected_hp: f32, bonus: bool| {
        if missing < minimum {
            return 0.0;
        }
        let restored = expected_hp.min(missing as f32);
        if bonus || me.hp * 3 <= me.build.hp {
            8.0 + restored + 4.0 * needs_healing
        } else {
            restored * 0.85
        }
    };
    let attack = |damage: f32, aim: Aim| expected_damage(position, damage, aim, expert);
    let targeted = |damage: f32, aim: Aim, expenditure: f32| {
        let expected = attack(damage, aim);
        let lethal = if damage >= (enemy.hp + enemy.temp) as f32 {
            1.25
        } else {
            0.0
        };
        expected.max(0.1) + lethal - if expert { expenditure } else { 0.0 }
    };
    let weapon = |ranged: bool, advantage: bool| {
        let (die, modifier) = if ranged {
            (me.build.ranged_die, me.build.ranged_mod)
        } else {
            (me.build.melee_die, me.build.melee_mod)
        };
        let (die, modifier) = battle
            .magic_weapon(battle.owner, ranged)
            .unwrap_or((die, modifier));
        let base = (die as f32 + 1.0) * 0.5 + modifier as f32;
        let rider = match class {
            Barbarian if !ranged && me.raging => 2.0,
            Rogue if advantage || me.attack_advantage || battle.martial_has_vex(battle.owner) => {
                3.5 * ((me.build.level + 1) / 2) as f32
            }
            _ => 0.0,
        };
        let attack_bonus = battle.martial_attack_bonus(battle.owner, ranged);
        let advantage = advantage || battle.martial_has_vex(battle.owner);
        let hit = weapon_accuracy(position, modifier + attack_bonus, ranged, advantage, expert);
        let repeat = if battle.action
            && me.build.level >= 5
            && matches!(class, Barbarian | Fighter | Monk | Paladin | Ranger)
        {
            1.7
        } else {
            1.0
        };
        hit * (base + rider) * repeat
    };
    let cantrip = if me.build.level >= 5 { 2.0 } else { 1.0 };
    let tier = || battle.slot_for_action(battle.owner, action).unwrap_or(1) as f32;
    let restore_with_spell = |die_mean: f32, bonus: bool| {
        let life = if class == Cleric && me.build.level >= 3 {
            tier() + 2.0
        } else {
            0.0
        };
        let heal_amount = 2.0 * tier() * die_mean + me.build.casting as f32 + life;
        let threshold = if bonus { 5.0 } else { 8.0 };
        heal(
            (heal_amount * 0.7).max(threshold) as i32,
            heal_amount,
            bonus,
        )
    };
    match action {
        Study if battle.phantasmal_force_dc(battle.owner).is_some() => 5.0,
        Magic22
            if me.concentration.is_none()
                && battle.prepared_spell(battle.owner, action)
                    == Some(super::magic::Spell::PhantasmalForce)
                && battle.phantasmal_enemy_target(battle.owner) =>
        {
            targeted(14.0, Aim::Save(3, false), 2.0)
        }
        Gear1 if class == Paladin && battle.action && can_reach => 9.0,
        Gear2
            if matches!(class, Ranger | Rogue | Bard)
                && position.ranged
                && !engaged
                && me.arrows > 0 =>
        {
            8.0
        }
        Gear2
            if class == Paladin
                && me.build.level >= 5
                && !engaged
                && battle.action
                && battle.entity(2 + battle.owner).is_none() =>
        {
            8.0
        }
        Gear2
            if class == Druid
                && me.build.level >= 2
                && me.resource > 0
                && !can_reach
                && battle.action
                && battle.entity(2 + battle.owner).is_none() =>
        {
            7.0
        }
        End => 0.0,
        Entity1 if class == Paladin && battle.entity(2 + battle.owner).is_none() => 10.0,
        Entity1
            if class == Druid
                && battle.entity(2 + battle.owner).is_none()
                && position.attack_distance > battle.movement_left() =>
        {
            6.0
        }
        Entity3 if class == Paladin => 4.0,
        Entity5 if class == Paladin && !can_reach => 7.0,
        Entity8 if class == Paladin && me.hp * 2 < me.build.hp => 9.0,
        Melee => weapon(false, false),
        Ranged => {
            let damage = weapon(true, false);
            if !position.ranged && can_reach && position.attack_distance > 0 {
                damage.min(3.0)
            } else {
                damage
            }
        }
        Dash if !can_reach && position.attack_available() => {
            let dash_reaches =
                position.attack_distance <= battle.movement_left() + battle.speed(battle.owner);
            if position.attack_distance < 255 && (!position.ranged || dash_reaches) {
                if expert && dash_reaches && !position.ranged {
                    6.5
                } else {
                    4.0
                }
            } else {
                0.0
            }
        }
        Dodge if position.enemy_path <= enemy.build.speed => 1.0,
        Special1 => match class {
            Barbarian if can_reach => 14.0,
            Bard => targeted(4.5 * cantrip, Aim::Attack, 0.0),
            Cleric => targeted(9.0, Aim::Save(2, true), 0.7),
            Druid
                if engaged
                    && needs_healing >= 0.45
                    && me.temp == 0
                    && me.slots.iter().sum::<i32>() <= 2 =>
            {
                10.0
            }
            Fighter => heal(7, 5.5 + me.build.level as f32, true),
            Monk => {
                if engaged {
                    8.0
                } else {
                    0.0
                }
            }
            Paladin => heal(7, me.resource.max(0) as f32, true),
            Rogue if position.attack_distance == 0 && battle.moved == 0 => 11.0,
            Sorcerer if position.attack_available() && can_reach => 14.0,
            Warlock if me.temp < 4 && needs_healing >= 0.50 => 4.0,
            _ => 0.0,
        },
        Special2 => match class {
            Barbarian => weapon(false, true) - if expert && !me.raging { 1.3 } else { 0.3 },
            Bard if expert && me.resource == 0 && me.slots[0] >= 2 => 6.0,
            Cleric => heal(8, 14.0, false),
            Fighter if !battle.action && battle.attacks == 0 && can_reach => 14.0,
            Monk if engaged => 8.5,
            Paladin => weapon(false, true) + if enemy.hp >= 15 { 2.0 } else { 0.0 },
            Rogue if !can_reach && position.attack_available() => 12.0,
            Sorcerer => targeted(28.0, Aim::DisadvantagedSave(1), 2.3),
            _ => 0.0,
        },
        Special3 => match class {
            Cleric => targeted(8.0, Aim::MeleeSpell, 0.0),
            Barbarian if !can_reach => 4.5,
            Druid => targeted(10.5, Aim::Save(2, false), 1.0) + (missing as f32).clamp(0.0, 10.5),
            Monk if expert && engaged && me.hp <= 12 => 8.8,
            Rogue if expert && engaged && battle.movement_left() > 0 => 12.0,
            Sorcerer => targeted(5.5 * cantrip, Aim::Attack, 0.25) + 3.0,
            _ => 0.0,
        },
        Special4 => match class {
            Cleric => restore_with_spell(4.5, false),
            Druid if engaged && battle.action => 5.0,
            Monk if !can_reach && position.attack_available() => 12.0,
            Rogue
                if expert
                    && position.kiting
                    && !engaged
                    && battle.movement_left() > 0
                    && position.enemy_path <= enemy.build.speed =>
            {
                7.5
            }
            Sorcerer if me.slots.iter().sum::<i32>() == 0 => 6.0,
            _ => 0.0,
        },
        Special5 => match class {
            Druid => targeted(11.0, Aim::Save(2, true), 0.0),
            Monk if expert && engaged && battle.movement_left() > 1 && me.hp <= 10 => 8.8,
            _ => 0.0,
        },
        Special6 => match class {
            Druid => restore_with_spell(4.5, false),
            Monk if expert && !can_reach && position.attack_available() => 11.0,
            _ => 0.0,
        },
        Special7 if class == Druid && expert && needs_healing > 0.40 => 4.0,
        Spell0 => match class {
            Bard => targeted(3.5 * cantrip, Aim::Save(4, false), 0.0) + 1.65,
            Cleric => targeted(4.5 * cantrip, Aim::Save(1, false), 0.0),
            Druid => targeted(4.5 * cantrip, Aim::Attack, 0.0),
            Paladin if !can_reach && enemy.hp > 15 && me.concentration.is_none() => 3.0,
            Ranger if can_reach && enemy.hp > 9 => 13.0,
            Sorcerer => targeted(5.5 * cantrip, Aim::Attack, 0.0),
            Warlock => targeted(
                (5.5 + if me.build.level >= 2 {
                    me.build.casting as f32
                } else {
                    0.0
                }) * cantrip,
                Aim::Attack,
                0.0,
            ),
            Wizard => {
                targeted(4.5 * cantrip, Aim::Cantrip, 0.0)
                    + if enemy.pos.dist(me.pos) > 2 { 1.2 } else { 0.0 }
            }
            _ => 0.0,
        },
        Spell1 => match class {
            Bard => targeted(3.5 * (tier() + 2.0), Aim::Save(4, true), 1.0),
            Cleric => targeted(3.5 * (tier() + 3.0), Aim::Attack, 1.0) + 0.9,
            Druid => {
                targeted(4.5 * (tier() + 1.0), Aim::Save(2, true), 0.5)
                    + if engaged { 1.0 } else { 0.0 }
            }
            Paladin if can_reach && enemy.hp > 10 => 13.0,
            Ranger => restore_with_spell(4.5, false),
            Sorcerer => targeted(4.5 * (tier() + 2.0), Aim::Attack, 1.0),
            Warlock if can_reach && enemy.hp > 12 => 13.0,
            Wizard => targeted(3.5 * (tier() + 2.0), Aim::Missile, 1.4),
            _ => 0.0,
        },
        Spell2 => match class {
            Bard | Cleric | Druid => restore_with_spell(2.5, true),
            Paladin => restore_with_spell(4.5, false),
            Sorcerer => targeted(3.5 * (tier() + 2.0), Aim::Missile, 1.4),
            Warlock | Wizard => targeted(3.5 * (tier() + 2.0), Aim::Save(1, true), 0.7),
            _ => 0.0,
        },
        Spell3 => match class {
            Bard => targeted(4.5 * (tier() + 1.0), Aim::Save(2, true), 1.1),
            Cleric if enemy.hp > 8 => {
                targeted(
                    4.5 * (tier() - 1.0) + me.build.casting as f32,
                    Aim::MeleeSpell,
                    0.6,
                ) + 5.0
            }
            Druid => {
                targeted(5.5 * tier(), Aim::Save(2, true), 0.8) + if engaged { 0.0 } else { 1.5 }
            }
            Sorcerer | Warlock | Wizard => targeted(7.0 * (tier() + 1.0), Aim::Attack, 1.7),
            _ => 0.0,
        },
        Spell4 => match class {
            Bard if expert && enemy.hp > 16 => 2.0,
            Cleric if battle.dist() <= 3 => targeted(13.5, Aim::Save(4, true), 0.5) + 4.0,
            Druid | Sorcerer | Warlock | Wizard => targeted(28.0, Aim::Save(1, true), 2.0),
            _ => 0.0,
        },
        Spell5 => match class {
            Bard => restore_with_spell(4.5, false),
            Cleric if position.attack_distance == 0 && me.concentration.is_none() => 2.0,
            Druid => targeted(3.5 * (tier() + 2.0), Aim::Save(1, true), 0.7),
            Sorcerer => {
                targeted(4.5 * cantrip, Aim::Attack, 0.0)
                    + if enemy.pos.dist(me.pos) > 2 { 1.2 } else { 0.0 }
            }
            Wizard => targeted(5.5 * cantrip, Aim::Cantrip, 0.0),
            _ => 0.0,
        },
        _ => 0.0,
    }
}

#[derive(Clone, Copy)]
enum Aim {
    Attack,
    Cantrip,
    MeleeSpell,
    Save(usize, bool),
    DisadvantagedSave(usize),
    Missile,
}

fn weapon_accuracy(
    position: &Position<'_>,
    modifier: i32,
    ranged: bool,
    advantage: bool,
    expert: bool,
) -> f32 {
    let enemy = position.enemy;
    let me = position.me;
    let ac = enemy.ac;
    let regular = ((21 + modifier + me.build.pb - ac) as f32 * 0.05).clamp(0.05, 0.95);
    let advantage = advantage || me.attack_advantage || enemy.reckless || enemy.stun_exposed;
    let disadvantage =
        (ranged && position.enemy_engaged()) || enemy.dodge || me.sap || me.attack_disadvantage;
    if expert || ranged {
        match (advantage, disadvantage) {
            (true, false) => 1.0 - (1.0 - regular).powi(2),
            (false, true) => regular.powi(2),
            _ => regular,
        }
    } else {
        regular
    }
}

fn expected_damage(position: &Position<'_>, damage: f32, aim: Aim, expert: bool) -> f32 {
    match aim {
        Aim::Missile => {
            let target = position.enemy;
            if target.shield > 0 {
                0.0
            } else if expert && matches!(target.build.class, Wizard | Sorcerer) {
                damage * 0.65
            } else {
                damage
            }
        }
        Aim::Attack | Aim::Cantrip | Aim::MeleeSpell => {
            let hit = weapon_accuracy(
                position,
                position.me.build.casting,
                !matches!(aim, Aim::MeleeSpell),
                false,
                expert,
            );
            let potent = matches!(aim, Aim::Cantrip)
                && position.me.build.class == Wizard
                && position.me.build.level >= 3;
            damage * if potent { 0.5 + 0.5 * hit } else { hit }
        }
        Aim::Save(stat, half) => {
            let success = save_chance(position, stat, expert);
            damage
                * if half {
                    1.0 - 0.5 * success
                } else {
                    1.0 - success
                }
        }
        Aim::DisadvantagedSave(stat) => {
            damage * (1.0 - 0.5 * save_chance(position, stat, expert).powi(2))
        }
    }
}

fn save_chance(position: &Position<'_>, stat: usize, expert: bool) -> f32 {
    if !expert {
        return 0.5;
    }
    let caster = position.me.build;
    let target = position.enemy.build;
    let proficiency = if target.saves[stat] { target.pb } else { 0 };
    let modifier = target.stats[stat] + proficiency;
    let dc = 8 + caster.pb + caster.casting;
    ((21 + modifier - dc) as f32 * 0.05).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::class::CLASSES;

    #[test]
    fn both_policies_ignore_private_state_and_unobserved_fighter_details() {
        use crate::arena2::class::CLASSES;

        for class in CLASSES {
            for side in 0..2 {
                for level in [1, 5] {
                    let mut classes = [Wizard; 2];
                    classes[side] = class;
                    let mut base = Battle::new(0, classes, [level; 2]);
                    base.owner = side;
                    base.map = Map::new(0);
                    let foe = 1 - side;
                    for covered in [false, true] {
                        let mut first = base.clone();
                        if !covered {
                            first.f[side].pos = Pos { x: 0, y: 0 };
                            first.f[foe].pos = Pos { x: 3, y: 0 };
                        }
                        assert_eq!(first.can_see(side, foe), !covered, "{class:?}");
                        let mut second = first.clone();
                        second.f[foe].slots = [0; 3];
                        second.f[foe].resource = 0;
                        second.f[foe].secondary = 0;
                        second.f[foe].reaction = false;
                        if covered {
                            second.f[foe].hp = 1;
                            second.f[foe].temp = 12;
                            second.f[foe].shield = 5;
                            second.f[foe].stun_exposed = true;
                            second.f[foe].reckless = true;
                            second.f[foe].dodge = true;
                            second.f[foe].slow = true;
                        }
                        for expert in [false, true] {
                            assert_eq!(
                                choose(&first, expert),
                                choose(&second, expert),
                                "{class:?}, level {level}, seat {side}, covered {covered}"
                            );
                        }
                    }

                    let mut first = base.clone();
                    let mut second = base.clone();
                    first.f[foe].hidden_dc = Some(20);
                    second.f[foe].hidden_dc = Some(20);
                    second.f[foe].pos.y = 0;
                    second.f[foe].hp = 1;
                    second.f[foe].slots = [0; 3];
                    assert_eq!(first.observed_pos(side, foe), None);
                    assert_eq!(second.observed_pos(side, foe), None);
                    for expert in [false, true] {
                        assert_eq!(
                            choose(&first, expert),
                            choose(&second, expert),
                            "{class:?}, level {level}, seat {side}, hidden"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn policy_groups_magic_missile_at_the_enemy_adventurer() {
        let mut duel = Battle::new(0, [Wizard, Fighter], [1, 1]);
        duel.owner = 0;
        duel.f[0].pos = Pos { x: 0, y: 0 };
        duel.f[1].pos = Pos { x: 8, y: 0 };
        duel.step(Act::Spell1).unwrap();
        assert_eq!(duel.phase_id(), 80);
        for expert in [false, true] {
            assert_eq!(choose(&duel, expert), ACTS[17]);
        }
    }

    #[test]
    fn melee_and_ranged_routes_lead_to_legal_targeting() {
        for seed in 0..16 {
            let map = Map::new(seed);
            let start = Map::spawn(0);
            let target = Map::spawn(1);
            for (range, ranged) in [(1, false), (12, true)] {
                let mut current = start;
                let mut remaining = attack_distance(map, current, target, ranged);
                while remaining > 0 {
                    let next = (0..8)
                        .filter_map(|direction| map.step(current, direction))
                        .find(|(position, cost)| {
                            *position != target
                                && remaining
                                    == *cost + attack_distance(map, *position, target, ranged)
                        });
                    let Some((position, cost)) = next else {
                        panic!("no shortest route on map seed {seed} from {current:?}");
                    };
                    current = position;
                    remaining -= cost;
                }
                assert!(current.dist(target) <= range && map.los(current, target));
            }
        }
    }

    #[test]
    fn targeted_fireball_chooses_a_safe_center_even_with_a_closer_unsafe_center() {
        let mut battle = Battle::new(0, [Wizard, Fighter], [5, 5]);
        battle.owner = 0;
        battle.f[0].pos = Pos { x: 0, y: 0 };
        battle.f[1].pos = Pos { x: 4, y: 0 };
        assert!(battle.target_hits(battle.f[1].pos, 0, 4));
        battle.step(Act::Spell4).unwrap();
        let row = choose(&battle, true);
        battle.step(row).unwrap();
        let column = choose(&battle, true);
        let center = Pos {
            x: column as i8,
            y: row as i8,
        };
        battle.step(column).unwrap();
        if battle.target_altitude_request().is_some() {
            let altitude = choose(&battle, true);
            let point = Point {
                cell: center,
                altitude: altitude as i8,
            };
            let area = battle.target_area(Act::Spell4, 0).unwrap();
            assert!(battle.area_hits_at(point, battle.fighter_point(1), area));
            assert!(!battle.area_hits_at(point, battle.fighter_point(0), area));
            battle.step(altitude).unwrap();
        }
        assert_eq!(battle.f[0].hp, battle.f[0].build.hp);
        assert!(battle.f[1].hp < battle.f[1].build.hp);
    }

    #[test]
    fn both_policies_use_only_offered_actions() {
        for (index, class) in CLASSES.into_iter().enumerate() {
            let opponent = CLASSES[(index + 5) % CLASSES.len()];
            for level in [1, 3, 5] {
                let mut battle = Battle::new(index as u64, [class, opponent], [level; 2]);
                let mut decision_count = 0;
                while !battle.finished() && decision_count < 1024 {
                    let expert = battle.actor() == Some(index % 2);
                    let action = choose(&battle, expert);
                    assert_ne!(battle.mask() & action.bit(), 0, "{class:?}, level {level}");
                    battle.step(action).unwrap();
                    decision_count += 1;
                }
                assert!(battle.finished(), "{class:?}, level {level}");
            }
        }
    }
}
