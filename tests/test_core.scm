;; tests/test_core.scm - Core Language, Interned Special Forms, Lists, Pipelines & TCO

;; 1. Conditionals: if, when, unless, cond, and, or
(assert (eq? (if #t 1 2) 1))
(assert (eq? (if #f 1 2) 2))
(assert (eq? (if #f 1) nil))

(define when-val 0)
(when #t (set! when-val 10) (set! when-val (+ when-val 5)))
(when #f (set! when-val 999))
(assert (= when-val 15))

(assert (= (unless #f 42 0) 42))
(assert (= (unless #t 42 0) 0))
(assert (nil? (unless #t 42)))

(define (classify-cond x)
  (cond
    ((< x 0) "negative")
    ((= x 0) :do "zero")
    ((< x 10) (define tag "small") tag)
    (else "large")))

(assert (eq? (classify-cond -5) "negative"))
(assert (eq? (classify-cond 0) "zero"))
(assert (eq? (classify-cond 7) "small"))
(assert (eq? (classify-cond 100) "large"))

(assert (eq? (and) #t))
(assert (eq? (and #t #t #t) #t))
(assert (eq? (and #t #f #t) #f))
(assert (eq? (or) #f))
(assert (eq? (or #f #f #t) #t))
(assert (eq? (or #f #f #f) #f))

;; 2. Bindings & Functions: define, :=, begin, let, let*, lambda, fn, \() shorthand
(assert (eq? (begin (define bx 10) (+ bx 5)) 15))
(assert (eq? (let ((y 20)) (+ y 10)) 30))
(assert (eq? (let* ((a 10) (b (+ a 5)) (c (* b 2))) c) 30))

(define fn-alias (fn (a b) (+ a b)))
(assert (= (fn-alias 12 30) 42))

shorthand-val := (|> (range 10) (map \(x) (+ x 1)) (foldr + 0))
(assert (= shorthand-val 55))

;; 3. Loops: while, until, repeat
(define w-cnt 0)
(while (< w-cnt 1000)
  (set! w-cnt (+ w-cnt 1)))
(assert (= w-cnt 1000))

(until (= w-cnt 0)
  (set! w-cnt (- w-cnt 1)))
(assert (= w-cnt 0))

(define rep-cnt 0)
(repeat (lambda () (set! rep-cnt (+ rep-cnt 1))) 5)
(assert (= rep-cnt 5))

;; 4. Quasiquote, unquote, unquote-splicing, delay & force
(define q-mid '(2 3))
(assert (eq? `(1 ~@q-mid ~(+ 2 2)) '(1 2 3 4)))

(define lazy-side-effect 0)
(define p (delay (begin (set! lazy-side-effect (+ lazy-side-effect 1)) 99)))
(assert (= lazy-side-effect 0))
(assert (= (force p) 99))
(assert (= lazy-side-effect 1))

;; 5. Native List Primitives & Core Combinators
(assert (= (count (list nil nil)) 2))
(assert (eq? (reverse '(1 2 3 4 5)) '(5 4 3 2 1)))
(assert (eq? (reverse '()) '()))
(assert (eq? (append '(1 2) '(3 4) '(5)) '(1 2 3 4 5)))
(assert (eq? (append '() '(1)) '(1)))
(assert (eq? (last '(10 20 30)) 30))
(assert (nil? (last '())))
(assert (eq? (range 5) '(0 1 2 3 4)))
(assert (eq? (range 6 2) '(2 3 4 5)))
(assert (eq? (range 10 0 3) '(0 3 6 9)))

(define nums '(1 2 3 4 5))
(assert (eq? (map (lambda (x) (* x x)) nums) '(1 4 9 16 25)))
(assert (eq? (filter even? nums) '(2 4)))
(assert (eq? (filter odd? nums) '(1 3 5)))
(assert (= (foldl + 0 nums) 15))
(assert (eq? (foldr cons '() nums) '(1 2 3 4 5)))

;; 6. Interned Pipelines: |> (thread-last) and -> (thread-first)
(assert (= (|> 42) 42))
(define (square x) (* x x))
(define (add-ten x) (+ x 10))
(define (sub a b) (- a b))

(assert (= (|> 5 square) 25))
(assert (= (|> 5 square add-ten) 35))
(assert (= (|> 10 (sub 30)) 20))
(assert (= (|> 5 (+ 5) (* 2)) 20))

(define pipe-res
  (|> nums
      (map \(x) (* x 2))
      (filter \(x) (> x 4))
      reverse))
(assert (eq? pipe-res '(10 8 6)))

(define rec-p {x 10 y 20})
(define rec-p2 (-> rec-p (assoc 'x 100) (assoc 'z 300)))
(assert (= (rget rec-p2 'x) 100))
(assert (= (rget rec-p2 'y) 20))
(assert (= (rget rec-p2 'z) 300))
(assert (= (-> 30 (sub 10)) 20))

;; 7. Tail Call Optimization (TCO): direct, mutual, let/begin, and core combinators
(define (sum-tco n acc)
  (if (= n 0)
      acc
      (sum-tco (- n 1) (+ n acc))))
(assert (= (sum-tco 50000 0) 1250025000))

(define (is-even-tco? n)
  (if (= n 0) #t (is-odd-tco? (- n 1))))
(define (is-odd-tco? n)
  (if (= n 0) #f (is-even-tco? (- n 1))))
(assert (eq? (is-even-tco? 10000) #t))
(assert (eq? (is-odd-tco? 10000) #f))

(define (sum-let-begin n acc)
  (if (= n 0)
      (begin acc)
      (let ((next-n (- n 1))
            (next-acc (+ n acc)))
        (sum-let-begin next-n next-acc))))
(assert (= (sum-let-begin 10000 0) 50005000))

;; Large list TCO on map/filter/foldr
(define large-list (range 5000))
(assert (= (count (map \(x) (+ x 1) large-list)) 5000))
(assert (= (count (filter even? large-list)) 2500))
(assert (= (foldr + 0 (range 1000)) 499500))
