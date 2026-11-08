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

#[tokio::test]
async fn test_rust_internal_mcp_storage_api() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    // 1. Initial storage is empty
    assert_eq!(server.mcp_store_list(None).len(), 0);
    assert_eq!(server.mcp_store_get("score"), None);

    // 2. Set and Get basic values
    server.mcp_store_set("score", json!(999));
    server.mcp_store_set("notes", json!("Agent explored dungeon north"));
    server.mcp_store_set("visited_rooms", json!([101, 102, 103]));

    assert_eq!(server.mcp_store_get("score"), Some(json!(999)));
    assert_eq!(
        server.mcp_store_get("notes"),
        Some(json!("Agent explored dungeon north"))
    );
    assert_eq!(
        server.mcp_store_get("visited_rooms"),
        Some(json!([101, 102, 103]))
    );

    // 3. List keys with and without prefix
    let all_keys = server.mcp_store_list(None);
    assert_eq!(all_keys.len(), 3);
    assert!(all_keys.contains(&"default:score".to_string()));
    assert!(all_keys.contains(&"default:notes".to_string()));
    assert!(all_keys.contains(&"default:visited_rooms".to_string()));

    let prefix_keys = server.mcp_store_list(Some("default:no"));
    assert_eq!(prefix_keys, vec!["default:notes".to_string()]);

    // 4. Scoped namespaces
    server.mcp_store_set_scoped(Some("blackboard"), "target_boss", json!("goblin_king"));
    server.mcp_store_set_scoped(Some("blackboard"), "risk_level", json!(0.85));

    // Scoped retrieval
    assert_eq!(
        server.mcp_store_get_scoped(Some("blackboard"), "target_boss"),
        Some(json!("goblin_king"))
    );
    // Retrieval via composite key
    assert_eq!(
        server.mcp_store_get("blackboard:target_boss"),
        Some(json!("goblin_king"))
    );

    // Scoped list
    let bb_keys = server.mcp_store_list_scoped(Some("blackboard"), None);
    assert_eq!(
        bb_keys,
        vec!["risk_level".to_string(), "target_boss".to_string()]
    );

    let default_keys = server.mcp_store_list_scoped(Some("default"), None);
    assert_eq!(default_keys.len(), 3);
    assert!(default_keys.contains(&"score".to_string()));

    // 5. Delete keys
    assert!(server.mcp_store_delete("score"));
    assert!(!server.mcp_store_delete("score"));
    assert_eq!(server.mcp_store_get("score"), None);

    assert!(server.mcp_store_delete_scoped(Some("blackboard"), "risk_level"));
    assert_eq!(
        server.mcp_store_get_scoped(Some("blackboard"), "risk_level"),
        None
    );

    // 6. Clear scoped namespace
    let cleared_bb = server.mcp_store_clear_scoped(Some("blackboard"));
    assert_eq!(cleared_bb, 1); // target_boss
    assert_eq!(
        server.mcp_store_list_scoped(Some("blackboard"), None).len(),
        0
    );
    // Default namespace items still exist
    assert_eq!(
        server.mcp_store_list_scoped(Some("default"), None).len(),
        2
    );

    // 7. Clear all
    server.mcp_store_clear();
    assert_eq!(server.mcp_store_list(None).len(), 0);

    server.handle.shutdown();
}

