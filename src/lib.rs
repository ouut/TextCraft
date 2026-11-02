pub mod logic;
pub mod runtime;
pub mod types;

pub use clap;
pub use logic::{OutboundMessage, SchemeLogic};
pub use runtime::{
    run_server, strip_ansi_codes, LogLevel, ServerArgs, ServerHandleExt, TextCraftServer,
    EMBEDDED_WEB_ASSETS,
};
pub use types::*;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use super::*;

    #[test]
    fn test_embedded_assets_present() {
        assert!(!EMBEDDED_WEB_ASSETS.is_empty());
        let (name, bytes, mime) = EMBEDDED_WEB_ASSETS[0];
        assert_eq!(name, "index.html");
        assert!(!bytes.is_empty());
        assert_eq!(mime, "text/html; charset=utf-8");
    }

    #[test]
    fn test_scheme_logic_lifecycle() {
        let scripts_dir = PathBuf::from("scripts");
        let dir = if scripts_dir.exists() {
            scripts_dir
        } else {
            PathBuf::from("../../scripts")
        };

        let mut logic = SchemeLogic::new(&dir, false, None).expect("Failed to initialize SchemeLogic");
        logic.init().expect("Failed to init Scheme scripts");

        // 1. Target FPS
        let fps = logic.query_target_fps();
        assert_eq!(fps, 60);

        // 2. Player Join
        logic.on_player_join(1, "test_token").expect("Player join failed");

        // 3. Render Snapshot
        let snap = logic.render_snapshot(1).expect("Render snapshot failed");
        assert_eq!(snap.cols, 80);
        assert_eq!(snap.rows, 24);
        assert!(!snap.screen.is_empty());
        assert_eq!(snap.hud["theme"], "cyberpunk");

        // 4. Tick
        logic.on_tick(0.016, 1).expect("Tick failed");

        // 5. Input
        let key_input = PlayerInput::Key {
            code: "KeyW".to_string(),
            shift: false,
            ctrl: false,
            alt: false,
        };
        logic.on_input(1, &key_input).expect("Key input failed");

        // 6. UI Action
        let payload = serde_json::json!({ "id": "potion" });
        logic.on_ui_action(1, "use_item", &payload).expect("UI Action failed");

        // 7. Command
        logic.on_command(1, "(help)").expect("Command failed");

        // 8. REPL
        let repl_res = logic.eval_repl("(+ 10 20)").expect("REPL failed");
        assert!(repl_res.contains("30"));

        // 9. State Export / Import
        let state = logic.export_state().expect("State export failed");
        assert!(state.is_object());
        logic.import_state(&state).expect("State import failed");

        // 10. Player Leave
        logic.on_player_leave(1).expect("Player leave failed");
    }

    #[test]
    fn test_state_saving_atomic() {
        let temp_dir = std::env::temp_dir().join(format!("textcraft_test_save_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let save_file = temp_dir.join("save.json");

        let scripts_dir = PathBuf::from("scripts");
        let dir = if scripts_dir.exists() {
            scripts_dir
        } else {
            PathBuf::from("../../scripts")
        };

        {
            let mut logic = SchemeLogic::new(&dir, false, Some(save_file.clone())).expect("SchemeLogic create failed");
            logic.init().expect("SchemeLogic init failed");
            logic.save_state_to_file().expect("Save state failed");
            assert!(save_file.exists());
        }

        {
            // Verify loading state
            let mut logic2 = SchemeLogic::new(&dir, false, Some(save_file.clone())).expect("SchemeLogic create failed");
            logic2.init().expect("SchemeLogic init failed");
            let state = logic2.export_state().expect("Export state failed");
            assert!(state.is_object());
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
