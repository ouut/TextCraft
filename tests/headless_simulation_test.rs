use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use game_server::ServerConfig;
use serde_json::json;
use textcraft::{
    compute_json_diff, SchemeLogic, TextCraftServer, EMBEDDED_WEB_ASSETS,
};

fn get_scripts_dir() -> PathBuf {
    let p = PathBuf::from("scripts");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../scripts")
    }
}

async fn send_mcp_http_post(
    server: &mut TextCraftServer,
    mcp_addr: SocketAddr,
    payload: serde_json::Value,
) -> serde_json::Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        let (_, json_body) = resp_str
            .split_once("\r\n\r\n")
            .expect("HTTP response should contain header/body separator");
        serde_json::from_str::<serde_json::Value>(json_body.trim()).unwrap()
    });

    while !join.is_finished() {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick(0.016).unwrap();
    }

    join.await.unwrap()
}

#[tokio::test]
async fn test_pause_and_resume_clock_mcp() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    // 1. Initial state: clock running
    assert!(!server.is_paused.load(std::sync::atomic::Ordering::Relaxed));
    let t0 = server.handle.server_tick();
    server.tick(0.016).unwrap();
    let t1 = server.handle.server_tick();
    assert_eq!(t1, t0 + 1);

    // 2. Pause clock via MCP tool call
    let pause_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "pause_clock",
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(pause_resp["result"]["isError"], false);
    assert!(server.is_paused.load(std::sync::atomic::Ordering::Relaxed));

    // When paused, physical tick does NOT advance server_tick
    let t_paused_start = server.handle.server_tick();
    for _ in 0..5 {
        server.tick(0.016).unwrap();
    }
    let t_paused_end = server.handle.server_tick();
    assert_eq!(
        t_paused_start, t_paused_end,
        "Server tick must not advance while clock is paused"
    );

    // 3. Resume clock via MCP tool call
    let resume_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "resume_clock",
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(resume_resp["result"]["isError"], false);
    assert!(!server.is_paused.load(std::sync::atomic::Ordering::Relaxed));

    let t_before_resume_tick = server.handle.server_tick();
    server.tick(0.016).unwrap();
    assert_eq!(server.handle.server_tick(), t_before_resume_tick + 1);
}

#[tokio::test]
async fn test_step_simulation_and_input_injection() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    // Join Player 1
    server.logic.on_player_join(1, "alice:admin").unwrap();
    // Pause clock so background tick doesn't race
    server.is_paused.store(true, std::sync::atomic::Ordering::Relaxed);

    // Get Player 1 initial position
    let state_init = server.logic.export_state().unwrap();
    let init_x = state_init["players"]["1"]["x"].as_i64().unwrap_or(0);
    let init_y = state_init["players"]["1"]["y"].as_i64().unwrap_or(0);

    // Call step_simulation with 2 steps, injecting KeyD at step 0
    let step_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 2,
                    "dt": 0.016,
                    "inputs": [
                        {
                            "step": 0,
                            "player_id": 1,
                            "type": "key",
                            "code": "KeyD"
                        }
                    ],
                    "include_snapshot": true,
                    "snapshot_player_id": 1,
                    "include_state_diff": true
                }
            }
        }),
    )
    .await;

    assert_eq!(step_resp["result"]["isError"], false);
    let text = step_resp["result"]["content"][0]["text"].as_str().unwrap();
    let res: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(res["steps_executed"], 2);
    assert!(res["duration_ms"].as_f64().unwrap() >= 0.0);
    assert!(res["snapshot"].is_object());
    assert!(res["snapshot"]["ascii_screen"].is_string());
    assert_eq!(res["snapshot"]["x"], init_x + 1);
    assert_eq!(res["snapshot"]["y"], init_y);

    // State diff should capture players.1.x change
    let diff = &res["state_diff"];
    assert!(diff.is_object());
    let changed = &diff["changed"];
    assert!(
        changed["players.1.x"].is_object(),
        "state_diff should record players.1.x change, got: {:?}",
        changed
    );
    assert_eq!(changed["players.1.x"]["old"], init_x);
    assert_eq!(changed["players.1.x"]["new"], init_x + 1);
}

#[tokio::test]
async fn test_state_diff_algorithm() {
    let old_state = json!({
        "players": {
            "1": { "x": 5, "y": 10, "hp": 100 }
        },
        "score": 0,
        "items": ["sword", "shield"]
    });

    let new_state = json!({
        "players": {
            "1": { "x": 6, "y": 10, "hp": 90 },
            "2": { "x": 0, "y": 0, "hp": 100 }
        },
        "items": ["sword", "golden_shield", "potion"]
    });

    let diff = compute_json_diff(&old_state, &new_state);

    // Changed: players.1.x (5 -> 6), players.1.hp (100 -> 90), items.1 ("shield" -> "golden_shield")
    assert_eq!(diff["changed"]["players.1.x"]["old"], 5);
    assert_eq!(diff["changed"]["players.1.x"]["new"], 6);
    assert_eq!(diff["changed"]["players.1.hp"]["old"], 100);
    assert_eq!(diff["changed"]["players.1.hp"]["new"], 90);
    assert_eq!(diff["changed"]["items.1"]["old"], "shield");
    assert_eq!(diff["changed"]["items.1"]["new"], "golden_shield");

    // Removed: score
    assert_eq!(diff["removed"]["score"], 0);

    // Added: players.2, items.2
    assert!(diff["added"]["players.2"].is_object());
    assert_eq!(diff["added"]["items.2"], "potion");
}

#[tokio::test]
async fn test_eval_assertion_mcp() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    // 1. Passing assertion
    let pass_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "eval_assertion",
                "arguments": {
                    "code": "(= (+ 2 3) 5)"
                }
            }
        }),
    )
    .await;
    assert_eq!(pass_resp["result"]["isError"], false);
    let pass_text = pass_resp["result"]["content"][0]["text"].as_str().unwrap();
    let pass_json: serde_json::Value = serde_json::from_str(pass_text).unwrap();
    assert_eq!(pass_json["passed"], true);
    assert_eq!(pass_json["result"], true);

    // 2. Failing assertion (returns #f)
    let fail_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": {
                "name": "eval_assertion",
                "arguments": {
                    "code": "(= (+ 2 3) 6)"
                }
            }
        }),
    )
    .await;
    assert_eq!(fail_resp["result"]["isError"], false);
    let fail_text = fail_resp["result"]["content"][0]["text"].as_str().unwrap();
    let fail_json: serde_json::Value = serde_json::from_str(fail_text).unwrap();
    assert_eq!(fail_json["passed"], false);
    assert!(fail_json["error"].as_str().unwrap().contains("#f"));

    // 3. Scheme runtime error assertion (undefined symbol or syntax error)
    let err_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "tools/call",
            "params": {
                "name": "eval_assertion",
                "arguments": {
                    "code": "(undefined-function-call-12345)"
                }
            }
        }),
    )
    .await;
    assert_eq!(err_resp["result"]["isError"], false);
    let err_text = err_resp["result"]["content"][0]["text"].as_str().unwrap();
    let err_json: serde_json::Value = serde_json::from_str(err_text).unwrap();
    assert_eq!(err_json["passed"], false);
    assert!(err_json["error"].as_str().unwrap().contains("error"));
}
