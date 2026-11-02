/// 2D Simplex Noise generator based on Ken Perlin's simplex formulation.
///
/// Pure Rust, deterministic with seed, zero external dependencies.
#[derive(Debug, Clone)]
pub struct SimplexNoise {
    perm: [u8; 512],
}

const F2: f64 = 0.3660254037844386; // 0.5 * (sqrt(3.0) - 1.0)
const G2: f64 = 0.2113248654051871; // (3.0 - sqrt(3.0)) / 6.0

const GRAD3: [(f64, f64); 12] = [
    (1.0, 1.0),
    (-1.0, 1.0),
    (1.0, -1.0),
    (-1.0, -1.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (1.0, 0.0),
    (-1.0, 0.0),
    (0.0, 1.0),
    (0.0, -1.0),
    (0.0, 1.0),
    (0.0, -1.0),
];

impl SimplexNoise {
    /// Create a new `SimplexNoise` instance initialized with the given `seed`.
    pub fn new(seed: u64) -> Self {
        let mut p = [0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }

        // SplitMix64 deterministic PRNG for Fisher-Yates shuffle
        let mut rng_state = seed ^ 0x9E3779B97F4A7C15;
        let mut next_u32 = || -> u32 {
            rng_state = rng_state.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = rng_state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            (z ^ (z >> 31)) as u32
        };

        for i in (1..256).rev() {
            let j = (next_u32() as usize) % (i + 1);
            p.swap(i, j);
        }

        let mut perm = [0u8; 512];
        for i in 0..256 {
            perm[i] = p[i];
            perm[i + 256] = p[i];
        }

        Self { perm }
    }

    /// Sample 2D simplex noise at coordinate `(x, y)`.
    /// Returns a normalized continuous float in `[-1.0, 1.0]`.
    pub fn sample_2d(&self, x: f64, y: f64) -> f64 {
        // Skew input coordinates to find simplex cell origin
        let s = (x + y) * F2;
        let i = (x + s).floor() as i32;
        let j = (y + s).floor() as i32;

        // Unskew back to Euclidean space
        let t = (i + j) as f64 * G2;
        let x0 = x - (i as f64 - t);
        let y0 = y - (j as f64 - t);

        // Determine which simplex triangle we are in
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        // Offsets for the remaining two corners
        let x1 = x0 - i1 as f64 + G2;
        let y1 = y0 - j1 as f64 + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;

        // Wrap coordinate hashes into permutation indices
        let ii = (i & 255) as usize;
        let jj = (j & 255) as usize;

        let gi0 = (self.perm[ii + self.perm[jj] as usize] % 12) as usize;
        let gi1 = (self.perm[ii + i1 as usize + self.perm[jj + j1 as usize] as usize] % 12) as usize;
        let gi2 = (self.perm[ii + 1 + self.perm[jj + 1] as usize] % 12) as usize;

        // Calculate corner contributions
        let mut t0 = 0.5 - x0 * x0 - y0 * y0;
        let n0 = if t0 < 0.0 {
            0.0
        } else {
            t0 *= t0;
            t0 * t0 * (GRAD3[gi0].0 * x0 + GRAD3[gi0].1 * y0)
        };

        let mut t1 = 0.5 - x1 * x1 - y1 * y1;
        let n1 = if t1 < 0.0 {
            0.0
        } else {
            t1 *= t1;
            t1 * t1 * (GRAD3[gi1].0 * x1 + GRAD3[gi1].1 * y1)
        };

        let mut t2 = 0.5 - x2 * x2 - y2 * y2;
        let n2 = if t2 < 0.0 {
            0.0
        } else {
            t2 *= t2;
            t2 * t2 * (GRAD3[gi2].0 * x2 + GRAD3[gi2].1 * y2)
        };

        // Scale to [-1.0, 1.0]
        (70.0 * (n0 + n1 + n2)).clamp(-1.0, 1.0)
    }

    /// Sample 2D simplex noise normalized to `[0.0, 1.0]`.
    #[inline]
    pub fn sample_2d_normalized(&self, x: f64, y: f64) -> f64 {
        ((self.sample_2d(x, y) + 1.0) * 0.5).clamp(0.0, 1.0)
    }
}

/// Generate a 2D cave grid using Cellular Automata (4-5 smoothing rule).
///
/// Output is a 2D boolean grid (`Vec<Vec<bool>>`) where:
/// - `true` represents a solid wall
/// - `false` represents an open floor
/// Dimensions: `grid.len() == height`, `grid[y].len() == width`.
pub fn generate_cave(
    width: usize,
    height: usize,
    wall_prob: f64,
    iterations: usize,
    seed: u64,
) -> Vec<Vec<bool>> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    // SplitMix64 PRNG for initial random distribution
    let mut rng_state = seed ^ 0x5555555555555555;
    let mut next_f64 = || -> f64 {
        rng_state = rng_state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        let u = z ^ (z >> 31);
        (u as f64) / (u64::MAX as f64)
    };

    // Initialize grid with perimeter walls and random interior
    let mut grid = vec![vec![true; width]; height];
    for y in 0..height {
        for x in 0..width {
            if x == 0 || x == width - 1 || y == 0 || y == height - 1 {
                grid[y][x] = true;
            } else {
                grid[y][x] = next_f64() < wall_prob;
            }
        }
    }

    // Apply standard 4-5 rule smoothing iterations
    for _ in 0..iterations {
        let mut next_grid = vec![vec![true; width]; height];
        for y in 1..(height - 1) {
            for x in 1..(width - 1) {
                let mut wall_count = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let ny = (y as isize + dy) as usize;
                        let nx = (x as isize + dx) as usize;
                        if grid[ny][nx] {
                            wall_count += 1;
                        }
                    }
                }
                // 4-5 rule: if >= 5 out of 9 neighbors (including self) are walls, becomes wall
                next_grid[y][x] = wall_count >= 5;
            }
        }
        grid = next_grid;
    }

    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplex_noise_deterministic() {
        let noise1 = SimplexNoise::new(12345);
        let noise2 = SimplexNoise::new(12345);

        for i in 0..10 {
            let x = i as f64 * 0.35;
            let y = i as f64 * 0.75;
            assert_eq!(noise1.sample_2d(x, y), noise2.sample_2d(x, y));
        }

        // Different seeds should produce different noise fields
        let noise3 = SimplexNoise::new(54321);
        let mut differs = false;
        for i in 0..10 {
            let x = i as f64 * 0.77 + 0.123;
            let y = i as f64 * 0.33 + 0.456;
            if (noise1.sample_2d(x, y) - noise3.sample_2d(x, y)).abs() > 1e-6 {
                differs = true;
                break;
            }
        }
        assert!(differs, "Different seeds should produce different noise fields");
    }

    #[test]
    fn test_simplex_noise_bounds() {
        let noise = SimplexNoise::new(42);
        for x_step in -20..20 {
            for y_step in -20..20 {
                let val = noise.sample_2d(x_step as f64 * 0.1, y_step as f64 * 0.1);
                assert!((-1.0..=1.0).contains(&val), "Value out of bounds: {}", val);

                let norm = noise.sample_2d_normalized(x_step as f64 * 0.1, y_step as f64 * 0.1);
                assert!((0.0..=1.0).contains(&norm), "Normalized out of bounds: {}", norm);
            }
        }
    }

    #[test]
    fn test_cave_generation_bounds_and_perimeter() {
        let width = 30;
        let height = 20;
        let cave = generate_cave(width, height, 0.45, 4, 9999);

        assert_eq!(cave.len(), height);
        for row in &cave {
            assert_eq!(row.len(), width);
        }

        // Perimeter must be solid walls
        for x in 0..width {
            assert!(cave[0][x], "Top perimeter breached at x={}", x);
            assert!(cave[height - 1][x], "Bottom perimeter breached at x={}", x);
        }
        for y in 0..height {
            assert!(cave[y][0], "Left perimeter breached at y={}", y);
            assert!(cave[y][width - 1], "Right perimeter breached at y={}", y);
        }
    }

    #[test]
    fn test_cave_generation_deterministic() {
        let cave1 = generate_cave(25, 25, 0.40, 3, 777);
        let cave2 = generate_cave(25, 25, 0.40, 3, 777);
        assert_eq!(cave1, cave2);

        let cave3 = generate_cave(25, 25, 0.40, 3, 888);
        assert_ne!(cave1, cave3);
    }

    #[test]
    fn test_cave_cellular_diffusion_smoothing() {
        // High iteration count causes smoothing to converge into continuous chambers
        let initial = generate_cave(20, 20, 0.45, 0, 123);
        let smoothed = generate_cave(20, 20, 0.45, 5, 123);

        // Raw noise has many isolated flips; smoothed cave reduces isolated wall/floor transitions
        let count_isolated = |grid: &Vec<Vec<bool>>| -> usize {
            let mut isolated = 0;
            for y in 1..(grid.len() - 1) {
                for x in 1..(grid[0].len() - 1) {
                    let tile = grid[y][x];
                    let diff_neighbors = [
                        grid[y - 1][x],
                        grid[y + 1][x],
                        grid[y][x - 1],
                        grid[y][x + 1],
                    ]
                    .iter()
                    .filter(|&&n| n != tile)
                    .count();
                    if diff_neighbors == 4 {
                        isolated += 1;
                    }
                }
            }
            isolated
        };

        assert!(
            count_isolated(&smoothed) <= count_isolated(&initial),
            "Smoothing should eliminate isolated single-tile noise"
        );
    }
}
