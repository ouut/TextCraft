//! Guide item 2 acceptance (revised by the scope ruling): snapshot v2
//! rollback is validated with GENERIC ECS content — entities carrying
//! props / tags / relationships built through the raw `ecs:*` primitives —
//! not with roguelike gameplay scenarios. The roguelike package under
//! models/roguelike serves only as the loading fixture (it provides the
//! prelude with the `ecs:*` DSL).
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

fn find_entity<'a>(snap: &'a serde_json::Value, pred: impl Fn(&serde_json::Value) -> bool) -> Option<&'a serde_json::Value> {
    snap["ecs_world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| pred(e))
}

fn part_by_kind<'a>(snap: &'a serde_json::Value, kind: &str) -> Option<&'a serde_json::Value> {
    find_entity(snap, |e| e["dynamic_props"]["kind"] == kind)
}

#[test]
fn test_snapshot_v2_generic_rollback() {
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic.on_player_join(1, "token_1").unwrap();

    // Generic graph: hub <-PartOf- arm (intact), hub <-PartOf- leg (intact),
    // built purely from ecs:* primitives.
    logic
        .eval_repl(
            r#"(begin
                 (define hub (ecs:spawn-named "Hub"))
                 (ecs:set-pos hub 5 5 LAYER-CREATURE)
                 (ecs:set-glyph hub "H" '(200 200 200) #f #t #f)
                 (ecs:set hub "kind" "hub")
                 (define arm (ecs:spawn-named "ArmPart"))
                 (ecs:set arm "kind" "arm")
                 (ecs:set arm "severed" #f)
                 (ecs:add-tag arm "Part")
                 (ecs:relate arm "PartOf" hub)
                 (define leg (ecs:spawn-named "LegPart"))
                 (ecs:set leg "kind" "leg")
                 (ecs:set leg "severed" #f)
                 (ecs:add-tag leg "Part")
                 (ecs:relate leg "PartOf" hub)
                 #t)"#,
        )
        .unwrap();

    // Pre-checkpoint mutation that must SURVIVE the rollback: sever arm.
    logic.eval_repl(r#"(ecs:set arm "severed" #t)"#).unwrap();
    let player_x_before = logic.export_state().unwrap()["players"]["1"]["x"].clone();

    // ---- Host checkpoint ---------------------------------------------------
    let snap = logic.export_snapshot_v2().unwrap();
    assert_eq!(snap["format"], "textcraft-snapshot");
    assert_eq!(snap["version"], 2);
    assert_eq!(part_by_kind(&snap, "arm").unwrap()["dynamic_props"]["severed"], true);
    let leg = part_by_kind(&snap, "leg").expect("leg in snapshot");
    assert_eq!(leg["dynamic_props"]["severed"], false);
    let leg_id = leg["id"].as_u64().unwrap();
    let hub_id = part_by_kind(&snap, "hub").unwrap()["id"].as_u64().unwrap();
    assert!(
        leg["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["relation"] == "PartOf" && r["target_id"] == hub_id),
        "leg PartOf hub in snapshot: {}",
        leg
    );
    assert!(leg["tags"].as_array().unwrap().iter().any(|t| t == "Part"));

    // ---- Mutate: sever leg, spawn a stray entity, move the player --------
    logic.eval_repl(r#"(ecs:set leg "severed" #t)"#).unwrap();
    logic
        .eval_repl(
            r#"(begin (define stray (ecs:spawn-named "Stray")) (ecs:set stray "kind" "stray") #t)"#,
        )
        .unwrap();
    let _ = logic.on_input(
        1,
        &textcraft::PlayerInput::Key {
            code: "KeyD".to_string(),
            action: Default::default(),
            shift: false,
            ctrl: false,
            alt: false,
        },
    );
    let mutated = logic.export_snapshot_v2().unwrap();
    assert_eq!(part_by_kind(&mutated, "leg").unwrap()["dynamic_props"]["severed"], true);
    assert!(part_by_kind(&mutated, "stray").is_some());

    // ---- Roll back ---------------------------------------------------------
    let id_map = logic.import_snapshot_v2(&snap).unwrap();
    assert!(
        id_map.iter().all(|(a, b)| a == b),
        "entity ids preserved on import: {:?}",
        id_map
    );

    let restored = logic.export_snapshot_v2().unwrap();
    let leg2 = part_by_kind(&restored, "leg").expect("leg after restore");
    assert_eq!(leg2["id"].as_u64().unwrap(), leg_id);
    assert_eq!(leg2["dynamic_props"]["severed"], false, "post-checkpoint sever undone");
    assert!(
        leg2["relationships"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["relation"] == "PartOf" && r["target_id"] == hub_id),
        "leg PartOf relation restored: {}",
        leg2
    );
    assert_eq!(
        part_by_kind(&restored, "arm").unwrap()["dynamic_props"]["severed"],
        true,
        "pre-checkpoint sever stays severed"
    );
    assert!(part_by_kind(&restored, "stray").is_none(), "stray entity gone after rollback");
    assert_eq!(
        logic.export_state().unwrap()["players"]["1"]["x"],
        player_x_before,
        "player coordinate restored"
    );
    assert_eq!(restored, snap, "re-export after restore equals snapshot");
}

#[test]
fn test_snapshot_v2_rejects_v1_shape() {
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();
    // A bare *world-state* (v1 checkpoint shape) has no version field.
    let v1 = logic.export_state().unwrap();
    let err = logic.import_snapshot_v2(&v1).unwrap_err();
    assert!(err.contains("version"), "explicit version error, got: {}", err);
}

#[test]
fn test_host_checkpoint_roundtrip_via_logic_api() {
    let mut logic = SchemeLogic::new(get_scripts_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic
        .eval_repl(r#"(begin (define e1 (ecs:spawn-named "CpEnt")) (ecs:set e1 "kind" "cp") (ecs:set e1 "n" 1) #t)"#)
        .unwrap();
    logic.create_checkpoint("cp1").unwrap();
    logic.eval_repl(r#"(ecs:set e1 "n" 99)"#).unwrap();
    assert!(logic.rollback_checkpoint("cp1").unwrap());
    let snap = logic.export_snapshot_v2().unwrap();
    let ent = find_entity(&snap, |e| e["dynamic_props"]["kind"] == "cp").unwrap();
    assert_eq!(ent["dynamic_props"]["n"], 1);
    assert!(!logic.rollback_checkpoint("missing_tag").unwrap());
}
