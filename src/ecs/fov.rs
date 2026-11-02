use rustc_hash::FxHashSet;

/// Metric for clipping field of view by maximum radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistanceMetric {
    /// Euclidean circle: dx*dx + dy*dy <= radius*radius
    #[default]
    Euclidean,
    /// Chebyshev square box: max(|dx|, |dy|) <= radius
    Chebyshev,
    /// Manhattan taxicab diamond: |dx| + |dy| <= radius
    Manhattan,
}

impl DistanceMetric {
    #[inline]
    pub fn is_within(&self, dx: i32, dy: i32, radius: i32) -> bool {
        match self {
            DistanceMetric::Euclidean => dx * dx + dy * dy <= radius * radius,
            DistanceMetric::Chebyshev => dx.abs() <= radius && dy.abs() <= radius,
            DistanceMetric::Manhattan => dx.abs() + dy.abs() <= radius,
        }
    }
}

/// Exact rational slope representation `num / den` (where `den > 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slope {
    pub num: i32,
    pub den: i32,
}

impl Slope {
    #[inline]
    pub const fn new(num: i32, den: i32) -> Self {
        debug_assert!(den > 0, "Slope denominator must be positive");
        Self { num, den }
    }

    /// Compare `self <= other` using exact cross-multiplication.
    #[inline]
    pub fn le(&self, other: &Self) -> bool {
        (self.num as i64) * (other.den as i64) <= (other.num as i64) * (self.den as i64)
    }

    /// Compare `self >= other` using exact cross-multiplication.
    #[inline]
    pub fn ge(&self, other: &Self) -> bool {
        (self.num as i64) * (other.den as i64) >= (other.num as i64) * (self.den as i64)
    }
}

/// A row segment of tiles bounded by `start_slope` and `end_slope` at `depth`.
#[derive(Debug, Clone, Copy)]
struct Row {
    depth: i32,
    start_slope: Slope,
    end_slope: Slope,
}

impl Row {
    #[inline]
    fn new(depth: i32, start_slope: Slope, end_slope: Slope) -> Self {
        Self {
            depth,
            start_slope,
            end_slope,
        }
    }

    #[inline]
    fn next(&self) -> Self {
        Self {
            depth: self.depth + 1,
            start_slope: self.start_slope,
            end_slope: self.end_slope,
        }
    }
}

/// Round rational `n / d` to nearest integer; ties (.5) round up towards +infinity.
/// Uses exact Euclidean division: floor((2n + d) / 2d).
#[inline]
fn round_ties_up(n: i64, d: i64) -> i32 {
    (2 * n + d).div_euclid(2 * d) as i32
}

/// Round rational `n / d` to nearest integer; ties (.5) round down towards -infinity.
/// Uses exact Euclidean division: floor((2n + d - 1) / 2d).
#[inline]
fn round_ties_down(n: i64, d: i64) -> i32 {
    (2 * n + d - 1).div_euclid(2 * d) as i32
}

/// Calculate the left edge slope of a tile at (depth, col):
/// slope = (2 * col - 1) / (2 * depth).
#[inline]
fn tile_slope(depth: i32, col: i32) -> Slope {
    Slope::new(2 * col - 1, 2 * depth)
}

/// Check if a tile at (depth, col) falls symmetrically within the row's slopes:
/// col >= depth * start_slope && col <= depth * end_slope.
#[inline]
fn is_symmetric(row: &Row, depth: i32, col: i32) -> bool {
    let c = col as i64;
    let d = depth as i64;
    (c * row.start_slope.den as i64 >= d * row.start_slope.num as i64)
        && (c * row.end_slope.den as i64 <= d * row.end_slope.num as i64)
}

/// Four cardinal quadrants for symmetric shadowcasting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quadrant {
    North,
    East,
    South,
    West,
}

impl Quadrant {
    const ALL: [Quadrant; 4] = [
        Quadrant::North,
        Quadrant::East,
        Quadrant::South,
        Quadrant::West,
    ];

