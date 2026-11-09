use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, ServerEnvelope};
use game_server::ServerConfig;
use textcraft::{
    clap::Parser, run_server_with_handle, LogLevel, PlayerInput, PlayerSnapshot, SchemeLogic,
    ServerArgs, TextCraftCommand, TextCraftEvent, TextCraftServer, EMBEDDED_WEB_ASSETS,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

fn get_scripts_dir() -> PathBuf {
    let p = PathBuf::from("scripts");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../scripts")
    }
}

/// Test 1: Full CLI argument parsing and server config construction.
#[test]
fn test_1_cli_argument_parsing_and_config() {
    // 1. Explicit arguments parsing
    let args = ServerArgs::try_parse_from([
        "textcraft-server",
        "--host",
        "0.0.0.0",
        "--port",
        "9999",
        "--scripts-dir",
        "custom_scripts",
        "--watch",
        "--log-level",
        "debug",
        "--time-scale",
        "1.5",
        "--admin-token",
        "my_super_token",
        "--save-file",
        "my_save.json",
        "--autosave-sec",
        "45",
        "--snapshot-heartbeat-ms",
        "500",
    ])
    .expect("CLI args parsing should succeed");

    assert_eq!(args.host, "0.0.0.0");
    assert_eq!(args.port, 9999);
    assert_eq!(args.scripts_dir, PathBuf::from("custom_scripts"));
    assert!(args.watch);
    assert_eq!(args.log_level, LogLevel::Debug);
    assert_eq!(args.log_level.as_str(), "debug");
    assert!((args.time_scale - 1.5).abs() < 1e-4);
    assert_eq!(args.admin_token, "my_super_token");
    assert_eq!(args.save_file, Some(PathBuf::from("my_save.json")));
    assert_eq!(args.autosave_sec, 45);
    assert_eq!(args.heartbeat_timeout_secs, 30);
    assert_eq!(args.snapshot_heartbeat_ms, 500);
    assert_eq!(args.web_dir, None);

    // Short flag `-w` and `--log-level trace` verification
    let short_args = ServerArgs::try_parse_from(["textcraft-server", "-w", "--log-level", "trace"])
        .expect("Short -w flag should parse");
    assert!(short_args.watch);
    assert_eq!(short_args.log_level, LogLevel::Trace);

    // 2. Default arguments verification
    let default_args =
        ServerArgs::try_parse_from(["textcraft-server"]).expect("Default args parsing should succeed");
    assert_eq!(default_args.host, "0.0.0.0");
    assert_eq!(default_args.port, 8081);
    assert_eq!(default_args.scripts_dir, PathBuf::from("scripts"));
    assert!(!default_args.watch);
    assert_eq!(default_args.log_level, LogLevel::Info);
    assert_eq!(default_args.admin_token, "admin");
    assert_eq!(default_args.autosave_sec, 30);
    assert_eq!(default_args.heartbeat_timeout_secs, 30);
    assert!(default_args.save_file.is_none());
    assert_eq!(default_args.snapshot_heartbeat_ms, 1000);
    assert_eq!(default_args.web_dir, None);

    // 3. Optional --web-dir and --heartbeat-timeout-secs argument parsing
    let args_web = ServerArgs::try_parse_from([
        "textcraft-server",
        "--web-dir",
        "/custom/static/dir",
        "--heartbeat-timeout-secs",
        "15",
    ])
    .expect("CLI args parsing with --web-dir should succeed");
    assert_eq!(args_web.web_dir, Some(PathBuf::from("/custom/static/dir")));
    assert_eq!(args_web.heartbeat_timeout_secs, 15);

    // 4. ServerConfig construction from parsed CLI args
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    assert_eq!(config.bind_addr, addr);
}

