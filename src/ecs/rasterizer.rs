use crate::ecs::components::Color;
use crate::ecs::world::EcsWorld;

/// High-performance terminal ANSI rasterizer with run-length color state caching.
#[derive(Debug, Clone)]
pub struct ViewportRasterizer {
    screen_buffer: String,
}

impl Default for ViewportRasterizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportRasterizer {
    pub fn new() -> Self {
        Self {
            screen_buffer: String::with_capacity(80 * 24 * 16),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            screen_buffer: String::with_capacity(capacity),
        }
    }

    /// Render a viewport centered at `(center_x, center_y)` into an ANSI string.
    ///
    /// - `cols`: total columns of the terminal (e.g. 80)
    /// - `rows`: total rows of the terminal (e.g. 24)
    /// - `double_width`: whether to use 2 terminal columns per logical grid cell
    #[inline]
    pub fn render(
        &mut self,
        ecs: &EcsWorld,
        center_x: i32,
        center_y: i32,
        cols: u16,
        rows: u16,
        double_width: bool,
    ) -> &str {
        self.render_with_visibility(ecs, center_x, center_y, cols, rows, double_width, None)
    }

    /// Render a viewport centered at `(center_x, center_y)` with an optional FOV visibility test.
    #[allow(clippy::too_many_arguments)]
    pub fn render_with_visibility(
        &mut self,
        ecs: &EcsWorld,
        center_x: i32,
        center_y: i32,
        cols: u16,
        rows: u16,
        double_width: bool,
        visibility: Option<&dyn Fn(i32, i32) -> bool>,
    ) -> &str {
        self.screen_buffer.clear();

        let grid_cols = if double_width { cols / 2 } else { cols };
        let half_w = (grid_cols / 2) as i32;
        let half_h = (rows / 2) as i32;
        let left = center_x - half_w;
        let top = center_y - half_h;

        let mut current_fg: Option<Color> = None;
        let mut current_bg: Option<Color> = None;

        for r in 0..rows {
            let y = top + r as i32;

            for c in 0..grid_cols {
                let x = left + c as i32;

                // Check FOV visibility if predicate supplied
                let is_visible = visibility.is_none_or(|f| f(x, y));

                let (ch, fg, bg, is_dw) = if !is_visible {
                    (' ', None, None, false)
                } else {
                    match ecs.spatial.top_glyph(x, y) {
                        Some(glyph) => {
                            let fg = if x == center_x && y == center_y && glyph.ch == '@' {
                                Some(Color::rgb(255, 255, 100))
                            } else {
                                glyph.fg
                            };
                            (glyph.ch, fg, glyph.bg, glyph.double_width)
                        }
                        None => (' ', None, None, false),
                    }
                };

                // State-cached ANSI color emission
                if fg != current_fg || bg != current_bg {
                    let need_reset = (current_fg.is_some() && fg.is_none())
                        || (current_bg.is_some() && bg.is_none());

                    if need_reset {
                        self.screen_buffer.push_str("\x1b[0m");
                        current_fg = None;
                        current_bg = None;
                    }

                    if fg != current_fg {
                        if let Some(fg_col) = fg {
                            fast_write_fg_rgb(&mut self.screen_buffer, fg_col.r, fg_col.g, fg_col.b);
                            current_fg = Some(fg_col);
                        }
                    }

                    if bg != current_bg {
                        if let Some(bg_col) = bg {
                            fast_write_bg_rgb(&mut self.screen_buffer, bg_col.r, bg_col.g, bg_col.b);
                            current_bg = Some(bg_col);
                        }
                    }
                }

                // Push glyph
                self.screen_buffer.push(ch);
                if double_width && !is_dw {
                    self.screen_buffer.push(' ');
                }
            }

            // End of row: reset color and append newline
            if current_fg.is_some() || current_bg.is_some() {
                self.screen_buffer.push_str("\x1b[0m");
                current_fg = None;
                current_bg = None;
            }

            if r + 1 < rows {
                self.screen_buffer.push('\n');
            }
        }

        &self.screen_buffer
    }
}

#[inline(always)]
fn write_u8_fast(buf: &mut String, mut n: u8) {
    if n >= 100 {
        buf.push((b'0' + (n / 100)) as char);
        n %= 100;
        buf.push((b'0' + (n / 10)) as char);
        buf.push((b'0' + (n % 10)) as char);
    } else if n >= 10 {
        buf.push((b'0' + (n / 10)) as char);
        buf.push((b'0' + (n % 10)) as char);
    } else {
        buf.push((b'0' + n) as char);
    }
}

#[inline(always)]
fn fast_write_fg_rgb(buf: &mut String, r: u8, g: u8, b: u8) {
    buf.push_str("\x1b[38;2;");
    write_u8_fast(buf, r);
    buf.push(';');
    write_u8_fast(buf, g);
    buf.push(';');
    write_u8_fast(buf, b);
    buf.push('m');
}

#[inline(always)]
fn fast_write_bg_rgb(buf: &mut String, r: u8, g: u8, b: u8) {
    buf.push_str("\x1b[48;2;");
    write_u8_fast(buf, r);
    buf.push(';');
    write_u8_fast(buf, g);
    buf.push(';');
    write_u8_fast(buf, b);
    buf.push('m');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::LAYER_TERRAIN;

    #[test]
    fn test_rasterizer_empty_viewport() {
        let ecs = EcsWorld::new();
        let mut rasterizer = ViewportRasterizer::new();

        let output = rasterizer.render(&ecs, 0, 0, 10, 5, false);
        let lines: Vec<&str> = output.split('\n').collect();
        assert_eq!(lines.len(), 5);
        for line in lines {
            assert_eq!(line, "          ");
        }
    }

    #[test]
    fn test_rasterizer_entity_glyph_and_colors() {
        let mut ecs = EcsWorld::new();
        let mut rasterizer = ViewportRasterizer::new();

        let e = ecs.spawn();
        ecs.set_pos(e, 0, 0, LAYER_TERRAIN);
        ecs.set_glyph(
            e,
            '#',
            Some(Color::new(200, 100, 50)),
            Some(Color::new(20, 20, 20)),
            true,
            true,
        );

        let output = rasterizer.render(&ecs, 0, 0, 3, 3, false);
        assert!(output.contains('#'));
        assert!(output.contains("\x1b[38;2;200;100;50m"));
        assert!(output.contains("\x1b[48;2;20;20;20m"));
        assert!(output.contains("\x1b[0m"));
    }

    #[test]
    fn test_rasterizer_double_width() {
        let mut ecs = EcsWorld::new();
        let mut rasterizer = ViewportRasterizer::new();

        let e = ecs.spawn();
        ecs.set_pos(e, 0, 0, LAYER_TERRAIN);
        ecs.set_glyph(e, '@', None, None, true, false);

        let output = rasterizer.render(&ecs, 0, 0, 6, 1, true);
        assert!(output.contains("@ "));
    }
}
