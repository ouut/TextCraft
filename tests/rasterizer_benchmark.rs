use std::time::Instant;
use textcraft::ecs::{
    Color, EcsWorld, ViewportRasterizer, LAYER_CREATURE, LAYER_ITEM_LIQUID, LAYER_OVERLAY,
    LAYER_TERRAIN,
};

#[test]
fn test_rasterizer_80x24_performance_target() {
    let mut ecs = EcsWorld::new();
    let mut rasterizer = ViewportRasterizer::new();

    // Populate a realistic 80x24 map (1,920 cells)
    for y in -12..12 {
        for x in -40..40 {
            let e = ecs.spawn();
            ecs.set_pos(e, x, y, LAYER_TERRAIN);
            if x == -40 || x == 39 || y == -12 || y == 11 {
                ecs.set_glyph(
                    e,
                    '#',
                    Some(Color::new(150, 150, 150)),
                    Some(Color::new(30, 30, 30)),
                    true,
                    true,
                );
            } else if (x + y) % 5 == 0 {
                ecs.set_glyph(
                    e,
                    ',',
                    Some(Color::new(50, 180, 50)),
                    None,
                    false,
                    false,
                );
            } else {
                ecs.set_glyph(
                    e,
                    '.',
                    Some(Color::new(80, 80, 80)),
                    None,
                    false,
                    false,
                );
            }
        }
    }

    // Add items and liquids (layer 1)
    for i in -20..20 {
        if i % 3 == 0 {
            let item = ecs.spawn();
            ecs.set_pos(item, i, i / 2, LAYER_ITEM_LIQUID);
            ecs.set_glyph(
                item,
                '~',
                Some(Color::new(0, 150, 255)),
                Some(Color::new(0, 30, 80)),
                false,
                false,
            );
        }
    }

    // Add creatures and monsters (layer 2)
    for i in -15..15 {
        if i % 2 == 0 {
            let monster = ecs.spawn();
            ecs.set_pos(monster, i, -i / 2, LAYER_CREATURE);
            ecs.set_glyph(
                monster,
                'g',
                Some(Color::new(255, 60, 60)),
                None,
                true,
                false,
            );
        }
    }

    // Add overlays (layer 3)
    let overlay = ecs.spawn();
    ecs.set_pos(overlay, 0, 0, LAYER_OVERLAY);
    ecs.set_glyph(
        overlay,
        '*',
        Some(Color::new(255, 255, 100)),
        None,
        false,
        false,
    );

    // Warm-up run
    for _ in 0..100 {
        let frame = rasterizer.render(&ecs, 0, 0, 80, 24, false);
        std::hint::black_box(frame);
    }

    // Benchmark 10,000 frames
    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let frame = rasterizer.render(&ecs, 0, 0, 80, 24, false);
        std::hint::black_box(frame);
    }
    let elapsed = start.elapsed();
    let per_frame_micros = elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64;

    println!(
        "\n=======================================================\n\
         [Phase 1 Benchmark] 80x24 Viewport ANSI Rasterizer\n\
         - Total frames: {}\n\
         - Total time:   {:?}\n\
         - Per frame:    {:.2} μs (Target: < 30.00 μs)\n\
         =======================================================",
        iterations, elapsed, per_frame_micros
    );

    #[cfg(not(debug_assertions))]
    {
        assert!(
            per_frame_micros < 30.0,
            "Rasterizer exceeded performance budget: {:.2} μs >= 30.0 μs",
            per_frame_micros
        );
    }
}
