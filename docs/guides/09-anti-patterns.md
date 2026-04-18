# Anti-Patterns

A catalog of what NOT to do in lykn — mistakes that span multiple
guides, patterns common in AI-generated code, and subtle traps that need
deeper treatment. Every entry includes a fix with a cross-reference.

lykn eliminates many JS anti-patterns at the language level. These are
documented as ELIMINATED entries — brief notes explaining the JS hazard
and how lykn prevents it. The remaining entries cover traps that still
exist in lykn (inherited from JS runtime semantics) plus new lykn-
specific anti-patterns.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: Using `==` Instead of `===`

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `==` applies a multi-step coercion cascade.

lykn eliminates this entirely: `(= a b)` compiles to `a === b` (strict
equality, DD-22). Loose equality is only available via `(js:eq a b)` for
the `== null` idiom.

---

## ID-02: Trusting `js:typeof` for All Type Checks

**Strength**: SHOULD-AVOID

**Summary**: `js:typeof` returns misleading results for `null`, arrays,
and `NaN`.

```lykn
;; Anti-pattern — typeof lies
(= (js:typeof null) "object")      ;; true — historical bug
(= (js:typeof #a()) "object")      ;; true — arrays are objects

;; Fix — use the right check for each type
(= value null)                      ;; null check
(Array:isArray value)               ;; array check
(Number:isNaN value)                ;; NaN check
```

**In lykn**: Type annotations on `func`/`fn` handle most type checking
automatically. Manual `js:typeof` checks are rarely needed.

**Fix**: `05-type-discipline.md` ID-06, ID-07, ID-08.

---

## ID-03: Using `or` for Defaults When `0`, `""`, or `false` Are Valid

**Strength**: MUST-AVOID

**Summary**: `(or level 50)` compiles to `level || 50`, which swallows
`0`, `""`, `false`, and `NaN`.

```lykn
;; Anti-pattern — or swallows legitimate values
(func set-volume :args (:any level) :returns :number :body
  (or level 50))   ;; 0 becomes 50!

;; Fix — ?? only triggers on null/undefined
(func set-volume :args (:any level) :returns :number :body
  (?? level 50))   ;; 0 is preserved
```

**Fix**: `01-core-idioms.md` ID-03.

---

## ID-04: Global `isNaN` vs `Number:isNaN`

**Strength**: SHOULD-AVOID

**Summary**: The global `isNaN` coerces its argument first, producing
false positives.

```lykn
;; Anti-pattern — coerces to NaN first
(isNaN "hello")       ;; true — misleading

;; Fix — no coercion
(Number:isNaN "hello")  ;; false
(Number:isNaN NaN)      ;; true
```

**In lykn**: `:number` type annotations on `func`/`fn` already reject
NaN at the boundary.

**Fix**: `05-type-discipline.md` ID-08.

---

## ID-05: `parseInt` Without a Radix

**Strength**: SHOULD-AVOID

**Summary**: `parseInt` without a radix infers the base from the string
prefix.

```lykn
;; Fix — always explicit radix
(parseInt "08" 10)     ;; 8

;; Better for strict parsing
(Number "123abc")      ;; NaN — rejects trailing characters
```

---

## ID-06: Constructor Wrappers — `new String`, `new Number`, `new Boolean`

**Strength**: SHOULD-AVOID

**Summary**: `(new Boolean false)` creates a truthy object.

```lykn
;; Anti-pattern — wrapper objects
(bind flag (new Boolean false))
(if flag (console:log "this runs!"))  ;; objects are always truthy

;; Fix — call without new for conversion
(bind flag (Boolean false))
(bind num (Number "42"))
```

---

## ID-07: Method Extraction Loses `this`

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, storing a method in a variable loses `this`.

lykn eliminates this: `this` does not exist in surface lykn. Functions
are values — extracting them always works. For class methods that need
`this`, use the kernel `class` form.

---

