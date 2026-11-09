use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, ServerEnvelope};
use game_server::ServerConfig;
use textcraft::ecs::components::{
    Color, LAYER_CREATURE, LAYER_ITEM_LIQUID, LAYER_TERRAIN,
};
use textcraft::ecs::rasterizer::ViewportRasterizer;
use textcraft::ecs::world::EcsWorld;
use textcraft::{
    PlayerInput, PlayerSnapshot, SchemeLogic, TextCraftCommand, TextCraftEvent, TextCraftServer,
    EMBEDDED_WEB_ASSETS,
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

// ============================================================================
// 1. 10,000+ Active Entities Scale Benchmark & Native Rasterizer Stress Test
// ============================================================================

#[test]
fn test_10000_entities_scale_and_rasterizer_benchmark() {
    let mut ecs = EcsWorld::new();

    let start_spawn = Instant::now();
    let grid_size = 200;

    // 1. Spawn 4,000 Terrain Tiles (Perimeter walls & internal structures)
    for i in 0..4000 {
        let x = i % grid_size;
        let y = i / grid_size;
        let is_wall = (x == 0 || y == 0 || x == grid_size - 1 || y == grid_size - 1) || (i % 7 == 0);
        let e = ecs.spawn();
        ecs.set_pos(e, x, y, LAYER_TERRAIN);
        let (ch, fg, solid) = if is_wall {
            ('#', Some(Color::rgb(140, 140, 140)), true)
        } else {
            ('.', Some(Color::rgb(60, 60, 70)), false)
        };
        ecs.set_glyph(e, ch, fg, None, solid, is_wall);
        ecs.add_tag(e, "Terrain");
        if is_wall {
            ecs.add_tag(e, "Solid");
        }
    }

    // 2. Spawn 2,000 Monster Entities with Dynamic Properties
    for i in 0..2000 {
        let x = (i * 3 + 17) % (grid_size - 2) + 1;
        let y = (i * 5 + 31) % (grid_size - 2) + 1;
        let e = ecs.spawn();
        ecs.set_pos(e, x, y, LAYER_CREATURE);
        ecs.set_glyph(
            e,
            'g',
            Some(Color::rgb(100, 220, 100)),
            None,
            true,
            false,
        );
        ecs.add_tag(e, "Monster");
        ecs.set_prop(e, "hp", serde_json::json!(50));
        ecs.set_prop(e, "max_hp", serde_json::json!(50));
        ecs.set_prop(e, "attack", serde_json::json!(12));
    }

    // 3. Spawn 2,000 Item Entities
    for i in 0..2000 {
        let x = (i * 7 + 11) % (grid_size - 2) + 1;
        let y = (i * 11 + 43) % (grid_size - 2) + 1;
        let e = ecs.spawn();
        ecs.set_pos(e, x, y, LAYER_ITEM_LIQUID);
        ecs.set_glyph(
            e,
            '%',
            Some(Color::rgb(80, 200, 120)),
            None,
            false,
            false,
        );
        ecs.add_tag(e, "Item");
        ecs.set_prop(e, "heal_amount", serde_json::json!(25));
    }

    // 4. Spawn 2,500 Liquid Pool Entities (Water, Acid, Fire)
    for i in 0..2500 {
        let x = (i * 13 + 5) % (grid_size - 2) + 1;
        let y = (i * 17 + 23) % (grid_size - 2) + 1;
        let ltype = match i % 3 {
            0 => "water",
            1 => "acid",
            _ => "fire",
        };
        let (ch, fg) = match ltype {
            "water" => ('~', Color::rgb(60, 160, 255)),
            "acid" => ('~', Color::rgb(140, 255, 40)),
            _ => ('#', Color::rgb(255, 120, 20)),
        };
        let e = ecs.spawn();
        ecs.set_pos(e, x, y, LAYER_ITEM_LIQUID);
        ecs.set_glyph(e, ch, Some(fg), None, false, false);
        ecs.add_tag(e, "Liquid");
        ecs.set_prop(e, "liquid_type", serde_json::json!(ltype));
        ecs.set_prop(e, "duration", serde_json::json!(10));
    }

    let spawn_elapsed = start_spawn.elapsed();
    let total_entities = ecs.count();
    println!(
        "Spawned {} active entities in {:.2?} ({:.1} entities/sec)",
        total_entities,
        spawn_elapsed,
        total_entities as f64 / spawn_elapsed.as_secs_f64()
    );
    assert!(
        total_entities >= 10000,
        "Total entity count should exceed 10,000, got {}",
        total_entities
    );

    // 5. Native ViewportRasterizer Benchmark inside 10,000-entity world
    let mut rasterizer = ViewportRasterizer::new();
    let center_x = 100;
    let center_y = 100;
    let cols = 80;
    let rows = 24;

    // Warm-up 20 frames
    for _ in 0..20 {
        let _ = rasterizer.render(&ecs, center_x, center_y, cols, rows, false);
    }

    // Benchmark 500 frames
    let bench_frames = 500;
    let start_render = Instant::now();
    for _ in 0..bench_frames {
        let _ = rasterizer.render(&ecs, center_x, center_y, cols, rows, false);
    }
    let total_render_time = start_render.elapsed();
    let avg_render_us = total_render_time.as_secs_f64() * 1_000_000.0 / bench_frames as f64;

    println!(
        "10,000 Entities Rasterizer Benchmark: 80x24 viewport rendered 500 frames in {:.2?} (Average: {:.2} μs/frame)",
        total_render_time, avg_render_us
    );

    // Verify performance budget (< 30μs in release mode, < 200μs in unoptimized debug mode)
    let max_budget_us = if cfg!(debug_assertions) { 200.0 } else { 30.0 };
    assert!(
        avg_render_us < max_budget_us,
        "ViewportRasterizer exceeded performance budget: {:.2} μs >= {:.2} μs",
        avg_render_us, max_budget_us
    );

    // 6. Simulation Tick Advance Performance
    let start_tick = Instant::now();
    for _ in 0..60 {
        ecs.progress(1.0 / 60.0);
    }
    let tick_elapsed = start_tick.elapsed();
    println!(
        "60 Simulation ticks completed in {:.2?} ({:.2} μs/tick across 10k entities)",
        tick_elapsed,
        tick_elapsed.as_secs_f64() * 1_000_000.0 / 60.0
    );

    // 7. Flecs C Query DSL Performance
    let monsters = ecs.query_expr("GridPos, Monster").unwrap();
    assert_eq!(monsters.len(), 2000);

    let liquids = ecs.query_expr("Liquid").unwrap();
    assert_eq!(liquids.len(), 2500);

    // 8. Math Operators on 10,000 Entity Grid
    let fov = ecs.fov(center_x, center_y, 15);
    assert!(!fov.is_empty(), "FOV should compute visible tiles");

    let path = ecs.find_path((center_x, center_y), (center_x + 5, center_y + 5), true);
    assert!(path.is_some(), "A* pathfinding should find path");
}

// ============================================================================
// 2. MCP JSON-RPC 2.0 Tools (ecs_inspect, ecs_query, ecs_modify) End-to-End
// ============================================================================

#[tokio::test]
async fn test_mcp_ecs_inspect_query_modify_tools() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);

    let mut logic = SchemeLogic::new(scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();

    let mcp_addr = server
        .start_mcp_server("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();

    // 1. tools/list: Verify ecs_inspect, ecs_query, ecs_modify are declared
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });
    let list_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", list_req).await;
    let tools = list_resp["result"]["tools"].as_array().expect("tools array");
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|v| v.as_str()))
        .collect();

    assert!(tool_names.contains(&"ecs_inspect"), "Should list ecs_inspect");
    assert!(tool_names.contains(&"ecs_query"), "Should list ecs_query");
    assert!(tool_names.contains(&"ecs_modify"), "Should list ecs_modify");

    // 2. Spawn a test creature via game_command Scheme REPL
    let spawn_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "game_command",
            "arguments": {
                "player_id": 1,
                "command": "! (let ((e (ecs:spawn \"AbyssalGargoyle\"))) (ecs:set-pos e 15 25 LAYER-CREATURE) (ecs:set-glyph e \"G\" '(180 80 220) #f #t #f) (ecs:add-tag e \"Monster\") (ecs:set e \"hp\" 88) (ecs:set e \"mana\" 40) e)"
            }
        }
    });
    let spawn_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", spawn_req).await;
    let spawn_text = spawn_resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text output");
    let target_eid: u64 = spawn_text
        .trim_start_matches("=>")
        .trim()
        .parse()
        .expect("Spawn command should return numeric eid");

    // 3. ecs_query: Query for 'Monster'
    let query_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "ecs_query",
            "arguments": {
                "query": "Monster",
                "limit": 10
            }
        }
    });
    let query_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", query_req).await;
    assert_eq!(query_resp["result"]["isError"], false);
    let q_text = query_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let q_val: serde_json::Value = serde_json::from_str(q_text).unwrap();
    assert!(q_val["total_matches"].as_u64().unwrap() >= 1);
    let entities = q_val["entities"].as_array().unwrap();
    let found = entities.iter().any(|e| e["id"] == target_eid);
    assert!(found, "Target entity {} should be in query results", target_eid);

    // 4. ecs_inspect: Introspect target entity
    let inspect_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "ecs_inspect",
            "arguments": {
                "eid": target_eid
            }
        }
    });
    let inspect_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", inspect_req).await;
    assert_eq!(inspect_resp["result"]["isError"], false);
    let ins_text = inspect_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let ins_val: serde_json::Value = serde_json::from_str(ins_text).unwrap();
    assert_eq!(ins_val["id"], target_eid);
    assert_eq!(ins_val["name"], "AbyssalGargoyle");
    assert_eq!(ins_val["position"]["x"], 15);
    assert_eq!(ins_val["position"]["y"], 25);
    assert_eq!(ins_val["glyph"]["char"], "G");
    assert_eq!(ins_val["dynamic_props"]["hp"], 88);
    assert_eq!(ins_val["dynamic_props"]["mana"], 40);

    // 5. ecs_modify: Modify properties, add tags, update position
    let modify_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "ecs_modify",
            "arguments": {
                "eid": target_eid,
                "set_props": {
                    "hp": 120,
                    "enraged": true
                },
                "add_tags": ["Enraged", "Burning"],
                "set_pos": {
                    "x": 16,
                    "y": 25
                }
            }
        }
    });
    let modify_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", modify_req).await;
    assert_eq!(modify_resp["result"]["isError"], false);
    let mod_text = modify_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let mod_val: serde_json::Value = serde_json::from_str(mod_text).unwrap();
    assert_eq!(mod_val["position"]["x"], 16);
    assert_eq!(mod_val["dynamic_props"]["hp"], 120);
    assert_eq!(mod_val["dynamic_props"]["enraged"], true);

    // 6. ecs_query with newly added tag 'Burning'
    let burning_query = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "ecs_query",
            "arguments": {
                "query": "Burning"
            }
        }
    });
    let burning_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", burning_query).await;
    let b_text = burning_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    let b_val: serde_json::Value = serde_json::from_str(b_text).unwrap();
    assert!(b_val["total_matches"].as_u64().unwrap() >= 1);

    // 7. ecs_inspect with inline modify to despawn
    let despawn_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "ecs_inspect",
            "arguments": {
                "eid": target_eid,
                "modify": {
                    "despawn": true
                }
            }
        }
    });
    let despawn_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", despawn_req).await;
    assert_eq!(despawn_resp["result"]["isError"], false);

    // Verify entity no longer exists
    let check_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "ecs_inspect",
            "arguments": {
                "eid": target_eid
            }
        }
    });
    let check_resp = send_mcp_http_post(&mut server, mcp_addr, "/mcp", check_req).await;
    assert_eq!(check_resp["result"]["isError"], true);
}

