use std::fs;
use textcraft::logic::SchemeLogic;
use textcraft::types::PlayerInput;

/// Test 1: Empty script files or completely missing hook functions should boot gracefully,
/// default correctly, not crash or log continuous errors.
#[test]
fn test_empty_scripts_graceful_fallbacks_and_defaults() {
    let temp_dir = std::env::temp_dir().join(format!("tc_empty_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    let prelude = temp_dir.join("prelude.scm");
    let main = temp_dir.join("main.scm");
    fs::write(&prelude, "").unwrap();
    fs::write(&main, "").unwrap();

    let mut logic = SchemeLogic::new(&temp_dir, false, None).expect("SchemeLogic::new should succeed on empty scripts");
    
    // Check init with empty scripts
    let init_res = logic.init();
    assert!(init_res.is_ok(), "init should succeed with empty scripts: {:?}", init_res.err());

    // Verify hooks are detected as not present
    assert!(!logic.has_hook("engine:on-tick"));
    assert!(!logic.has_hook("engine:render-snapshot"));
    assert!(!logic.has_hook("engine:target-fps"));
    assert!(!logic.has_hook("engine:viewport-size"));
    assert!(!logic.has_hook("engine:on-player-join"));

    // Verify default target FPS is 60
    assert_eq!(logic.query_target_fps(), 60);

    // Verify default viewport size is (80, 24)
    assert_eq!(logic.query_viewport_size(1), (80, 24));

    // Verify on_tick skips with Ok(())
    let tick_res = logic.on_tick(0.016, 1);
    assert!(tick_res.is_ok());

    // Verify default minimal snapshot (80x24 empty screen + empty HUD)
    let snapshot = logic.render_snapshot(1).expect("render_snapshot should return default minimalist snapshot");
    assert_eq!(snapshot.cols, 80);
    assert_eq!(snapshot.rows, 24);
    assert_eq!(snapshot.screen, "");
    assert_eq!(snapshot.hud, serde_json::json!({}));

    // Verify event hooks skip cleanly with Ok(())
    assert!(logic.on_player_join(1, "token123").is_ok());
    assert!(logic.on_player_disconnect(1).is_ok());
    assert!(logic.on_player_reconnect(1).is_ok());
    assert!(logic.on_player_leave(1).is_ok());
    assert!(logic.on_command(1, "look").is_ok());
    assert!(logic.on_ui_action(1, "click", &serde_json::json!({})).is_ok());
    assert!(logic.on_input(1, &PlayerInput::Cmd { input: "test".to_string() }).is_ok());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test 2: Scheme script compile error reporting has rich diagnostic information (filename, line, column, snippet).
#[test]
fn test_scheme_compile_error_rich_diagnostics() {
    let temp_dir = std::env::temp_dir().join(format!("tc_err_diag_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    let main_scm = temp_dir.join("main.scm");
    
    // Write invalid Scheme code with an undefined variable
    fs::write(
        &main_scm,
        "(define (engine:on-tick dt tick)\n  (+ dt unresolved_symbol_xyz))\n",
    )
    .unwrap();

    let mut logic = SchemeLogic::new(&temp_dir, false, None).expect("SchemeLogic::new should succeed");
    let err = logic.init().unwrap_err();
    let err_msg = err.to_string();

    // Verify error message contains file name, line number, and variable name
    assert!(err_msg.contains("main.scm"), "Error message should contain filename: {}", err_msg);
    assert!(err_msg.contains("unresolved_symbol_xyz"), "Error message should mention the undefined symbol: {}", err_msg);
    assert!(err_msg.contains("2:") || err_msg.contains("line"), "Error message should contain line number: {}", err_msg);

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test 3: Scheme REPL evaluation error reporting has rich diagnostic information.
#[test]
fn test_scheme_repl_eval_error_rich_diagnostics() {
    let temp_dir = std::env::temp_dir().join(format!("tc_repl_diag_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    let main_scm = temp_dir.join("main.scm");
    fs::write(&main_scm, "").unwrap();

    let mut logic = SchemeLogic::new(&temp_dir, false, None).expect("SchemeLogic::new should succeed");
    logic.init().unwrap();

    // REPL evaluation with runtime division by zero
    let res = logic.eval_repl("(/ 10 0)");
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.contains("division by zero"), "Error should explain division by zero: {}", err);
    assert!(err.contains("(/ 10 0)"), "Error should include source code snippet: {}", err);

    // REPL evaluation with undefined variable
    let res2 = logic.eval_repl("(+ 1 imaginary_variable_abc)");
    assert!(res2.is_err());
    let err2 = res2.unwrap_err();
    assert!(err2.contains("imaginary_variable_abc"), "Error should mention undefined variable: {}", err2);
    assert!(err2.contains("(+ 1 imaginary_variable_abc)"), "Error should include code snippet: {}", err2);

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test 4: Dynamic hot reload updates hook cache.
#[test]
fn test_hot_reload_updates_hook_cache() {
    let temp_dir = std::env::temp_dir().join(format!("tc_reload_hooks_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&temp_dir).unwrap();
    let main_scm = temp_dir.join("main.scm");
    fs::write(&main_scm, "").unwrap();

    let mut logic = SchemeLogic::new(&temp_dir, false, None).expect("SchemeLogic::new should succeed");
    logic.init().unwrap();
    assert!(!logic.has_hook("engine:target-fps"));
    assert_eq!(logic.query_target_fps(), 60);

    // Now update script to define engine:target-fps returning 30
    fs::write(
        &main_scm,
        "(define (engine:target-fps) 30)\n",
    ).unwrap();

    let reload_res = logic.force_reload(false);
    assert!(reload_res.is_ok(), "force_reload should succeed: {:?}", reload_res.err());

    // Hook registry should now have engine:target-fps
    assert!(logic.has_hook("engine:target-fps"));
    assert_eq!(logic.query_target_fps(), 30);

    let _ = fs::remove_dir_all(&temp_dir);
}
