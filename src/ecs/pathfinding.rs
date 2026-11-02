use rustc_hash::FxHashMap;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AStarNode {
    f_cost: u32,
    g_cost: u32,
    pos: (i32, i32),
}

impl Ord for AStarNode {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap; reverse comparison for min-heap behavior
        other
            .f_cost
            .cmp(&self.f_cost)
            .then_with(|| other.g_cost.cmp(&self.g_cost))
    }
}

impl PartialOrd for AStarNode {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compute heuristic distance to goal:
/// Manhattan distance for 4-way, Chebyshev distance for 8-way movement.
#[inline]
fn heuristic(pos: (i32, i32), goal: (i32, i32), allow_diagonal: bool) -> u32 {
    let dx = (goal.0 - pos.0).unsigned_abs();
    let dy = (goal.1 - pos.1).unsigned_abs();
    if allow_diagonal {
        dx.max(dy)
    } else {
        dx + dy
    }
}

/// Directions for 4-way movement (cardinal only).
const CARDINAL_DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Directions for 8-way movement (cardinal + diagonal).
const ALL_8_DIRS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

/// Maximum number of node expansions before abandoning path search (prevents infinite search on unbounded grids).
pub const DEFAULT_MAX_EXPANSIONS: usize = 10_000;

/// A* Grid Pathfinding algorithm with custom expansion budget.
///
/// Returns `Option<Vec<(i32, i32)>>` with shortest path from `start` to `goal` (inclusive).
/// If `start == goal`, returns `Some(vec![start])`.
/// If `goal` is unreachable, blocked, or search budget exceeded, returns `None`.
pub fn find_path_bounded<F>(
    start: (i32, i32),
    goal: (i32, i32),
    allow_diagonal: bool,
    max_expansions: usize,
    is_passable: F,
) -> Option<Vec<(i32, i32)>>
where
    F: Fn(i32, i32) -> bool,
{
    if start == goal {
        return Some(vec![start]);
    }

    // If goal itself is not passable, path cannot be formed
    if !is_passable(goal.0, goal.1) {
        return None;
    }

    let dirs = if allow_diagonal {
        &ALL_8_DIRS[..]
    } else {
        &CARDINAL_DIRS[..]
    };

    let mut open_set = BinaryHeap::new();
    let mut g_score: FxHashMap<(i32, i32), u32> = FxHashMap::default();
    let mut came_from: FxHashMap<(i32, i32), (i32, i32)> = FxHashMap::default();

    g_score.insert(start, 0);
    let h_start = heuristic(start, goal, allow_diagonal);
    open_set.push(AStarNode {
        f_cost: h_start,
        g_cost: 0,
        pos: start,
    });

    let mut expansions = 0;

    while let Some(current) = open_set.pop() {
        if current.pos == goal {
            // Reconstruct path
            let mut path = Vec::with_capacity(current.g_cost as usize + 1);
            let mut curr = goal;
            path.push(curr);
            while curr != start {
                curr = came_from[&curr];
                path.push(curr);
            }
            path.reverse();
            return Some(path);
        }

        // Skip node if a cheaper path to this position was already found
        if let Some(&best_g) = g_score.get(&current.pos) {
            if current.g_cost > best_g {
                continue;
            }
        }

        expansions += 1;
        if expansions > max_expansions {
            return None;
        }

        let (cx, cy) = current.pos;

        for &(dx, dy) in dirs {
            let nx = cx + dx;
            let ny = cy + dy;
            let next_pos = (nx, ny);

            // Prevent diagonal corner cutting through two solid diagonal walls
            if dx != 0 && dy != 0 && !is_passable(cx + dx, cy) && !is_passable(cx, cy + dy) {
                continue;
            }

            if next_pos != goal && !is_passable(nx, ny) {
                continue;
            }

            let tentative_g = current.g_cost + 1;
            let current_best_g = g_score.get(&next_pos).copied().unwrap_or(u32::MAX);

            if tentative_g < current_best_g {
                g_score.insert(next_pos, tentative_g);
                came_from.insert(next_pos, current.pos);
                let f_cost = tentative_g + heuristic(next_pos, goal, allow_diagonal);
                open_set.push(AStarNode {
                    f_cost,
                    g_cost: tentative_g,
                    pos: next_pos,
                });
            }
        }
    }

    None
}

/// A* Grid Pathfinding algorithm using default search budget.
#[inline]
pub fn find_path<F>(
    start: (i32, i32),
    goal: (i32, i32),
    allow_diagonal: bool,
    is_passable: F,
) -> Option<Vec<(i32, i32)>>
where
    F: Fn(i32, i32) -> bool,
{
    find_path_bounded(start, goal, allow_diagonal, DEFAULT_MAX_EXPANSIONS, is_passable)
}

/// Dijkstra Flow Field / Distance Field generator.
///
/// Computes minimum step distances from `target` for all reachable passable tiles
/// within `max_distance`.
/// Target tile itself has distance 0.
pub fn dijkstra_map<F>(
    target: (i32, i32),
    max_distance: u32,
    allow_diagonal: bool,
    is_passable: F,
) -> FxHashMap<(i32, i32), u32>
where
    F: Fn(i32, i32) -> bool,
{
    let mut dist_map: FxHashMap<(i32, i32), u32> = FxHashMap::default();
    let mut queue: VecDeque<((i32, i32), u32)> = VecDeque::new();

    let dirs = if allow_diagonal {
        &ALL_8_DIRS[..]
    } else {
        &CARDINAL_DIRS[..]
    };

    dist_map.insert(target, 0);
    queue.push_back((target, 0));

    while let Some(((cx, cy), dist)) = queue.pop_front() {
        if dist >= max_distance {
            continue;
        }

        for &(dx, dy) in dirs {
            let nx = cx + dx;
            let ny = cy + dy;
            let next_pos = (nx, ny);

            // Prevent corner cutting through two solid diagonal walls
            if dx != 0 && dy != 0 && !is_passable(cx + dx, cy) && !is_passable(cx, cy + dy) {
                continue;
            }

            if !is_passable(nx, ny) {
                continue;
            }

            if let std::collections::hash_map::Entry::Vacant(e) = dist_map.entry(next_pos) {
                let next_dist = dist + 1;
                e.insert(next_dist);
                queue.push_back((next_pos, next_dist));
            }
        }
    }

    dist_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::FxHashSet;

    #[test]
    fn test_astar_straight_line() {
        let path_4way = find_path((0, 0), (0, 4), false, |_x, _y| true).unwrap();
        assert_eq!(path_4way, vec![(0, 0), (0, 1), (0, 2), (0, 3), (0, 4)]);

        let path_8way = find_path((0, 0), (3, 3), true, |_x, _y| true).unwrap();
        assert_eq!(path_8way, vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
    }

    #[test]
    fn test_astar_start_equals_goal() {
        let path = find_path((5, 5), (5, 5), false, |_x, _y| true).unwrap();
        assert_eq!(path, vec![(5, 5)]);
    }

    #[test]
    fn test_astar_around_obstacle() {
        // Wall from (2, 0) to (2, 3) blocking direct horizontal route from (0, 2) to (4, 2)
        let is_passable = |x: i32, y: i32| !(x == 2 && (0..=3).contains(&y));

        let path = find_path((0, 2), (4, 2), false, is_passable).unwrap();
        assert_eq!(*path.first().unwrap(), (0, 2));
        assert_eq!(*path.last().unwrap(), (4, 2));

        // Check none of the path steps touch the obstacle
        for pos in &path {
            assert!(is_passable(pos.0, pos.1), "Path traversed obstacle at {:?}", pos);
        }
    }

    #[test]
    fn test_astar_unreachable_goal() {
        // Enclosed room: 10x10 map bounds
        let walls: FxHashSet<(i32, i32)> = [
            (4, 4), (5, 4), (6, 4),
            (4, 5),         (6, 5),
            (4, 6), (5, 6), (6, 6),
        ].into_iter().collect();

        // Passable only within [0..=10, 0..=10] and not a wall
        let is_passable = |x: i32, y: i32| {
            (0..=10).contains(&x) && (0..=10).contains(&y) && !walls.contains(&(x, y))
        };
        let path = find_path((0, 0), (5, 5), false, is_passable);
        assert!(path.is_none());
    }

    #[test]
    fn test_astar_goal_is_obstacle() {
        let is_passable = |x: i32, y: i32| !(x == 3 && y == 3);
        let path = find_path((0, 0), (3, 3), false, is_passable);
        assert!(path.is_none());
    }

    #[test]
    fn test_dijkstra_map_open_field() {
        let dmap = dijkstra_map((5, 5), 2, false, |_x, _y| true);
        assert_eq!(dmap.get(&(5, 5)), Some(&0));
        assert_eq!(dmap.get(&(6, 5)), Some(&1));
        assert_eq!(dmap.get(&(4, 5)), Some(&1));
        assert_eq!(dmap.get(&(5, 6)), Some(&1));
        assert_eq!(dmap.get(&(5, 4)), Some(&1));
        assert_eq!(dmap.get(&(7, 5)), Some(&2));
        // Distance 3 should be omitted since max_distance = 2
        assert_eq!(dmap.get(&(8, 5)), None);
    }

    #[test]
    fn test_dijkstra_map_around_wall() {
        // Wall at (1, 0)
        let is_passable = |x: i32, y: i32| !(x == 1 && y == 0);
        let dmap = dijkstra_map((0, 0), 4, false, is_passable);

        assert_eq!(dmap.get(&(0, 0)), Some(&0));
        assert_eq!(dmap.get(&(1, 0)), None); // Wall has no distance
        // Route to (2, 0) goes via (0, 1) -> (1, 1) -> (2, 1) -> (2, 0) = 4 steps
        assert_eq!(dmap.get(&(2, 0)), Some(&4));
    }
}
