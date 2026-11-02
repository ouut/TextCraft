use flecs_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 24-bit TrueColor representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const RED: Color = Color { r: 255, g: 0, b: 0 };
    pub const GREEN: Color = Color { r: 0, g: 255, b: 0 };
    pub const BLUE: Color = Color { r: 0, g: 0, b: 255 };
    pub const YELLOW: Color = Color { r: 255, g: 255, b: 0 };
    pub const CYAN: Color = Color { r: 0, g: 255, b: 255 };
    pub const MAGENTA: Color = Color { r: 255, g: 0, b: 255 };

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[inline]
    pub fn to_tuple(self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }

    #[inline]
    pub fn from_tuple(t: (u8, u8, u8)) -> Self {
        Self { r: t.0, g: t.1, b: t.2 }
    }
}

impl From<(u8, u8, u8)> for Color {
    #[inline]
    fn from(t: (u8, u8, u8)) -> Self {
        Self::from_tuple(t)
    }
}

impl From<[u8; 3]> for Color {
    #[inline]
    fn from(arr: [u8; 3]) -> Self {
        Self { r: arr[0], g: arr[1], b: arr[2] }
    }
}

/// Rendering and collision layer constants.
pub const LAYER_TERRAIN: u8 = 0;
pub const LAYER_ITEM_LIQUID: u8 = 1;
pub const LAYER_CREATURE: u8 = 2;
pub const LAYER_OVERLAY: u8 = 3;

/// Grid position component representing world coordinates and rendering layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Component)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
    pub layer: u8,
}

impl GridPos {
    #[inline]
    pub const fn new(x: i32, y: i32, layer: u8) -> Self {
        Self { x, y, layer }
    }

    #[inline]
    pub const fn terrain(x: i32, y: i32) -> Self {
        Self { x, y, layer: LAYER_TERRAIN }
    }

    #[inline]
    pub const fn item(x: i32, y: i32) -> Self {
        Self { x, y, layer: LAYER_ITEM_LIQUID }
    }

    #[inline]
    pub const fn creature(x: i32, y: i32) -> Self {
        Self { x, y, layer: LAYER_CREATURE }
    }

    #[inline]
    pub const fn overlay(x: i32, y: i32) -> Self {
        Self { x, y, layer: LAYER_OVERLAY }
    }
}

/// Visual representation and physical collision/opacity attributes of an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Component)]
pub struct VisualGlyph {
    pub ch: char,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub solid: bool,
    pub opaque: bool,
    pub double_width: bool,
}

impl Default for VisualGlyph {
    #[inline]
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: None,
            bg: None,
            solid: false,
            opaque: false,
            double_width: false,
        }
    }
}

impl VisualGlyph {
    #[inline]
    pub const fn new(
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
        solid: bool,
        opaque: bool,
    ) -> Self {
        Self {
            ch,
            fg,
            bg,
            solid,
            opaque,
            double_width: false,
        }
    }

    #[inline]
    pub const fn ascii(ch: char, fg: Option<Color>, bg: Option<Color>) -> Self {
        Self {
            ch,
            fg,
            bg,
            solid: false,
            opaque: false,
            double_width: false,
        }
    }

    #[inline]
    pub const fn solid(ch: char, fg: Option<Color>, bg: Option<Color>) -> Self {
        Self {
            ch,
            fg,
            bg,
            solid: true,
            opaque: true,
            double_width: false,
        }
    }

    #[inline]
    pub const fn with_double_width(mut self, dw: bool) -> Self {
        self.double_width = dw;
        self
    }
}

/// Player network session component linking entity to player state and viewport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Component)]
pub struct PlayerSession {
    pub player_id: u64,
    pub online: bool,
    pub viewport_cols: u16,
    pub viewport_rows: u16,
}

impl PlayerSession {
    pub fn new(player_id: u64, viewport_cols: u16, viewport_rows: u16) -> Self {
        Self {
            player_id,
            online: true,
            viewport_cols,
            viewport_rows,
        }
    }
}

/// Free-form dynamic properties storage for Scheme VM integration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Component)]
pub struct DynamicProps {
    pub values: HashMap<String, serde_json::Value>,
}

impl DynamicProps {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    #[inline]
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.values.get(key).and_then(|v| v.as_i64())
    }

    #[inline]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.values.get(key).and_then(|v| v.as_f64())
    }

    #[inline]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.values.get(key).and_then(|v| v.as_str())
    }

    #[inline]
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.values.get(key).and_then(|v| v.as_bool())
    }

    #[inline]
    pub fn set(&mut self, key: impl Into<String>, val: serde_json::Value) {
        self.values.insert(key.into(), val);
    }

    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.values.remove(key)
    }

    #[inline]
    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}

// ----------------------------------------------------------------------------
// Built-in Marker Tags & Relationship Tags
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Monster;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Player;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Item;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Terrain;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Dead;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Liquid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct Burning;

// Relationships
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct EquippedOn;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct PartOf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Component)]
pub struct CoveredWith;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constructors() {
        let c = Color::new(10, 20, 30);
        assert_eq!(c.to_tuple(), (10, 20, 30));
        assert_eq!(Color::from((10, 20, 30)), c);
        assert_eq!(Color::from([10, 20, 30]), c);
    }

    #[test]
    fn test_grid_pos_constructors() {
        let p1 = GridPos::terrain(5, 10);
        assert_eq!(p1.layer, LAYER_TERRAIN);
        let p2 = GridPos::creature(5, 10);
        assert_eq!(p2.layer, LAYER_CREATURE);
        assert_eq!(p1.x, p2.x);
        assert_eq!(p1.y, p2.y);
    }

    #[test]
    fn test_dynamic_props() {
        let mut props = DynamicProps::new();
        props.set("hp", serde_json::json!(100));
        props.set("name", serde_json::json!("Hero"));
        props.set("active", serde_json::json!(true));

        assert_eq!(props.get_i64("hp"), Some(100));
        assert_eq!(props.get_str("name"), Some("Hero"));
        assert_eq!(props.get_bool("active"), Some(true));
        assert!(props.has("hp"));
        assert!(!props.has("mp"));

        assert_eq!(props.remove("hp"), Some(serde_json::json!(100)));
        assert!(!props.has("hp"));
    }
}
