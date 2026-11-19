//! Batched ABI. Only table-free, fixed-size numeric copies cross the hot path.
use super::{
    battle::Battle,
    bot,
    class::{Class, CLASSES, EQUIPMENT, ROSTER},
    equipment::Carried,
    geometry::{Pos, HEIGHT, WIDTH},
    target_ids::relative_id,
    ACTS,
};
use std::{ffi::c_char, slice};

#[path = "bridge_entities.rs"]
mod entity_view;
#[path = "bridge_table.rs"]
mod table_view;

const FEATURES: usize = 1024;
const ACTIONS: usize = 128;
const RAW: usize = 224;
const SUBCLASS_PHASE_BASE: usize = 140;
const SUBCLASS_PHASE_WIDTH: usize = 8;
const SUBCLASS_PHASES: usize = SUBCLASS_PHASE_WIDTH * SUBCLASS_PHASE_WIDTH;
struct Batch(Vec<Battle>);

fn result(b: &Battle) -> i32 {
    if b.finished() {
        b.winner().map_or(2, |s| s as i32)
    } else {
        -1
    }
}
fn flag(x: bool) -> f32 {
    if x {
        1.0
    } else {
        0.0
    }
}

fn reason(b: &Battle) -> i32 {
    if !b.finished() {
        0
    } else if b.winner().is_some() {
        1
    } else if b.f.iter().all(|f| f.hp <= 0) {
        2
    } else if b.turns >= 60 {
        3
    } else {
        4
    }
}

fn perceived(b: &Battle, viewer: usize, subject: usize) -> bool {
    viewer == subject || b.can_see(viewer, subject) || b.familiar_sight(viewer, subject)
}
fn combatant_features(b: &Battle, me: usize, i: usize, out: &mut [f32]) {
    let f = &b.f[i];
    let u = f.build;
    let own = i == me;
    let n = usize::from(!own);
    let j = 32 + n * 26;
    out[j] = u.level as f32 / 5.;
    out[j + 1] = u.hp as f32 / 60.;
    out[j + 25] = u.speed as f32 / 8.;
    if let Some(p) = b.observed_pos(me, i) {
        out[j + 4] = p.x as f32 / 8.;
        out[j + 5] = p.y as f32 / 6.;
        out[330 + n * 4] = 1.;
        out[331 + n * 4] = f.elevation as f32 / 6.;
    }
    if !perceived(b, me, i) {
        return;
    }
    out[24 + n] = f.hp as f32 / b.magic_max_hp(i) as f32;
    out[j + 2] = f.temp as f32 / 20.;
    out[j + 3] = armor(b, i) as f32 / 20.;
    if own {
        out[j + 6..j + 12].copy_from_slice(&[
            f.resource as f32 / 10.,
            f.secondary as f32 / 6.,
            f.slots[0] as f32 / 4.,
            f.slots[1] as f32 / 3.,
            f.slots[2] as f32 / 2.,
            flag(f.reaction),
        ]);
        out[84..100].copy_from_slice(&b.magic_features(i));
        out[100..108].copy_from_slice(&b.martial_features(i));
        out[155] = f.arrows as f32 / 20.;
    }
    out[j + 12..j + 25].copy_from_slice(&[
        flag(f.dodge),
        f.shield as f32 / 5.,
        flag(f.sap),
        flag(f.slow),
        flag(f.stunned),
        flag(f.stun_slow),
        flag(f.stun_exposed),
        flag(f.attack_advantage),
        flag(f.attack_disadvantage),
        flag(f.speed_zero),
        flag(f.raging),
        flag(f.reckless),
        flag(b.public_concentration(me, i)),
    ]);
    out[332 + n * 4] = flag(f.prone);
    out[333 + n * 4] = flag(f.grappled.is_some());
    let g = &f.gear;
    let k = 338 + n * 16;
    for (slot, location) in [g.melee, g.bow, g.shield].into_iter().enumerate() {
        out[k + slot] = flag(location == Carried::Held);
        if own {
            out[k + 3 + slot] = flag(location == Carried::Packed);
            if let Carried::Ground(p) = location {
                out[k + 6 + 2 * slot] = (p.x + 1) as f32 / 9.;
                out[k + 7 + 2 * slot] = (p.y + 1) as f32 / 7.;
            }
        }
    }
    out[k + 12] = flag(b.gear_two_handed(i));
    if own {
        out[k + 13] = flag(g.interaction);
        out[k + 14] = flag(g.after_attack);
        out[k + 15] = flag(f.hidden_dc.is_some());
    }
}