#[tokio::test]
async fn test_mcp_storage_tools_jsonrpc() {
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

    // 1. Tool: mcp_store_set
    let set_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_set",
                "arguments": {
                    "key": "eval_metrics",
                    "value": {
                        "steps": 42,
                        "accuracy": 0.98,
                        "tags": ["sim", "agent"]
                    },
                    "namespace": "eval"
                }
            }
        }),
    )
    .await;
    assert_eq!(set_resp["result"]["isError"], false);
    let set_data: serde_json::Value =
        serde_json::from_str(set_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(set_data["success"], true);
    assert_eq!(set_data["namespace"], "eval");
    assert_eq!(set_data["key"], "eval_metrics");
    assert_eq!(set_data["composite_key"], "eval:eval_metrics");

    // 2. Tool: mcp_store_get
    let get_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_get",
                "arguments": {
                    "key": "eval_metrics",
                    "namespace": "eval"
                }
            }
        }),
    )
    .await;
    assert_eq!(get_resp["result"]["isError"], false);
    let get_data: serde_json::Value =
        serde_json::from_str(get_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(get_data["found"], true);
    assert_eq!(get_data["value"]["accuracy"], 0.98);
    assert_eq!(get_data["value"]["steps"], 42);

    // 3. Tool: mcp_store_get for nonexistent key
    let get_none_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_get",
                "arguments": {
                    "key": "nonexistent"
                }
            }
        }),
    )
    .await;
    assert_eq!(get_none_resp["result"]["isError"], false);
    let get_none_data: serde_json::Value = serde_json::from_str(
        get_none_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(get_none_data["found"], false);
    assert_eq!(get_none_data["value"], serde_json::Value::Null);

    // 4. Tool: mcp_store_list
    send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_set",
                "arguments": {
                    "key": "eval_loss",
                    "value": 0.02,
                    "namespace": "eval"
                }
            }
        }),
    )
    .await;

    let list_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_list",
                "arguments": {
                    "namespace": "eval"
                }
            }
        }),
    )
    .await;
    assert_eq!(list_resp["result"]["isError"], false);
    let list_data: serde_json::Value =
        serde_json::from_str(list_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(list_data["count"], 2);
    let keys = list_data["keys"].as_array().unwrap();
    assert!(keys.contains(&json!("eval_loss")));
    assert!(keys.contains(&json!("eval_metrics")));

    // 5. Tool: mcp_store_delete
    let del_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_delete",
                "arguments": {
                    "key": "eval_loss",
                    "namespace": "eval"
                }
            }
        }),
    )
    .await;
    assert_eq!(del_resp["result"]["isError"], false);
    let del_data: serde_json::Value =
        serde_json::from_str(del_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(del_data["success"], true);
    assert_eq!(del_data["deleted"], true);

    // 6. Tool: mcp_store_clear
    let clear_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_clear",
                "arguments": {
                    "namespace": "eval"
                }
            }
        }),
    )
    .await;
    assert_eq!(clear_resp["result"]["isError"], false);
    let clear_data: serde_json::Value = serde_json::from_str(
        clear_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(clear_data["success"], true);
    assert_eq!(clear_data["cleared_count"], 1);

    server.handle.shutdown();
}

