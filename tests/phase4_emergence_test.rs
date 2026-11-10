use std::path::PathBuf;
use textcraft::ecs::components::LAYER_CREATURE;
use textcraft::logic::SchemeLogic;

fn get_scripts_dir() -> PathBuf {
    let scripts_dir = PathBuf::from("models/roguelike");
    if scripts_dir.exists() {
        scripts_dir
    } else {
        PathBuf::from("../../models/roguelike")
    }
}

fn eval_str(logic: &mut SchemeLogic, expr: &str) -> String {
    let out = logic.eval_repl(expr).unwrap();
    out.trim_start_matches("=>").trim().to_string()
}

fn eval_int(logic: &mut SchemeLogic, expr: &str) -> isize {
    let s = eval_str(logic, expr);
    let last = s.split_whitespace().last().unwrap_or(&s);
    last.parse::<isize>()
        .unwrap_or_else(|e| panic!("Failed to parse int from {:?}: {}", s, e))
}

fn eval_bool(logic: &mut SchemeLogic, expr: &str) -> bool {
    let s = eval_str(logic, expr);
    s == "#true" || s == "#t"
}

#[test]
fn test_anatomy_system_and_dismemberment() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    // 1. Spawn a player hero and build humanoid body
    let hero = eval_int(&mut logic, "(ecs:spawn 'Hero)");
    eval_bool(&mut logic, &format!("(ecs:set-pos {} 10 10 {})", hero, LAYER_CREATURE));

    let _body = eval_str(&mut logic, &format!("(anatomy:build-humanoid-body {})", hero));

    // Verify limbs exist
    let hand_r = eval_int(&mut logic, &format!("(anatomy:find-limb {} 'hand_right)", hero));
    let arm_r = eval_int(&mut logic, &format!("(anatomy:find-limb {} 'arm_right)", hero));
    assert!(hand_r > 0);
    assert!(arm_r > 0);
    assert!(eval_bool(&mut logic, &format!("(anatomy:can-hold-weapon? {})", hero)));
    assert_eq!(eval_int(&mut logic, &format!("(anatomy:count-usable-hands {})", hero)), 2);

    // 2. Spawn and equip an Iron Sword on right hand
    let sword = eval_int(&mut logic, "(ecs:spawn 'IronSword)");
    assert!(eval_bool(&mut logic, &format!("(anatomy:equip {} {})", hand_r, sword)));
    assert_eq!(eval_int(&mut logic, &format!("(anatomy:get-equipped-item {})", hand_r)), sword);

    // 3. Dismemberment! Sever the right arm
    // Severing the arm should cascade: sever arm, sever right hand, and drop the equipped sword!
    let severed_drops = eval_str(&mut logic, &format!("(anatomy:sever-limb {} {})", hero, arm_r));
    assert!(severed_drops.contains(&arm_r.to_string()));
    assert!(severed_drops.contains(&hand_r.to_string()));
    assert!(severed_drops.contains(&sword.to_string()));

    // Right arm is now a physical item on the floor at (10, 10)
    let arm_pos = eval_str(&mut logic, &format!("(ecs:get-pos {})", arm_r));
    assert_eq!(arm_pos, "(10 10 1)"); // Layer 1: LAYER_ITEM_LIQUID

    // Right arm has severed glyph '%' and tag Item
    let arm_glyph = eval_str(&mut logic, &format!("(hash-ref (ecs:get-glyph {}) \"ch\")", arm_r));
    assert_eq!(arm_glyph, "#\\%");
    assert!(eval_bool(&mut logic, &format!("(ecs:has-tag? {} 'SeveredLimb)", arm_r)));

    // Sword has been dropped to the ground at (10, 10) and unequipped
    let sword_pos = eval_str(&mut logic, &format!("(ecs:get-pos {})", sword));
    assert_eq!(sword_pos, "(10 10 1)");
    assert!(!eval_bool(&mut logic, &format!("(ecs:get {} \"equipped\" #f)", sword)));

    // Hero now has only 1 usable hand left (left hand)
    assert_eq!(eval_int(&mut logic, &format!("(anatomy:count-usable-hands {})", hero)), 1);
}

