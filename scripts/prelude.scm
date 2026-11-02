;; ============================================================================
;; TextCraft 3.0 - Standard Prelude (scripts/prelude.scm)
;; Pure Scheme Chunked World Storage, Dynamic Viewport ANSI Rasterizer, and HUD
;; ============================================================================

(define *chunk-size* 16)
(define *esc* "\x1b;")
(define *ansi-reset* (string-append *esc* "[0m"))
(define *visual-version* 1)
(define (bump-visual-version!)
  (set! *visual-version* (+ *visual-version* 1)))

;; ----------------------------------------------------------------------------
;; Generic Helpers
;; ----------------------------------------------------------------------------

(define (map-get m k default-val)
  (if (and (hash? m) (hash-contains? m k))
      (hash-ref m k)
      default-val))

(define (world:normalize-layer layer)
  (cond
    ((symbol? layer) (symbol->string layer))
    ((string? layer) layer)
    (else (to-string layer))))

(define (coord->chunk-coord v)
  (inexact->exact (floor (/ v *chunk-size*))))

(define (chunk-key cx cy)
  (string-append (number->string cx) "," (number->string cy)))

(define (tile-key x y layer)
  (string-append (number->string x) "," (number->string y) "," (world:normalize-layer layer)))

(define (ansi-fg r g b)
  (string-append *esc* "[38;2;" (number->string r) ";" (number->string g) ";" (number->string b) "m"))

(define (ansi-bg r g b)
  (string-append *esc* "[48;2;" (number->string r) ";" (number->string g) ";" (number->string b) "m"))

;; ----------------------------------------------------------------------------
;; Double-width (Square Grid) Mode Support
;; ----------------------------------------------------------------------------

