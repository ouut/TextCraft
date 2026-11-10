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

    /// Remove a dynamic property from an entity.
    pub fn delete_prop(&mut self, eid: u64, key: &str) -> bool {
        if !self.exists(eid) {
            return false;
        }
        let ent = self.world.entity_from_id(eid);
        if let Some(mut props) = ent.cloned::<Option<&DynamicProps>>() {
            let removed = props.remove(key).is_some();
            ent.set(props);
            removed
        } else {
            false
        }
    }

    /// Clear all entities and reset spatial grid.
    pub fn clear(&mut self) {
        let mut new_world = Self::new();
        std::mem::swap(self, &mut new_world);
    }

    /// Check whether an entity has a specific dynamic property.
    pub fn has_prop(&self, eid: u64, key: &str) -> bool {
        if !self.exists(eid) {
            return false;
        }
        self.world
            .entity_from_id(eid)
            .cloned::<Option<&DynamicProps>>()
            .is_some_and(|p| p.has(key))
    }

    /// Check whether the spatial index has no entities.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.spatial.is_empty()
    }

    /// Return the count of entities indexed in the spatial grid across all layers.
    #[inline]
    pub fn count(&self) -> usize {
        self.spatial.total_entries()
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
        let mut builder = self.world.query::<()>();
        builder.expr(expr);
        let Some(query) = builder.try_build() else {
            return Err(format!("Invalid query expression: {}", expr));
        };
        let mut results = Vec::new();
        query.each_entity(|e, _| {
            results.push(*e.id());
        });
        Ok(results)
    }

    /// Detailed introspection of an entity by ID, including coordinates, glyph, tags, props, and relationships.
    pub fn inspect_entity(&self, eid: u64) -> Option<serde_json::Value> {
        if !self.exists(eid) {
            return None;
        }
        let ent = self.world.entity_from_id(eid);
        let name = ent.get_name();
        let pos = self.get_pos(eid);
        let glyph = self.get_glyph(eid);

        let dynamic_props = ent
            .cloned::<Option<&DynamicProps>>()
            .map(|dp| dp.values.clone());

        let player_session = ent
            .cloned::<Option<&PlayerSession>>()
            .map(|ps| serde_json::json!({
                "player_id": ps.player_id,
                "viewport_cols": ps.viewport_cols,
                "viewport_rows": ps.viewport_rows,
            }));

        let mut tags = Vec::new();
        let mut relationships = Vec::new();

        ent.each_component(|id| {
            if id.is_pair() {
                let rel_name = id.get_first_id().map(|e| e.name()).unwrap_or_default();
                let tgt_id = id.get_second_id().map(|e| *e.id()).unwrap_or(0);
                let tgt_name = id.get_second_id().and_then(|e| e.get_name());
                relationships.push(serde_json::json!({
                    "relation": rel_name,
                    "target_id": tgt_id,
                    "target_name": tgt_name,
                }));
            } else if let Some(view) = id.get_entity_view() {
                let tname = view.name();
                if !tname.is_empty() {
                    tags.push(tname);
                }
            }
        });

        Some(serde_json::json!({
            "id": eid,
            "name": name,
            "exists": true,
            "position": pos.map(|p| serde_json::json!({
                "x": p.x,
                "y": p.y,
                "layer": p.layer,
            })),
            "glyph": glyph.map(|g| serde_json::json!({
                "char": g.ch.to_string(),
                "fg": g.fg.map(|c| [c.r, c.g, c.b]),
                "bg": g.bg.map(|c| [c.r, c.g, c.b]),
                "solid": g.solid,
                "opaque": g.opaque,
            })),
            "dynamic_props": dynamic_props,
            "player_session": player_session,
            "tags": tags,
            "relationships": relationships,
        }))
    }

    /// Query matching entities and return their inspected state up to `limit`.
    pub fn query_entities(&self, expr: &str, limit: usize) -> Result<serde_json::Value, String> {
        let eids = self.query_expr(expr)?;
        let total = eids.len();
        let limited_entities: Vec<serde_json::Value> = eids
            .into_iter()
            .take(limit)
            .filter_map(|eid| self.inspect_entity(eid))
            .collect();

        Ok(serde_json::json!({
            "query": expr,
            "total_matches": total,
            "count": limited_entities.len(),
            "entities": limited_entities,
        }))
    }

    /// Modify an entity with a patch JSON object (props, tags, pos, glyph, relations, player session, despawn).
    pub fn modify_entity(
        &mut self,
        eid: u64,
        patch: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        if !self.exists(eid) {
            return Err(format!("Entity {} does not exist", eid));
        }

        // Check for despawn
        if patch.get("despawn").and_then(|v| v.as_bool()).unwrap_or(false) {
            self.despawn(eid);
            return Ok(serde_json::json!({
                "id": eid,
                "despawned": true,
            }));
        }

        // Dynamic props to set
        if let Some(props) = patch.get("set_props").and_then(|v| v.as_object()) {
            for (k, v) in props {
                self.set_prop(eid, k, v.clone());
            }
        }

        // Dynamic props to delete
        if let Some(del_keys) = patch.get("delete_props").and_then(|v| v.as_array()) {
            for k in del_keys {
                if let Some(key_str) = k.as_str() {
                    self.delete_prop(eid, key_str);
                }
            }
        }

        // Tags to add
        if let Some(add_tags) = patch.get("add_tags").and_then(|v| v.as_array()) {
            for tag in add_tags {
                if let Some(tag_str) = tag.as_str() {
                    self.add_tag(eid, tag_str);
                }
            }
        }

        // Tags to remove
        if let Some(rem_tags) = patch.get("remove_tags").and_then(|v| v.as_array()) {
            for tag in rem_tags {
                if let Some(tag_str) = tag.as_str() {
                    self.remove_tag(eid, tag_str);
                }
            }
        }

        // Position update
        if let Some(pos_obj) = patch.get("set_pos").and_then(|v| v.as_object()) {
            if let (Some(x), Some(y)) = (
                pos_obj.get("x").and_then(|v| v.as_i64()),
                pos_obj.get("y").and_then(|v| v.as_i64()),
            ) {
                let layer = pos_obj
                    .get("layer")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u8)
                    .or_else(|| self.get_pos(eid).map(|p| p.layer))
                    .unwrap_or(LAYER_CREATURE);
                self.set_pos(eid, x as i32, y as i32, layer);
            }
        }

        // Glyph update
        if let Some(g_obj) = patch.get("set_glyph").and_then(|v| v.as_object()) {
            let ch = g_obj
                .get("char")
                .and_then(|v| v.as_str())
                .and_then(|s| s.chars().next())
                .unwrap_or('?');
            let fg = g_obj.get("fg").and_then(|v| v.as_array()).and_then(|arr| {
                if arr.len() >= 3 {
                    Some(Color::rgb(
                        arr[0].as_u64().unwrap_or(255) as u8,
                        arr[1].as_u64().unwrap_or(255) as u8,
                        arr[2].as_u64().unwrap_or(255) as u8,
                    ))
                } else {
                    None
                }
            });
            let bg = g_obj.get("bg").and_then(|v| v.as_array()).and_then(|arr| {
                if arr.len() >= 3 {
                    Some(Color::rgb(
                        arr[0].as_u64().unwrap_or(0) as u8,
                        arr[1].as_u64().unwrap_or(0) as u8,
                        arr[2].as_u64().unwrap_or(0) as u8,
                    ))
                } else {
                    None
                }
            });
            let solid = g_obj.get("solid").and_then(|v| v.as_bool()).unwrap_or(false);
            let opaque = g_obj.get("opaque").and_then(|v| v.as_bool()).unwrap_or(false);
            self.set_glyph(eid, ch, fg, bg, solid, opaque);
        }

        // Player session update
        if let Some(ps_obj) = patch.get("set_player_session").and_then(|v| v.as_object()) {
            if let Some(pid) = ps_obj.get("player_id").and_then(|v| v.as_u64()) {
                let cols = ps_obj
                    .get("viewport_cols")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(80) as u16;
                let rows = ps_obj
                    .get("viewport_rows")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(24) as u16;
                self.world
                    .entity_from_id(eid)
                    .set(PlayerSession::new(pid, cols, rows));
                self.add_tag(eid, "Player");
            }
        }

        // Relate
        if let Some(rel_arr) = patch.get("relate").and_then(|v| v.as_array()) {
            for item in rel_arr {
                if let (Some(rel), Some(tgt)) = (
                    item.get("relation").and_then(|v| v.as_str()),
                    item.get("target").and_then(|v| v.as_u64()),
                ) {
                    self.relate(eid, rel, tgt);
                }
            }
        }

        // Unrelate
        if let Some(unrel_arr) = patch.get("unrelate").and_then(|v| v.as_array()) {
            for item in unrel_arr {
                if let (Some(rel), Some(tgt)) = (
                    item.get("relation").and_then(|v| v.as_str()),
                    item.get("target").and_then(|v| v.as_u64()),
                ) {
                    self.unrelate(eid, rel, tgt);
                }
            }
        }

        self.inspect_entity(eid)
            .ok_or_else(|| format!("Entity {} disappeared after modification", eid))
    }

    // ------------------------------------------------------------------
    // Snapshot v2: full-world export / import (guide item 2)
    // ------------------------------------------------------------------

    /// Tag/component names that appear in `inspect_entity`'s `tags` list but
    /// are structural (the four core components and Flecs internals), not
    /// user tags. They must not be re-applied as tags on import.
    fn is_structural_tag(name: &str) -> bool {
        matches!(
            name,
            "GridPos"
                | "VisualGlyph"
                | "DynamicProps"
                | "PlayerSession"
                | "Component"
                | "Identifier"
                | "Name"
                | "Symbol"
                | "EcsComponent"
                | "EcsIdentifier"
                | "flecs"
                | "ChildOf"
                | "IsA"
                | "DependsOn"
        ) || name.contains('.')
            || name.starts_with("flecs")
            || name.starts_with("Ecs")
    }

    /// Component names whose *values* `inspect_entity` serializes.
    const KNOWN_VALUE_COMPONENTS: [&str; 4] =
        ["GridPos", "VisualGlyph", "DynamicProps", "PlayerSession"];
    /// Registered marker tags: their *presence* is serialized in `tags`.
    const KNOWN_TAGS: [&str; 7] =
        ["Monster", "Player", "Item", "Terrain", "Dead", "Liquid", "Burning"];
    /// Registered relations: name + target are serialized in `relationships`.
    const KNOWN_RELATIONS: [&str; 3] = ["EquippedOn", "PartOf", "CoveredWith"];

    /// True when an entity view lives in the Flecs (or flecs-rust external
    /// component) namespace tree, e.g. `::flecs::core::Component` or
    /// `::external_components::vector::String`. Such entities are Flecs
    /// infrastructure, never game state. Unnamed entities return no path
    /// and are handled by the meaningful-content test instead: every
    /// component/relation they carry belongs to a flecs-owned type, so
    /// they never qualify (see `entity_has_game_content`).
    fn is_infra_path(path: Option<&str>) -> bool {
        match path {
            Some(p) => {
                p == "::flecs"
                    || p.starts_with("::flecs::")
                    || p == "::external_components"
                    || p.starts_with("::external_components::")
            }
            None => false,
        }
    }

    /// True when the entity is a component / tag / relation *definition*:
    /// it carries the `flecs::Component` marker (everything registered via
    /// `world.component::<T>()` does, including the core types registered
    /// in `EcsWorld::new`), or some other entity uses its id as a
    /// component/tag id or as the first (relation) id of a pair — which is
    /// how the plain named entities created on demand by
    /// `add_tag`/`relate` for dynamic tags/relations show up.
    fn is_definition_entity(
        ent: EntityView<'_>,
        type_ids: &std::collections::HashSet<u64>,
    ) -> bool {
        ent.has(flecs_ecs::prelude::flecs::Component::id())
            || type_ids.contains(&*ent.id())
    }

    /// True when an entity carries any *game* content: a name, or at least
    /// one component / tag / pair whose type is not Flecs-owned and not
    /// structural. Structural pairs such as `(Identifier, Name)` and
    /// `(ChildOf, flecs-parent)` do not count on their own — the name they
    /// encode is already covered by the name test.
    fn entity_has_game_content(ent: EntityView<'_>) -> bool {
        // Flecs wildcard entities ("*" / "_") sit under `::flecs::core`
        // but report a bare path, so a bare name is not enough: an entity
        // parented (ChildOf) under a flecs-namespace entity is infra,
        // however it is named.
        let mut child_of_infra = false;
        ent.each_component(|cid| {
            if cid.is_pair() {
                let rel_name = cid.get_first_id().map(|e| e.name()).unwrap_or_default();
                if rel_name == "ChildOf" {
                    if let Some(parent) = cid.get_second_id() {
                        if Self::is_infra_path(parent.path().as_deref()) {
                            child_of_infra = true;
                        }
                    }
                }
            }
        });
        if !child_of_infra
            && ent
                .get_name()
                .is_some_and(|n| !n.is_empty() && n != "*" && n != "_")
        {
            return true;
        }
        let mut qualifies = false;
        ent.each_component(|cid| {
            if qualifies {
                return;
            }
            // Note: `is_structural_tag` must NOT be used here — it lists
            // the four core components, which are game content. Flecs
            // structural types (Identifier, ChildOf, DependsOn, ...) are
            // all flecs-owned, so the path test already excludes them.
            if cid.is_pair() {
                if let Some(rel) = cid.get_first_id() {
                    let rname = rel.name();
                    let rpath = rel.path();
                    if !rname.is_empty() && !Self::is_infra_path(rpath.as_deref()) {
                        qualifies = true;
                    }
                }
            } else if let Some(view) = cid.get_entity_view() {
                let cname = view.name();
                let cpath = view.path();
                if !cname.is_empty() && !Self::is_infra_path(cpath.as_deref()) {
                    qualifies = true;
                }
            }
        });
        qualifies
    }

    /// Names of registered components/relations on `ent` whose values the
    /// export does not serialize: anything carrying the
    /// `flecs::Component` definition marker that is not one of the four
    /// value components, the known marker tags, or the known relations.
    /// Plain named entities used as dynamic tags/relations (created by
    /// `add_tag`/`relate`) have no such marker and are *not* unknown —
    /// their presence round-trips through `tags` / `relationships`.
    fn unknown_components_of(ent: EntityView<'_>) -> Vec<String> {
        let mut unknown: Vec<String> = Vec::new();
        ent.each_component(|cid| {
            if cid.is_pair() {
                if let Some(rel) = cid.get_first_id() {
                    let rname = rel.name();
                    let rpath = rel.path();
                    if !rname.is_empty()
                        && !Self::KNOWN_RELATIONS.contains(&rname.as_str())
                        && !Self::is_structural_tag(&rname)
                        && !Self::is_infra_path(rpath.as_deref())
                        && rel.has(flecs_ecs::prelude::flecs::Component::id())
                        && !unknown.contains(&rname)
                    {
                        unknown.push(rname);
                    }
                }
            } else if let Some(view) = cid.get_entity_view() {
                let cname = view.name();
                let cpath = view.path();
                if !cname.is_empty()
                    && !Self::KNOWN_VALUE_COMPONENTS.contains(&cname.as_str())
                    && !Self::KNOWN_TAGS.contains(&cname.as_str())
                    && !Self::is_structural_tag(&cname)
                    && !Self::is_infra_path(cpath.as_deref())
                    && view.has(flecs_ecs::prelude::flecs::Component::id())
                    && !unknown.contains(&cname)
                {
                    unknown.push(cname);
                }
            }
        });
        unknown.sort();
        unknown
    }

    /// Export every *game* entity as JSON, reusing `inspect_entity`'s shape.
    ///
    /// An entity counts as a game entity iff it carries any game content —
    /// a component, a tag, a relationship, or a name (see
    /// `entity_has_game_content`) — and is not Flecs infrastructure:
    /// component/tag/relation *definition* entities (see
    /// `is_definition_entity`) and anything under the `flecs` /
    /// `external_components` namespaces are excluded. Definitions are
    /// recreated by `EcsWorld::new` and by `add_tag`/`relate` on demand.
    /// Entities whose registered components the export cannot serialize
    /// get an `unknown_components` field listing their names instead of
    /// being silently dropped or aborting the export. Entities are sorted
    /// by id so the export is deterministic for a given world.
    pub fn export_world_json(&self) -> serde_json::Value {
        let mut ids: Vec<u64> = Vec::new();
        self.world.each_alive_entity(|e| {
            ids.push(*e.id());
        });
        ids.sort_unstable();

        // Every id used as a type somewhere in the world: a component/tag
        // id, or the first (relation) id of a pair. Entities in this set
        // are definitions, however they were created.
        let mut type_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();
        for &id in &ids {
            self.world.entity_from_id(id).each_component(|cid| {
                if cid.is_pair() {
                    if let Some(first) = cid.get_first_id() {
                        type_ids.insert(*first.id());
                    }
                } else if let Some(view) = cid.get_entity_view() {
                    type_ids.insert(*view.id());
                }
            });
        }

        let mut entities = Vec::new();
        for id in ids {
            let ent = self.world.entity_from_id(id);
            if Self::is_infra_path(ent.path().as_deref())
                || Self::is_definition_entity(ent, &type_ids)
                || !Self::entity_has_game_content(ent)
            {
                continue;
            }
            if let Some(mut v) = self.inspect_entity(id) {
                // Drop structural names from tags so export is import-ready.
                if let Some(tags) = v.get_mut("tags").and_then(|t| t.as_array_mut()) {
                    tags.retain(|t| {
                        t.as_str().is_some_and(|s| !Self::is_structural_tag(s))
                    });
                }
                let unknown = Self::unknown_components_of(ent);
                if !unknown.is_empty() {
                    v["unknown_components"] = serde_json::json!(unknown);
                }
                entities.push(v);
            }
        }
        serde_json::json!({ "entities": entities })
    }

    /// Import a world previously produced by `export_world_json`.
    ///
    /// Semantics (guide 4.2): the current world is cleared; entities are
    /// recreated trying to preserve their original ids via Flecs
    /// `make_alive`. If an id cannot be preserved it is remapped and the
    /// returned `id_map` (old -> new) records every entity's mapping.
    /// SpatialGrid is rebuilt as a side effect of `set_pos`/`set_glyph`.
    /// All parsing/validation happens before the world is touched, so a
    /// malformed snapshot leaves the current world intact.
    pub fn import_world_json(
        &mut self,
        data: &serde_json::Value,
    ) -> Result<std::collections::HashMap<u64, u64>, String> {
        let arr = data
            .get("entities")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "ecs_world.entities must be an array".to_string())?;

        // ---- Validate & parse into plain structs first -----------------
        struct ParsedEnt {
            old_id: u64,
            name: Option<String>,
            pos: Option<GridPos>,
            glyph: Option<VisualGlyph>,
            props: Option<std::collections::HashMap<String, serde_json::Value>>,
            session: Option<PlayerSession>,
            tags: Vec<String>,
            relations: Vec<(String, u64)>,
        }
        let mut parsed: Vec<ParsedEnt> = Vec::with_capacity(arr.len());
        for item in arr {
            let old_id = item
                .get("id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "entity entry missing numeric 'id'".to_string())?;
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            // Note: values that round-tripped through the Scheme VM may
            // turn JSON null into `false`; treat any non-object as absent.
            let pos = match item.get("position") {
                Some(p) if p.is_object() => Some(GridPos::new(
                    p.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    p.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
                    p.get("layer").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
                )),
                _ => None,
            };
            let glyph = match item.get("glyph") {
                Some(g) if g.is_object() => {
                    let ch = g
                        .get("char")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.chars().next())
                        .unwrap_or(' ');
                    let parse_col = |v: &serde_json::Value| -> Option<Color> {
                        v.as_array().and_then(|a| {
                            if a.len() >= 3 {
                                Some(Color::rgb(
                                    a[0].as_u64().unwrap_or(0) as u8,
                                    a[1].as_u64().unwrap_or(0) as u8,
                                    a[2].as_u64().unwrap_or(0) as u8,
                                ))
                            } else {
                                None
                            }
                        })
                    };
                    Some(VisualGlyph::new(
                        ch,
                        g.get("fg").and_then(parse_col),
                        g.get("bg").and_then(parse_col),
                        g.get("solid").and_then(|v| v.as_bool()).unwrap_or(false),
                        g.get("opaque").and_then(|v| v.as_bool()).unwrap_or(false),
                    ))
                }
                _ => None,
            };
            let props = match item.get("dynamic_props") {
                Some(serde_json::Value::Object(map)) => {
                    Some(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                }
                _ => None,
            };
            let session = match item.get("player_session") {
                Some(s) if s.is_object() => Some(PlayerSession::new(
                    s.get("player_id").and_then(|v| v.as_u64()).unwrap_or(0),
                    s.get("viewport_cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16,
                    s.get("viewport_rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16,
                )),
                _ => None,
            };
            let tags = item
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|t| t.as_str().map(|s| s.to_string()))
                        .filter(|s| !Self::is_structural_tag(s))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            let relations = item
                .get("relationships")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|r| {
                            let rel = r.get("relation")?.as_str()?.to_string();
                            let tgt = r.get("target_id")?.as_u64()?;
                            Some((rel, tgt))
                        })
                        .collect::<Vec<(String, u64)>>()
                })
                .unwrap_or_default();
            parsed.push(ParsedEnt {
                old_id,
                name,
                pos,
                glyph,
                props,
                session,
                tags,
                relations,
            });
        }

        // ---- Switch worlds ----------------------------------------------
        self.clear();

        // Pass 1: reserve ids (make_alive) before any dynamic tag/relation
        // entity is created, so new definition entities cannot collide.
        let mut id_map: std::collections::HashMap<u64, u64> = std::collections::HashMap::new();
        for p in &parsed {
            let view = self.world.make_alive(p.old_id);
            let new_id = *view.id();
            if new_id == p.old_id && self.exists(p.old_id) {
                id_map.insert(p.old_id, p.old_id);
            } else {
                let spawned = self.spawn();
                id_map.insert(p.old_id, spawned);
            }
        }

        // Pass 2: components, names, tags.
        for p in &parsed {
            let new_id = id_map[&p.old_id];
            if let Some(name) = &p.name {
                if !name.is_empty() {
                    self.world.entity_from_id(new_id).set_name(name);
                }
            }
            if let Some(pos) = p.pos {
                self.set_pos(new_id, pos.x, pos.y, pos.layer);
            }
            if let Some(g) = p.glyph {
                self.set_glyph(new_id, g.ch, g.fg, g.bg, g.solid, g.opaque);
            }
            if let Some(props) = &p.props {
                if props.is_empty() {
                    self.world
                        .entity_from_id(new_id)
                        .set(DynamicProps::default());
                } else {
                    for (k, v) in props {
                        self.set_prop(new_id, k, v.clone());
                    }
                }
            }
            if let Some(sess) = &p.session {
                self.world.entity_from_id(new_id).set(sess.clone());
            }
            for tag in &p.tags {
                self.add_tag(new_id, tag);
            }
        }

        // Pass 3: relationships (targets remapped through id_map).
        for p in &parsed {
            let new_id = id_map[&p.old_id];
            for (rel, tgt) in &p.relations {
                if let Some(&new_tgt) = id_map.get(tgt) {
                    self.relate(new_id, rel, new_tgt);
                }
                // Targets outside the snapshot (definition entities) are
                // not causal game state; see state_inventory.md.
            }
        }

        Ok(id_map)
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

    #[test]
    fn test_ecs_inspect_query_modify() {
        let mut ecs = EcsWorld::new();

        let goblin = ecs.spawn_named("Goblin_Grunt");
        ecs.set_pos(goblin, 12, 8, LAYER_CREATURE);
        ecs.set_glyph(goblin, 'g', Some(Color::rgb(100, 200, 100)), None, true, false);
        ecs.add_tag(goblin, "Monster");
        ecs.set_prop(goblin, "hp", serde_json::json!(45));

        // 1. Inspect
        let ins = ecs.inspect_entity(goblin).expect("Goblin should exist");
        assert_eq!(ins["name"], "Goblin_Grunt");
        assert_eq!(ins["position"]["x"], 12);
        assert_eq!(ins["position"]["y"], 8);
        assert_eq!(ins["glyph"]["char"], "g");
        assert_eq!(ins["dynamic_props"]["hp"], 45);
        assert!(ins["tags"].as_array().unwrap().iter().any(|t| t == "Monster"));

        // 2. Query
        let q_res = ecs.query_entities("Monster", 10).unwrap();
        assert_eq!(q_res["total_matches"], 1);
        assert_eq!(q_res["entities"][0]["name"], "Goblin_Grunt");

        // 3. Modify
        let patch = serde_json::json!({
            "set_props": { "hp": 30, "status": "poisoned" },
            "add_tags": ["Poisoned"],
            "set_pos": { "x": 13, "y": 8 }
        });
        let mod_res = ecs.modify_entity(goblin, &patch).unwrap();
        assert_eq!(mod_res["position"]["x"], 13);
        assert_eq!(mod_res["dynamic_props"]["hp"], 30);
        assert_eq!(mod_res["dynamic_props"]["status"], "poisoned");
        assert!(mod_res["tags"].as_array().unwrap().iter().any(|t| t == "Poisoned"));

        // 4. Clear
        ecs.clear();
        assert_eq!(ecs.count(), 0);
        assert!(ecs.inspect_entity(goblin).is_none());
    }

    #[test]
    fn test_world_export_lists_unknown_components() {
        // A registered component type the export logic does not know how
        // to serialize (only GridPos / VisualGlyph / DynamicProps /
        // PlayerSession values are exported).
        #[derive(Debug, Clone, Copy, Default, Component)]
        #[allow(dead_code)] // value is never read back: that is the point
        struct SecretStats {
            mana: i32,
        }

        let mut ecs = EcsWorld::new();
        ecs.world.component::<SecretStats>();

        let bearer = ecs.spawn_named("Bearer");
        ecs.world
            .entity_from_id(bearer)
            .set(SecretStats { mana: 9 });

        let plain = ecs.spawn_named("Plain");
        ecs.set_prop(plain, "hp", serde_json::json!(10));
        ecs.add_tag(plain, "Monster");
        // Dynamic tag: round-trips through `tags`, must NOT be unknown.
        ecs.add_tag(plain, "QuestTagged");

        let exported = ecs.export_world_json();
        let ents = exported["entities"].as_array().unwrap();
        let bearer_v = ents.iter().find(|e| e["id"] == bearer).expect("bearer exported");
        assert_eq!(
            bearer_v["unknown_components"],
            serde_json::json!(["SecretStats"]),
            "unknown component listed, not silently dropped: {}",
            bearer_v
        );
        let plain_v = ents.iter().find(|e| e["id"] == plain).expect("plain exported");
        assert!(
            plain_v.get("unknown_components").is_none(),
            "known components/tags must not be flagged unknown: {}",
            plain_v
        );
        // The SecretStats definition entity itself must not be exported.
        assert!(
            ents.iter().all(|e| e["name"] != "SecretStats"),
            "component definition entity not exported: {}",
            exported
        );
    }

    #[test]
    fn test_world_export_import_roundtrip_preserves_ids_and_relations() {
        let mut ecs = EcsWorld::new();
        let body = ecs.spawn_named("Body");
        ecs.set_pos(body, 3, 4, LAYER_CREATURE);
        ecs.set_glyph(body, '@', None, None, true, false);
        ecs.set_prop(body, "hp", serde_json::json!(80));
        let limb = ecs.spawn_named("Arm");
        ecs.set_prop(limb, "severed", serde_json::json!(false));
        ecs.set_prop(limb, "limb_type", serde_json::json!("arm_left"));
        ecs.relate(limb, "PartOf", body);
        ecs.add_tag(limb, "Limb");

        let exported = ecs.export_world_json();
        let ents = exported["entities"].as_array().unwrap();
        assert_eq!(ents.len(), 2, "only game entities exported: {}", exported);

        let mut restored = EcsWorld::new();
        let filler = restored.spawn(); // occupy an unrelated id slot first
        let _ = filler;
        let id_map = restored.import_world_json(&exported).unwrap();
        assert_eq!(id_map.get(&body), Some(&body), "body id preserved: {:?}", id_map);
        assert_eq!(id_map.get(&limb), Some(&limb), "limb id preserved: {:?}", id_map);

        let re = restored.export_world_json();
        assert_eq!(re, exported, "re-export must equal original export");
        assert_eq!(restored.target(limb, "PartOf"), Some(body));
        assert!(restored.has_tag(limb, "Limb"));
        assert_eq!(restored.get_prop(limb, "severed"), Some(serde_json::json!(false)));
        assert_eq!(restored.at(3, 4, None), vec![body], "spatial rebuilt");
    }
}
