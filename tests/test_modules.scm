;; tests/test_modules.scm - Module System, Visibility, Aliases, Hierarchical Packages & Load

;; 1. Basic import and :as aliases
(import point)
(assert (eq? (point/new-point 1 2) {x 1 y 2}))

(import point :as p)
(assert (eq? (p/new-point 3 4) {x 3 y 4}))

(import (point :as pt))
(assert (eq? (pt/new-point 5 6) {x 5 y 6}))

(import (point pnt))
(assert (eq? (pnt/new-point 7 8) {x 7 y 8}))

;; 2. Visibility (:public and :private)
(import mod_example)
(assert (eq? mod_example/a 10))
(assert (eq? mod_example/d 40))
(assert (eq? mod_example/f 50))

(define test-private-b
  (lambda ()
    (try
      mod_example/b
      (catch err 'caught-private-b))))
(assert (eq? (test-private-b) 'caught-private-b))

(define test-private-c
  (lambda ()
    (try
      mod_example/c
      (catch err 'caught-private-c))))
(assert (eq? (test-private-c) 'caught-private-c))

;; 3. Hierarchical and package (mod.scm) imports + SEL_PATH
(import modules_test/math/geom)
(assert (= (geom/square 4) 16))
(assert (= (geom/hypot 3 4) 5.0))

(import modules_test/pkg)
(assert (eq? (pkg/greet "Alice") "Hello from pkg, Alice!"))

(import "modules_test/math/geom" :as g)
(assert (= (g/square 5) 25))

(import ("modules_test/pkg" :as pkg-alias))
(assert (eq? (pkg-alias/greet "Bob") "Hello from pkg, Bob!"))

(set-env! "SEL_PATH" "tests/modules_test")
(import math/geom :as mg)
(assert (= (mg/hypot 6 8) 10.0))

(import pkg :as mp)
(assert (eq? (mp/greet "World") "Hello from pkg, World!"))

;; 4. Dynamic file loading via (load ...)
(define load-res (load "helper_load.scm"))
(assert (eq? load-res 42))
(assert (eq? loaded-var "hello-from-helper"))
(assert (eq? (loaded-func 5) 50))

(define load-missing-caught #f)
(try
  (load "does_not_exist.scm")
  (catch err (set! load-missing-caught #t)))
(assert load-missing-caught)

(define load-type-err-caught #f)
(try
  (load 123)
  (catch err (set! load-type-err-caught #t)))
(assert load-type-err-caught)