/// Test 2: HTTP asset serving: GET `/` and GET `/index.html` return the embedded Hybrid Web Terminal HTML.
#[tokio::test]
async fn test_2_http_asset_serving_embedded_web_terminal() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    let local_addr = server.local_addr();

    // 1. Test GET /
    let mut stream = TcpStream::connect(local_addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let resp_str = String::from_utf8_lossy(&response);

    assert!(resp_str.contains("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("content-type: text/html; charset=utf-8"));
    assert!(resp_str.contains("<pre id=\"screen\">"));
    assert!(resp_str.contains("id=\"hud-overlay\""));
    assert!(resp_str.contains("id=\"status-card\""));
    assert!(resp_str.contains("id=\"hp-bar\""));
    assert!(resp_str.contains("id=\"mp-bar\""));

    // 2. Test GET /index.html
    let mut stream2 = TcpStream::connect(local_addr).await.unwrap();
    stream2
        .write_all(b"GET /index.html HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut response2 = Vec::new();
    stream2.read_to_end(&mut response2).await.unwrap();
    let resp_str2 = String::from_utf8_lossy(&response2);

    assert!(resp_str2.contains("HTTP/1.1 200 OK"));
    assert!(resp_str2.contains("content-type: text/html; charset=utf-8"));
    assert!(resp_str2.contains("<pre id=\"screen\">"));
    assert!(resp_str2.contains("id=\"hud-overlay\""));
    assert!(resp_str2.contains("id=\"status-card\""));
    assert!(resp_str2.contains("id=\"hp-bar\""));
    assert!(resp_str2.contains("id=\"mp-bar\""));

    server.handle.shutdown();
}

/// Test 2b: HTTP asset serving with custom web directory overrides embedded assets.
#[tokio::test]
async fn test_2b_http_asset_serving_custom_web_directory() {
    let scripts_dir = get_scripts_dir();
    let temp_dir = std::env::temp_dir().join(format!(
        "textcraft_test_web_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let custom_html = "<!DOCTYPE html><html><body><h1>Custom Web Assets</h1></body></html>";
    tokio::fs::write(temp_dir.join("index.html"), custom_html)
        .await
        .unwrap();

    let args = ServerArgs {
        host: "127.0.0.1".to_string(),
        port: 0,
        scripts_dir,
        watch: false,
        log_level: LogLevel::Info,
        time_scale: 1.0,
        admin_token: "admin".to_string(),
        save_file: None,
        autosave_sec: 0,
        heartbeat_timeout_secs: 30,
        snapshot_heartbeat_ms: 1000,
        mcp_port: None,
        web_dir: Some(temp_dir.clone()),
    };

    let server = run_server_with_handle(args).await.unwrap();
    let local_addr = server.local_addr();

    let mut stream = TcpStream::connect(local_addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let resp_str = String::from_utf8_lossy(&response);

    assert!(resp_str.contains("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("Custom Web Assets"));

    server.handle.shutdown();
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

/// Test 2c: HTTP asset serving with web_dir: None auto-detects local web/ directory if present.
#[tokio::test]
async fn test_2c_http_asset_serving_local_web_auto_detection() {
    let scripts_dir = get_scripts_dir();
    let args = ServerArgs {
        host: "127.0.0.1".to_string(),
        port: 0,
        scripts_dir,
        watch: false,
        log_level: LogLevel::Info,
        time_scale: 1.0,
        admin_token: "admin".to_string(),
        save_file: None,
        autosave_sec: 0,
        heartbeat_timeout_secs: 30,
        snapshot_heartbeat_ms: 1000,
        mcp_port: None,
        web_dir: None,
    };

    let server = run_server_with_handle(args).await.unwrap();
    let local_addr = server.local_addr();

    let mut stream = TcpStream::connect(local_addr).await.unwrap();
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let resp_str = String::from_utf8_lossy(&response);

    assert!(resp_str.contains("HTTP/1.1 200 OK"));
    assert!(resp_str.contains("TextCraft"));

    server.handle.shutdown();
}

/// Test 3: WebSocket client join, welcome event received, initial snapshot received with cols, rows, screen (ANSI), and hud (hp, mp, inventory, logs).
#[tokio::test]
async fn test_3_websocket_client_join_and_initial_snapshot() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();
    let local_addr = server.local_addr();

    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        let mut last_tick = std::time::Instant::now();
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    let now = std::time::Instant::now();
                    let dt = now.duration_since(last_tick).as_secs_f32();
                    last_tick = now;
                    let _ = server.tick(dt);
                }
            }
        }
    });

    let ws_url = format!("ws://{}/ws", local_addr);
    let (mut ws_stream, _) = connect_async(&ws_url).await.unwrap();

    // Send Auth
    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "player_join_test".to_string(),
        client_version: "3.0.0".to_string(),
    };
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    // 1. Verify AuthSuccess
    let msg1 = ws_stream.next().await.unwrap().unwrap();
    let env1: ServerEnvelope<PlayerSnapshot, TextCraftEvent> = match msg1 {
        Message::Text(t) => DualCodec::decode_text(&t).unwrap(),
        _ => panic!("Expected text message"),
    };
    let player_id = match env1 {
        ServerEnvelope::AuthSuccess { player_id, .. } => {
            assert!(player_id > 0);
            player_id
        }
        other => panic!("Expected AuthSuccess, got {:?}", other),
    };

    // 2. Verify Welcome event
    let msg2 = ws_stream.next().await.unwrap().unwrap();
    let env2: ServerEnvelope<PlayerSnapshot, TextCraftEvent> = match msg2 {
        Message::Text(t) => DualCodec::decode_text(&t).unwrap(),
        _ => panic!("Expected text message"),
    };
    match env2 {
        ServerEnvelope::Event(TextCraftEvent::Welcome {
            player_id: pid,
            message,
        }) => {
            assert_eq!(pid, player_id);
            assert!(message.contains("Welcome to TextCraft 3.0"));
        }
        other => panic!("Expected Welcome event, got {:?}", other),
    }

    // 3. Verify Snapshot received with cols, rows, screen (ANSI), and hud (hp, mp, inventory, logs)
    let msg3 = ws_stream.next().await.unwrap().unwrap();
    let env3: ServerEnvelope<PlayerSnapshot, TextCraftEvent> = match msg3 {
        Message::Text(t) => DualCodec::decode_text(&t).unwrap(),
        _ => panic!("Expected text message"),
    };
    match env3 {
        ServerEnvelope::Snapshot(snap) => {
            assert_eq!(snap.cols, 80);
            assert_eq!(snap.rows, 24);
            assert!(!snap.screen.is_empty(), "Screen ANSI text should not be empty");
            // Check hud fields: hp, mp, inventory, logs
            assert!(snap.hud.get("hp").is_some(), "HUD should contain hp");
            assert_eq!(snap.hud["hp"], 100);
            assert!(snap.hud.get("mp").is_some(), "HUD should contain mp");
            assert_eq!(snap.hud["mp"], 50);
            assert!(
                snap.hud.get("inventory").is_some(),
                "HUD should contain inventory"
            );
            assert!(snap.hud["inventory"].is_array(), "inventory should be an array");
            assert!(snap.hud.get("logs").is_some(), "HUD should contain logs");
            assert!(snap.hud["logs"].is_array(), "logs should be an array");
        }
        other => panic!("Expected Snapshot, got {:?}", other),
    }

    let _ = stop_tx.send(());
    let _ = server_task.await;
}

/// Test 4: Player movement (`PlayerInput::Key { code: "KeyD", ... }`) updates player position in ANSI screen.
#[tokio::test]
async fn test_4_player_movement_updates_ansi_screen() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();
    let local_addr = server.local_addr();

    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        let mut last_tick = std::time::Instant::now();
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    let now = std::time::Instant::now();
                    let dt = now.duration_since(last_tick).as_secs_f32();
                    last_tick = now;
                    let _ = server.tick(dt);
                }
            }
        }
    });

    let ws_url = format!("ws://{}/ws", local_addr);
    let (mut ws_stream, _) = connect_async(&ws_url).await.unwrap();

    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "movement_tester".to_string(),
        client_version: "3.0.0".to_string(),
    };
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    // Consume AuthSuccess & Welcome
    let _ = ws_stream.next().await.unwrap().unwrap();
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Receive initial snapshot
    let initial_snap = match ws_stream.next().await.unwrap().unwrap() {
        Message::Text(t) => {
            let env: ServerEnvelope<PlayerSnapshot, TextCraftEvent> =
                DualCodec::decode_text(&t).unwrap();
            match env {
                ServerEnvelope::Snapshot(s) => s,
                other => panic!("Expected initial Snapshot, got {:?}", other),
            }
        }
        _ => panic!("Expected text message"),
    };

    // Send KeyD input (move East)
    let move_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Key {
        code: "KeyD".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    });
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&move_cmd).unwrap()))
        .await
        .unwrap();

    // Wait for subsequent snapshot reflecting new position and screen
    let mut updated_screen = None;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = ws_stream.next().await {
            if let Ok(ServerEnvelope::Snapshot(snap)) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if snap.screen != initial_snap.screen {
                    updated_screen = Some(snap);
                    break;
                }
            }
        }
    }

    assert!(
        updated_screen.is_some(),
        "Player movement KeyD should update the ANSI screen rasterization"
    );

    // Query position via (where) command to ensure player moved to (1, 0)
    let where_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: "(where)".to_string(),
    });
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&where_cmd).unwrap()))
        .await
        .unwrap();

    let mut where_confirmed = false;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = ws_stream.next().await {
            if let Ok(ServerEnvelope::Snapshot(snap)) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if let Some(logs) = snap.hud["logs"].as_array() {
                    for entry in logs {
                        if let Some(text) = entry["text"].as_str() {
                            if text.contains("(1, 0)") {
                                where_confirmed = true;
                                break;
                            }
                        }
                    }
                }
            }
            if where_confirmed {
                break;
            }
        }
    }
    assert!(where_confirmed, "Player position (1, 0) confirmed in logs");

    let _ = stop_tx.send(());
    let _ = server_task.await;
}

