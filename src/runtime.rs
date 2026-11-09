use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use game_net::PlayerId;
use game_server::{ServerConfig, ServerHandle};

use crate::logic::{OutboundMessage, SchemeLogic};
use crate::types::{PlayerInput, PlayerSnapshot, TextCraftCommand, TextCraftEvent};

/// Log verbosity level for the TextCraft server.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    /// Returns the `tracing` filter directive string for this log level.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// In-memory static web client assets embedded directly into the binary.
pub static EMBEDDED_WEB_ASSETS: [(&str, &[u8], &str); 2] = [
    (
        "index.html",
        include_bytes!("../web/index.html"),
        "text/html; charset=utf-8",
    ),
    (
        "",
        include_bytes!("../web/index.html"),
        "text/html; charset=utf-8",
    ),
];

/// Extension trait providing ergonomic snapshot and event sending on `ServerHandle`.
pub trait ServerHandleExt<TSnapshot, TEvent> {
    fn send_snapshot(&self, player_id: PlayerId, snapshot: TSnapshot) -> bool;
    fn send_event(&self, player_id: PlayerId, event: TEvent) -> bool;
}

impl<TInput, TCmd, TSnapshot: Clone, TEvent> ServerHandleExt<TSnapshot, TEvent>
    for ServerHandle<TInput, TCmd, TSnapshot, TEvent>
{
    fn send_snapshot(&self, player_id: PlayerId, snapshot: TSnapshot) -> bool {
        self.send_to(player_id, game_net::ServerEnvelope::Snapshot(snapshot))
    }

    fn send_event(&self, player_id: PlayerId, event: TEvent) -> bool {
        self.send_to(player_id, game_net::ServerEnvelope::Event(event))
    }
}

/// Command line arguments for the TextCraft server.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "textcraft-server",
    version = "3.0.0",
    about = "TextCraft 3.0 Adaptive Clock Server Gateway"
)]
pub struct ServerArgs {
    /// Host / IP address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// TCP port to listen on
    #[arg(long, default_value_t = 8081)]
    pub port: u16,

    /// Directory containing Scheme scripts (prelude.scm, main.scm)
    #[arg(long, default_value = "scripts")]
    pub scripts_dir: PathBuf,

    /// Enable hot reloading file watcher for Scheme scripts (polls every 2s)
    #[arg(short = 'w', long, default_value_t = false)]
    pub watch: bool,

    /// Log verbosity level (error, warn, info, debug, trace)
    #[arg(long, value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,

    /// Simulation clock time scale factor
    #[arg(long, default_value_t = 1.0)]
    pub time_scale: f32,

    /// Token required for admin REPL evaluation (`!`)
    #[arg(long, default_value = "admin")]
    pub admin_token: String,

    /// File path for world state persistence
    #[arg(long)]
    pub save_file: Option<PathBuf>,

    /// Autosave interval in seconds (0 disables autosave)
    #[arg(long, default_value_t = 30)]
    pub autosave_sec: u64,

    /// Heartbeat timeout in seconds before an active session enters the grace period
    #[arg(long, default_value_t = 30)]
    pub heartbeat_timeout_secs: u64,

    /// Snapshot heartbeat interval in milliseconds (guarantees periodic snapshot even if unchanged; 0 disables deduplication)
    #[arg(long, default_value_t = 1000)]
    pub snapshot_heartbeat_ms: u64,

    /// Optional HTTP port for Model Context Protocol (MCP) JSON-RPC 2.0 server (e.g. 8082)
    #[arg(long)]
    pub mcp_port: Option<u16>,

    /// Custom directory for serving web client static assets (overrides embedded assets)
    #[arg(long)]
    pub web_dir: Option<PathBuf>,
}

type McpRequestMsg = (
    serde_json::Value,
    tokio::sync::oneshot::Sender<serde_json::Value>,
);

/// Strips ANSI CSI escape sequences (e.g. `\x1b[...m`) from a string,
/// preserving newlines `\n` and plain character grids for LLM-readable ASCII viewports.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next_c in chars.by_ref() {
                    if ('@'..='~').contains(&next_c) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

async fn mcp_http_handler(
    axum::extract::State(tx): axum::extract::State<
        tokio::sync::mpsc::UnboundedSender<McpRequestMsg>,
    >,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> impl axum::response::IntoResponse {
    let id = payload
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    if tx.send((payload, resp_tx)).is_err() {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32000,
                    "message": "TextCraft server is shutting down"
                }
            })),
        );
    }
    match resp_rx.await {
        Ok(resp) => (axum::http::StatusCode::OK, axum::Json(resp)),
        Err(_) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32603,
                    "message": "MCP request canceled before completion"
                }
            })),
        ),
    }
}

