//! Guide item 4 acceptance: run_epoch determinism, sensitivity to model
//! changes, host-side assertion reporting. (The hard-coded hash regression
//! lives here as well; see EXPECTED_FIXED_EPOCH_HASH.)
use std::net::SocketAddr;
use std::path::PathBuf;
use game_server::ServerConfig;
use serde_json::json;
use textcraft::{SchemeLogic, TextCraftServer, EMBEDDED_WEB_ASSETS};

fn model_dir() -> PathBuf {
    let p = PathBuf::from("models/roguelike");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../models/roguelike")
    }
}

async fn make_server() -> TextCraftServer {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(model_dir(), false, None).unwrap();
    logic.init().unwrap();
    TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap()
}

fn epoch_args() -> serde_json::Value {
    json!({
        "seed": 12345,
        "steps": 20,
        "inputs": [ { "step": 0, "player_id": 1, "type": "key", "code": "KeyD" } ],
        "assertions": [ { "path": "tick_count", "op": "eq", "expected": 20 } ]
    })
}

#[tokio::test]
async fn test_run_epoch_deterministic_and_assertions() {
    let mut server = make_server().await;
    let r1 = server.run_epoch(&epoch_args()).unwrap();
    let r2 = server.run_epoch(&epoch_args()).unwrap();
    assert_eq!(
        serde_json::to_string(&r1).unwrap(),
        serde_json::to_string(&r2).unwrap(),
        "same epoch config must produce a byte-identical report"
    );
    assert_eq!(r1["all_passed"], true, "report: {}", r1);
    assert_eq!(r1["seed"], 12345);
    assert_eq!(r1["steps"], 20);
    assert!(r1["scripts_hash"].as_str().unwrap().len() == 64);
    assert!(r1["assertions_hash"].as_str().unwrap().len() == 64);
    assert!(r1["final_state_hash"].as_str().unwrap().len() == 64);
    // Host fingerprint (task 5): compile-time constants only, so the
    // byte-identical report assertion above already proves they carry
    // no timestamps or other per-run values.
    assert_eq!(r1["host"]["crate_version"], "5.0.0");
    assert!(
        r1["host"]["flecs_ecs_rev"].as_str().unwrap().starts_with("bf956db"),
        "host fingerprint must pin the flecs_ecs rev: {}", r1["host"]
    );
    assert_eq!(r1["host"]["steel_core_version"], "0.8.3");
    // dt provenance (task 6): epochs always step with a fixed dt.
    assert_eq!(r1["dt"]["source"], "fixed");
    assert!((r1["dt"]["value"].as_f64().unwrap() - 0.016).abs() < 0.001);
    server.handle.shutdown();
}

#[tokio::test]
async fn test_run_epoch_assertion_failure_reported() {
    let mut server = make_server().await;
    let mut args = epoch_args();
    args["assertions"] = json!([ { "path": "tick_count", "op": "eq", "expected": 999 } ]);
    let r = server.run_epoch(&args).unwrap();
    assert_eq!(r["all_passed"], false);
    assert_eq!(r["results"][0]["passed"], false);
    assert_eq!(r["results"][0]["expected"], 999);
    assert_eq!(r["results"][0]["actual"], 20);
    assert_eq!(r["results"][0]["path"], "tick_count");
    server.handle.shutdown();
}

#[tokio::test]
async fn test_run_epoch_hash_changes_with_model_constant() {
    let mut server = make_server().await;
    let base = server.run_epoch(&epoch_args()).unwrap();

    // Copy the model to a temp dir and change one numeric constant
    // (player default gold 50 -> 52 in prelude.scm).
    let tmp = std::env::temp_dir().join(format!("tc_epoch_model_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    for entry in std::fs::read_dir(model_dir()).unwrap() {
        let p = entry.unwrap().path();
        if p.is_file() {
            std::fs::copy(&p, tmp.join(p.file_name().unwrap())).unwrap();
        }
    }
    let prelude = tmp.join("prelude.scm");
    let code = std::fs::read_to_string(&prelude).unwrap();
    assert!(code.contains("\"gold\" 50"), "fixture assumption: default gold 50");
    std::fs::write(&prelude, code.replacen("\"gold\" 50", "\"gold\" 52", 1)).unwrap();

    let mut args = epoch_args();
    args["scripts_dir"] = json!(tmp.to_str().unwrap());
    let changed = server.run_epoch(&args).unwrap();
    assert_ne!(
        changed["final_state_hash"], base["final_state_hash"],
        "changing a model constant must change the final state hash"
    );
    assert_ne!(changed["scripts_hash"], base["scripts_hash"]);
    let _ = std::fs::remove_dir_all(&tmp);
    server.handle.shutdown();
}

/// Fixed-epoch regression: seed/inputs/assertions and the expected final
/// state hash are all pinned. If a host change alters epoch semantics,
/// this fails loudly instead of silently drifting.
///
/// Updated 2026-11-10 (snapshot v3 / single-store migration): the hash
/// changed from f91ceb60… to 28e8df35… purely because the storage
/// LAYOUT changed — model state moved from the snapshot's separate
/// `world_state` section into the ecs_world section (WorldState
/// singleton entity), and the snapshot version field went 2 -> 3.
/// Epoch BEHAVIOR is unchanged (same seed/inputs produce the same
/// tick_count, balances and assertion results; the deterministic test
/// above produces byte-identical reports across runs, and the new
/// hash was observed identical in repeated independent runs).
const EXPECTED_FIXED_EPOCH_HASH: &str = "28e8df35ce9950128beb046a2be4ca48eb32cb777def2285791b858b414c44ce";

#[tokio::test]
async fn test_fixed_epoch_regression_hash() {
    let mut server = make_server().await;
    let r = server.run_epoch(&epoch_args()).unwrap();
    assert_eq!(r["all_passed"], true);
    assert_eq!(
        r["final_state_hash"].as_str().unwrap(),
        EXPECTED_FIXED_EPOCH_HASH,
        "fixed epoch hash drifted"
    );
    server.handle.shutdown();
}
