---
title: Standard & Core Library
description: Comprehensive reference for all native built-in functions (implemented in Rust) and Scheme standard library routines loaded inside the core runtime.
---

This document describes all functions, macros, and operators available globally in the `sel` Lisp runtime environment. They are divided into **Native Built-in Functions** (implemented directly in the Rust interpreter core) and the **Scheme Standard Library** (defined in `core.scm`).

---

## 1. Native Built-in Functions (Rust-Implemented)

These core primitives are registered directly inside the environment by the interpreter runtime.

### Arithmetic & Numeric Operators

| Function | Arity | Description | Example |
| :--- | :--- | :--- | :--- |
| `+` | Variadic | Sums all numeric arguments. | `(+ 1 2 3)` -> `6` |
| `-` | > 1 | Subtraction. If 1 argument is passed, returns its negation. | `(- 10 3)` -> `7`, `(- 5)` -> `-5` |
| `*` | Variadic | Multiplies all numeric arguments. | `(* 2 3 4)` -> `24` |
| `/` | > 2 | Performs division. Supports integers and floats. | `(/ 10 2)` -> `5`, `(/ 5.0 2.0)` -> `2.5` |
| `mod` | 2 | Computes the remainder of dividing the first argument by the second. | `(mod 10 3)` -> `1` |

### Equalities & Comparators

| Function | Arity | Description | Example |
| :--- | :--- | :--- | :--- |
| `eq?` | 2 | Checks identity equality. Evaluates symbols, nil, booleans, and numbers. | `(eq? 'a 'a)` -> `#t`, `(eq? 1 1)` -> `#t` |
| `=` | 2 | Numeric value equality check. | `(= 5 5.0)` -> `#t` |
| `!=` | 2 | Numeric value inequality check. | `(!= 5 10)` -> `#t` |
| `<` | 2 | Evaluates if the first number is strictly less than the second. | `(< 2 3)` -> `#t` |
| `>` | 2 | Evaluates if the first number is strictly greater than the second. | `(> 5 3)` -> `#t` |
| `<=` | 2 | Evaluates if the first number is less than or equal to the second. | `(<= 3 3)` -> `#t` |
| `>=` | 2 | Evaluates if the first number is greater than or equal to the second. | `(>= 5 2)` -> `#t` |
| `not` | 1 | Logically negates the boolean argument. | `(not #f)` -> `#t`, `(not nil)` -> `#t` |

### Type Query & Reflection

These predicates allow runtime type introspection. They all take **1 argument** and return a boolean (`#t` or `#f`).

- `(nil? x)`: Returns `#t` if `x` is `nil`.
- `(list? x)`: Returns `#t` if `x` is a sequence list.
- `(number? x)`: Returns `#t` if `x` is an integer or floating-point number.
- `(string? x)`: Returns `#t` if `x` is a string.
- `(string-contains? str substr)`: Returns `#t` if `str` contains substring `substr`, else `#f`.
- `(symbol? x)`: Returns `#t` if `x` is an interned symbol.
- `(function? x)`: Returns `#t` if `x` is a native function or compiled Scheme closure.
- `(record? x)`: Returns `#t` if `x` is a record mapping.
- `(char? x)`: Returns `#t` if `x` is a first-class character.
- `(type-of x)`: Evaluates `x` and returns its type name as an interned symbol:
  ```lisp
  (type-of 10)       ; 'integer
  (type-of "hello")  ; 'string
  (type-of {a 1})    ; 'record
  (type-of #\a)      ; 'char
  ```
- `(gensym [prefix])`: Generates a globally unique interned symbol with an optional prefix (defaults to `"g"`). Indispensable for generating non-colliding variable names inside macros:
  ```lisp
  (gensym)        ; 'g0
  (gensym "temp") ; 'temp1
  ```

### Character Conversions

These functions convert between character values and their corresponding integer Unicode scalar values.

- `(char->integer char)`: Returns the Unicode scalar value integer of the character `char`.
  ```lisp
  (char->integer #\a) ; 97
  ```
- `(integer->char int)`: Returns the first-class character corresponding to the Unicode scalar value integer `int`.
  ```lisp
  (integer->char 97)  ; #\a
  ```

### List Manipulation Primitives

- `(cons head tail)`: Prepend `head` to the front of the list `tail`. If `tail` is not a list, constructs a new list containing `(head tail)`.
  ```lisp
  (cons 1 '(2 3)) ; (1 2 3)
  ```
- `(car list)`: Returns the first element of `list`. Triggers an error if empty.
  ```lisp
  (car '(10 20 30)) ; 10
  ```
- `(cdr list)`: Returns a new list containing all elements of `list` except the first. If the list has only one element, returns `nil`.
  ```lisp
  (cdr '(10 20 30)) ; (20 30)
  ```
- `(drop n list)`: Drops the first `n` elements from `list` and returns the remaining sublist in $O(1)$ time without copying. If `n >= (count list)`, returns `nil`.
  ```lisp
  (drop 2 '(10 20 30 40)) ; (30 40)
  ```
- `(nth list index)`: Accesses the element at 0-indexed position `index` in `list`. Returns `nil` if out of bounds.
  ```lisp
  (nth '(a b c) 1) ; 'b
  ```
