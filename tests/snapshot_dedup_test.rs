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

struct TestHarness {
    pub local_addr: SocketAddr,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    server_task: Option<tokio::task::JoinHandle<()>>,
}

impl TestHarness {
    async fn new(heartbeat_ms: u64) -> Self {
        let scripts_dir = get_scripts_dir();
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
        let mut logic = SchemeLogic::new(&scripts_dir, false, None).unwrap();
        logic.init().unwrap();

        let mut server = TextCraftServer::start_with_config(
            config,
            logic,
            "test_admin".to_string(),
            1.0,
            0,
        )
        .await
        .unwrap()
        .with_snapshot_heartbeat_ms(heartbeat_ms);

        let local_addr = server.local_addr();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();

        let server_task = tokio::spawn(async move {
            let mut last_tick = std::time::Instant::now();
            let mut interval = tokio::time::interval(Duration::from_millis(16));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    _ = interval.tick() => {
                        let now = std::time::Instant::now();
                        let dt = now.duration_since(last_tick).as_secs_f32();
                        last_tick = now;
                        let _ = server.tick(dt);
                    }
                }
            }
        });

        Self {
            local_addr,
            stop_tx: Some(stop_tx),
            server_task: Some(server_task),
        }
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.server_task.take() {
            handle.abort();
        }
    }
}

async fn read_next_envelope(
    ws: &mut (impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin),
) -> Option<ServerEnvelope<PlayerSnapshot, TextCraftEvent>> {
    while let Some(msg_res) = ws.next().await {
        match msg_res {
            Ok(Message::Text(text)) => {
                if let Ok(env) = DualCodec::decode_text::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&text) {
                    return Some(env);
                }
            }
            Ok(Message::Binary(bin)) => {
                if let Ok(env) = DualCodec::decode_binary::<ServerEnvelope<PlayerSnapshot, TextCraftEvent>>(&bin) {
                    return Some(env);
                }
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            _ => return None,
        }
    }
    None
}

async fn connect_and_auth(
    local_addr: SocketAddr,
    token: &str,
) -> (
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    u64,
    PlayerSnapshot,
) {
    let ws_url = format!("ws://{}/ws", local_addr);
    let (mut ws, _) = connect_async(&ws_url).await.expect("Failed to connect WebSocket");

    let auth_msg = ClientEnvelope::<PlayerInput, TextCraftCommand>::Auth {
        token: token.to_string(),
        client_version: "3.0.0".to_string(),
    };
    ws.send(Message::Text(DualCodec::encode_text(&auth_msg).unwrap()))
        .await
        .expect("Failed to send Auth message");

    let mut auth_pid = None;
    let mut welcome_received = false;
    let mut initial_snapshot = None;

    while auth_pid.is_none() || !welcome_received || initial_snapshot.is_none() {
        let env = tokio::time::timeout(Duration::from_secs(3), read_next_envelope(&mut ws))
            .await
            .expect("Timeout waiting for handshake envelope")
            .expect("Connection closed during handshake");

        match env {
            ServerEnvelope::AuthSuccess { player_id, .. } => {
                auth_pid = Some(player_id);
            }
            ServerEnvelope::Event(TextCraftEvent::Welcome { .. }) => {
                welcome_received = true;
            }
            ServerEnvelope::Snapshot(snap) => {
                initial_snapshot = Some(snap);
            }
            _ => {}
        }
    }

    (ws, auth_pid.unwrap(), initial_snapshot.unwrap())
}

/// Test 1: Snapshot deduplication skips unchanged frames when client is idle.
#[tokio::test]
async fn test_snapshot_dedup_skips_unchanged_frames() {
    let harness = TestHarness::new(600).await;
    let (mut ws, _pid, initial_snap) = connect_and_auth(harness.local_addr, "player_test1").await;

    assert_eq!(initial_snap.cols, 80);
    assert_eq!(initial_snap.rows, 24);

    // Keep still for 80ms while server runs multiple ticks (~5 ticks at 16ms/tick)
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Timeout read for 100ms: assert that NO duplicate snapshot message is received
    let res = tokio::time::timeout(Duration::from_millis(100), read_next_envelope(&mut ws)).await;
    assert!(
        res.is_err(),
        "Expected timeout with no duplicate snapshot, but received: {:?}",
        res
    );
}

