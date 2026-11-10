use flecs_ecs::prelude::*;
use std::sync::Arc;
use parking_lot::Mutex;
use steel::gc::Gc;
use steel::rvals::SteelVal;
use steel::steel_vm::engine::Engine;
use steel::steel_vm::register_fn::RegisterFn;
use game_script::{json_to_steel_val, steel_val_to_json};

use crate::ecs::components::{Color, DynamicProps};
use crate::ecs::rasterizer::ViewportRasterizer;
use crate::ecs::world::EcsWorld;

/// Name of the host-managed singleton entity that is the SINGLE
/// authoritative store for all model causal state (snapshot v3: the
/// former Scheme `*world-state*` hash). Every top-level state key is
/// one entry of this entity's `DynamicProps`, so the existing
/// `export_world_json` / `import_world_json` path serializes the whole
/// model state with the rest of the Flecs world — there is no second
/// store and no separate `world_state` snapshot section any more.
pub const WORLD_STATE_ENTITY_NAME: &str = "WorldState";

/// Look up (creating on first use) the world-state singleton entity
/// and guarantee it carries a (possibly empty) `DynamicProps`
/// component. `spawn_named` is idempotent by name in Flecs, so this
/// is cheap and safe to call on every state access; after a world
/// clear / snapshot import it simply re-resolves the entity.
pub fn ensure_world_state_entity(ecs: &mut EcsWorld) -> u64 {
    let id = ecs.spawn_named(WORLD_STATE_ENTITY_NAME);
    let has_props = ecs
        .world
        .entity_from_id(id)
        .try_get::<&DynamicProps>(|_| ())
        .is_some();
    if !has_props {
        ecs.world.entity_from_id(id).set(DynamicProps::default());
    }
    id
}

// Singleton accessors that do NOT go through `EcsWorld::get_prop` /
// `set_prop`: those clone the entity's whole `DynamicProps`, which
// for the world-state singleton means deep-cloning the ENTIRE model
// state on every single key access (measured: a plain on-tick went
// from 0.26 ms pre-migration to ~16 ms). `try_get` with `&` / `&mut`
// component references touches only the requested key's value.

fn ws_get(ecs: &EcsWorld, id: u64, key: &str) -> Option<serde_json::Value> {
    ecs.world
        .entity_from_id(id)
        .try_get::<&DynamicProps>(|p| p.get(key).cloned())
        .flatten()
}

fn ws_has(ecs: &EcsWorld, id: u64, key: &str) -> bool {
    ecs.world
        .entity_from_id(id)
        .try_get::<&DynamicProps>(|p| p.has(key))
        .unwrap_or(false)
}

fn ws_set(ecs: &mut EcsWorld, id: u64, key: &str, val: serde_json::Value) -> bool {
    let done = ecs
        .world
        .entity_from_id(id)
        .try_get::<&mut DynamicProps>(|p| p.set(key.to_string(), val.clone()))
        .is_some();
    if done {
        return true;
    }
    let mut props = DynamicProps::new();
    props.set(key.to_string(), val);
    ecs.world.entity_from_id(id).set(props);
    true
}

fn ws_remove(ecs: &mut EcsWorld, id: u64, key: &str) -> bool {
    ecs.world
        .entity_from_id(id)
        .try_get::<&mut DynamicProps>(|p| p.remove(key).is_some())
        .unwrap_or(false)
}