- `(count x)`: Returns the integer length of list `x`, string `x`, or `0` for `nil`.
  ```lisp
  (count "hello") ; 5
  (count '(1 2))  ; 2
  ```
- `(list &args)`: Constructs a new list containing the evaluated arguments.
  ```lisp
  (list 1 2 3) ; (1 2 3)
  ```
- `(empty? x)`: Evaluates to `#t` if `x` is `nil`, an empty list `()`, or an empty string `""`.
  ```lisp
  (empty? '()) ; #t
  ```
- `(reverse x)`: Reverses the elements of list `x` or characters of string `x` natively.
- `(append &lists)`: Concatenates zero or more lists (or strings) into a single list.
  ```lisp
  (append '(1 2) '(3 4) '(5)) ; (1 2 3 4 5)
  ```
- `(last x)`: Returns the final element of list `x` or final character of string `x` in $O(1)$ time (`nil` if empty).
- `(range end [start [step]])`: Fast native integer sequence generator:
  - `(range end)`: Sequence from `0` to `end - 1` with step `1`.
  - `(range end start)`: Sequence from `start` to `end - 1` with step `1`.
  - `(range end start step)`: Sequence from `start` to `end - 1` in increments of `step`.
  ```lisp
  (range 5)       ; (0 1 2 3 4)
  (range 5 2)     ; (2 3 4)
  (range 10 2 2)  ; (2 4 6 8)
  ```

### Record Primitives

Records (`{key val}`) are manipulated using these functional primitives. They return a new, updated record, maintaining persistent immutable semantics.

- `(rget record symbol)`: Retrieves the value associated with `symbol` key from `record`. Returns `nil` if missing.
  ```lisp
  (rget {a 10} 'a) ; 10
  ```
- `(rset record symbol value)` / `(assoc record symbol value)`: Returns a new copy of `record` with the key `symbol` bound to `value`.
  ```lisp
  (rset {a 1} 'b 2)  ; {a 1 b 2}
  (assoc {a 1} 'b 2) ; {a 1 b 2}
  ```
- `(rdel record symbol)` / `(dissoc record symbol)`: Returns a copy of `record` with the key `symbol` removed.
  ```lisp
  (rdel {a 1 b 2} 'a)   ; {b 2}
  (dissoc {a 1 b 2} 'a) ; {b 2}
  ```
- `(rkeys record)`: Returns a list containing all the symbol keys of the record.
  ```lisp
  (rkeys {a 1 b 2}) ; (a b)
  ```
- `(rvals record)`: Returns a list containing all the values inside the record.
  ```lisp
  (rvals {a 1 b 2}) ; (1 2)
  ```
- `(rcontains? record symbol)`: Evaluates to `#t` if `symbol` is a key present inside `record`.
  ```lisp
  (rcontains? {a 1} 'a) ; #t
  ```

### Unified OS & Filesystem Interface (Message Passing)

`sel` consolidates all operating system and host filesystem interactions into a single, unified message-passing interface: `(system message &args)`. The legacy `(file-system message &args)` primitive is retained as an alias forwarding directly to `system`.

- `(system message &args)`:
  - **Process & Environment**:
    - `(system 'args)`: Returns a list of strings representing the CLI arguments passed to the script.
    - `(system 'getenv key-str)`: Retrieves an OS environment variable by key string. Returns `nil` if missing.
    - `(system 'setenv key-str val-str)`: Sets an OS environment variable for the process.
    - `(system 'cwd)`: Returns the current working directory path as a string.
    - `(system 'time)`: Returns the current Unix epoch timestamp in milliseconds.
    - `(system 'sleep secs-int)`: Pauses process execution for `secs-int` seconds.
    - `(system 'sleep-ms ms-int)`: Pauses process execution for `ms-int` milliseconds.
    - `(system 'exit code-int)`: Immediately terminates the `sel` process returning `code-int` status.
  - **Filesystem Operations**:
    - `(system 'exists? path-str)`: Returns `#t` if a file or directory exists at `path-str`, `#f` otherwise.
    - `(system 'read path-str)`: Reads the entire contents of a file at `path-str` and returns it as a string.
    - `(system 'write path-str content-str)`: Writes `content-str` to a file at `path-str`. Returns `nil` if successful.
    - `(system 'list path-str)`: Returns a list of file/directory name strings located inside `path-str`.
    - `(system 'delete path-str)`: Deletes the file or directory (recursively) at `path-str`. Returns `nil`.
- `(file-system message &args)`:
  - Backwards-compatible alias for `(system message &args)`.

### Output & Logging

- `(display x)`: Prints the string representation of `x` to standard output without a newline.
- `(println x)`: Prints `x` followed by a newline.
- `(newline)`: Prints a single newline.
- `(error msg &args)`: Immediately halts evaluation and throws a runtime exception with a descriptive error message.

---

## 2. Interned Special Forms & Core Library (`core.scm`)

High-frequency control flow, pattern matching, comprehension, and pipeline constructs are **interned directly into the Rust parser and bytecode compiler** for zero macro-expansion overhead, while higher-order functional combinators live in `core.scm`.