/// Test 5: UI Action (`PlayerInput::UIAction { action: "use_item", payload: ... }`) consumes potion, restores HP, logs combat event.
#[tokio::test]
async fn test_5_ui_action_consume_potion_restores_hp() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server =
        TextCraftServer::start_with_config(config, logic, "admin_pass".to_string(), 1.0, 0)
            .await
            .unwrap();
    let local_addr = server.local_addr();

    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        let mut last_tick = std::time::Instant::now();
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    let now = std::time::Instant::now();
                    let dt = now.duration_since(last_tick).as_secs_f32();
                    last_tick = now;
                    let _ = server.tick(dt);
                }
            }
        }
    });

    let ws_url = format!("ws://{}/ws", local_addr);
    let (mut ws_stream, _) = connect_async(&ws_url).await.unwrap();

    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "admin_pass".to_string(),
        client_version: "3.0.0".to_string(),
    };
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    // Consume AuthSuccess & Welcome
    let auth_resp = ws_stream.next().await.unwrap().unwrap();
    let pid = match match auth_resp {
        Message::Text(t) => {
            DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t).unwrap()
        }
        _ => panic!("Expected text"),
    } {
        ServerEnvelope::AuthSuccess { player_id, .. } => player_id,
        _ => panic!("Expected AuthSuccess"),
    };
    let _ = ws_stream.next().await.unwrap().unwrap(); // Welcome
    let _ = ws_stream.next().await.unwrap().unwrap(); // Initial snapshot

    // Step 1: Reduce player's HP to 50 via REPL to test healing
    let set_hp_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: format!(
            "! (set-player {} (hash-insert (ensure-player {}) \"hp\" 50))",
            pid, pid
        ),
    });
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&set_hp_cmd).unwrap()))
        .await
        .unwrap();

    // Verify HP became 50
    let mut hp_reduced = false;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = ws_stream.next().await {
            if let Ok(ServerEnvelope::Snapshot(snap)) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if snap.hud["hp"] == 50 {
                    hp_reduced = true;
                    break;
                }
            }
        }
    }
    assert!(
        hp_reduced,
        "HP should be reduced to 50 prior to potion consumption"
    );

    // Step 2: Send UIAction to consume potion
    let ui_action = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::UIAction {
        action: "use_item".to_string(),
        payload: serde_json::json!({ "id": "potion" }),
    });
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&ui_action).unwrap()))
        .await
        .unwrap();

    // Step 3: Verify HP restored to 75, potion count decremented to 2, and combat log recorded
    let mut action_verified = false;
    for _ in 0..15 {
        if let Some(Ok(Message::Text(t))) = ws_stream.next().await {
            if let Ok(ServerEnvelope::Snapshot(snap)) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                let hp = snap.hud["hp"].as_i64().unwrap_or(0);
                let inv = snap.hud["inventory"].as_array();
                let potion_count = inv.and_then(|items| {
                    items
                        .iter()
                        .find(|it| it["id"] == "potion")
                        .and_then(|p| p["count"].as_i64())
                });
                let has_combat_log = snap.hud["logs"].as_array().is_some_and(|logs| {
                    logs.iter().any(|l| {
                        l["channel"] == "combat"
                            && l["text"]
                                .as_str()
                                .is_some_and(|s| s.contains("恢复了 25 点生命值"))
                    })
                });

                if hp == 75 && potion_count == Some(2) && has_combat_log {
                    action_verified = true;
                    break;
                }
            }
        }
    }
    assert!(
        action_verified,
        "UIAction use_item should consume potion, restore HP to 75, and log combat event"
    );

    let _ = stop_tx.send(());
    let _ = server_task.await;
}