// ============================================================================
// 3. Multiplayer Gateway Synchronization & Emergence Tick Benchmark
// ============================================================================

#[tokio::test]
async fn test_multiplayer_gateway_and_chemistry_emergence_tick() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);

    let mut logic = SchemeLogic::new(scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(config, logic, "admin".to_string(), 1.0, 0)
        .await
        .unwrap();
    server.snapshot_heartbeat_ms = 0; // immediate broadcast

    let local_addr = server.local_addr();

    // 1. Connect Client via WebSocket and Authenticate
    let ws_url = format!("ws://{}/ws", local_addr);
    let (mut ws_stream, _) = connect_async(&ws_url)
        .await
        .expect("WebSocket connection should succeed");

    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "alice:admin".to_string(),
        client_version: "3.0.0".to_string(),
    };
    ws_stream
        .send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    // 2. Read Welcome Event / Initial Snapshot
    let mut got_welcome = false;
    let mut got_snap = false;
    for _ in 0..5 {
        if let Ok(Some(env)) =
            tokio::time::timeout(Duration::from_millis(200), read_next_envelope(&mut ws_stream)).await
        {
            match env {
                ServerEnvelope::AuthSuccess { .. }
                | ServerEnvelope::Event(TextCraftEvent::Welcome { .. }) => {
                    got_welcome = true;
                }
                ServerEnvelope::Snapshot(snap) => {
                    assert!(snap.cols > 0 && snap.rows > 0);
                    assert!(!snap.screen.is_empty());
                    got_snap = true;
                }
                _ => {}
            }
        }
        if got_welcome && got_snap {
            break;
        }
    }
    assert!(
        got_welcome || got_snap,
        "Client should receive initial welcome or snapshot"
    );

    // 3. Send Key Movement 'KeyD'
    let move_cmd: ClientEnvelope<PlayerInput, TextCraftCommand> =
        ClientEnvelope::Input(PlayerInput::Key {
            code: "KeyD".to_string(),
            shift: false,
            ctrl: false,
            alt: false,
            action: Default::default(),
        });
    let text = DualCodec::encode_text(&move_cmd).unwrap();
    ws_stream.send(Message::Text(text)).await.unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;
    server.tick(0.016).unwrap();

    // 5. Ignite Fire liquid pool at (2, 0) via admin REPL
    let repl_res = server.logic.eval_repl("(chemistry:ignite 2 0)").unwrap();
    assert!(repl_res.contains("ok") || !repl_res.is_empty());

    // Verify fire tile spawned in Flecs
    let ecs = server.logic.ecs_world();
    let fire_tiles = ecs.lock().at(2, 0, Some(LAYER_ITEM_LIQUID));
    assert!(!fire_tiles.is_empty(), "Fire tile should exist at (2, 0)");

    // 6. Advance 65 ticks to trigger chemistry:tick-environment (executes every 30 ticks)
    for _ in 0..65 {
        server.tick(0.033).unwrap();
    }

    // Check that fire duration decreased from initial 5
    let guard = ecs.lock();
    let fire_tiles_after = guard.at(2, 0, Some(LAYER_ITEM_LIQUID));
    assert!(!fire_tiles_after.is_empty(), "Fire tile should remain active during burn duration");
    let dur = guard.get_prop(fire_tiles_after[0], "duration");
    let dur_val = dur.expect("Fire should have duration prop").as_i64().expect("duration as integer");
    assert!(
        dur_val < 5,
        "Fire duration should have ticked down from initial 5, got {}",
        dur_val
    );
}
