//! Guide item 6 acceptance (promoted to the main verification path by the
//! scope ruling): the non-grid fixture model counter_economy runs a full
//! epoch on the same base/binary as the roguelike example model.
use std::net::SocketAddr;
use std::path::PathBuf;
use game_server::ServerConfig;
use serde_json::json;
use textcraft::{SchemeLogic, TextCraftServer, EMBEDDED_WEB_ASSETS};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/counter_economy")
}

fn roguelike_dir() -> PathBuf {
    let p = PathBuf::from("models/roguelike");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../models/roguelike")
    }
}

async fn make_server(dir: PathBuf) -> TextCraftServer {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(dir, false, None).unwrap();
    logic.init().unwrap();
    TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_counter_economy_epoch() {
    let mut server = make_server(fixture_dir()).await;
    // No assertions passed: the manifest-declared assertions.json is used.
    let args = json!({
        "seed": 1,
        "steps": 10,
        "inputs": [ { "step": 0, "player_id": 1, "type": "key", "code": "KeyT" } ]
    });
    let r1 = server.run_epoch(&args).unwrap();
    assert_eq!(r1["all_passed"], true, "fixture epoch report: {}", r1);
    assert_eq!(r1["results"].as_array().unwrap().len(), 3);

    // Deterministic end state: transfer 10 at step 0, then 10 interest ticks.
    let state = server.logic.export_state().unwrap();
    assert_eq!(state["tick_count"], 10);
    assert_eq!(state["transfers"], 1);
    assert_eq!(state["accounts"]["alice"]["balance"], 1088);
    assert_eq!(state["accounts"]["bob"]["balance"], 560);

    // Re-run: byte-identical report.
    let r2 = server.run_epoch(&args).unwrap();
    assert_eq!(serde_json::to_string(&r1).unwrap(), serde_json::to_string(&r2).unwrap());

    // Inline assertions on exact balances also pass.
    let mut args2 = args.clone();
    args2["assertions"] = json!([
        { "path": "accounts.alice.balance", "op": "eq", "expected": 1088 },
        { "path": "accounts.bob.balance", "op": "eq", "expected": 560 }
    ]);
    let r3 = server.run_epoch(&args2).unwrap();
    assert_eq!(r3["all_passed"], true, "report: {}", r3);
    server.handle.shutdown();
}

#[tokio::test]
async fn test_same_binary_switches_models() {
    // Start on the roguelike example model, switch to the fixture and back
    // through run_epoch's scripts_dir, all on one server/binary.
    let mut server = make_server(roguelike_dir()).await;

    let mut to_fixture = json!({ "seed": 1, "steps": 10, "inputs": [] });
    to_fixture["scripts_dir"] = json!(fixture_dir().to_str().unwrap());
    let rf = server.run_epoch(&to_fixture).unwrap();
    assert_eq!(rf["all_passed"], true, "fixture report after switch: {}", rf);
    assert_eq!(server.logic.export_state().unwrap()["tick_count"], 10);

    let mut back = json!({
        "seed": 12345,
        "steps": 20,
        "inputs": [ { "step": 0, "player_id": 1, "type": "key", "code": "KeyD" } ],
        "assertions": [ { "path": "tick_count", "op": "eq", "expected": 20 } ]
    });
    back["scripts_dir"] = json!(roguelike_dir().canonicalize().unwrap().to_str().unwrap());
    let rb = server.run_epoch(&back).unwrap();
    assert_eq!(rb["all_passed"], true, "roguelike report after switch back: {}", rb);
    server.handle.shutdown();
}