/// Test 6: Scheme dynamic FPS query and adaptive scheduler verification.
#[test]
fn test_6_scheme_dynamic_fps_query_and_adaptive_scheduler() {
    let scripts_dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    // 1. Initial target FPS is 60 (active state)
    let fps_init = logic.query_target_fps();
    assert_eq!(fps_init, 60);

    let tick_duration_60 = Duration::from_micros(1_000_000 / fps_init);
    assert_eq!(tick_duration_60.as_millis(), 16);

    // 2. Inactivity > 2.0s degrades target FPS to 30
    logic.on_tick(2.1, 1).expect("tick 2.1s should succeed");
    let fps_idle2 = logic.query_target_fps();
    assert_eq!(fps_idle2, 30);

    let tick_duration_30 = Duration::from_micros(1_000_000 / fps_idle2);
    assert_eq!(tick_duration_30.as_millis(), 33);

    // 3. Inactivity > 5.0s degrades target FPS to 15
    logic
        .on_tick(3.0, 2)
        .expect("tick additional 3.0s should succeed");
    let fps_idle5 = logic.query_target_fps();
    assert_eq!(fps_idle5, 15);

    let tick_duration_15 = Duration::from_micros(1_000_000 / fps_idle5);
    assert_eq!(tick_duration_15.as_millis(), 66);

    // 4. Player input resets activity timer and restores target FPS to 60
    logic.on_player_join(1, "test_token").unwrap();
    let key = PlayerInput::Key {
        code: "KeyW".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    };
    logic.on_input(1, &key).expect("input should succeed");
    let fps_wake = logic.query_target_fps();
    assert_eq!(fps_wake, 60);

    // 5. Explicit command sets target FPS
    logic
        .on_command(1, "(fps 45)")
        .expect("command (fps 45) should succeed");
    let fps_custom = logic.query_target_fps();
    assert_eq!(fps_custom, 45);

    // 6. Adaptive scheduler sleep calculation verification
    for target_fps in [60, 45, 30, 15] {
        let tick_dur = Duration::from_micros(1_000_000 / target_fps);
        let simulated_compute = Duration::from_millis(2);
        let sleep_dur = tick_dur.saturating_sub(simulated_compute);
        assert!(sleep_dur < tick_dur);
        assert!(sleep_dur.as_micros() > 0);
    }
}

