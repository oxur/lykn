# Type Discipline

Discipline for writing type-safe lykn code. lykn's surface language has
built-in type annotations on all function boundaries — `:number`,
`:string`, `:boolean`, `:any`, etc. — which the compiler enforces at
runtime. Combined with `type` constructors, `match` for exhaustive
dispatch, and `:pre`/`:post` contracts, lykn provides a type discipline
that is stricter than JS+JSDoc without requiring TypeScript.

This guide covers lykn's type system, runtime type checking, coercion
traps (inherited from JS), number edge cases, and typed collections.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: Every Function Parameter Must Have a Type Annotation

**Strength**: MUST (compiler-enforced)

**Summary**: In `func` and `fn`, every parameter requires a type
keyword. Use `:any` as the explicit opt-out.

```lykn
;; Good — all params typed
(func add
  :args (:number a :number b)
  :returns :number
  :body (+ a b))

;; Good — :any for genuinely untyped params
(func identity :args (:any x) :returns :any :body x)

;; Good — fn also requires types
(bind double (fn (:number x) (* x 2)))
```

Compiles to:

```js
function add(a, b) {
  if (typeof a !== "number" || Number.isNaN(a))
    throw new TypeError("add: arg 'a' expected number, got " + typeof a);
  if (typeof b !== "number" || Number.isNaN(b))
    throw new TypeError("add: arg 'b' expected number, got " + typeof b);
  const result__gensym0 = a + b;
  if (typeof result__gensym0 !== "number" || Number.isNaN(result__gensym0))
    throw new TypeError("add: return value expected number, got " + typeof result__gensym0);
  return result__gensym0;
}
```

**Supported type annotations**:

| Annotation | Runtime check |
|---|---|
| `:number` | `typeof !== "number" \|\| Number.isNaN(...)` |
| `:string` | `typeof !== "string"` |
| `:boolean` | `typeof !== "boolean"` |
| `:function` | `typeof !== "function"` |
| `:object` | `typeof !== "object" \|\| === null` |
| `:array` | `!Array.isArray(...)` |
| `:symbol` | `typeof !== "symbol"` |
| `:bigint` | `typeof !== "bigint"` |
| `:any` | No check |
| `:void` | No return (`:returns` only) |

**Rationale**: lykn replaces JSDoc annotations with built-in type
keywords that the compiler enforces at runtime. Every function boundary
is type-checked — no silent coercion. Use `--strip-assertions` to
remove checks in production builds.

**See also**: `00-lykn-surface-forms.md` Functions

---

## ID-02: Use `:returns` to Declare Return Types

**Strength**: SHOULD

**Summary**: Declare the return type with `:returns` to get
compile-time return value checking.

```lykn
;; Good — return type checked
(func parse-port
  :args (:string input)
  :returns :number
  :body
  (bind port (Number input))
  (if (or (not (Number:isFinite port)) (< port 0) (> port 65535))
    (throw (new RangeError (template "Invalid port: " input))))
  port)

;; Good — :void for side-effect functions
(func log-message :args (:string msg) :returns :void
  :body (console:log msg))

;; Good — :any when return type varies
(func find-user :args (:string id) :returns :any
  :body (users:get id))
```

**Rationale**: `:returns :number` ensures the function actually returns
a number — the compiler inserts a check on the return value. This
catches bugs where a code path accidentally returns `undefined` or the
wrong type.

---

## ID-03: Use `type` for Algebraic Data Types

**Strength**: SHOULD

**Summary**: `type` defines tagged constructors with typed fields.
This is lykn's native algebraic data type — no JSDoc `@typedef`, no
`kind` discriminant, no `switch` boilerplate.

```lykn
;; Define variants with typed fields
(type Shape
  (Circle :number radius)
  (Rect :number width :number height)
  (Triangle :number base :number height))

;; Constructors validate fields automatically
(Circle 5)     ;; ok → { tag: "Circle", radius: 5 }
(Circle "no")  ;; TypeError: Circle: field 'radius' expected number
```

Compiles to:

