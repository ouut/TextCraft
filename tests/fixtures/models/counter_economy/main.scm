;; counter_economy — non-grid fixture model (guide item 6).
;; A minimal economy simulation proving the base runs models that use NO
;; grid concepts: no ecs:* calls, no GridPos/Glyph/viewport. All state
;; lives in the host-managed ECS world-state singleton (snapshot v3),
;; accessed only through the state:* facade — this model previously
;; kept its own *world-state* hash (the dual-storage pattern removed
;; in the v3 migration).
;;
;; Rules: two accounts (alice, bob). Each tick every account earns
;; integer interest of balance/100 (floor). Input "KeyT" or the command
;; (transfer N) moves N from alice to bob.

(define (fresh-state)
  (hash
    "tick_count" 0
    "transfers" 0
    "accounts" (hash
                 "alice" (hash "balance" 1000)
                 "bob"   (hash "balance" 500))))

(state:seed-defaults! (fresh-state))

(define (engine:on-init)
  (state:replace! (fresh-state))
  'ok)

(define (balance-of name)
  (hash-ref (hash-ref (state:get "accounts") name) "balance"))

(define (set-balance! name new-balance)
  (let* ((accounts (state:get "accounts"))
         (acct (hash-ref accounts name))
         (new-acct (hash-insert acct "balance" new-balance))
         (new-accounts (hash-insert accounts name new-acct)))
    (state:set! "accounts" new-accounts)))

(define (do-transfer! amount)
  (let ((a (balance-of "alice"))
        (b (balance-of "bob")))
    (if (and (number? amount) (> amount 0) (>= a amount))
        (begin
          (set-balance! "alice" (- a amount))
          (set-balance! "bob" (+ b amount))
          (state:set! "transfers" (+ (state:get "transfers") 1))
          #t)
        #f)))

(define (engine:on-tick dt tick-id)
  (set-balance! "alice" (+ (balance-of "alice") (quotient (balance-of "alice") 100)))
  (set-balance! "bob"   (+ (balance-of "bob")   (quotient (balance-of "bob") 100)))
  (state:set! "tick_count" (+ (state:get "tick_count") 1))
  'ok)

(define (engine:on-input pid input-map)
  (if (and (hash? input-map)
           (hash-contains? input-map "code")
           (equal? (hash-ref input-map "code") "KeyT"))
      (do-transfer! 10)
      #f)
  'ok)

(define (engine:on-command pid datum raw)
  (if (and (list? datum)
           (pair? datum)
           (equal? (car datum) 'transfer)
           (pair? (cdr datum)))
      (do-transfer! (cadr datum))
      #f)
  'ok)

(define (engine:export-state)
  (state:all))

(define (engine:import-state data)
  (if (hash? data)
      (begin
        (state:replace! data)
        'ok)
      'error))