#[test]
fn test_chemistry_and_thermodynamic_reactions() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    // 1. Spawn an oil pool at (5, 5)
    let oil = eval_int(&mut logic, "(chemistry:spawn-liquid 5 5 'oil)");
    assert!(oil > 0);
    assert!(eval_bool(&mut logic, &format!("(ecs:has-tag? {} 'Liquid)", oil)));

    // 2. Ignite the oil pool -> bursts into fire!
    assert!(eval_bool(&mut logic, "(chemistry:ignite 5 5)"));

    // Verify fire liquid pool replaced the oil
    let fire_at_5_5 = eval_str(&mut logic, "(ecs:at 5 5 1)");
    assert!(!fire_at_5_5.contains(&oil.to_string()));

    // 3. Water pool interaction: pouring water and igniting creates steam
    let _water = eval_int(&mut logic, "(chemistry:spawn-liquid 6 6 'water)");
    assert!(!eval_bool(&mut logic, "(chemistry:ignite 6 6)")); // Quenched by water, returns #f

    // Verify steam was generated at (6, 6) and blocks vision (opaque = #t)
    let steam_ids = eval_str(&mut logic, "(ecs:at 6 6 1)");
    assert!(!steam_ids.is_empty());
    let steam_eid: isize = steam_ids
        .trim_matches(|c| c == '(' || c == ')')
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(eval_bool(&mut logic, &format!("(hash-ref (ecs:get-glyph {}) \"opaque\")", steam_eid)));

    // 4. Acid pool: damages and stains creature
    let victim = eval_int(&mut logic, "(ecs:spawn 'Victim)");
    eval_bool(&mut logic, &format!("(ecs:set-pos {} 7 7 {})", victim, LAYER_CREATURE));
    eval_bool(&mut logic, &format!("(ecs:set {} \"hp\" 100)", victim));

    let _acid = eval_int(&mut logic, "(chemistry:pour-acid 7 7)");
    let hp_after_acid = eval_int(&mut logic, &format!("(ecs:get {} \"hp\")", victim));
    assert_eq!(hp_after_acid, 80);
    assert!(eval_bool(&mut logic, &format!("(ecs:has-tag? {} 'CoveredInAcid)", victim)));
}

#[test]
fn test_mutation_system_and_cooldowns() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    let mutant = eval_int(&mut logic, "(ecs:spawn 'Mutant)");
    eval_bool(&mut logic, &format!("(ecs:set-pos {} 20 20 {})", mutant, LAYER_CREATURE));
    let _body = eval_str(&mut logic, &format!("(anatomy:build-humanoid-body {})", mutant));

    // Initially 2 hands
    assert_eq!(eval_int(&mut logic, &format!("(anatomy:count-usable-hands {})", mutant)), 2);

    // 1. Grant MultipleArms physical mutation -> gives extra arm and hand!
    assert!(eval_bool(&mut logic, &format!("(mutation:grant {} 'MultipleArms)", mutant)));
    assert!(eval_bool(&mut logic, &format!("(mutation:has? {} 'MultipleArms)", mutant)));
    assert_eq!(eval_int(&mut logic, &format!("(anatomy:count-usable-hands {})", mutant)), 3);

    // 2. Grant FlamingHands and use it
    assert!(eval_bool(&mut logic, &format!("(mutation:grant {} 'FlamingHands)", mutant)));
    let skill_res = eval_str(&mut logic, &format!("(hash-ref (mutation:use {} 'FlamingHands 21 20) \"status\")", mutant));
    assert_eq!(skill_res, "\"success\"");

    // Skill is now on cooldown
    let retry_res = eval_str(&mut logic, &format!("(hash-ref (mutation:use {} 'FlamingHands 21 20) \"status\")", mutant));
    assert_eq!(retry_res, "\"failed\"");

    // 3. Tick cooldowns 5 times -> ready again!
    for _ in 0..5 {
        eval_str(&mut logic, &format!("(mutation:tick-cooldowns {})", mutant));
    }
    let ready_res = eval_str(&mut logic, &format!("(hash-ref (mutation:use {} 'FlamingHands 21 20) \"status\")", mutant));
    assert_eq!(ready_res, "\"success\"");
}

#[test]
fn test_worldgen_caves_and_salt_marsh() {
    let dir = get_scripts_dir();
    let mut logic = SchemeLogic::new(&dir, false, None).expect("SchemeLogic creation failed");
    logic.init().expect("SchemeLogic init failed");

    // 1. Generate Subterranean Cave Dungeon (25x20)
    let cave_info = eval_str(&mut logic, "(worldgen:generate-dungeon 'caves 25 20 42)");
    assert!(cave_info.contains("\"zone\""));
    assert!(cave_info.contains("\"caves\""));

    // Verify cave wall and floor entities exist in ECS
    let walls = eval_str(&mut logic, "(ecs:query \"Wall\")");
    let floors = eval_str(&mut logic, "(ecs:query \"Floor\")");
    assert!(!walls.is_empty());
    assert!(!floors.is_empty());

    // 2. Generate Salt Marsh
    let marsh_info = eval_str(&mut logic, "(worldgen:generate-dungeon 'salt_marsh 25 20 101)");
    assert!(marsh_info.contains("\"zone\""));
    assert!(marsh_info.contains("\"salt_marsh\""));

    // Verify salt pillars spawned via Simplex noise
    let pillars = eval_str(&mut logic, "(ecs:query \"Pillar\")");
    assert!(!pillars.is_empty());
}
