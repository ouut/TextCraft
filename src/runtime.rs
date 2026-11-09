use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

pub use crate::types::{resolve_namespace_and_key, McpStorage};

/// Core server gateway and session scheduling coordinator.
pub struct TextCraftServer {
    pub handle: ServerHandle<PlayerInput, TextCraftCommand, PlayerSnapshot, TextCraftEvent>,
    pub logic: SchemeLogic,
    pub admin_token: String,
    pub time_scale: f32,
    pub autosave_sec: u64,
    pub snapshot_heartbeat_ms: u64,
    pub log_level: LogLevel,
    pub is_paused: Arc<AtomicBool>,
    pub mcp_storage: McpStorage,
    last_snapshots: HashMap<PlayerId, (Instant, PlayerSnapshot)>,
    player_tokens: HashMap<PlayerId, String>,
    last_reload_check: Option<Instant>,
    mcp_rx: Option<tokio::sync::mpsc::UnboundedReceiver<McpRequestMsg>>,
    mcp_addr: Option<SocketAddr>,
}

impl TextCraftServer {
    /// Starts the server instance with parsed CLI arguments.
    pub async fn start(args: ServerArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        let base_config = ServerConfig::new(addr).with_heartbeat_timeout_secs(args.heartbeat_timeout_secs);

        // Resolve web static assets priority:
        // 1. Explicit --web-dir if provided and exists
        // 2. Local ./web directory if present in current working directory or adjacent to exe
        // 3. Fallback to embedded static assets
        let exe_dir_web = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("web")));
        let config = if let Some(ref dir) = args.web_dir {
            if dir.is_dir() {
                tracing::info!(path = %dir.display(), "Using custom web assets directory from --web-dir");
                base_config.with_static_dir(dir)
            } else {
                tracing::warn!(path = %dir.display(), "Specified --web-dir does not exist, falling back to auto-detection");
                if PathBuf::from("web").is_dir() {
                    tracing::info!("Detected local ./web directory, using external web assets");
                    base_config.with_static_dir("web")
                } else if let Some(ref exe_web) = exe_dir_web.filter(|p| p.is_dir()) {
                    tracing::info!(path = %exe_web.display(), "Detected adjacent web directory, using external web assets");
                    base_config.with_static_dir(exe_web)
                } else {
                    tracing::info!("Using embedded web assets (zero-dependency)");
                    base_config.with_embedded_assets(&EMBEDDED_WEB_ASSETS)
                }
            }
        } else if PathBuf::from("web").is_dir() {
            tracing::info!("Detected local ./web directory, using external web assets");
            base_config.with_static_dir("web")
        } else if let Some(ref exe_web) = exe_dir_web.filter(|p| p.is_dir()) {
            tracing::info!(path = %exe_web.display(), "Detected adjacent web directory, using external web assets");
            base_config.with_static_dir(exe_web)
        } else {
            tracing::info!("Using embedded web assets (zero-dependency)");
            base_config.with_embedded_assets(&EMBEDDED_WEB_ASSETS)
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
        if let Some(save_path) = server.logic.save_file() {
            let mcp_path = save_path.with_extension("mcp.json");
            if mcp_path.exists() {
                if let Err(e) = server.mcp_store_import_from_file(&mcp_path, true) {
                    tracing::warn!(error = %e, "Failed to load MCP storage from save file");
                }
            }
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

        let mcp_storage = logic.mcp_storage.clone();

        Ok(Self {
            handle,
            logic,
            admin_token,
            time_scale,
            autosave_sec,
            snapshot_heartbeat_ms: 1000,
            log_level: LogLevel::Info,
            is_paused: Arc::new(AtomicBool::new(false)),
            mcp_storage,
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

    // ==========================================
    // Dedicated MCP Storage & Blackboard Methods
    // ==========================================

    /// Retrieve a stored value from dedicated MCP storage by key.
    pub fn mcp_store_get(&self, key: &str) -> Option<serde_json::Value> {
        self.mcp_store_get_scoped(None, key)
    }

    /// Store a value into dedicated MCP storage.
    pub fn mcp_store_set(&self, key: &str, value: serde_json::Value) {
        self.mcp_store_set_scoped(None, key, value);
    }

    /// Delete a stored key from dedicated MCP storage.
    pub fn mcp_store_delete(&self, key: &str) -> bool {
        self.mcp_store_delete_scoped(None, key)
    }

    /// List all keys in dedicated MCP storage, optionally filtered by key prefix.
    pub fn mcp_store_list(&self, prefix: Option<&str>) -> Vec<String> {
        self.mcp_store_list_scoped(None, prefix)
    }

    /// Clear all keys across all namespaces in dedicated MCP storage.
    pub fn mcp_store_clear(&self) {
        self.mcp_storage.write().clear();
    }

    /// Retrieve a stored value with explicit namespace support.
    pub fn mcp_store_get_scoped(
        &self,
        namespace: Option<&str>,
        key: &str,
    ) -> Option<serde_json::Value> {
        let (ns, sub) = resolve_namespace_and_key(namespace, key);
        let composite = format!("{}:{}", ns, sub);
        let guard = self.mcp_storage.read();
        guard
            .get(&composite)
            .cloned()
            .or_else(|| guard.get(key).cloned())
    }

    /// Store a value with explicit namespace support.
    pub fn mcp_store_set_scoped(
        &self,
        namespace: Option<&str>,
        key: &str,
        value: serde_json::Value,
    ) {
        let (ns, sub) = resolve_namespace_and_key(namespace, key);
        let composite = format!("{}:{}", ns, sub);
        let mut guard = self.mcp_storage.write();
        guard.insert(composite, value);
    }

    /// Delete a key with explicit namespace support.
    pub fn mcp_store_delete_scoped(&self, namespace: Option<&str>, key: &str) -> bool {
        let (ns, sub) = resolve_namespace_and_key(namespace, key);
        let composite = format!("{}:{}", ns, sub);
        let mut guard = self.mcp_storage.write();
        let r1 = guard.remove(&composite).is_some();
        let r2 = guard.remove(key).is_some();
        r1 || r2
    }

    /// List keys filtered by namespace and/or prefix.
    pub fn mcp_store_list_scoped(
        &self,
        namespace: Option<&str>,
        prefix: Option<&str>,
    ) -> Vec<String> {
        let guard = self.mcp_storage.read();
        if let Some(ns) = namespace.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let ns_prefix = format!("{}:", ns);
            let mut list: Vec<String> = guard
                .keys()
                .filter_map(|k| {
                    if let Some(sub) = k.strip_prefix(&ns_prefix) {
                        if prefix.is_none_or(|p| sub.starts_with(p) || k.starts_with(p)) {
                            Some(sub.to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            list.sort();
            list
        } else {
            let mut list: Vec<String> = guard
                .keys()
                .filter(|k| prefix.is_none_or(|p| k.starts_with(p)))
                .cloned()
                .collect();
            list.sort();
            list
        }
    }

    /// Clear dedicated MCP storage, optionally limited to a specific namespace.
    pub fn mcp_store_clear_scoped(&self, namespace: Option<&str>) -> usize {
        if let Some(ns) = namespace.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            let ns_prefix = format!("{}:", ns);
            let mut guard = self.mcp_storage.write();
            let to_remove: Vec<String> = guard
                .keys()
                .filter(|k| k.starts_with(&ns_prefix))
                .cloned()
                .collect();
            let count = to_remove.len();
            for k in to_remove {
                guard.remove(&k);
            }
            count
        } else {
            let mut guard = self.mcp_storage.write();
            let count = guard.len();
            guard.clear();
            count
        }
    }

    /// Export dedicated MCP storage atomically to a JSON file.
    pub fn mcp_store_export_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let tmp_path = path.with_extension(format!(
            "{}.tmp",
            path.extension().and_then(|s| s.to_str()).unwrap_or("json")
        ));
        let guard = self.mcp_storage.read();
        let content =
            serde_json::to_string_pretty(&*guard).map_err(std::io::Error::other)?;
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Import entries into dedicated MCP storage from a JSON file.
    pub fn mcp_store_import_from_file(
        &self,
        path: &Path,
        overwrite: bool,
    ) -> std::io::Result<usize> {
        if !path.exists() {
            return Ok(0);
        }
        let content = std::fs::read_to_string(path)?;
        let map: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).map_err(std::io::Error::other)?;
        let count = map.len();
        let mut guard = self.mcp_storage.write();
        for (k, v) in map {
            if overwrite || !guard.contains_key(&k) {
                guard.insert(k, v);
            }
        }
        Ok(count)
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

        // 5. Advance world simulation tick (skipped when is_paused is true)
        if !self.is_paused.load(Ordering::Relaxed) {
            let tick_id = self.handle.increment_server_tick();
            if let Err(e) = self.logic.on_tick(dt, tick_id) {
                tracing::warn!(error = %e, "on_tick error");
            }
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
        for pid in self.handle.connected_players() {
            match self.logic.render_snapshot(pid) {
                Ok(Some(new_snapshot)) => {
                    tracing::trace!(player_id = pid, "Sending updated snapshot to player");
                    self.last_snapshots.insert(pid, (now, new_snapshot.clone()));
                    self.handle.send_snapshot(pid, new_snapshot);
                }
                Ok(None) => {
                    // Scheme declared no visual changes for this player
                    if self.snapshot_heartbeat_ms == 0 {
                        // Deduplication disabled: re-send cached snapshot if present
                        if let Some((_, cached_snap)) = self.last_snapshots.get(&pid) {
                            self.handle.send_snapshot(pid, cached_snap.clone());
                        }
                    } else if let Some((last_sent, cached_snap)) = self.last_snapshots.get_mut(&pid) {
                        if now.duration_since(*last_sent)
                            >= Duration::from_millis(self.snapshot_heartbeat_ms)
                        {
                            *last_sent = now;
                            let snap = cached_snap.clone();
                            tracing::trace!(
                                player_id = pid,
                                "Sending periodic heartbeat snapshot"
                            );
                            self.handle.send_snapshot(pid, snap);
                        } else {
                            tracing::trace!(
                                player_id = pid,
                                "Skipping unchanged snapshot (Scheme not dirty, heartbeat within window)"
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

    /// Returns the last cached snapshot for a player, if any.
    pub fn last_snapshot(&self, pid: PlayerId) -> Option<&PlayerSnapshot> {
        self.last_snapshots.get(&pid).map(|(_, snap)| snap)
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
                            "description": "Key action ('down' | 'up') when type='key' (default: 'down'), or UI action name when type='ui' (e.g. 'use_item', 'buy_potion', 'close_dialog')"
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
                "description": "Force immediate hot-reload of all Scheme (.scm) scripts in scripts/. If clean is true (default), re-initializes world state via (engine:on-init); if false, preserves existing *world-state*.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "clean": {
                            "type": "boolean",
                            "description": "Whether to perform a clean reload by re-calling (engine:on-init) instead of restoring previous world state. Defaults to true."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "pause_clock",
                "description": "Pause the physical game simulation clock. Tick advancing will halt, while WebSocket connections, network heartbeats, and commands remain fully active.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "resume_clock",
                "description": "Resume the physical game simulation clock.",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "step_simulation",
                "description": "Step forward the simulation world state deterministically by a specified number of ticks, optionally injecting player inputs, calculating state diffs, and rendering snapshots.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "steps": {
                            "type": "integer",
                            "description": "Number of simulation steps to advance (default: 1)"
                        },
                        "dt": {
                            "type": "number",
                            "description": "Delta time per step in seconds (default: 0.016)"
                        },
                        "seed": {
                            "type": "integer",
                            "description": "Optional deterministic PRNG seed to set before stepping"
                        },
                        "state_projection": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of dot-separated paths to project and return from world state instead of full state (e.g. ['players.1.x', 'players.1.gold'])"
                        },
                        "diff_path_prefix": {
                            "type": "string",
                            "description": "Optional dot-separated key path prefix to scope the diff comparison (e.g. 'players.1')"
                        },
                        "diff_exclude_keys": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of keys to exclude from state diff comparison (defaults to ['chunks'])"
                        },
                        "inputs": {
                            "type": "array",
                            "description": "Optional player inputs to inject during stepping. Each item: { player_id, input, step?, hold_ticks? }",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "player_id": { "type": "integer" },
                                    "step": { "type": "integer", "description": "0-based step index at which to inject this input (default: 0)" },
                                    "hold_ticks": { "type": "integer", "description": "Optional number of ticks to hold this key down. Automatically injects 'action': 'down' at 'step', and 'action': 'up' at 'step + hold_ticks'." },
                                    "type": { "type": "string", "enum": ["key", "ui", "mouse", "cmd"] },
                                    "code": { "type": "string" },
                                    "action": { "type": "string", "description": "Key action ('down' | 'up') when type='key', or UI action name when type='ui'" },
                                    "payload": { "type": "object" },
                                    "command": { "type": "string" },
                                    "x": { "type": "integer" },
                                    "y": { "type": "integer" },
                                    "button": { "type": "string" }
                                }
                            }
                        },
                        "include_snapshot": {
                            "type": "boolean",
                            "description": "Whether to include the latest ANSI viewport and HUD in output (default: false)"
                        },
                        "snapshot_player_id": {
                            "type": "integer",
                            "description": "Target player ID for viewport snapshot when include_snapshot is true (default: 1)"
                        },
                        "include_state_diff": {
                            "type": "boolean",
                            "description": "Whether to calculate and return state diff between before and after step execution (default: false)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "get_state_diff",
                "description": "Compute flat recursive key-path state diff (added, removed, changed) between two JSON states or against current exported world state.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "old_state": {
                            "type": "object",
                            "description": "Base state to compare from. If omitted, uses current world state."
                        },
                        "new_state": {
                            "type": "object",
                            "description": "Target state to compare to. If omitted, uses current world state."
                        },
                        "path_prefix": {
                            "type": "string",
                            "description": "Optional dot-separated key path prefix to scope the diff comparison to a subtree (e.g. 'players.1')"
                        },
                        "exclude_keys": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of keys or key paths to exclude from diff comparison (e.g. ['chunks'])"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "query_state",
                "description": "Query and project specific parts of current exported world state by dot-separated path masks (e.g. ['players.1.x', 'players.1.hp', 'monsters.boss_1.hp']) to reduce token overhead.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of dot-separated key paths to extract (e.g. ['players.1.x', 'players.1.gold'])"
                        }
                    },
                    "required": ["paths"]
                }
            }),
            serde_json::json!({
                "name": "eval_assertion",
                "description": "Evaluate Scheme assertion expressions under watchdog protection. Supports single expression ('code' or 'expr') or batch assertions ('exprs'). Returns structured assertion report with pass/fail and rich diff details.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "code": {
                            "type": "string",
                            "description": "Scheme assertion expression to evaluate, e.g. '(= (map-get (get-player 1) \"x\" 0) 5)'"
                        },
                        "expr": {
                            "type": "string",
                            "description": "Alias for code: single Scheme assertion expression to evaluate"
                        },
                        "exprs": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Batch of Scheme assertion expressions to evaluate together in one request"
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Watchdog timeout in milliseconds per assertion (default: 1000)"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "mcp_store_set",
                "description": "Store or update an arbitrary JSON value in dedicated MCP agent storage (blackboard) without polluting game state or normal saves.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Storage key identifier"
                        },
                        "value": {
                            "description": "Arbitrary JSON value to store (object, array, string, number, bool)"
                        },
                        "namespace": {
                            "type": "string",
                            "description": "Optional namespace partition (default: 'default')"
                        }
                    },
                    "required": ["key", "value"]
                }
            }),
            serde_json::json!({
                "name": "mcp_store_get",
                "description": "Retrieve a stored value from dedicated MCP agent storage by key and optional namespace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Storage key identifier"
                        },
                        "namespace": {
                            "type": "string",
                            "description": "Optional namespace partition (default: 'default')"
                        }
                    },
                    "required": ["key"]
                }
            }),
            serde_json::json!({
                "name": "mcp_store_delete",
                "description": "Delete a key-value entry from dedicated MCP agent storage.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Storage key identifier"
                        },
                        "namespace": {
                            "type": "string",
                            "description": "Optional namespace partition (default: 'default')"
                        }
                    },
                    "required": ["key"]
                }
            }),
            serde_json::json!({
                "name": "mcp_store_list",
                "description": "List keys stored in dedicated MCP storage filtered by namespace or key prefix.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "prefix": {
                            "type": "string",
                            "description": "Optional prefix to filter keys"
                        },
                        "namespace": {
                            "type": "string",
                            "description": "Optional namespace partition to list within"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "mcp_store_clear",
                "description": "Clear all entries in dedicated MCP agent storage, or only entries within a specific namespace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "namespace": {
                            "type": "string",
                            "description": "Optional namespace to clear. If omitted, clears all entries across all namespaces."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "mcp_store_export",
                "description": "Export the entire MCP dedicated storage to a JSON file or inline JSON object.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Optional destination file path on disk. If omitted, returns full storage data inline."
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "mcp_store_import",
                "description": "Import key-value entries into MCP dedicated storage from a file or inline JSON object.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Optional source file path on disk"
                        },
                        "data": {
                            "type": "object",
                            "description": "Optional JSON key-value map to import directly"
                        },
                        "overwrite": {
                            "type": "boolean",
                            "description": "Whether to overwrite existing keys (default: true)"
                        }
                    }
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
                        Ok(Some(snap)) => {
                            self.last_snapshots.insert(pid, (Instant::now(), snap.clone()));
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
                        Ok(None) => {
                            let snap = self.last_snapshots.get(&pid).map(|(_, s)| s.clone());
                            if let Some(snap) = snap {
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
                            } else {
                                ("No snapshot available".to_string(), false)
                            }
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
                    "key" => {
                        let action_str = arguments.get("action").and_then(|v| v.as_str()).unwrap_or("down");
                        let action = match action_str {
                            "up" => crate::types::KeyAction::Up,
                            _ => crate::types::KeyAction::Down,
                        };
                        PlayerInput::Key {
                            code: arguments
                                .get("code")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            action,
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
                        }
                    }
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
                    Ok(Some(snap)) => {
                        self.last_snapshots.insert(pid, (Instant::now(), snap.clone()));
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
                    Ok(None) => {
                        let snap = self.last_snapshots.get(&pid).map(|(_, s)| s.clone());
                        if let Some(snap) = snap {
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
                        } else {
                            ("No snapshot available".to_string(), false)
                        }
                    }
                    Err(err) => (err, true),
                }
            }
            "reload_scripts" => {
                let clean = arguments
                    .get("clean")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                match self.logic.force_reload(clean) {
                    Ok(()) => ("Reloaded successfully".to_string(), false),
                    Err(err) => (err, true),
                }
            }
            "pause_clock" => {
                self.is_paused.store(true, Ordering::Relaxed);
                let out = serde_json::json!({
                    "paused": true,
                    "message": "Physical simulation clock paused"
                });
                (serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()), false)
            }
            "resume_clock" => {
                self.is_paused.store(false, Ordering::Relaxed);
                let out = serde_json::json!({
                    "paused": false,
                    "message": "Physical simulation clock resumed"
                });
                (serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()), false)
            }
            "step_simulation" => {
                let start_time = Instant::now();
                let steps = arguments.get("steps").and_then(|v| v.as_u64()).unwrap_or(1);
                let dt = arguments.get("dt").and_then(|v| v.as_f64()).unwrap_or(0.016) as f32;
                let include_snapshot = arguments.get("include_snapshot").and_then(|v| v.as_bool()).unwrap_or(false);
                let snap_pid = arguments.get("snapshot_player_id").and_then(|v| v.as_u64()).unwrap_or(1);
                let include_diff = arguments.get("include_state_diff").and_then(|v| v.as_bool()).unwrap_or(false);

                if let Some(seed_val) = arguments.get("seed").and_then(|v| v.as_i64()) {
                    self.logic.set_random_seed(Some(seed_val as u64));
                }

                let initial_state = if include_diff {
                    self.logic.export_state().ok()
                } else {
                    None
                };

                let empty_vec = Vec::new();
                let input_items = arguments
                    .get("inputs")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty_vec);

                enum InjectedAction {
                    Cmd(String),
                    Key {
                        code: String,
                        action: crate::types::KeyAction,
                        shift: bool,
                        ctrl: bool,
                        alt: bool,
                    },
                    UI {
                        action: String,
                        payload: serde_json::Value,
                    },
                    Mouse {
                        x: i32,
                        y: i32,
                        button: String,
                    },
                }

                struct ScheduledItem {
                    step: u64,
                    player_id: u64,
                    action: InjectedAction,
                }

                let mut scheduled: Vec<ScheduledItem> = Vec::new();
                for item in input_items {
                    let pid = item.get("player_id").and_then(|v| v.as_u64()).unwrap_or(1);
                    let step = item.get("step").and_then(|v| v.as_u64()).unwrap_or(0);
                    let input_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("key");
                    match input_type {
                        "cmd" => {
                            if let Some(cmd_str) = item.get("command").and_then(|v| v.as_str()) {
                                scheduled.push(ScheduledItem {
                                    step,
                                    player_id: pid,
                                    action: InjectedAction::Cmd(cmd_str.to_string()),
                                });
                            }
                        }
                        "key" => {
                            let code = item.get("code").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let shift = item.get("shift").and_then(|v| v.as_bool()).unwrap_or(false);
                            let ctrl = item.get("ctrl").and_then(|v| v.as_bool()).unwrap_or(false);
                            let alt = item.get("alt").and_then(|v| v.as_bool()).unwrap_or(false);
                            let hold_ticks = item.get("hold_ticks").and_then(|v| v.as_u64());

                            if let Some(n) = hold_ticks {
                                scheduled.push(ScheduledItem {
                                    step,
                                    player_id: pid,
                                    action: InjectedAction::Key {
                                        code: code.clone(),
                                        action: crate::types::KeyAction::Down,
                                        shift,
                                        ctrl,
                                        alt,
                                    },
                                });
                                scheduled.push(ScheduledItem {
                                    step: step + n,
                                    player_id: pid,
                                    action: InjectedAction::Key {
                                        code,
                                        action: crate::types::KeyAction::Up,
                                        shift,
                                        ctrl,
                                        alt,
                                    },
                                });
                            } else {
                                let action_str = item.get("action").and_then(|v| v.as_str()).unwrap_or("down");
                                let action = match action_str {
                                    "up" => crate::types::KeyAction::Up,
                                    _ => crate::types::KeyAction::Down,
                                };
                                scheduled.push(ScheduledItem {
                                    step,
                                    player_id: pid,
                                    action: InjectedAction::Key {
                                        code,
                                        action,
                                        shift,
                                        ctrl,
                                        alt,
                                    },
                                });
                            }
                        }
                        "ui" => {
                            let action = item.get("action").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let payload = item.get("payload").cloned().unwrap_or(serde_json::Value::Null);
                            scheduled.push(ScheduledItem {
                                step,
                                player_id: pid,
                                action: InjectedAction::UI { action, payload },
                            });
                        }
                        "mouse" => {
                            let x = item.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let y = item.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let button = item.get("button").and_then(|v| v.as_str()).unwrap_or("left").to_string();
                            scheduled.push(ScheduledItem {
                                step,
                                player_id: pid,
                                action: InjectedAction::Mouse { x, y, button },
                            });
                        }
                        _ => {}
                    }
                }

                for step_idx in 0..steps {
                    // Inject inputs scheduled for this step
                    for item in &scheduled {
                        if item.step == step_idx {
                            match &item.action {
                                InjectedAction::Cmd(cmd_str) => {
                                    let trimmed = cmd_str.trim();
                                    if trimmed.starts_with('!') {
                                        let code = trimmed.strip_prefix('!').unwrap_or(trimmed).trim();
                                        let _ = self.logic.eval_repl(code);
                                    } else {
                                        let _ = self.logic.on_command(item.player_id, trimmed);
                                    }
                                }
                                InjectedAction::Key { code, action, shift, ctrl, alt } => {
                                    let _ = self.logic.on_input(item.player_id, &PlayerInput::Key {
                                        code: code.clone(),
                                        action: *action,
                                        shift: *shift,
                                        ctrl: *ctrl,
                                        alt: *alt,
                                    });
                                }
                                InjectedAction::UI { action, payload } => {
                                    let _ = self.logic.on_ui_action(item.player_id, action, payload);
                                }
                                InjectedAction::Mouse { x, y, button } => {
                                    let _ = self.logic.on_input(item.player_id, &PlayerInput::Mouse {
                                        x: *x,
                                        y: *y,
                                        button: button.clone(),
                                    });
                                }
                            }
                        }
                    }

                    // Advance world simulation step
                    let tick_id = self.handle.increment_server_tick();
                    if let Err(e) = self.logic.on_tick(dt, tick_id) {
                        return (format!("step_simulation tick failed at step {}: {}", step_idx, e), true);
                    }
                }

                let final_tick = self.handle.server_tick();
                let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

                let mut out = serde_json::json!({
                    "steps_executed": steps,
                    "final_tick": final_tick,
                    "duration_ms": duration_ms,
                });

                if include_snapshot {
                    let pid_key = snap_pid.to_string();
                    let p = self.logic.export_state().ok().and_then(|st| {
                        st.get("players").and_then(|pls| pls.get(&pid_key)).cloned()
                    });
                    let (px, py) = if let Some(ref pval) = p {
                        (pval.get("x").and_then(|v| v.as_i64()).unwrap_or(0),
                         pval.get("y").and_then(|v| v.as_i64()).unwrap_or(0))
                    } else {
                        (0, 0)
                    };
                    let maybe_snap = match self.logic.render_snapshot(snap_pid) {
                        Ok(Some(s)) => {
                            self.last_snapshots.insert(snap_pid, (Instant::now(), s.clone()));
                            Some(s)
                        }
                        Ok(None) => self.last_snapshots.get(&snap_pid).map(|(_, s)| s.clone()),
                        Err(_) => None,
                    };
                    if let Some(snap) = maybe_snap {
                        out["snapshot"] = serde_json::json!({
                            "player_id": snap_pid,
                            "x": px,
                            "y": py,
                            "ascii_screen": strip_ansi_codes(&snap.screen),
                            "cols": snap.cols,
                            "rows": snap.rows,
                            "hud": snap.hud,
                        });
                    }
                }

                if let Some(proj_arr) = arguments.get("state_projection").and_then(|v| v.as_array()) {
                    let paths: Vec<String> = proj_arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if let Ok(st) = self.logic.export_state() {
                        let proj_val = crate::logic::project_json_value(&st, &paths);
                        out["projected_state"] = proj_val.clone();
                        out["state_projection"] = proj_val;
                    }
                }

                if include_diff {
                    let diff_path_prefix = arguments.get("diff_path_prefix").and_then(|v| v.as_str());
                    let diff_exclude_keys: Vec<String> = if let Some(arr) = arguments.get("diff_exclude_keys").and_then(|v| v.as_array()) {
                        arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                    } else {
                        vec!["chunks".to_string()]
                    };
                    let diff_exclude_refs: Vec<&str> = diff_exclude_keys.iter().map(|s| s.as_str()).collect();

                    if let (Some(old_st), Ok(new_st)) = (initial_state, self.logic.export_state()) {
                        out["state_diff"] = crate::logic::compute_json_diff_scoped(
                            &old_st,
                            &new_st,
                            diff_path_prefix,
                            Some(&diff_exclude_refs),
                        );
                    }
                }

                (serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()), false)
            }
            "get_state_diff" => {
                let current_state = match self.logic.export_state() {
                    Ok(st) => st,
                    Err(e) => return (format!("Failed to export current world state: {}", e), true),
                };
                let old_state = arguments.get("old_state").unwrap_or(&current_state);
                let new_state = arguments.get("new_state").unwrap_or(&current_state);
                let path_prefix = arguments.get("path_prefix").and_then(|v| v.as_str());
                let exclude_keys_vec: Option<Vec<&str>> = arguments
                    .get("exclude_keys")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect());
                let diff = crate::logic::compute_json_diff_scoped(
                    old_state,
                    new_state,
                    path_prefix,
                    exclude_keys_vec.as_deref(),
                );
                (serde_json::to_string_pretty(&diff).unwrap_or_else(|_| diff.to_string()), false)
            }
            "query_state" => {
                let Some(paths_arr) = arguments.get("paths").and_then(|v| v.as_array()) else {
                    return ("Missing required parameter 'paths' (array of strings)".to_string(), true);
                };
                let paths: Vec<String> = paths_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                let current_state = match self.logic.export_state() {
                    Ok(st) => st,
                    Err(e) => return (format!("Failed to export world state: {}", e), true),
                };
                let projected = crate::logic::project_json_value(&current_state, &paths);
                (serde_json::to_string_pretty(&projected).unwrap_or_else(|_| projected.to_string()), false)
            }
            "eval_assertion" => {
                let timeout_ms = arguments.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(1000);
                if let Some(exprs_val) = arguments.get("exprs").and_then(|v| v.as_array()) {
                    let exprs: Vec<String> = exprs_val
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    let summary = self.logic.eval_assertions(&exprs, timeout_ms);
                    (serde_json::to_string_pretty(&summary).unwrap_or_else(|_| summary.to_string()), false)
                } else if let Some(code) = arguments.get("code").or_else(|| arguments.get("expr")).and_then(|v| v.as_str()) {
                    match self.logic.eval_assertion(code, timeout_ms) {
                        Ok(res_val) => {
                            let is_failure = res_val
                                .as_object()
                                .and_then(|obj| obj.get("passed"))
                                .and_then(|p| p.as_bool())
                                == Some(false);
                            let out = if is_failure {
                                res_val
                            } else {
                                serde_json::json!({
                                    "passed": true,
                                    "result": res_val,
                                })
                            };
                            (serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()), false)
                        }
                        Err(err) => {
                            let out = serde_json::json!({
                                "passed": false,
                                "error": err,
                            });
                            (serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()), false)
                        }
                    }
                } else {
                    ("Missing required parameter: provide 'code'/'expr' string or 'exprs' array".to_string(), true)
                }
            }
            "mcp_store_set" => {
                let Some(key) = arguments.get("key").and_then(|v| v.as_str()) else {
                    return ("Missing required string parameter 'key'".to_string(), true);
                };
                if key.trim().is_empty() {
                    return ("Parameter 'key' cannot be empty".to_string(), true);
                }
                let Some(val) = arguments.get("value") else {
                    return ("Missing required parameter 'value'".to_string(), true);
                };
                let namespace = arguments.get("namespace").and_then(|v| v.as_str());
                self.mcp_store_set_scoped(namespace, key, val.clone());
                let (ns, sub) = resolve_namespace_and_key(namespace, key);
                let out = serde_json::json!({
                    "success": true,
                    "namespace": ns,
                    "key": sub,
                    "composite_key": format!("{}:{}", ns, sub)
                });
                (
                    serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                    false,
                )
            }
            "mcp_store_get" => {
                let Some(key) = arguments.get("key").and_then(|v| v.as_str()) else {
                    return ("Missing required string parameter 'key'".to_string(), true);
                };
                let namespace = arguments.get("namespace").and_then(|v| v.as_str());
                let (ns, sub) = resolve_namespace_and_key(namespace, key);
                let val = self.mcp_store_get_scoped(namespace, key);
                let found = val.is_some();
                let out = serde_json::json!({
                    "found": found,
                    "namespace": ns,
                    "key": sub,
                    "value": val.unwrap_or(serde_json::Value::Null)
                });
                (
                    serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                    false,
                )
            }
            "mcp_store_delete" => {
                let Some(key) = arguments.get("key").and_then(|v| v.as_str()) else {
                    return ("Missing required string parameter 'key'".to_string(), true);
                };
                let namespace = arguments.get("namespace").and_then(|v| v.as_str());
                let (ns, sub) = resolve_namespace_and_key(namespace, key);
                let deleted = self.mcp_store_delete_scoped(namespace, key);
                let out = serde_json::json!({
                    "success": true,
                    "deleted": deleted,
                    "namespace": ns,
                    "key": sub
                });
                (
                    serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                    false,
                )
            }
            "mcp_store_list" => {
                let prefix = arguments.get("prefix").and_then(|v| v.as_str());
                let namespace = arguments.get("namespace").and_then(|v| v.as_str());
                let keys = self.mcp_store_list_scoped(namespace, prefix);
                let out = serde_json::json!({
                    "namespace": namespace,
                    "prefix": prefix,
                    "count": keys.len(),
                    "keys": keys
                });
                (
                    serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                    false,
                )
            }
            "mcp_store_clear" => {
                let namespace = arguments.get("namespace").and_then(|v| v.as_str());
                let count = self.mcp_store_clear_scoped(namespace);
                let out = serde_json::json!({
                    "success": true,
                    "namespace": namespace,
                    "cleared_count": count
                });
                (
                    serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                    false,
                )
            }
            "mcp_store_export" => {
                if let Some(path_str) = arguments.get("path").and_then(|v| v.as_str()) {
                    let path = Path::new(path_str);
                    match self.mcp_store_export_to_file(path) {
                        Ok(()) => {
                            let count = self.mcp_storage.read().len();
                            let out = serde_json::json!({
                                "success": true,
                                "path": path_str,
                                "count": count
                            });
                            (
                                serde_json::to_string_pretty(&out)
                                    .unwrap_or_else(|_| out.to_string()),
                                false,
                            )
                        }
                        Err(e) => (
                            format!("Failed to export MCP store to {}: {}", path_str, e),
                            true,
                        ),
                    }
                } else {
                    let guard = self.mcp_storage.read();
                    let map_val = serde_json::to_value(&*guard).unwrap_or(serde_json::Value::Null);
                    let out = serde_json::json!({
                        "count": guard.len(),
                        "data": map_val
                    });
                    (
                        serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                        false,
                    )
                }
            }
            "mcp_store_import" => {
                let overwrite = arguments
                    .get("overwrite")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                if let Some(path_str) = arguments.get("path").and_then(|v| v.as_str()) {
                    let path = Path::new(path_str);
                    match self.mcp_store_import_from_file(path, overwrite) {
                        Ok(count) => {
                            let out = serde_json::json!({
                                "success": true,
                                "path": path_str,
                                "imported_count": count
                            });
                            (
                                serde_json::to_string_pretty(&out)
                                    .unwrap_or_else(|_| out.to_string()),
                                false,
                            )
                        }
                        Err(e) => (
                            format!("Failed to import MCP store from {}: {}", path_str, e),
                            true,
                        ),
                    }
                } else if let Some(serde_json::Value::Object(map)) = arguments.get("data") {
                    let mut imported_count = 0;
                    let mut guard = self.mcp_storage.write();
                    for (k, v) in map {
                        if overwrite || !guard.contains_key(k) {
                            guard.insert(k.clone(), v.clone());
                            imported_count += 1;
                        }
                    }
                    let out = serde_json::json!({
                        "success": true,
                        "imported_count": imported_count
                    });
                    (
                        serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string()),
                        false,
                    )
                } else {
                    (
                        "Missing required parameter: either 'path' or 'data'".to_string(),
                        true,
                    )
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
                if let Some(save_path) = self.logic.save_file() {
                    if !self.mcp_storage.read().is_empty() {
                        let mcp_path = save_path.with_extension("mcp.json");
                        if let Err(e) = self.mcp_store_export_to_file(&mcp_path) {
                            tracing::error!(error = %e, "MCP storage autosave failed");
                        }
                    }
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
