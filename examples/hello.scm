;; Hello World & Feature Showcase in sel Lisp

(println "Hello, World from sel Lisp!")

;; Infinite recursion safe with TCO
(define (countdown n)
  (if (= n 0)
      "Done!"
      (begin
        (println "Counting down:" n)
        (countdown (- n 1)))))

(println (countdown 3))

;; Modern record structure
(define user {name "Marcos" role "Developer"})
(println "User name:" (rget user 'name))

;; Thread-first pipeline
(define updated (-> user (assoc 'status "Active")))
(println "Updated status:" (rget updated 'status))

;; Unified system interface
(println "Current directory:" (system 'cwd))
