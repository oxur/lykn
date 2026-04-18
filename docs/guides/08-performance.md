# Performance

Measurement discipline, data structure choices, allocation patterns,
lazy evaluation, caching, and — critically — what NOT to micro-optimize.
lykn compiles to JavaScript, so all JS engine performance characteristics
apply. The most damaging performance anti-pattern is premature
optimization. This guide establishes "measure first" before any
optimization technique.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: Don't Optimize Without Measuring — Profile First

**Strength**: MUST

**Summary**: Never optimize code based on intuition. Measure first,
identify the bottleneck, then optimize only that path.

```lykn
;; Good — measure before optimizing
(bind start (performance:now))
(bind result (process-data large-data-set))
(bind elapsed (- (performance:now) start))
(console:log (template "processData: " (elapsed:toFixed 2) "ms"))
```

**The performance discipline**:
1. Write clear, idiomatic lykn code first
2. Measure with real workloads
3. Identify the actual bottleneck
4. Optimize only the bottleneck
5. Measure again to confirm

---

## ID-02: `Deno:bench` for Microbenchmarks

**Strength**: SHOULD

**Summary**: Use Deno's built-in benchmark runner for comparing
implementation alternatives.

```lykn
;; bench.lykn → compile to bench.js, run with: deno bench bench.js
(bind map-data (new Map #a(#a("a" 1) #a("b" 2) #a("c" 3))))
(bind obj-data (obj :a 1 :b 2 :c 3))

(Deno:bench "Map lookup" (fn () (map-data:get "b")))
(Deno:bench "Object lookup" (fn () obj-data:b))
```

---

## ID-03: `performance:now` for Timing Critical Sections

**Strength**: SHOULD

**Summary**: Use `performance:now` for wall-clock timing of specific
code paths.

```lykn
(async (func load-and-process :args (:string path) :returns :any :body
  (bind t0 (performance:now))
  (bind raw (await (Deno:readTextFile path)))
  (bind t1 (performance:now))
  (bind result (process-data raw))
  (bind t2 (performance:now))
  (console:log (template "Read: " ((- t1 t0):toFixed 2) "ms, Process: " ((- t2 t1):toFixed 2) "ms"))
  result))
```

---

## ID-04: Beware Microbenchmark Traps

**Strength**: SHOULD

**Summary**: JIT warmup, dead code elimination, and GC pauses can skew
results. Use `Deno:bench` which handles these automatically.

---

## ID-05: `Map` over `obj` for Dynamic Key Collections

**Strength**: SHOULD

**Summary**: `Map` provides O(1) lookup for any key type, no prototype
pollution, and a `:size` property.

```lykn
;; Good — Map for dynamic collection
(bind frequency (new Map))
(for-of word words
  (frequency:set word (+ (?? (frequency:get word) 0) 1)))
```

**See also**: `01-core-idioms.md` ID-16

---

## ID-06: `Set` over Arrays for Membership Testing

**Strength**: SHOULD

**Summary**: `Set:has` is O(1); `Array:includes` is O(n).

```lykn
;; Good — O(1) per lookup
(bind banned (new Set #a("spam" "phishing" "malware")))
(bind clean (items:filter (fn (:any item) (not (banned:has item:category)))))

;; Good — deduplication
(bind unique (array (spread (new Set items))))
```

---

## ID-07: `WeakMap` for Object-Keyed Caches

**Strength**: SHOULD

**Summary**: `WeakMap` lets entries be collected when the key has no
other references.

```lykn
(bind layout-cache (new WeakMap))

(func get-layout :args (:any element) :returns :any :body
  (if (layout-cache:has element) (layout-cache:get element)
    (block
      (bind layout (compute-expensive-layout element))
      (layout-cache:set element layout)
      layout)))
```

---

## ID-08: TypedArrays for Numeric/Binary Data

**Strength**: CONSIDER

**Summary**: TypedArrays are ~4x faster and ~8x less memory-efficient
than regular Arrays for numeric data.

```lykn
;; Good — contiguous, unboxed, zero-initialized
(bind data (new Float64Array 1000000))
(bind pixels (new Uint8ClampedArray (* width height 4)))
```

**See also**: `05-type-discipline.md` ID-27

---

## ID-09: Avoid Sparse Arrays — Holes Are Slower

**Strength**: SHOULD

**Summary**: Sparse arrays force engines into slower dictionary-mode
representations.

```lykn
;; Good — dense initialization
(bind a (Array:from (obj :length 1000) (fn () 0)))

;; Bad — creates holes
;; (bind a (new Array 1000))
```