/// Public items on the floor and the current character's chosen weapon and payments.
fn gear_observation(b: &Battle, viewer: usize, out: &mut [f32]) {
    let f = &b.f[viewer];
    let g = &f.gear;
    out[584..590].copy_from_slice(&[
        flag(g.diamond50),
        g.diamonds300 as f32,
        g.incense10 as f32,
        g.holy_water25 as f32,
        flag(g.poisoners_kit),
        flag(b.gear_free_hand(viewer)),
    ]);
    out[590] = b
        .gear_weapon_kind(viewer, false)
        .map_or(0., |w| (1 + w as u8) as f32 / 9.);
    out[591] = b
        .gear_weapon_kind(viewer, true)
        .map_or(0., |w| (1 + w as u8) as f32 / 9.);
    if b.actor() == Some(viewer) && matches!(b.phase_id(), 68 | 69) {
        out[592] = (g.throw_cell.y + 1) as f32 / 7.;
        if b.phase_id() == 69 {
            out[593] = (g.throw_cell.x + 1) as f32 / 9.;
        }
    }
    for (n, i) in [viewer, 1 - viewer].into_iter().enumerate() {
        let gear = &b.f[i].gear;
        for (slot, loc) in [gear.melee, gear.bow].into_iter().enumerate() {
            let k = 600 + 4 * (2 * n + slot);
            if let Carried::Ground(pos) = loc {
                let p = b.map.landing(super::geometry::Point {
                    cell: pos,
                    altitude: super::geometry::MAX_ELEVATION,
                });
                if b.map.los_at(b.fighter_point(viewer), p) {
                    out[k] = (pos.x + 1) as f32 / 9.;
                    out[k + 1] = (pos.y + 1) as f32 / 7.;
                }
            }
            if viewer != i {
                out[k + 2] = flag(loc == Carried::ForeignHeld);
                out[k + 3] = flag(loc == Carried::ForeignPacked);
            }
        }
    }
}

fn armor(b: &Battle, i: usize) -> i32 {
    b.ac(i)
}