### Logical Control & Assertions (Interned Special Forms)

- `(when test &body)`: If `test` is truthy, executes `body` expressions sequentially.
  ```lisp
  (when (= 1 1)
    (println "Math holds")
    (println "True!"))
  ```
- `(unless test false-branch [true-branch])`: If `test` is falsy, executes `false-branch`, otherwise `true-branch` (or `nil`).
  ```lisp
  (unless (= 1 2)
    (println "Inequal"))
  ```
- `(cond (test body...) ... [(else body...)])`: Standard Scheme multi-branch conditional selection. Evaluates each clause's `test` in order and executes the body of the first truthy clause. Supports `else` (or `:else`) as the catch-all clause and optional `:do` markers:
  ```lisp
  (cond
    ((= x 1) "One")
    ((= x 2) "Two")
    (else    "Other"))
  ```
- `(assert test [msg...])`: Interned assertion form. Evaluates `test`; if `#f` or `nil`, raises a runtime error at the caller's source location without evaluating `msg` on passing assertions.
  ```lisp
  (assert (= 2 2) "Math is broken!")
  ```

### Iteration Loops

- `(while test &body)`: Interned loop form that executes `body` continuously as long as `test` evaluates to truthy.
- `(until test &body)`: Interned loop form that executes `body` continuously until `test` evaluates to truthy.
- `(repeat f n)`: Tail-recursive helper in `core.scm` that executes a zero-argument function `f` `n` times.

### Tail-Recursive Functional List Combinators (`core.scm`)

All standard list combinators in `core.scm` are tail-recursive (TCO) and safe on large lists:

- `(map f l)`: Applies a one-argument function `f` to each element in list `l`, returning a new list of results.
  ```lisp
  (map \(x) (* x x) '(1 2 3)) ; (1 4 9)
  ```
- `(filter f l)`: Returns a new list containing elements from list `l` for which `(f element)` evaluates to truthy.
  ```lisp
  (filter even? '(1 2 3 4)) ; (2 4)
  ```
- `(foldl f acc l)`: Tail-recursive left-associative list fold (reduce).
  ```lisp
  (foldl + 0 '(1 2 3)) ; 6
  ```
- `(foldr f acc l)`: Tail-recursive right-associative list fold (reduce).
- `(even? x)` / `(odd? x)`: Returns `#t` if integer `x` is even or odd.

### Promises & Laziness

- `(delay expr...)`: *Interned Special Form*. Wraps `expr` inside a zero-argument lambda promise without runtime macro overhead.
- `(force promise)`: Forces evaluation of a lazy promise.
  ```lisp
  (define lazy-value (delay (+ 10 20))) ; Not evaluated yet
  (force lazy-value)                    ; Evaluates and returns 30
  ```

### Monadic Error Types (`core.scm`)

Allows functional error-handling patterns without stack-unwinding `try/catch` clauses. Monads return structures tagged with `'ok` or `'err`.

- `(ok val)`: Wraps a successful value into `(ok val)`.
- `(err msg)`: Wraps an error message into `(err msg)`.
- `(ok? x)`: Evaluates to `#t` if `x` is an ok container.
- `(err? x)`: Evaluates to `#t` if `x` is an error container.
- `(unwrap x)`: Extracts the value inside an ok container. Throws an exception if `x` is an error container.
- `(error-value x)`: Extracts the error message inside an err container.
- `(attempt expr)`: *Macro*. Wraps the evaluation of `expr` inside a try-catch, returning `(ok result)` if successful, or `(err exception)` if it failed.
- `(try-bind val var body)`: *Macro*. Evaluates monadic `val`; short-circuits on error or binds the unwrapped value to `var` and evaluates `body`.

### Syntactic Helpers (Interned Special Forms)

- `(ffi-func symbol return-type argument-types)`: Interned special form that wraps a raw FFI symbol pointer in a Scheme closure.
- `(match-lambda &clauses)`: Interned special form that creates an anonymous single-argument lambda that pattern-matches its argument against `clauses`:
  ```lisp
  (define describe-num
    (match-lambda
      (0 "zero")
      (1 "one")
      (n (+ n 100))))
  ```
- `(defn head &clauses)`: Interned special form that defines a named function using multi-clause pattern matching (Elixir/Erlang style).
- `(|> val &forms)` and `(-> val &forms)`: Interned thread-last and thread-first pipeline forms lowered directly during AST optimization.
- `(with [clauses] :do body ... [:else ...])`: Interned railway-oriented pattern-matching chain.
- `(for (pat seq) ... [:let (bindings)] [:when cond] :do body ...)`: Interned list comprehension lowered directly to tail-recursive folds and pattern matches.

### Filesystem Conveniences (`core.scm`)

Built-in wrappers over the unified `system` interface defined in `core.scm`:

- `(fs-exists? path)`: Evaluates to `#t` if `path` exists.
- `(fs-read path)`: Reads file contents as a string.
- `(fs-write path content)`: Writes string `content` to `path`.
- `(fs-list path)`: Lists directory contents.
- `(fs-delete path)`: Deletes file or directory.