#[tokio::test]
async fn test_checkpoint_and_time_travel_rollback() {
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

    // Pause physical simulation clock for deterministic test control
    server.is_paused.store(true, std::sync::atomic::Ordering::Relaxed);

    // 1. Establish player 1 and advance clock to tick 10
    server.logic.on_player_join(1, "token_1").unwrap();
    for _ in 0..10 {
        server.handle.increment_server_tick();
        server.logic.on_tick(0.016, server.handle.server_tick()).unwrap();
    }
    let initial_tick = server.handle.server_tick();
    assert_eq!(initial_tick, 10);

    // Get initial player coordinates
    let init_state = server.logic.export_state().unwrap();
    let init_x = init_state["players"]["1"]["x"].as_i64().unwrap_or(0);
    let init_y = init_state["players"]["1"]["y"].as_i64().unwrap_or(0);

    // 2. Create checkpoint "initial" via MCP tool
    let cp_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "create_checkpoint",
                "arguments": {
                    "tag": "initial"
                }
            }
        }),
    )
    .await;
    assert_eq!(cp_resp["result"]["isError"], false);
    let cp_data: serde_json::Value =
        serde_json::from_str(cp_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(cp_data["success"], true);
    assert_eq!(cp_data["tag"], "initial");

    // Checkpoint is directly accessible in dedicated MCP storage under "checkpoint" namespace
    let stored_cp = server.mcp_store_get_scoped(Some("checkpoint"), "initial");
    assert!(stored_cp.is_some(), "Checkpoint 'initial' must be stored in mcp-store under 'checkpoint' namespace");
    let cp_val = stored_cp.unwrap();
    assert!(cp_val.get("players").is_some());
    assert_eq!(cp_val["players"]["1"]["x"], init_x);

    // 3. Advance simulation and modify game state: step 20 ticks, inject move KeyD
    for _ in 0..20 {
        let _ = server.logic.on_input(
            1,
            &textcraft::PlayerInput::Key {
                code: "KeyD".to_string(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        );
        let t = server.handle.increment_server_tick();
        server.logic.on_tick(0.016, t).unwrap();
    }
    let advanced_tick = server.handle.server_tick();
    assert_eq!(advanced_tick, 30);

    let moved_state = server.logic.export_state().unwrap();
    let moved_x = moved_state["players"]["1"]["x"].as_i64().unwrap_or(0);
    assert_ne!(
        init_x, moved_x,
        "Player should have moved after KeyD input"
    );

    // 4. Create second checkpoint "after_move" via MCP tool
    let cp2_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "create_checkpoint",
                "arguments": {
                    "tag": "after_move"
                }
            }
        }),
    )
    .await;
    assert_eq!(cp2_resp["result"]["isError"], false);
    let cp2_data: serde_json::Value =
        serde_json::from_str(cp2_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(cp2_data["success"], true);
    assert_eq!(cp2_data["tag"], "after_move");

    let stored_after = server.mcp_store_get_scoped(Some("checkpoint"), "after_move");
    assert!(stored_after.is_some());
    assert_eq!(stored_after.unwrap()["players"]["1"]["x"], moved_x);

    let cp_keys = server.mcp_store_list_scoped(Some("checkpoint"), None);
    assert!(cp_keys.contains(&"initial".to_string()));
    assert!(cp_keys.contains(&"after_move".to_string()));

    // Further modify state: player moves further
    for _ in 0..10 {
        let _ = server.logic.on_input(
            1,
            &textcraft::PlayerInput::Key {
                code: "KeyD".to_string(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        );
        let t = server.handle.increment_server_tick();
        server.logic.on_tick(0.016, t).unwrap();
    }
    assert_eq!(server.handle.server_tick(), 40);

    // 5. Rollback to "after_move" via MCP tool
    let rb_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "rollback_checkpoint",
                "arguments": {
                    "tag": "after_move"
                }
            }
        }),
    )
    .await;
    assert_eq!(rb_resp["result"]["isError"], false);
    let rb_data: serde_json::Value =
        serde_json::from_str(rb_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(rb_data["success"], true);
    assert_eq!(rb_data["tag"], "after_move");

    let restored_state = server.logic.export_state().unwrap();
    assert_eq!(
        restored_state["players"]["1"]["x"],
        moved_x,
        "Player x should match after_move checkpoint"
    );

    // 6. Rollback to "initial" via MCP tool
    let rb_init_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "rollback_checkpoint",
                "arguments": {
                    "tag": "initial"
                }
            }
        }),
    )
    .await;
    assert_eq!(rb_init_resp["result"]["isError"], false);
    let rb_init_data: serde_json::Value =
        serde_json::from_str(rb_init_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(rb_init_data["success"], true);
    assert_eq!(rb_init_data["tag"], "initial");

    let initial_restored = server.logic.export_state().unwrap();
    assert_eq!(
        initial_restored["players"]["1"]["x"], init_x,
        "Player x should match initial checkpoint"
    );
    assert_eq!(
        initial_restored["players"]["1"]["y"], init_y,
        "Player y should match initial checkpoint"
    );

    // 7. Rollback to nonexistent checkpoint returns error
    let err_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "rollback_checkpoint",
                "arguments": {
                    "tag": "does_not_exist"
                }
            }
        }),
    )
    .await;
    assert_eq!(err_resp["result"]["isError"], true);

    server.handle.shutdown();
}

