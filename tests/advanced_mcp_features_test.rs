use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use game_server::ServerConfig;
use serde_json::json;
use textcraft::{SchemeLogic, TextCraftServer, EMBEDDED_WEB_ASSETS};

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

async fn setup_test_server() -> (TextCraftServer, SocketAddr) {
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

    server.is_paused.store(true, std::sync::atomic::Ordering::Relaxed);
    (server, mcp_addr)
}

#[tokio::test]
async fn test_feature_1_prng_seed_determinism_mcp() {
    let (mut server, mcp_addr) = setup_test_server().await;
    server.logic.on_player_join(1, "alice:admin").unwrap();

    // 1. Evaluate with seed set in Scheme via game_command REPL mode ("! ...")
    let resp1 = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "!(begin (random:seed! 4242) (list (engine:random-int 1 1000) (engine:random-int 1 1000) (engine:random-int 1 1000)))"
                }
            }
        }),
    )
    .await;
    assert_eq!(resp1["result"]["isError"], false);
    let seq1 = resp1["result"]["content"][0]["text"].as_str().unwrap().to_string();

    // 2. Reset seed to identical 4242 and verify exact same sequence
    let resp2 = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "!(begin (random:seed! 4242) (list (engine:random-int 1 1000) (engine:random-int 1 1000) (engine:random-int 1 1000)))"
                }
            }
        }),
    )
    .await;
    assert_eq!(resp2["result"]["isError"], false);
    let seq2 = resp2["result"]["content"][0]["text"].as_str().unwrap().to_string();

    assert_eq!(seq1, seq2, "PRNG sequence must be identical when seeded with same value");

    // 3. Test step_simulation with seed parameter
    let step_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 1,
                    "seed": 99999
                }
            }
        }),
    )
    .await;
    assert_eq!(step_resp["result"]["isError"], false);
}

#[tokio::test]
async fn test_feature_2_batch_assertions_and_rich_diff_mcp() {
    let (mut server, mcp_addr) = setup_test_server().await;

    // 1. Batch eval with both passing and failing assertions
    let resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "eval_assertion",
                "arguments": {
                    "exprs": [
                        "(= (+ 2 3) 5)",
                        "(assert:equal (+ 10 20) 999)",
                        "(assert:true (= 1 2))",
                        "(assert:equal \"hello\" \"hello\")"
                    ]
                }
            }
        }),
    )
    .await;

    assert_eq!(resp["result"]["isError"], false);
    let text = resp["result"]["content"][0]["text"].as_str().unwrap();
    let res: serde_json::Value = serde_json::from_str(text).unwrap();

    assert_eq!(res["all_passed"], false);
    assert_eq!(res["total"], 4);
    assert_eq!(res["passed"], 2);
    assert_eq!(res["failed"], 2);

    let failures = res["failures"].as_array().unwrap();
    assert_eq!(failures.len(), 2);
    assert_eq!(failures[0]["expr"], "(assert:equal (+ 10 20) 999)");
    assert_eq!(failures[0]["expected"], 999);
    assert_eq!(failures[0]["actual"], 30);

    assert_eq!(failures[1]["expr"], "(assert:true (= 1 2))");

    // 2. Batch eval where all pass
    let pass_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "eval_assertion",
                "arguments": {
                    "exprs": [
                        "(= 1 1)",
                        "(assert:equal (+ 1 1) 2)",
                        "(assert:true #t)"
                    ]
                }
            }
        }),
    )
    .await;
    let pass_text = pass_resp["result"]["content"][0]["text"].as_str().unwrap();
    let pass_res: serde_json::Value = serde_json::from_str(pass_text).unwrap();
    assert_eq!(pass_res["all_passed"], true);
    assert_eq!(pass_res["total"], 3);
    assert_eq!(pass_res["passed"], 3);
    assert_eq!(pass_res["failed"], 0);
    assert_eq!(pass_res["failures"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_feature_3_state_projection_and_masking_mcp() {
    let (mut server, mcp_addr) = setup_test_server().await;
    server.logic.on_player_join(1, "alice:admin").unwrap();

    // 1. query_state with paths
    let query_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": {
                "name": "query_state",
                "arguments": {
                    "paths": ["tick_count", "players.1.x", "players.1.y"]
                }
            }
        }),
    )
    .await;

    assert_eq!(query_resp["result"]["isError"], false);
    let text = query_resp["result"]["content"][0]["text"].as_str().unwrap();
    let res: serde_json::Value = serde_json::from_str(text).unwrap();

    assert!(res["tick_count"].is_number());
    assert!(res["players"]["1"]["x"].is_number());
    assert!(res["players"]["1"]["y"].is_number());
    // Should NOT contain unrelated chunks or monsters
    assert!(res["chunks"].is_null());
    assert!(res["monsters"].is_null());

    // 2. step_simulation with state_projection on world state
    let step_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 1,
                    "state_projection": ["players.1.x", "players.1.y"]
                }
            }
        }),
    )
    .await;

    let step_text = step_resp["result"]["content"][0]["text"].as_str().unwrap();
    let step_res: serde_json::Value = serde_json::from_str(step_text).unwrap();

    let proj = &step_res["projected_state"];
    assert!(proj.is_object());
    assert!(proj["players"]["1"]["x"].is_number());
    assert!(proj["players"]["1"]["y"].is_number());
    // chunks should not be present in projected_state
    assert!(proj["chunks"].is_null());
}

