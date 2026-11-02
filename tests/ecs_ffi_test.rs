use std::path::PathBuf;
use textcraft::ecs::components::{Color, PlayerSession, LAYER_CREATURE, LAYER_TERRAIN};
use textcraft::logic::SchemeLogic;

fn get_scripts_dir() -> PathBuf {
    let scripts_dir = PathBuf::from("scripts");
    if scripts_dir.exists() {
        scripts_dir
    } else {
        PathBuf::from("../../scripts")
    }
}

fn eval_int(logic: &mut SchemeLogic, expr: &str) -> isize {
    let out = logic.eval_repl(expr).unwrap();
    let clean = out.trim_start_matches("=>").trim();
    let last = clean.split_whitespace().last().unwrap_or(clean);
    last
        .parse::<isize>()
        .unwrap_or_else(|e| panic!("Failed to parse int from {:?}: {}", out, e))
}

fn eval_str(logic: &mut SchemeLogic, expr: &str) -> String {
    let out = logic.eval_repl(expr).unwrap();
    out.trim_start_matches("=>").trim().to_string()
}

fn eval_bool(logic: &mut SchemeLogic, expr: &str) -> bool {
    let s = eval_str(logic, expr);
    s == "#true" || s == "#t"
}

#[test]
fn test_ecs_ffi_entity_lifecycle() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    // 1. (ecs:spawn) and (ecs:spawn name)
    let e1 = eval_int(&mut logic, "(ecs:spawn)");
    assert!(e1 > 0);

    let e2 = eval_int(&mut logic, "(ecs:spawn 'Goblin)");
    assert!(e2 > 0);
    assert_ne!(e1, e2);

    // 2. (ecs:exists? eid)
    assert!(eval_bool(&mut logic, &format!("(ecs:exists? {})", e1)));
    assert!(eval_bool(&mut logic, &format!("(ecs:exists? {})", e2)));
    assert!(!eval_bool(&mut logic, "(ecs:exists? 999999)"));
    assert!(!eval_bool(&mut logic, "(ecs:exists? #f)"));

    // 3. (ecs:set-pos eid x y layer) & (ecs:get-pos eid)
    assert!(eval_bool(&mut logic, &format!("(ecs:set-pos {} 12 34 {})", e1, LAYER_CREATURE)));

    let get_pos_res = eval_str(&mut logic, &format!("(ecs:get-pos {})", e1));
    assert_eq!(get_pos_res, "(12 34 2)");

    assert!(!eval_bool(&mut logic, "(ecs:get-pos 999999)"));

    // 4. (ecs:set-glyph eid char fg bg solid? opaque?) & (ecs:get-glyph eid)
    assert!(eval_bool(
        &mut logic,
        &format!("(ecs:set-glyph {} #\\@ '(255 100 50) '(10 20 30) #t #f)", e1),
    ));

    let glyph_ch = eval_str(&mut logic, &format!("(hash-ref (ecs:get-glyph {}) \"ch\")", e1));
    assert!(glyph_ch.contains("@"));

    let glyph_fg = eval_str(&mut logic, &format!("(hash-ref (ecs:get-glyph {}) \"fg\")", e1));
    assert_eq!(glyph_fg, "(255 100 50)");

    assert!(eval_bool(&mut logic, &format!("(hash-ref (ecs:get-glyph {}) \"solid\")", e1)));
    assert!(!eval_bool(&mut logic, &format!("(hash-ref (ecs:get-glyph {}) \"opaque\")", e1)));

    // 5. (ecs:at x y [layer])
    let at_res = eval_str(&mut logic, "(ecs:at 12 34)");
    assert!(at_res.contains(&e1.to_string()));

    let at_layer_res = eval_str(&mut logic, &format!("(ecs:at 12 34 {})", LAYER_CREATURE));
    assert!(at_layer_res.contains(&e1.to_string()));

    let at_other_layer = eval_str(&mut logic, &format!("(ecs:at 12 34 {})", LAYER_TERRAIN));
    assert_eq!(at_other_layer, "()");

    // 6. (ecs:despawn eid)
    assert!(eval_bool(&mut logic, &format!("(ecs:despawn {})", e1)));
    assert!(!eval_bool(&mut logic, &format!("(ecs:exists? {})", e1)));
    assert_eq!(eval_str(&mut logic, "(ecs:at 12 34)"), "()");
}

