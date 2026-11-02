use flecs_ecs::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use crate::ecs::components::*;
use crate::ecs::fov::compute_fov;
use crate::ecs::noise::{self, SimplexNoise};
use crate::ecs::pathfinding;
use crate::ecs::spatial::SpatialGrid;

/// Central authoritative ECS World wrapping Flecs and maintaining the spatial index.
pub struct EcsWorld {
    pub world: World,
    pub spatial: SpatialGrid,
}

// Soundness: EcsWorld is protected by Mutex/RwLock in multi-threaded contexts,
// and all stored components implement Send + Sync.
unsafe impl Send for EcsWorld {}
unsafe impl Sync for EcsWorld {}

impl Default for EcsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl EcsWorld {
    /// Create and initialize a new `EcsWorld` with core components and spatial grid.
    pub fn new() -> Self {
        let world = World::new();

        // Register core foundation components
        world.component::<GridPos>();
        world.component::<VisualGlyph>();
        world.component::<PlayerSession>();
        world.component::<DynamicProps>();

        // Register marker tags
        world.component::<Monster>();
        world.component::<Player>();
        world.component::<Item>();
        world.component::<Terrain>();
        world.component::<Dead>();
        world.component::<Liquid>();
        world.component::<Burning>();

        // Register relationships
        world.component::<EquippedOn>();
        world.component::<PartOf>();
        world.component::<CoveredWith>();

        Self {
            world,
            spatial: SpatialGrid::new(),
        }
    }

    /// Spawn a new unnamed entity and return its numeric entity ID.
    pub fn spawn(&mut self) -> u64 {
        let e = self.world.entity();
        *e.id()
    }

    /// Spawn a new named entity and return its numeric entity ID.
    pub fn spawn_named(&mut self, name: &str) -> u64 {
        let e = self.world.entity_named(name);
        *e.id()
    }

    /// Check if an entity exists and is alive.
    pub fn exists(&self, eid: u64) -> bool {
        self.world.is_alive(eid)
    }

    /// Despawn an entity, removing it from spatial indexing and Flecs.
    pub fn despawn(&mut self, eid: u64) -> bool {
        if !self.exists(eid) {
            return false;
        }

        // Clean up from spatial grid if it has a GridPos
        let ent = self.world.entity_from_id(eid);
        if let Some(pos) = ent.cloned::<Option<&GridPos>>() {
            self.spatial.remove_entity(eid, pos.x, pos.y);
        }

        ent.destruct();
        true
    }

    /// Set an entity's physical grid coordinate and layer, updating spatial index.
    pub fn set_pos(&mut self, eid: u64, x: i32, y: i32, layer: u8) -> bool {
        if !self.exists(eid) {
            return false;
        }

        let ent = self.world.entity_from_id(eid);
        let glyph = ent.cloned::<Option<&VisualGlyph>>();
        let old_pos = ent.cloned::<Option<&GridPos>>();
        if let Some(old) = old_pos {
            self.spatial.move_entity(eid, old.x, old.y, x, y, layer, glyph);
        } else {
            self.spatial.insert(eid, x, y, layer, glyph);
        }

        ent.set(GridPos::new(x, y, layer));
        true
    }

    /// Get an entity's grid position if it exists.
    pub fn get_pos(&self, eid: u64) -> Option<GridPos> {
        if !self.exists(eid) {
            return None;
        }
        self.world.entity_from_id(eid).cloned::<Option<&GridPos>>()
    }

    /// Set an entity's visual glyph, colors, and physical attributes.
    pub fn set_glyph(
        &mut self,
        eid: u64,
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
        solid: bool,
        opaque: bool,
    ) -> bool {
        if !self.exists(eid) {
            return false;
        }
        let ent = self.world.entity_from_id(eid);
        let glyph = VisualGlyph::new(ch, fg, bg, solid, opaque);
        ent.set(glyph);

        if let Some(pos) = ent.cloned::<Option<&GridPos>>() {
            self.spatial.update_glyph(eid, pos.x, pos.y, glyph);
        }
        true
    }

    /// Get an entity's visual glyph if present.
    pub fn get_glyph(&self, eid: u64) -> Option<VisualGlyph> {
        if !self.exists(eid) {
            return None;
        }
        self.world.entity_from_id(eid).cloned::<Option<&VisualGlyph>>()
    }

