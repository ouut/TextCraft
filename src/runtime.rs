use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use game_net::PlayerId;
use game_server::{ServerConfig, ServerHandle};

use crate::logic::{OutboundMessage, SchemeLogic};
use crate::types::{PlayerInput, PlayerSnapshot, TextCraftCommand, TextCraftEvent};

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

    /// Disable hot reloading file watcher for Scheme scripts
    #[arg(long, default_value_t = false)]
    pub no_watch_scripts: bool,

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
}

/// Core server gateway and session scheduling coordinator.
pub struct TextCraftServer {
    pub handle: ServerHandle<PlayerInput, TextCraftCommand, PlayerSnapshot, TextCraftEvent>,
    pub logic: SchemeLogic,
    pub admin_token: String,
    pub time_scale: f32,
    pub autosave_sec: u64,
    player_tokens: HashMap<PlayerId, String>,
}

impl TextCraftServer {
    /// Starts the server instance with parsed CLI arguments.
    pub async fn start(args: ServerArgs) -> Result<Self, Box<dyn std::error::Error>> {
        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        let config = ServerConfig::new(addr).with_embedded_assets(&EMBEDDED_WEB_ASSETS);
        let mut logic = SchemeLogic::new(args.scripts_dir, !args.no_watch_scripts, args.save_file)?;
        logic.init()?;

        Self::start_with_config(
            config,
            logic,
            args.admin_token,
            args.time_scale,
            args.autosave_sec,
        )
        .await
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
            player_tokens: HashMap::new(),
        })
    }

    /// Returns the bound local socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.handle.local_addr()
    }

    /// Performs a single simulation tick: session events, inputs, tick advance, and snapshots.
    pub fn tick(&mut self, dt: f32) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Script hot reloading check
        let _ = self.logic.check_reload();

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
                }
                game_server::SessionEvent::PlayerDisconnected { player_id, .. }
                | game_server::SessionEvent::PlayerTerminated { player_id } => {
                    self.player_tokens.remove(&player_id);
                    if let Err(e) = self.logic.on_player_leave(player_id) {
                        tracing::warn!(player_id, error = %e, "on_player_leave error");
                    }
                }
                game_server::SessionEvent::PlayerReconnected { player_id } => {
                    tracing::info!(player_id, "Player reconnected");
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
        for pid in self.handle.connected_players() {
            match self.logic.render_snapshot(pid) {
                Ok(snapshot) => {
                    self.handle.send_snapshot(pid, snapshot);
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
                    if let Err(e) = self.logic.on_command(pid, &raw_cmd) {
                        tracing::warn!(player_id = pid, error = %e, "on_command error");
                    }
                }
            }
            PlayerInput::Key {
                code,
                shift,
                ctrl,
                alt,
            } => {
                let key_input = PlayerInput::Key {
                    code,
                    shift,
                    ctrl,
                    alt,
                };
                if let Err(e) = self.logic.on_input(pid, &key_input) {
                    tracing::warn!(player_id = pid, error = %e, "on_input error");
                }
            }
            PlayerInput::UIAction { action, payload } => {
                if let Err(e) = self.logic.on_ui_action(pid, &action, &payload) {
                    tracing::warn!(player_id = pid, error = %e, "on_ui_action error");
                }
            }
            PlayerInput::Mouse { x, y, button } => {
                let mouse_val = serde_json::json!({
                    "type": "mouse",
                    "x": x,
                    "y": y,
                    "button": button,
                });
                if let Err(e) = self.logic.on_ui_action(pid, "mouse_click", &mouse_val) {
                    tracing::debug!(player_id = pid, error = %e, "mouse action unhandled");
                }
            }
        }
    }

    fn process_admin_eval(&mut self, pid: PlayerId, code: &str) {
        let is_admin = self.player_tokens.get(&pid) == Some(&self.admin_token);

        if is_admin {
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
                    self.handle.send_event(pid, TextCraftEvent::Error { text: err });
                }
            }
        } else {
            self.handle.send_event(
                pid,
                TextCraftEvent::Error {
                    text: "Permission denied: admin token required".to_string(),
                },
            );
        }
    }

    /// Runs the adaptive dynamic FPS main tick loop until shutdown or Ctrl+C.
    pub async fn run_loop(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut last_tick_time = Instant::now();
        let mut last_autosave = Instant::now();
        let autosave_interval = Duration::from_secs(self.autosave_sec);

        tracing::info!(
            local_addr = %self.handle.local_addr(),
            admin_token = %self.admin_token,
            "TextCraft 3.0 Server running"
        );

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
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received Ctrl+C interrupt, shutting down server...");
                    break;
                }
                _ = tokio::time::sleep(sleep_duration) => {}
            }
        }

        // On server shutdown, save state
        if let Err(e) = self.logic.save_state_to_file() {
            tracing::error!(error = %e, "Failed to save state on shutdown");
        }
        self.handle.shutdown();
        Ok(())
    }
}

/// Convenience function to start and run the server from CLI arguments.
pub async fn run_server(args: ServerArgs) -> Result<(), Box<dyn std::error::Error>> {
    let server = TextCraftServer::start(args).await?;
    server.run_loop().await
}