## ID-08: Regular Functions as Callbacks When `this` Matters

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, ordinary functions as callbacks lose `this` in
strict mode.

lykn eliminates this: `fn` produces arrow functions. There is no `this`
in surface code to lose.

---

## ID-09: Arrow Functions as Object Methods

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, arrow functions as object methods inherit `this`
from the enclosing scope instead of the object.

lykn eliminates this: there is no `this` in surface lykn. Object
methods are built with `type` + `func`, not with `this`-dependent
method syntax.

---

## ID-10: `var` in Loops with Closures

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `var` creates one binding per function scope.
Closures in a loop all share the same variable.

lykn eliminates this: `var` does not exist. `for-of` always creates
an immutable binding per iteration.

---

## ID-11: Accidental Globals — Missing `bind`

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, assigning to an undeclared variable creates a
global property.

lykn eliminates this: all bindings require `bind`. There is no
bare assignment in surface lykn. ESM strict mode provides an
additional safety net.

---

## ID-12: Shadowing Outer Variables Accidentally

**Strength**: SHOULD-AVOID

**Summary**: An inner `bind` can shadow an outer one of the same name.

```lykn
;; Anti-pattern — shadow hides outer, adjustment lost
(bind result (compute-initial))
(if needs-adjustment
  (block
    (bind result (compute-adjusted))  ;; NEW binding — outer unchanged
    (console:log result)))
(console:log result)   ;; still the initial value

;; Fix — use distinct names
(bind result (compute-initial))
(if needs-adjustment
  (block
    (bind adjusted (compute-adjusted))
    (console:log adjusted)))
```

**Fix**: `06-functions-closures.md` ID-14.

---

## ID-13: Relying on `var` Hoisting

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `var` hoists to `undefined`.

lykn eliminates this: `var` does not exist. `bind` produces `const`,
which has a TDZ that catches early access with `ReferenceError`.

---

## ID-14: Mutating Function Arguments

**Strength**: MUST-AVOID

**Summary**: Objects and arrays are passed by identity. Mutating a
parameter mutates the caller's data.

```lykn
;; Anti-pattern — mutates caller's array
(func log-all :args (:array arr) :returns :void :body
  (while (> arr:length 0)
    (console:log (arr:shift))))

;; Fix — iterate without mutation
(func log-all :args (:array arr) :returns :void :body
  (for-of item arr
    (console:log item)))

;; Fix — use toSorted instead of sort
(func get-sorted :args (:array arr) :returns :array :body
  (arr:toSorted))
```

**In lykn**: The surface language steers you toward immutability.
`assoc`/`conj` create new values. But JS array methods like `:shift`,
`:sort`, `:splice` still mutate — use non-destructive alternatives.

**Fix**: `04-values-references.md` ID-12.

---

## ID-15: Assuming `bind` Prevents Deep Mutation

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `const` freezes the binding but not the value.

lykn eliminates the confusion: `bind` produces `const` AND the surface
language provides no way to mutate via `=`. For object updates, use
`assoc`/`dissoc`/`conj`. For controlled mutation, use `cell`.

**See also**: `04-values-references.md` ID-04.

---

## ID-16: Shallow Copy Surprise — `assoc` Doesn't Copy Nested Objects

**Strength**: SHOULD-AVOID

**Summary**: `assoc` is a shallow copy. Nested objects are shared.

```lykn
;; Anti-pattern — shallow copy shares nested refs
(bind original (obj :user (obj :name "Alice") :tags #a("admin")))
(bind copy (object (spread original)))
;; copy:user is the same reference as original:user

;; Fix — structuredClone for deep independence
(bind copy (structuredClone original))

;; Fix — nested assoc for targeted update
(bind copy (assoc original :user (assoc original:user :name "Bob")))
```

**Fix**: `04-values-references.md` ID-08, ID-11.

---

## ID-17: `:sort` Mutates in Place and Defaults to String Comparison