#[tokio::test]
async fn test_mcp_storage_file_persistence_and_no_game_save_pollution() {
    let temp_dir = std::env::temp_dir().join(format!("textcraft_mcp_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let game_save = temp_dir.join("game_save.json");

    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, Some(game_save.clone())).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    // 1. Write dedicated MCP variables
    server.mcp_store_set("agent_plan", json!({"objective": "defeat_boss", "step": 3}));
    server.mcp_store_set_scoped(Some("heuristics"), "h_score", json!(42.5));

    // 2. Export MCP storage to independent file
    let mcp_export_file = temp_dir.join("dedicated_mcp.json");
    let exp_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_export",
                "arguments": {
                    "path": mcp_export_file.to_str().unwrap()
                }
            }
        }),
    )
    .await;
    assert_eq!(exp_resp["result"]["isError"], false);
    assert!(mcp_export_file.exists());

    // 3. Clear MCP storage and verify it can be imported back
    server.mcp_store_clear();
    assert_eq!(server.mcp_store_list(None).len(), 0);

    let imp_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "mcp_store_import",
                "arguments": {
                    "path": mcp_export_file.to_str().unwrap()
                }
            }
        }),
    )
    .await;
    assert_eq!(imp_resp["result"]["isError"], false);
    assert_eq!(
        server.mcp_store_get("agent_plan"),
        Some(json!({"objective": "defeat_boss", "step": 3}))
    );
    assert_eq!(
        server.mcp_store_get_scoped(Some("heuristics"), "h_score"),
        Some(json!(42.5))
    );

    // 4. Save game world state
    server.logic.save_state_to_file().unwrap();
    assert!(game_save.exists());

    // 5. Verify that game_save.json does NOT contain agent_plan or h_score (no pollution)
    let game_save_content = std::fs::read_to_string(&game_save).unwrap();
    let game_save_json: serde_json::Value = serde_json::from_str(&game_save_content).unwrap();
    assert!(
        game_save_json.get("agent_plan").is_none(),
        "MCP agent_plan must not pollute game save file"
    );
    assert!(
        game_save_json.get("heuristics").is_none(),
        "MCP heuristics must not pollute game save file"
    );

    server.handle.shutdown();
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_scheme_direct_mcp_store_functions() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    // 1. Write from Scheme REPL to default namespace
    let set_res = server.logic.eval_repl("(mcp-store:set! \"agent_state\" (hash \"stage\" \"init\" \"hp_threshold\" 30))").unwrap();
    assert!(set_res.contains("#true") || set_res.contains("#t"));

    // Rust sees it immediately in mcp_storage
    let stored = server.mcp_store_get("agent_state");
    assert_eq!(
        stored,
        Some(json!({"stage": "init", "hp_threshold": 30}))
    );

    // 2. Write from Scheme REPL to explicit namespace "blackboard"
    let set_bb = server.logic.eval_repl("(mcp-store:set! \"boss_weakness\" \"fire\" \"blackboard\")").unwrap();
    assert!(set_bb.contains("#true") || set_bb.contains("#t"));

    // Rust sees it in scoped namespace
    assert_eq!(
        server.mcp_store_get_scoped(Some("blackboard"), "boss_weakness"),
        Some(json!("fire"))
    );

    // 3. Scheme reads value back
    let get_val = server.logic.eval_repl("(mcp-store:get \"boss_weakness\" \"blackboard\")").unwrap();
    assert!(get_val.contains("fire"));

    // 4. Scheme lists keys in namespace
    let list_val = server.logic.eval_repl("(mcp-store:list \"blackboard\")").unwrap();
    assert!(list_val.contains("boss_weakness"));

    // 5. Scheme deletes key
    let del_res = server.logic.eval_repl("(mcp-store:delete! \"agent_state\")").unwrap();
    assert!(del_res.contains("#true") || del_res.contains("#t"));
    assert_eq!(server.mcp_store_get("agent_state"), None);

    server.handle.shutdown();
}