    #[inline]
    fn transform(&self, cx: i32, cy: i32, depth: i32, col: i32) -> (i32, i32) {
        match self {
            Quadrant::North => (cx + col, cy - depth),
            Quadrant::South => (cx + col, cy + depth),
            Quadrant::East => (cx + depth, cy + col),
            Quadrant::West => (cx - depth, cy + col),
        }
    }
}

/// Compute field of view using Symmetric Recursive Shadowcasting with configurable distance metric.
///
/// Ensures reciprocal symmetry: if floor tile A can see floor tile B, then B can see A.
/// Walls along the field of view are also marked visible.
pub fn compute_fov_with_metric<F>(
    cx: i32,
    cy: i32,
    radius: i32,
    metric: DistanceMetric,
    is_opaque: F,
) -> FxHashSet<(i32, i32)>
where
    F: Fn(i32, i32) -> bool,
{
    let mut visible = FxHashSet::default();
    if radius < 0 {
        return visible;
    }

    // Origin is always visible
    visible.insert((cx, cy));
    if radius == 0 {
        return visible;
    }

    for quad in Quadrant::ALL {
        scan_quadrant(
            cx,
            cy,
            radius,
            quad,
            Row::new(1, Slope::new(-1, 1), Slope::new(1, 1)),
            metric,
            &is_opaque,
            &mut visible,
        );
    }

    visible
}

/// Compute field of view using Symmetric Recursive Shadowcasting with Euclidean circular clipping.
#[inline]
pub fn compute_fov<F>(cx: i32, cy: i32, radius: i32, is_opaque: F) -> FxHashSet<(i32, i32)>
where
    F: Fn(i32, i32) -> bool,
{
    compute_fov_with_metric(cx, cy, radius, DistanceMetric::Euclidean, is_opaque)
}