fn observation(b: &Battle, me: usize, mask: u128, out: &mut [f32]) {
    out.fill(0.);
    let foe = 1 - me;
    let a = b.f[me].pos;
    let target = b.observed_pos(me, foe);
    out[b.f[me].build.class as usize] = 1.;
    out[12 + b.f[foe].build.class as usize] = 1.;
    for (perspective, fighter) in [me, foe].into_iter().enumerate() {
        let variant = usize::from(b.f[fighter].build.variant);
        if (1..=2).contains(&variant) {
            out[916 + 2 * perspective + variant - 1] = 1.;
        }
    }
    out[28] = flag(b.owner == me);
    out[30] = b.turns as f32 / 60.;
    combatant_features(b, me, me, out);
    combatant_features(b, me, foe, out);
    if b.f[me].build.variant != 0 {
        let subclass = match b.f[me].build.class {
            Class::Barbarian | Class::Monk => b.martial_subclass_features(me),
            Class::Bard | Class::Ranger => b.bard_ranger_subclass_features(me),
            Class::Cleric | Class::Druid | Class::Paladin => b.divine_subclass_features(me),
            Class::Fighter | Class::Rogue => b.fighter_rogue_subclass_features(me),
            Class::Sorcerer | Class::Warlock | Class::Wizard => b.arcane_subclass_features(me),
        };
        out[108..116].copy_from_slice(&subclass);
    }
    let layout = b.map.layout_id();
    if layout < 4 {
        out[132 + layout] = 1.;
    } else {
        out[599] = (layout - 3) as f32;
    }
    if let Some(t) = target {
        out[26] = b.dist() as f32 / 8.;
        out[27] = b.map.path(a, t) as f32 / 14.;
        out[29] = flag(b.can_see(me, foe));
        out[154] = b.map.path_to_los(a, t) as f32 / 10.;
        out[158] = (t.x - a.x) as f32 / 8.;
        out[159] = (t.y - a.y) as f32 / 6.;
    }
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            out[370 + (y * WIDTH + x) as usize] = b.surface_tile(Pos { x, y }) as f32 / 2.;
        }
    }
    for (n, relative) in (4..super::objects::END_OBJECTS).enumerate() {
        let id = relative_id(me, relative);
        if let Some(obj) = b.observed_object(me, id) {
            let k = if n < 6 {
                768 + 8 * n
            } else {
                920 + 8 * (n - 6)
            };
            out[k..k + 8].copy_from_slice(&[
                1.,
                (obj.point.cell.x + 1) as f32 / 9.,
                (obj.point.cell.y + 1) as f32 / 7.,
                (obj.point.altitude + 1) as f32 / 7.,
                obj.ac as f32 / 20.,
                obj.hp as f32 / 10.,
                flag(obj.burning),
                flag(obj.kind.flammable()),
            ]);
        }
    }
    for (n, i) in [me, foe].into_iter().enumerate() {
        let k = 816 + 24 * n;
        out[k..k + 24].copy_from_slice(&b.magic_state_features(me, i));
    }
    let (rite, used) = b.warlock_rite_progress(me);
    out[908] = rite as f32 / 10.;
    out[909] = flag(used);
    for (n, id) in [me, foe].into_iter().enumerate() {
        if !perceived(b, me, id) {
            continue;
        }
        let k = 896 + 6 * n;
        out[k..k + 6].copy_from_slice(&[
            flag(b.armor_untrained(id)),
            flag(b.armor_holds_foreign(id)),
            flag(b.gear_holds_foreign_shield(id)),
            flag(b.gear_shield_worn(id)),
            b.gear_loose_shields(id) as f32 / 3.,
            flag(b.gear_carries_shield(id)),
        ]);
        if id != me {
            out[k + 1] = 0.;
            out[k + 2] = 0.;
            out[k + 5] = 0.;
        }
    }
    let identified = b.public_identified_spells(me);
    for bit in 0..40 {
        out[976 + bit] = flag(identified & (1 << bit) != 0);
    }
    for relative in 0..4 {
        let id = relative_id(me, relative);
        let visible = if id < 2 {
            perceived(b, me, id)
        } else {
            b.observed_entity(me, id).is_some()
        };
        if !visible {
            continue;
        }
        if id % 2 == me {
            out[912 + relative] = b.temporary_hp_source(id);
        }
        let k = 952 + 6 * relative;
        if let Some(body) = b.footprint(id) {
            out[k..k + 6].copy_from_slice(&[
                body.edge as f32 / 2.,
                b.combatant_size(id) as f32 / 5.,
                flag(b.is_squeezing(id)),
                flag(b.projectile_glowing(id)),
                flag(b.guiding_advantage(id)),
                b.frost_slow(id) as f32 / 2.,
            ]);
        }
    }
    let (prefix, extra) = out.split_at_mut(910);
    entity_view::collect_observation(b, me, &mut prefix[433..497], &mut extra[..2]);
    if b.owner == me {
        out[31] = b.movement_left() as f32 / 12.;
        out[141..146].copy_from_slice(&[
            flag(b.action),
            flag(b.bonus),
            b.attacks as f32 / 2.,
            b.moved as f32 / 12.,
            flag(b.disengaged),
        ]);
        out[157] = b.dash as f32 / 12.;
    }
    out[521..545].copy_from_slice(&b.common_decision_features(me));
    out[568..576].copy_from_slice(&b.traversal_features(me));
    if let Some((id, done)) = b.origin_mending_progress(me) {
        out[597] = (1 + relative_id(me, id)) as f32 / 10.;
        out[598] = done as f32 / 10.;
    }
    out[594] = b.origin_stonecunning_uses(me) as f32 / b.f[me].build.pb as f32;
    out[595] = flag(b.origin_tremorsense_active(me));
    out[596] = flag(b.map.touches_stone(b.fighter_point(me)));
    for (n, i) in [me, foe].into_iter().enumerate() {
        if !perceived(b, me, i) {
            continue;
        }
        let [weight, worn, task, remaining] = b.armor_projection(i);
        let k = 576 + n * 4;
        out[k..k + 4].copy_from_slice(&[
            weight as f32 / 3.,
            worn as f32,
            task as f32 / 2.,
            remaining as f32 / 100.,
        ]);
    }
    out[632..640].copy_from_slice(&b.weapon_option_features(me));
    gear_observation(b, me, out);
    if b.actor() != Some(me) {
        return;
    }
    let phase = b.phase_id() as usize;
    if phase < 74 {
        out[256 + phase] = 1.;
    } else if phase < 90 {
        out[616 + phase - 74] = 1.;
    } else if phase < 122 {
        out[864 + phase - 90] = 1.;
    } else if (SUBCLASS_PHASE_BASE..SUBCLASS_PHASE_BASE + SUBCLASS_PHASES).contains(&phase) {
        // Encode new menus with the quotient and remainder of the phase
        // index, keeping both sets of bits clear in legacy phases.
        let phase = phase - SUBCLASS_PHASE_BASE;
        out[116 + phase % SUBCLASS_PHASE_WIDTH] = 1.;
        out[124 + phase / SUBCLASS_PHASE_WIDTH] = 1.;
    }
    if phase <= 6 {
        out[136 + phase.min(4)] = 1.;
    }
    out[146..154].copy_from_slice(&b.pending_features());
    if phase == 0 && b.phantasmal_force_dc(me).is_some() {
        out[153] = 1.;
    }
    if b.phase_id() == super::magic::PHANTASM_HEIGHT {
        if let Some(cell) = b.phantasm_pending_cell(me) {
            out[156] = (cell.y + 1) as f32 / 7.;
            out[157] = (cell.x + 1) as f32 / 9.;
        }
    }
    let targeting = b.save_choice_actor().is_none();
    if let Some((action, row)) = targeting.then(|| b.target_request()).flatten() {
        if let Some(spec) = b.target_spec(action) {
            out[154] = (1 + spec.kind as u8) as f32;
        }
        out[155] = action as usize as f32 / 32.;
        out[156] = row.map_or(0., |y| f32::from(y + 1) / 7.);
        out[157] = flag(row.is_some());
        for axis in 0..9 {
            let offset = if axis < 8 { 160 + axis * 8 } else { 146 };
            out[offset..offset + 8].copy_from_slice(&b.target_choice_features(row, action, axis));
        }
    } else if let Some((action, cell)) = targeting.then(|| b.target_altitude_request()).flatten() {
        out[154] = b
            .target_spec(action)
            .map_or(0., |s| (1 + s.kind as u8) as f32);
        out[155] = action as usize as f32 / 32.;
        out[156] = (cell.y + 1) as f32 / 7.;
        out[157] = (cell.x + 1) as f32 / 9.;
        for h in 0..7 {
            out[160 + h * 8..168 + h * 8]
                .copy_from_slice(&b.target_altitude_features(action, cell, h));
        }
    } else if phase == 112 {
        let ids = b.target_permutation();
        for id in 0..4 {
            out[160 + 8 * id..168 + 8 * id]
                .copy_from_slice(&b.armor_candidate(me, ids.decode(id)));
        }
    } else if matches!(phase, 13 | 16..=18 | 72) {
        let ids = b.target_permutation();
        for index in 0..9 {
            let k = if index < 8 { 160 + 8 * index } else { 146 };
            let values = if phase == 72 {
                b.weapon_option_candidate(me, ids.decode(index))
            } else {
                b.traversal_choice_features(me, ACTS[index])
            };
            out[k..k + 8].copy_from_slice(&values);
        }
    } else {
        for dir in 0..8 {
            if let Some((p, cost)) = b.map.step(a, dir) {
                if Some(p) == target {
                    continue;
                }
                let d = 160 + dir * 8;
                out[d] = 1.;
                out[d + 1] = cost as f32 / 2.;
                if let Some(t) = target {
                    out[d + 2..d + 8].copy_from_slice(&[
                        p.dist(t) as f32 / 8.,
                        (b.map.path(a, t) - b.map.path(p, t)) as f32 / 3.,
                        flag(b.map.los(p, t)),
                        (b.map.path_to_los(a, t) - b.map.path_to_los(p, t)) as f32 / 3.,
                        flag(
                            a.dist(t) == 1 && p.dist(t) > 1 && !b.disengaged && b.can_see(me, foe),
                        ),
                        flag(p.dist(t) == 1 && b.map.los(p, t)),
                    ]);
                }
            }
        }
    }
    out[497..521].copy_from_slice(&b.magic_decision_features(me));
    if matches!(phase, 86 | 87) {
        out[545..552].copy_from_slice(&b.origin_decision_features(me));
    } else if matches!(phase, 92..=96 | 104 | 105) {
        out[545..552].copy_from_slice(&b.area_choice_features(me));
    } else {
        out[545..552].copy_from_slice(&b.projectile_features(me));
    }
    if matches!(phase, 78 | 79 | 97..=99 | 101) {
        out[497..505].copy_from_slice(&b.class_feature_observation(me));
    }
    if matches!(phase, 107..=111) {
        out[497..505].copy_from_slice(&b.ready_spell_features(me));
    }
    if matches!(phase, 73..=77 | 117 | 121) {
        out[497..505].copy_from_slice(&b.spell_movement_features(me));
    }
    let response = (phase == 101 || (phase >= SUBCLASS_PHASE_BASE && !targeting))
        .then(|| b.save_choice_features())
        .flatten()
        .or_else(|| b.arcane_initiative_features(me));
    if let Some(roll) = response {
        out[1016..1024].copy_from_slice(&roll);
    }
    out[552..568].copy_from_slice(&b.martial_decision_features(me));
    for a in ACTS {
        let id = a as usize;
        let legal = flag(mask & a.bit() != 0);
        if id < 32 {
            out[224 + id] = legal;
        }
        out[640 + id] = legal;
    }
}