```js
{
  function Circle(radius) {
    if (typeof radius !== "number" || Number.isNaN(radius))
      throw new TypeError("Circle: field 'radius' expected number, got " + typeof radius);
    return {tag: "Circle", radius: radius};
  }
  function Rect(width, height) { /* type checks ... */ }
  const Triangle = /* ... */;
}
```

**Rationale**: `type` replaces the JS pattern of discriminated unions
with `kind` properties and JSDoc `@typedef`. The constructor validates
fields, the `tag` property enables `match` dispatch, and the compiler
generates all the boilerplate.

**See also**: `01-core-idioms.md` ID-30, `02-api-design.md` ID-11

---

## ID-04: Use `match` for Exhaustive Type Dispatch

**Strength**: SHOULD

**Summary**: `match` on `type` constructors provides exhaustive
dispatch — the compiler throws if no pattern matches.

```lykn
(type Shape
  (Circle :number radius)
  (Rect :number width :number height)
  (Triangle :number base :number height))

(func area :args (:any shape) :returns :number :body
  (match shape
    ((Circle r) (* Math:PI (* r r)))
    ((Rect w h) (* w h))
    ((Triangle b h) (/ (* b h) 2))))
```

Compiles to:

```js
function area(shape) {
  const result__gensym1 = (() => {
    const target__gensym0 = shape;
    if (target__gensym0.tag === "Circle") {
      const r = target__gensym0.radius;
      return Math.PI * (r * r);
    }
    if (target__gensym0.tag === "Rect") { /* ... */ }
    if (target__gensym0.tag === "Triangle") { /* ... */ }
    throw new Error("match: no matching pattern");
  })();
  return result__gensym1;
}
```

**Rationale**: `match` replaces JS `switch` on discriminant properties.
When the last clause is not a wildcard, the compiler adds a `throw`
fallback — any unhandled variant is caught at runtime. Adding a new
variant to a `type` forces updating all `match` expressions.

---

## ID-05: Use `:pre`/`:post` Contracts for Domain Constraints

**Strength**: SHOULD

**Summary**: Type annotations check the JS type (string, number, etc.).
Contracts check domain constraints (positive, non-empty, in range).

```lykn
;; Type annotation: port is a number
;; Pre-condition: port is in valid range
;; Post-condition: result is non-null
(func connect
  :args (:string host :number port)
  :returns :object
  :pre (and (> host:length 0) (>= port 0) (<= port 65535))
  :post (not (= ~ null))
  :body (create-connection host port))
```

Compiles to:

```js
function connect(host, port) {
  if (typeof host !== "string") throw new TypeError(/* ... */);
  if (typeof port !== "number" || Number.isNaN(port)) throw new TypeError(/* ... */);
  if (!(host.length > 0 && port >= 0 && port <= 65535))
    throw new Error("connect: pre-condition failed: ... — caller blame");
  const result__gensym0 = createConnection(host, port);
  if (!(result__gensym0 !== null))
    throw new Error("connect: post-condition failed: ... — callee blame");
  return result__gensym0;
}
```

**Contract types**:
- `:pre` — **caller blame**: the caller passed invalid data
- `:post` — **callee blame**: the function itself has a bug. Use `~`
  to reference the return value.

**Rationale**: Contracts are self-documenting validation that lives in
the function signature. They throw with blame attribution, making
debugging faster. Use `--strip-assertions` in production.

**See also**: `02-api-design.md` ID-27, `03-error-handling.md` ID-23

---

## ID-06: Use `js:typeof` for Runtime Type Checks

**Strength**: MUST

**Summary**: Know the two `typeof` quirks: `null` returns `"object"`,
and functions return `"function"`. In lykn, use `js:typeof` for
explicit typeof checks.

```lykn
;; typeof quirks (JS facts that apply to lykn)
(= (js:typeof null) "object")        ;; true — historical bug
(= (js:typeof #a()) "object")        ;; true — arrays are objects

;; Correct null check
(= value null)

;; Correct array check
(Array:isArray value)

;; Correct object check (excludes null)
(and (= (js:typeof value) "object") (!= value null))
```

**Rationale**: In most lykn code, you don't need `js:typeof` — the
type annotations on `func`/`fn` handle type checking automatically.
Use `js:typeof` when you need manual type discrimination in non-`func`
code.

---