#[tokio::test]
async fn test_feature_4_temporal_event_telemetry_mcp() {
    let (mut server, mcp_addr) = setup_test_server().await;
    server.logic.on_player_join(1, "alice:admin").unwrap();

    // Advance 5 steps so current-tick is 5
    send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 30,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 5
                }
            }
        }),
    )
    .await;

    // Add a log via Scheme REPL (channel: "system", text: "Telemetry test log")
    send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 31,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "!(player:add-log 1 \"system\" \"Telemetry test log\")"
                }
            }
        }),
    )
    .await;

    // Fetch snapshot
    let snap = server.logic.render_snapshot(1).unwrap();
    let logs = snap.hud["logs"].as_array().expect("hud.logs should be array");
    assert!(!logs.is_empty(), "Logs should contain at least one entry");

    let latest_log = &logs[logs.len() - 1];
    assert_eq!(latest_log["text"], "Telemetry test log");
    assert!(
        latest_log["tick"].is_number(),
        "Log entry must contain tick id"
    );
    assert_eq!(latest_log["tick"].as_i64().unwrap(), 5);
}

#[tokio::test]
async fn test_feature_5_scoped_state_diff_mcp() {
    let (mut server, mcp_addr) = setup_test_server().await;
    server.logic.on_player_join(1, "alice:admin").unwrap();

    // 1. Test get_state_diff tool with explicit old_state, new_state, prefix and exclude_keys
    let old_state = json!({
        "players": {
            "1": { "x": 10, "y": 20 }
        },
        "chunks": {
            "0:0": { "tiles": { "10,10,surface": { "glyph": "#" } } }
        }
    });
    let new_state = json!({
        "players": {
            "1": { "x": 11, "y": 20 }
        },
        "chunks": {
            "0:0": { "tiles": { "10,10,surface": { "glyph": "." } } }
        }
    });

    let diff_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 40,
            "method": "tools/call",
            "params": {
                "name": "get_state_diff",
                "arguments": {
                    "old_state": old_state,
                    "new_state": new_state,
                    "path_prefix": "players.1",
                    "exclude_keys": ["chunks"]
                }
            }
        }),
    )
    .await;

    assert_eq!(diff_resp["result"]["isError"], false);
    let diff_text = diff_resp["result"]["content"][0]["text"].as_str().unwrap();
    let diff: serde_json::Value = serde_json::from_str(diff_text).unwrap();

    let changed = &diff["changed"];
    assert!(changed["players.1.x"].is_object(), "Player x change must be captured");
    assert_eq!(changed["players.1.x"]["old"], 10);
    assert_eq!(changed["players.1.x"]["new"], 11);
    for key in changed.as_object().unwrap().keys() {
        assert!(!key.starts_with("chunks"), "Chunks must be excluded from diff: {}", key);
    }

    // 2. Test step_simulation with include_state_diff and diff_exclude_keys
    let sim_diff_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 41,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 1,
                    "inputs": [
                        {
                            "step": 0,
                            "player_id": 1,
                            "type": "key",
                            "code": "KeyD"
                        }
                    ],
                    "include_state_diff": true,
                    "diff_exclude_keys": ["chunks"]
                }
            }
        }),
    )
    .await;
    let sim_text = sim_diff_resp["result"]["content"][0]["text"].as_str().unwrap();
    let sim_res: serde_json::Value = serde_json::from_str(sim_text).unwrap();
    let sim_diff = &sim_res["state_diff"];
    assert!(sim_diff.is_object());
    assert!(sim_diff["changed"]["players.1.x"].is_object());
}

#[tokio::test]
async fn test_feature_6_continuous_input_and_hold_ticks_mcp() {
    let (mut server, mcp_addr) = setup_test_server().await;
    server.logic.on_player_join(1, "alice:admin").unwrap();

    // 1. Send KeyDown via player_input tool
    let down_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 50,
            "method": "tools/call",
            "params": {
                "name": "player_input",
                "arguments": {
                    "player_id": 1,
                    "type": "key",
                    "code": "KeyW",
                    "action": "down"
                }
            }
        }),
    )
    .await;
    assert_eq!(down_resp["result"]["isError"], false);

    // Verify key pressed in Scheme
    let check1 = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 51,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "!(player:key-pressed? 1 \"KeyW\")"
                }
            }
        }),
    )
    .await;
    let text1 = check1["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text1.contains("#true") || text1.contains("#t"), "KeyW should be pressed: {}", text1);

    // Advance 3 ticks via step_simulation
    send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 515,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 3
                }
            }
        }),
    )
    .await;

    let hold_check = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 52,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "!(player:key-hold-ticks 1 \"KeyW\")"
                }
            }
        }),
    )
    .await;
    let hold_text = hold_check["result"]["content"][0]["text"].as_str().unwrap();
    let num_str = hold_text.strip_prefix("=> ").unwrap_or(hold_text).trim();
    let hold_val: i64 = num_str.parse().unwrap_or(0);
    assert!(hold_val >= 3, "Hold ticks should be >= 3, got: {}", hold_text);

    // Send KeyUp
    let up_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 53,
            "method": "tools/call",
            "params": {
                "name": "player_input",
                "arguments": {
                    "player_id": 1,
                    "type": "key",
                    "code": "KeyW",
                    "action": "up"
                }
            }
        }),
    )
    .await;
    assert_eq!(up_resp["result"]["isError"], false);

    let check2 = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 54,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "!(player:key-pressed? 1 \"KeyW\")"
                }
            }
        }),
    )
    .await;
    let text2 = check2["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text2.contains("#false") || text2.contains("#f"), "KeyW should be released: {}", text2);

    // 2. Test step_simulation with hold_ticks
    let step_hold_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 55,
            "method": "tools/call",
            "params": {
                "name": "step_simulation",
                "arguments": {
                    "steps": 3,
                    "hold_ticks": 3,
                    "inputs": [
                        {
                            "step": 0,
                            "player_id": 1,
                            "type": "key",
                            "code": "KeyD"
                        }
                    ]
                }
            }
        }),
    )
    .await;
    assert_eq!(step_hold_resp["result"]["isError"], false);
}