fn raw(b: &Battle, viewer: usize, row: &mut [i32]) {
    row.fill(0);
    row[..12].copy_from_slice(&[
        b.actor().map_or(-1, |x| x as i32),
        b.owner as i32,
        b.turns,
        b.phase_id(),
        result(b),
        b.movement_left(),
        b.action as i32,
        b.bonus as i32,
        b.attacks,
        b.moved,
        b.disengaged as i32,
        b.map.layout_id() as i32,
    ]);
    let own_decision = b.actor() == Some(viewer);
    let targeting = own_decision && b.save_choice_actor().is_none();
    if !own_decision && !b.finished() {
        row[3] = -1;
    }
    if b.owner != viewer {
        row[5..11].fill(0);
    }
    row[84] = reason(b);
    let target = targeting.then(|| b.target_request()).flatten();
    let altitude = targeting.then(|| b.target_altitude_request()).flatten();
    row[97] = altitude.map_or(-1, |(a, _)| a as i32);
    row[98] = altitude.map_or(-1, |(_, p)| p.x as i32);
    row[99] = altitude.map_or(-1, |(_, p)| p.y as i32);
    row[82] = target.map_or(-1, |(act, _)| act as i32);
    row[83] = if targeting {
        target
            .and_then(|(_, r)| r)
            .or_else(|| entity_view::phase_row(b, viewer))
            .or_else(|| b.spell_movement_row(viewer))
            .or_else(|| b.projectile_target_row(viewer))
            .map_or(-1, i32::from)
    } else {
        -1
    };
    for id in 4..super::objects::END_OBJECTS {
        if let Some(obj) = b.observed_object(viewer, id) {
            let k = 110 + 8 * (id - 4);
            row[k..k + 8].copy_from_slice(&[
                1,
                obj.point.cell.x.into(),
                obj.point.cell.y.into(),
                obj.point.altitude.into(),
                obj.ac,
                obj.hp,
                i32::from(obj.burning),
                i32::from(obj.kind.flammable()),
            ]);
        }
    }
    row[108] = b.origin_stonecunning_uses(viewer);
    row[109] = i32::from(b.origin_tremorsense_active(viewer));
    for (i, f) in b.f.iter().enumerate() {
        row[190 + i] = i32::from(f.build.variant);
        let j = 12 + i * 35;
        let hazard = b
            .observed_object(viewer, 8 + i)
            .map(|object| object.point.cell);
        row[j..j + 35].copy_from_slice(&[
            f.build.class as i32,
            f.build.level,
            f.hp,
            if perceived(b, viewer, i) {
                b.magic_max_hp(i)
            } else {
                f.build.hp
            },
            f.temp,
            armor(b, i),
            f.pos.x as i32,
            f.pos.y as i32,
            f.resource,
            f.secondary,
            f.slots[0],
            f.slots[1],
            f.slots[2],
            f.reaction as i32,
            f.dodge as i32,
            f.shield,
            f.sap as i32,
            f.slow as i32,
            f.stunned as i32,
            f.stun_slow as i32,
            f.stun_exposed as i32,
            f.attack_advantage as i32,
            f.attack_disadvantage as i32,
            f.speed_zero as i32,
            f.raging as i32,
            f.reckless as i32,
            b.public_concentration(viewer, i) as i32,
            f.build.speed,
            f.build.melee_die,
            f.build.ranged_die,
            f.build.pb,
            f.build.casting,
            f.arrows,
            hazard.map_or(-1, |p| p.x as i32),
            hazard.map_or(-1, |p| p.y as i32),
        ]);
        let k = 85 + 6 * i;
        let known = b.observed_pos(viewer, i).is_some();
        let visible = perceived(b, viewer, i);
        row[k..k + 6].copy_from_slice(&[
            known as i32,
            visible as i32,
            if known { i32::from(f.elevation) } else { -1 },
            i32::from(visible && f.prone),
            i32::from(visible && f.grappled.is_some()),
            i32::from(viewer == i && f.hidden_dc.is_some()),
        ]);
        if visible {
            row[100 + 4 * i..104 + 4 * i].copy_from_slice(&b.armor_projection(i));
        }
        if !known {
            row[j + 6..j + 8].fill(-1);
        }
        if !visible {
            row[j + 2] = -1;
            row[j + 4..j + 6].fill(-1);
            row[j + 14..j + 27].fill(0);
        }
        if viewer != i {
            row[j + 8..j + 14].fill(-1);
            row[j + 32] = -1;
        }
    }
}

