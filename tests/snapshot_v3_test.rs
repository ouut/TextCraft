//! Snapshot v3 acceptance (storage-group task, round 2): the Flecs
//! world is the ONLY authoritative store. Model causal state lives in
//! the host-managed `WorldState` singleton entity's DynamicProps and
//! is reached from Scheme exclusively through the `state:*` facade.
//! Snapshot v3 top level = ecs_world + prng + tick; no `world_state`.
use std::path::PathBuf;
use textcraft::SchemeLogic;

fn get_scripts_dir() -> PathBuf {
    let p = PathBuf::from("models/roguelike");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../models/roguelike")
    }
}

fn world_state_entity(snap: &serde_json::Value) -> serde_json::Value {
    snap["ecs_world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "WorldState")
        .expect("WorldState singleton entity must be in the snapshot")
        .clone()
}

#[test]
fn test_facade_only_mutation_snapshot_clear_restore_hash_equal() {
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();

    // Mutate state ONLY through the facade primitives (no model code,
    // no *world-state* — it no longer exists). Include the fidelity
    // traps: an int, a float that starts at 0.0, a nested hash, a
    // list, a bool false, and a string.
    logic
        .eval_repl(
            r#"(begin
                 (state:set! "tick_count" 777)
                 (state:set! "probe"
                   (hash "n" 42
                         "f" 0.0
                         "flag" #f
                         "name" "goblin"
                         "lst" (list 1 2.5 #t "s")
                         "nested" (hash "inner" (list (hash "hp" 30)))))
                 (state:set! "elapsed_sec" 1.5)
                 #t)"#,
        )
        .unwrap();

    // Type fidelity read back through the facade -> host JSON.
    let st = logic.world_state_json();
    assert_eq!(st["tick_count"], 777);
    assert!(st["tick_count"].is_i64() || st["tick_count"].is_u64());
    assert_eq!(st["probe"]["n"], 42);
    assert!(
        st["probe"]["f"].is_f64(),
        "0.0 must stay a float, got: {}",
        st["probe"]["f"]
    );
    assert_eq!(st["probe"]["f"], 0.0);
    assert_eq!(st["probe"]["flag"], false);
    assert_eq!(st["probe"]["lst"][1], 2.5);
    assert_eq!(st["probe"]["nested"]["inner"][0]["hp"], 30);
    assert_eq!(st["elapsed_sec"], 1.5);
    // Scheme-side: a stored #f is present (has?) even though it reads #f.
    let has = logic.eval_repl(r#"(state:has? "probe")"#).unwrap();
    assert!(has.contains("true") || has.contains("#t"), "has? -> {}", has);

    let hash_before = logic.state_hash().unwrap();
    let snap = logic.export_snapshot().unwrap();

    // v3 shape: version 3, NO world_state section, tick from singleton.
    assert_eq!(snap["format"], "textcraft-snapshot");
    assert_eq!(snap["version"], 3);
    assert!(
        snap.get("world_state").is_none(),
        "snapshot v3 top level must not contain world_state: {}",
        snap
    );
    assert_eq!(snap["tick"], 777);
    // The state is inside ecs_world, on the singleton entity.
    let ws = world_state_entity(&snap);
    assert_eq!(ws["dynamic_props"]["tick_count"], 777);
    assert_eq!(ws["dynamic_props"]["probe"]["nested"]["inner"][0]["hp"], 30);

    // Mutate + physically clear the whole ECS world (singleton gone).
    logic
        .eval_repl(r#"(begin (state:set! "tick_count" 1) (state:remove! "probe") #t)"#)
        .unwrap();
    logic.ecs_world().lock().clear();
    assert_ne!(logic.state_hash().unwrap(), hash_before);

    // Restore: hash must converge exactly.
    logic.import_snapshot(&snap).unwrap();
    assert_eq!(logic.state_hash().unwrap(), hash_before);
    let restored = logic.export_snapshot().unwrap();
    assert_eq!(restored, snap, "re-export after restore equals snapshot");
    assert_eq!(logic.world_state_json()["probe"]["f"], 0.0);
    assert!(logic.world_state_json()["probe"]["f"].is_f64());
}

#[test]
fn test_v2_snapshot_compat_import_pours_world_state_into_singleton() {
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic
        .eval_repl(r#"(begin (state:set! "tick_count" 55) (state:set! "probe" (hash "x" 1)) #t)"#)
        .unwrap();
    let state_before = logic.export_state().unwrap();
    let v3 = logic.export_snapshot().unwrap();

    // Hand-build the equivalent v2 snapshot: world_state section holds
    // the state; ecs_world holds every entity EXCEPT the singleton.
    let mut entities: Vec<serde_json::Value> = v3["ecs_world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["name"] != "WorldState")
        .cloned()
        .collect();
    let _ = &mut entities;
    let v2 = serde_json::json!({
        "format": "textcraft-snapshot",
        "version": 2,
        "model": v3["model"],
        "tick": v3["tick"],
        "prng": v3["prng"],
        "world_state": state_before,
        "ecs_world": { "entities": entities },
    });

    // Diverge, then import the v2 snapshot.
    logic
        .eval_repl(r#"(begin (state:set! "tick_count" 999) (state:remove! "probe") #t)"#)
        .unwrap();
    logic.import_snapshot(&v2).unwrap();

    assert_eq!(
        logic.export_state().unwrap(),
        state_before,
        "v2 world_state must land in the ECS singleton verbatim"
    );
    // And re-export is v3 with no world_state section.
    let re = logic.export_snapshot().unwrap();
    assert_eq!(re["version"], 3);
    assert!(re.get("world_state").is_none());
    assert_eq!(re["tick"], 55);
}

#[test]
fn test_legacy_monolithic_chunks_key_migrates_to_per_chunk_keys() {
    // A pre-v3 state could carry the whole tile map under one "chunks"
    // key. On first tile touch the prelude must split it into
    // "chunk:<cx>,<cy>" keys and drop the legacy key, losslessly.
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic
        .eval_repl(
            r##"(begin
                 (state:set! "chunks"
                   (hash "0,0" (hash "tiles"
                     (hash "3,4,terrain" (hash "glyph" "#" "name" "legacy_wall")))))
                 #t)"##,
        )
        .unwrap();
    let tile = logic
        .eval_repl(r#"(let ((t (world:get-tile 3 4 'terrain))) (and t (map-get t "name" #f)))"#)
        .unwrap();
    assert!(tile.contains("legacy_wall"), "legacy tile readable: {}", tile);
    let st = logic.world_state_json();
    assert!(st.get("chunks").is_none(), "legacy key removed: {}", st);
    assert_eq!(
        st["chunk:0,0"]["tiles"]["3,4,terrain"]["name"], "legacy_wall",
        "tile landed under its per-chunk key"
    );
}

#[test]
fn test_ecs_clear_preserves_world_state_singleton() {
    // The roguelike's world:clear! calls ecs:clear! during on-init;
    // causal state must survive it (it lived outside ECS pre-v3).
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic
        .eval_repl(r#"(begin (state:set! "probe" (hash "x" 9)) (ecs:clear!) #t)"#)
        .unwrap();
    assert_eq!(logic.world_state_json()["probe"]["x"], 9);
    assert!(logic.world_state_json()["tick_count"].is_number());
}