    /// Set a free-form dynamic property on an entity.
    pub fn set_prop(&mut self, eid: u64, key: &str, val: serde_json::Value) -> bool {
        if !self.exists(eid) {
            return false;
        }
        let ent = self.world.entity_from_id(eid);
        let mut props = ent.cloned::<Option<&DynamicProps>>().unwrap_or_default();
        props.set(key, val);
        ent.set(props);
        true
    }

    /// Get a free-form dynamic property from an entity.
    pub fn get_prop(&self, eid: u64, key: &str) -> Option<serde_json::Value> {
        if !self.exists(eid) {
            return None;
        }
        self.world
            .entity_from_id(eid)
            .cloned::<Option<&DynamicProps>>()
            .and_then(|p| p.get(key).cloned())
    }

    /// Check whether an entity has a specific dynamic property.
    pub fn has_prop(&self, eid: u64, key: &str) -> bool {
        if !self.exists(eid) {
            return false;
        }
        self.world
            .entity_from_id(eid)
            .cloned::<Option<&DynamicProps>>()
            .map_or(false, |p| p.has(key))
    }

    /// Check whether the spatial index has no entities.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.spatial.is_empty()
    }

    /// Add a tag (named entity) to an entity.
    pub fn add_tag(&mut self, eid: u64, tag: &str) -> bool {
        if !self.exists(eid) {
            return false;
        }
        let tag_ent = self.world.entity_named(tag);
        let ent = self.world.entity_from_id(eid);
        ent.add(tag_ent);
        true
    }

    /// Remove a tag from an entity.
    pub fn remove_tag(&mut self, eid: u64, tag: &str) -> bool {
        if !self.exists(eid) {
            return false;
        }
        let tag_ent = self.world.entity_named(tag);
        let ent = self.world.entity_from_id(eid);
        ent.remove(tag_ent);
        true
    }

    /// Check whether an entity has a tag.
    pub fn has_tag(&self, eid: u64, tag: &str) -> bool {
        if !self.exists(eid) {
            return false;
        }
        let tag_ent = self.world.entity_named(tag);
        let ent = self.world.entity_from_id(eid);
        ent.has(tag_ent)
    }

    /// Relate `subject` to `target` with the given `relation` name: `(relation, target)`.
    pub fn relate(&mut self, subject: u64, relation: &str, target: u64) -> bool {
        if !self.exists(subject) || !self.exists(target) {
            return false;
        }
        let rel_ent = self.world.entity_named(relation);
        let tgt_ent = self.world.entity_from_id(target);
        self.world.entity_from_id(subject).add((rel_ent, tgt_ent));
        true
    }

    /// Remove the relationship `(relation, target)` from `subject`.
    pub fn unrelate(&mut self, subject: u64, relation: &str, target: u64) -> bool {
        if !self.exists(subject) || !self.exists(target) {
            return false;
        }
        let rel_ent = self.world.entity_named(relation);
        let tgt_ent = self.world.entity_from_id(target);
        self.world.entity_from_id(subject).remove((rel_ent, tgt_ent));
        true
    }

    /// Retrieve the target entity of a relationship on `subject`.
    pub fn target(&self, subject: u64, relation: &str) -> Option<u64> {
        if !self.exists(subject) {
            return None;
        }
        let rel_ent = self.world.entity_named(relation);
        self.world
            .entity_from_id(subject)
            .target(rel_ent, 0)
            .map(|e| *e.id())
    }

    /// Retrieve all subject entities that point to `target` with `relation`.
    pub fn children(&self, target: u64, relation: &str) -> Vec<u64> {
        if !self.exists(target) {
            return Vec::new();
        }
        let rel_ent = self.world.entity_named(relation);
        let tgt_ent = self.world.entity_from_id(target);
        let mut results = Vec::new();
        let query = self.world.query::<()>().with((rel_ent, tgt_ent)).build();
        query.each_entity(|e, _| {
            results.push(*e.id());
        });
        results
    }

    /// Query all entities at coordinate `(x, y)` matching an optional layer.
    pub fn at(&self, x: i32, y: i32, layer: Option<u8>) -> Vec<u64> {
        self.spatial.query_at(x, y, layer)
    }

    /// Execute a Flecs C query DSL string (e.g. "GridPos, Monster, !Dead").
    pub fn query_expr(&self, expr: &str) -> Result<Vec<u64>, String> {
        let mut results = Vec::new();
        let query = self.world.query::<()>().expr(expr).build();
        query.each_entity(|e, _| {
            results.push(*e.id());
        });
        Ok(results)
    }

    /// Check whether coordinate `(x, y)` has any opaque entity.
    #[inline]
    pub fn is_opaque(&self, x: i32, y: i32) -> bool {
        self.spatial.is_opaque(x, y)
    }

    /// Check whether coordinate `(x, y)` has any solid entity.
    #[inline]
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        self.spatial.is_solid(x, y)
    }

    /// Check whether coordinate `(x, y)` is passable (no solid entities).
    #[inline]
    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.spatial.is_passable(x, y)
    }

    /// Compute field of view from `(cx, cy)` within `radius` using Symmetric Recursive Shadowcasting.
    pub fn fov(&self, cx: i32, cy: i32, radius: i32) -> FxHashSet<(i32, i32)> {
        compute_fov(cx, cy, radius, |x, y| self.is_opaque(x, y))
    }

    /// Find shortest path from `start` to `goal` using A* grid pathfinding.
    pub fn find_path(
        &self,
        start: (i32, i32),
        goal: (i32, i32),
        allow_diagonal: bool,
    ) -> Option<Vec<(i32, i32)>> {
        pathfinding::find_path(start, goal, allow_diagonal, |x, y| {
            (x == start.0 && y == start.1) || self.is_passable(x, y)
        })
    }

    /// Generate Dijkstra flow field / distance field radiating from `target` up to `max_distance`.
    pub fn dijkstra_map(
        &self,
        target: (i32, i32),
        max_distance: u32,
        allow_diagonal: bool,
    ) -> FxHashMap<(i32, i32), u32> {
        pathfinding::dijkstra_map(target, max_distance, allow_diagonal, |x, y| {
            (x == target.0 && y == target.1) || self.is_passable(x, y)
        })
    }

    /// Sample 2D simplex noise at `(x, y)` with frequency scaling `scale`.
    pub fn noise_2d(seed: u64, x: f64, y: f64, scale: f64) -> f64 {
        let noise = SimplexNoise::new(seed);
        let s = if scale == 0.0 { 1.0 } else { scale };
        noise.sample_2d(x * s, y * s)
    }

    /// Generate a 2D cellular automata cave grid with standard 4-5 rule.
    pub fn generate_cave(
        width: usize,
        height: usize,
        wall_prob: f64,
        iterations: usize,
        seed: u64,
    ) -> Vec<Vec<bool>> {
        noise::generate_cave(width, height, wall_prob, iterations, seed)
    }

    /// Progress world simulation by `delta_time` seconds.
    pub fn progress(&mut self, delta_time: f32) -> bool {
        self.world.progress_time(delta_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecs_world_spawn_despawn_pos() {
        let mut ecs = EcsWorld::new();
        let e1 = ecs.spawn();
        assert!(ecs.exists(e1));

        assert!(ecs.set_pos(e1, 10, 20, LAYER_CREATURE));
        let pos = ecs.get_pos(e1).unwrap();
        assert_eq!(pos.x, 10);
        assert_eq!(pos.y, 20);
        assert_eq!(pos.layer, LAYER_CREATURE);

        assert_eq!(ecs.at(10, 20, None), vec![e1]);
        let top = ecs.spatial.top_at(10, 20).unwrap();
        assert_eq!(top.eid, e1);
        assert_eq!(top.layer, LAYER_CREATURE);

        // Move entity
        assert!(ecs.set_pos(e1, 15, 25, LAYER_CREATURE));
        assert!(ecs.at(10, 20, None).is_empty());
        assert_eq!(ecs.at(15, 25, None), vec![e1]);

        // Despawn
        assert!(ecs.despawn(e1));
        assert!(!ecs.exists(e1));
        assert!(ecs.at(15, 25, None).is_empty());
    }

    #[test]
    fn test_ecs_world_props_and_tags() {
        let mut ecs = EcsWorld::new();
        assert!(ecs.is_empty());
        let e1 = ecs.spawn_named("Goblin");

        assert!(!ecs.has_prop(e1, "hp"));
        ecs.set_prop(e1, "hp", serde_json::json!(50));
        assert!(ecs.has_prop(e1, "hp"));
        assert_eq!(ecs.get_prop(e1, "hp"), Some(serde_json::json!(50)));

        assert!(!ecs.has_tag(e1, "Monster"));
        ecs.add_tag(e1, "Monster");
        assert!(ecs.has_tag(e1, "Monster"));
        ecs.remove_tag(e1, "Monster");
        assert!(!ecs.has_tag(e1, "Monster"));
    }

    #[test]
    fn test_ecs_world_relations() {
        let mut ecs = EcsWorld::new();
        let player = ecs.spawn_named("Hero");
        let hand = ecs.spawn_named("RightHand");
        let axe = ecs.spawn_named("IronAxe");

        // hand PartOf player
        assert!(ecs.relate(hand, "PartOf", player));
        assert_eq!(ecs.target(hand, "PartOf"), Some(player));

        // axe EquippedOn hand
        assert!(ecs.relate(axe, "EquippedOn", hand));
        assert_eq!(ecs.target(axe, "EquippedOn"), Some(hand));

        // Children of hand with EquippedOn should be axe
        let equipped = ecs.children(hand, "EquippedOn");
        assert_eq!(equipped, vec![axe]);

        // Unrelate
        assert!(ecs.unrelate(axe, "EquippedOn", hand));
        assert_eq!(ecs.target(axe, "EquippedOn"), None);
        assert!(ecs.children(hand, "EquippedOn").is_empty());
    }

    #[test]
    fn test_ecs_world_query_expr() {
        let mut ecs = EcsWorld::new();
        let m1 = ecs.spawn();
        let m2 = ecs.spawn();
        let t1 = ecs.spawn();

        ecs.set_pos(m1, 1, 1, LAYER_CREATURE);
        ecs.add_tag(m1, "Monster");

        ecs.set_pos(m2, 2, 2, LAYER_CREATURE);
        ecs.add_tag(m2, "Monster");

        ecs.set_pos(t1, 3, 3, LAYER_TERRAIN);

        let monsters = ecs.query_expr("GridPos, Monster").unwrap();
        assert_eq!(monsters.len(), 2);
        assert!(monsters.contains(&m1));
        assert!(monsters.contains(&m2));
    }

    #[test]
    fn test_ecs_world_math_operators() {
        let mut ecs = EcsWorld::new();

        // Spawn a player at (5, 5)
        let player = ecs.spawn_named("Hero");
        ecs.set_pos(player, 5, 5, LAYER_CREATURE);
        ecs.set_glyph(player, '@', None, None, true, false);

        // Spawn a solid, opaque wall at (5, 4)
        let wall = ecs.spawn_named("Wall");
        ecs.set_pos(wall, 5, 4, LAYER_TERRAIN);
        ecs.set_glyph(wall, '#', None, None, true, true);

        // Test passability and opacity
        assert!(ecs.is_solid(5, 4));
        assert!(ecs.is_opaque(5, 4));
        assert!(!ecs.is_passable(5, 4));

        assert!(ecs.is_passable(6, 5));
        assert!(!ecs.is_opaque(6, 5));

        // Test FOV: wall at (5, 4) is visible, but (5, 3) directly behind it is blocked
        let fov = ecs.fov(5, 5, 4);
        assert!(fov.contains(&(5, 5)));
        assert!(fov.contains(&(5, 4))); // Wall itself is visible
        assert!(!fov.contains(&(5, 3))); // Occluded by wall
        assert!(fov.contains(&(6, 5))); // Open tile visible

        // Test A* pathfinding around the wall from (5, 5) to (5, 3)
        let path = ecs.find_path((5, 5), (5, 3), false).unwrap();
        assert_eq!(*path.first().unwrap(), (5, 5));
        assert_eq!(*path.last().unwrap(), (5, 3));
        assert!(!path.contains(&(5, 4)), "Path should not pass through wall");

        // Test Dijkstra map
        let dmap = ecs.dijkstra_map((5, 5), 3, false);
        assert_eq!(dmap.get(&(5, 5)), Some(&0));
        assert_eq!(dmap.get(&(5, 4)), None); // Solid wall excluded
        assert_eq!(dmap.get(&(6, 5)), Some(&1));

        // Test noise
        let n1 = EcsWorld::noise_2d(42, 10.0, 20.0, 0.1);
        let n2 = EcsWorld::noise_2d(42, 10.0, 20.0, 0.1);
        assert_eq!(n1, n2);
        assert!((-1.0..=1.0).contains(&n1));

        // Test cave generator
        let cave = EcsWorld::generate_cave(20, 15, 0.45, 4, 1234);
        assert_eq!(cave.len(), 15);
        assert_eq!(cave[0].len(), 20);
        assert!(cave[0][0]); // Perimeter wall
    }
}