#[tokio::test]
async fn test_reload_scripts_clean_and_lossless() {
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

    // 1. Establish player 1 and verify initial coordinates and gold
    server.logic.on_player_join(1, "token_1").unwrap();
    let init_state = server.logic.export_state().unwrap();
    let init_x = init_state["players"]["1"]["x"].as_i64().unwrap_or(0);
    let init_y = init_state["players"]["1"]["y"].as_i64().unwrap_or(0);
    assert_eq!(init_x, 0);
    assert_eq!(init_y, 0);

    // 2. Modify world state: move player with KeyD and add gold
    for _ in 0..5 {
        let _ = server.logic.on_input(
            1,
            &textcraft::PlayerInput::Key {
                code: "KeyD".to_string(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        );
        let t = server.handle.increment_server_tick();
        server.logic.on_tick(0.016, t).unwrap();
    }
    server
        .logic
        .eval_repl("(set-player 1 (hash-insert (ensure-player 1) \"gold\" 9999))")
        .unwrap();

    let mod_state = server.logic.export_state().unwrap();
    let mod_x = mod_state["players"]["1"]["x"].as_i64().unwrap();
    let mod_gold = mod_state["players"]["1"]["gold"].as_i64().unwrap();
    assert!(mod_x > 0, "Player x should have moved");
    assert_eq!(mod_gold, 9999, "Player gold should be modified");

    // 3. Call reload_scripts with default arguments (clean defaults to true)
    let reload_default_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "reload_scripts",
                "arguments": {}
            }
        }),
    )
    .await;
    assert_eq!(reload_default_resp["result"]["isError"], false);
    assert_eq!(
        reload_default_resp["result"]["content"][0]["text"].as_str().unwrap(),
        "Reloaded successfully"
    );

    // Verify state was reset: previously modified player 1 is gone from world state
    let clean_state = server.logic.export_state().unwrap();
    assert!(
        clean_state["players"].get("1").is_none(),
        "Player state should be completely cleared after clean reload"
    );

    // When player 1 joins the fresh world, initial coordinates are reset back to (0, 0) and default gold (0)
    server.logic.on_player_join(1, "token_1").unwrap();
    let fresh_state = server.logic.export_state().unwrap();
    assert_eq!(fresh_state["players"]["1"]["x"], 0);
    assert_eq!(fresh_state["players"]["1"]["y"], 0);
    assert_ne!(fresh_state["players"]["1"]["gold"], 9999);

    // 4. Modify state again: move player and set gold to 5555
    for _ in 0..3 {
        let _ = server.logic.on_input(
            1,
            &textcraft::PlayerInput::Key {
                code: "KeyD".to_string(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        );
        let t = server.handle.increment_server_tick();
        server.logic.on_tick(0.016, t).unwrap();
    }
    server
        .logic
        .eval_repl("(set-player 1 (hash-insert (ensure-player 1) \"gold\" 5555))")
        .unwrap();
    let mod2_state = server.logic.export_state().unwrap();
    let mod2_x = mod2_state["players"]["1"]["x"].as_i64().unwrap();
    let mod2_gold = mod2_state["players"]["1"]["gold"].as_i64().unwrap();
    assert!(mod2_x > 0);
    assert_eq!(mod2_gold, 5555);

    // 5. Call reload_scripts with clean: false (lossless hot reload)
    let reload_lossless_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "reload_scripts",
                "arguments": {
                    "clean": false
                }
            }
        }),
    )
    .await;
    assert_eq!(reload_lossless_resp["result"]["isError"], false);
    assert_eq!(
        reload_lossless_resp["result"]["content"][0]["text"].as_str().unwrap(),
        "Reloaded successfully"
    );

    // Verify modified state is preserved when clean: false
    let preserved_state = server.logic.export_state().unwrap();
    assert_eq!(
        preserved_state["players"]["1"]["x"].as_i64().unwrap(),
        mod2_x,
        "Player x coordinate must be preserved when clean: false"
    );
    assert_eq!(
        preserved_state["players"]["1"]["gold"].as_i64().unwrap(),
        5555,
        "Player gold must be preserved when clean: false"
    );

    // 6. Explicit clean: true also resets world state
    let reload_clean_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "reload_scripts",
                "arguments": {
                    "clean": true
                }
            }
        }),
    )
    .await;
    assert_eq!(reload_clean_resp["result"]["isError"], false);
    let explicit_clean_state = server.logic.export_state().unwrap();
    assert!(
        explicit_clean_state["players"].get("1").is_none(),
        "Player state should be completely cleared after clean: true reload"
    );

    server.handle.shutdown();
}