/// Test 7: Admin REPL evaluation (`!`) with admin token succeeds; unauthenticated player rejected.
#[tokio::test]
async fn test_7_admin_repl_evaluation_and_authorization() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(
        config,
        logic,
        "super_secret_admin_token".to_string(),
        1.0,
        0,
    )
    .await
    .unwrap();
    let local_addr = server.local_addr();

    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        let mut last_tick = std::time::Instant::now();
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    let now = std::time::Instant::now();
                    let dt = now.duration_since(last_tick).as_secs_f32();
                    last_tick = now;
                    let _ = server.tick(dt);
                }
            }
        }
    });

    let ws_url = format!("ws://{}/ws", local_addr);

    // Part A: Admin client with admin token
    let (mut admin_ws, _) = connect_async(&ws_url).await.unwrap();
    let admin_auth = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "super_secret_admin_token".to_string(),
        client_version: "3.0.0".to_string(),
    };
    admin_ws
        .send(Message::Text(DualCodec::encode_text(&admin_auth).unwrap()))
        .await
        .unwrap();

    // Consume AuthSuccess & Welcome
    let _ = admin_ws.next().await.unwrap().unwrap();
    let _ = admin_ws.next().await.unwrap().unwrap();

    // Send Admin REPL command: `! (+ 1000 337)`
    let repl_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: "! (+ 1000 337)".to_string(),
    });
    admin_ws
        .send(Message::Text(DualCodec::encode_text(&repl_cmd).unwrap()))
        .await
        .unwrap();

    let mut admin_eval_succeeded = false;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = admin_ws.next().await {
            if let Ok(ServerEnvelope::Event(TextCraftEvent::Message { channel, text })) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if channel == "repl" && text.contains("1337") {
                    admin_eval_succeeded = true;
                    break;
                }
            }
        }
    }
    assert!(
        admin_eval_succeeded,
        "Admin REPL eval should evaluate (+ 1000 337) => 1337"
    );

    // Part B: Regular client with non-admin token
    let (mut guest_ws, _) = connect_async(&ws_url).await.unwrap();
    let guest_auth = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "guest_player_token".to_string(),
        client_version: "3.0.0".to_string(),
    };
    guest_ws
        .send(Message::Text(DualCodec::encode_text(&guest_auth).unwrap()))
        .await
        .unwrap();

    // Consume AuthSuccess & Welcome
    let _ = guest_ws.next().await.unwrap().unwrap();
    let _ = guest_ws.next().await.unwrap().unwrap();

    // Regular user attempts REPL command
    let forbidden_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: "! (* 7 8)".to_string(),
    });
    guest_ws
        .send(Message::Text(DualCodec::encode_text(&forbidden_cmd).unwrap()))
        .await
        .unwrap();

    let mut guest_rejected = false;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = guest_ws.next().await {
            if let Ok(ServerEnvelope::Event(TextCraftEvent::Error { text })) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if text.contains("Permission denied: admin token required") {
                    guest_rejected = true;
                    break;
                }
            }
        }
    }
    assert!(
        guest_rejected,
        "Non-admin REPL request must be rejected with Permission denied"
    );

    let _ = stop_tx.send(());
    let _ = server_task.await;
}