#[no_mangle]
extern "C" fn arena_features() -> usize {
    FEATURES
}
#[no_mangle]
extern "C" fn arena_actions() -> usize {
    ACTIONS
}
#[no_mangle]
extern "C" fn arena_schema() -> u32 {
    2002
}
#[no_mangle]
extern "C" fn arena_new(n: usize) -> *mut Batch {
    if n == 0 || n > 65536 {
        return std::ptr::null_mut();
    }
    Box::into_raw(Box::new(Batch(
        (0..n)
            .map(|i| Battle::new(i as u64, [Class::Fighter; 2], [1; 2]))
            .collect(),
    )))
}
#[no_mangle]
unsafe extern "C" fn arena_free(batch: *mut Batch) {
    if !batch.is_null() {
        drop(Box::from_raw(batch));
    }
}
#[no_mangle]
unsafe extern "C" fn arena_reset(
    batch: &mut Batch,
    k: usize,
    idx: *const i32,
    seeds: *const u64,
    classes: *const i32,
    levels: *const i32,
) -> bool {
    let indices = slice::from_raw_parts(idx, k);
    let seeds = slice::from_raw_parts(seeds, k);
    let cs = slice::from_raw_parts(classes, k * 2);
    let ls = slice::from_raw_parts(levels, k * 2);
    if indices
        .iter()
        .any(|&v| v < 0 || v as usize >= batch.0.len())
        || cs.iter().any(|v| !(0..12).contains(v))
        || ls.iter().any(|v| !(1..=5).contains(v))
    {
        return false;
    }
    for (j, &idx) in indices.iter().enumerate() {
        batch.0[idx as usize] = Battle::new(
            seeds[j],
            [CLASSES[cs[j * 2] as usize], CLASSES[cs[j * 2 + 1] as usize]],
            [ls[j * 2], ls[j * 2 + 1]],
        );
    }
    true
}

#[no_mangle]
extern "C" fn arena_roster_size() -> usize {
    ROSTER.len()
}

#[no_mangle]
unsafe extern "C" fn arena_roster_text(
    id: usize,
    description: bool,
    out: *mut c_char,
    cap: usize,
) -> bool {
    ROSTER.get(id).is_some_and(|preset| {
        let text = if description {
            preset.description
        } else {
            preset.name
        };
        write_text(text, out, cap)
    })
}

