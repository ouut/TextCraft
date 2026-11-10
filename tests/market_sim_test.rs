//! market_sim acceptance: a genuinely ECS-driven, non-grid model.
//!
//! The fixture under tests/fixtures/models/market_sim keeps ALL causal
//! state in the Flecs world (DynamicProps values, tags, generic
//! relations) — unlike counter_economy, which uses no ECS at all. These
//! tests prove the ECS + non-grid combination end to end:
//!   1. run_epoch with host assertions on exact balances / inventory;
//!   2. checkpoint -> mutate -> rollback -> re-run reproduces the exact
//!      state hash, with relations and tags intact after the rollback;
//!   3. non-grid gate: the model source contains no grid primitive
//!      tokens (guide section 7.2) and no entity carries a position.
use std::net::SocketAddr;
use std::path::PathBuf;
use game_server::ServerConfig;
use serde_json::json;
use textcraft::{PlayerInput, SchemeLogic, TextCraftServer, EMBEDDED_WEB_ASSETS};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/market_sim")
}

async fn make_server() -> TextCraftServer {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(fixture_dir(), false, None).unwrap();
    logic.init().unwrap();
    TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap()
}

fn key_b(step: u64) -> serde_json::Value {
    json!({ "step": step, "player_id": 1, "type": "key", "code": "KeyB" })
}

// ---------------------------------------------------------------------------
// 1. run_epoch: fixed seed + input sequence, exact end-state values.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_market_sim_epoch() {
    let mut server = make_server().await;
    // No assertions passed: the manifest-declared assertions.json is used.
    let args = json!({
        "seed": 7,
        "steps": 10,
        "inputs": [ key_b(2), key_b(5) ]
    });
    let r1 = server.run_epoch(&args).unwrap();
    assert_eq!(r1["all_passed"], true, "market epoch report: {}", r1);
    assert_eq!(r1["results"].as_array().unwrap().len(), 6);

    // Exact end state (seed 7): 10 tick trades + 2 KeyB trades = 12 trades,
    // 18 units sold at price 7 => till 126, fees 12 (1 per trade).
    let state = server.logic.export_state().unwrap();
    assert_eq!(state["tick_count"], 10);
    assert_eq!(state["trades"], 12);
    assert_eq!(state["fees_collected"], 12);
    assert_eq!(state["accounts"]["alice"]["balance"], 946);
    assert_eq!(state["accounts"]["bob"]["balance"], 416);
    assert_eq!(state["merchant"]["till"], 126);
    assert_eq!(state["merchant"]["inventory"], 22);

    // Re-run: byte-identical report (same seed + inputs => same outcome).
    let r2 = server.run_epoch(&args).unwrap();
    assert_eq!(
        serde_json::to_string(&r1).unwrap(),
        serde_json::to_string(&r2).unwrap()
    );

    // Inline host assertions on the two headline numbers also pass.
    let mut args2 = args.clone();
    args2["assertions"] = json!([
        { "path": "accounts.alice.balance", "op": "eq", "expected": 946 },
        { "path": "merchant.inventory", "op": "eq", "expected": 22 }
    ]);
    let r3 = server.run_epoch(&args2).unwrap();
    assert_eq!(r3["all_passed"], true, "report: {}", r3);
    server.handle.shutdown();
}

// ---------------------------------------------------------------------------
// 2. Snapshot -> rollback -> re-run consistency, relations/tags intact.
// ---------------------------------------------------------------------------
fn eval_int(logic: &mut SchemeLogic, expr: &str) -> isize {
    let out = logic.eval_repl(expr).unwrap();
    let clean = out.trim_start_matches("=>").trim();
    let last = clean.split_whitespace().last().unwrap_or(clean);
    last.parse::<isize>()
        .unwrap_or_else(|e| panic!("Failed to parse int from {:?}: {}", out, e))
}

fn eval_bool(logic: &mut SchemeLogic, expr: &str) -> bool {
    let out = logic.eval_repl(expr).unwrap();
    let s = out.trim_start_matches("=>").trim();
    s == "#true" || s == "#t"
}

fn find_by_kind<'a>(snap: &'a serde_json::Value, kind: &str) -> &'a serde_json::Value {
    snap["ecs_world"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["dynamic_props"]["kind"] == kind)
        .unwrap_or_else(|| panic!("entity kind={} in snapshot", kind))
}