---

## ID-10: `push`/`pop` Are O(1); `shift`/`unshift` Are O(n)

**Strength**: SHOULD

**Summary**: Work from the end of an array (stack pattern) when
possible. Front operations re-index every element.

```lykn
;; Good — stack: O(1) at both ends
(bind stack #a())
(stack:push item)
(stack:pop)
```

---

## ID-11: Pre-Allocate When Size Is Known

**Strength**: CONSIDER

**Summary**: When the output size is known, pre-allocate to avoid
repeated array resizing.

```lykn
;; Good — pre-allocate dense array
(bind result (Array:from (obj :length n) (fn (:any _v :number i) (compute i))))
```

---

## ID-12: `:flatMap` Over `:filter` + `:map` Chains

**Strength**: CONSIDER

**Summary**: `:flatMap` combines filter and transform in one pass.

```lykn
;; One pass — return #a() to filter, #a(value) to keep
(bind result (items:flatMap (fn (:any x)
  (if x:active #a(x:name) #a()))))
```

---

## ID-13: Short-Circuiting Methods — `:find` / `:some` over `:filter`

**Strength**: SHOULD

**Summary**: `:find`, `:some`, `:every` stop at the first match.
`:filter` scans the entire array.

```lykn
;; Good — stops at first match
(bind admin (users:find (fn (:any u) (= u:role "admin"))))
(bind has-errors (results:some (fn (:any r) (= r:status "error"))))
```

---

## ID-14: Generators for Lazy Sequences

**Strength**: SHOULD

**Summary**: Generators compute values one at a time. Combined with
`->>` threading, they enable lazy pipelines.

```lykn
;; Good — lazy: only computes what's consumed
(function* lazy-filter (pred iterable)
  (for-of x iterable (if (pred x) (yield x))))

(function* lazy-map (f iterable)
  (for-of x iterable (yield (f x))))

(function* take (n iterable)
  (for-of x iterable
    (if (<= n 0) (return))
    (yield x)
    (-- n)))
```

---

## ID-15: Iterator Helpers for Lazy Pipelines

**Strength**: CONSIDER

**Summary**: ES2025 iterator helpers add lazy `:map`, `:filter`,
`:take`, `:drop` to all iterators.

```lykn
;; Good — lazy pipeline, no intermediate arrays
(bind result
  (-> (naturals)
    (:filter (fn (:number n) (= (% n 2) 0)))
    (:map (fn (:number n) (* n n)))
    (:take 5)
    (:toArray)))
;; [0, 4, 16, 36, 64]
```

---

## ID-16: Iteration Form — Readability Wins

**Strength**: SHOULD

**Summary**: Choose the iteration form that best expresses intent.
Don't convert `:map` to `for` loops without profiling evidence.

```lykn
;; Good — intent is clear
(bind names (users:map (fn (:any u) u:name)))

;; Good — for-of for side effects
(for-of user users
  (if (not user:banned) (notify user)))
```

---

## ID-17: Early Exit with `break`/`return` in `for-of`

**Strength**: SHOULD

**Summary**: `for-of` supports `break` and early `return`. Array
methods like `:forEach` do not.

---

## ID-18: Keep Object Shapes Consistent

**Strength**: SHOULD

**Summary**: Initialize all properties in the same order every time.
lykn's `obj` naturally encourages this.

```lykn
;; Good — consistent shape: always the same keys
(func create-point
  :args (:number x :number y :number z)
  :returns :object
  :body (obj :x x :y y :z (?? z 0)))
```

---

## ID-19: Use `dissoc` Instead of `delete`

**Strength**: SHOULD

**Summary**: `delete` changes the object's shape, degrading
performance. `dissoc` creates a new object without the property.

```lykn
;; Good — new object without the property (non-destructive)
(bind clean (dissoc config :temp))

;; Bad — shape mutation via kernel delete
;; (delete config:temp)
```

**See also**: `04-values-references.md` ID-14

---

## ID-20: Cache Deep Property Lookups in `bind`

**Strength**: CONSIDER

**Summary**: In hot loops, cache deeply nested values in a `bind`.

```lykn
;; Good — cache before the loop
(bind transform config:pipeline:transform)
(for-of item items
  (result:push (transform item)))
```

---

## ID-21: `Object:keys` / `Object:entries` Allocate Arrays

**Strength**: CONSIDER

**Summary**: These methods create a new array on every call. Cache in
hot paths.

---

## ID-22: `template` vs `+` — Equivalent in Practice

