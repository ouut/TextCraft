;;; ============================================================================
;;; TextCraft 4.0 - Chemistry, Liquids & Thermodynamics System
;;; Inspired by Caves of Qud & Noita: Liquid Pools, Thermodynamic Reactions,
;;; Vision-Obscuring Steam, Flammable Oils, and Corrosive Acids.
;;; ============================================================================

;; ----------------------------------------------------------------------------
;; Liquid Pool Spawning & Properties
;; ----------------------------------------------------------------------------

(define (chemistry:spawn-liquid x y liquid-type)
  (let* ((type-str (to-string liquid-type))
         (eid (ecs:spawn (string-append type-str "_pool"))))
    (ecs:add-tag eid "Liquid")
    (ecs:set-pos eid x y LAYER-ITEM-LIQUID)
    (ecs:set eid "liquid_type" type-str)
    (ecs:set eid "volume" 100)
    (ecs:set eid "temp" 20) ; Ambient 20°C

    (cond
      ((string=? type-str "water")
       (ecs:set-glyph eid "~" '(50 160 255) '(10 30 70) #f #f)
       (ecs:set eid "flammable" #f)
       (ecs:set eid "boiling_pt" 100))

      ((string=? type-str "acid")
       (ecs:set-glyph eid "~" '(80 255 50) '(15 50 15) #f #f)
       (ecs:set eid "flammable" #f)
       (ecs:set eid "corrosive" #t)
       (ecs:set eid "damage_per_tick" 15))

      ((string=? type-str "oil")
       (ecs:set-glyph eid "~" '(200 150 30) '(45 35 10) #f #f)
       (ecs:set eid "flammable" #t)
       (ecs:set eid "ignition_temp" 60))

      ((string=? type-str "blood")
       (ecs:set-glyph eid "~" '(200 30 30) '(50 10 10) #f #f)
       (ecs:set eid "flammable" #f))

      ((string=? type-str "fire")
       (ecs:set-glyph eid "^" '(255 120 0) '(100 20 0) #f #f)
       (ecs:set eid "temp" 400)
       (ecs:set eid "duration" 5)
       (ecs:add-tag eid "Burning"))

      ((string=? type-str "steam")
       ;; Steam is vision-obscuring: opaque = #t
       (ecs:set-glyph eid "*" '(220 220 240) #f #f #t)
       (ecs:set eid "duration" 4)
       (ecs:set eid "temp" 105))

      (else
       (ecs:set-glyph eid "~" '(180 180 180) #f #f #f)))

    eid))

;; ----------------------------------------------------------------------------
;; Thermodynamic Reactions (燃烧、蒸发与酸蚀)
;; ----------------------------------------------------------------------------

(define (chemistry:ignite x y)
  (let ((liquids (ecs:at x y LAYER-ITEM-LIQUID))
        (ignited? #f))
    (for-each
      (lambda (lid)
        (let ((ltype (ecs:get lid "liquid_type" ""))
              (flammable? (ecs:get lid "flammable" #f)))
          (cond
            ;; Flammable oil catches fire immediately
            ((or flammable? (string=? ltype "oil"))
             (ecs:despawn lid)
             (chemistry:spawn-liquid x y "fire")
             (set! ignited? #t))

            ;; Water quenches fire and boils into sight-blocking steam
            ((string=? ltype "water")
             (ecs:despawn lid)
             (chemistry:spawn-liquid x y "steam")
             (set! ignited? #f)))))
      liquids)

    ;; If tile has no extinguishing water, spawn fire
    (if (and (null? liquids) (not ignited?))
        (begin
          (chemistry:spawn-liquid x y "fire")
          (set! ignited? #t))
        #f)
    ignited?))

(define (chemistry:pour-acid x y)
  (let ((pool (chemistry:spawn-liquid x y "acid"))
        (creatures (ecs:at x y LAYER-CREATURE)))
    ;; Immediately corrode any creatures or items on this tile
    (for-each
      (lambda (cid)
        (let ((hp (ecs:get cid "hp" 100)))
          (ecs:set cid "hp" (- hp 20))
          (chemistry:stain cid "acid")))
      creatures)
    pool))

(define (chemistry:stain creature-eid liquid-type)
  (if (ecs:exists? creature-eid)
      (let ((type-str (to-string liquid-type)))
        (ecs:set creature-eid "stained_with" type-str)
        (cond
          ((string=? type-str "oil")
           (ecs:add-tag creature-eid "CoveredInOil")
           (ecs:remove-tag creature-eid "CoveredInWater"))
          ((string=? type-str "water")
           (ecs:add-tag creature-eid "CoveredInWater")
           (ecs:remove-tag creature-eid "CoveredInOil")
           (ecs:remove-tag creature-eid "Burning"))
          ((string=? type-str "acid")
           (ecs:add-tag creature-eid "CoveredInAcid"))
          (else #f))
        #t)
      #f))

(define (chemistry:cleanse-stains creature-eid)
  (if (ecs:exists? creature-eid)
      (begin
        (ecs:set creature-eid "stained_with" #f)
        (ecs:remove-tag creature-eid "CoveredInOil")
        (ecs:remove-tag creature-eid "CoveredInWater")
        (ecs:remove-tag creature-eid "CoveredInAcid")
        #t)
      #f))

;; ----------------------------------------------------------------------------
;; Environmental Turn Tick (环境热力学与反应循环)
;; ----------------------------------------------------------------------------

(define (chemistry:tick-environment)
  (let ((all-liquids (ecs:query "Liquid")))
    (for-each
      (lambda (lid)
        (if (ecs:exists? lid)
            (let ((ltype (ecs:get lid "liquid_type" ""))
                  (pos (ecs:get-pos lid)))
              (if pos
                  (let ((lx (car pos))
                        (ly (cadr pos)))
                    (cond
                      ;; Fire decays or spreads to adjacent oil
                      ((string=? ltype "fire")
                       (let ((dur (ecs:get lid "duration" 1)))
                         (if (<= dur 1)
                             (ecs:despawn lid)
                             (begin
                               (ecs:set lid "duration" (- dur 1))
                               ;; Damage creatures standing in fire
                               (let ((creatures (ecs:at lx ly LAYER-CREATURE)))
                                 (for-each
                                   (lambda (cid)
                                     (let* ((hp (ecs:get cid "hp" 100))
                                            (is-oiled? (ecs:has-tag? cid "CoveredInOil"))
                                            (dmg (if is-oiled? 40 15)))
                                       (ecs:set cid "hp" (- hp dmg))
                                       (ecs:add-tag cid "Burning")))
                                   creatures))
                               ;; Spread fire to adjacent tiles if oil is present
                               (let ((adj (list (list (+ lx 1) ly)
                                                (list (- lx 1) ly)
                                                (list lx (+ ly 1))
                                                (list lx (- ly 1)))))
                                 (for-each
                                   (lambda (coord)
                                     (let* ((ax (car coord))
                                            (ay (cadr coord))
                                            (adj-liquids (ecs:at ax ay LAYER-ITEM-LIQUID)))
                                       (for-each
                                         (lambda (alid)
                                           (if (string=? (ecs:get alid "liquid_type" "") "oil")
                                               (chemistry:ignite ax ay)
                                               #f))
                                         adj-liquids)))
                                   adj))))))

                      ;; Steam dissipates over turns
                      ((string=? ltype "steam")
                       (let ((dur (ecs:get lid "duration" 1)))
                         (if (<= dur 1)
                             (ecs:despawn lid)
                             (ecs:set lid "duration" (- dur 1)))))

                      ;; Acid continuously damages entities on the tile
                      ((string=? ltype "acid")
                       (let ((creatures (ecs:at lx ly LAYER-CREATURE)))
                         (for-each
                           (lambda (cid)
                             (let ((hp (ecs:get cid "hp" 100)))
                               (ecs:set cid "hp" (- hp 10))))
                           creatures)))

                      (else #f)))
                  #f))
            #f))
      all-liquids)))