#[allow(clippy::too_many_arguments)]
fn scan_quadrant<F>(
    cx: i32,
    cy: i32,
    radius: i32,
    quad: Quadrant,
    mut row: Row,
    metric: DistanceMetric,
    is_opaque: &F,
    visible: &mut FxHashSet<(i32, i32)>,
) where
    F: Fn(i32, i32) -> bool,
{
    if row.depth > radius {
        return;
    }

    let min_col = round_ties_up(
        row.depth as i64 * row.start_slope.num as i64,
        row.start_slope.den as i64,
    );
    let max_col = round_ties_down(
        row.depth as i64 * row.end_slope.num as i64,
        row.end_slope.den as i64,
    );

    let mut prev_tile_opaque: Option<bool> = None;

    for col in min_col..=max_col {
        let (gx, gy) = quad.transform(cx, cy, row.depth, col);
        let opaque = is_opaque(gx, gy);

        if opaque || is_symmetric(&row, row.depth, col) {
            let dx = gx - cx;
            let dy = gy - cy;
            if metric.is_within(dx, dy, radius) {
                visible.insert((gx, gy));
            }
        }

        if let Some(prev_opaque) = prev_tile_opaque {
            if prev_opaque && !opaque {
                // Transition: wall -> floor: narrow start slope
                row.start_slope = tile_slope(row.depth, col);
            }
            if !prev_opaque && opaque {
                // Transition: floor -> wall: split and scan sub-row with narrowed end slope
                let mut next_row = row.next();
                next_row.end_slope = tile_slope(row.depth, col);
                scan_quadrant(cx, cy, radius, quad, next_row, metric, is_opaque, visible);
            }
        }

        prev_tile_opaque = Some(opaque);
    }

    if let Some(prev_opaque) = prev_tile_opaque {
        if !prev_opaque {
            scan_quadrant(cx, cy, radius, quad, row.next(), metric, is_opaque, visible);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fov_open_room() {
        let radius = 3;
        let visible = compute_fov(10, 10, radius, |_x, _y| false);

        // Center must be visible
        assert!(visible.contains(&(10, 10)));
        // Cardinal points at radius must be visible
        assert!(visible.contains(&(10 + radius, 10)));
        assert!(visible.contains(&(10 - radius, 10)));
        assert!(visible.contains(&(10, 10 + radius)));
        assert!(visible.contains(&(10, 10 - radius)));

        // Beyond radius must NOT be visible
        assert!(!visible.contains(&(10 + radius + 1, 10)));
        assert!(!visible.contains(&(10, 10 + radius + 1)));

        // All visible points must satisfy Euclidean distance
        for &(x, y) in &visible {
            let dx = x - 10;
            let dy = y - 10;
            assert!(dx * dx + dy * dy <= radius * radius);
        }
    }

    #[test]
    fn test_fov_wall_shadow() {
        // Horizontal wall of length 5 at y = 8, from x = 8 to 12
        // Origin is at (10, 10), radius = 5
        let is_opaque = |x: i32, y: i32| y == 8 && (8..=12).contains(&x);

        let visible = compute_fov(10, 10, 5, is_opaque);

        // The wall itself should be visible
        assert!(visible.contains(&(10, 8)));
        assert!(visible.contains(&(9, 8)));
        assert!(visible.contains(&(11, 8)));

        // The tiles directly behind the center of the wall should be occluded
        assert!(!visible.contains(&(10, 7)));
        assert!(!visible.contains(&(10, 6)));
        assert!(!visible.contains(&(9, 7)));
        assert!(!visible.contains(&(11, 7)));

        // Tiles to the sides of the wall should still be visible
        assert!(visible.contains(&(7, 8)));
        assert!(visible.contains(&(13, 8)));
    }

    #[test]
    fn test_fov_corner_and_pillar() {
        // Single pillar at (11, 9)
        let is_opaque = |x: i32, y: i32| x == 11 && y == 9;

        let visible = compute_fov(10, 10, 5, is_opaque);

        // Pillar is visible
        assert!(visible.contains(&(11, 9)));

        // Shadow cast behind pillar (at (12, 8)) should be occluded
        assert!(!visible.contains(&(12, 8)));
        // But tiles adjacent to pillar should be visible
        assert!(visible.contains(&(10, 9)));
        assert!(visible.contains(&(11, 10)));
    }

    #[test]
    fn test_fov_radius_zero_and_negative() {
        let neg = compute_fov(5, 5, -1, |_x, _y| false);
        assert!(neg.is_empty());

        let zero = compute_fov(5, 5, 0, |_x, _y| false);
        assert_eq!(zero.len(), 1);
        assert!(zero.contains(&(5, 5)));
    }

    #[test]
    fn test_fov_reciprocal_symmetry() {
        // Map: 11x11 grid with obstacles
        let walls: FxHashSet<(i32, i32)> = [
            (3, 3), (3, 4), (3, 5),
            (7, 2), (7, 3),
            (5, 7), (6, 7), (7, 7),
            (2, 8),
        ].into_iter().collect();

        let is_opaque = |x: i32, y: i32| walls.contains(&(x, y));
        let radius = 6;

        // Collect all floor tiles in 0..10, 0..10
        let floor_tiles: Vec<(i32, i32)> = (0..=10)
            .flat_map(|x| (0..=10).map(move |y| (x, y)))
            .filter(|p| !is_opaque(p.0, p.1))
            .collect();

        // Verify mutual symmetry: For every floor tile A and B:
        // A sees B <=> B sees A
        for &a in &floor_tiles {
            let fov_a = compute_fov(a.0, a.1, radius, is_opaque);
            for &b in &floor_tiles {
                let a_sees_b = fov_a.contains(&b);
                if a_sees_b {
                    let fov_b = compute_fov(b.0, b.1, radius, is_opaque);
                    assert!(
                        fov_b.contains(&a),
                        "Symmetry violated: {:?} sees {:?}, but {:?} cannot see {:?}",
                        a, b, b, a
                    );
                }
            }
        }
    }
}