#[no_mangle]
unsafe extern "C" fn arena_reset_builds(
    batch: &mut Batch,
    k: usize,
    idx: *const i32,
    seeds: *const u64,
    builds: *const u8,
    arenas: *const i32,
) -> bool {
    if k == 0 {
        return true;
    }
    let indices = slice::from_raw_parts(idx, k);
    let seeds = slice::from_raw_parts(seeds, k);
    let builds = slice::from_raw_parts(builds.cast::<[u8; 2]>(), k);
    let arenas = (!arenas.is_null()).then(|| slice::from_raw_parts(arenas, k));
    if indices
        .iter()
        .any(|&v| v < 0 || v as usize >= batch.0.len())
        || builds
            .iter()
            .flatten()
            .any(|&v| usize::from(v) >= ROSTER.len())
        || arenas.is_some_and(|ids| {
            ids.iter()
                .any(|&v| v < 0 || v as usize >= arena_layout_count())
        })
    {
        return false;
    }
    for (j, &idx) in indices.iter().enumerate() {
        batch.0[idx as usize] = if let Some(arenas) = arenas {
            Battle::new_roster_in(seeds[j], builds[j], arenas[j] as usize)
        } else {
            Battle::new_roster(seeds[j], builds[j])
        };
    }
    true
}

#[no_mangle]
extern "C" fn arena_layout_count() -> usize {
    super::geometry::LAYOUT_COUNT
}
#[no_mangle]
unsafe extern "C" fn arena_observe(
    batch: &Batch,
    x: *mut f32,
    masks: *mut u8,
    actors: *mut i32,
    results: *mut i32,
) {
    for (i, b) in batch.0.iter().enumerate() {
        let mask = b.mask();
        let row = slice::from_raw_parts_mut(x.add(i * FEATURES), FEATURES);
        observation(b, b.actor().unwrap_or(b.owner), mask, row);
        for a in 0..ACTIONS {
            *masks.add(i * ACTIONS + a) = ((mask >> a) & 1) as u8;
        }
        *actors.add(i) = b.actor().map_or(-1, |p| p as i32);
        *results.add(i) = result(b);
    }
}
#[no_mangle]
unsafe extern "C" fn arena_choices(batch: &Batch, expert: bool, out: *mut i32) {
    for (i, b) in batch.0.iter().enumerate() {
        *out.add(i) = if b.finished() {
            -1
        } else {
            bot::choose(b, expert) as i32
        };
    }
}
#[no_mangle]
unsafe extern "C" fn arena_step(batch: &mut Batch, actions: *const i32) -> i32 {
    let actions = slice::from_raw_parts(actions, batch.0.len());
    for (i, (b, &id)) in batch.0.iter().zip(actions).enumerate() {
        if id != -1
            && (!(0..ACTIONS as i32).contains(&id) || b.mask() & ACTS[id as usize].bit() == 0)
        {
            return i as i32;
        }
    }
    for (b, &id) in batch.0.iter_mut().zip(actions) {
        if id >= 0 {
            b.step(ACTS[id as usize]).expect("action validated");
        }
    }
    -1
}
#[no_mangle]
unsafe extern "C" fn arena_raw(
    batch: &Batch,
    i: usize,
    viewer: i32,
    out: *mut i32,
    terrain: *mut u8,
) -> bool {
    let Some(b) = batch.0.get(i) else {
        return false;
    };
    let Some(viewer) = resolve_viewer(b, viewer) else {
        return false;
    };
    raw(b, viewer, slice::from_raw_parts_mut(out, RAW));
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            *terrain.add(y as usize * WIDTH as usize + x as usize) = b.surface_tile(Pos { x, y });
        }
    }
    true
}
fn resolve_viewer(b: &Battle, viewer: i32) -> Option<usize> {
    match viewer {
        -1 => Some(b.actor().unwrap_or(b.owner)),
        0 | 1 => Some(viewer as usize),
        _ => None,
    }
}

#[no_mangle]
unsafe extern "C" fn arena_results(batch: &Batch, out: *mut i32) {
    for (i, b) in batch.0.iter().enumerate() {
        *out.add(i) = reason(b);
    }
}

#[no_mangle]
unsafe extern "C" fn arena_view(
    batch: &Batch,
    i: usize,
    viewer: i32,
    x: *mut f32,
    masks: *mut u8,
) -> bool {
    let Some(b) = batch.0.get(i) else {
        return false;
    };
    let Some(viewer) = resolve_viewer(b, viewer) else {
        return false;
    };
    let mask = if b.actor() == Some(viewer) {
        b.mask()
    } else {
        0
    };
    observation(b, viewer, mask, slice::from_raw_parts_mut(x, FEATURES));
    for a in 0..ACTIONS {
        *masks.add(a) = ((mask >> a) & 1) as u8;
    }
    true
}

