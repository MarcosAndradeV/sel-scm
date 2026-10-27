;; tests/test_system.scm - Unified System & OS Interface Tests

;; 1. Environment variables (via system and convenience functions)
(set-env! "SEL_TEST_VAR" "sel_value_123")
(assert (eq? (get-env "SEL_TEST_VAR") "sel_value_123"))
(assert (eq? (system 'getenv "SEL_TEST_VAR") "sel_value_123"))
(system 'setenv "SEL_TEST_VAR" "updated_456")
(assert (eq? (get-env "SEL_TEST_VAR") "updated_456"))
(assert (nil? (get-env "SEL_NON_EXISTENT_VAR_XYZ")))
(assert (nil? (system 'getenv "THIS_ENV_VAR_DOES_NOT_EXIST_12345")))

;; 2. OS Args and Current Working Directory
(assert (list? (system 'args)))
(assert (string? (system 'cwd)))
(assert (not (empty? (system 'cwd))))

;; 3. Time & Sleep primitives
(define t1 (time-now-ms))
(assert (> t1 1000000))
(sleep-ms 5)
(define t2 (system 'time))
(assert (>= t2 t1))

;; 4. Filesystem operations (via system, file-system alias, and fs-* helpers)
(define test-file "tests/temp_test_file.txt")
(when (fs-exists? test-file)
  (fs-delete test-file))

(assert (not (fs-exists? test-file)))
(assert (not (system 'exists? "tests/this_file_does_not_exist_404.txt")))

(fs-write test-file "Hello Sel Filesystem!")
(assert (fs-exists? test-file))
(assert (system 'exists? test-file))
(assert (file-system 'exists? test-file))
(assert (eq? (fs-read test-file) "Hello Sel Filesystem!"))
(assert (eq? (system 'read test-file) "Hello Sel Filesystem!"))

;; 5. Directory listing
(define entries (fs-list "tests"))
(assert (list? entries))
(assert (> (count entries) 0))

;; 6. Cleanup
(fs-delete test-file)
(assert (not (fs-exists? test-file)))