## ID-07: Use `Array:isArray`, Not `js:typeof` or `instanceof`

**Strength**: MUST

**Summary**: `Array:isArray` is the only reliable array check.

```lykn
;; Good — cross-realm safe
(Array:isArray #a(1 2 3))   ;; true
(Array:isArray "string")     ;; false

;; Bad — typeof returns "object" for arrays
(= (js:typeof #a()) "object")  ;; true, but not useful
```

---

## ID-08: Use `Number:isNaN`, `Number:isFinite`, `Number:isInteger`

**Strength**: MUST

**Summary**: The `Number:` methods do not coerce their arguments. The
global versions coerce first, producing wrong results.

```lykn
;; Good — no coercion
(Number:isNaN NaN)         ;; true
(Number:isNaN "abc")       ;; false — string is not NaN
(Number:isFinite 42)       ;; true
(Number:isFinite "42")     ;; false — string is not a number
(Number:isInteger 5.0)     ;; true
(Number:isInteger 5.1)     ;; false
```

**Note**: lykn's `:number` type annotation already checks for NaN —
`(func f :args (:number x) ...)` rejects NaN at the boundary. Manual
NaN checks are only needed for dynamic data inside function bodies.

---

## ID-09: Avoid Implicit Coercion — Use `template` for Strings

**Strength**: SHOULD

**Summary**: Use `template` for string building. Use explicit
conversion for type conversion. Avoid relying on `+` coercion.

```lykn
;; Good — explicit string building
(bind label (template "Count: " count))

;; Good — explicit conversion
(bind n (Number user-input))
(bind s (String count))

;; Bad — relying on + coercion
(bind label (+ "Count: " count))  ;; works but obscure
```

**Rationale**: The `+` operator in lykn (like JS) adds numbers OR
concatenates strings. Using `template` makes string-building intent
explicit.

---

## ID-10: Understand the `+` Operator's Dual Nature

**Strength**: MUST

**Summary**: `+` adds numbers or concatenates strings, depending on
operand types. This is a JS fact that lykn inherits.

```lykn
;; String wins — if either operand is a string, concatenation occurs
(+ "1" 2)      ;; "12"
(+ 1 "2")      ;; "12"

;; Good — be explicit about intent
(bind sum (+ (Number a) (Number b)))   ;; arithmetic
(bind label (template "Count: " count)) ;; string building
```

---

## ID-11: `NaN` Is Not Equal to Itself

**Strength**: MUST

**Summary**: `(= NaN NaN)` is `false` because `=` compiles to `===`
(DD-22), and `NaN === NaN` is `false` in JS. Use `Number:isNaN`.

```lykn
;; Bad — always false
(= NaN NaN)              ;; false
(= x NaN)               ;; always false

;; Good — precise NaN detection
(Number:isNaN NaN)        ;; true
(Number:isNaN "abc")      ;; false — no coercion
```

**Note**: lykn's `:number` type annotation rejects NaN at function
boundaries, preventing NaN from entering your functions in the first
place.

---

## ID-12: `-0` Exists and `(= 0 -0)` — Use `Object:is` When It Matters

**Strength**: CONSIDER

**Summary**: JavaScript has both `+0` and `-0`. `=` (strict equality)
cannot distinguish them.

```lykn
(= 0 (- 0))              ;; true — = treats them as equal
(Object:is 0 (- 0))      ;; false — the only reliable check
```

---

## ID-13: Floating-Point Precision — `0.1 + 0.2 !== 0.3`

**Strength**: MUST

**Summary**: JavaScript numbers are IEEE 754 doubles. Most decimal
fractions cannot be represented exactly.

```lykn
;; The classic
(= (+ 0.1 0.2) 0.3)   ;; false! result is 0.30000000000000004

;; Good — integer arithmetic for money (store cents)
(bind price-in-cents 199)
(bind tax-in-cents 15)
(bind total-in-cents (+ price-in-cents tax-in-cents))  ;; 214 — exact
```

---

## ID-14: `BigInt` Cannot Be Mixed with `Number`

**Strength**: MUST

**Summary**: Arithmetic operators throw `TypeError` when mixing
`BigInt` and `Number`. Convert explicitly.

