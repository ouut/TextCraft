;;; ============================================================================
;;; TextCraft 4.0 - Procedural World Generation System
;;; Inspired by Caves of Qud: Subterranean Caves, Salt Marshes, and Ancient Ruins
;;; Utilizing native Rust Cellular Automata and Simplex Noise operators.
;;; ============================================================================

;; ----------------------------------------------------------------------------
;; 1. Subterranean Caves (元胞自动机洞窟生成)
;; ----------------------------------------------------------------------------

(define (worldgen:generate-cave-dungeon width height seed)
  (let* ((actual-seed (if seed seed 42))
         (raw-grid (ecs:generate-cave width height 0.45 4 actual-seed))
         (spawn-pos #f)
         (monsters-spawned '()))

    (let loop-y ((y 0) (rows raw-grid))
      (if (null? rows)
          #t
          (let ((row (car rows)))
            (let loop-x ((x 0) (cols row))
              (if (null? cols)
                  #t
                  (let ((is-wall? (car cols)))
                    (if is-wall?
                        ;; Solid cave wall
                        (let ((w (ecs:spawn "cave_wall")))
                          (ecs:set-pos w x y LAYER-TERRAIN)
                          (ecs:set-glyph w "#" '(120 115 110) '(30 28 26) #t #t)
                          (ecs:add-tag w "Wall"))
                        ;; Open cave floor
                        (let ((f (ecs:spawn "cave_floor")))
                          (ecs:set-pos f x y LAYER-TERRAIN)
                          (let ((decor-seed (modulo (+ (* x 31) (* y 17) actual-seed) 100)))
                            (cond
                              ;; Salt/Moss patches
                              ((< decor-seed 8)
                               (ecs:set-glyph f "," '(70 160 80) #f #f #f))
                              ;; Rough gravel
                              ((< decor-seed 16)
                               (ecs:set-glyph f ";" '(100 95 90) #f #f #f))
                              ;; Standard dirt floor
                              (else
                               (ecs:set-glyph f "." '(80 75 70) #f #f #f))))
                          (ecs:add-tag f "Floor")

                          ;; Mark first open floor as candidate spawn position
                          (if (not spawn-pos)
                              (set! spawn-pos (list x y))
                              #f)

                          ;; Sparse liquid pools in natural cave depressions
                          (let ((liquid-roll (modulo (+ (* x 13) (* y 47) actual-seed) 200)))
                            (cond
                              ((= liquid-roll 0) (chemistry:spawn-liquid x y "acid"))
                              ((< liquid-roll 4) (chemistry:spawn-liquid x y "water"))
                              ((= liquid-roll 5) (chemistry:spawn-liquid x y "oil"))
                              (else #f)))

                          ;; Sparse cave monsters
                          (let ((mob-roll (modulo (+ (* x 71) (* y 53) actual-seed) 150)))
                            (cond
                              ((= mob-roll 0)
                               (let ((bat (ecs:spawn "cave_bat")))
                                 (ecs:set-pos bat x y LAYER-CREATURE)
                                 (ecs:set-glyph bat "b" '(160 120 90) #f #t #f)
                                 (ecs:set bat "hp" 25)
                                 (ecs:set bat "max_hp" 25)
                                 (ecs:set bat "name" "洞窟蝙蝠")
                                 (ecs:add-tag bat "Monster")
                                 (set! monsters-spawned (cons bat monsters-spawned))))
                              ((= mob-roll 1)
                               (let ((trog (ecs:spawn "troglodyte")))
                                 (ecs:set-pos trog x y LAYER-CREATURE)
                                 (ecs:set-glyph trog "t" '(220 80 80) #f #t #f)
                                 (ecs:set trog "hp" 60)
                                 (ecs:set trog "max_hp" 60)
                                 (ecs:set trog "name" "穴居食腐者")
                                 (ecs:add-tag trog "Monster")
                                 ;; Give troglodyte physical limbs
                                 (anatomy:build-humanoid-body trog)
                                 (set! monsters-spawned (cons trog monsters-spawned))))
                              (else #f)))))
                    (loop-x (+ x 1) (cdr cols)))))
            (loop-y (+ y 1) (cdr rows)))))

    (hash
      "zone" "caves"
      "spawn_pos" (if spawn-pos spawn-pos (list 1 1))
      "monsters" monsters-spawned)))

;; ----------------------------------------------------------------------------
;; 2. Salt Marsh (类 Qud 盐沼地表与残垣)
;; ----------------------------------------------------------------------------

(define (worldgen:generate-salt-marsh width height seed)
  (let* ((actual-seed (if seed seed 101))
         (spawn-pos #f)
         (monsters-spawned '()))

    (let loop-y ((y 0))
      (if (>= y height)
          #t
          (begin
            (let loop-x ((x 0))
              (if (>= x width)
                  #t
                  (let* ((nx (* (exact->inexact x) 0.08))
                         (ny (* (exact->inexact y) 0.08))
                         (noise-val (ecs:noise-2d actual-seed nx ny 1.0)))
                    (cond
                      ;; Ruined ancient obelisks / stone pillars
                      ((> noise-val 0.65)
                       (let ((p (ecs:spawn "salt_pillar")))
                         (ecs:set-pos p x y LAYER-TERRAIN)
                         (ecs:set-glyph p "I" '(210 200 180) '(50 45 40) #t #t)
                         (ecs:add-tag p "Pillar")))

                      ;; Brine & shallow salt marsh water
                      ((< noise-val -0.35)
                       (let ((f (ecs:spawn "marsh_shallows")))
                         (ecs:set-pos f x y LAYER-TERRAIN)
                         (ecs:set-glyph f "~" '(100 180 230) '(20 40 60) #f #f)
                         (ecs:add-tag f "WaterFloor")
                         (chemistry:spawn-liquid x y "water")))

                      ;; Salt reeds & marsh vegetation
                      ((and (> noise-val 0.2) (< noise-val 0.45))
                       (let ((f (ecs:spawn "salt_reeds")))
                         (ecs:set-pos f x y LAYER-TERRAIN)
                         (ecs:set-glyph f "\"" '(150 200 90) #f #f #f)
                         (ecs:add-tag f "Floor")
                         (if (not spawn-pos) (set! spawn-pos (list x y)) #f)))

                      ;; White salt flats
                      (else
                       (let ((f (ecs:spawn "salt_flats")))
                         (ecs:set-pos f x y LAYER-TERRAIN)
                         (ecs:set-glyph f "." '(230 230 240) #f #f #f)
                         (ecs:add-tag f "Floor")
                         (if (not spawn-pos) (set! spawn-pos (list x y)) #f))))

                    (loop-x (+ x 1)))))
            (loop-y (+ y 1)))))

    (hash
      "zone" "salt_marsh"
      "spawn_pos" (if spawn-pos spawn-pos (list 1 1))
      "monsters" monsters-spawned)))

;; ----------------------------------------------------------------------------
;; Unified Worldgen Dispatcher
;; ----------------------------------------------------------------------------

(define (worldgen:generate-dungeon zone-type width height seed)
  (let ((z (to-string zone-type)))
    (cond
      ((string=? z "caves") (worldgen:generate-cave-dungeon width height seed))
      ((string=? z "salt_marsh") (worldgen:generate-salt-marsh width height seed))
      (else (worldgen:generate-cave-dungeon width height seed)))))