#[no_mangle]
unsafe extern "C" fn arena_text(
    batch: &Batch,
    i: usize,
    viewer: i32,
    code: i32,
    out: *mut c_char,
    cap: usize,
) -> bool {
    let Some(b) = batch.0.get(i) else {
        return false;
    };
    let Some(viewer) = resolve_viewer(b, viewer) else {
        return false;
    };
    let text = match code {
        140..=149 => b
            .observed_object(viewer, 4 + (code - 140) as usize)
            .map_or_else(String::new, |o| o.label().to_owned()),
        128..=129 if (code - 128) as usize != viewer => {
            b.magic_public_summary(viewer, (code - 128) as usize)
        }
        0..=127 if b.actor() == Some(viewer) => b.choice_label(viewer, ACTS[code as usize]),
        128 | 129 if (code - 128) as usize == viewer => {
            format!("{} {}", b.martial_summary(viewer), b.magic_summary(viewer))
                .trim()
                .to_owned()
        }
        130 if b.actor() == Some(viewer) => match b.phase_id() {
            73..=77 | 117 | 121 => b.spell_movement_pending(viewer),
            80..=85 => b.projectile_pending(viewer),
            _ => b.pending_summary(),
        },
        151 => b.spellbook(viewer),
        152 => table_view::cover_map(b, viewer),
        153 if b.actor() == Some(viewer) && b.save_choice_actor().is_none() => table_view::target_columns(b),
        154 if b.actor() == Some(viewer) && b.phase_id() == 0 => (0..8)
            .map(|d| b.movement_step(viewer, d).map_or(0, |(_, cost)| cost).to_string())
            .collect::<Vec<_>>().join(" "),
        156 if b.actor() == Some(viewer) && b.phase_id() == 0 => table_view::ground_edges(b),
        155 if b.actor() == Some(viewer) && b.save_choice_actor().is_none() => table_view::area_slices(b),
        150 => b
            .study_candidates(viewer)
            .into_iter()
            .filter(|target| b.public_identified_spells(viewer) & (1 << target.id) != 0)
            .map(|target| target.identity)
            .collect::<Vec<_>>()
            .join(", "),
        133 | 134 => {
            let i = (code - 133) as usize;
            if perceived(b, viewer, i) {
                b.armor_status(i)
            } else {
                String::new()
            }
        }
        131 | 132 => {
            let side = (code - 131) as usize;
            let f = &b.f[side];
            let base = EQUIPMENT[f.build.class as usize];
            if b.magic_is_wolf(side) && perceived(b, viewer, side) {
                format!("{base} (carried equipment merged into Wild Shape)")
            } else if side == viewer {
                format!("{base} ({})", f.gear.describe())
            } else {
                base.into()
            }
        }
        _ => String::new(),
    };
    write_text(&text, out, cap)
}

unsafe fn write_text(text: &str, out: *mut c_char, cap: usize) -> bool {
    if out.is_null() || text.len() >= cap {
        return false;
    }
    std::ptr::copy_nonoverlapping(text.as_ptr(), out.cast(), text.len());
    *out.add(text.len()) = 0;
    true
}