#[test]
fn test_ecs_ffi_dynamic_props_and_tags() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    let eid = eval_int(&mut logic, "(ecs:spawn \"Tester\")");

    // 1. Dynamic Props
    assert!(!eval_bool(&mut logic, &format!("(ecs:has? {} \"hp\")", eid)));
    assert!(!eval_bool(&mut logic, &format!("(ecs:get {} \"hp\")", eid)));
    assert_eq!(eval_str(&mut logic, &format!("(ecs:get {} \"hp\" 100)", eid)), "100");

    assert!(eval_bool(&mut logic, &format!("(ecs:set {} \"hp\" 250)", eid)));
    assert!(eval_bool(&mut logic, &format!("(ecs:has? {} \"hp\")", eid)));
    assert_eq!(eval_str(&mut logic, &format!("(ecs:get {} \"hp\")", eid)), "250");

    // Complex data in dynamic prop (hash and list)
    eval_str(&mut logic, &format!("(ecs:set {} \"inventory\" '(sword shield potion))", eid));
    let inv = eval_str(&mut logic, &format!("(ecs:get {} \"inventory\")", eid));
    assert!(inv.contains("sword"));
    assert!(inv.contains("shield"));

    // 2. Tags
    assert!(!eval_bool(&mut logic, &format!("(ecs:has-tag? {} \"Monster\")", eid)));
    assert!(eval_bool(&mut logic, &format!("(ecs:add-tag {} \"Monster\")", eid)));
    assert!(eval_bool(&mut logic, &format!("(ecs:has-tag? {} \"Monster\")", eid)));
    assert!(eval_bool(&mut logic, &format!("(ecs:remove-tag {} \"Monster\")", eid)));
    assert!(!eval_bool(&mut logic, &format!("(ecs:has-tag? {} \"Monster\")", eid)));
}

#[test]
fn test_ecs_ffi_relations_and_queries() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    let player = eval_int(&mut logic, "(ecs:spawn 'Player)");
    let hand = eval_int(&mut logic, "(ecs:spawn 'Hand)");
    let axe = eval_int(&mut logic, "(ecs:spawn 'Axe)");

    // hand PartOf player
    assert!(eval_bool(&mut logic, &format!("(ecs:relate {} \"PartOf\" {})", hand, player)));
    assert_eq!(
        eval_str(&mut logic, &format!("(ecs:target {} \"PartOf\")", hand)),
        player.to_string()
    );

    // axe EquippedOn hand
    assert!(eval_bool(&mut logic, &format!("(ecs:relate {} \"EquippedOn\" {})", axe, hand)));
    assert_eq!(
        eval_str(&mut logic, &format!("(ecs:target {} \"EquippedOn\")", axe)),
        hand.to_string()
    );

    // Children: hand children with EquippedOn should be axe
    let children_res = eval_str(&mut logic, &format!("(ecs:children {} \"EquippedOn\")", hand));
    assert!(children_res.contains(&axe.to_string()));

    // Unrelate
    assert!(eval_bool(&mut logic, &format!("(ecs:unrelate {} \"EquippedOn\" {})", axe, hand)));
    assert!(!eval_bool(&mut logic, &format!("(ecs:target {} \"EquippedOn\")", axe)));

    // Flecs C Query DSL
    let m1 = eval_int(&mut logic, "(ecs:spawn)");
    let m2 = eval_int(&mut logic, "(ecs:spawn)");
    eval_str(&mut logic, &format!("(ecs:set-pos {} 1 1 2)", m1));
    eval_str(&mut logic, &format!("(ecs:add-tag {} \"Creature\")", m1));
    eval_str(&mut logic, &format!("(ecs:set-pos {} 2 2 2)", m2));
    eval_str(&mut logic, &format!("(ecs:add-tag {} \"Creature\")", m2));

    let query_res = eval_str(&mut logic, "(ecs:query \"GridPos, Creature\")");
    assert!(query_res.contains(&m1.to_string()));
    assert!(query_res.contains(&m2.to_string()));
}