```lykn
;; Good — explicit conversion
(+ 2n (BigInt 1))       ;; 3n

;; Comparison works across types
(> 2n 1)                ;; true
(= 2n 2)               ;; false (different types, = is ===)
```

---

## ID-15: Safe Integers — Know the Limits

**Strength**: MUST

**Summary**: JavaScript numbers can only represent integers exactly up
to `2^53 - 1`. Beyond that, distinct values collide.

```lykn
Number:MAX-SAFE-INTEGER   ;; 9007199254740991

;; Good — validate before arithmetic
(func safe-add
  :args (:number a :number b)
  :returns :number
  :pre (and (Number:isSafeInteger a) (Number:isSafeInteger b))
  :body (+ a b))
```

---

## ID-16: lykn Type Annotations Replace JSDoc `@param`/`@returns`

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, JSDoc `@param` and `@returns` annotations
provide editor support and type checking without TypeScript.

lykn eliminates the need for JSDoc: type annotations on `func` and `fn`
(`:number`, `:string`, `:any`, etc.) are built into the language and
enforced at runtime. The compiled JS output carries the type checks
directly — no separate annotation layer needed.

```lykn
;; lykn — types are in the language
(func parse-port
  :args (:string input)
  :returns :number
  :body
  (bind port (Number input))
  (if (or (not (Number:isFinite port)) (< port 0) (> port 65535))
    (throw (new RangeError (template "Invalid port: " input))))
  port)
```

---

## ID-17: Use `type` Instead of `@typedef` for Object Shapes

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `@typedef` defines reusable types for complex
object shapes. In lykn, use `type` constructors — they are real values
with runtime validation, not just documentation annotations.

```lykn
(type ServerConfig
  (Config :string host :number port))

(func start-server :args (:any config) :returns :void :body
  (match config
    ((Config h p) (listen h p))))
```

---

## ID-18: Use `type` with Zero-Field Variants Instead of `@enum`

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `@enum` combined with `Object.freeze()` creates
a closed set of constants. In lykn, use `type` with zero-field
variants.

```lykn
;; Good — type replaces frozen enum objects
(type LogLevel Debug Info Warn ErrorLevel)

(func log :args (:string message :any level) :returns :void :body
  (match level
    (Debug (console:debug message))
    (Info (console:info message))
    (Warn (console:warn message))
    (ErrorLevel (console:error message))))

(log "Server started" Info)
```

**Rationale**: `type` variants are tagged values with exhaustive
`match` dispatch. No need for `Object:freeze` or `switch` with a
throwing `default` — `match` adds the throw automatically.

---

## ID-19: No `@template` Needed — `:any` Is the Opt-Out

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `@template` adds type parameters for generic
patterns. In lykn, use `:any` for parameters that accept any type.
Parametric types are planned for a future version.

```lykn
;; Good — :any for generic-like behavior
(func find :args (:array arr :function predicate) :returns :any :body
  (bind result (cell undefined))
  (for-of item arr
    (if (predicate item)
      (block (reset! result item) (break))))
  (express result))
```

---

## ID-20: Deno LSP Works with Compiled JS Output

**Strength**: SHOULD

**Summary**: Deno's LSP reads JSDoc annotations on the compiled JS
output. For libraries that publish compiled JS, you can add JSDoc
annotations to the compiled output or provide `.d.ts` files.

For lykn source files, the type annotations in `func`/`fn` serve as
the primary type documentation. The compiled JS carries the runtime
checks.

---

## ID-21: Distinguish `undefined` from `null`

**Strength**: SHOULD

**Summary**: Use `undefined` for system-level absence (missing
parameters, uninitialized). Use `null` for programmer-intentional
absence. Or use `type Option (Some :any value) None` for explicit
modeling.

```lykn
;; Good — type models presence explicitly
(type Option (Some :any value) None)

(func find-config :args (:string key) :returns :any :body
  (bind val (config:get key))
  (if (js:eq val null) (None) (Some val)))

;; Caller uses match — can't forget to check
(match (find-config "theme")
  ((Some v) (apply-theme v))
  (None (use-default-theme)))
```

**See also**: `02-api-design.md` ID-24