// Run scripted opponents inside the batch and stop at the first player decision,
// including a response inside the other player's turn. -1 disables the lane.
#[no_mangle]
unsafe extern "C" fn arena_advance(batch: &mut Batch, seats: *const i32, expert: bool) -> bool {
    let seats = slice::from_raw_parts(seats, batch.0.len());
    if seats.iter().any(|v| !(-1..=1).contains(v)) {
        return false;
    }
    for (b, &seat) in batch.0.iter_mut().zip(seats) {
        if seat < 0 {
            continue;
        }
        while let Some(actor) = b.actor() {
            if actor == seat as usize {
                break;
            }
            let choice = bot::choose(b, expert);
            b.step(choice).expect("bot actions are legal");
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena2::Act;

    #[test]
    fn deployed_caltrops_visibility_depends_on_the_object_and_not_its_owner() {
        use crate::arena2::class::Class;
        use crate::arena2::geometry::{Map, Point};
        for viewer in 0..2 {
            let source = 1 - viewer;
            let mut battle = Battle::new(17, [Class::Rogue; 2], [5; 2]);
            battle.owner = viewer;
            battle.map = Map::new(0);
            battle.f[viewer].pos = Pos { x: 0, y: 0 };
            let observer = battle.fighter_point(viewer);
            let visible = Pos { x: 1, y: 0 };
            let hidden = (0..HEIGHT)
                .flat_map(|y| (0..WIDTH).map(move |x| Pos { x, y }))
                .find(|&p| {
                    battle.map.can_occupy(Point::ground(p))
                        && !battle.map.los_at(observer, Point::ground(p))
                })
                .unwrap();
            for (trap, owner) in [(hidden, visible), (visible, hidden)] {
                battle.f[source].pos = owner;
                battle.place_readied_caltrops(source, trap);
                let seen = trap == visible;
                let mut snap = [0; RAW];
                raw(&battle, viewer, &mut snap);
                let offset = 12 + 35 * source + 33;
                assert_eq!(
                    &snap[offset..offset + 2],
                    if seen {
                        [i32::from(trap.x), i32::from(trap.y)]
                    } else {
                        [-1, -1]
                    }
                );
                let mut obs = [0.; FEATURES];
                observation(&battle, viewer, battle.mask(), &mut obs);
                assert_eq!(obs[768 + 8 * 5], flag(seen));
            }
        }
    }

    #[test]
    fn relative_armor_candidates_align_with_objects_and_the_selected_issuer() {
        use crate::arena2::{geometry::{Map, Point}, temp_hp::TempSource};

        for actor in 0..2 {
            let foe = 1 - actor;
            let mut classes = [Class::Rogue; 2];
            classes[actor] = Class::Fighter;
            let mut b = Battle::new(1, classes, [5; 2]);
            b.owner = actor;
            b.map = Map::new(0);
            b.f[actor].pos = Pos { x: 1, y: 0 };
            b.f[foe].pos = Pos { x: 3, y: 0 };
            let at = Point::ground(b.f[actor].pos);
            b.drop_form_armor(actor, at);
            b.drop_form_armor(foe, at);
            b.grant_temporary_hp(actor, 4, TempSource::FiendishVigor);
            b.grant_temporary_hp(foe, 5, TempSource::Heroism(foe));
            b.step(Act::Other5).unwrap();
            assert_eq!(b.phase_id(), 112);

            let mut obs = [0.; FEATURES];
            observation(&b, actor, b.mask(), &mut obs);
            assert_eq!(&obs[160..163], &[1., 0., 1.]);
            assert_eq!(&obs[168..171], &[1., 1., 1. / 3.]);
            assert_eq!((obs[640], obs[641]), (1., 1.));
            assert_eq!((obs[920], obs[928]), (1., 1.));
            assert_eq!((obs[927], obs[935]), (0., 1.));
            assert_eq!(&obs[912..916], &[0.5, 0., 0., 0.]);

            observation(&b, foe, 0, &mut obs);
            assert_eq!(&obs[912..916], &[0.25, 0., 0., 0.]);
            assert_eq!((obs[927], obs[935]), (1., 0.));
            assert_eq!(obs[886], 0.);
            assert_eq!((obs[640], obs[641]), (0., 0.));

            b.step(ACTS[0]).unwrap();
            assert!(b.object(10 + actor).is_none());
            assert!(b.object(10 + foe).is_some());
        }
    }

    #[test]
    fn unseen_hidden_locations_change_neither_mask_nor_observation_for_any_preset() {
        let places = [
            Pos { x: 0, y: 1 },
            Pos { x: 1, y: 0 },
            Pos { x: 4, y: 0 },
            Pos { x: 7, y: 5 },
        ];
        let mut leaks = std::collections::BTreeSet::new();
        for viewer in 0..=1 {
            for class in CLASSES {
                for level in 1..=5 {
                    let mut classes = [Class::Rogue; 2];
                    classes[viewer] = class;
                    let mut b = Battle::new(11, classes, [level; 2]);
                    b.owner = viewer;
                    b.f[viewer].pos = Pos { x: 0, y: 0 };
                    let target = 1 - viewer;
                    b.f[target].hidden_dc = Some(18);
                    let mut baseline: Option<(u128, [f32; FEATURES], [i32; RAW])> = None;
                    for p in places {
                        b.f[target].pos = p;
                        let mask = b.mask();
                        let mut features = [0.; FEATURES];
                        observation(&b, viewer, mask, &mut features);
                        let mut snap = [0; RAW];
                        raw(&b, viewer, &mut snap);
                        if let Some((old_mask, old_features, old_snap)) = baseline {
                            if mask != old_mask {
                                let changes: Vec<_> = ACTS
                                    .iter()
                                    .filter(|a| (mask ^ old_mask) & a.bit() != 0)
                                    .collect();
                                leaks.insert(format!("{class:?} mask {changes:?}"));
                            }
                            for j in (0..640).chain(768..FEATURES) {
                                if (j < 224 || j >= 256) && features[j] != old_features[j] {
                                    leaks.insert(format!("{class:?} feature {j}"));
                                }
                            }
                            if old_snap != snap {
                                leaks.insert(format!("{class:?} raw view"));
                            }
                        } else {
                            baseline = Some((mask, features, snap));
                        }
                    }
                }
            }
        }
        assert!(
            leaks.is_empty(),
            "hidden opponent location leaks: {leaks:?}"
        );
    }

    #[test]
    fn other_seat_cannot_read_action_menu_while_actor_selects_parameters() {
        for actor in 0..=1 {
            let mut b = Battle::new(1, [Class::Bard; 2], [5; 2]);
            b.owner = actor;
            b.step(Act::Ready).unwrap();
            assert_ne!(b.phase_id(), 0);
            let mut obs = [0.; FEATURES];
            observation(&b, 1 - actor, 0, &mut obs);
            assert!(obs[256..330].iter().all(|&v| v == 0.));
            assert!(obs[640..768].iter().all(|&v| v == 0.));
            let mut snapshot = [0; RAW];
            raw(&b, 1 - actor, &mut snapshot);
            assert_eq!(snapshot[3], -1);
            assert_eq!(snapshot[82], -1);
            observation(&b, actor, b.mask(), &mut obs);
            assert_eq!(obs[256 + b.phase_id() as usize], 1.);
            assert!(obs[640..768].iter().any(|&v| v == 1.));
        }
    }
}
