;; tests/test_macros.scm - Macros, Compile-Time Evaluation & Interaction with Interned Forms

;; 1. Basic macro definition and expansion
(defmacro unless-local (test &body)
  (list 'if test 'nil (cons 'begin body)))

(define y 3)
(unless-local (> y 5)
  (set! y 10))
(assert (eq? y 10))

;; 2. Compile-time evaluation helper
(define (compile-time-fact n)
  (if (<= n 1)
      1
      (* n (compile-time-fact (- n 1)))))

(defmacro static-fact (n)
  (compile-time-fact n))

(assert (= (static-fact 5) 120))
(assert (= (static-fact 10) 3628800))

;; 3. Quasiquote macro with gensym
(defmacro when-let (binding &body)
  (define var (car binding))
  (define val (car (cdr binding)))
  `(let ((~var ~val))
     (when ~var
       ~@body)))

(define test-val nil)
(when-let (x 42)
  (set! test-val x))
(assert (= test-val 42))

(when-let (z #f)
  (set! test-val 999))
(assert (= test-val 42))

;; 4. Recursive macro expansion
(defmacro nested-add (n expr)
  (if (<= n 0)
      expr
      `(nested-add ~(- n 1) (+ 1 ~expr))))

(assert (= (nested-add 10 0) 10))

;; 5. Macros combined with interned pipelines (|> and ->) and defn
(defmacro double-add (a b)
  `(+ (* ~a 2) ~b))

(assert (= (|> 5 (double-add 10)) 25))
(assert (= (-> 5 (double-add 10)) 20))