/// Core server gateway and session scheduling coordinator.
pub struct TextCraftServer {
    pub handle: ServerHandle<PlayerInput, TextCraftCommand, PlayerSnapshot, TextCraftEvent>,
    pub logic: SchemeLogic,
    pub admin_token: String,
    pub time_scale: f32,
    pub autosave_sec: u64,
    pub snapshot_heartbeat_ms: u64,
    pub log_level: LogLevel,
    last_snapshots: HashMap<PlayerId, (u64, Instant, u64, PlayerSnapshot)>,
    player_tokens: HashMap<PlayerId, String>,
    last_reload_check: Option<Instant>,
    mcp_rx: Option<tokio::sync::mpsc::UnboundedReceiver<McpRequestMsg>>,
    mcp_addr: Option<SocketAddr>,
}

fn hash_json_value<H: Hasher>(val: &serde_json::Value, state: &mut H) {
    match val {
        serde_json::Value::Null => 0u8.hash(state),
        serde_json::Value::Bool(b) => {
            1u8.hash(state);
            b.hash(state);
        }
        serde_json::Value::Number(n) => {
            2u8.hash(state);
            n.hash(state);
        }
        serde_json::Value::String(s) => {
            3u8.hash(state);
            s.hash(state);
        }
        serde_json::Value::Array(arr) => {
            4u8.hash(state);
            arr.len().hash(state);
            for item in arr {
                hash_json_value(item, state);
            }
        }
        serde_json::Value::Object(map) => {
            5u8.hash(state);
            map.len().hash(state);
            for (k, v) in map {
                k.hash(state);
                hash_json_value(v, state);
            }
        }
    }
}