**Strength**: SHOULD-AVOID

**Summary**: `:sort` mutates the original array AND defaults to
lexicographic comparison.

```lykn
;; Anti-pattern — mutates the original
(bind original #a(3 1 2))
(bind sorted (original:sort))
;; sorted and original are the same reference, both [1, 2, 3]

;; Also: default is lexicographic
;; #a(10 9 2):sort → [10, 2, 9]

;; Fix — non-destructive (ES2023)
(bind sorted (original:toSorted (fn (:number a :number b) (- a b))))
```

**Fix**: `04-values-references.md` ID-15.

---

## ID-18: Sequential `await` on Independent Operations

**Strength**: MUST-AVOID

**Summary**: Consecutive `await` on unrelated operations serializes
them. Use `Promise:all`.

```lykn
;; Anti-pattern — sequential, total time = sum
(bind users (await (fetch-users)))
(bind posts (await (fetch-posts)))

;; Fix — parallel, total time = max
(bind (array users posts) (await (Promise:all #a(
  (fetch-users)
  (fetch-posts)))))
```

**Fix**: `07-async-concurrency.md` ID-13.

---

## ID-19: `:map(async fn)` Without `Promise:all`

**Strength**: MUST-AVOID

**Summary**: `:map` with an async function returns `Promise[]`, not
resolved values.

```lykn
;; Fix — wrap with Promise:all
(bind results (await (Promise:all
  (items:map (async (fn (:any item) (fetch-data item:id)))))))
```

**Fix**: `07-async-concurrency.md` ID-21.

---

## ID-20: Fire-and-Forget Promises

**Strength**: MUST-AVOID

**Summary**: Calling an async function without `await` loses rejections.
In Deno, this terminates the process.

```lykn
;; Anti-pattern
(log-request req)    ;; async, unawaited

;; Fix — await it
(await (log-request req))

;; Fix — explicit catch for intentional fire-and-forget
((log-request req):catch (fn (:any err) (console:error "Log failed:" err)))
```

**Fix**: `03-error-handling.md` ID-20.

---

## ID-21: Sync Throws in Promise-Returning Functions

**Strength**: SHOULD-AVOID

**Summary**: A function that returns a Promise must not throw
synchronously. Use `async` — sync throws become rejections.

**Fix**: `03-error-handling.md` ID-18.

---

## ID-22: `:then` Nesting Instead of Chaining

**Strength**: SHOULD-AVOID

**Summary**: Nesting `:then` inside `:then` recreates callback hell.
Use `async`/`await` or flat chains.

**Fix**: `03-error-handling.md` ID-14.

---

## ID-23: `fetch` Without `signal`

**Strength**: SHOULD-AVOID

**Summary**: A `fetch` without an `AbortSignal` cannot be cancelled.

```lykn
;; Fix — pass a signal
(bind data (await (fetch "/api/data"
  (obj :signal (AbortSignal:timeout 5000)))))
```

**Fix**: `07-async-concurrency.md` ID-24, ID-26.

---

## ID-24: Functions That Both Return AND Throw for Expected Cases

**Strength**: MUST-AVOID

**Summary**: Use one error channel. Don't return `null` on some
failures and throw on others. In lykn, consider using `type` with
`Some`/`None` or `Ok`/`Err` for explicit result modeling.

**Fix**: `03-error-handling.md` ID-02, ID-25.

---

## ID-25: Boolean Parameters — Unreadable Call Sites

**Strength**: SHOULD-AVOID

**Summary**: `(create-user "Alice" true false true)` is unreadable.
Use `obj` with keyword keys.

```lykn
;; Anti-pattern
(create-user "Alice" true false true)

;; Fix — keyword options
(create-user "Alice" (obj :admin true :verified false :notify true))
```

**Fix**: `02-api-design.md` ID-01.

---

## ID-26: Returning Objects from Constructors

