use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlayerInput {
    #[serde(rename = "key")]
    Key {
        code: String,
        #[serde(default)]
        shift: bool,
        #[serde(default)]
        ctrl: bool,
        #[serde(default)]
        alt: bool,
    },
    #[serde(rename = "cmd")]
    Cmd { input: String },
    #[serde(rename = "ui")]
    UIAction {
        action: String,
        #[serde(default)]
        payload: serde_json::Value,
    },
    #[serde(rename = "mouse")]
    Mouse {
        x: i32,
        y: i32,
        button: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TextCraftCommand {
    Input(PlayerInput),
    AdminEval { code: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub cols: u16,
    pub rows: u16,
    pub screen: String,
    pub hud: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum TextCraftEvent {
    Welcome { player_id: u64, message: String },
    Message { channel: String, text: String },
    Error { text: String },
}

/// Thread-safe in-memory key-value blackboard storage for MCP agent state, temporary variables, and checkpoints.
pub type McpStorage = std::sync::Arc<parking_lot::RwLock<std::collections::HashMap<String, serde_json::Value>>>;

/// Resolves a namespace and subkey from optional namespace and key strings.
///
/// Priority:
/// 1. If explicit namespace is given and not empty/default, returns (namespace, stripped_or_original_key).
/// 2. If key contains ':', splits into (namespace, subkey).
/// 3. Otherwise returns ("default", key).
pub fn resolve_namespace_and_key<'a>(
    namespace: Option<&'a str>,
    key: &'a str,
) -> (&'a str, &'a str) {
    let clean_ns = namespace.map(|s| s.trim()).filter(|s| !s.is_empty());
    match clean_ns {
        Some(ns) if ns != "default" => {
            if let Some(sub) = key.strip_prefix(ns).and_then(|k| k.strip_prefix(':')) {
                (ns, sub)
            } else {
                (ns, key)
            }
        }
        _ => {
            if let Some((ns, sub)) = key.split_once(':') {
                if !ns.is_empty() && !sub.is_empty() {
                    (ns, sub)
                } else {
                    ("default", key)
                }
            } else {
                ("default", key)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_input_key_serde() {
        let json = r#"{"type":"key","code":"KeyW"}"#;
        let input: PlayerInput = serde_json::from_str(json).unwrap();
        match input {
            PlayerInput::Key {
                code,
                shift,
                ctrl,
                alt,
            } => {
                assert_eq!(code, "KeyW");
                assert!(!shift);
                assert!(!ctrl);
                assert!(!alt);
            }
            _ => panic!("Expected PlayerInput::Key"),
        }

        let serialized = serde_json::to_string(&PlayerInput::Key {
            code: "KeyS".to_string(),
            shift: true,
            ctrl: false,
            alt: false,
        })
        .unwrap();
        assert!(serialized.contains(r#""type":"key""#));
        assert!(serialized.contains(r#""code":"KeyS""#));
        assert!(serialized.contains(r#""shift":true"#));
    }

    #[test]
    fn test_player_input_cmd_serde() {
        let json = r#"{"type":"cmd","input":"(say \"hello\")"}"#;
        let input: PlayerInput = serde_json::from_str(json).unwrap();
        match input {
            PlayerInput::Cmd { input } => {
                assert_eq!(input, "(say \"hello\")");
            }
            _ => panic!("Expected PlayerInput::Cmd"),
        }
    }

    #[test]
    fn test_player_input_ui_serde() {
        let json = r#"{"type":"ui","action":"use_item","payload":{"slot":2}}"#;
        let input: PlayerInput = serde_json::from_str(json).unwrap();
        match input {
            PlayerInput::UIAction { action, payload } => {
                assert_eq!(action, "use_item");
                assert_eq!(payload["slot"], 2);
            }
            _ => panic!("Expected PlayerInput::UIAction"),
        }

        let default_payload = r#"{"type":"ui","action":"close_inventory"}"#;
        let input2: PlayerInput = serde_json::from_str(default_payload).unwrap();
        match input2 {
            PlayerInput::UIAction { action, payload } => {
                assert_eq!(action, "close_inventory");
                assert_eq!(payload, serde_json::Value::Null);
            }
            _ => panic!("Expected PlayerInput::UIAction"),
        }
    }

    #[test]
    fn test_player_input_mouse_serde() {
        let json = r#"{"type":"mouse","x":100,"y":200,"button":"left"}"#;
        let input: PlayerInput = serde_json::from_str(json).unwrap();
        match input {
            PlayerInput::Mouse { x, y, button } => {
                assert_eq!(x, 100);
                assert_eq!(y, 200);
                assert_eq!(button, "left");
            }
            _ => panic!("Expected PlayerInput::Mouse"),
        }
    }

    #[test]
    fn test_textcraft_command_serde() {
        let cmd = TextCraftCommand::AdminEval {
            code: "(+ 1 2)".to_string(),
        };
        let s = serde_json::to_string(&cmd).unwrap();
        let deserialized: TextCraftCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_player_snapshot_serde() {
        let snapshot = PlayerSnapshot {
            cols: 80,
            rows: 24,
            screen: "test screen".to_string(),
            hud: serde_json::json!({
                "theme": "cyberpunk",
                "hp": 100,
                "mp": 50
            }),
        };
        let s = serde_json::to_string(&snapshot).unwrap();
        let deserialized: PlayerSnapshot = serde_json::from_str(&s).unwrap();
        assert_eq!(snapshot, deserialized);
    }

    #[test]
    fn test_textcraft_event_serde() {
        let ev = TextCraftEvent::Welcome {
            player_id: 42,
            message: "Welcome to TextCraft".to_string(),
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains(r#""event":"Welcome""#));
        assert!(s.contains(r#""player_id":42"#));
        let deserialized: TextCraftEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(ev, deserialized);

        let msg_ev = TextCraftEvent::Message {
            channel: "combat".to_string(),
            text: "Goblin defeated".to_string(),
        };
        let s2 = serde_json::to_string(&msg_ev).unwrap();
        assert!(s2.contains(r#""event":"Message""#));
        let deserialized2: TextCraftEvent = serde_json::from_str(&s2).unwrap();
        assert_eq!(msg_ev, deserialized2);

        let err_ev = TextCraftEvent::Error {
            text: "Syntax error".to_string(),
        };
        let s3 = serde_json::to_string(&err_ev).unwrap();
        assert!(s3.contains(r#""event":"Error""#));
        let deserialized3: TextCraftEvent = serde_json::from_str(&s3).unwrap();
        assert_eq!(err_ev, deserialized3);
    }
}
