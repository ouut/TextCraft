;; market_sim — non-grid ECS proof model.
;;
;; Purpose: prove end to end that a model built ONLY on the core ECS
;; container primitives (guide section 7.1) works: entities, dynamic-prop
;; component values, tags, generic relations, the seeded host PRNG,
;; snapshot/checkpoint rollback and run_epoch host assertions — with no
;; grid concepts anywhere and no state held outside the Flecs world.
;;
;; Domain: one small market.
;;
;; Entities (spawned in engine:on-init after ecs:clear!):
;;   market-hall   kind "market"    the singleton ledger/clock entity
;;   acct-alice    kind "account"   holder "alice", Balance in prop "balance"
;;   acct-bob      kind "account"   holder "bob",   Balance in prop "balance"
;;   merchant-mira kind "merchant"  prop "price" and cash box prop "till"
;;   stock-mira    kind "stock"     Inventory in prop "units"
;;   order-slot-N  kind "order"     a bounded order book: 8 slot entities
;;                                  spawned at init and REUSED by trades
;;                                  (props filled/qty/total/tick/buyer).
;;                                  Slots are reused instead of spawning
;;                                  one entity per trade on purpose: the
;;                                  host state hash covers raw entity
;;                                  ids, and entities spawned after a
;;                                  snapshot rollback receive fresh ids
;;                                  from the allocator, which would make
;;                                  a re-run diverge in ids despite an
;;                                  identical causal state. A fixed slot
;;                                  pool keeps post-rollback trajectories
;;                                  id-stable, so hashes match exactly.
;;
;; Component values: every causal datum is a DynamicProps entry written
;; with ecs:set and read with ecs:get. There is no model-side state hash:
;; engine:export-state below is a pure read projection that rebuilds the
;; report hash from ecs:query + ecs:get on every call, and
;; engine:import-state is a no-op (see its comment).
;;
;; Tags: "Market", "Account", "Merchant", "Stock", "Order", plus one
;; unique lookup tag per singleton ("AcctAlice", "AcctBob",
;; "MerchantMira", "StockMira") so any entity can be re-found with
;; ecs:query after a snapshot rollback remaps nothing we cached — the
;; model deliberately never caches entity ids in Scheme variables.
;;
;; Relations (all through the generic ecs:relate primitive, which accepts
;; any relation name — no built-in relation had to be borrowed):
;;   merchant-mira --Owns-->       stock-mira    (ownership of the stock)
;;   merchant-mira --TradesWith--> acct-alice    (serves this account)
;;   merchant-mira --TradesWith--> acct-bob
;;   order-slot-N  --PlacedBy-->   buyer account (rewired on each reuse)
;;   order-slot-N  --FilledBy-->   merchant-mira
;;   acct-*/merchant/stock --PartOf--> market-hall (market membership)
;;
;; Rules (deterministic: host seeded PRNG + tick drive only):
;;   each tick: tick_count += 1; draw buyer = random-int 0..1
;;   (0 = alice, 1 = bob), draw qty = random-int 1..3; if the stock has
;;   qty units and the buyer can pay qty*price + 1 fee, the trade runs:
;;   buyer balance -= cost + fee, merchant till += cost, market
;;   fees_collected += fee, stock units -= qty, an order entity spawns.
;;   input "KeyA"/"KeyB": alice/bob immediately buy a fixed qty of 2
;;   through the same execution path.

;; ---- lookup helpers (id-free: safe across snapshot rollback) -----------