**Strength**: SHOULD-AVOID

**Summary**: Returning a non-primitive from a constructor breaks
`instanceof`. Use static factory methods.

---

## ID-27: Overloaded Functions — Use Multi-Clause `func`

**Strength**: SHOULD-AVOID

**Summary**: Functions that change behavior based on argument count or
type are hard to reason about. In lykn, use multi-clause `func` for
clean overloading.

```lykn
;; Good — multi-clause dispatch in lykn
(func create-point
  (:args (:number x :number y) :returns :object :body (obj :x x :y y))
  (:args (:string s) :returns :object :body
    (bind parts (s:split ","))
    (obj :x (Number (get parts 0)) :y (Number (get parts 1)))))
```

**Fix**: `06-functions-closures.md` ID-30.

---

## ID-28: `var` in Any Code

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: `var` does not exist in lykn.

---

## ID-29: `for-in` on Arrays

**Strength**: MUST-AVOID

**Summary**: `for-in` iterates enumerable string keys, including
inherited ones. Use `for-of`.

```lykn
;; Good — for-of for values
(for-of value arr (console:log value))

;; Good — for-of with entries for index + value
(for-of (array i value) (arr:entries)
  (console:log (template i ": " value)))
```

**Fix**: `01-core-idioms.md` ID-25.

---

## ID-30: Using the `arguments` Object

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: `arguments` does not exist in lykn. Use `(rest ...)` in
parameter lists.

---

## ID-31: `eval` and `new Function`

**Strength**: MUST-AVOID

**Summary**: `eval` executes arbitrary code. It is a security
vulnerability and prevents engine optimization. In lykn, `js:eval`
exists as an escape hatch but should never be used in application code.

---

## ID-32: IIFEs in ESM Code

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: Module scope is already isolated in ESM. IIFEs add
complexity with zero benefit. lykn modules are always ESM.

---

## ID-33: CommonJS `require()` in ESM Context

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: lykn only produces ESM output. `require()` does not
exist.

---

## ID-34: `delete` on Array Elements

**Strength**: SHOULD-AVOID

**Summary**: Kernel `delete` on arrays creates holes. Use `:toSpliced`
(non-destructive) or `:splice` (destructive).

```lykn
;; Fix — non-destructive removal (ES2023)
(bind result (arr:toSpliced 1 1))
```

**Fix**: `08-performance.md` ID-09.

---

## ID-35: `JSON:parse(JSON:stringify(obj))` for Deep Copy

**Strength**: SHOULD-AVOID

**Summary**: The JSON round-trip silently drops `undefined`, functions,
and symbols.

```lykn
;; Fix — structuredClone
(bind copy (structuredClone original))
```

**Fix**: `04-values-references.md` ID-09, ID-10.

---

## ID-36: Catching Errors and Only Logging

**Strength**: SHOULD-AVOID

**Summary**: Catching an error and only logging it converts a failure
into silent success. Handle, rethrow, or both.

```lykn
;; Anti-pattern — swallowed
(try (await (process-data input))
  (catch err (console:log err)))
(continue-with-assumption)

;; Fix — rethrow after logging
(try (await (process-data input))
  (catch err
    (console:error "processData failed:" err)
    (throw err)))
```

**Fix**: `03-error-handling.md` ID-08, ID-09.

---

## lykn-Specific Anti-Patterns

The following anti-patterns are unique to lykn and have no JS parallel.

---

## ID-37: Forgetting `express` When Reading a Cell

**Strength**: MUST-AVOID

**Summary**: A `cell` wraps its value in `{ value: ... }`. Reading the
cell without `express` gives you the wrapper object, not the value.

```lykn
;; Anti-pattern — gets the wrapper object {value: 0}, not 0
(bind counter (cell 0))
(console:log counter)          ;; logs {value: 0}

;; Fix — use express to read the value
(console:log (express counter))  ;; logs 0
```