(define (engine:set-double-width! enabled?)
  (set! *world-state* (hash-insert *world-state* "double_width" (if enabled? #t #f)))
  (bump-visual-version!))

(define (engine:double-width?)
  (map-get *world-state* "double_width" #f))

(define (player:double-width? player-id)
  (let ((p (get-player player-id)))
    (if (and p (hash-contains? p "double_width"))
        (let ((v (hash-ref p "double_width")))
          (if (boolean? v) v (engine:double-width?)))
        (engine:double-width?))))

;; ----------------------------------------------------------------------------
;; Global World State (Pure Data - Zero Lambdas/Closures)
;; ----------------------------------------------------------------------------

(define *world-state*
  (hash
    "tick_count" 0
    "elapsed_sec" 0.0
    "target_fps" 60
    "idle_timer" 0.0
    "chunks" (hash)
    "players" (hash)
    "saved_players" (hash)
    "monsters" '()
    "timers" (hash)))

;; ----------------------------------------------------------------------------
;; 16x16 Chunked World Storage
;; ----------------------------------------------------------------------------

(define (world:ensure-chunks)
  (if (not (hash-contains? *world-state* "chunks"))
      (set! *world-state* (hash-insert *world-state* "chunks" (hash)))
      #f))

(define (world:set-tile x y layer tile-map)
  (world:ensure-chunks)
  (let* ((cx (coord->chunk-coord x))
         (cy (coord->chunk-coord y))
         (ckey (chunk-key cx cy))
         (tkey (tile-key x y layer))
         (chunks (map-get *world-state* "chunks" (hash)))
         (chunk (map-get chunks ckey (hash "tiles" (hash))))
         (tiles (map-get chunk "tiles" (hash)))
         (new-tiles (hash-insert tiles tkey tile-map))
         (new-chunk (hash-insert chunk "tiles" new-tiles))
         (new-chunks (hash-insert chunks ckey new-chunk)))
    (set! *world-state* (hash-insert *world-state* "chunks" new-chunks))
    (bump-visual-version!)
    tile-map))

(define (world:get-tile x y layer)
  (let* ((cx (coord->chunk-coord x))
         (cy (coord->chunk-coord y))
         (ckey (chunk-key cx cy))
         (chunks (map-get *world-state* "chunks" (hash)))
         (chunk (map-get chunks ckey #f)))
    (if chunk
        (let ((tiles (map-get chunk "tiles" (hash))))
          (map-get tiles (tile-key x y layer) #f))
        #f)))

(define (world:remove-tile x y layer)
  (let* ((cx (coord->chunk-coord x))
         (cy (coord->chunk-coord y))
         (ckey (chunk-key cx cy))
         (tkey (tile-key x y layer))
         (chunks (map-get *world-state* "chunks" (hash)))
         (chunk (map-get chunks ckey #f)))
    (if chunk
        (let ((tiles (map-get chunk "tiles" (hash))))
          (if (hash-contains? tiles tkey)
              (let* ((new-tiles (hash-remove tiles tkey))
                     (new-chunk (hash-insert chunk "tiles" new-tiles))
                     (new-chunks (hash-insert chunks ckey new-chunk)))
                (set! *world-state* (hash-insert *world-state* "chunks" new-chunks))
                (bump-visual-version!)
                #t)
              #f))
        #f)))

(define (world:clear!)
  (set! *world-state*
    (hash-insert
      (hash-insert *world-state* "chunks" (hash))
      "monsters" '()))
  (bump-visual-version!))

;; ----------------------------------------------------------------------------
;; Player State Helpers
;; ----------------------------------------------------------------------------

(define (make-default-player player-id token)
  (hash
    "id" player-id
    "token" (if (string? token) token (to-string token))
    "name" (string-append "Hero#" (to-string player-id))
    "theme" "cyberpunk"
    "double_width" #f
    "x" 0
    "y" 0
    "cols" 80
    "rows" 24
    "hp" 100
    "max_hp" 100
    "mp" 50
    "max_mp" 50
    "level" 1
    "gold" 50
    "inventory" (list
                  (hash "id" "sword" "name" "铁剑" "icon" "⚔️" "count" 1)
                  (hash "id" "potion" "name" "生命药水" "icon" "🧪" "count" 3))
    "dialog" #f
    "logs" (list
             (hash "channel" "system" "text" "欢迎进入 TextCraft 3.0! (Scheme 驱动引擎)"))
    "last_action_tick" 0))

(define (get-player player-id)
  (let* ((pid-key (to-string player-id))
         (players (map-get *world-state* "players" (hash))))
    (map-get players pid-key #f)))

(define (ensure-player player-id)
  (let ((p (get-player player-id)))
    (if p
        p
        (let ((new-p (make-default-player player-id "")))
          (set-player player-id new-p)
          new-p))))

(define (set-player player-id player-map)
  (let* ((pid-key (to-string player-id))
         (players (map-get *world-state* "players" (hash)))
         (tok (map-get player-map "token" ""))
         (cleaned-players
           (if (and (string? tok) (not (string=? tok "")))
               (let ((res players))
                 (for-each
                   (lambda (k)
                     (if (not (string=? k pid-key))
                         (let ((op (map-get res k #f)))
                           (if (and op (equal? (map-get op "token" "") tok))
                               (set! res (hash-remove res k))
                               #f))
                         #f))
                   (hash-keys->list players))
                 res)
               players))
         (new-players (hash-insert cleaned-players pid-key player-map))
         (ws1 (hash-insert *world-state* "players" new-players)))
    (if (and (string? tok) (not (string=? tok "")))
        (let* ((saved (map-get ws1 "saved_players" (hash)))
               (new-saved (hash-insert saved tok player-map)))
          (set! *world-state* (hash-insert ws1 "saved_players" new-saved)))
        (set! *world-state* ws1))
    (bump-visual-version!)))

(define (remove-player player-id)
  (let* ((pid-key (to-string player-id))
         (players (map-get *world-state* "players" (hash)))
         (cur-p (map-get players pid-key #f))
         (ws0 (if cur-p
                  (let ((tok (map-get cur-p "token" "")))
                    (if (and (string? tok) (not (string=? tok "")))
                        (let* ((saved (map-get *world-state* "saved_players" (hash)))
                               (new-saved (hash-insert saved tok cur-p)))
                          (hash-insert *world-state* "saved_players" new-saved))
                        *world-state*))
                  *world-state*)))
    (set! *world-state*
      (hash-insert ws0 "players"
        (hash-remove players pid-key)))
    (bump-visual-version!)))

(define (player:add-log player-id channel text)
  (let* ((p (ensure-player player-id))
         (old-logs (map-get p "logs" '()))
         (new-log (hash "channel" channel "text" text))
         (all-logs (append old-logs (list new-log)))
         (trimmed (if (> (length all-logs) 30)
                      (list-tail all-logs (- (length all-logs) 30))
                      all-logs))
         (p2 (hash-insert p "logs" trimmed)))
    (set-player player-id p2)))

;; ----------------------------------------------------------------------------
;; HUD Builder
;; ----------------------------------------------------------------------------

(define (build-player-hud player-id)
  (let ((p (ensure-player player-id)))
    (hash
      "theme" (map-get p "theme" "cyberpunk")
      "hp" (map-get p "hp" 100)
      "max_hp" (map-get p "max_hp" 100)
      "mp" (map-get p "mp" 50)
      "max_mp" (map-get p "max_mp" 50)
      "level" (map-get p "level" 1)
      "gold" (map-get p "gold" 0)
      "inventory" (map-get p "inventory" '())
      "dialog" (map-get p "dialog" #f)
      "logs" (map-get p "logs" '()))))

;; ----------------------------------------------------------------------------
;; Dynamic Viewport ANSI Rasterizer
;; ----------------------------------------------------------------------------

(define (resolve-cell-parts x y tiles is-self self-tile other-player is-dw)
  (let* ((base-xy (string-append (number->string x) "," (number->string y) ","))
         (overlay-tile (if tiles (map-get tiles (string-append base-xy "overlay") #f) #f))
         (actor-tile   (if tiles (map-get tiles (string-append base-xy "actor") #f) #f))
         (surface-tile (if tiles (map-get tiles (string-append base-xy "surface") #f) #f))
         (terrain-tile (if tiles (map-get tiles (string-append base-xy "terrain") #f) #f))
         ;; Determine top visible tile for glyph and foreground
         (top-tile (cond
                     (overlay-tile overlay-tile)
                     (is-self self-tile)
                     (other-player other-player)
                     (actor-tile actor-tile)
                     (surface-tile surface-tile)
                     (terrain-tile terrain-tile)
                     (else #f)))
         ;; Determine background color: top-tile bg or fallback to terrain bg
         (bg (if top-tile
                 (let ((t-bg (map-get top-tile "bg" #f)))
                   (if t-bg
                       t-bg
                       (if terrain-tile (map-get terrain-tile "bg" #f) #f)))
                 (if terrain-tile (map-get terrain-tile "bg" #f) #f))))
    (if (not top-tile)
        (list (if is-dw "  " " ") "" "")
        (let* ((glyph (map-get top-tile "glyph" " "))
               (raw-glyph-str (if (char? glyph) (string glyph) (if (string? glyph) glyph (to-string glyph))))
               (glyph-str (if (and is-dw (<= (string-length raw-glyph-str) 1))
                              (string-append raw-glyph-str " ")
                              raw-glyph-str))
               (fg (map-get top-tile "fg" #f))
               (fg-str (if (and (list? fg) (= (length fg) 3))
                           (ansi-fg (car fg) (cadr fg) (caddr fg))
                           ""))
               (bg-str (if (and (list? bg) (= (length bg) 3))
                           (ansi-bg (car bg) (cadr bg) (caddr bg))
                           "")))
          (list glyph-str fg-str bg-str)))))

(define (resolve-cell-ansi x y tiles is-self self-tile other-player is-dw)
  (let* ((parts (resolve-cell-parts x y tiles is-self self-tile other-player is-dw))
         (glyph-str (car parts))
         (fg-str (cadr parts))
         (bg-str (caddr parts)))
    (if (and (string=? fg-str "") (string=? bg-str ""))
        glyph-str
        (string-append fg-str bg-str glyph-str *ansi-reset*))))

(define (render-viewport-ansi player-id cols rows)
  (let* ((p (get-player player-id))
         (px (if p (map-get p "x" 0) 0))
         (py (if p (map-get p "y" 0) 0))
         (is-dw (or (engine:double-width?) (player:double-width? player-id)))
         (grid-cols (if is-dw (quotient cols 2) cols))
         (half-w (quotient grid-cols 2))
         (half-h (quotient rows 2))
         (left (- px half-w))
         (top (- py half-h))
         (self-tile (hash "glyph" "@" "fg" '(255 255 100) "bg" #f "solid" #t "name" "hero"))
         (chunks (map-get *world-state* "chunks" (hash)))
         (players (map-get *world-state* "players" (hash)))
         (cur-pid-key (to-string player-id))
         ;; Pre-index other players by coordinate for O(1) lookup
         (other-player-map
           (let ((res (hash)))
             (for-each
               (lambda (k)
                 (if (not (string=? k cur-pid-key))
                     (let* ((op (hash-ref players k))
                            (ox (map-get op "x" 0))
                            (oy (map-get op "y" 0))
                            (okey (string-append (number->string ox) "," (number->string oy)))
                            (otile (hash "glyph" "@" "fg" '(100 200 255) "bg" #f "solid" #t "name" (map-get op "name" "player"))))
                       (set! res (hash-insert res okey otile)))
                     #f))
               (hash-keys->list players))
             res)))
    (let loop-row ((r 0) (row-acc '()))
      (if (< r rows)
          (let* ((y (+ top r))
                 (y-str (number->string y))
                 (chk-y (coord->chunk-coord y))
                 (row-str
                   (let loop-col ((c 0)
                                  (last-chk-x #f)
                                  (last-tiles #f)
                                  (cur-fg "")
                                  (cur-bg "")
                                  (col-acc '()))
                     (if (< c grid-cols)
                         (let* ((x (+ left c))
                                (chk-x (coord->chunk-coord x))
                                (tiles (if (and last-chk-x (= chk-x last-chk-x))
                                           last-tiles
                                           (let ((chunk (map-get chunks (chunk-key chk-x chk-y) #f)))
                                             (if chunk (map-get chunk "tiles" (hash)) #f))))
                                (is-self (and (= x px) (= y py)))
                                (other-p
                                  (if is-self
                                      #f
                                      (map-get other-player-map (string-append (number->string x) "," y-str) #f)))
                                (parts (resolve-cell-parts x y tiles is-self self-tile other-p is-dw))
                                (glyph-str (car parts))
                                (fg-str (cadr parts))
                                (bg-str (caddr parts))
                                (same-color? (and (string=? fg-str cur-fg) (string=? bg-str cur-bg)))
                                (cell-out
                                  (if same-color?
                                      glyph-str
                                      (let* ((had-color? (not (and (string=? cur-fg "") (string=? cur-bg ""))))
                                             (has-color? (not (and (string=? fg-str "") (string=? bg-str ""))))
                                             (need-reset? (or (and (not (string=? cur-fg "")) (string=? fg-str ""))
                                                              (and (not (string=? cur-bg "")) (string=? bg-str ""))))
                                             (prefix (if (and had-color? (or (not has-color?) need-reset?))
                                                         *ansi-reset*
                                                         ""))
                                             (eff-fg (if (and (not need-reset?) (string=? fg-str cur-fg)) "" fg-str))
                                             (eff-bg (if (and (not need-reset?) (string=? bg-str cur-bg)) "" bg-str)))
                                        (string-append prefix eff-fg eff-bg glyph-str)))))
                           (loop-col (+ c 1) chk-x tiles fg-str bg-str (cons cell-out col-acc)))
                         (let* ((end-reset (if (and (string=? cur-fg "") (string=? cur-bg ""))
                                               ""
                                               *ansi-reset*))
                                (final-cols (if (string=? end-reset "")
                                                col-acc
                                                (cons end-reset col-acc))))
                           (string-join (reverse final-cols) ""))))))
            (loop-row (+ r 1) (cons row-str row-acc)))
          (string-join (reverse row-acc) "\n")))))