/// All world-state key/value pairs as a JSON object (empty object when
/// the singleton carries no keys yet). Wholesale by definition — used
/// by export paths, not by per-key access.
pub fn world_state_json(ecs: &mut EcsWorld) -> serde_json::Value {
    let id = ensure_world_state_entity(ecs);
    match ecs
        .world
        .entity_from_id(id)
        .try_get::<&DynamicProps>(|p| {
            serde_json::Value::Object(p.values.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        }) {
        Some(v) => v,
        None => serde_json::json!({}),
    }
}

/// Replace the whole world-state contents with `data` (must be a JSON
/// object). Returns false (leaving state untouched) for non-objects.
pub fn set_world_state_json(ecs: &mut EcsWorld, data: &serde_json::Value) -> bool {
    let Some(map) = data.as_object() else {
        return false;
    };
    let id = ensure_world_state_entity(ecs);
    let mut props = DynamicProps::new();
    for (k, v) in map {
        props.set(k.clone(), v.clone());
    }
    ecs.world.entity_from_id(id).set(props);
    true
}

fn steel_to_string(v: &SteelVal) -> String {
    match v {
        SteelVal::StringV(s) => s.to_string(),
        SteelVal::SymbolV(s) => s.to_string(),
        other => other.to_string(),
    }
}

fn val_to_eid(v: &SteelVal) -> Option<u64> {
    match v {
        SteelVal::IntV(i) if *i >= 0 => Some(*i as u64),
        SteelVal::NumV(f) if *f >= 0.0 => Some(*f as u64),
        _ => None,
    }
}

fn steel_to_char(v: &SteelVal) -> char {
    match v {
        SteelVal::CharV(c) => *c,
        SteelVal::StringV(s) => s.chars().next().unwrap_or(' '),
        SteelVal::SymbolV(s) => s.chars().next().unwrap_or(' '),
        SteelVal::IntV(n) => char::from_u32(*n as u32).unwrap_or(' '),
        _ => ' ',
    }
}

fn steel_to_u8(v: &SteelVal) -> u8 {
    match v {
        SteelVal::IntV(n) => (*n).clamp(0, 255) as u8,
        SteelVal::NumV(f) => (*f).clamp(0.0, 255.0) as u8,
        _ => 0,
    }
}

fn parse_rgb(v: &SteelVal) -> Option<Color> {
    match v {
        SteelVal::ListV(l) if l.len() >= 3 => {
            let r = steel_to_u8(&l[0]);
            let g = steel_to_u8(&l[1]);
            let b = steel_to_u8(&l[2]);
            Some(Color::new(r, g, b))
        }
        _ => None,
    }
}

fn steel_to_bool(v: &SteelVal) -> bool {
    match v {
        SteelVal::BoolV(b) => *b,
        _ => v.is_truthy(),
    }
}

/// Register the complete suite of `ecs:*` Scheme FFI bindings into the Steel VM engine.
pub fn register_ecs_primitives(
    engine: &mut Engine,
    ecs: Arc<Mutex<EcsWorld>>,
    rasterizer: Arc<Mutex<ViewportRasterizer>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // ------------------------------------------------------------------------
    // 1. Entity Lifecycle
    // ------------------------------------------------------------------------
    let e1 = Arc::clone(&ecs);
    engine.register_fn("__ecs:spawn", move || -> isize {
        e1.lock().spawn() as isize
    });

    let e2 = Arc::clone(&ecs);
    engine.register_fn("__ecs:spawn-named", move |name: SteelVal| -> isize {
        let n = steel_to_string(&name);
        e2.lock().spawn_named(&n) as isize
    });

    let e3 = Arc::clone(&ecs);
    engine.register_fn("ecs:despawn", move |eid: SteelVal| -> bool {
        match val_to_eid(&eid) {
            Some(id) => e3.lock().despawn(id),
            None => false,
        }
    });

    let e4 = Arc::clone(&ecs);
    engine.register_fn("ecs:exists?", move |eid: SteelVal| -> bool {
        match val_to_eid(&eid) {
            Some(id) => e4.lock().exists(id),
            None => false,
        }
    });

    let e5 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:set-pos",
        move |eid: SteelVal, x: isize, y: isize, layer: isize| -> bool {
            match val_to_eid(&eid) {
                Some(id) => e5.lock().set_pos(id, x as i32, y as i32, layer as u8),
                None => false,
            }
        },
    );

    let e6 = Arc::clone(&ecs);
    engine.register_fn("ecs:get-pos", move |eid: SteelVal| -> SteelVal {
        match val_to_eid(&eid).and_then(|id| e6.lock().get_pos(id)) {
            Some(pos) => SteelVal::ListV(
                vec![
                    SteelVal::IntV(pos.x as isize),
                    SteelVal::IntV(pos.y as isize),
                    SteelVal::IntV(pos.layer as isize),
                ]
                .into(),
            ),
            None => SteelVal::BoolV(false),
        }
    });

    let e7 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:set-glyph",
        move |eid: SteelVal,
              ch: SteelVal,
              fg: SteelVal,
              bg: SteelVal,
              solid: SteelVal,
              opaque: SteelVal|
              -> bool {
            match val_to_eid(&eid) {
                Some(id) => {
                    let char_val = steel_to_char(&ch);
                    let fg_col = parse_rgb(&fg);
                    let bg_col = parse_rgb(&bg);
                    let is_solid = steel_to_bool(&solid);
                    let is_opaque = steel_to_bool(&opaque);
                    e7.lock()
                        .set_glyph(id, char_val, fg_col, bg_col, is_solid, is_opaque)
                }
                None => false,
            }
        },
    );

    let e8 = Arc::clone(&ecs);
    engine.register_fn("ecs:get-glyph", move |eid: SteelVal| -> SteelVal {
        match val_to_eid(&eid).and_then(|id| e8.lock().get_glyph(id)) {
            Some(glyph) => {
                let mut map = steel::HashMap::new();
                map.insert(SteelVal::StringV("ch".into()), SteelVal::CharV(glyph.ch));
                map.insert(
                    SteelVal::StringV("glyph".into()),
                    SteelVal::StringV(glyph.ch.to_string().into()),
                );
                map.insert(
                    SteelVal::StringV("fg".into()),
                    match glyph.fg {
                        Some(col) => SteelVal::ListV(
                            vec![
                                SteelVal::IntV(col.r as isize),
                                SteelVal::IntV(col.g as isize),
                                SteelVal::IntV(col.b as isize),
                            ]
                            .into(),
                        ),
                        None => SteelVal::BoolV(false),
                    },
                );
                map.insert(
                    SteelVal::StringV("bg".into()),
                    match glyph.bg {
                        Some(col) => SteelVal::ListV(
                            vec![
                                SteelVal::IntV(col.r as isize),
                                SteelVal::IntV(col.g as isize),
                                SteelVal::IntV(col.b as isize),
                            ]
                            .into(),
                        ),
                        None => SteelVal::BoolV(false),
                    },
                );
                map.insert(
                    SteelVal::StringV("solid".into()),
                    SteelVal::BoolV(glyph.solid),
                );
                map.insert(
                    SteelVal::StringV("opaque".into()),
                    SteelVal::BoolV(glyph.opaque),
                );
                SteelVal::HashMapV(Gc::new(map).into())
            }
            None => SteelVal::BoolV(false),
        }
    });

    let e9 = Arc::clone(&ecs);
    engine.register_fn("__ecs:at", move |x: isize, y: isize| -> SteelVal {
        let ids = e9.lock().at(x as i32, y as i32, None);
        let list: Vec<SteelVal> = ids.into_iter().map(|id| SteelVal::IntV(id as isize)).collect();
        SteelVal::ListV(list.into())
    });

    let e10 = Arc::clone(&ecs);
    engine.register_fn(
        "__ecs:at-layer",
        move |x: isize, y: isize, layer: isize| -> SteelVal {
            let ids = e10.lock().at(x as i32, y as i32, Some(layer as u8));
            let list: Vec<SteelVal> = ids.into_iter().map(|id| SteelVal::IntV(id as isize)).collect();
            SteelVal::ListV(list.into())
        },
    );

    // ------------------------------------------------------------------------
    // 2. Dynamic Properties & Tags
    // ------------------------------------------------------------------------
    let e11 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:set",
        move |eid: SteelVal, key: SteelVal, val: SteelVal| -> bool {
            match val_to_eid(&eid) {
                Some(id) => {
                    let k = steel_to_string(&key);
                    let json = steel_val_to_json(&val)
                        .unwrap_or_else(|_| serde_json::Value::String(val.to_string()));
                    e11.lock().set_prop(id, &k, json)
                }
                None => false,
            }
        },
    );

    let e12 = Arc::clone(&ecs);
    engine.register_fn("__ecs:get", move |eid: SteelVal, key: SteelVal| -> SteelVal {
        match val_to_eid(&eid) {
            Some(id) => {
                let k = steel_to_string(&key);
                match e12.lock().get_prop(id, &k) {
                    Some(json) => json_to_steel_val(&json),
                    None => SteelVal::BoolV(false),
                }
            }
            None => SteelVal::BoolV(false),
        }
    });

    let e13 = Arc::clone(&ecs);
    engine.register_fn(
        "__ecs:get-default",
        move |eid: SteelVal, key: SteelVal, default: SteelVal| -> SteelVal {
            match val_to_eid(&eid) {
                Some(id) => {
                    let k = steel_to_string(&key);
                    match e13.lock().get_prop(id, &k) {
                        Some(json) => json_to_steel_val(&json),
                        None => default,
                    }
                }
                None => default,
            }
        },
    );

    let e14 = Arc::clone(&ecs);
    engine.register_fn("ecs:has?", move |eid: SteelVal, key: SteelVal| -> bool {
        match val_to_eid(&eid) {
            Some(id) => {
                let k = steel_to_string(&key);
                e14.lock().has_prop(id, &k)
            }
            None => false,
        }
    });

    let e15 = Arc::clone(&ecs);
    engine.register_fn("ecs:add-tag", move |eid: SteelVal, tag: SteelVal| -> bool {
        match val_to_eid(&eid) {
            Some(id) => {
                let t = steel_to_string(&tag);
                e15.lock().add_tag(id, &t)
            }
            None => false,
        }
    });

    let e16 = Arc::clone(&ecs);
    engine.register_fn("ecs:remove-tag", move |eid: SteelVal, tag: SteelVal| -> bool {
        match val_to_eid(&eid) {
            Some(id) => {
                let t = steel_to_string(&tag);
                e16.lock().remove_tag(id, &t)
            }
            None => false,
        }
    });

    let e17 = Arc::clone(&ecs);
    engine.register_fn("ecs:has-tag?", move |eid: SteelVal, tag: SteelVal| -> bool {
        match val_to_eid(&eid) {
            Some(id) => {
                let t = steel_to_string(&tag);
                e17.lock().has_tag(id, &t)
            }
            None => false,
        }
    });

    // ------------------------------------------------------------------------
    // 3. Entity Relations (Pairs & Hierarchies)
    // ------------------------------------------------------------------------
    let e18 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:relate",
        move |subject: SteelVal, relation: SteelVal, target: SteelVal| -> bool {
            match (val_to_eid(&subject), val_to_eid(&target)) {
                (Some(s), Some(t)) => {
                    let rel = steel_to_string(&relation);
                    e18.lock().relate(s, &rel, t)
                }
                _ => false,
            }
        },
    );

    let e19 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:unrelate",
        move |subject: SteelVal, relation: SteelVal, target: SteelVal| -> bool {
            match (val_to_eid(&subject), val_to_eid(&target)) {
                (Some(s), Some(t)) => {
                    let rel = steel_to_string(&relation);
                    e19.lock().unrelate(s, &rel, t)
                }
                _ => false,
            }
        },
    );

    let e20 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:target",
        move |subject: SteelVal, relation: SteelVal| -> SteelVal {
            match val_to_eid(&subject) {
                Some(s) => {
                    let rel = steel_to_string(&relation);
                    match e20.lock().target(s, &rel) {
                        Some(t) => SteelVal::IntV(t as isize),
                        None => SteelVal::BoolV(false),
                    }
                }
                None => SteelVal::BoolV(false),
            }
        },
    );

    let e21 = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:children",
        move |target: SteelVal, relation: SteelVal| -> SteelVal {
            match val_to_eid(&target) {
                Some(t) => {
                    let rel = steel_to_string(&relation);
                    let children = e21.lock().children(t, &rel);
                    let list: Vec<SteelVal> =
                        children.into_iter().map(|id| SteelVal::IntV(id as isize)).collect();
                    SteelVal::ListV(list.into())
                }
                None => SteelVal::ListV(vec![].into()),
            }
        },
    );

    // ------------------------------------------------------------------------
    // 4. Flecs C Query DSL
    // ------------------------------------------------------------------------
    let e22 = Arc::clone(&ecs);
    engine.register_fn("ecs:query", move |expr: SteelVal| -> SteelVal {
        let expr_str = steel_to_string(&expr);
        match e22.lock().query_expr(&expr_str) {
            Ok(ids) => {
                let list: Vec<SteelVal> =
                    ids.into_iter().map(|id| SteelVal::IntV(id as isize)).collect();
                SteelVal::ListV(list.into())
            }
            Err(e) => {
                tracing::warn!("ecs:query error for expr '{}': {}", expr_str, e);
                SteelVal::ListV(vec![].into())
            }
        }
    });

    // ------------------------------------------------------------------------
    // 5. Native Viewport Rasterizer primitive & Helpers
    // ------------------------------------------------------------------------
    let e23 = Arc::clone(&ecs);
    let rast23 = Arc::clone(&rasterizer);
    engine.register_fn(
        "ecs:render-viewport",
        move |cx: isize, cy: isize, cols: isize, rows: isize, is_dw: bool| -> String {
            rast23.lock().render(
                &e23.lock(),
                cx as i32,
                cy as i32,
                cols as u16,
                rows as u16,
                is_dw,
            ).to_string()
        },
    );

    let e24 = Arc::clone(&ecs);
    engine.register_fn("ecs:count", move || -> isize {
        e24.lock().spatial.len() as isize
    });

    let e_clear = Arc::clone(&ecs);
    engine.register_fn("ecs:clear!", move || -> bool {
        // The world-state singleton is NOT a game entity: clearing the
        // ECS world (e.g. the roguelike's `world:clear!` on init) must
        // not destroy the model's causal state, which used to live in
        // the separate *world-state* hash and survived tile clearing.
        // Capture its props, clear, and restore them on the recreated
        // singleton. (Snapshot import does NOT go through here — it
        // uses `EcsWorld::clear` directly and restores the singleton
        // from the snapshot's entity list.)
        let mut guard = e_clear.lock();
        let saved = {
            let id = guard.spawn_named(WORLD_STATE_ENTITY_NAME);
            guard
                .inspect_entity(id)
                .and_then(|v| v["dynamic_props"].as_object().cloned())
        };
        guard.clear();
        if let Some(map) = saved {
            let obj = serde_json::Value::Object(map);
            set_world_state_json(&mut guard, &obj);
        }
        true
    });

    // ------------------------------------------------------------------------
    // 5b. World-state facade (`state:*`): the ONLY way model scripts
    // read/write model causal state (snapshot v3). All operations go
    // to the DynamicProps of the host-managed `WorldState` singleton
    // entity, so state automatically rides along with ECS snapshots.
    // Value conversion is the standard steel<->JSON interop used by
    // `ecs:set`/`ecs:get`: ints stay ints, floats stay floats (0.0
    // round-trips as a float), bools/strings/nested hashes/lists are
    // preserved; chars and symbols degrade to strings (as with
    // `ecs:set`), and closures cannot be stored at all — causal state
    // must be pure data, exactly as *world-state* required.
    // ------------------------------------------------------------------------
    let s1 = Arc::clone(&ecs);
    engine.register_fn("__state:get", move |key: SteelVal| -> SteelVal {
        let k = steel_to_string(&key);
        let mut guard = s1.lock();
        let id = ensure_world_state_entity(&mut guard);
        match ws_get(&guard, id, &k) {
            Some(json) => json_to_steel_val(&json),
            None => SteelVal::BoolV(false),
        }
    });

    let s2 = Arc::clone(&ecs);
    engine.register_fn(
        "__state:get-default",
        move |key: SteelVal, default: SteelVal| -> SteelVal {
            let k = steel_to_string(&key);
            let mut guard = s2.lock();
            let id = ensure_world_state_entity(&mut guard);
            match ws_get(&guard, id, &k) {
                Some(json) => json_to_steel_val(&json),
                None => default,
            }
        },
    );

    let s3 = Arc::clone(&ecs);
    engine.register_fn(
        "state:set!",
        move |key: SteelVal, val: SteelVal| -> bool {
            let k = steel_to_string(&key);
            let json = match steel_val_to_json(&val) {
                Ok(j) => j,
                Err(_) => return false,
            };
            let mut guard = s3.lock();
            let id = ensure_world_state_entity(&mut guard);
            ws_set(&mut guard, id, &k, json)
        },
    );

    let s4 = Arc::clone(&ecs);
    engine.register_fn("state:has?", move |key: SteelVal| -> bool {
        let k = steel_to_string(&key);
        let mut guard = s4.lock();
        let id = ensure_world_state_entity(&mut guard);
        ws_has(&guard, id, &k)
    });

    let s5 = Arc::clone(&ecs);
    engine.register_fn("state:remove!", move |key: SteelVal| -> bool {
        let k = steel_to_string(&key);
        let mut guard = s5.lock();
        let id = ensure_world_state_entity(&mut guard);
        ws_remove(&mut guard, id, &k)
    });

    let s6 = Arc::clone(&ecs);
    engine.register_fn("state:all", move || -> SteelVal {
        let mut guard = s6.lock();
        json_to_steel_val(&world_state_json(&mut guard))
    });

    let s7 = Arc::clone(&ecs);
    engine.register_fn("state:replace!", move |data: SteelVal| -> bool {
        let json = match steel_val_to_json(&data) {
            Ok(j) => j,
            Err(_) => return false,
        };
        let mut guard = s7.lock();
        set_world_state_json(&mut guard, &json)
    });

    let s8 = Arc::clone(&ecs);
    engine.register_fn("state:seed-defaults!", move |data: SteelVal| -> bool {
        // Fill in only MISSING keys from the given hash — used by a
        // model's prelude at load time to declare its initial state
        // without wiping live state on hot reload.
        let json = match steel_val_to_json(&data) {
            Ok(j) => j,
            Err(_) => return false,
        };
        let Some(map) = json.as_object() else {
            return false;
        };
        let mut guard = s8.lock();
        let id = ensure_world_state_entity(&mut guard);
        for (k, v) in map {
            if !ws_has(&guard, id, k) {
                ws_set(&mut guard, id, k, v.clone());
            }
        }
        true
    });

    let s_keys = Arc::clone(&ecs);
    engine.register_fn("state:keys", move || -> SteelVal {
        let mut guard = s_keys.lock();
        let all = world_state_json(&mut guard);
        let keys: Vec<SteelVal> = match all.as_object() {
            Some(map) => map
                .keys()
                .map(|k| SteelVal::StringV(k.as_str().into()))
                .collect(),
            None => Vec::new(),
        };
        SteelVal::ListV(keys.into())
    });

    let s9 = Arc::clone(&ecs);
    engine.register_fn("state:entity-id", move || -> isize {
        let mut guard = s9.lock();
        ensure_world_state_entity(&mut guard) as isize
    });

    let e_inspect = Arc::clone(&ecs);
    engine.register_fn("ecs:inspect", move |eid: SteelVal| -> SteelVal {
        let eid_num = match eid {
            SteelVal::IntV(n) => n as u64,
            SteelVal::NumV(f) => f as u64,
            _ => return SteelVal::BoolV(false),
        };
        match e_inspect.lock().inspect_entity(eid_num) {
            Some(json) => json_to_steel_val(&json),
            None => SteelVal::BoolV(false),
        }
    });

    // Snapshot v2 (guide item 2): full ECS world export / import.
    let e_export = Arc::clone(&ecs);
    engine.register_fn("ecs:export-world", move || -> SteelVal {
        let json = e_export.lock().export_world_json();
        json_to_steel_val(&json)
    });

    let e_import = Arc::clone(&ecs);
    engine.register_fn("ecs:import-world", move |data: SteelVal| -> SteelVal {
        let json = match steel_val_to_json(&data) {
            Ok(j) => j,
            Err(_) => return SteelVal::BoolV(false),
        };
        match e_import.lock().import_world_json(&json) {
            Ok(id_map) => {
                let mut map = std::collections::HashMap::new();
                for (old, new) in id_map {
                    map.insert(old.to_string(), serde_json::json!(new));
                }
                json_to_steel_val(&serde_json::Value::Object(map.into_iter().collect()))
            }
            Err(_) => SteelVal::BoolV(false),
        }
    });

    let e_sess = Arc::clone(&ecs);
    engine.register_fn(
        "__ecs:set-player-session",
        move |eid: SteelVal, pid: SteelVal, cols: isize, rows: isize| -> bool {
            let eid_num = match eid {
                SteelVal::IntV(n) => n as u64,
                SteelVal::NumV(f) => f as u64,
                _ => return false,
            };
            let pid_num = match pid {
                SteelVal::IntV(n) => n as u64,
                SteelVal::NumV(f) => f as u64,
                _ => return false,
            };
            let mut guard = e_sess.lock();
            if !guard.exists(eid_num) {
                return false;
            }
            guard
                .world
                .entity_from_id(eid_num)
                .set(crate::ecs::components::PlayerSession::new(
                    pid_num,
                    cols as u16,
                    rows as u16,
                ));
            guard.add_tag(eid_num, "Player");
            true
        },
    );

    // ------------------------------------------------------------------------
    // 6. Roguelike Universal Mathematical Operators (FOV, A*, Dijkstra, Noise)
    // ------------------------------------------------------------------------
    let e_fov = Arc::clone(&ecs);
    engine.register_fn(
        "ecs:fov",
        move |cx: isize, cy: isize, radius: isize| -> SteelVal {
            let vis = e_fov.lock().fov(cx as i32, cy as i32, radius as i32);
            let coords: Vec<SteelVal> = vis
                .into_iter()
                .map(|(x, y)| {
                    SteelVal::ListV(vec![SteelVal::IntV(x as isize), SteelVal::IntV(y as isize)].into())
                })
                .collect();
            SteelVal::ListV(coords.into())
        },
    );

    let e_path = Arc::clone(&ecs);
    engine.register_fn(
        "__ecs:find-path",
        move |sx: isize, sy: isize, gx: isize, gy: isize, allow_diag: bool| -> SteelVal {
            match e_path.lock().find_path((sx as i32, sy as i32), (gx as i32, gy as i32), allow_diag) {
                Some(path) => {
                    let steps: Vec<SteelVal> = path
                        .into_iter()
                        .map(|(x, y)| {
                            SteelVal::ListV(vec![SteelVal::IntV(x as isize), SteelVal::IntV(y as isize)].into())
                        })
                        .collect();
                    SteelVal::ListV(steps.into())
                }
                None => SteelVal::BoolV(false),
            }
        },
    );

    let e_dijkstra = Arc::clone(&ecs);
    engine.register_fn(
        "__ecs:dijkstra-map",
        move |tx: isize, ty: isize, max_dist: isize, allow_diag: bool| -> SteelVal {
            let map = e_dijkstra
                .lock()
                .dijkstra_map((tx as i32, ty as i32), max_dist.max(0) as u32, allow_diag);
            let mut entries = steel::HashMap::new();
            for ((x, y), dist) in map {
                let key = format!("{},{}", x, y);
                entries.insert(SteelVal::StringV(key.into()), SteelVal::IntV(dist as isize));
            }
            SteelVal::HashMapV(Gc::new(entries).into())
        },
    );

    engine.register_fn(
        "ecs:noise-2d",
        move |seed: isize, x: f64, y: f64, scale: f64| -> f64 {
            EcsWorld::noise_2d(seed as u64, x, y, scale)
        },
    );

    engine.register_fn(
        "ecs:generate-cave",
        move |width: isize, height: isize, wall_prob: f64, iterations: isize, seed: isize| -> SteelVal {
            let cave = EcsWorld::generate_cave(
                width as usize,
                height as usize,
                wall_prob,
                iterations as usize,
                seed as u64,
            );
            let rows: Vec<SteelVal> = cave
                .into_iter()
                .map(|r| {
                    let cols: Vec<SteelVal> = r.into_iter().map(SteelVal::BoolV).collect();
                    SteelVal::ListV(cols.into())
                })
                .collect();
            SteelVal::ListV(rows.into())
        },
    );

    // ------------------------------------------------------------------------
    // 7. Scheme Case-Lambda Variadic DSL Bridges
    // ------------------------------------------------------------------------
    let bridge_code = r#"
    (define ecs:spawn
      (case-lambda
        (() (__ecs:spawn))
        ((name) (__ecs:spawn-named (if (symbol? name) (symbol->string name) (to-string name))))))

    (define (ecs:spawn-named name)
      (__ecs:spawn-named (if (symbol? name) (symbol->string name) (to-string name))))

    (define ecs:get
      (case-lambda
        ((eid key) (__ecs:get eid key))
        ((eid key default) (__ecs:get-default eid key default))))

    (define state:get
      (case-lambda
        ((key) (__state:get key))
        ((key default) (__state:get-default key default))))

    (define ecs:at
      (case-lambda
        ((x y) (__ecs:at x y))
        ((x y layer) (__ecs:at-layer x y layer))))

    (define ecs:find-path
      (case-lambda
        ((sx sy gx gy) (__ecs:find-path sx sy gx gy #f))
        ((sx sy gx gy allow-diag) (__ecs:find-path sx sy gx gy allow-diag))))

    (define ecs:dijkstra-map
      (case-lambda
        ((tx ty max-dist) (__ecs:dijkstra-map tx ty max-dist #f))
        ((tx ty max-dist allow-diag) (__ecs:dijkstra-map tx ty max-dist allow-diag))))
    "#;
    engine
        .run(bridge_code.to_string())
        .map_err(|e| format!("Failed to register Scheme ecs bridges: {}", e))?;

    Ok(())
}
