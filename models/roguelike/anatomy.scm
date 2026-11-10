;;; ============================================================================
;;; TextCraft 4.0 - Anatomy & Equipment Tree System
;;; Inspired by Caves of Qud: Dynamic Limb Hierarchies, Equipment Slots,
;;; and Emergent Dismemberment Mechanics.
;;; ============================================================================

;; ----------------------------------------------------------------------------
;; Limb Constructors & Hierarchies
;; ----------------------------------------------------------------------------

(define (anatomy:create-limb type-name display-name hp max-hp)
  (let ((lid (ecs:spawn display-name)))
    (ecs:add-tag lid "Limb")
    (ecs:set lid "limb_type" (to-string type-name))
    (ecs:set lid "name" (to-string display-name))
    (ecs:set lid "hp" hp)
    (ecs:set lid "max_hp" max-hp)
    (ecs:set lid "severed" #f)
    lid))

(define (anatomy:add-limb parent-eid type-name display-name hp max-hp)
  (let ((lid (anatomy:create-limb type-name display-name hp max-hp)))
    (ecs:relate lid "PartOf" parent-eid)
    lid))

(define (anatomy:build-humanoid-body creature-eid)
  (let* ((torso (anatomy:add-limb creature-eid "torso" "躯干" 100 100))
         (head (anatomy:add-limb torso "head" "头部" 40 40))
         (arm-l (anatomy:add-limb torso "arm_left" "左臂" 35 35))
         (arm-r (anatomy:add-limb torso "arm_right" "右臂" 35 35))
         (hand-l (anatomy:add-limb arm-l "hand_left" "左手" 20 20))
         (hand-r (anatomy:add-limb arm-r "hand_right" "右手" 20 20))
         (leg-l (anatomy:add-limb torso "leg_left" "左腿" 40 40))
         (leg-r (anatomy:add-limb torso "leg_right" "右腿" 40 40))
         (foot-l (anatomy:add-limb leg-l "foot_left" "左脚" 20 20))
         (foot-r (anatomy:add-limb leg-r "foot_right" "右脚" 20 20)))
    (hash
      "torso" torso
      "head" head
      "arm_left" arm-l
      "arm_right" arm-r
      "hand_left" hand-l
      "hand_right" hand-r
      "leg_left" leg-l
      "leg_right" leg-r
      "foot_left" foot-l
      "foot_right" foot-r)))

;; ----------------------------------------------------------------------------
;; Limb Queries & Introspection
;; ----------------------------------------------------------------------------

(define (anatomy:get-child-limbs target-eid)
  (let ((children (ecs:children target-eid "PartOf")))
    (let loop ((curr children) (acc '()))
      (if (null? curr)
          acc
          (let* ((lid (car curr))
                 (sub (anatomy:get-child-limbs lid)))
            (loop (cdr curr) (append (cons lid sub) acc)))))))

(define (anatomy:get-all-limbs creature-eid)
  (anatomy:get-child-limbs creature-eid))

(define (anatomy:find-limb creature-eid type-name)
  (let ((limbs (anatomy:get-all-limbs creature-eid))
        (target-str (to-string type-name)))
    (let loop ((curr limbs))
      (if (null? curr)
          #f
          (let* ((lid (car curr))
                 (ltype (ecs:get lid "limb_type" ""))
                 (is-severed (ecs:get lid "severed" #f)))
            (if (and (string=? ltype target-str) (not is-severed))
                lid
                (loop (cdr curr))))))))

(define (anatomy:can-hold-weapon? creature-eid)
  (let ((limbs (anatomy:get-all-limbs creature-eid)))
    (let loop ((curr limbs))
      (if (null? curr)
          #f
          (let* ((lid (car curr))
                 (ltype (ecs:get lid "limb_type" ""))
                 (is-severed (ecs:get lid "severed" #f)))
            (if (and (or (string=? ltype "hand_left")
                         (string=? ltype "hand_right")
                         (string=? ltype "hand_extra"))
                     (not is-severed))
                #t
                (loop (cdr curr))))))))

(define (anatomy:count-usable-hands creature-eid)
  (let ((limbs (anatomy:get-all-limbs creature-eid)))
    (let loop ((curr limbs) (count 0))
      (if (null? curr)
          count
          (let* ((lid (car curr))
                 (ltype (ecs:get lid "limb_type" ""))
                 (is-severed (ecs:get lid "severed" #f)))
            (if (and (or (string=? ltype "hand_left")
                         (string=? ltype "hand_right")
                         (string=? ltype "hand_extra"))
                     (not is-severed))
                (loop (cdr curr) (+ count 1))
                (loop (cdr curr) count)))))))

;; ----------------------------------------------------------------------------
;; Equipment & Slots System
;; ----------------------------------------------------------------------------

(define (anatomy:equip limb-eid item-eid)
  (if (and (ecs:exists? limb-eid)
           (ecs:exists? item-eid)
           (not (ecs:get limb-eid "severed" #f)))
      (begin
        ;; Unequip any existing item on this limb first
        (let ((old-item (anatomy:get-equipped-item limb-eid)))
          (if old-item (anatomy:unequip old-item) #f))
        ;; Relate item EquippedOn limb
        (ecs:relate item-eid "EquippedOn" limb-eid)
        (ecs:set item-eid "equipped" #t)
        #t)
      #f))

(define (anatomy:unequip item-eid)
  (if (ecs:exists? item-eid)
      (let ((target-limb (ecs:target item-eid "EquippedOn")))
        (if target-limb
            (begin
              (ecs:unrelate item-eid "EquippedOn" target-limb)
              (ecs:set item-eid "equipped" #f)
              #t)
            #f))
      #f))

(define (anatomy:get-equipped-item limb-eid)
  (let ((items (ecs:children limb-eid "EquippedOn")))
    (if (null? items)
        #f
        (car items))))

(define (anatomy:get-all-equipped-items creature-eid)
  (let ((limbs (anatomy:get-all-limbs creature-eid)))
    (let loop ((curr limbs) (acc '()))
      (if (null? curr)
          acc
          (let* ((lid (car curr))
                 (item (anatomy:get-equipped-item lid)))
            (if item
                (loop (cdr curr) (cons item acc))
                (loop (cdr curr) acc)))))))

;; ----------------------------------------------------------------------------
;; Emergent Dismemberment (断肢与掉落系统)
;; ----------------------------------------------------------------------------

(define (anatomy:sever-limb creature-eid limb-eid)
  (if (or (not (ecs:exists? creature-eid))
          (not (ecs:exists? limb-eid))
          (ecs:get limb-eid "severed" #f))
      '()
      (let* ((pos (ecs:get-pos creature-eid))
             (px (if pos (car pos) 0))
             (py (if pos (cadr pos) 0))
             (parent (ecs:target limb-eid "PartOf"))
             (dropped-items '()))

        ;; 1. Mark severed
        (ecs:set limb-eid "severed" #t)

        ;; 2. Unrelate from parent
        (if parent (ecs:unrelate limb-eid "PartOf" parent) #f)

        ;; 3. Handle equipment on this limb: drop to world floor
        (let ((item (anatomy:get-equipped-item limb-eid)))
          (if item
              (begin
                (anatomy:unequip item)
                (ecs:set-pos item px py LAYER-ITEM-LIQUID)
                (set! dropped-items (cons item dropped-items)))
              #f))

        ;; 4. Recursively sever any sub-limbs attached to this limb
        ;; e.g. Severing an arm also severs the attached hand and drops its held weapon!
        (let ((sub-limbs (ecs:children limb-eid "PartOf")))
          (for-each
            (lambda (sub-id)
              (let ((sub-drops (anatomy:sever-limb creature-eid sub-id)))
                (set! dropped-items (append sub-drops dropped-items))))
            sub-limbs))

        ;; 5. The severed limb becomes a physical item entity in the world
        (ecs:set-pos limb-eid px py LAYER-ITEM-LIQUID)
        (ecs:set-glyph limb-eid "%" '(220 50 50) #f #f #f)
        (ecs:add-tag limb-eid "Item")
        (ecs:add-tag limb-eid "SeveredLimb")
        (let ((original-name (ecs:get limb-eid "name" "肢体")))
          (ecs:set limb-eid "name" (string-append "断落的" original-name)))

        ;; 6. Return list of all dropped physical entities
        (cons limb-eid dropped-items))))