(define (only-id tag)
  (let ((ids (ecs:query tag)))
    (if (pair? ids) (car ids) #f)))

(define (market-id)   (only-id "Market"))
(define (alice-id)    (only-id "AcctAlice"))
(define (bob-id)      (only-id "AcctBob"))
(define (merchant-id) (only-id "MerchantMira"))
(define (stock-id)    (only-id "StockMira"))

(define (order-slot-id n)
  (only-id (string-append "OrderSlot" (number->string n))))

;; ---- trade execution -----------------------------------------------------

;; Fill the next order-book slot (ring of 8) for a trade that has already
;; been counted in the market's "trades" prop. The slot's previous
;; PlacedBy pair is removed before wiring the new buyer, so a slot always
;; points at exactly one buyer; FilledBy always targets the merchant.
(define (fill-order-slot! buyer qty cost tick)
  (let* ((market (market-id))
         (n (ecs:get market "trades"))
         (slot (order-slot-id (+ 1 (modulo (- n 1) 8))))
         (prev-buyer (ecs:get slot "buyer_eid" #f)))
    (if prev-buyer
        (ecs:unrelate slot "PlacedBy" prev-buyer)
        #f)
    (ecs:set slot "filled" #t)
    (ecs:set slot "qty" qty)
    (ecs:set slot "total" cost)
    (ecs:set slot "tick" tick)
    (ecs:set slot "buyer" (ecs:get buyer "holder"))
    (ecs:set slot "buyer_eid" buyer)
    (ecs:relate slot "PlacedBy" buyer)
    (ecs:relate slot "FilledBy" (merchant-id))
    slot))

(define (execute-buy! buyer qty)
  (let* ((market (market-id))
         (merchant (merchant-id))
         (stock (stock-id))
         (price (ecs:get merchant "price"))
         (units (ecs:get stock "units"))
         (balance (ecs:get buyer "balance"))
         (cost (* qty price))
         (fee 1))
    (if (and (>= units qty) (>= balance (+ cost fee)))
        (begin
          (ecs:set buyer "balance" (- balance cost fee))
          (ecs:set merchant "till" (+ (ecs:get merchant "till") cost))
          (ecs:set stock "units" (- units qty))
          (ecs:set market "fees_collected"
                   (+ (ecs:get market "fees_collected") fee))
          (ecs:set market "trades" (+ (ecs:get market "trades") 1))
          (fill-order-slot! buyer qty cost (ecs:get market "tick_count"))
          #t)
        #f)))

;; ---- lifecycle hooks -------------------------------------------------------

(define (engine:on-init)
  (ecs:clear!)
  (let ((market (ecs:spawn-named "market-hall"))
        (alice (ecs:spawn-named "acct-alice"))
        (bob (ecs:spawn-named "acct-bob"))
        (merchant (ecs:spawn-named "merchant-mira"))
        (stock (ecs:spawn-named "stock-mira")))
    (ecs:add-tag market "Market")
    (ecs:set market "kind" "market")
    (ecs:set market "tick_count" 0)
    (ecs:set market "trades" 0)
    (ecs:set market "fees_collected" 0)

    (ecs:add-tag alice "Account")
    (ecs:add-tag alice "AcctAlice")
    (ecs:set alice "kind" "account")
    (ecs:set alice "holder" "alice")
    (ecs:set alice "balance" 1000)

    (ecs:add-tag bob "Account")
    (ecs:add-tag bob "AcctBob")
    (ecs:set bob "kind" "account")
    (ecs:set bob "holder" "bob")
    (ecs:set bob "balance" 500)

    (ecs:add-tag merchant "Merchant")
    (ecs:add-tag merchant "MerchantMira")
    (ecs:set merchant "kind" "merchant")
    (ecs:set merchant "name" "mira")
    (ecs:set merchant "price" 7)
    (ecs:set merchant "till" 0)

    (ecs:add-tag stock "Stock")
    (ecs:add-tag stock "StockMira")
    (ecs:set stock "kind" "stock")
    (ecs:set stock "units" 40)

    (ecs:relate merchant "Owns" stock)
    (ecs:relate merchant "TradesWith" alice)
    (ecs:relate merchant "TradesWith" bob)
    (ecs:relate alice "PartOf" market)
    (ecs:relate bob "PartOf" market)
    (ecs:relate merchant "PartOf" market)
    (ecs:relate stock "PartOf" market)

    ;; The bounded order book: 8 reusable slot entities.
    (let loop ((i 1))
      (if (<= i 8)
          (begin
            (let ((slot (ecs:spawn-named
                          (string-append "order-slot-" (number->string i)))))
              (ecs:add-tag slot "Order")
              (ecs:add-tag slot (string-append "OrderSlot" (number->string i)))
              (ecs:set slot "kind" "order")
              (ecs:set slot "filled" #f)
              (ecs:set slot "qty" 0)
              (ecs:set slot "total" 0)
              (ecs:set slot "tick" 0)
              (ecs:set slot "buyer" "")
              (ecs:relate slot "PartOf" market))
            (loop (+ i 1)))
          #f))

    ;; Mirror the clock into the host world state singleton (also ECS
    ;; DynamicProps): snapshot v3 reads its authoritative tick from
    ;; there. The market entity's own "tick_count" prop stays the
    ;; model's clock; both are written from the same value every tick.
    (state:set! "tick_count" 0)
    'ok))

(define (engine:on-tick dt tick-id)
  (let ((market (market-id)))
    (let ((new-tick (+ (ecs:get market "tick_count") 1)))
      (ecs:set market "tick_count" new-tick)
      (state:set! "tick_count" new-tick))
    (let ((buyer (if (= (engine:random-int 0 1) 0) (alice-id) (bob-id)))
          (qty (engine:random-int 1 3)))
      (execute-buy! buyer qty)))
  'ok)

(define (engine:on-input pid input-map)
  (if (hash? input-map)
      (let ((code (if (hash-contains? input-map "code")
                      (hash-ref input-map "code")
                      "")))
        (cond
          ((equal? code "KeyA") (execute-buy! (alice-id) 2))
          ((equal? code "KeyB") (execute-buy! (bob-id) 2))
          (else #f)))
      #f)
  'ok)

;; Read-only projection of the ECS world into the host report shape.
;; Nothing is stored here: every value is read live from component props.
(define (engine:export-state)
  (let ((market (market-id))
        (alice (alice-id))
        (bob (bob-id))
        (merchant (merchant-id))
        (stock (stock-id)))
    (hash
      "tick_count" (ecs:get market "tick_count")
      "trades" (ecs:get market "trades")
      "fees_collected" (ecs:get market "fees_collected")
      "accounts" (hash
                   "alice" (hash "balance" (ecs:get alice "balance"))
                   "bob"   (hash "balance" (ecs:get bob "balance")))
      "merchant" (hash
                   "name" (ecs:get merchant "name")
                   "price" (ecs:get merchant "price")
                   "till" (ecs:get merchant "till")
                   "inventory" (ecs:get stock "units")))))

;; No-op by design: snapshot restore is handled entirely host-side
;; (the Flecs world, the world state singleton and the PRNG), and this
;; model keeps no causal state outside the ECS, so there is nothing
;; model-side left to restore. The tick clock rides inside the market
;; entity's "tick_count" prop (mirrored to the singleton each tick).
(define (engine:import-state data)
  'ok)
