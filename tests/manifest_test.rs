//! Guide item 7 acceptance: model manifest drives snapshot validation;
//! mismatched snapshots fail explicitly; manifest-less models still run.
use std::path::PathBuf;
use textcraft::SchemeLogic;

fn roguelike_dir() -> PathBuf {
    let p = PathBuf::from("models/roguelike");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../models/roguelike")
    }
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/counter_economy")
}

#[test]
fn test_manifest_loaded_and_snapshot_model_recorded() {
    let mut logic = SchemeLogic::new(roguelike_dir(), false, None).unwrap();
    logic.init().unwrap();
    assert_eq!(logic.model_manifest.name, "roguelike");
    assert_eq!(logic.model_manifest.state_schema, 1);
    let snap = logic.export_snapshot_v2().unwrap();
    assert_eq!(snap["model"]["name"], "roguelike");
    assert_eq!(snap["model"]["state_schema"], 1);
    assert!(snap["model"]["scripts_hash"].as_str().unwrap().len() == 64);
    // Matching snapshot imports fine.
    logic.import_snapshot_v2(&snap).unwrap();
}

#[test]
fn test_snapshot_rejects_name_schema_version_mismatch() {
    let mut logic = SchemeLogic::new(roguelike_dir(), false, None).unwrap();
    logic.init().unwrap();
    let snap = logic.export_snapshot_v2().unwrap();

    let mut bad_name = snap.clone();
    bad_name["model"]["name"] = serde_json::json!("other_model");
    let err = logic.import_snapshot_v2(&bad_name).unwrap_err();
    assert!(err.contains("model mismatch"), "name mismatch error: {}", err);

    let mut bad_schema = snap.clone();
    bad_schema["model"]["state_schema"] = serde_json::json!(99);
    let err = logic.import_snapshot_v2(&bad_schema).unwrap_err();
    assert!(err.contains("state_schema mismatch"), "schema mismatch error: {}", err);

    let mut bad_version = snap.clone();
    bad_version["version"] = serde_json::json!(99);
    let err = logic.import_snapshot_v2(&bad_version).unwrap_err();
    assert!(err.contains("version"), "version error: {}", err);
}

#[test]
fn test_manifestless_model_still_starts() {
    // Copy the fixture's main.scm into a temp dir WITHOUT model.json.
    let tmp = std::env::temp_dir().join(format!("tc_nomanifest_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::copy(fixture_dir().join("main.scm"), tmp.join("main.scm")).unwrap();

    let mut logic = SchemeLogic::new(&tmp, false, None).unwrap();
    logic.init().unwrap();
    assert_eq!(logic.model_manifest.name, "default");
    assert_eq!(logic.model_manifest.state_schema, 1);
    // It ticks and snapshots with the derived default identity.
    logic.on_tick(0.016, 1).unwrap();
    let snap = logic.export_snapshot_v2().unwrap();
    assert_eq!(snap["model"]["name"], "default");
    assert_eq!(snap["world_state"]["tick_count"], 1);
    logic.import_snapshot_v2(&snap).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
}