**Why it's wrong**: `cell` creates `{ value: 0 }`. Without `express`,
you pass the wrapper to functions that expect the inner value. This
produces silent failures — the function receives an object when it
expects a number.

**Fix**: `01-core-idioms.md` ID-01, `04-values-references.md` ID-20.

---

## ID-38: Using Kernel Forms When Surface Forms Exist

**Strength**: SHOULD-AVOID

**Summary**: Writing `(const x 42)` instead of `(bind x 42)`, or
`(=== a b)` instead of `(= a b)`, or `(&& x y)` instead of `(and x y)`.
Surface forms are the idiomatic layer.

```lykn
;; Anti-pattern — kernel forms in surface code
(const x 42)
(=== a b)
(&& x y)

;; Fix — surface forms
(bind x 42)
(= a b)
(and x y)
```

**Why it's wrong**: Kernel forms bypass surface semantics. `(const x 42)`
works but doesn't communicate "this is an immutable binding" the way
`(bind x 42)` does. `(=== a b)` works but doesn't benefit from DD-22's
design. Mixing kernel and surface forms makes code harder to read and
harder for tools to analyze.

**Fix**: `00-lykn-surface-forms.md` for the complete surface vocabulary.

---

## ID-39: Missing Type Annotations on Function Parameters

**Strength**: MUST-AVOID

**Summary**: Every parameter in `func` and `fn` requires a type keyword.
Using bare symbols is a compile error. Using `:any` everywhere defeats
the purpose.

```lykn
;; Anti-pattern — :any everywhere
(func process :args (:any x :any y :any z) :returns :any :body
  (+ x y z))

;; Fix — specific types enable runtime checking
(func process :args (:number x :number y :number z) :returns :number :body
  (+ x y z))
```

**Why it's wrong**: `:any` disables all type checking. If every
parameter is `:any`, you get no protection from wrong-type arguments.
Use specific type keywords (`:number`, `:string`, `:boolean`, etc.)
and reserve `:any` for genuinely polymorphic parameters.

**Fix**: `05-type-discipline.md` ID-01.

---

## ID-40: Overusing `js:` Interop

**Strength**: SHOULD-AVOID

**Summary**: The `js:` namespace is an escape hatch for JS interop.
It should be rare and greppable.

```lykn
;; Anti-pattern — using js: when surface forms exist
(js:eq a b)           ;; when you mean (= a b)
(js:typeof x)         ;; when func type annotations would suffice

;; Good — js: for genuine interop needs
(js:eq x null)        ;; the == null idiom — no surface equivalent
(js:bind obj:method obj)  ;; binding a method for callback
```

**Why it's wrong**: `js:` forms bypass surface language semantics.
They should only be used when surface forms genuinely cannot express
the operation. Overusing them defeats the safety guarantees of the
surface language.

---

## ID-41: Using `cell` When a Pure Approach Works

**Strength**: SHOULD-AVOID

**Summary**: Reaching for `cell` + `swap!` when `assoc`, `conj`, or
`:reduce` would produce cleaner, safer code.

```lykn
;; Anti-pattern — cell for accumulation
(bind result (cell #a()))
(for-of item items
  (if (valid? item)
    (swap! result (fn (:array r) (conj r (transform item))))))
(express result)

;; Fix — pure approach with :filter + :map
(-> items
  (:filter valid?)
  (:map transform))
```

**Why it's wrong**: `cell` is for state that genuinely changes over
time (counters, caches, UI state). For data transformations, pure
approaches using `:map`, `:filter`, `:reduce`, and threading macros
are clearer, safer, and more composable.

