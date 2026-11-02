use crate::ecs::components::VisualGlyph;
use rustc_hash::FxHashMap;

/// An entry in the spatial grid representing an entity, its layer, and cached glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpatialEntry {
    pub eid: u64,
    pub layer: u8,
    pub glyph: Option<VisualGlyph>,
}

impl SpatialEntry {
    #[inline]
    pub const fn new(eid: u64, layer: u8, glyph: Option<VisualGlyph>) -> Self {
        Self { eid, layer, glyph }
    }
}

/// Fast spatial hash grid for O(1) coordinate overlap, collision, and layered visual queries.
/// Uses `FxHashMap` for sub-nanosecond integer coordinate hashing.
#[derive(Debug, Clone, Default)]
pub struct SpatialGrid {
    cells: FxHashMap<(i32, i32), Vec<SpatialEntry>>,
}

impl SpatialGrid {
    pub fn new() -> Self {
        Self {
            cells: FxHashMap::default(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cells: FxHashMap::with_capacity_and_hasher(capacity, Default::default()),
        }
    }

    /// Check whether the spatial grid contains no entity cells.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Return the count of occupied grid coordinates.
    #[inline]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Insert or update an entity at `(x, y)` on the given `layer` with optional `glyph`.
    /// The bucket maintains ascending layer order (layer 0: Terrain, 1: Item, 2: Creature, 3: Overlay).
    pub fn insert(&mut self, eid: u64, x: i32, y: i32, layer: u8, glyph: Option<VisualGlyph>) {
        let bucket = self.cells.entry((x, y)).or_default();
        if let Some(pos) = bucket.iter().position(|e| e.eid == eid) {
            bucket.remove(pos);
        }
        let insert_idx = bucket.partition_point(|e| e.layer <= layer);
        bucket.insert(insert_idx, SpatialEntry::new(eid, layer, glyph));
    }

    /// Update the cached glyph of an entity at `(x, y)`.
    pub fn update_glyph(&mut self, eid: u64, x: i32, y: i32, glyph: VisualGlyph) -> bool {
        if let Some(bucket) = self.cells.get_mut(&(x, y)) {
            if let Some(entry) = bucket.iter_mut().find(|e| e.eid == eid) {
                entry.glyph = Some(glyph);
                return true;
            }
        }
        false
    }

    /// Remove an entity from coordinate `(x, y)` with a known layer.
    #[inline]
    pub fn remove(&mut self, eid: u64, x: i32, y: i32, _layer: u8) -> bool {
        self.remove_entity(eid, x, y)
    }

    /// Remove an entity from coordinate `(x, y)` regardless of layer.
    pub fn remove_entity(&mut self, eid: u64, x: i32, y: i32) -> bool {
        if let Some(bucket) = self.cells.get_mut(&(x, y)) {
            if let Some(pos) = bucket.iter().position(|e| e.eid == eid) {
                bucket.remove(pos);
                if bucket.is_empty() {
                    self.cells.remove(&(x, y));
                }
                return true;
            }
        }
        false
    }

    /// Move an entity from `(old_x, old_y)` to `(new_x, new_y)` at `layer`.
    #[allow(clippy::too_many_arguments)]
    pub fn move_entity(
        &mut self,
        eid: u64,
        old_x: i32,
        old_y: i32,
        new_x: i32,
        new_y: i32,
        layer: u8,
        glyph: Option<VisualGlyph>,
    ) {
        if old_x != new_x || old_y != new_y {
            self.remove_entity(eid, old_x, old_y);
            self.insert(eid, new_x, new_y, layer, glyph);
        } else {
            self.insert(eid, new_x, new_y, layer, glyph);
        }
    }

    /// Get all entity entries at `(x, y)` sorted from lowest layer to highest layer.
    #[inline]
    pub fn at(&self, x: i32, y: i32) -> &[SpatialEntry] {
        match self.cells.get(&(x, y)) {
            Some(bucket) => bucket.as_slice(),
            None => &[],
        }
    }

    /// Get all entity IDs at `(x, y)` matching an optional layer filter.
    pub fn query_at(&self, x: i32, y: i32, layer: Option<u8>) -> Vec<u64> {
        let bucket = self.at(x, y);
        match layer {
            Some(target_l) => bucket
                .iter()
                .filter(|e| e.layer == target_l)
                .map(|e| e.eid)
                .collect(),
            None => bucket.iter().map(|e| e.eid).collect(),
        }
    }

    /// Get the entity with the given layer at `(x, y)`, if any.
    #[inline]
    pub fn at_layer(&self, x: i32, y: i32, layer: u8) -> Option<u64> {
        self.at(x, y)
            .iter()
            .find(|e| e.layer == layer)
            .map(|e| e.eid)
    }

    /// Get the topmost entity entry (highest layer) at `(x, y)`.
    #[inline]
    pub fn top_at(&self, x: i32, y: i32) -> Option<&SpatialEntry> {
        self.at(x, y).last()
    }

    /// Get the topmost visual glyph at `(x, y)`.
    #[inline]
    pub fn top_glyph(&self, x: i32, y: i32) -> Option<VisualGlyph> {
        let bucket = self.at(x, y);
        for entry in bucket.iter().rev() {
            if let Some(glyph) = entry.glyph {
                return Some(glyph);
            }
        }
        None
    }

    /// Query all entities within a rectangular bounding box `[min_x..=max_x, min_y..=max_y]`.
    pub fn query_rect(
        &self,
        min_x: i32,
        min_y: i32,
        max_x: i32,
        max_y: i32,
    ) -> Vec<(i32, i32, u64, u8)> {
        let mut results = Vec::new();
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                for entry in self.at(x, y) {
                    results.push((x, y, entry.eid, entry.layer));
                }
            }
        }
        results
    }

    /// Total number of occupied coordinates.
    #[inline]
    pub fn occupied_cells(&self) -> usize {
        self.cells.len()
    }

    /// Total number of entity entries across all coordinates.
    pub fn total_entities(&self) -> usize {
        self.cells.values().map(|v| v.len()).sum()
    }

    /// Check whether coordinate `(x, y)` has any opaque entity.
    #[inline]
    pub fn is_opaque(&self, x: i32, y: i32) -> bool {
        self.at(x, y)
            .iter()
            .any(|e| e.glyph.map_or(false, |g| g.opaque))
    }

    /// Check whether coordinate `(x, y)` has any solid entity.
    #[inline]
    pub fn is_solid(&self, x: i32, y: i32) -> bool {
        self.at(x, y)
            .iter()
            .any(|e| e.glyph.map_or(false, |g| g.solid))
    }

    /// Check whether coordinate `(x, y)` is passable (no solid entities).
    #[inline]
    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        !self.is_solid(x, y)
    }

    /// Clear all cells.
    pub fn clear(&mut self) {
        self.cells.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_grid_layer_ordering() {
        let mut grid = SpatialGrid::new();

        grid.insert(100, 5, 5, 2, None); // creature
        grid.insert(101, 5, 5, 0, None); // terrain
        grid.insert(102, 5, 5, 3, None); // overlay
        grid.insert(103, 5, 5, 1, None); // item

        let entities = grid.at(5, 5);
        assert_eq!(entities.len(), 4);
        assert_eq!(entities[0].eid, 101);
        assert_eq!(entities[1].eid, 103);
        assert_eq!(entities[2].eid, 100);
        assert_eq!(entities[3].eid, 102);

        assert_eq!(grid.top_at(5, 5).unwrap().eid, 102);
        assert_eq!(grid.at_layer(5, 5, 0), Some(101));
        assert_eq!(grid.at_layer(5, 5, 2), Some(100));
        assert_eq!(grid.at_layer(5, 5, 4), None);
    }

    #[test]
    fn test_spatial_grid_move_and_remove() {
        let mut grid = SpatialGrid::new();
        grid.insert(42, 10, 10, 2, None);
        assert_eq!(grid.top_at(10, 10).unwrap().eid, 42);

        grid.move_entity(42, 10, 10, 11, 10, 2, None);
        assert!(grid.at(10, 10).is_empty());
        assert_eq!(grid.top_at(11, 10).unwrap().eid, 42);

        assert!(grid.remove_entity(42, 11, 10));
        assert!(grid.at(11, 10).is_empty());
        assert_eq!(grid.occupied_cells(), 0);
    }
}