#[test]
fn test_market_sim_snapshot_rollback_rerun() {
    let mut logic = SchemeLogic::new(fixture_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic.set_random_seed(Some(7));

    let tick = |logic: &mut SchemeLogic, n: u64, from: u64| {
        for i in 0..n {
            logic.on_tick(0.016, from + i).unwrap();
        }
    };

    // N steps + one input-driven trade, then checkpoint.
    tick(&mut logic, 4, 0);
    logic
        .on_input(
            1,
            &PlayerInput::Key {
                code: "KeyB".to_string(),
                action: Default::default(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        )
        .unwrap();
    let at_checkpoint = logic.export_state().unwrap();
    logic.create_checkpoint("msim").unwrap();

    // M more steps; record hash + values.
    tick(&mut logic, 3, 4);
    let hash_a = logic.state_hash().unwrap();
    let state_a = logic.export_state().unwrap();
    assert_ne!(state_a, at_checkpoint, "post-checkpoint ticks changed state");

    // Roll back: state returns to the checkpoint exactly.
    assert!(logic.rollback_checkpoint("msim").unwrap());
    assert_eq!(logic.export_state().unwrap(), at_checkpoint);

    // Relations and tags survived the rollback (checked both through the
    // model's own ecs:* lookups and through the snapshot JSON shape).
    let merchant = eval_int(&mut logic, "(merchant-id)");
    let stock = eval_int(&mut logic, "(stock-id)");
    assert_eq!(
        eval_int(&mut logic, "(ecs:target (merchant-id) \"Owns\")"),
        stock,
        "merchant Owns stock after rollback"
    );
    assert!(eval_bool(
        &mut logic,
        "(ecs:has-tag? (merchant-id) \"Merchant\")"
    ));
    assert!(
        eval_int(
            &mut logic,
            "(length (ecs:children (merchant-id) \"FilledBy\"))"
        ) >= 1,
        "at least one order is FilledBy the merchant after rollback"
    );
    let order_buyer = eval_int(
        &mut logic,
        "(ecs:target (order-slot-id 1) \"PlacedBy\")",
    );
    let alice = eval_int(&mut logic, "(alice-id)");
    let bob = eval_int(&mut logic, "(bob-id)");
    assert!(
        order_buyer == alice || order_buyer == bob,
        "order PlacedBy an account after rollback"
    );

    let snap = logic.export_snapshot().unwrap();
    let merchant_json = find_by_kind(&snap, "merchant");
    let stock_id = find_by_kind(&snap, "stock")["id"].as_u64().unwrap();
    let rels = merchant_json["relationships"].as_array().unwrap();
    assert!(
        rels.iter()
            .any(|r| r["relation"] == "Owns" && r["target_id"] == stock_id),
        "Owns relation in snapshot JSON: {}",
        merchant_json
    );
    assert_eq!(
        rels.iter().filter(|r| r["relation"] == "TradesWith").count(),
        2,
        "merchant TradesWith both accounts: {}",
        merchant_json
    );
    assert!(
        merchant_json["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t == "Merchant"),
        "Merchant tag in snapshot JSON: {}",
        merchant_json
    );
    let _ = merchant;

    // Re-run the same M steps: identical hash and identical values.
    tick(&mut logic, 3, 4);
    assert_eq!(logic.state_hash().unwrap(), hash_a);
    assert_eq!(logic.export_state().unwrap(), state_a);
}

// ---------------------------------------------------------------------------
// 3. Non-grid gate + not-a-shell check.
// ---------------------------------------------------------------------------
#[test]
fn test_market_sim_non_grid_gate() {
    let src = std::fs::read_to_string(fixture_dir().join("main.scm")).unwrap();
    let lower = src.to_lowercase();

    // Forbidden tokens: grid primitives & concepts from guide section 7.2
    // (position/glyph/spatial index/rasterizer/grid operators/player
    // session viewport binding) plus the legacy global-state hash this
    // model must not use for causal state.
    for token in [
        "set-pos",
        "get-pos",
        "gridpos",
        "spatial",
        "raster",
        "fov",
        "noise",
        "find-path",
        "dijkstra",
        "generate-cave",
        "set-glyph",
        "get-glyph",
        "render-viewport",
        "set-player-session",
        "ecs:at",
        "layer",
        "world-state",
    ] {
        assert!(
            !lower.contains(token),
            "market_sim source must not contain grid/legacy token {:?}",
            token
        );
    }

    // Not a shell: the model really drives the ECS through the core
    // primitives and the seeded host PRNG.
    for token in [
        "ecs:spawn",
        "ecs:clear!",
        "ecs:set",
        "ecs:get",
        "ecs:add-tag",
        "ecs:relate",
        "ecs:query",
        "engine:random-int",
        "\"owns\"",
        "\"tradeswith\"",
    ] {
        assert!(
            lower.contains(token),
            "market_sim source must use {:?}",
            token
        );
    }

    // Runtime side of the gate: after running, no exported entity carries
    // a position component (nothing ever entered a spatial index).
    let mut logic = SchemeLogic::new(fixture_dir(), false, None).unwrap();
    logic.init().unwrap();
    logic.set_random_seed(Some(7));
    for i in 0..3 {
        logic.on_tick(0.016, i).unwrap();
    }
    let snap = logic.export_snapshot().unwrap();
    let entities = snap["ecs_world"]["entities"].as_array().unwrap();
    assert!(!entities.is_empty());
    for e in entities {
        assert!(
            e["position"].is_null(),
            "entity must not carry a grid position: {}",
            e
        );
    }
}