---

## ID-22: Use `??` and `some->` — They Understand Null/Undefined

**Strength**: MUST

**Summary**: `??` defaults only on `null`/`undefined`. `some->` short-
circuits on `null`/`undefined`. Both preserve `0`, `""`, and `false`.

```lykn
;; Good — ?? preserves valid falsy values
(bind timeout (?? options:timeout 5000))

;; Good — some-> chains safely
(bind zip (?? (some-> user :address :zip-code) "N/A"))

;; Bad — or discards valid falsy values
(bind timeout (or options:timeout 5000))  ;; 0 becomes 5000!
```

**See also**: `01-core-idioms.md` ID-03, ID-04

---

## ID-23: Use `(= x undefined)` or `(= x null)` — Not `js:typeof`

**Strength**: SHOULD

**Summary**: In lykn, `(= x undefined)` compiles to `x === undefined`.
The `(js:typeof x)` guard is a legacy pattern.

```lykn
;; Good — direct and modern
(if (= x undefined) (handle-missing))
(if (= x null) (handle-empty))
(if (js:eq x null) (handle-null-or-undefined))  ;; loose check
```

---

## ID-24: `type` + `match` Replaces Discriminated Unions

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, the closest equivalent to algebraic data types is
discriminated unions with a `kind` property and `switch` dispatch.

lykn has native algebraic data types via `type` and exhaustive dispatch
via `match`. No `kind` property, no `switch`, no throwing `default`.

```lykn
;; lykn — native ADT
(type Shape
  (Circle :number radius)
  (Rect :number width :number height)
  (Triangle :number base :number height))

(func area :args (:any shape) :returns :number :body
  (match shape
    ((Circle r) (* Math:PI (* r r)))
    ((Rect w h) (* w h))
    ((Triangle b h) (/ (* b h) 2))))
```

---

## ID-25: `match` Provides Exhaustiveness — No Manual `default` Needed

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, you need a `default` case in `switch` that throws
to catch unhandled variants. In lykn, `match` adds the throw
automatically when the patterns are not exhaustive.

---

## ID-26: Use `Map`/`Set` for Dynamic Typed Collections

**Strength**: SHOULD

**Summary**: Use `Map` for dynamic key-value collections and `Set` for
unique-value collections.

```lykn
;; Good — Map for dynamic collections
(bind word-counts (new Map))
(word-counts:set "hello" 1)
(word-counts:set "world" 2)

;; Good — Set for unique values
(bind visited (new Set))
(visited:add "/home")
(visited:add "/about")
```

**See also**: `01-core-idioms.md` ID-16

---

## ID-27: Use TypedArray for Binary/Numeric Data

**Strength**: CONSIDER

**Summary**: For binary data and numeric-heavy computation, TypedArrays
enforce element type and are faster.

```lykn
;; Good — fixed-type, zero-initialized
(bind pixels (new Uint8ClampedArray (* width height 4)))
(bind samples (new Float32Array (* sample-rate duration)))
```

---

## ID-28: Homogeneous Arrays

**Strength**: SHOULD

**Summary**: Arrays should contain elements of a single type. Use
`type` constructors for collections of structured data.

```lykn
;; Good — homogeneous
(bind names #a("Alice" "Bob" "Carol"))
(bind scores #a(95 87 92))

;; Good — array of typed records
(type UserScore (US :string name :number score))
(bind results #a((US "Alice" 95) (US "Bob" 87)))
```

---

## ID-29: Validate at Boundaries with Type Annotations and Contracts

**Strength**: SHOULD

**Summary**: Use `func` type annotations for JS type checking and
`:pre` contracts for domain constraints. Trust types within.

```lykn
;; Good — boundary validation via func
(export (func connect
  :args (:string host :number port)
  :returns :object
  :pre (and (> host:length 0) (>= port 0) (<= port 65535))
  :body (create-connection host port)))

;; Internal functions trust the types — no redundant checks
(func format-address :args (:string host :number port) :returns :string
  :body (template host ":" port))
```

**Rationale**: `func` annotations generate type checks automatically.
`:pre` adds domain constraints. Internal functions trust the validated
types — redundant checking adds noise without safety.

