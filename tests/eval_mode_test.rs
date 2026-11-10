//! Guide item 5 acceptance: eval mode freezes the write-class tool
//! surface at the MCP dispatch layer, while dev mode (default) keeps the
//! full v5.0 development surface: '!' REPL, ecs_modify, hot reload with
//! rollback, runtime (mcp:def-tool ...) registration, Scheme assert_state.
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
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

async fn make_server(eval: bool) -> (TextCraftServer, SocketAddr) {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(model_dir(), false, None).unwrap();
    logic.init().unwrap();
    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();
    if eval {
        server.set_eval_mode(true);
    }
    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    server.is_paused.store(true, std::sync::atomic::Ordering::Relaxed);
    (server, mcp_addr)
}

async fn call(
    server: &mut TextCraftServer,
    mcp_addr: SocketAddr,
    name: &str,
    arguments: serde_json::Value,
) -> (bool, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let payload = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    });
    let body = serde_json::to_string(&payload).unwrap();
    let req_str = format!(
        "POST /mcp HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        mcp_addr,
        body.len(),
        body
    );
    let join = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(mcp_addr).await.unwrap();
        stream.write_all(req_str.as_bytes()).await.unwrap();
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await.unwrap();
        let resp_str = String::from_utf8(buf).unwrap();
        let (_, json_body) = resp_str.split_once("\r\n\r\n").unwrap();
        serde_json::from_str::<serde_json::Value>(json_body.trim()).unwrap()
    });
    while !join.is_finished() {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick(0.016).unwrap();
    }
    let resp = join.await.unwrap();
    let is_error = resp["result"]["isError"].as_bool().unwrap_or(true);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap_or("").to_string();
    (is_error, text)
}

#[tokio::test]
async fn test_eval_mode_blocks_write_tools_allows_reads() {
    let (mut server, mcp) = make_server(true).await;
    server.logic.on_player_join(1, "token_1").unwrap();

    // Blocked: ecs_modify
    let (err, _) = call(&mut server, mcp, "ecs_modify", json!({ "eid": 1, "set_props": { "hp": 1 } })).await;
    assert!(err, "ecs_modify must be rejected in eval mode");
    // Blocked: ecs_inspect with modify patch
    let (err, _) = call(&mut server, mcp, "ecs_inspect", json!({ "eid": 1, "modify": { "set_props": { "hp": 1 } } })).await;
    assert!(err, "ecs_inspect+modify must be rejected in eval mode");
    // Blocked: '!' REPL via game_command
    let (err, _) = call(&mut server, mcp, "game_command", json!({ "player_id": 1, "command": "! (+ 1 2)" })).await;
    assert!(err, "'!' REPL must be rejected in eval mode");
    // Blocked: reload_scripts
    let (err, _) = call(&mut server, mcp, "reload_scripts", json!({ "clean": false })).await;
    assert!(err, "reload_scripts must be rejected in eval mode");
    // Blocked: checkpoints (host snapshots are run_epoch-internal in eval)
    let (err, _) = call(&mut server, mcp, "create_checkpoint", json!({ "tag": "x" })).await;
    assert!(err, "create_checkpoint must be rejected in eval mode");
    // Blocked: Scheme-side assertions
    let (err, _) = call(&mut server, mcp, "assert_state", json!({ "code": "(= 1 1)" })).await;
    assert!(err, "Scheme assert_state must be rejected in eval mode");
    // Blocked: dynamic tool registered AFTER eval started
    server
        .logic
        .eval_repl(r#"(mcp:def-tool "evil_tool" "x" (hash "type" "object" "properties" (hash)) (lambda (args) (hash "ok" #t)))"#)
        .unwrap();
    let (err, _) = call(&mut server, mcp, "evil_tool", json!({})).await;
    assert!(err, "post-eval dynamic registration must be rejected in eval mode");

    // Allowed: ordinary game command (input, not REPL)
    let (err, _) = call(&mut server, mcp, "game_command", json!({ "player_id": 1, "command": "(where)" })).await;
    assert!(!err, "plain game commands stay available in eval mode");
    // Allowed: step / query / hash / epoch
    let (err, _) = call(&mut server, mcp, "step_simulation", json!({ "steps": 2 })).await;
    assert!(!err, "step_simulation works in eval mode");
    let (err, _) = call(&mut server, mcp, "query_state", json!({ "paths": ["tick_count"] })).await;
    assert!(!err, "query_state works in eval mode");
    let (err, text) = call(&mut server, mcp, "state_hash", json!({})).await;
    assert!(!err && text.contains("state_hash"), "state_hash works in eval mode: {}", text);
    let (err, text) = call(
        &mut server,
        mcp,
        "run_epoch",
        json!({ "seed": 3, "steps": 5, "assertions": [ { "path": "tick_count", "op": "eq", "expected": 5 } ] }),
    )
    .await;
    assert!(!err && text.contains("\"all_passed\": true"), "run_epoch works in eval mode: {}", text);
    // Allowed: dynamic tools registered BEFORE eval started stay usable.
    let (err, _) = call(&mut server, mcp, "get_world_summary", json!({})).await;
    assert!(!err, "load-time dynamic tools stay available in eval mode");

    server.handle.shutdown();
}

#[tokio::test]
async fn test_dev_mode_keeps_full_v5_surface() {
    let (mut server, mcp) = make_server(false).await;
    server.logic.on_player_join(1, "token_1").unwrap();

    // '!' REPL works
    let (err, text) = call(&mut server, mcp, "game_command", json!({ "player_id": 1, "command": "! (+ 40 2)" })).await;
    assert!(!err && text.contains("42"), "dev REPL must work: {}", text);

    // ecs_modify works: spawn via REPL, parse the id, modify, inspect.
    let (err, text) = call(&mut server, mcp, "game_command", json!({ "player_id": 1, "command": "! (ecs:spawn-named \"DevEnt\")" })).await;
    assert!(!err, "dev spawn via REPL: {}", text);
    let eid: u64 = text
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .expect("spawn result should contain the entity id");
    let (err, text) = call(&mut server, mcp, "ecs_modify", json!({ "eid": eid, "set_props": { "hp": 7 } })).await;
    assert!(!err && text.contains("\"hp\": 7"), "dev ecs_modify must work: {}", text);

    // Runtime (mcp:def-tool ...) registration works and the tool is callable.
    let (err, _) = call(
        &mut server,
        mcp,
        "game_command",
        json!({ "player_id": 1, "command": "! (mcp:def-tool \"dev_tool\" \"t\" (hash \"type\" \"object\" \"properties\" (hash)) (lambda (args) (hash \"ok\" #t)))" }),
    )
    .await;
    assert!(!err, "dev def-tool registration must work");
    let (err, text) = call(&mut server, mcp, "dev_tool", json!({})).await;
    assert!(!err && text.contains("ok"), "dev dynamic tool callable: {}", text);

    // Scheme assert_state works in dev.
    let (err, text) = call(&mut server, mcp, "assert_state", json!({ "code": "(= 1 1)" })).await;
    assert!(!err && text.contains("\"passed\": true"), "dev assert_state must work: {}", text);

    // Hot reload (lossless) works in dev.
    let (err, text) = call(&mut server, mcp, "reload_scripts", json!({ "clean": false })).await;
    assert!(!err && text.contains("Reloaded successfully"), "dev reload must work: {}", text);

    server.handle.shutdown();
}