**Strength**: SHOULD

**Summary**: No meaningful performance difference. Choose `template`
for readability.

```lykn
;; Both are fine — template is more readable
(bind msg (template "Hello, " name "! You have " count " items."))
```

---

## ID-23: Collect String Pieces in Array, `:join` at the End

**Strength**: SHOULD

**Summary**: For building large strings from many fragments, collect
in an array and `:join` once.

```lykn
(func build-csv :args (:array rows) :returns :string :body
  (bind lines (cell #a()))
  (for-of row rows
    (swap! lines (fn (:array l) (conj l (row:join ",")))))
  ((express lines):join "\n"))
```

---

## ID-24: Avoid Allocations in Hot Loops

**Strength**: SHOULD

**Summary**: Hoist object/array creation out of loops when the same
structure is reused.

```lykn
;; Good — regex hoisted, :test returns boolean (no allocation)
(bind error-pattern (regex "^ERROR:"))
(for-of line lines
  (if (error-pattern:test line) (++ error-count)))
```

---

## ID-25: `structuredClone` Is Not Free

**Strength**: SHOULD

**Summary**: Use `assoc` for flat data and `structuredClone` only when
nested independence is required.

```lykn
;; Good — shallow copy is sufficient for flat data
(bind copy (object (spread config)))

;; Good — deep copy only when nesting requires it
(bind independent (structuredClone deeply-nested))
```

---

## ID-26: Object Pooling for High-Churn Scenarios

**Strength**: CONSIDER

**Summary**: For objects created and discarded millions of times,
reuse from a pool. Only for extreme scenarios.

---

## ID-27: Memoize Expensive Pure Functions

**Strength**: SHOULD

**Summary**: Cache results of expensive computations keyed by arguments.

```lykn
(func memoize :args (:function f) :returns :function :body
  (bind cache (new Map))
  (fn (:any arg)
    (if (cache:has arg) (cache:get arg)
      (block
        (bind result (f arg))
        (cache:set arg result)
        result))))

(bind expensive-calc (memoize (fn (:number n)
  (bind result (cell 0))
  (for (let i 0) (< i n) (++ i)
    (swap! result (fn (:number r) (+ r (Math:sqrt i)))))
  (express result))))
```

---

## ID-28: Bounded Caches — LRU or Size-Limited

**Strength**: SHOULD

**Summary**: Unbounded caches grow forever. Implement a size limit.

```lykn
(func memoize-lru :args (:function f :number max-size) :returns :function :body
  (bind cache (new Map))
  (fn (:any arg)
    (if (cache:has arg)
      (block
        (bind value (cache:get arg))
        (cache:delete arg)
        (cache:set arg value)
        value)
      (block
        (if (>= cache:size max-size)
          (cache:delete (-> (cache:keys) (:next) :value)))
        (bind result (f arg))
        (cache:set arg result)
        result))))
```

---

## ID-29: `WeakMap` for Per-Object Memoization

**Strength**: SHOULD

**Summary**: When the cache key is an object, use `WeakMap` so entries
are collected when the key is GC'd.

```lykn
(bind style-cache (new WeakMap))

(func compute-styles :args (:any element) :returns :any :body
  (if (style-cache:has element) (style-cache:get element)
    (block
      (bind styles (derive-styles element))
      (style-cache:set element styles)
      styles)))
```

---

## ID-30: Named Exports Enable Tree Shaking

**Strength**: SHOULD

**Summary**: Named exports let bundlers eliminate unused code.

```lykn
;; Good — individually tree-shakeable
(export (func format-date :args (:any d) :returns :string :body (d:toISOString)))
(export (func parse-date :args (:string s) :returns :any :body (new Date s)))
```

**See also**: `01-core-idioms.md` ID-07, `02-api-design.md` ID-07

---

## ID-31: No Side Effects at Module Level

**Strength**: SHOULD

**Summary**: Keep module top-level code declaration-only.

**See also**: `02-api-design.md` ID-09

---

## ID-32: Don't Hand-Unroll Loops

**Strength**: MUST

**Summary**: The JIT compiler unrolls loops far better than hand-written
code. Don't sacrifice readability.

```lykn
;; Good — clear, idiomatic, JIT-friendly
(func sum :args (:array arr) :returns :number :body
  (bind total (cell 0))
  (for-of n arr (swap! total (fn (:number t) (+ t n))))
  (express total))
```

---

## ID-33: Don't Trade Clarity for Speed Without Evidence

**Strength**: MUST