**Fix**: `06-functions-closures.md` ID-28.

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Anti-Pattern | Strength | Status |
|----|-------------|----------|--------|
| 01 | `==` instead of `===` | MUST-AVOID | ELIMINATED |
| 02 | Trusting `js:typeof` | SHOULD-AVOID | Converted |
| 03 | `or` for defaults | MUST-AVOID | Converted |
| 04 | Global `isNaN` | SHOULD-AVOID | Converted |
| 05 | `parseInt` without radix | SHOULD-AVOID | Converted |
| 06 | `new Boolean/String/Number` | SHOULD-AVOID | Converted |
| 07 | Method extraction loses `this` | MUST-AVOID | ELIMINATED |
| 08 | Regular function as callback | SHOULD-AVOID | ELIMINATED |
| 09 | Arrow as method | SHOULD-AVOID | ELIMINATED |
| 10 | `var` in loops | MUST-AVOID | ELIMINATED |
| 11 | Accidental globals | MUST-AVOID | ELIMINATED |
| 12 | Accidental shadowing | SHOULD-AVOID | Converted |
| 13 | `var` hoisting | SHOULD-AVOID | ELIMINATED |
| 14 | Mutating function arguments | MUST-AVOID | Converted |
| 15 | `const` = immutable | SHOULD-AVOID | ELIMINATED |
| 16 | Shallow copy surprise | SHOULD-AVOID | Converted |
| 17 | `:sort` mutates + string default | SHOULD-AVOID | Converted |
| 18 | Sequential `await` (independent) | MUST-AVOID | Converted |
| 19 | `:map(async fn)` without `all` | MUST-AVOID | Converted |
| 20 | Fire-and-forget promises | MUST-AVOID | Converted |
| 21 | Sync throws in Promise functions | SHOULD-AVOID | Converted |
| 22 | `:then` nesting | SHOULD-AVOID | Converted |
| 23 | `fetch` without `signal` | SHOULD-AVOID | Converted |
| 24 | Mixed return/throw | MUST-AVOID | Converted |
| 25 | Boolean parameters | SHOULD-AVOID | Converted |
| 26 | Return object from constructor | SHOULD-AVOID | Converted |
| 27 | Overloaded functions | SHOULD-AVOID | Converted |
| 28 | `var` in any code | MUST-AVOID | ELIMINATED |
| 29 | `for-in` on arrays | MUST-AVOID | Converted |
| 30 | `arguments` object | SHOULD-AVOID | ELIMINATED |
| 31 | `eval` | MUST-AVOID | Converted |
| 32 | IIFEs in ESM | CONSIDER-AVOIDING | ELIMINATED |
| 33 | CommonJS `require()` | MUST-AVOID | ELIMINATED |
| 34 | `delete` on arrays | SHOULD-AVOID | Converted |
| 35 | JSON deep copy | SHOULD-AVOID | Converted |
| 36 | Catch-and-log only | SHOULD-AVOID | Converted |
| 37 | Forgetting `express` | MUST-AVOID | **lykn-specific** |
| 38 | Kernel forms in surface code | SHOULD-AVOID | **lykn-specific** |
| 39 | `:any` everywhere | MUST-AVOID | **lykn-specific** |
| 40 | Overusing `js:` interop | SHOULD-AVOID | **lykn-specific** |
| 41 | `cell` when pure works | SHOULD-AVOID | **lykn-specific** |

**12 ELIMINATED** by language design. **24 converted** from JS. **5 new**
lykn-specific anti-patterns.

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for positive patterns behind
  ID-01, 03, 11, 28, 29, 33
- **API Design**: See `02-api-design.md` for ID-24, 25, 26, 27
- **Error Handling**: See `03-error-handling.md` for ID-18-22, 24, 36
- **Values & References**: See `04-values-references.md` for ID-14-16, 35
- **Type Discipline**: See `05-type-discipline.md` for ID-02, 04, 05, 06
- **Functions & Closures**: See `06-functions-closures.md` for
  ID-07-10, 12, 13
- **Async & Concurrency**: See `07-async-concurrency.md` for ID-18-23
- **Performance**: See `08-performance.md` for ID-17, 34
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for
  the complete surface vocabulary
