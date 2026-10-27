;; Sel Core Library

; map :: (a -> b) -> [a] -> [b]
(define (map f l)
  (define (loop rem acc)
    (if (empty? rem)
        (reverse acc)
        (loop (cdr rem) (cons (f (car rem)) acc))))
  (loop l '()))

; filter :: (a -> Bool) -> [a] -> [a]
(define (filter f l)
  (define (loop rem acc)
    (if (empty? rem)
        (reverse acc)
        (let ((h (car rem)))
          (loop (cdr rem) (if (f h) (cons h acc) acc)))))
  (loop l '()))

; foldl :: (b -> a -> b) -> b -> [a] -> b
(define (foldl f acc l)
  (if (empty? l)
      acc
      (foldl f (f acc (car l)) (cdr l))))

; foldr :: (a -> b -> b) -> b -> [a] -> b
(define (foldr f acc l)
  (define (loop rem res)
    (if (empty? rem)
        res
        (loop (cdr rem) (f (car rem) res))))
  (loop (reverse l) acc))

; repeat :: (->) -> Int -> Nil
(define (repeat f n)
  (if (<= n 0)
      nil
      (begin (f) (repeat f (- n 1)))))

; force :: (-> a) -> a
(define (force promise)
  (promise))

; even? :: Int -> Bool
(define (even? x) (= (mod x 2) 0))

; odd? :: Int -> Bool
(define (odd? x) (!= (mod x 2) 0))

;; Monadic Error Values
(define (ok val) (list 'ok val))
(define (err msg) (list 'err msg))
(define (ok? x) (and (list? x) (not (empty? x)) (eq? (car x) 'ok)))
(define (err? x) (and (list? x) (not (empty? x)) (eq? (car x) 'err)))

(define (unwrap x)
  (if (ok? x)
      (car (cdr x))
      (error "Called unwrap on err value:" (car (cdr x)))))

(define (error-value x)
  (if (err? x)
      (car (cdr x))
      (error "Called error-value on ok value:" (car (cdr x)))))

(defmacro attempt (expr)
  (list 'try
        (list 'ok expr)
        (list 'catch 'e (list 'err 'e))))

(defmacro try-bind (val var body)
  (list 'let (list (list '_try_bind_res_ val))
        (list 'if (list 'err? '_try_bind_res_)
              '_try_bind_res_
              (list 'let (list (list var (list 'unwrap '_try_bind_res_)))
                    body))))

;; Mathematical Constants
(define pi 3.141592653589793)
(define tau 6.283185307179586)
(define e 2.718281828459045)

;; Filesystem Conveniences (Unified System Interface)
(define (fs-exists? path) (system 'exists? path))
(define (fs-read path) (system 'read path))
(define (fs-write path content) (system 'write path content))
(define (fs-list path) (system 'list path))
(define (fs-delete path) (system 'delete path))
