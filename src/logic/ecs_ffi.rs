use std::sync::Arc;
use parking_lot::Mutex;
use steel::gc::Gc;
use steel::rvals::SteelVal;
use steel::steel_vm::engine::Engine;
use steel::steel_vm::register_fn::RegisterFn;
use game_script::{json_to_steel_val, steel_val_to_json};

use crate::ecs::components::Color;
use crate::ecs::rasterizer::ViewportRasterizer;
use crate::ecs::world::EcsWorld;

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

    // ------------------------------------------------------------------------
    // 6. Scheme Case-Lambda Variadic DSL Bridges
    // ------------------------------------------------------------------------
    let bridge_code = r#"
    (define ecs:spawn
      (case-lambda
        (() (__ecs:spawn))
        ((name) (__ecs:spawn-named (if (symbol? name) (symbol->string name) (to-string name))))))

    (define ecs:get
      (case-lambda
        ((eid key) (__ecs:get eid key))
        ((eid key default) (__ecs:get-default eid key default))))

    (define ecs:at
      (case-lambda
        ((x y) (__ecs:at x y))
        ((x y layer) (__ecs:at-layer x y layer))))
    "#;
    engine
        .run(bridge_code.to_string())
        .map_err(|e| format!("Failed to register Scheme ecs bridges: {}", e))?;

    Ok(())
}