**Summary**: Readability is the default priority. Never trade clarity
for hypothetical performance.

```lykn
;; Good — clear intent
(bind n (Math:floor (/ x y)))

;; Bad — bit tricks hide bugs and truncate to 32-bit
;; (bind n (| (/ x y) 0))
```

---

## ID-34: Don't Cache `array:length` in `for` Loops

**Strength**: SHOULD

**Summary**: Engines optimize `array:length` access. Manual caching
adds noise.

---

## ID-35: Proxies Have Overhead — Avoid in Hot Paths

**Strength**: CONSIDER

**Summary**: Proxy traps add overhead on every intercepted operation.
Validate once at the boundary, then access directly.

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | Profile first, optimize second | MUST | Don't optimize without measuring |
| 02 | `Deno:bench` for microbenchmarks | SHOULD | Handles warmup, sampling |
| 03 | `performance:now` for timing | SHOULD | Sub-millisecond precision |
| 04 | Microbenchmark traps | SHOULD | JIT warmup, dead code elimination |
| 05 | `Map` for dynamic collections | SHOULD | O(1), any key type |
| 06 | `Set` for membership testing | SHOULD | O(1) `:has` vs O(n) `:includes` |
| 07 | `WeakMap` for GC-safe caches | SHOULD | Auto-eviction on key collection |
| 08 | TypedArrays for numeric data | CONSIDER | ~4x faster, ~8x less memory |
| 09 | Avoid sparse arrays | SHOULD | Holes force slower engine paths |
| 10 | `push`/`pop` O(1) vs `shift` O(n) | SHOULD | Work from the end |
| 11 | Pre-allocate when size known | CONSIDER | Avoid incremental resizing |
| 12 | `:flatMap` over `:filter` + `:map` | CONSIDER | One pass, readable |
| 13 | Short-circuiting methods | SHOULD | `:find`/`:some` stop early |
| 14 | Generators for lazy evaluation | SHOULD | No intermediate arrays |
| 15 | Iterator helpers | CONSIDER | Lazy pipeline on any iterator |
| 16 | Readability wins | SHOULD | Don't convert `:map` to `for` without evidence |
| 17 | Early exit in `for-of` | SHOULD | `break`/`return` not possible in `:forEach` |
| 18 | Consistent object shapes | SHOULD | `obj` encourages this |
| 19 | `dissoc` instead of `delete` | SHOULD | Non-destructive, preserves shape |
| 20 | Cache deep lookups | CONSIDER | `bind` in hot loops |
| 21 | `Object:keys` allocates | CONSIDER | Cache in hot paths |
| 22 | `template` ≈ `+` | SHOULD | Readability first |
| 23 | Array + `:join` for large strings | SHOULD | One allocation |
| 24 | Hoist allocations out of hot loops | SHOULD | Regex, objects, spreads |
| 25 | `structuredClone` is not free | SHOULD | `assoc` for flat data |
| 26 | Object pooling | CONSIDER | Extreme high-churn only |
| 27 | Memoize pure functions | SHOULD | Closure + `Map` |
| 28 | Bounded caches (LRU) | SHOULD | Unbounded caches leak memory |
| 29 | `WeakMap` for object memoization | SHOULD | Auto-evicts on GC |
| 30 | Named exports for tree shaking | SHOULD | Individually removable |
| 31 | No side effects at module level | SHOULD | Defeats tree shaking |
| 32 | Don't hand-unroll loops | MUST | JIT does it better |
| 33 | Don't trade clarity for speed | MUST | Bit tricks hide bugs |
| 34 | Don't cache `array:length` | SHOULD | Engines optimize this |
| 35 | Proxies have overhead | CONSIDER | Validate once, access directly |

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for `Map`/`Set` (ID-16),
  `for-of` (ID-25), named exports (ID-07)
- **API Design**: See `02-api-design.md` for module design (ID-06-10),
  tree shaking (ID-07)
- **Values & References**: See `04-values-references.md` for
  `structuredClone` (ID-09), `assoc` (ID-06), `dissoc` (ID-14)
- **Type Discipline**: See `05-type-discipline.md` for TypedArrays
  (ID-27), `Map`/`Set` (ID-26)
- **Functions & Closures**: See `06-functions-closures.md` for
  generators (ID-05), closures (ID-06-09), higher-order functions
  (ID-20-24)
- **Async & Concurrency**: See `07-async-concurrency.md` for parallel
  vs sequential `await` (ID-13, ID-18)
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for
  `dissoc`, `assoc`, `conj`, `template`