impl TextCraftServer {
    /// Starts the server instance with parsed CLI arguments.
    pub async fn start(args: ServerArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

        // Resolve web static assets priority:
        // 1. Explicit --web-dir if provided and exists
        // 2. Local ./web directory if present in current working directory or adjacent to exe
        // 3. Fallback to embedded static assets
        let exe_dir_web = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("web")));
        let config = ServerConfig::new(addr).with_heartbeat_timeout_secs(args.heartbeat_timeout_secs);
        let config = if let Some(ref dir) = args.web_dir {
            if dir.is_dir() {
                tracing::info!(path = %dir.display(), "Using custom web assets directory from --web-dir");
                config.with_static_dir(dir)
            } else {
                tracing::warn!(path = %dir.display(), "Specified --web-dir does not exist, falling back to auto-detection");
                if PathBuf::from("web").is_dir() {
                    tracing::info!("Detected local ./web directory, using external web assets");
                    config.with_static_dir("web")
                } else if let Some(ref exe_web) = exe_dir_web.filter(|p| p.is_dir()) {
                    tracing::info!(path = %exe_web.display(), "Detected adjacent web directory, using external web assets");
                    config.with_static_dir(exe_web)
                } else {
                    tracing::info!("Using embedded web assets (zero-dependency)");
                    config.with_embedded_assets(&EMBEDDED_WEB_ASSETS)
                }
            }
        } else if PathBuf::from("web").is_dir() {
            tracing::info!("Detected local ./web directory, using external web assets");
            config.with_static_dir("web")
        } else if let Some(ref exe_web) = exe_dir_web.filter(|p| p.is_dir()) {
            tracing::info!(path = %exe_web.display(), "Detected adjacent web directory, using external web assets");
            config.with_static_dir(exe_web)
        } else {
            tracing::info!("Using embedded web assets (zero-dependency)");
            config.with_embedded_assets(&EMBEDDED_WEB_ASSETS)
        };

        let mut logic = SchemeLogic::new(args.scripts_dir, args.watch, args.save_file)?;
        logic.init()?;

        let mut server = Self::start_with_config(
            config,
            logic,
            args.admin_token,
            args.time_scale,
            args.autosave_sec,
        )
        .await?;
        server.snapshot_heartbeat_ms = args.snapshot_heartbeat_ms;
        server.log_level = args.log_level;
        if let Some(mcp_port) = args.mcp_port {
            let mcp_addr: SocketAddr = format!("{}:{}", args.host, mcp_port).parse()?;
            server.start_mcp_server(mcp_addr).await?;
        }
        Ok(server)
    }

    /// Starts the server with explicit configuration and prepared logic engine.
    pub async fn start_with_config(
        config: ServerConfig,
        logic: SchemeLogic,
        admin_token: String,
        time_scale: f32,
        autosave_sec: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let handle = game_server::start_server::<
            PlayerInput,
            TextCraftCommand,
            PlayerSnapshot,
            TextCraftEvent,
        >(config)
        .await?;

        Ok(Self {
            handle,
            logic,
            admin_token,
            time_scale,
            autosave_sec,
            snapshot_heartbeat_ms: 1000,
            log_level: LogLevel::Info,
            last_snapshots: HashMap::new(),
            player_tokens: HashMap::new(),
            last_reload_check: None,
            mcp_rx: None,
            mcp_addr: None,
        })
    }

    /// Starts the lightweight Axum HTTP JSON-RPC 2.0 server for Model Context Protocol (MCP).
    pub async fn start_mcp_server(
        &mut self,
        addr: SocketAddr,
    ) -> Result<SocketAddr, std::io::Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let bound_addr = listener.local_addr()?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.mcp_rx = Some(rx);
        self.mcp_addr = Some(bound_addr);

        let router = axum::Router::new()
            .route("/mcp", axum::routing::post(mcp_http_handler))
            .route("/", axum::routing::post(mcp_http_handler))
            .layer(tower_http::cors::CorsLayer::permissive())
            .with_state(tx);

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                tracing::error!(error = %e, "MCP HTTP server error");
            }
        });

        tracing::info!(
            mcp_addr = %bound_addr,
            "TextCraft MCP HTTP JSON-RPC 2.0 Server listening"
        );
        Ok(bound_addr)
    }

    /// Returns the bound MCP HTTP socket address if started.
    pub fn mcp_local_addr(&self) -> Option<SocketAddr> {
        self.mcp_addr
    }

    /// Sets the snapshot heartbeat interval in milliseconds (0 disables deduplication).
    pub fn with_snapshot_heartbeat_ms(mut self, ms: u64) -> Self {
        self.snapshot_heartbeat_ms = ms;
        self
    }

    /// Returns the bound local socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.handle.local_addr()
    }

    /// Performs a single simulation tick: session events, inputs, tick advance, and snapshots.
    pub fn tick(&mut self, dt: f32) -> Result<(), Box<dyn std::error::Error>> {
        let now = Instant::now();

        // 1. Script hot reloading check (only when --watch is enabled, throttled to at most once per 2s)
        if self.logic.watch_enabled()
            && self
                .last_reload_check
                .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(2))
        {
            let _ = self.logic.check_reload();
            self.last_reload_check = Some(now);
        }

        // 2. Consume session lifecycle events
        for event in self.handle.drain_session_events() {
            match event {
                game_server::SessionEvent::PlayerJoined { player_id, token } => {
                    self.player_tokens.insert(player_id, token.clone());
                    if let Err(e) = self.logic.on_player_join(player_id, &token) {
                        tracing::warn!(player_id, error = %e, "on_player_join error");
                    }
                    self.handle.send_event(
                        player_id,
                        TextCraftEvent::Welcome {
                            player_id,
                            message: format!(
                                "Welcome to TextCraft 3.0! Connected as Player {}",
                                player_id
                            ),
                        },
                    );
                    tracing::info!(player_id, token = %token, "Player joined");
                }
                game_server::SessionEvent::PlayerDisconnected {
                    player_id,
                    grace_period_secs,
                } => {
                    self.last_snapshots.remove(&player_id);
                    if let Err(e) = self.logic.on_player_disconnect(player_id) {
                        tracing::warn!(player_id, error = %e, "on_player_disconnect error");
                    }
                    tracing::info!(
                        player_id,
                        grace_period_secs,
                        "Player disconnected, marked offline and entering grace period"
                    );
                }
                game_server::SessionEvent::PlayerReconnected { player_id } => {
                    self.last_snapshots.remove(&player_id);
                    if let Some(session) = self
                        .handle
                        .session_manager()
                        .get_session_by_player(player_id)
                    {
                        self.player_tokens.insert(player_id, session.token);
                    }
                    if let Err(e) = self.logic.on_player_reconnect(player_id) {
                        tracing::warn!(player_id, error = %e, "on_player_reconnect error");
                    }
                    self.handle.send_event(
                        player_id,
                        TextCraftEvent::Welcome {
                            player_id,
                            message: format!(
                                "Welcome back to TextCraft 3.0! Reconnected as Player {}",
                                player_id
                            ),
                        },
                    );
                    tracing::info!(player_id, "Player reconnected");
                }
                game_server::SessionEvent::PlayerTerminated { player_id } => {
                    self.player_tokens.remove(&player_id);
                    self.last_snapshots.remove(&player_id);
                    if let Err(e) = self.logic.on_player_leave(player_id) {
                        tracing::warn!(player_id, error = %e, "on_player_leave error");
                    }
                    tracing::info!(player_id, "Player session terminated and archived");
                }
            }
        }

        // 3. Consume player inputs (Latest-Only pipeline)
        for (pid, input) in self.handle.drain_inputs() {
            self.process_player_input(pid, input);
        }

        // 4. Consume player commands (FIFO pipeline)
        for (pid, cmds) in self.handle.drain_commands() {
            for cmd in cmds {
                match cmd {
                    TextCraftCommand::Input(input) => {
                        self.process_player_input(pid, input);
                    }
                    TextCraftCommand::AdminEval { code } => {
                        self.process_admin_eval(pid, &code);
                    }
                }
            }
        }

        // 4.5. Drain and execute MCP JSON-RPC 2.0 requests safely on the main simulation thread
        self.drain_mcp_requests();

        // 5. Advance world simulation tick
        let tick_id = self.handle.increment_server_tick();
        if let Err(e) = self.logic.on_tick(dt, tick_id) {
            tracing::warn!(error = %e, "on_tick error");
        }

        // 6. Drain outbound messages generated by Scheme host functions
        for msg in self.logic.drain_outbound_messages() {
            match msg {
                OutboundMessage::SendMessage {
                    player_id,
                    channel,
                    text,
                } => {
                    self.handle
                        .send_event(player_id, TextCraftEvent::Message { channel, text });
                }
                OutboundMessage::Broadcast { channel, text } => {
                    self.handle.broadcast(game_net::ServerEnvelope::Event(
                        TextCraftEvent::Message { channel, text },
                    ));
                }
            }
        }

        // 7. Render and broadcast snapshots for each online player
        let current_ver = self.logic.visual_version();
        for pid in self.handle.connected_players() {
            if self.snapshot_heartbeat_ms > 0 {
                if let Some(&(_, last_sent, last_ver, _)) = self.last_snapshots.get(&pid) {
                    if last_ver == current_ver {
                        if now.duration_since(last_sent)
                            >= Duration::from_millis(self.snapshot_heartbeat_ms)
                        {
                            if let Some(entry) = self.last_snapshots.get_mut(&pid) {
                                entry.1 = now;
                                let snap = entry.3.clone();
                                tracing::trace!(
                                    player_id = pid,
                                    visual_version = current_ver,
                                    "Sending periodic heartbeat snapshot"
                                );
                                self.handle.send_snapshot(pid, snap);
                            }
                        } else {
                            tracing::trace!(
                                player_id = pid,
                                visual_version = current_ver,
                                "Skipping unchanged snapshot (visual version unchanged)"
                            );
                        }
                        continue;
                    }
                }
            }

            match self.logic.render_snapshot(pid) {
                Ok(snapshot) => {
                    if self.snapshot_heartbeat_ms == 0 {
                        tracing::trace!(
                            player_id = pid,
                            visual_version = current_ver,
                            "Sending snapshot (deduplication disabled)"
                        );
                        self.handle.send_snapshot(pid, snapshot);
                    } else {
                        let mut hasher = DefaultHasher::new();
                        snapshot.cols.hash(&mut hasher);
                        snapshot.rows.hash(&mut hasher);
                        snapshot.screen.hash(&mut hasher);
                        hash_json_value(&snapshot.hud, &mut hasher);
                        let current_hash = hasher.finish();

                        let now_after_render = Instant::now();
                        let should_send = match self.last_snapshots.get(&pid) {
                            None => true,
                            Some(&(last_hash, last_sent, _, _)) => {
                                current_hash != last_hash
                                    || now_after_render.duration_since(last_sent)
                                        >= Duration::from_millis(self.snapshot_heartbeat_ms)
                            }
                        };

                        if should_send {
                            tracing::trace!(
                                player_id = pid,
                                visual_version = current_ver,
                                hash = current_hash,
                                "Sending updated snapshot to player"
                            );
                            self.last_snapshots.insert(
                                pid,
                                (current_hash, now_after_render, current_ver, snapshot.clone()),
                            );
                            self.handle.send_snapshot(pid, snapshot);
                        } else if let Some(entry) = self.last_snapshots.get_mut(&pid) {
                            entry.2 = current_ver;
                            tracing::trace!(
                                player_id = pid,
                                visual_version = current_ver,
                                "Skipping snapshot (content hash unchanged)"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(player_id = pid, error = %e, "Failed to render snapshot");
                }
            }
        }

        Ok(())
    }

    fn process_player_input(&mut self, pid: PlayerId, input: PlayerInput) {
        match input {
            PlayerInput::Cmd { input: raw_cmd } => {
                let trimmed = raw_cmd.trim();
                if trimmed.starts_with('!') {
                    let code = trimmed.strip_prefix('!').unwrap_or(trimmed).trim();
                    self.process_admin_eval(pid, code);
                } else {
                    tracing::debug!(player_id = pid, cmd = %raw_cmd, "Processing player command");
                    if let Err(e) = self.logic.on_command(pid, &raw_cmd) {
                        tracing::warn!(player_id = pid, error = %e, "on_command error");
                    }
                }
            }
            PlayerInput::Key { .. } | PlayerInput::Mouse { .. } => {
                tracing::trace!(player_id = pid, ?input, "Dispatching player input");
                if let Err(e) = self.logic.on_input(pid, &input) {
                    tracing::warn!(player_id = pid, error = %e, "on_input error");
                }
            }
            PlayerInput::UIAction { action, payload } => {
                tracing::trace!(player_id = pid, action = %action, "Dispatching player UI action");
                if let Err(e) = self.logic.on_ui_action(pid, &action, &payload) {
                    tracing::warn!(player_id = pid, error = %e, "on_ui_action error");
                }
            }
        }
    }

    /// Checks whether a player holds admin REPL authorization.
    /// Supports both legacy exact token matching (`raw_token == admin_token`)
    /// and multi-user composite tokens (`username:auth_token` where `auth_token == admin_token`).
    pub fn is_admin_authorized(&self, pid: PlayerId) -> bool {
        if self.admin_token.is_empty() {
            return false;
        }
        let Some(raw_token) = self.player_tokens.get(&pid) else {
            return false;
        };
        if raw_token == &self.admin_token {
            return true;
        }
        if let Some((_, auth_token)) = raw_token.split_once(':') {
            return !auth_token.is_empty() && auth_token == self.admin_token;
        }
        false
    }

    fn process_admin_eval(&mut self, pid: PlayerId, code: &str) {
        let is_admin = self.is_admin_authorized(pid);

        if is_admin {
            tracing::debug!(player_id = pid, code = %code, "Executing admin REPL evaluation");
            match self.logic.eval_repl(code) {
                Ok(result) => {
                    self.handle.send_event(
                        pid,
                        TextCraftEvent::Message {
                            channel: "repl".to_string(),
                            text: result,
                        },
                    );
                }
                Err(err) => {
                    tracing::warn!(player_id = pid, error = %err, "Admin REPL evaluation error");
                    self.handle.send_event(pid, TextCraftEvent::Error { text: err });
                }
            }
        } else {
            tracing::warn!(
                player_id = pid,
                code = %code,
                "Rejected unauthorized admin REPL evaluation attempt"
            );
            self.handle.send_event(
                pid,
                TextCraftEvent::Error {
                    text: "Permission denied: admin token required".to_string(),
                },
            );
        }
    }

    /// Drains all pending MCP JSON-RPC 2.0 requests and executes them on the main simulation thread.
    pub fn drain_mcp_requests(&mut self) {
        let mut pending = Vec::new();
        if let Some(ref mut rx) = self.mcp_rx {
            while let Ok(req) = rx.try_recv() {
                pending.push(req);
            }
        }
        for (req_json, resp_tx) in pending {
            let resp_json = self.handle_mcp_request(req_json);
            let _ = resp_tx.send(resp_json);
        }
    }

    fn host_mcp_tools_schema() -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "name": "game_command",
                "description": "Execute a TextCraft game command (e.g. '(where)', '(heal)', '(say \"hello\")') or prefix with '!' to evaluate arbitrary Scheme REPL expressions (e.g. '! (+ 40 2)').",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "player_id": {
                            "type": "integer",
                            "description": "Target player ID (default: 1)"
                        },
                        "command": {
                            "type": "string",
                            "description": "Command string or '!' prefixed Scheme REPL expression"
                        }
                    },
                    "required": ["command"]
                }
            }),
            serde_json::json!({
                "name": "player_input",
                "description": "Simulate player keyboard movement/interaction (KeyW/KeyA/KeyS/KeyD/Space), UI action, or mouse click, and return the updated plain-text ASCII viewport and HUD.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "player_id": {
                            "type": "integer",
                            "description": "Target player ID (default: 1)"
                        },
                        "type": {
                            "type": "string",
                            "enum": ["key", "ui", "mouse"],
                            "description": "Input category: 'key', 'ui', or 'mouse'"
                        },
                        "code": {
                            "type": "string",
                            "description": "Key code when type='key' (e.g. 'KeyW', 'KeyA', 'KeyS', 'KeyD', 'Space')"
                        },
                        "action": {
                            "type": "string",
                            "description": "UI action name when type='ui' (e.g. 'use_item', 'buy_potion', 'close_dialog')"
                        },
                        "payload": {
                            "type": "object",
                            "description": "Optional payload object when type='ui'"
                        },
                        "x": {
                            "type": "integer",
                            "description": "Viewport X coordinate when type='mouse'"
                        },
                        "y": {
                            "type": "integer",
                            "description": "Viewport Y coordinate when type='mouse'"
                        },
                        "button": {
                            "type": "string",
                            "description": "Mouse button when type='mouse' (default: 'left')"
                        }
                    },
                    "required": ["type"]
                }
            }),
            serde_json::json!({
                "name": "reload_scripts",
                "description": "Force immediate hot-reload of all Scheme (.scm) scripts in scripts/ while preserving *world-state*, returning 'Reloaded successfully' or detailed compilation errors.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "ecs_inspect",
                "description": "Inspect detailed Flecs ECS state of an entity by its numeric ID (`eid`), including name, coordinates, glyph, tags, dynamic properties, relationships (e.g. PartOf, EquippedOn), and PlayerSession. Optionally apply modifications if `modify` is provided.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "eid": {
                            "type": "integer",
                            "description": "Entity ID to inspect"
                        },
                        "modify": {
                            "type": "object",
                            "description": "Optional patch object to modify the entity (set_props, delete_props, add_tags, remove_tags, set_pos, set_glyph, relate, unrelate, despawn)"
                        }
                    },
                    "required": ["eid"]
                }
            }),
            serde_json::json!({
                "name": "ecs_query",
                "description": "Query ECS entities using Flecs DSL expression (e.g. 'GridPos, Monster', 'Liquid', 'Item, !Dead'). Returns matching entities with their full inspected state up to `limit`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Flecs query DSL expression (e.g. 'GridPos, Monster', 'Liquid', 'Floor')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of entity details to return (default: 50)"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "ecs_modify",
                "description": "Modify an ECS entity's state: set dynamic properties, add/remove tags, update position/glyph, manage relationships, or despawn.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "eid": {
                            "type": "integer",
                            "description": "Target entity ID"
                        },
                        "set_props": {
                            "type": "object",
                            "description": "Key-value map of dynamic properties to set/update"
                        },
                        "delete_props": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of dynamic property keys to remove"
                        },
                        "add_tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of string tag names to add"
                        },
                        "remove_tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of string tag names to remove"
                        },
                        "set_pos": {
                            "type": "object",
                            "properties": {
                                "x": { "type": "integer" },
                                "y": { "type": "integer" },
                                "layer": { "type": "integer" }
                            },
                            "required": ["x", "y"],
                            "description": "Set spatial coordinates"
                        },
                        "set_glyph": {
                            "type": "object",
                            "description": "Visual glyph update (char, fg, bg, solid, opaque)"
                        },
                        "set_player_session": {
                            "type": "object",
                            "properties": {
                                "player_id": { "type": "integer" },
                                "viewport_cols": { "type": "integer" },
                                "viewport_rows": { "type": "integer" }
                            },
                            "required": ["player_id"],
                            "description": "Bind player session to this entity"
                        },
                        "relate": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "relation": { "type": "string" },
                                    "target": { "type": "integer" }
                                },
                                "required": ["relation", "target"]
                            },
                            "description": "Relationships to add"
                        },
                        "unrelate": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "relation": { "type": "string" },
                                    "target": { "type": "integer" }
                                },
                                "required": ["relation", "target"]
                            },
                            "description": "Relationships to remove"
                        },
                        "despawn": {
                            "type": "boolean",
                            "description": "If true, despawn this entity"
                        }
                    },
                    "required": ["eid"]
                }
            }),
        ]
    }

    fn handle_mcp_request(&mut self, req: serde_json::Value) -> serde_json::Value {
        let id = req
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let method = req
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let params = req
            .get("params")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        match method {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": true
                        }
                    },
                    "serverInfo": {
                        "name": "textcraft-mcp",
                        "version": "3.1.0"
                    }
                }
            }),
            "notifications/initialized" | "ping" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            }),
            "tools/list" => {
                let mut tools = Self::host_mcp_tools_schema();
                if let Ok(serde_json::Value::Array(dynamic_tools)) = self.logic.mcp_list_tools() {
                    for dt in dynamic_tools {
                        if let Some(name) = dt.get("name").and_then(|n| n.as_str()) {
                            if !tools
                                .iter()
                                .any(|t| t.get("name").and_then(|n| n.as_str()) == Some(name))
                            {
                                tools.push(dt);
                            }
                        }
                    }
                }
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tools
                    }
                })
            }
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                let (text, is_error) = self.execute_mcp_tool(name, &arguments);
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": text
                            }
                        ],
                        "isError": is_error
                    }
                })
            }
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {}", method)
                }
            }),
        }
    }

    fn execute_mcp_tool(&mut self, name: &str, arguments: &serde_json::Value) -> (String, bool) {
        match name {
            "game_command" => {
                let pid = arguments
                    .get("player_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                let Some(raw_cmd) = arguments.get("command").and_then(|v| v.as_str()) else {
                    return ("Missing required string parameter 'command'".to_string(), true);
                };
                let trimmed = raw_cmd.trim();
                if trimmed.starts_with('!') {
                    let code = trimmed.strip_prefix('!').unwrap_or(trimmed).trim();
                    match self.logic.eval_repl(code) {
                        Ok(res) => (res, false),
                        Err(err) => (err, true),
                    }
                } else {
                    if let Err(err) = self.logic.on_command(pid, trimmed) {
                        return (err, true);
                    }
                    match self.logic.render_snapshot(pid) {
                        Ok(snap) => {
                            let out = serde_json::json!({
                                "player_id": pid,
                                "command": trimmed,
                                "ascii_screen": strip_ansi_codes(&snap.screen),
                                "cols": snap.cols,
                                "rows": snap.rows,
                                "hud": snap.hud,
                            });
                            (
                                serde_json::to_string_pretty(&out)
                                    .unwrap_or_else(|_| out.to_string()),
                                false,
                            )
                        }
                        Err(err) => (err, true),
                    }
                }
            }
            "player_input" => {
                let pid = arguments
                    .get("player_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                let input_type = arguments
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("key");
                let input = match input_type {
                    "key" => PlayerInput::Key {
                        code: arguments
                            .get("code")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        shift: arguments
                            .get("shift")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        ctrl: arguments
                            .get("ctrl")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                        alt: arguments
                            .get("alt")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    },
                    "ui" => PlayerInput::UIAction {
                        action: arguments
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        payload: arguments
                            .get("payload")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    },
                    "mouse" => PlayerInput::Mouse {
                        x: arguments.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                        y: arguments.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                        button: arguments
                            .get("button")
                            .and_then(|v| v.as_str())
                            .unwrap_or("left")
                            .to_string(),
                    },
                    other => {
                        return (format!("Unsupported player_input type: {}", other), true);
                    }
                };
                self.process_player_input(pid, input);
                match self.logic.render_snapshot(pid) {
                    Ok(snap) => {
                        let out = serde_json::json!({
                            "player_id": pid,
                            "ascii_screen": strip_ansi_codes(&snap.screen),
                            "cols": snap.cols,
                            "rows": snap.rows,
                            "hud": snap.hud,
                        });
                        (
                            serde_json::to_string_pretty(&out)
                                .unwrap_or_else(|_| out.to_string()),
                            false,
                        )
                    }
                    Err(err) => (err, true),
                }
            }
            "reload_scripts" => match self.logic.force_reload() {
                Ok(()) => ("Reloaded successfully".to_string(), false),
                Err(err) => (err, true),
            },
            "ecs_inspect" => {
                let Some(eid) = arguments.get("eid").and_then(|v| v.as_u64()) else {
                    return ("Missing required integer parameter 'eid'".to_string(), true);
                };
                let ecs = self.logic.ecs_world();
                let mut guard = ecs.lock();
                if let Some(patch) = arguments.get("modify") {
                    if patch.is_object() {
                        match guard.modify_entity(eid, patch) {
                            Ok(res) => {
                                drop(guard);
                                self.logic.increment_extra_version();
                                (
                                    serde_json::to_string_pretty(&res)
                                        .unwrap_or_else(|_| res.to_string()),
                                    false,
                                )
                            }
                            Err(err) => (err, true),
                        }
                    } else {
                        ("Parameter 'modify' must be an object".to_string(), true)
                    }
                } else {
                    match guard.inspect_entity(eid) {
                        Some(res) => (
                            serde_json::to_string_pretty(&res)
                                .unwrap_or_else(|_| res.to_string()),
                            false,
                        ),
                        None => (format!("Entity {} does not exist", eid), true),
                    }
                }
            }
            "ecs_query" => {
                let Some(query_str) = arguments.get("query").and_then(|v| v.as_str()) else {
                    return ("Missing required string parameter 'query'".to_string(), true);
                };
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;
                let ecs = self.logic.ecs_world();
                let guard = ecs.lock();
                match guard.query_entities(query_str, limit) {
                    Ok(res) => (
                        serde_json::to_string_pretty(&res).unwrap_or_else(|_| res.to_string()),
                        false,
                    ),
                    Err(err) => (err, true),
                }
            }
            "ecs_modify" => {
                let Some(eid) = arguments.get("eid").and_then(|v| v.as_u64()) else {
                    return ("Missing required integer parameter 'eid'".to_string(), true);
                };
                let ecs = self.logic.ecs_world();
                let mut guard = ecs.lock();
                match guard.modify_entity(eid, arguments) {
                    Ok(res) => {
                        drop(guard);
                        self.logic.increment_extra_version();
                        (
                            serde_json::to_string_pretty(&res)
                                .unwrap_or_else(|_| res.to_string()),
                            false,
                        )
                    }
                    Err(err) => (err, true),
                }
            }
            _ => match self.logic.mcp_call_tool(name, arguments) {
                Ok(val) => {
                    let is_err = val
                        .get("error")
                        .is_some_and(|e| !e.is_null() && e != &serde_json::Value::Bool(false));
                    let text = match val {
                        serde_json::Value::String(s) => s,
                        other => serde_json::to_string_pretty(&other)
                            .unwrap_or_else(|_| other.to_string()),
                    };
                    (text, is_err)
                }
                Err(err) => (err, true),
            },
        }
    }

    /// Runs the adaptive dynamic FPS main tick loop until shutdown or Ctrl+C.
    pub async fn run_loop(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_tick_time = Instant::now();
        let mut last_autosave = Instant::now();
        let autosave_interval = Duration::from_secs(self.autosave_sec);

        tracing::info!(
            addr = %self.handle.local_addr(),
            watch = self.logic.watch_enabled(),
            log_level = %self.log_level,
            "TextCraft 3.0 Server running"
        );

        let ctrl_c = tokio::signal::ctrl_c();
        tokio::pin!(ctrl_c);

        loop {
            let now = Instant::now();
            let elapsed = now.duration_since(last_tick_time);
            last_tick_time = now;
            let dt = elapsed.as_secs_f32() * self.time_scale;

            // Dynamically query target FPS from Scheme (clamped 1~60, default 60)
            let target_fps = self.logic.query_target_fps();
            let tick_duration = Duration::from_micros(1_000_000 / target_fps);

            // Execute tick
            if let Err(e) = self.tick(dt) {
                tracing::error!(error = %e, "Error during tick execution");
            }

            // Autosave check
            if self.autosave_sec > 0
                && self.logic.has_save_file()
                && now.duration_since(last_autosave) >= autosave_interval
            {
                if let Err(e) = self.logic.save_state_to_file() {
                    tracing::error!(error = %e, "Autosave failed");
                }
                last_autosave = now;
            }

            // Calculate remaining sleep time to match target FPS
            let compute_duration = Instant::now().duration_since(now);
            let sleep_duration = tick_duration.saturating_sub(compute_duration);

            tokio::select! {
                _ = &mut ctrl_c => {
                    tracing::info!("Received Ctrl+C interrupt, shutting down server...");
                    break;
                }
                _ = tokio::time::sleep(sleep_duration) => {}
            }
        }

        // Shutdown networking; SchemeLogic::drop handles final state save
        self.handle.shutdown();
        Ok(())
    }
}

/// Convenience function to start and run the server from CLI arguments.
pub async fn run_server(args: ServerArgs) -> Result<(), Box<dyn std::error::Error>> {
    let server = TextCraftServer::start(args).await?;
    server.run_loop().await
}

/// Convenience function to start the server returning TextCraftServer (which exposes its ServerHandle).
pub async fn run_server_with_handle(args: ServerArgs) -> Result<TextCraftServer, Box<dyn std::error::Error>> {
    TextCraftServer::start(args).await
}