**See also**: `03-error-handling.md` ID-23

---

## ID-30: `--strip-assertions` for Production Builds

**Strength**: SHOULD

**Summary**: Use `--strip-assertions` to remove all type checks and
contracts from production builds, leaving only the core logic.

```sh
# Development — full type checks and contracts
lykn compile main.lykn -o main.js

# Production — no type checks, no contracts
lykn compile main.lykn --strip-assertions -o main.js
```

```lykn
(func add
  :args (:number a :number b)
  :returns :number
  :pre (and (>= a 0) (>= b 0))
  :body (+ a b))
```

With `--strip-assertions`:

```js
function add(a, b) {
  return a + b;
}
```

**Rationale**: Type checks and contracts are valuable during
development and testing but add runtime overhead in production. The
`--strip-assertions` flag removes all generated checks while preserving
the core logic.

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | Type annotations on all params | MUST | `:number`, `:string`, etc. — compiler-enforced |
| 02 | `:returns` for return types | SHOULD | Catches wrong-type returns |
| 03 | `type` for algebraic data types | SHOULD | Tagged constructors with field validation |
| 04 | `match` for exhaustive dispatch | SHOULD | Compiler throws on unhandled variants |
| 05 | `:pre`/`:post` contracts | SHOULD | Domain constraints with blame attribution |
| 06 | `js:typeof` quirks | MUST | `null` → `"object"`, arrays → `"object"` |
| 07 | `Array:isArray` | MUST | Only reliable array check |
| 08 | `Number:isNaN`/`isFinite`/`isInteger` | MUST | No coercion, unlike global versions |
| 09 | `template` for strings, explicit conversion | SHOULD | Avoid implicit `+` coercion |
| 10 | `+` dual nature | MUST | String wins in lykn just like JS |
| 11 | `NaN !== NaN` | MUST | `Number:isNaN` is the only check |
| 12 | `-0 === 0` | CONSIDER | `Object:is` for edge cases |
| 13 | Floating-point precision | MUST | Integer arithmetic for money |
| 14 | BigInt/Number mixing | MUST | Throws TypeError; convert explicitly |
| 15 | Safe integer limits | MUST | Beyond 2^53-1, values collide |
| 16 | JSDoc replaced by type annotations | ELIMINATED | lykn has built-in types |
| 17 | `@typedef` replaced by `type` | ELIMINATED | Real values, not documentation |
| 18 | `@enum` replaced by `type` variants | ELIMINATED | Zero-field variants + `match` |
| 19 | `@template` replaced by `:any` | ELIMINATED | Parametric types planned |
| 20 | Deno LSP on compiled JS | SHOULD | Types in lykn, LSP on output |
| 21 | `undefined` vs `null` — use `Option` | SHOULD | `Some`/`None` is safer |
| 22 | `??` and `some->` | MUST | Preserve `0`, `""`, `false` |
| 23 | `(= x undefined)` not `js:typeof` | SHOULD | Direct check in modern code |
| 24 | `type`+`match` replaces discriminated unions | ELIMINATED | Native ADTs |
| 25 | `match` auto-throws | ELIMINATED | No manual `default` needed |
| 26 | `Map`/`Set` for typed collections | SHOULD | Any key type, `.size` |
| 27 | TypedArray for binary/numeric | CONSIDER | Fixed type, faster |
| 28 | Homogeneous arrays | SHOULD | Consistent types, safe iteration |
| 29 | Validate at boundaries with contracts | SHOULD | `:pre` for domain constraints |
| 30 | `--strip-assertions` for production | SHOULD | Zero-cost types in production |

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for `=` equality (ID-02),
  `??` (ID-03), `some->` (ID-04), `type`+`match` (ID-30)
- **API Design**: See `02-api-design.md` for contracts (ID-27), `type`
  constructors (ID-11, ID-15), `?` predicates (ID-17)
- **Error Handling**: See `03-error-handling.md` for validation at
  boundaries (ID-23), fail-fast (ID-24)
- **Values & References**: See `04-values-references.md` for `Object:freeze`
  (ID-16), equality (ID-24), cell model (ID-20)
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for type
  annotation table, `func` syntax, `match` patterns
