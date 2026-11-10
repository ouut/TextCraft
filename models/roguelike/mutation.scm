;;; ============================================================================
;;; TextCraft 4.0 - Mutation & Affix System
;;; Inspired by Caves of Qud: Physical & Mental Mutations, Cooldowns,
;;; and Dynamic Limb/Chemical Synergies.
;;; ============================================================================

(define *mutation-registry* (hash))

(define (mutation:register id name type cooldown-turns handler-proc)
  (let ((entry (hash
                 "id" (to-string id)
                 "name" (to-string name)
                 "type" (to-string type) ; "physical" or "mental"
                 "cooldown" cooldown-turns
                 "handler" handler-proc)))
    (set! *mutation-registry* (hash-insert *mutation-registry* (to-string id) entry))
    id))

;; ----------------------------------------------------------------------------
;; Built-in Mutations
;; ----------------------------------------------------------------------------

;; 1. MultipleArms (Physical): Adds extra arms and hands to the creature
(mutation:register
  "MultipleArms"
  "多臂异化"
  "physical"
  0 ; Passive
  (lambda (creature-eid target-x target-y)
    ;; Find torso
    (let ((torso (anatomy:find-limb creature-eid "torso")))
      (if torso
          (let* ((arm-extra (anatomy:add-limb torso "arm_extra" "变异臂" 30 30))
                 (hand-extra (anatomy:add-limb arm-extra "hand_extra" "变异手" 20 20)))
            (ecs:add-tag creature-eid "MutatedArms")
            (hash "status" "success" "message" "长出了一对附生变异手臂！"))
          (hash "status" "failed" "message" "找不到躯干肢体")))))

;; 2. FlamingHands (Physical): Shoots a burst of fire that ignites tiles
(mutation:register
  "FlamingHands"
  "灼热双掌"
  "physical"
  5 ; 5-turn cooldown
  (lambda (creature-eid target-x target-y)
    (let* ((pos (ecs:get-pos creature-eid))
           (px (if pos (car pos) 0))
           (py (if pos (cadr pos) 0))
           (path (ecs:find-path px py target-x target-y #f)))
      (if (and path (pair? path))
          (begin
            ;; Ignite each step along the path except caster's own cell
            (for-each
              (lambda (step)
                (let ((sx (car step))
                      (sy (cadr step)))
                  (if (not (and (= sx px) (= sy py)))
                      (chemistry:ignite sx sy)
                      #f)))
              path)
            (hash "status" "success" "message" "火焰自掌心汹涌喷薄而出！"))
          (begin
            (chemistry:ignite target-x target-y)
            (hash "status" "success" "message" "目标点爆发出剧烈烈焰！"))))))

;; 3. Teleportation (Mental): Instantly warps creature to target passable tile
(mutation:register
  "Teleportation"
  "折跃相位"
  "mental"
  10 ; 10-turn cooldown
  (lambda (creature-eid target-x target-y)
    (if (ecs:set-pos creature-eid target-x target-y LAYER-CREATURE)
        (hash "status" "success" "message" "空间折跃：你瞬间穿梭至目标坐标！")
        (hash "status" "failed" "message" "折跃失败：目标位置无法通行"))))

;; 4. CorrosiveSlime (Physical): Ejects an acid pool at target tile
(mutation:register
  "CorrosiveSlime"
  "强酸喷射"
  "physical"
  4 ; 4-turn cooldown
  (lambda (creature-eid target-x target-y)
    (chemistry:pour-acid target-x target-y)
    (hash "status" "success" "message" "喷吐出一滩冒泡的浓稠强酸！")))

;; ----------------------------------------------------------------------------
;; Mutation Lifecycle & Cooldown Tracking
;; ----------------------------------------------------------------------------

(define (mutation:grant creature-eid mutation-id)
  (let* ((mid (to-string mutation-id))
         (meta (hash-ref *mutation-registry* mid)))
    (if meta
        (let* ((mut-list (ecs:get creature-eid "mutations" '()))
               (has-already? (let loop ((m mut-list))
                               (cond ((null? m) #f)
                                     ((string=? (car m) mid) #t)
                                     (else (loop (cdr m)))))))
          (if (not has-already?)
              (begin
                (ecs:set creature-eid "mutations" (cons mid mut-list))
                ;; If it's an on-acquire passive (like MultipleArms), trigger once
                (if (= (hash-ref meta "cooldown") 0)
                    ((hash-ref meta "handler") creature-eid 0 0)
                    #f)
                #t)
              #f))
        #f)))

(define (mutation:has? creature-eid mutation-id)
  (let* ((mid (to-string mutation-id))
         (mut-list (ecs:get creature-eid "mutations" '())))
    (let loop ((m mut-list))
      (cond ((null? m) #f)
            ((string=? (car m) mid) #t)
            (else (loop (cdr m)))))))

(define (mutation:use creature-eid mutation-id target-x target-y)
  (if (not (mutation:has? creature-eid mutation-id))
      (hash "status" "failed" "message" "未觉醒此项异能")
      (let* ((mid (to-string mutation-id))
             (meta (hash-ref *mutation-registry* mid))
             (cd-map (ecs:get creature-eid "cooldowns" (hash)))
             (current-cd (if (hash? cd-map) (map-get cd-map mid #f) #f))
             (cd-turns (if current-cd current-cd 0)))
        (if (> cd-turns 0)
            (hash "status" "failed" "message" (string-append "技能正在冷却中 (剩余 " (to-string cd-turns) " 回合)"))
            (let* ((handler (hash-ref meta "handler"))
                   (res (handler creature-eid target-x target-y))
                   (base-cd (hash-ref meta "cooldown")))
              ;; Set cooldown
              (let ((new-cd-map (if (hash? cd-map) cd-map (hash))))
                (ecs:set creature-eid "cooldowns" (hash-insert new-cd-map mid base-cd)))
              res)))))

(define (mutation:tick-cooldowns creature-eid)
  (let ((cd-map (ecs:get creature-eid "cooldowns" (hash))))
    (if (hash? cd-map)
        (let* ((keys (hash-keys->list cd-map))
               (new-map (hash)))
          (for-each
            (lambda (k)
              (let* ((cd (map-get cd-map k 0))
                     (next-cd (max 0 (- cd 1))))
                (if (> next-cd 0)
                    (set! new-map (hash-insert new-map k next-cd))
                    #f)))
            keys)
          (ecs:set creature-eid "cooldowns" new-map)
          new-map)
        #f)))
