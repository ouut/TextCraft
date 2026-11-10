use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use game_net::{ClientEnvelope, DualCodec, ServerEnvelope};
use game_server::ServerConfig;
use textcraft::{
    EMBEDDED_WEB_ASSETS, PlayerInput, PlayerSnapshot, SchemeLogic, TextCraftCommand,
    TextCraftEvent, TextCraftServer,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

fn get_scripts_dir() -> PathBuf {
    let p = PathBuf::from("models/roguelike");
    if p.exists() {
        p
    } else {
        PathBuf::from("../../models/roguelike")
    }
}

#[tokio::test]
async fn test_embedded_web_terminal_http() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let server = TextCraftServer::start_with_config(config, logic, "admin_secret".to_string(), 1.0, 0)
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
    assert!(resp_str.contains("TextCraft 3.0"));

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
    assert!(resp_str2.contains("TextCraft 3.0"));
}

#[tokio::test]
async fn test_websocket_auth_tick_and_repl() {
    let scripts_dir = get_scripts_dir();
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
    let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
    logic.init().unwrap();

    let mut server = TextCraftServer::start_with_config(
        config,
        logic,
        "super_secret_admin".to_string(),
        1.0,
        0,
    )
    .await
    .unwrap();

    let local_addr = server.local_addr();

    // Spawn server tick loop driver task
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

    // 1. Connect admin client
    let ws_url = format!("ws://{}/ws", local_addr);
    let (mut admin_ws, _) = connect_async(&ws_url).await.unwrap();

    // Send Auth with admin token
    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "super_secret_admin".to_string(),
        client_version: "3.0.0".to_string(),
    };
    admin_ws
        .send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .unwrap();

    // Verify AuthSuccess
    let msg1 = admin_ws.next().await.unwrap().unwrap();
    let env1: ServerEnvelope<PlayerSnapshot, TextCraftEvent> = match msg1 {
        Message::Text(t) => DualCodec::decode_text(&t).unwrap(),
        _ => panic!("Expected text message"),
    };
    match env1 {
        ServerEnvelope::AuthSuccess { player_id, .. } => {
            assert!(player_id > 0);
        }
        other => panic!("Expected AuthSuccess, got {:?}", other),
    }

    // Verify Welcome event
    let msg2 = admin_ws.next().await.unwrap().unwrap();
    let env2: ServerEnvelope<PlayerSnapshot, TextCraftEvent> = match msg2 {
        Message::Text(t) => DualCodec::decode_text(&t).unwrap(),
        _ => panic!("Expected text message"),
    };
    match env2 {
        ServerEnvelope::Event(TextCraftEvent::Welcome { player_id, message }) => {
            assert!(player_id > 0);
            assert!(message.contains("TextCraft 3.0"));
        }
        other => panic!("Expected Welcome event, got {:?}", other),
    }

    // Verify Snapshot received
    let msg3 = admin_ws.next().await.unwrap().unwrap();
    let env3: ServerEnvelope<PlayerSnapshot, TextCraftEvent> = match msg3 {
        Message::Text(t) => DualCodec::decode_text(&t).unwrap(),
        _ => panic!("Expected text message"),
    };
    match env3 {
        ServerEnvelope::Snapshot(snap) => {
            assert_eq!(snap.cols, 80);
            assert_eq!(snap.rows, 24);
            assert!(!snap.screen.is_empty());
            assert_eq!(snap.hud["theme"], "cyberpunk");
        }
        other => panic!("Expected Snapshot, got {:?}", other),
    }

    // 2. Test Admin REPL Command: `! (+ 123 456)`
    let repl_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: "! (+ 123 456)".to_string(),
    });
    admin_ws
        .send(Message::Text(DualCodec::encode_text(&repl_cmd).unwrap()))
        .await
        .unwrap();

    // Wait for REPL message response
    let mut repl_success = false;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = admin_ws.next().await {
            if let Ok(ServerEnvelope::Event(TextCraftEvent::Message { channel, text })) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if channel == "repl" && text.contains("579") {
                    repl_success = true;
                    break;
                }
            }
        }
    }
    assert!(repl_success, "Admin REPL command did not return expected result 579");

    // 3. Connect non-admin client
    let (mut user_ws, _) = connect_async(&ws_url).await.unwrap();
    let user_auth = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: "regular_user_token".to_string(),
        client_version: "3.0.0".to_string(),
    };
    user_ws
        .send(Message::Text(DualCodec::encode_text(&user_auth).unwrap()))
        .await
        .unwrap();

    // Consume AuthSuccess & Welcome
    let _ = user_ws.next().await;
    let _ = user_ws.next().await;

    // Non-admin attempts REPL command
    let forbidden_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Cmd {
        input: "! (* 10 10)".to_string(),
    });
    user_ws
        .send(Message::Text(DualCodec::encode_text(&forbidden_cmd).unwrap()))
        .await
        .unwrap();

    let mut permission_denied = false;
    for _ in 0..10 {
        if let Some(Ok(Message::Text(t))) = user_ws.next().await {
            if let Ok(ServerEnvelope::Event(TextCraftEvent::Error { text })) =
                DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&t)
            {
                if text.contains("Permission denied: admin token required") {
                    permission_denied = true;
                    break;
                }
            }
        }
    }
    assert!(permission_denied, "Non-admin did not receive permission denied error");

    // Stop background driver task
    let _ = stop_tx.send(());
    let _ = server_task.await;
}
