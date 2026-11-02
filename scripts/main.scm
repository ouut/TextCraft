;; ============================================================================
;; TextCraft 3.0 - Main Game Logic (scripts/main.scm)
;; World Initialization, Dynamic FPS Regulation, Collision, Combat, and UI Actions
;; ============================================================================

;; ----------------------------------------------------------------------------
;; Activity & FPS Management
;; ----------------------------------------------------------------------------

(define (touch-activity!)
  (set! *world-state*
    (hash-insert
      (hash-insert *world-state* "idle_timer" 0.0)
      "target_fps" 60)))

(define (set-target-fps! fps)
  (set! *world-state* (hash-insert *world-state* "target_fps" fps)))

;; ----------------------------------------------------------------------------
;; Inventory Helpers
;; ----------------------------------------------------------------------------

(define (player:find-item inv item-id)
  (let loop ((items inv))
    (if (null? items)
        #f
        (let ((it (car items)))
          (if (equal? (map-get it "id" "") item-id)
              it
              (loop (cdr items)))))))

(define (player:add-item pid item-id name icon count)
  (let* ((p (ensure-player pid))
         (inv (map-get p "inventory" '()))
         (found #f)
         (new-inv '()))
    (for-each
      (lambda (it)
        (if (equal? (map-get it "id" "") item-id)
            (begin
              (set! found #t)
              (let* ((cur-count (map-get it "count" 1))
                     (updated-it (hash-insert it "count" (+ cur-count count))))
                (set! new-inv (cons updated-it new-inv))))
            (set! new-inv (cons it new-inv))))
      inv)
    (if (not found)
        (set! new-inv (cons (hash "id" item-id "name" name "icon" icon "count" count) new-inv))
        #f)
    (set-player pid (hash-insert p "inventory" (reverse new-inv)))))

(define (player:consume-item pid item-id count)
  (let* ((p (ensure-player pid))
         (inv (map-get p "inventory" '()))
         (success #f)
         (new-inv '()))
    (for-each
      (lambda (it)
        (if (equal? (map-get it "id" "") item-id)
            (let ((cur-count (map-get it "count" 1)))
              (if (>= cur-count count)
                  (begin
                    (set! success #t)
                    (let ((rem (- cur-count count)))
                      (if (> rem 0)
                          (set! new-inv (cons (hash-insert it "count" rem) new-inv))
                          #f)))
                  (set! new-inv (cons it new-inv))))
            (set! new-inv (cons it new-inv))))
      inv)
    (if success
        (begin
          (set-player pid (hash-insert p "inventory" (reverse new-inv)))
          #t)
        #f)))

;; ----------------------------------------------------------------------------
;; World Initialization
;; ----------------------------------------------------------------------------

(define (init-demo-world)
  (world:clear!)

  ;; 1. Terrain: Central outpost surrounded by stone walls (-10 to 10, -7 to 7)
  (let loop-y ((y -9))
    (if (<= y 9)
        (begin
          (let loop-x ((x -14))
            (if (<= x 14)
                (begin
                  (cond
                    ;; Outpost boundaries (with door openings at x=0)
                    ((and (or (= x -10) (= x 10)) (and (>= y -6) (<= y 6)))
                     (world:set-tile x y 'terrain
                       (hash "glyph" "#" "fg" '(180 180 190) "bg" '(40 40 45) "solid" #t "name" "stone_wall")))
                    ((and (or (= y -7) (= y 7)) (and (>= x -10) (<= x 10)) (not (= x 0)))
                     (world:set-tile x y 'terrain
                       (hash "glyph" "#" "fg" '(180 180 190) "bg" '(40 40 45) "solid" #t "name" "stone_wall")))
                    ;; Outpost interior: Wood floor
                    ((and (> x -10) (< x 10) (> y -7) (< y 7))
                     (world:set-tile x y 'terrain
                       (hash "glyph" "." "fg" '(160 120 70) "bg" '(32 24 16) "solid" #f "name" "wood_floor")))
                    ;; Exterior: Grass
                    (else
                     (world:set-tile x y 'terrain
                       (hash "glyph" "\"" "fg" '(70 170 70) "bg" '(18 38 18) "solid" #f "name" "grass"))))
                  (loop-x (+ x 1)))
                #f))
          (loop-y (+ y 1)))
        #f))

  ;; 2. Exterior features: Trees
  (let ((trees '((-12 . -5) (-13 . 3) (-12 . 7) (12 . -5) (13 . 4) (12 . 8) (-5 . -9) (5 . -9) (-6 . 9) (6 . 9))))
    (for-each
      (lambda (pair)
        (world:set-tile (car pair) (cdr pair) 'terrain
          (hash "glyph" "T" "fg" '(40 190 50) "bg" '(18 38 18) "solid" #t "name" "tree")))
      trees))

  ;; 3. Exterior features: Water pond to the east
  (let loop-wy ((wy -2))
    (if (<= wy 2)
        (begin
          (let loop-wx ((wx 13))
            (if (<= wx 17)
                (begin
                  (world:set-tile wx wy 'terrain
                    (hash "glyph" "~" "fg" '(90 170 255) "bg" '(25 45 90) "solid" #t "name" "water"))
                  (loop-wx (+ wx 1)))
                #f))
          (loop-wy (+ wy 1)))
        #f))

  ;; 4. Surface: Chests, Gold, Potions, Workbench
  ;; Chest 1 with potion
  (world:set-tile 6 -4 'surface
    (hash "glyph" "C" "fg" '(240 190 50) "bg" #f "solid" #t "name" "treasure_chest"
          "meta" (hash "loot" "potion" "opened" #f)))
  ;; Chest 2 with gold
  (world:set-tile -6 -4 'surface
    (hash "glyph" "C" "fg" '(240 190 50) "bg" #f "solid" #t "name" "treasure_chest"
          "meta" (hash "loot" "gold" "amount" 50 "opened" #f)))

  ;; Gold piles on ground
  (world:set-tile 4 4 'surface
    (hash "glyph" "$" "fg" '(255 215 0) "bg" #f "solid" #f "name" "gold_coins" "meta" (hash "amount" 25)))
  (world:set-tile -4 4 'surface
    (hash "glyph" "$" "fg" '(255 215 0) "bg" #f "solid" #f "name" "gold_coins" "meta" (hash "amount" 35)))
  (world:set-tile 8 -2 'surface
    (hash "glyph" "$" "fg" '(255 215 0) "bg" #f "solid" #f "name" "gold_coins" "meta" (hash "amount" 40)))

  ;; Health Potions on surface
  (world:set-tile 2 -2 'surface
    (hash "glyph" "!" "fg" '(240 60 60) "bg" #f "solid" #f "name" "health_potion" "meta" (hash "heal" 25)))
  (world:set-tile -2 -2 'surface
    (hash "glyph" "!" "fg" '(240 60 60) "bg" #f "solid" #f "name" "health_potion" "meta" (hash "heal" 25)))

  ;; Workbench
  (world:set-tile 0 -5 'surface
    (hash "glyph" "W" "fg" '(200 150 70) "bg" #f "solid" #t "name" "workbench" "meta" (hash "craft" "available")))

  ;; 5. NPCs and Monsters
  ;; Merchant NPC (friendly)
  (world:set-tile 2 -5 'actor
    (hash "glyph" "M" "fg" '(100 220 255) "bg" #f "solid" #t "name" "merchant"
          "meta" (hash "hp" 100 "dialog" "欢迎来到 TextCraft 3.0 前哨站！按 WASD 移动，点击背包使用物品。输入 (help) 查看指令。")))

  ;; Initialize monsters state
  (let ((m1 (hash "id" "goblin_1" "name" "哥布林" "glyph" "g" "fg" '(80 230 80)
                  "x" 7 "y" 3 "hp" 30 "max_hp" 30 "attack" 6))
        (m2 (hash "id" "orc_1" "name" "半兽人" "glyph" "O" "fg" '(240 100 80)
                  "x" -7 "y" 3 "hp" 60 "max_hp" 60 "attack" 12)))
    (set! *world-state* (hash-insert *world-state* "monsters" (list m1 m2)))
    (world:set-tile 7 3 'actor
      (hash "glyph" "g" "fg" '(80 230 80) "bg" #f "solid" #t "name" "goblin" "meta" m1))
    (world:set-tile -7 3 'actor
      (hash "glyph" "O" "fg" '(240 100 80) "bg" #f "solid" #t "name" "orc" "meta" m2)))

  'ok)

;; ----------------------------------------------------------------------------
;; Collision & Interaction
;; ----------------------------------------------------------------------------

(define (is-solid? tile)
  (if tile
      (map-get tile "solid" #f)
      #f))

(define (check-wall-collision x y)
  (let ((t (world:get-tile x y 'terrain)))
    (if (is-solid? t)
        t
        #f)))

;; ----------------------------------------------------------------------------
;; Standard Specification Lifecycle & Engine Hooks
;; ----------------------------------------------------------------------------

(define (engine:target-fps)
  (let ((fps (map-get *world-state* "target_fps" 60)))
    (if (number? fps)
        fps
        60)))

(define (engine:viewport-size pid)
  (let ((p (get-player pid)))
    (if p
        (list (map-get p "cols" 80) (map-get p "rows" 24))
        '(80 24))))

(define (engine:hud-config)
  (hash
    "theme" "cyberpunk"
    "layout" "surround"))

(define (engine:on-init)
  (init-demo-world)
  (touch-activity!)
  'ok)

(define (engine:on-player-join pid token)
  (let* ((tok-str (if (string? token) token (to-string token)))
         (parsed (parse-identity-token tok-str))
         (uname (map-get parsed "username" ""))
         (auth-tok (map-get parsed "auth_token" ""))
         (id-key (map-get parsed "identity_key" ""))
         (_ (evict-duplicate-identity! pid id-key))
         (saved (map-get *world-state* "saved_players" (hash)))
         (saved-p (if (not (string=? id-key ""))
                      (map-get saved id-key #f)
                      #f)))
    (if saved-p
        (let* ((restored-name (if (not (string=? uname ""))
                                  uname
                                  (map-get saved-p "name" (string-append "Hero_" (to-string pid)))))
               (restored-p
                 (hash-insert
                   (hash-insert
                     (hash-insert
                       (hash-insert
                         (hash-insert
                           (hash-insert saved-p "id" pid)
                           "token" tok-str)
                         "auth_token" auth-tok)
                       "identity_key" id-key)
                     "name" restored-name)
                   "online" #t)))
          (set-player pid restored-p)
          (touch-activity!)
          (player:add-log pid "system" "欢迎回来！已恢复你的角色存档进度。")
          'ok)
        (let ((p (make-default-player pid tok-str)))
          (set-player pid p)
          (touch-activity!)
          (player:add-log pid "system" "已成功连接至 TextCraft 3.0 纯 Scheme 虚拟世界！")
          'ok))))

(define (engine:on-player-leave pid)
  (remove-player pid)
  'ok)

(define (engine:on-tick dt tick-id)
  (let* ((old-ticks (map-get *world-state* "tick_count" 0))
         (old-elapsed (map-get *world-state* "elapsed_sec" 0.0))
         (old-idle (map-get *world-state* "idle_timer" 0.0))
         (new-elapsed (+ old-elapsed dt))
         (new-idle (+ old-idle dt)))
    (set! *world-state*
      (hash-insert
        (hash-insert
          (hash-insert *world-state* "tick_count" (+ old-ticks 1))
          "elapsed_sec" new-elapsed)
        "idle_timer" new-idle))

    ;; Dynamic FPS adaptation based on inactivity
    (cond
      ((> new-idle 5.0) (set-target-fps! 15))
      ((> new-idle 2.0) (set-target-fps! 30))
      (else #f))

    ;; Monster AI execution every ~30 ticks
    (if (= (modulo (+ old-ticks 1) 30) 0)
        (let* ((monsters (map-get *world-state* "monsters" '()))
               (players (map-get *world-state* "players" (hash)))
               (p-keys (hash-keys->list players)))
          (if (not (null? p-keys))
              (let ((new-monsters '()))
                (for-each
                  (lambda (m)
                    (let* ((mx (map-get m "x" 0))
                           (my (map-get m "y" 0))
                           (mhp (map-get m "hp" 0))
                           (mname (map-get m "name" "怪物"))
                           (mattack (map-get m "attack" 5))
                           (cur-players (map-get *world-state* "players" (hash)))
                           (cur-p-keys (hash-keys->list cur-players))
                           ;; Find closest online player by Manhattan distance
                           (nearest-info
                             (let loop-pk ((ks cur-p-keys) (best-p #f) (best-dist 999999))
                               (if (null? ks)
                                   (cons best-p best-dist)
                                   (let ((cand-p (hash-ref cur-players (car ks))))
                                     (if (map-get cand-p "online" #t)
                                         (let* ((cx (map-get cand-p "x" 0))
                                                (cy (map-get cand-p "y" 0))
                                                (d (+ (abs (- mx cx)) (abs (- my cy)))))
                                           (if (< d best-dist)
                                               (loop-pk (cdr ks) cand-p d)
                                               (loop-pk (cdr ks) best-p best-dist)))
                                         (loop-pk (cdr ks) best-p best-dist))))))
                           (target-p (car nearest-info))
                           (dist (cdr nearest-info)))
                      (if (> mhp 0)
                          (if target-p
                              (let ((px (map-get target-p "x" 0))
                                    (py (map-get target-p "y" 0))
                                    (pid (map-get target-p "id" 1)))
                                (if (= dist 1)
                                    ;; Monster attacks closest player
                                    (let* ((cur-hp (map-get target-p "hp" 100))
                                           (next-hp (max 0 (- cur-hp mattack)))
                                           (p-damaged (hash-insert target-p "hp" next-hp)))
                                      (set-player pid p-damaged)
                                      (player:add-log pid "combat"
                                        (string-append mname " 对你发动攻击，造成 " (number->string mattack) " 点伤害！"))
                                      (touch-activity!)
                                      (set! new-monsters (cons m new-monsters)))
                                    ;; Monster stalks closest player along X or Y axis
                                    (let* ((dx (if (< mx px) 1 (if (> mx px) -1 0)))
                                           (dy (if (< my py) 1 (if (> my py) -1 0)))
                                           (is-tile-blocked?
                                             (lambda (tx ty)
                                               (or (check-wall-collision tx ty)
                                                   (world:get-tile tx ty 'surface)
                                                   (world:get-tile tx ty 'actor)
                                                   (let any-p ((ks cur-p-keys))
                                                     (if (null? ks)
                                                         #f
                                                         (let ((op (hash-ref cur-players (car ks))))
                                                           (if (and (map-get op "online" #t)
                                                                    (= (map-get op "x" 0) tx)
                                                                    (= (map-get op "y" 0) ty))
                                                               #t
                                                               (any-p (cdr ks)))))))))
                                           ;; Prefer X axis first; only move along Y axis if dx == 0 or X step is blocked
                                           (step-x (if (not (= dx 0)) (cons (+ mx dx) my) #f))
                                           (step-y (if (not (= dy 0)) (cons mx (+ my dy)) #f))
                                           (chosen-step
                                             (cond
                                               ((and step-x (not (is-tile-blocked? (car step-x) (cdr step-x))))
                                                step-x)
                                               ((and step-y (not (is-tile-blocked? (car step-y) (cdr step-y))))
                                                step-y)
                                               (else #f))))
                                      (if chosen-step
                                          (let ((target-x (car chosen-step))
                                                (target-y (cdr chosen-step)))
                                            (world:remove-tile mx my 'actor)
                                            (let ((m-updated (hash-insert (hash-insert m "x" target-x) "y" target-y)))
                                              (world:set-tile target-x target-y 'actor
                                                (hash "glyph" (map-get m "glyph" "m") "fg" (map-get m "fg" '(200 100 100))
                                                      "bg" #f "solid" #t "name" mname "meta" m-updated))
                                              (set! new-monsters (cons m-updated new-monsters))))
                                          (set! new-monsters (cons m new-monsters))))))
                              (set! new-monsters (cons m new-monsters)))
                          #f)))
                  monsters)
                (set! *world-state* (hash-insert *world-state* "monsters" (reverse new-monsters))))
              #f))
        #f))
  'ok)

(define (engine:on-input pid input-map)
  (touch-activity!)
  (let* ((p (ensure-player pid))
         (atype (map-get input-map "type" ""))
         (code (map-get input-map "code" "")))
    (if (or (equal? atype "key") (equal? atype "key_down"))
        (let ((dx 0) (dy 0) (is-move #f))
          (cond
            ((or (equal? code "KeyW") (equal? code "ArrowUp") (equal? code "w") (equal? code "k"))
             (set! dy -1) (set! is-move #t))
            ((or (equal? code "KeyS") (equal? code "ArrowDown") (equal? code "s") (equal? code "j"))
             (set! dy 1) (set! is-move #t))
            ((or (equal? code "KeyA") (equal? code "ArrowLeft") (equal? code "a") (equal? code "h"))
             (set! dx -1) (set! is-move #t))
            ((or (equal? code "KeyD") (equal? code "ArrowRight") (equal? code "d") (equal? code "l"))
             (set! dx 1) (set! is-move #t))
            ((equal? code "Space")
             (player:add-log pid "system" "你观察四周，警惕着周围的动静。")
             #f)
            (else #f))

          (if is-move
              (let* ((cur-x (map-get p "x" 0))
                     (cur-y (map-get p "y" 0))
                     (nx (+ cur-x dx))
                     (ny (+ cur-y dy))
                     (terrain-wall (check-wall-collision nx ny))
                     (actor-tile (world:get-tile nx ny 'actor))
                     (surface-tile (world:get-tile nx ny 'surface)))
                (cond
                  ;; 1. Blocked by solid terrain (stone wall, tree, water)
                  (terrain-wall
                   (let ((tname (map-get terrain-wall "name" "障碍物")))
                     (player:add-log pid "system" (string-append "前方被 " tname " 挡住了。"))))

                  ;; 2. Interaction with Actor (Monster or NPC)
                  (actor-tile
                   (let ((aname (map-get actor-tile "name" "actor"))
                         (ameta (map-get actor-tile "meta" (hash))))
                     (cond
                       ;; Attack monster
                       ((or (equal? aname "goblin") (equal? aname "orc"))
                        (let* ((mhp (map-get ameta "hp" 30))
                               (mname (map-get ameta "name" aname))
                               (dmg 15)
                               (next-mhp (- mhp dmg)))
                          (if (<= next-mhp 0)
                              ;; Monster killed
                              (begin
                                (world:remove-tile nx ny 'actor)
                                ;; Drop gold
                                (world:set-tile nx ny 'surface
                                  (hash "glyph" "$" "fg" '(255 215 0) "bg" #f "solid" #f
                                        "name" "gold_coins" "meta" (hash "amount" 35)))
                                ;; Update monsters in world state
                                (let* ((ms (map-get *world-state* "monsters" '()))
                                       (filtered
                                         (let filter-ms ((l ms))
                                           (if (null? l)
                                               '()
                                               (if (and (= (map-get (car l) "x" 0) nx)
                                                        (= (map-get (car l) "y" 0) ny))
                                                   (filter-ms (cdr l))
                                                   (cons (car l) (filter-ms (cdr l))))))))
                                  (set! *world-state* (hash-insert *world-state* "monsters" filtered)))
                                (player:add-log pid "combat"
                                  (string-append "你发动强力斩击，斩杀了 " mname "！地上掉落了金币！")))
                              ;; Monster wounded
                              (let* ((updated-meta (hash-insert ameta "hp" next-mhp))
                                     (updated-tile (hash-insert actor-tile "meta" updated-meta)))
                                (world:set-tile nx ny 'actor updated-tile)
                                ;; Update monsters list
                                (let* ((ms (map-get *world-state* "monsters" '()))
                                       (updated-ms
                                         (let update-ms ((l ms))
                                           (if (null? l)
                                               '()
                                               (let ((m (car l)))
                                                 (if (and (= (map-get m "x" 0) nx)
                                                          (= (map-get m "y" 0) ny))
                                                     (cons (hash-insert m "hp" next-mhp) (update-ms (cdr l)))
                                                     (cons m (update-ms (cdr l)))))))))
                                  (set! *world-state* (hash-insert *world-state* "monsters" updated-ms)))
                                (player:add-log pid "combat"
                                  (string-append "你攻击了 " mname "，造成 " (number->string dmg)
                                                 " 点伤害！(剩余HP: " (number->string next-mhp) ")"))))))

                       ;; Talk to Merchant NPC
                       ((equal? aname "merchant")
                        (let ((dialog-text (map-get ameta "dialog" "你好，旅行者！"))
                              (dialog-obj (hash "title" "神秘商人"
                                                "text" (map-get ameta "dialog" "你好，旅行者！")
                                                "options" (list (hash "text" "购买生命药水 (25G)" "action" "buy_potion")
                                                                (hash "text" "离开" "action" "close_dialog")))))
                          (set-player pid (hash-insert p "dialog" dialog-obj))
                          (set-target-fps! 30)
                          (player:add-log pid "system" (string-append "商人: " dialog-text))))

                       (else
                        (player:add-log pid "system" (string-append "遭遇了 " aname "。"))))))

                  ;; 3. Interaction with Solid Surface (e.g. Chest, Workbench)
                  ((and surface-tile (is-solid? surface-tile))
                   (let ((sname (map-get surface-tile "name" "object"))
                         (smeta (map-get surface-tile "meta" (hash))))
                     (cond
                       ((equal? sname "treasure_chest")
                        (let ((loot (map-get smeta "loot" "gold"))
                              (opened (map-get smeta "opened" #f)))
                          (if (not opened)
                              (begin
                                (world:set-tile nx ny 'surface
                                  (hash "glyph" "c" "fg" '(150 120 40) "bg" #f "solid" #f
                                        "name" "opened_chest" "meta" (hash "opened" #t)))
                                (cond
                                  ((equal? loot "potion")
                                   (player:add-item pid "potion" "生命药水" "🧪" 1)
                                   (player:add-log pid "system" "你打开了宝箱，获得了生命药水 x1！"))
                                  ((equal? loot "gold")
                                   (let* ((amt (map-get smeta "amount" 50))
                                          (p-gold (map-get p "gold" 0))
                                          (p2 (hash-insert p "gold" (+ p-gold amt))))
                                     (set-player pid p2)
                                     (player:add-log pid "system" (string-append "你打开了宝箱，获得了金币 +" (number->string amt) "！"))))
                                  (else #f)))
                              (player:add-log pid "system" "这个宝箱已经被开启过了。"))))

                       ((equal? sname "workbench")
                        (player:add-log pid "system" "工作台散发着微光，可用于合成进阶道具。"))

                       (else
                        (player:add-log pid "system" (string-append "前方的 " sname " 挡住了道路。"))))))

                  ;; 4. Walkable tile: Move player and collect floor pickups
                  (else
                   (let ((p2 (hash-insert (hash-insert p "x" nx) "y" ny)))
                     (set-player pid p2)
                     ;; Check pickups on the new tile surface
                     (if surface-tile
                         (let ((sname (map-get surface-tile "name" ""))
                               (smeta (map-get surface-tile "meta" (hash))))
                           (cond
                             ;; Gold coins pickup
                             ((equal? sname "gold_coins")
                              (let* ((amt (map-get smeta "amount" 20))
                                     (cur-g (map-get p2 "gold" 0))
                                     (p3 (hash-insert p2 "gold" (+ cur-g amt))))
                                (world:remove-tile nx ny 'surface)
                                (set-player pid p3)
                                (player:add-log pid "system" (string-append "捡起地上的金币 +" (number->string amt) "！"))))

                             ;; Potion pickup
                             ((equal? sname "health_potion")
                              (world:remove-tile nx ny 'surface)
                              (player:add-item pid "potion" "生命药水" "🧪" 1)
                              (player:add-log pid "system" "捡起地上的生命药水 x1！"))

                             (else #f)))
                         #f)))))
              #f))
        #f))
  'ok)

(define (engine:on-ui-action pid action-map)
  (touch-activity!)
  (let* ((p (ensure-player pid))
         (action (map-get action-map "action" ""))
         (item-id (map-get action-map "id" (map-get action-map "item_id" ""))))
    (cond
      ;; Use item from HUD inventory (e.g. potion)
      ((or (equal? action "use_item") (equal? action "consume"))
       (cond
         ((or (equal? item-id "potion") (equal? item-id "potion_hp"))
          (if (player:consume-item pid "potion" 1)
              (let* ((p-now (ensure-player pid))
                     (cur-hp (map-get p-now "hp" 100))
                     (max-hp (map-get p-now "max_hp" 100))
                     (healed (min max-hp (+ cur-hp 25)))
                     (p-healed (hash-insert p-now "hp" healed)))
                (set-player pid p-healed)
                (player:add-log pid "combat" (string-append "你使用了生命药水，恢复了 25 点生命值！(HP: " (number->string healed) "/" (number->string max-hp) ")")))
              (player:add-log pid "system" "背包中没有生命药水了！")))

         ((equal? item-id "sword")
          (player:add-log pid "combat" "你紧握手中的铁剑，全身充满了力量！"))

         (else
          (player:add-log pid "system" (string-append "使用了物品: " (to-string item-id))))))

      ;; Close dialog
      ((or (equal? action "close_dialog") (equal? action "dialog_close") (equal? action "dialog_confirm"))
       (set-player pid (hash-insert p "dialog" #f)))

      ;; Buy potion from merchant dialog
      ((equal? action "buy_potion")
       (let ((cur-gold (map-get p "gold" 0)))
         (if (>= cur-gold 25)
             (begin
               (set-player pid (hash-insert (hash-insert p "gold" (- cur-gold 25)) "dialog" #f))
               (player:add-item pid "potion" "生命药水" "🧪" 1)
               (player:add-log pid "system" "你花费 25 金币购买了生命药水 x1！"))
             (begin
               (set-player pid (hash-insert p "dialog" #f))
               (player:add-log pid "system" "金币不足（需要 25 金币）！")))))

      ;; Set HUD theme from client UI
      ((equal? action "set_theme")
       (let* ((payload (map-get action-map "payload" (hash)))
              (t (map-get action-map "theme" (if (hash? payload) (map-get payload "theme" "") ""))))
         (if (and (string? t) (not (string=? t "")))
             (begin
               (set-player pid (hash-insert p "theme" t))
               (player:add-log pid "system" (string-append "HUD 主题切换为: " t)))
             #f)))

      ;; Clear player logs from client UI
      ((equal? action "clear_logs")
       (set-player pid (hash-insert p "logs" '())))

      ;; Viewport resize from client
      ((equal? action "set_viewport")
       (let* ((cols (map-get action-map "cols" 80))
              (rows (map-get action-map "rows" 24))
              (p2 (hash-insert (hash-insert p "cols" cols) "rows" rows)))
         (set-player pid p2)))

      (else
       (player:add-log pid "system" (string-append "收到未知 UI 交互: " (to-string action))))))
  'ok)

(define (engine:on-command pid datum raw)
  (touch-activity!)
  (let* ((p (ensure-player pid))
         (raw-str (if (string? raw) raw (to-string raw))))
    (cond
      ((or (equal? datum '(help)) (equal? datum 'help) (equal? raw-str "help") (equal? raw-str "(help)"))
       (player:add-log pid "system" "可用指令: (help), (say \"文本\"), (where), (heal), (dw), (fps 60|30|15), (theme \"cyberpunk\")"))

      ((or (equal? datum '(dw)) (equal? datum 'dw) (equal? raw-str "dw") (equal? raw-str "(dw)")
           (equal? datum '(double-width)) (equal? datum 'double-width)
           (and (pair? datum) (or (equal? (car datum) 'dw) (equal? (car datum) 'double-width))))
       (let* ((arg (if (and (pair? datum) (pair? (cdr datum))) (cadr datum) 'toggle))
              (cur-dw (player:double-width? pid))
              (new-state (cond
                           ((equal? arg 'toggle) (not cur-dw))
                           ((or (equal? arg #t) (equal? arg 1) (equal? arg 'on)) #t)
                           (else #f))))
         (engine:set-double-width! new-state)
         (set-player pid (hash-insert p "double_width" new-state))
         (player:add-log pid "system"
           (if new-state
               "已开启【双宽字符模式】(1:1 正方形网格，横向扩展对齐)"
               "已切换为【单宽标准字符模式】(1:2 传统终端等宽格式)"))))

      ((or (equal? datum '(where)) (equal? datum 'where) (equal? raw-str "where") (equal? raw-str "(where)"))
       (let ((px (map-get p "x" 0))
             (py (map-get p "y" 0)))
         (player:add-log pid "system" (string-append "当前位置: (" (number->string px) ", " (number->string py) ")"))))

      ((or (equal? datum '(heal)) (equal? datum 'heal) (equal? raw-str "heal") (equal? raw-str "(heal)"))
       (let* ((max-hp (map-get p "max_hp" 100))
              (max-mp (map-get p "max_mp" 50))
              (p2 (hash-insert (hash-insert p "hp" max-hp) "mp" max-mp)))
         (set-player pid p2)
         (player:add-log pid "combat" "受到神秘祈福，HP与MP已完全恢复！")))

      ((and (pair? datum) (equal? (car datum) 'say) (pair? (cdr datum)))
       (let* ((txt (if (string? (cadr datum)) (cadr datum) (to-string (cadr datum))))
              (pname (map-get p "name" (string-append "Hero_" (to-string pid))))
              (msg (string-append pname " 说: " txt))
              (players (map-get *world-state* "players" (hash))))
         (for-each
           (lambda (k)
             (let ((op (hash-ref players k)))
               (if (map-get op "online" #t)
                   (let ((op-id (map-get op "id" k)))
                     (player:add-log op-id "chat" msg))
                   #f)))
           (hash-keys->list players))
         (engine:broadcast "chat" msg)))

      ((and (pair? datum) (equal? (car datum) 'fps) (pair? (cdr datum)))
       (let ((f (cadr datum)))
         (if (number? f)
             (begin
               (set-target-fps! f)
               (player:add-log pid "system" (string-append "目标帧率已设置为 " (number->string f) " FPS")))
             (player:add-log pid "system" "帧率必须为数字"))))

      ((and (pair? datum) (equal? (car datum) 'theme) (pair? (cdr datum)))
       (let* ((t (to-string (cadr datum)))
              (p2 (hash-insert p "theme" t)))
         (set-player pid p2)
         (player:add-log pid "system" (string-append "HUD 主题切换为: " t))))

      (else
       (player:add-log pid "system" (string-append "未识别指令: " raw-str ". 输入 (help) 获取帮助。")))))
  'ok)

(define (engine:render-snapshot pid)
  (let* ((vp (engine:viewport-size pid))
         (cols (car vp))
         (rows (cadr vp))
         (screen (render-viewport-ansi pid cols rows))
         (hud (build-player-hud pid)))
    (hash
      "screen" screen
      "cols" cols
      "rows" rows
      "hud" hud)))

(define (engine:export-state)
  *world-state*)

(define (engine:import-state data)
  (if (hash? data)
      (begin
        (set! *world-state* data)
        (bump-visual-version!)
        'ok)
      'error))
