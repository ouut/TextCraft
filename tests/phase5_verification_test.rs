use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, ServerEnvelope};
use game_server::ServerConfig;
use serde_json::json;
use textcraft::{
    OutboundMessage, PlayerInput, PlayerSnapshot, SchemeLogic, TextCraftCommand, TextCraftEvent,
    TextCraftServer, EMBEDDED_WEB_ASSETS,
};
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

async fn read_next_envelope(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Option<ServerEnvelope<PlayerSnapshot, TextCraftEvent>> {
    while let Some(msg_res) = ws.next().await {
        match msg_res {
            Ok(Message::Text(text)) => {
                if let Ok(env) =
                    DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&text)
                {
                    return Some(env);
                }
            }
            Ok(Message::Binary(bin)) => {
                if let Ok(env) =
                    DualCodec::decode_binary::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&bin)
                {
                    return Some(env);
                }
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            _ => return None,
        }
    }
    None
}

// ============================================================================
// Task 5.1: 会话断线重连与 Token 持久化测试
// ============================================================================

/// Task 5.1 (Part A): 验证玩家（含 Admin）在 `PlayerDisconnected` 宽限期内角色未被删除，
/// 重连后恢复 Token 权限并立即收到快照。
#[tokio::test]
async fn test_task_5_1_admin_disconnect_grace_period_and_reconnect() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let admin_secret = "admin_reconnect_token_secret".to_string();
    let mut server =
        TextCraftServer::start_with_config(config, logic, admin_secret.clone(), 1.0, 0)
            .await
            .unwrap()
            .with_snapshot_heartbeat_ms(5000); // Long heartbeat to prove immediate snapshot on reconnect

    let local_addr = server.local_addr();
    let ws_url = format!("ws://{}/ws", local_addr);

    // 1. First connection with Admin token
    let (mut ws1, _) = connect_async(&ws_url).await.unwrap();
    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: admin_secret.clone(),
        client_version: "3.0.0".to_string(),
    };
    ws1.send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    // Wait briefly for network ingestion and run server tick
    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    let mut first_pid = None;
    let mut got_initial_snap = false;
    for _ in 0..5 {
        if let Ok(Some(env)) =
            tokio::time::timeout(Duration::from_millis(200), read_next_envelope(&mut ws1)).await
        {
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => first_pid = Some(player_id),
                ServerEnvelope::Snapshot(_) => got_initial_snap = true,
                _ => {}
            }
        }
        if first_pid.is_some() && got_initial_snap {
            break;
        }
    }
    let pid = first_pid.expect("Should receive AuthSuccess with player_id");
    assert!(got_initial_snap, "Should receive initial snapshot");

    // 2. Move player East (KeyD) and set custom gold via Admin REPL in separate ticks
    let move_input = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Key {
        code: "KeyD".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    });
    ws1.send(Message::Text(DualCodec::encode_text(&move_input).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    let repl_input = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: format!(
            "! (set-player {} (hash-insert (ensure-player {}) \"gold\" 777))",
            pid, pid
        ),
    });
    ws1.send(Message::Text(DualCodec::encode_text(&repl_input).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    // Verify player state in *world-state* before disconnect
    let state_before = server.logic.export_state().unwrap();
    let pid_str = pid.to_string();
    assert_eq!(state_before["players"][&pid_str]["x"], 1);
    assert_eq!(state_before["players"][&pid_str]["gold"], 777);

    // 3. Abruptly drop WebSocket connection to trigger PlayerDisconnected (entering grace period)
    drop(ws1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    server.tick(0.016).unwrap();

    // Verify disconnected character stays in `players` with `online == false` (no ghost on map for others)
    // and is also safely persisted in `saved_players` by token during the grace period!
    let state_during_grace = server.logic.export_state().unwrap();
    assert_eq!(
        state_during_grace["players"][&pid_str]["online"],
        false,
        "Disconnected player must remain in 'players' with online=false during grace period"
    );
    assert_eq!(state_during_grace["players"][&pid_str]["x"], 1);
    assert_eq!(state_during_grace["players"][&pid_str]["gold"], 777);
    assert_eq!(state_during_grace["saved_players"][&admin_secret]["x"], 1);
    assert_eq!(state_during_grace["saved_players"][&admin_secret]["gold"], 777);

    // 4. Reconnect with the same Admin token within grace period
    let (mut ws2, _) = connect_async(&ws_url).await.unwrap();
    ws2.send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    server.tick(0.016).unwrap();

    // Verify online flag is flipped back to true after reconnect
    let state_after_reconn = server.logic.export_state().unwrap();
    assert_eq!(
        state_after_reconn["players"][&pid_str]["online"],
        true,
        "Reconnected player must have online=true"
    );

    // Verify reconnected client receives AuthSuccess (same pid), Welcome back event, and immediate Snapshot
    let mut reconn_pid = None;
    let mut reconn_welcome = false;
    let mut reconn_snap = None;

    for _ in 0..6 {
        if let Ok(Some(env)) =
            tokio::time::timeout(Duration::from_millis(250), read_next_envelope(&mut ws2)).await
        {
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => {
                    reconn_pid = Some(player_id);
                }
                ServerEnvelope::Event(TextCraftEvent::Welcome { message, .. }) => {
                    if message.contains("Reconnected") || message.contains("Welcome") {
                        reconn_welcome = true;
                    }
                }
                ServerEnvelope::Snapshot(snap) => {
                    reconn_snap = Some(snap);
                }
                _ => {}
            }
        }
        if reconn_pid.is_some() && reconn_welcome && reconn_snap.is_some() {
            break;
        }
    }

    assert_eq!(reconn_pid, Some(pid), "Reconnected player_id should match");
    assert!(reconn_welcome, "Should receive Welcome event on reconnect");
    let snap = reconn_snap.expect("Should immediately receive Snapshot on reconnect");
    assert_eq!(snap.hud["gold"], 777, "Restored snapshot HUD should have gold=777");

    // 5. Verify Admin token privileges are preserved after reconnect
    let admin_check = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: "! (+ 2000 26)".to_string(),
    });
    ws2.send(Message::Text(DualCodec::encode_text(&admin_check).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    let mut admin_eval_ok = false;
    for _ in 0..5 {
        if let Ok(Some(ServerEnvelope::Event(TextCraftEvent::Message { channel, text }))) =
            tokio::time::timeout(Duration::from_millis(200), read_next_envelope(&mut ws2)).await
        {
            if channel == "repl" && text.contains("2026") {
                admin_eval_ok = true;
                break;
            }
        }
    }
    assert!(
        admin_eval_ok,
        "Admin REPL privilege must still work after reconnection"
    );

    server.handle.shutdown();
}

/// Task 5.1 (Part B): 验证玩家断线终止（`on_player_leave`）后，再次使用相同非空 `token` 加入（`on_player_join`）时，
/// 能够从 `"saved_players"` 自动恢复之前的坐标、金币和背包状态。
#[test]
fn test_task_5_1_token_persistence_across_leave_and_rejoin() {
    let scripts_dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let token = "hero_persistent_token_001";

    // 1. Player joins with pid = 10 and non-empty token
    logic.on_player_join(10, token).unwrap();

    // Move player to (1, -1) using KeyD and KeyW
    logic
        .on_input(
            10,
            &PlayerInput::Key {
                code: "KeyD".to_string(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        )
        .unwrap();
    logic
        .on_input(
            10,
            &PlayerInput::Key {
                code: "KeyW".to_string(),
                shift: false,
                ctrl: false,
                alt: false,
            },
        )
        .unwrap();

    // Consume 1 potion so inventory potion count becomes 2, and set gold to 350
    logic
        .on_ui_action(10, "use_item", &json!({ "id": "potion" }))
        .unwrap();
    logic
        .eval_repl("(set-player 10 (hash-insert (ensure-player 10) \"gold\" 350))")
        .unwrap();

    // Verify current state before leaving
    let state1 = logic.export_state().unwrap();
    assert_eq!(state1["players"]["10"]["x"], 1);
    assert_eq!(state1["players"]["10"]["y"], -1);
    assert_eq!(state1["players"]["10"]["gold"], 350);

    // 2. Session terminates: call `on_player_leave(10)`
    logic.on_player_leave(10).unwrap();

    let state_after_leave = logic.export_state().unwrap();
    assert!(
        state_after_leave["players"].get("10").is_none(),
        "Active player 10 should be removed from 'players' after on_player_leave"
    );
    assert!(
        state_after_leave["saved_players"].get(token).is_some(),
        "Player data must be persisted in 'saved_players' under token"
    );
    assert_eq!(state_after_leave["saved_players"][token]["x"], 1);
    assert_eq!(state_after_leave["saved_players"][token]["y"], -1);
    assert_eq!(state_after_leave["saved_players"][token]["gold"], 350);

    // 3. Player rejoins later with a new player_id (pid = 99) but the SAME token
    logic.on_player_join(99, token).unwrap();

    let state_rejoined = logic.export_state().unwrap();
    let p99 = &state_rejoined["players"]["99"];
    assert_eq!(p99["id"], 99);
    assert_eq!(p99["token"], token);
    assert_eq!(p99["x"], 1, "X coordinate should be restored from saved_players");
    assert_eq!(p99["y"], -1, "Y coordinate should be restored from saved_players");
    assert_eq!(p99["gold"], 350, "Gold should be restored from saved_players");

    // Verify inventory potion count was restored to 2
    let inv = p99["inventory"].as_array().expect("inventory should be array");
    let potion_count = inv
        .iter()
        .find(|item| item["id"] == "potion")
        .and_then(|item| item["count"].as_i64());
    assert_eq!(
        potion_count,
        Some(2),
        "Inventory potion count (2) should be restored from saved_players"
    );
}

// ============================================================================
// Task 5.2: 安全防注入与前后端协议契约测试
// ============================================================================

/// Task 5.2 (Part A): 验证恶意命令输入 `logic.on_command(1, "1) (let loop () (loop)")`
/// 和 `logic.on_command(1, "help (let loop () (loop))")` 不会触发代码求值或卡死主线程（耗时远小于 50ms）。
#[test]
fn test_task_5_2_command_injection_prevention_and_safety() {
    let scripts_dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();
    logic.on_player_join(1, "sec_tester").unwrap();

    // 1. Test injection payload 1: `1) (let loop () (loop)`
    let start1 = Instant::now();
    let res1 = logic.on_command(1, "1) (let loop () (loop)");
    let elapsed1 = start1.elapsed();
    assert!(res1.is_ok(), "Malicious command 1 should be handled safely");
    assert!(
        elapsed1 < Duration::from_millis(50),
        "on_command with payload 1 took {:?}, expected < 50ms (no infinite loop eval)",
        elapsed1
    );

    // 2. Test injection payload 2: `help (let loop () (loop))`
    let start2 = Instant::now();
    let res2 = logic.on_command(1, "help (let loop () (loop))");
    let elapsed2 = start2.elapsed();
    assert!(res2.is_ok(), "Malicious command 2 should be handled safely");
    assert!(
        elapsed2 < Duration::from_millis(50),
        "on_command with payload 2 took {:?}, expected < 50ms (no infinite loop eval)",
        elapsed2
    );
}

/// Task 5.2 (Part B): 验证与商人 NPC 交互后 `hud["dialog"]` 为包含 `"title"`、`"text"`、`"options"` 的对象，
/// 测试 `"buy_potion"` 购买药水扣减金币、以及 `"dialog_close"` / `"close_dialog"` / `"dialog_confirm"` 均能将 `"dialog"` 关闭。
#[test]
fn test_task_5_2_merchant_dialog_contract_and_actions() {
    let scripts_dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();
    logic.on_player_join(1, "dialog_tester").unwrap();

    // Merchant is at (2, -5) in init-demo-world.
    // Place player 1 at (2, -4) and move North (KeyW) to interact with Merchant at (2, -5).
    logic
        .eval_repl("(set-player 1 (hash-insert (hash-insert (ensure-player 1) \"x\" 2) \"y\" -4))")
        .unwrap();

    let move_north = PlayerInput::Key {
        code: "KeyW".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    };

    // 1. Interact with merchant -> verify dialog object structure
    logic.on_input(1, &move_north).unwrap();
    let snap = logic.render_snapshot(1).unwrap();
    let dialog = &snap.hud["dialog"];
    assert!(
        dialog.is_object(),
        "hud['dialog'] must be a structured JSON object, got: {:?}",
        dialog
    );
    assert_eq!(dialog["title"], "神秘商人");
    assert!(
        dialog["text"].as_str().is_some_and(|t| !t.is_empty()),
        "dialog['text'] should be a non-empty string"
    );
    let options = dialog["options"]
        .as_array()
        .expect("dialog['options'] must be an array");
    assert!(
        options.iter().any(|o| o["action"] == "buy_potion"),
        "dialog['options'] should contain buy_potion"
    );
    assert!(
        options.iter().any(|o| o["action"] == "close_dialog"),
        "dialog['options'] should contain close_dialog"
    );

    // 2. Test "buy_potion" UI action: deducts 25 gold (50 -> 25), increments potion count (3 -> 4), closes dialog
    logic.on_ui_action(1, "buy_potion", &json!(null)).unwrap();
    let snap_after_buy = logic.render_snapshot(1).unwrap();
    assert_eq!(snap_after_buy.hud["gold"], 25, "Buying potion should deduct 25 gold");
    assert_eq!(
        snap_after_buy.hud["dialog"],
        json!(false),
        "Dialog should close after buying potion"
    );
    let potion_count = snap_after_buy.hud["inventory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|it| it["id"] == "potion")
        .and_then(|it| it["count"].as_i64());
    assert_eq!(potion_count, Some(4), "Potion count should increase from 3 to 4");

    // 3. Test closing dialog via all 3 close action variants: "dialog_close", "close_dialog", "dialog_confirm"
    for close_action in ["dialog_close", "close_dialog", "dialog_confirm"] {
        // Re-open dialog by interacting with Merchant
        logic.on_input(1, &move_north).unwrap();
        let snap_open = logic.render_snapshot(1).unwrap();
        assert!(
            snap_open.hud["dialog"].is_object(),
            "Dialog should be open before testing {}",
            close_action
        );

        // Send close action
        logic.on_ui_action(1, close_action, &json!(null)).unwrap();
        let snap_closed = logic.render_snapshot(1).unwrap();
        assert_eq!(
            snap_closed.hud["dialog"],
            json!(false),
            "Action '{}' must set hud['dialog'] to false",
            close_action
        );
    }
}

/// Task 5.2 (Part C): 验证 `"set_theme"`、`"clear_logs"` UI 动作以及 `(say "hello")` 跨玩家广播日志与 `OutboundMessage::Broadcast` 生效。
#[test]
fn test_task_5_2_ui_sync_and_say_broadcast() {
    let scripts_dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    logic.on_player_join(1, "tok_p1").unwrap();
    logic.on_player_join(2, "tok_p2").unwrap();

    // 1. Test "set_theme" UI action
    logic
        .on_ui_action(1, "set_theme", &json!({ "theme": "amber" }))
        .unwrap();
    let snap1 = logic.render_snapshot(1).unwrap();
    assert_eq!(snap1.hud["theme"], "amber");

    // 2. Test "clear_logs" UI action
    assert!(
        !snap1.hud["logs"].as_array().unwrap().is_empty(),
        "Logs should not be empty initially"
    );
    logic.on_ui_action(1, "clear_logs", &json!(null)).unwrap();
    let snap1_cleared = logic.render_snapshot(1).unwrap();
    assert!(
        snap1_cleared.hud["logs"].as_array().unwrap().is_empty(),
        "clear_logs should empty player 1's logs"
    );

    // 3. Test `(say "hello")` cross-player log broadcast & OutboundMessage::Broadcast
    let _ = logic.drain_outbound_messages(); // clear any prior messages
    logic.on_command(1, "(say \"hello\")").unwrap();

    // Check both Player 1 and Player 2 have the chat message in their HUD logs
    for pid in [1, 2] {
        let snap = logic.render_snapshot(pid).unwrap();
        let has_chat = snap.hud["logs"].as_array().unwrap().iter().any(|entry| {
            entry["channel"] == "chat"
                && entry["text"]
                    .as_str()
                    .is_some_and(|t| (t.contains("Hero_1") || t.contains("Hero#1")) && t.contains("hello"))
        });
        assert!(
            has_chat,
            "Player {} HUD logs should contain broadcast chat message",
            pid
        );
    }

    // Check OutboundMessage::Broadcast was emitted
    let outbound = logic.drain_outbound_messages();
    let has_broadcast = outbound.iter().any(|msg| match msg {
        OutboundMessage::Broadcast { channel, text } => {
            channel == "chat" && (text.contains("Hero_1") || text.contains("Hero#1")) && text.contains("hello")
        }
        _ => false,
    });
    assert!(
        has_broadcast,
        "Outbound queue should contain OutboundMessage::Broadcast for chat"
    );
}

/// Task 5.1 (Part C): 验证 URL 多用户 `username:token`（如 `"alice:"` 与 `"bob:admin"`）的身份解析、
/// 管理员权限校验（留空 Token 无权限、匹配 Admin Token 有权限），以及断线宽限期内 `online = #f` 杜绝其他玩家视口中的离线幽灵残影。
#[tokio::test]
async fn test_task_5_1_multi_user_url_token_routing_and_offline_ghost_filtering() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server =
        TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
            .await
            .unwrap()
            .with_snapshot_heartbeat_ms(0);

    let local_addr = server.local_addr();
    let ws_url = format!("ws://{}/ws", local_addr);

    // 1. Connect Alice (`#/alice` -> handshake token `"alice:"`) and Bob (`#/bob/admin` -> `"bob:admin"`)
    let (mut ws_alice, _) = connect_async(&ws_url).await.unwrap();
    ws_alice
        .send(Message::Text(
            DualCodec::encode_text(&ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
                token: "alice:".to_string(),
                client_version: "3.0.0".to_string(),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    let (mut ws_bob, _) = connect_async(&ws_url).await.unwrap();
    ws_bob
        .send(Message::Text(
            DualCodec::encode_text(&ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
                token: "bob:admin".to_string(),
                client_version: "3.0.0".to_string(),
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;
    server.tick(0.016).unwrap();

    let mut alice_pid = None;
    let mut alice_snap = None;
    for _ in 0..5 {
        if let Ok(Some(env)) =
            tokio::time::timeout(Duration::from_millis(200), read_next_envelope(&mut ws_alice))
                .await
        {
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => alice_pid = Some(player_id),
                ServerEnvelope::Snapshot(s) => alice_snap = Some(s),
                _ => {}
            }
        }
        if alice_pid.is_some() && alice_snap.is_some() {
            break;
        }
    }
    let alice_id = alice_pid.expect("Alice should authenticate");
    let alice_initial_snap = alice_snap.expect("Alice should receive initial snapshot");
    assert_eq!(alice_initial_snap.hud["name"], "alice");

    let mut bob_pid = None;
    let mut bob_snap = None;
    for _ in 0..5 {
        if let Ok(Some(env)) =
            tokio::time::timeout(Duration::from_millis(200), read_next_envelope(&mut ws_bob)).await
        {
            match env {
                ServerEnvelope::AuthSuccess { player_id, .. } => bob_pid = Some(player_id),
                ServerEnvelope::Snapshot(s) => bob_snap = Some(s),
                _ => {}
            }
        }
        if bob_pid.is_some() && bob_snap.is_some() {
            break;
        }
    }
    let bob_id = bob_pid.expect("Bob should authenticate");
    let bob_initial_snap = bob_snap.expect("Bob should receive initial snapshot");
    assert_eq!(bob_initial_snap.hud["name"], "bob");

    // Verify saved_players indexed by username ("alice" and "bob")
    let state = server.logic.export_state().unwrap();
    assert_eq!(state["saved_players"]["alice"]["name"], "alice");
    assert_eq!(state["saved_players"]["bob"]["name"], "bob");
    assert!(!server.is_admin_authorized(alice_id), "alice: must not have admin rights");
    assert!(server.is_admin_authorized(bob_id), "bob:admin must have admin rights");

    // 2. Move Bob East to (1, 0) so Alice at (0, 0) can see Bob's blue `@` (`\x1b[38;2;100;200;255m`)
    ws_bob
        .send(Message::Text(
            DualCodec::encode_text(&ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(
                PlayerInput::Key {
                    code: "KeyD".to_string(),
                    shift: false,
                    ctrl: false,
                    alt: false,
                },
            ))
            .unwrap(),
        ))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    let snap_alice_with_bob_online = server.logic.render_snapshot(alice_id).unwrap();
    assert!(
        snap_alice_with_bob_online
            .screen
            .contains("\x1b[38;2;100;200;255m"),
        "Online Bob at (1,0) should be rendered in Alice's viewport with other-player fg color (100,200,255)"
    );

    // 3. Bob disconnects (enters grace period -> online = false)
    drop(ws_bob);
    tokio::time::sleep(Duration::from_millis(50)).await;
    server.tick(0.016).unwrap();

    let state_grace = server.logic.export_state().unwrap();
    assert_eq!(
        state_grace["players"][&bob_id.to_string()]["online"],
        false,
        "Disconnected Bob must have online=false"
    );

    // Verify Bob's offline ghost is immediately hidden from Alice's viewport!
    let snap_alice_with_bob_offline = server.logic.render_snapshot(alice_id).unwrap();
    assert!(
        !snap_alice_with_bob_offline
            .screen
            .contains("\x1b[38;2;100;200;255m"),
        "Offline Bob in grace period must NOT appear as a ghost in Alice's viewport"
    );

    server.handle.shutdown();
}

// ============================================================================
// Task 5.3: 怪物 AI 寻路与主循环性能基准测试
// ============================================================================

/// Task 5.3 (Part A): 验证怪物 AI 在 Y 轴和 X 轴方向均能向距离最近的玩家移动并攻击。
#[test]
fn test_task_5_3_monster_ai_nearest_player_x_and_y_movement_and_attack() {
    let scripts_dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    // Join 2 players:
    // Player 1 is far away at (-5, -5)
    // Player 2 is close to goblin_1 (which starts at (7, 3)): place Player 2 at (5, 1)
    logic.on_player_join(1, "far_player").unwrap();
    logic.on_player_join(2, "near_player").unwrap();

    logic
        .eval_repl("(set-player 1 (hash-insert (hash-insert (ensure-player 1) \"x\" -5) \"y\" -5))")
        .unwrap();
    logic
        .eval_repl("(set-player 2 (hash-insert (hash-insert (ensure-player 2) \"x\" 5) \"y\" 1))")
        .unwrap();

    // Keep only goblin_1 at (7, 3) to cleanly observe its pathfinding toward Player 2 at (5, 1)
    // Manhattan distance from (7, 3) to Player 2 (5, 1) is |7-5| + |3-1| = 2 + 2 = 4.
    // Distance to Player 1 (-5, -5) is |7 - (-5)| + |3 - (-5)| = 12 + 8 = 20.
    let mut tick_id = 0u64;
    let mut run_ai_step = |logic: &mut SchemeLogic| {
        for _ in 0..30 {
            tick_id += 1;
            logic.on_tick(0.016, tick_id).unwrap();
        }
    };

    let get_goblin = |logic: &mut SchemeLogic| -> serde_json::Value {
        let state = logic.export_state().unwrap();
        state["monsters"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == "goblin_1")
            .unwrap()
            .clone()
    };

    let g0 = get_goblin(&mut logic);
    assert_eq!((g0["x"].as_i64().unwrap(), g0["y"].as_i64().unwrap()), (7, 3));

    // AI Step 1 (30 ticks): Goblin moves from (7, 3) -> (6, 3) (X-axis movement: dx = -1)
    run_ai_step(&mut logic);
    let g1 = get_goblin(&mut logic);
    let (x1, y1) = (g1["x"].as_i64().unwrap(), g1["y"].as_i64().unwrap());
    assert_eq!(
        (x1, y1),
        (6, 3),
        "Step 1: Goblin should move along X-axis from (7,3) to (6,3)"
    );

    // AI Step 2 (60 ticks): From (6, 3) toward (5, 1), prefers X-axis first -> moves to (5, 3)
    run_ai_step(&mut logic);
    let g2 = get_goblin(&mut logic);
    let (x2, y2) = (g2["x"].as_i64().unwrap(), g2["y"].as_i64().unwrap());
    assert_eq!(
        (x2, y2),
        (5, 3),
        "Step 2: Goblin should move along X-axis from (6,3) to (5,3)"
    );

    // AI Step 3 (90 ticks): From (5, 3) toward (5, 1), dx=0, dy=-1 -> moves along Y-axis to (5, 2) (distance = 1 to Player 2 at (5, 1))
    run_ai_step(&mut logic);
    let g3 = get_goblin(&mut logic);
    let (x3, y3) = (g3["x"].as_i64().unwrap(), g3["y"].as_i64().unwrap());
    assert_eq!(
        (x3, y3),
        (5, 2),
        "Step 3: Goblin should move along Y-axis from (5,3) to (5,2), adjacent (dist=1) to Player 2 at (5,1)"
    );

    // AI Step 4 (120 ticks): At distance = 1, Goblin attacks nearest Player 2!
    run_ai_step(&mut logic);
    let state_after_attack = logic.export_state().unwrap();
    let p2_hp = state_after_attack["players"]["2"]["hp"].as_i64().unwrap();
    let p1_hp = state_after_attack["players"]["1"]["hp"].as_i64().unwrap();
    assert_eq!(
        p2_hp, 94,
        "Nearest Player 2 should take 6 damage from Goblin (100 -> 94)"
    );
    assert_eq!(p1_hp, 100, "Far Player 1 should remain unharmed at 100 HP");
}

/// Task 5.3 (Part B): 验证 ANSI RLE 优化后 `render_snapshot` 的屏幕字符串长度明显紧凑，
/// 并测量 10 名在线玩家下 `server.tick(0.016)` 的平均耗时。
#[tokio::test]
async fn test_task_5_3_ansi_rle_compactness_and_10_players_tick_benchmark() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    // 1. Verify ANSI RLE compactness on a standard 80x24 viewport (1920 cells)
    logic.on_player_join(100, "bench_inspector").unwrap();
    let snap = logic.render_snapshot(100).unwrap();
    let screen_bytes = snap.screen.len();
    let reset_count = snap.screen.matches("\x1b[0m").count();

    // Without RLE, 1920 cells with full RGB fg+bg+reset took ~75,000+ bytes and ~1920 resets.
    // With RLE, adjacent cells with identical colors share escape sequences, reducing resets to < 300 and size to < 15,000 bytes.
    println!(
        "[Benchmark] 80x24 ANSI RLE screen size: {} bytes, ANSI resets: {}",
        screen_bytes, reset_count
    );
    assert!(
        screen_bytes < 15_000,
        "ANSI RLE screen size should be compact (< 15,000 bytes), got {} bytes",
        screen_bytes
    );
    assert!(
        reset_count < 300,
        "ANSI RLE reset count should be drastically reduced (< 300), got {}",
        reset_count
    );
    logic.on_player_leave(100).unwrap();

    // 2. Benchmark `server.tick(0.016)` with 10 connected WebSocket players
    let mut server =
        TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
            .await
            .unwrap()
            .with_snapshot_heartbeat_ms(1000);

    let local_addr = server.local_addr();
    let ws_url = format!("ws://{}/ws", local_addr);

    let mut clients = Vec::with_capacity(10);
    for i in 0..10 {
        let (mut ws, _) = connect_async(&ws_url).await.unwrap();
        let auth = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
            token: format!("bench_player_{}", i),
            client_version: "3.0.0".to_string(),
        };
        ws.send(Message::Text(DualCodec::encode_text(&auth).unwrap()))
            .await
            .unwrap();
        clients.push(ws);
    }

    // Wait for all 10 Auth handshakes to arrive and process initial join tick
    tokio::time::sleep(Duration::from_millis(80)).await;
    server.tick(0.016).unwrap();
    assert_eq!(
        server.handle.connected_players().len(),
        10,
        "All 10 players should be connected"
    );

    // Measure 20 ticks (frames 1..=20; note tick 29 will trigger 30th-tick monster AI)
    let iterations = 20u32;
    let start_idle = Instant::now();
    for _ in 0..iterations {
        server.tick(0.016).unwrap();
    }
    let total_idle = start_idle.elapsed();
    let avg_idle_ms = total_idle.as_secs_f64() * 1000.0 / (iterations as f64);

    // Also measure active movement frame where a player moves
    let move_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Key {
        code: "KeyD".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
    });
    clients[0]
        .send(Message::Text(DualCodec::encode_text(&move_msg).unwrap()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let start_active = Instant::now();
    server.tick(0.016).unwrap();
    let active_ms = start_active.elapsed().as_secs_f64() * 1000.0;

    println!(
        "[Benchmark] 10 online players: avg tick(0.016) = {:.3} ms (total {:.2} ms for {} ticks), active movement tick = {:.3} ms",
        avg_idle_ms,
        total_idle.as_secs_f64() * 1000.0,
        iterations,
        active_ms
    );

    // Ensure average quiescent tick time with 10 players is < 0.5ms thanks to Dirty Version short-circuiting!
    assert!(
        avg_idle_ms < 0.5,
        "10-player average quiescent tick should be < 0.5ms, got {:.3}ms",
        avg_idle_ms
    );

    server.handle.shutdown();
}


// ============================================================================
// Task 5.4: MCP HTTP JSON-RPC 2.0 管道、宿主命令桥接与 Scheme 动态工具扩展测试
// ============================================================================

async fn send_mcp_http_post(
    server: &mut TextCraftServer,
    mcp_addr: SocketAddr,
    path: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let body = serde_json::to_string(&payload).unwrap();
    let req_str = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
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
async fn test_mcp_http_server_command_bridge_and_dynamic_scheme_tools() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    // Bind MCP HTTP server on dynamic port
    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    assert_eq!(server.mcp_local_addr(), Some(mcp_addr));

    // Join player 1
    server.logic.on_player_join(1, "alice:admin").unwrap();

    // 1. Test `initialize` handshake
    let init_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }),
    )
    .await;
    assert_eq!(init_resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "textcraft-mcp");
    assert_eq!(
        init_resp["result"]["capabilities"]["tools"]["listChanged"],
        true
    );

    // 2. Test `tools/list`: verify both host bridge tools and Scheme built-in tools are present
    let list_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;
    let tools = list_resp["result"]["tools"]
        .as_array()
        .expect("tools should be an array");
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for expected in [
        "game_command",
        "player_input",
        "reload_scripts",
        "get_world_summary",
        "get_viewport",
        "teleport_player",
        "spawn_monster",
    ] {
        assert!(
            tool_names.contains(&expected),
            "tools/list should contain '{}', got {:?}",
            expected,
            tool_names
        );
    }

    // 3. Test `game_command` with normal game command `(where)` and `! (+ 40 2)` REPL evaluation
    let cmd_where_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "player_id": 1,
                    "command": "(where)"
                }
            }
        }),
    )
    .await;
    assert_eq!(cmd_where_resp["result"]["isError"], false);
    let cmd_where_text = cmd_where_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let cmd_where_json: serde_json::Value = serde_json::from_str(cmd_where_text).unwrap();
    assert!(
        !cmd_where_json["ascii_screen"]
            .as_str()
            .unwrap()
            .contains("\x1b["),
        "ascii_screen must have ANSI escape sequences stripped"
    );
    assert!(
        cmd_where_json["hud"]["logs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["text"].as_str().is_some_and(|t| t.contains("当前位置"))),
        "Player 1 HUD logs should contain (where) result"
    );

    let cmd_repl_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "command": "! (+ 40 2)"
                }
            }
        }),
    )
    .await;
    assert_eq!(cmd_repl_resp["result"]["isError"], false);
    let repl_text = cmd_repl_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        repl_text.contains("42"),
        "REPL evaluation of ! (+ 40 2) should return 42, got: {}",
        repl_text
    );

    // 4. Test Scheme built-in tools: `teleport_player` and `spawn_monster`
    let tp_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "teleport_player",
                "arguments": {
                    "player_id": 1,
                    "x": 3,
                    "y": -2
                }
            }
        }),
    )
    .await;
    assert_eq!(tp_resp["result"]["isError"], false);
    let tp_json: serde_json::Value =
        serde_json::from_str(tp_resp["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(tp_json["x"], 3);
    assert_eq!(tp_json["y"], -2);

    let spawn_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "spawn_monster",
                "arguments": {
                    "id": "dragon_1",
                    "name": "远古红龙",
                    "glyph": "D",
                    "x": 4,
                    "y": -2,
                    "hp": 200,
                    "attack": 25
                }
            }
        }),
    )
    .await;
    assert_eq!(spawn_resp["result"]["isError"], false);

    // Verify via `get_world_summary` and `get_viewport`
    let summary_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "get_world_summary",
                "arguments": {}
            }
        }),
    )
    .await;
    let summary_json: serde_json::Value = serde_json::from_str(
        summary_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert!(
        summary_json.get("chunks").is_none(),
        "get_world_summary must exclude bulky chunks data"
    );
    assert!(
        summary_json["monsters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "dragon_1" && m["x"] == 4 && m["y"] == -2),
        "get_world_summary should include newly spawned dragon_1"
    );

    // 5. Core verification: dynamically register a brand-new MCP tool at runtime via `game_command`
    let def_tool_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "game_command",
                "arguments": {
                    "command": "! (mcp:def-tool \"custom_ping\" \"AI dynamic tool\" (hash \"type\" \"object\") (lambda (args) (hash \"pong\" (map-get args \"msg\" \"ok\"))))"
                }
            }
        }),
    )
    .await;
    assert_eq!(def_tool_resp["result"]["isError"], false);

    // Immediately verify `custom_ping` appears in `tools/list`
    let list_resp2 = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await;
    let tools2 = list_resp2["result"]["tools"].as_array().unwrap();
    assert!(
        tools2.iter().any(|t| t["name"] == "custom_ping"),
        "Dynamically registered 'custom_ping' must immediately appear in tools/list"
    );

    // Immediately call `custom_ping` via `tools/call`
    let call_custom_resp = send_mcp_http_post(
        &mut server,
        mcp_addr,
        "/mcp",
        json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "custom_ping",
                "arguments": {
                    "msg": "hello_from_ai"
                }
            }
        }),
    )
    .await;
    assert_eq!(call_custom_resp["result"]["isError"], false);
    let custom_out: serde_json::Value = serde_json::from_str(
        call_custom_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(custom_out["pong"], "hello_from_ai");

    server.handle.shutdown();
}