/// Test 8: State persistence: save file written on shutdown/autosave and successfully restored on restart.
#[tokio::test]
async fn test_8_state_persistence_shutdown_and_restart() {
    let scripts_dir = get_scripts_dir();
    let temp_dir = std::env::temp_dir().join(format!(
        "textcraft_persist_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let save_file = temp_dir.join("save_state.json");

    // 1. First Server session: modify state and save
    {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
        let mut logic = SchemeLogic::new(&scripts_dir, false, Some(save_file.clone())).unwrap();
        logic.init().unwrap();

        let mut server =
            TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
                .await
                .unwrap();

        // Connect a player and modify state
        server.logic.on_player_join(42, "persisted_token").unwrap();
        // Move player to (5, 5) and set gold to 888
        server
            .logic
            .eval_repl(
                "(set-player 42 (hash-insert (hash-insert (ensure-player 42) \"x\" 5) \"y\" 5))",
            )
            .unwrap();
        server
            .logic
            .eval_repl("(set-player 42 (hash-insert (ensure-player 42) \"gold\" 888))")
            .unwrap();

        // Save state explicitly (simulating autosave/shutdown)
        server.logic.save_state_to_file().unwrap();
        assert!(save_file.exists(), "Save file should be written to disk");

        // Verify JSON contents on disk
        let file_content = std::fs::read_to_string(&save_file).unwrap();
        let json_val: serde_json::Value = serde_json::from_str(&file_content).unwrap();
        assert_eq!(json_val["players"]["42"]["gold"], 888);
        assert_eq!(json_val["players"]["42"]["x"], 5);
        assert_eq!(json_val["players"]["42"]["y"], 5);

        // Server handle shuts down
        server.handle.shutdown();
    }

    // 2. Second Server session: restart with existing save file and verify state restoration
    {
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
        let mut logic2 = SchemeLogic::new(&scripts_dir, false, Some(save_file.clone())).unwrap();
        logic2.init().unwrap();

        let mut server2 =
            TextCraftServer::start_with_config(config, logic2, "admin".to_string(), 1.0, 0)
                .await
                .unwrap();

        // Verify that Player 42's state was restored
        let exported = server2.logic.export_state().unwrap();
        assert_eq!(exported["players"]["42"]["gold"], 888);
        assert_eq!(exported["players"]["42"]["x"], 5);
        assert_eq!(exported["players"]["42"]["y"], 5);

        // Verify that snapshot renders from restored state
        let snap = server2.logic.render_snapshot(42).unwrap();
        assert_eq!(snap.hud["gold"], 888);

        server2.handle.shutdown();
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
}