#[test]
fn test_prelude_dsl_and_macros() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    // 1. define-entity macro
    let define_code = r#"
    (begin
      (define-entity hero
        (pos 15 25 2)
        (glyph #\@ '(255 255 100) #f)
        (props ("hp" 180) ("mana" 90)))
      hero)
    "#;
    let hero_eid = eval_int(&mut logic, define_code);
    assert!(hero_eid > 0);

    let hero_pos = eval_str(&mut logic, &format!("(ecs:get-pos {})", hero_eid));
    assert_eq!(hero_pos, "(15 25 2)");

    let hero_hp = eval_str(&mut logic, &format!("(ecs:get {} \"hp\")", hero_eid));
    assert_eq!(hero_hp, "180");

    // 2. with-tag macro
    let tag_test = format!(
        r#"
        (let ((before (ecs:has-tag? {eid} "Enraged")))
          (with-tag {eid} "Enraged"
            (let ((during (ecs:has-tag? {eid} "Enraged")))
              (list before during (ecs:has-tag? {eid} "Enraged")))))
        "#,
        eid = hero_eid
    );
    let tag_res = eval_str(&mut logic, &tag_test);
    assert_eq!(tag_res, "(#false #true #true)");
    // After with-tag block, tag must be cleaned up
    assert!(!eval_bool(&mut logic, &format!("(ecs:has-tag? {} \"Enraged\")", hero_eid)));

    // 3. Relation helpers: (relate! a rel b), (equipped-on a b), (part-of a b)
    let setup_relations = format!(
        r#"
        (begin
          (define sword (ecs:spawn "SteelSword"))
          (define hand (ecs:spawn "LeftHand"))
          (part-of hand {hero})
          (equipped-on sword hand)
          (list (part-of? hand {hero}) (equipped-on? sword hand) (equipped-items hand) (parts-of {hero})))
        "#,
        hero = hero_eid
    );
    let rel_res = eval_str(&mut logic, &setup_relations);
    assert!(rel_res.contains("#true #true"));
}

#[test]
fn test_render_snapshot_with_native_rasterizer() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    // Spawn a player entity directly in ECS World with GridPos, VisualGlyph, and PlayerSession
    let player_id: u64 = 777;
    {
        let ecs_arc = logic.ecs_world();
        let mut ecs = ecs_arc.lock();
        let e = ecs.spawn_named("Player777");
        ecs.set_pos(e, 10, 10, LAYER_CREATURE);
        ecs.set_glyph(
            e,
            '@',
            Some(Color::new(255, 255, 0)),
            None,
            true,
            true,
        );

        let ent = ecs.world.entity_from_id(e);
        ent.set(PlayerSession::new(player_id, 80, 24));

        // Add some surrounding walls
        let w1 = ecs.spawn();
        ecs.set_pos(w1, 9, 9, LAYER_TERRAIN);
        ecs.set_glyph(w1, '#', Some(Color::new(100, 100, 100)), None, true, true);
    }

    // Connect player in Scheme session
    logic.on_player_join(player_id, "token_777").unwrap();

    let snap = logic.render_snapshot(player_id).expect("Render snapshot failed");
    assert_eq!(snap.cols, 80);
    assert_eq!(snap.rows, 24);
    assert!(!snap.screen.is_empty());
    // Since ECS spatial grid is populated, the native rasterizer renders '@' and '#'
    assert!(snap.screen.contains('@'));
    assert!(snap.screen.contains('#'));
}