/// Test 2: Snapshot is delivered immediately on player action without waiting for heartbeat timeout.
#[tokio::test]
async fn test_snapshot_delivered_immediately_on_player_action() {
    let harness = TestHarness::new(600).await;
    let (mut ws, _pid, _initial_snap) = connect_and_auth(harness.local_addr, "player_test2").await;

    // Wait 50ms during the quiescent window before heartbeat expires
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Client sends movement input (KeyW)
    let key_cmd = ClientEnvelope::<PlayerInput, TextCraftCommand>::Input(PlayerInput::Key {
        code: "KeyW".to_string(),
        shift: false,
        ctrl: false,
        alt: false,
        action: Default::default(),
    });
    ws.send(Message::Text(DualCodec::encode_text(&key_cmd).unwrap()))
        .await
        .expect("Failed to send KeyW");

    // Immediately read updated snapshot; verify delivered within millisecond-level window (< 250ms),
    // well before the 600ms heartbeat interval
    let start = std::time::Instant::now();
    let res = tokio::time::timeout(Duration::from_millis(250), read_next_envelope(&mut ws))
        .await
        .expect("Snapshot should be delivered immediately upon player action")
        .expect("Stream should not close");

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(300),
        "Action-triggered snapshot took too long: {:?}",
        elapsed
    );

    match res {
        ServerEnvelope::Snapshot(snap) => {
            assert_eq!(snap.cols, 80);
        }
        other => panic!("Expected Snapshot, got {:?}", other),
    }
}

/// Test 3: Snapshot heartbeat fires periodically to keep the connection alive when idle.
#[tokio::test]
async fn test_snapshot_heartbeat_fires_periodically() {
    // Set heartbeat interval to 600ms
    let harness = TestHarness::new(600).await;
    let (mut ws, _pid, _initial_snap) = connect_and_auth(harness.local_addr, "player_test3").await;

    // 1. Stay still for 420ms (~26 ticks, before the 30-tick monster AI fires at ~480ms)
    // and assert no duplicate/heartbeat snapshot is received yet (heartbeat not expired)
    let pre_res = tokio::time::timeout(Duration::from_millis(420), read_next_envelope(&mut ws)).await;
    assert!(
        pre_res.is_err(),
        "Snapshot should not arrive before heartbeat interval: {:?}",
        pre_res
    );

    // 2. Drain any intermediate monster AI movement snapshot if one occurred around tick 30 (~480ms),
    // then verify the periodic heartbeat snapshot fires after the 600ms quiescent interval
    let res = tokio::time::timeout(Duration::from_millis(750), read_next_envelope(&mut ws))
        .await
        .expect("Expected snapshot to fire")
        .expect("Stream should not close");

    match res {
        ServerEnvelope::Snapshot(snap) => {
            assert_eq!(snap.cols, 80);
        }
        other => panic!("Expected heartbeat Snapshot, got {:?}", other),
    }
}

/// Test 4: Snapshot deduplication is completely disabled when heartbeat is zero.
#[tokio::test]
async fn test_snapshot_dedup_disabled_when_heartbeat_is_zero() {
    // When snapshot_heartbeat_ms == 0, deduplication is disabled
    let harness = TestHarness::new(0).await;
    let (mut ws, _pid, _initial_snap) = connect_and_auth(harness.local_addr, "player_test4").await;

    // In still state with NO actions, snapshots should continue to arrive continuously every tick
    for i in 0..3 {
        let res = tokio::time::timeout(Duration::from_millis(150), read_next_envelope(&mut ws))
            .await
            .unwrap_or_else(|_| panic!("Failed to receive snapshot frame {} with heartbeat=0", i + 1))
            .expect("Stream should not close");

        match res {
            ServerEnvelope::Snapshot(snap) => {
                assert_eq!(snap.cols, 80);
            }
            other => panic!("Expected continuous Snapshot frame, got {:?}", other),
        }
    }
}
