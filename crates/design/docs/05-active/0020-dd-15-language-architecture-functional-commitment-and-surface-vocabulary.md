---
number: 20
title: "DD-15: Language Architecture, Functional Commitment, and Surface Vocabulary"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-03-27
updated: 2026-03-31
state: Active
supersedes: null
superseded-by: null
version: 1.1
---


# DD-15: Language Architecture, Functional Commitment, and Surface Vocabulary

**Status**: Decided
**Date**: 2026-03-27
**Session**: v0.3.0 safety layer design

## Summary

lykn is formally split into two layers: **lykn/kernel** (DD-01 through
DD-09, compiled by the existing JS compiler) and **lykn/surface** (the
user-facing language, compiled by a new Rust-based surface compiler that
emits kernel forms). lykn/surface is a functional language: all bindings
are immutable, mutation occurs only through explicit `cell` containers,
and `this` is absent from the surface vocabulary. Keywords (`:name`) are
activated as a reader-level type. The surface form vocabulary is `bind`,
`func`, `fn`/`lambda`, `type`, `match`, `macro`, `obj`, and `cell`.
JS interop uses the `js:` colon-namespace as an explicit escape hatch.

## Decisions

### Two-layer architecture: lykn/kernel and lykn/surface

**Decision**: lykn consists of two formally named layers. **lykn/kernel**
is the set of core forms defined in DD-01 through DD-09 (`const`,
`function`, `lambda`, `=>`, `if`, `import`, `export`, `class`, `for`,
`try`, etc.). The JS compiler (reader.js + compiler.js + astring)
compiles kernel forms to JavaScript. lykn/kernel is the compilation
target, not the authoring surface. **lykn/surface** is the
user-facing language — what developers write in `.lykn` files. Surface
forms expand to kernel forms. The surface layer is where all safety,
ergonomics, and functional discipline live.

**Naming rationale**: "Kernel" and "surface" are standard PL
terminology. Haskell has Core (System FC) vs surface syntax. Racket
has "fully expanded programs" vs user-facing language. Scheme
distinguishes "core forms" from "derived expressions." The terms
are precise, widely understood, and unambiguous.

**Rationale**: Clean separation of concerns. The kernel compiler is
a thin, stable s-expression-to-ESTree translator that maps 1:1 to
JavaScript semantics. The surface compiler handles safety, type
checking, exhaustiveness analysis, and ergonomic sugar. Neither
needs to understand the other's domain. The kernel compiler never
changes for surface-layer features.

### Rust surface compiler

**Decision**: lykn/surface is compiled by a new Rust-based tool that
parses surface syntax, performs static analysis, and emits lykn/kernel
forms. The pipeline becomes:

```
.lykn source → Rust surface compiler (surface → kernel + diagnostics)
             → JS kernel compiler (kernel → ESTree → JS via astring)
```

The Rust surface compiler handles:
- Parsing lykn/surface syntax (all forms defined in DD-15+)
- Macro expansion (DD-10 through DD-14 machinery)
- Static analysis (exhaustiveness checking, occurrence typing,
  unused bindings, contract verification, dead code detection)
- Error messages with source locations in the *surface* syntax
- Linting and formatting (natural extensions of the same AST)

**Rationale**: A dedicated surface compiler in Rust provides static
analysis capabilities impossible with macro-only expansion.
Exhaustiveness checking for `match`, occurrence typing after type
predicates, contract verification, and unused binding detection all
require analysis passes over the surface AST. Rust's type system
provides rigor for the compiler itself — the tool that enforces
safety should itself be safe. The existing Rust binary in the lykn
toolchain provides a natural home.

### Functional commitment: immutable by default

**Decision**: lykn/surface is a functional language. All bindings
created with `bind` are immutable. There is no reassignable binding
form — no `var`, no `let`-as-mutable, no `bind!`, no `set!` for
rebinding. Mutation occurs only through `cell` containers (see
"Controlled mutation via cells" below).

"Update" operations on objects and arrays produce new values via
shallow copy. The surface language provides `assoc`, `dissoc`, and
`conj` macros that expand to spread-based copies in kernel.

**Syntax**:

```lisp
;; All bindings are immutable
(bind name "Duncan")
(bind age 42)
```

```javascript
const name = "Duncan";
const age = 42;
```

```lisp
;; "Updating" produces a new value
(bind user (obj :name "Duncan" :age 42))
(bind updated (assoc user :age 43))
```

```javascript
const user = { name: "Duncan", age: 42 };
const updated = { ...user, age: 43 };
```

**ESTree nodes**: `VariableDeclaration` with `kind: "const"` (always).
`assoc` expands to `SpreadElement` inside `ObjectExpression`.

**Rationale**: Immutability-by-default is the single highest-impact
safety feature identified in the JavaScript Hazard Landscape research.
It eliminates stale closure bugs, race conditions on shared variables,
unexpected side effects from function calls, and prototype pollution
through mutation. ClojureScript has proven this model works at scale
for a Lisp compiling to JavaScript. The shallow-copy cost (O(n) in
object size via spread operators) is acceptable for the vast majority
of use cases. When it isn't, a persistent data structure library can
be imported.

### `bind` type annotations

**Decision**: `bind` supports optional type annotations. When
present, the type keyword appears between `bind` and the name.
Type annotations are optional for primitive literal initializers
(numbers, strings, booleans, keywords, `null`, `undefined`) because
the type is self-evident from the literal. For non-literal
initializers (variable references, function calls, operator
expressions, constructor calls, `obj` construction), whether a type
annotation is required is an open question (see Open Questions —
deferred to the Rust compiler design session).

**Syntax**:

```lisp
;; Primitive literals — type optional, self-evident
(bind name "Duncan")
(bind age 42)
(bind active true)

;; Primitive literals — type annotation available but redundant
(bind :string name "Duncan")
(bind :number age 42)

;; Non-literal initializers — type annotation recommended/required
(bind :number b (parse-float a))
(bind :string c a)
(bind :Option d (find-user id))
(bind :object e (obj :x 1 :y 2))
(bind :number f (+ 1 2))
(bind :array g (map double items))
```

```javascript
// All compile to const — type annotations are dev-mode checks
const name = "Duncan";
const age = 42;
const active = true;
const b = parseFloat(a);      // + dev-mode typeof check
const c = a;                   // + dev-mode typeof check
const d = findUser(id);        // + dev-mode tag check
const e = { x: 1, y: 2 };     // + dev-mode typeof check
const f = 1 + 2;               // + dev-mode typeof check
const g = items.map(double);   // + dev-mode Array.isArray check
```

**ESTree nodes**: `VariableDeclaration` with `kind: "const"` (always).
Type annotations → `IfStatement` + `ThrowStatement` before value use
(same pattern as `func` type checks, stripped by `--strip-assertions`).

**Rationale**: Primitive literals are self-typing — writing
`:string` next to `"Duncan"` is redundant noise. But when the
initializer is a variable reference, function call, or expression,
the type is not visible at the binding site. The developer (and the
compiler) must trace through the code to determine what type flows
into the binding. Type annotations at these sites make the data flow
explicit and enable dev-mode assertions. This creates a natural
spectrum across the surface language: `type` constructor fields
require annotations (strictest — data definitions), `func`/`fn`
parameters require annotations (interfaces), `bind` with literals
is optional (self-evident).

### Controlled mutation via cells

**Decision**: The only mutation mechanism in lykn/surface is the `cell`
container. A `cell` is an immutable binding to a mutable container.
The binding itself is `const` — it cannot be reassigned. Only the
value inside the cell can change, and only through explicit `swap!`
or `reset!` operations. Reading the value uses `express`.

Four forms constitute the complete cell vocabulary:

| Form | Purpose | Kernel expansion |
|------|---------|------------------|
| `(cell value)` | Create mutable container | `(object (value value))` |
| `(express c)` | Read current value | `c:value` |
| `(swap! c f)` | Update via function | `(= c:value (f c:value))` |
| `(reset! c v)` | Replace value directly | `(= c:value v)` |

**Syntax**:

```lisp
(bind counter (cell 0))
(swap! counter inc)
(reset! counter 0)
(console:log (express counter))
```

```javascript
const counter = { value: 0 };
counter.value = inc(counter.value);
counter.value = 0;
console.log(counter.value);
```

**ESTree nodes**: `cell` → `ObjectExpression` with single `Property`
(`key: "value"`). `express` → `MemberExpression` (`.value`). `swap!`
→ `AssignmentExpression` with call. `reset!` → `AssignmentExpression`.

**Rationale**: Cell-based mutation provides a single, explicit,
auditable path for all state changes. Every `!` in a codebase marks
a mutation point — greppable, visible in code review, impossible to
miss. The slight ceremony of `cell`/`express`/`swap!` compared to
direct reassignment is intentional friction that pushes developers
toward functional solutions. The compiled JS uses plain objects with
a `.value` property — no runtime library, no special class, zero
dependencies. Any JS code can interop with cells trivially.

**Naming rationale**: `cell` comes from Rust's `Cell<T>` and ML's
`ref` cells — two independent language traditions converging on the
same word for interior mutability behind an immutable reference.
`express` comes from biology: gene expression reads information from
a cell and makes it available without altering the cell. The metaphor
is precise — the cell contains information, expressing it produces a
usable value, the cell is unchanged. `swap!` and `reset!` follow
Clojure convention for cell-mutation operations, with the `!` suffix
marking effectful operations per Scheme tradition.

### No `this` in the surface language

**Decision**: lykn/surface has no `this` form. The surface vocabulary
provides no way to reference `this`. Class methods in lykn/kernel
(DD-07) continue to use `this` internally, but the surface layer
encourages functional patterns: closures, explicit parameters, and
`obj` with methods that close over state.

JS interop for methods requiring `this` uses the `js:` namespace:

```lisp
;; Calling a JS method (this is bound correctly by colon syntax)
obj:method arg1 arg2

;; Extracting and rebinding a method
(bind bound-fn (js:bind obj:method obj))
```

```javascript
obj.method(arg1, arg2);
const boundFn = obj.method.bind(obj);
```

**ESTree nodes**: `js:bind` expands to `MemberExpression` +
`CallExpression` on `.bind()`.

**Rationale**: `this` binding loss is BugAID pattern #10 and one of
the most frequently reported JavaScript confusions. Eliminating `this`
from the surface language removes the entire hazard category by
construction. Closure-based objects and explicit parameters are
idiomatic in a functional Lisp. The `js:` escape hatch provides
interop when calling JS libraries that require `this`.

### Keywords as reader-level type

**Decision**: Leading `:` (reserved since DD-01) is activated.
A keyword is an atom beginning with `:`. The reader produces a
distinct node type: `{ type: "keyword", value: "name" }`. Keywords
are self-evaluating and compile to string literals.

```lisp
:name
:age
:first-name
```

```javascript
"name"
"age"
"firstName"
```

Keywords follow camelCase conversion (DD-01): `:first-name` compiles
to `"firstName"`.

**ESTree nodes**: `Literal` with `value: "camelCasedName"` (string).

**Rationale**: Keywords serve as the syntactic glue for lykn/surface.
They enable keyword-value alternation in object construction
(eliminating grouped pairs), type annotations in function parameters,
field selectors in pattern matching, and option markers across the
surface language. Compiling to string literals is the simplest
possible implementation — no runtime `Keyword` class, no symbol
registry, zero dependencies. Keywords are visually distinct from
symbols (the `:` prefix), unambiguous to the reader, and
self-documenting.

### Surface object syntax with keywords

**Decision**: lykn/surface introduces `obj` as the object construction
form using keyword-value alternation. No pair grouping required.

```lisp
;; Surface: keyword-value alternation
(obj :name "Duncan" :age 42 :active true)
```

```javascript
({ name: "Duncan", age: 42, active: true })
```

```lisp
;; Reader dispatch shorthand
#o(:name "Duncan" :age 42)
```

```javascript
({ name: "Duncan", age: 42 })
```

```lisp
;; With computed values
(obj :name user-name :score (* base multiplier))
```

```javascript
({ name: userName, score: base * multiplier })
```

The surface `obj` form expands to the kernel `object` form with
grouped pairs:

```
(obj :name "Duncan" :age 42)
  ↓ surface expansion
(object (name "Duncan") (age 42))
  ↓ kernel compilation
({ name: "Duncan", age: 42 })
```

The kernel `object` form with grouped pairs (DD-06) continues to
work unchanged. `obj` is surface sugar. The `#o(...)` reader dispatch
(DD-12 v1.2) is updated to accept keyword-value alternation in
addition to grouped pairs.

**ESTree nodes**: `ObjectExpression` with `Property` nodes (unchanged
from kernel).

**Rationale**: `obj` is shorter than `object` and visually
distinguishes surface from kernel code. Keyword-value alternation
eliminates the extra parentheses around each pair, reducing visual
noise. The alternating pattern is self-delimiting — each `:keyword`
starts a new key-value pair. This is how Clojure maps work and is
natural in s-expressions.

### Surface form vocabulary

**Decision**: lykn/surface defines the following form heads. Each
expands to kernel forms. The kernel compiler is unchanged.

| Surface form | Kernel expansion | Purpose |
|---|---|---|
| `bind` | `const` | Immutable binding (optionally typed) |
| `func` | `function` | Named function with built-in contracts |
| `fn` | `lambda` or `=>` | Anonymous function |
| `lambda` | `lambda` or `=>` | Alias for `fn` (nostalgia) |
| `type` | Constructor functions + metadata | Algebraic data type |
| `match` | Nested `if` + `get` + destructuring | Pattern matching |
| `obj` | `object` (grouped pairs) | Object construction with keywords |
| `cell` | `(object (value x))` | Mutable container |
| `express` | `c:value` | Read cell value |
| `swap!` | `(= c:value (f c:value))` | Update cell via function |
| `reset!` | `(= c:value v)` | Replace cell value |
| `macro` | (DD-11, unchanged) | Macro definition |
| `import` | (DD-04, unchanged) | Module import |
| `export` | (DD-04, unchanged) | Module export |
| `some->` | Nested `let` + nil checks | Nil-safe thread-first |
| `some->>` | Nested `let` + nil checks | Nil-safe thread-last |
| `->` | Nested calls (thread-first) | Threading macro |
| `->>` | Nested calls (thread-last) | Threading macro |
| `if-let` | `let` + nil check | Conditional binding |
| `when-let` | `let` + nil check | Conditional binding (no else) |
| `assoc` | Spread + object | Immutable object update |
| `dissoc` | Destructure + spread | Immutable object key removal |
| `conj` | Spread + array | Immutable array append |

`type` is not a JavaScript keyword or reserved word. It is a valid
identifier in the JS specification. Safe to use as a surface form.

**Rationale**: Short English words over Lisp abbreviations (`func`
not `defn`, `type` not `deftype`, `bind` not `def`). Each form has
a unique name — no `def-` prefix family. The vocabulary is small
enough to memorize, large enough to cover all common patterns.
`fn` and `lambda` are aliases — `fn` for daily use, `lambda` for
developers who prefer the traditional Lisp name.

### `func` includes contract support

**Decision**: `func` is the canonical function definition form in
lykn/surface. It includes optional type annotations and optional
`:pre`/`:post` contract clauses. Contracts are built into the
function form, not a separate `func/c` variant.

```lisp
;; Minimal: zero-arg positional shorthand
(func make-timestamp
  (Date:now))
```

```javascript
function makeTimestamp() {
  return Date.now();
}
```

```lisp
;; With type annotations (keyword-labeled, all params typed)
(func add
  :args (:number a :number b)
  :returns :number
  :body (+ a b))
```

```javascript
// Dev mode: runtime type assertions emitted
function add(a, b) {
  if (typeof a !== "number" || Number.isNaN(a))
    throw new TypeError("add: arg 'a' expected number, got " + typeof a);
  if (typeof b !== "number" || Number.isNaN(b))
    throw new TypeError("add: arg 'b' expected number, got " + typeof b);
  return a + b;
}

// Production mode (--strip-assertions): assertions elided
function add(a, b) {
  return a + b;
}
```

```lisp
;; With contracts (single expression, and/or composition)
(func withdraw
  :args (:number amount :account acct)
  :returns :account
  :pre (and (> amount 0)
            (<= amount (express (get acct :balance))))
  :post (>= (express (get ~ :balance)) 0)
  :body
  (assoc acct :balance (- (express (get acct :balance)) amount)))
```

```javascript
// Dev mode
function withdraw(amount, acct) {
  if (typeof amount !== "number" || Number.isNaN(amount))
    throw new TypeError("withdraw: arg 'amount' expected number, got " + typeof amount);
  if (!(amount > 0 && amount <= acct.balance.value))
    throw new Error("withdraw: pre-condition failed: (and (> amount 0) (<= amount (express (get acct :balance)))) — caller blame");
  const _result = { ...acct, balance: acct.balance.value - amount };
  if (!(_result.balance.value >= 0))
    throw new Error("withdraw: post-condition failed: (>= (express (get ~ :balance)) 0) — callee blame");
  return _result;
}
```

Type annotations in parameters use keyword-type-name pairs:
`(:number a :string b)`. The keyword names the type, the following
symbol names the parameter. All parameters require type annotations
— bare symbols are a compile error. Use `:any` to explicitly opt out
of type checking.

Return type uses `:returns` keyword. If absent, the function returns
nothing (void).

`:pre` and `:post` are optional keyword clauses. Each takes a single
boolean expression. Multiple conditions are composed explicitly with
`and`/`or`. In `:post`, `~` refers to the return value.

**ESTree nodes**: `FunctionDeclaration`. Type assertions →
`IfStatement` + `ThrowStatement` + `NewExpression` (`TypeError` for
type violations, `Error` for contract violations). Body →
`BlockStatement`.

**Rationale**: Making contracts part of the standard function form
means the cheapest thing to write is the safe thing. Developers
don't add contracts as an afterthought — they're a natural part of
function definition. The `:pre`/`:post` keywords are optional, so
simple functions have zero ceremony. Contract assertions are
strippable in production via `--strip-assertions`, following
Clojure.spec's `*compile-asserts*` pattern. The 32% of server-side
bugs from missing input validation (BugsJS) are directly addressed.

### JS interop via `js:` namespace

**Decision**: lykn/surface provides a `js:` colon-namespace for
explicit interop escape hatches. These forms bypass surface-level
safety guarantees and are greppable in code review.

| Form | Kernel expansion | JS output |
|------|------------------|-----------|
| `(js:call obj:method args...)` | `(obj:method args...)` | `obj.method(args)` |
| `(js:bind obj:method obj)` | method `.bind()` call | `obj.method.bind(obj)` |
| `(js:eval code)` | `(eval code)` | `eval(code)` |
| `(js:eq a b)` | loose equality | `a == b` |
| `(js:typeof x)` | `(typeof x)` | `typeof x` |

The Rust surface compiler recognizes the `js:` prefix and processes
these forms specially. They do not reach the kernel compiler as
`js.call(...)` — the surface compiler strips the `js:` namespace and
emits the appropriate kernel form.

**Rationale**: Every unsafe JS operation is explicitly namespaced and
greppable. Security auditors can search for `js:` to find every place
where surface-level safety guarantees are bypassed. The colon syntax
is consistent with DD-01's member-access convention — `js:call` looks
like accessing `call` on a `js` module, which is the right mental
model (it's accessing the raw JS semantics).

### Strict equality: `=` always means `===`

**Decision**: In lykn/surface, `(= a b)` compiles to `a === b`.
`(not= a b)` compiles to `a !== b`. The loose equality operator
(`==`) is only available via `(js:eq a b)`.

```lisp
(= a b)
(not= a b)
(js:eq a b)     ;; escape hatch for loose equality
```

```javascript
a === b
a !== b
a == b
```

**ESTree nodes**: `BinaryExpression` with `operator: "==="` or
`"!=="` or `"=="`.

**Rationale**: CoffeeScript proved this is the single simplest
safety win with the broadest impact. Eliminates the entire coercion
equality table, BugAID pattern #4, and the most prevalent harmful
coercion identified by Pradel & Sen (2015). If `=` already emits
`===` in lykn/kernel, this decision documents and reinforces it.

### Safe iteration: no `for...in`

**Decision**: lykn/surface never emits `for...in`. The `for` form
compiles to `for...of`. Object key iteration uses explicit `entries`,
`keys`, or `values` calls.

```lisp
;; Iterate values
(for (x items) (process x))
```

```javascript
for (const x of items) { process(x); }
```

```lisp
;; Iterate key-value pairs
(for ((array k v) (Object:entries obj))
  (console:log k v))
```

```javascript
for (const [k, v] of Object.entries(obj)) {
  console.log(k, v);
}
```

**ESTree nodes**: `ForOfStatement` (never `ForInStatement`).

**Rationale**: `for...in` traverses the prototype chain, making
prototype pollution payloads visible as iteration values. It is
the source of DLint's L2 checker findings and a well-documented
antipattern. `for...of` with explicit `Object.entries()` /
`Object.keys()` is safer and more explicit.

### No `eval` in surface language

**Decision**: lykn/surface provides no `eval` form. The reader/surface
compiler rejects `eval` as a form head. `new Function()` is similarly
rejected. The only path to eval is `(js:eval code)`, which is the
explicit escape hatch.

**Rationale**: `eval` and `new Function()` are the two primary code
injection vectors in JavaScript. Eliminating them at the language
level means code injection requires bypassing the lykn compiler.
The macro system uses `new Function()` internally (DD-11, DD-14)
at *compile time* in a sandboxed environment — this is unaffected.
The restriction applies to *runtime* eval in compiled output only.

## Rejected Alternatives

### `def` for immutable bindings

**What**: Use `def` (Clojure convention) as the binding form.

**Why rejected**: `def` collides conceptually with the `def-` prefix
family (`defn`, `defmacro`, `deftype`). Since lykn/surface uses
standalone names (`func`, `macro`, `type`) rather than the `def-`
prefix pattern, `def` is an orphan — it suggests a naming convention
that doesn't exist. `bind` is more descriptive and has no prefix
collision.

### `set` for immutable bindings

**What**: Use `set` as the binding form.

**Why rejected**: `set` implies mutation in most programming languages
(Clojure's `set!`, Python's sets, C's assignment). Using it for
immutable bindings creates cognitive dissonance. `bind` accurately
describes what happens — a name is bound to a value.

### `val` for immutable bindings

**What**: Use `val` (Kotlin/Scala convention) for immutable bindings.

**Why rejected**: No strong objection — `val` was a viable candidate.
`bind` was preferred because it is more descriptive (binding a name
to a value), has Lisp heritage (Scheme's binding forms), and `val`
was considered as a potential name for the cell-read operation before
`express` was chosen.

### `const` for immutable bindings

**What**: Use `const` (JS convention) in the surface language.

**Why rejected**: Semantic overloading. `const` means different things
in C (compile-time constant), C++ (immutable reference), JavaScript
(non-reassignable binding to potentially mutable value), and Rust
(`const` vs `let`). `bind` has a single, clear meaning.

### `static` for immutable bindings

**What**: Use `static` as the immutable binding form.

**Why rejected**: DD-07 already uses `static` as a wrapper for class
members. Collision within lykn itself. Also, `static` implies
compile-time allocation in C/C++, which is unrelated.

### `var` / `bind!` for mutable bindings

**What**: Provide a reassignable binding form alongside `cell`.

**Why rejected**: Two mutation mechanisms create ambiguity about which
to use. Cell-only mutation provides a single, consistent path. The
slight ceremony of `cell`/`express`/`swap!` is intentional friction
that pushes developers toward functional solutions. The patterns
where `bind!` would be most convenient (indexed loops, mutable
accumulators) are exactly the patterns a functional language wants
to discourage.

### `atom` for mutable containers

**What**: Use Clojure's `atom` name for mutable containers.

**Why rejected**: In every Lisp except Clojure, "atom" means "not a
list" (McCarthy's 1960 definition). Repurposing it for mutable
containers creates confusion for developers with Lisp background.

### `ref` for mutable containers

**What**: Use ML's `ref` name.

**Why rejected**: `ref` is heavily used in React and Vue for reactive
references. While the concepts are spiritually similar, the API
semantics differ. The overlap would confuse frontend developers.

### `box` for mutable containers

**What**: Use Racket's `box` name.

**Why rejected**: Rust's `Box<T>` is a heap allocation wrapper, not
a mutable cell. Given that lykn borrows from Rust conventions
elsewhere, the naming collision would cause confusion.

### `deref` for cell read operation

**What**: Use Clojure's `deref` to read cell values.

**Why rejected**: `deref` (dereference) is technically inaccurate.
Dereferencing means following a pointer to a memory location — a
C/Rust concept. What lykn cells do is property access (`cell.value`).
Clojure's usage is metaphorical (borrowed from its JVM
`AtomicReference` backing). `express` is more accurate — from
biology's gene expression: reading information from a cell and
making it available without altering the cell.

### `defn` / `deftype` / `defmacro` prefix family

**What**: Use the traditional Lisp `def-` prefix for definition forms.

**Why rejected**: lykn/surface uses short English words as form heads:
`func`, `type`, `macro`, `bind`. Each form has a unique name without
a shared prefix. This is more consistent with lykn's design principle
of JS-aligned naming over Lisp conventions. The `def-` family was
released — `def` is not reserved.

### `object` in surface language

**What**: Use the full word `object` in lykn/surface.

**Why rejected**: `obj` is shorter, visually distinguishes surface
from kernel, and creates a clear signal about which layer you're
working in. `object` remains as the kernel form.

### Slash namespace for JS interop (`js/call`)

**What**: Use `/` as the namespace separator for JS interop.

**Why rejected**: lykn uses `:` for member access (DD-01). `js:call`
is consistent with the existing colon syntax convention. `/` would
introduce a second namespace separator with different semantics.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `bind` with no value | Compile error | `(bind x)` → error: bind requires a value |
| `bind` to mutable JS value | Binding is `const`, but JS value may be mutable internally | `(bind arr (js:call Array:from items))` — `arr` can't be reassigned but array contents are mutable from JS |
| Keyword with no value after it | Compile error in `obj` context | `(obj :name)` → error: keyword :name has no value |
| Keyword as standalone expression | Compiles to string literal | `:name` → `"name"` |
| Keyword in non-object context | Compiles to string literal | `(console:log :hello)` → `console.log("hello")` |
| Keyword with camelCase | Flagged by linter, not an error | `:firstName` → `"firstName"` (linter suggests `:first-name`) |
| `express` on non-cell | Runtime error (no `.value` property) | `(express 42)` → `(42).value` → `undefined` |
| `swap!` on non-cell | Runtime error | `(swap! 42 inc)` → `(42).value = inc(...)` → TypeError |
| Nested cells | Supported but discouraged | `(bind c (cell (cell 0)))` → `{ value: { value: 0 } }` |
| `cell` with no initial value | Compile error | `(cell)` → error: cell requires an initial value |
| `func` with `:pre` but `:any` types | Valid — contracts work alongside `:any` | `(func f :args (:any x) :pre (> x 0) :body ...)` |
| `func` with types but no `:pre` | Valid — type checks only | `(func f :args (:number x) :body ...)` |
| `func` with bare param (no type) | Compile error | `(func f :args (x) :body ...)` → error: parameter 'x' missing type annotation |
| `fn` with types | Required — all params must have types | `(fn (:number x) (+ x 1))` |
| `fn` with bare param (no type) | Compile error | `(fn (x) (+ x 1))` → error: parameter 'x' missing type annotation |
| `fn` with contracts | Not supported — contracts require a name for error messages | `(fn (:any x) :pre ...)` → error |
| `lambda` anywhere `fn` works | Identical behavior | `(lambda (:number x) (+ x 1))` = `(fn (:number x) (+ x 1))` |
| `bind` with type on literal | Valid but redundant | `(bind :string name "Duncan")` |
| `bind` without type on literal | Valid — literal is self-typing | `(bind name "Duncan")` |
| `bind` without type on variable ref | Open question — compiler error or linter warning | `(bind b a)` — see open questions |
| Surface form used in kernel context | Works if macro expansion is active | Kernel compiler alone does not recognize surface forms |
| Kernel form used in surface context | Permitted — surface is a superset of kernel | `(const x 42)` compiles normally through surface compiler |

## Dependencies

- **Depends on**: DD-01 (colon syntax — leading `:` reserved, now
  activated for keywords; colon splitting for `js:` namespace),
  DD-02 (function forms — `function` as kernel target for `func`),
  DD-04 (modules — `import`/`export` unchanged in surface),
  DD-06 (destructuring — `object` with grouped pairs as kernel
  target for `obj`), DD-10 through DD-14 (macro system — surface
  forms are implemented as macros in the expansion pipeline)
- **Affects**: DD-01 (amends leading `:` from "reserved" to "active
  — keyword type"), DD-12 (amends `#o(...)` to accept keyword-value
  alternation alongside grouped pairs), DD-13 (dispatch table gains
  entries for surface forms: `bind`, `func`, `fn`, `lambda`, `obj`,
  `cell`, `express`, `swap!`, `reset!`, `match`, `type`, threading
  macros). Future DDs: DD-16 (`func` detailed design), DD-17
  (`type` + `match` detailed design), DD-18 (threading macros),
  DD-19 (contracts detailed design), DD-20 (Rust surface compiler
  architecture)

## Open Questions

- [ ] `@` as reader-level deref syntax for `express` — deferred to
  a future DD. Available if community demand emerges.
- [ ] Deref syntax in quasiquote context — does `@` conflict with
  `,@` (unquote-splicing)?  Needs investigation if `@` is activated.
- [x] Type annotation syntax for `bind` — resolved in DD-15 v1.1.
  `(bind :type name value)` with type keyword between `bind` and
  the name. Optional for primitive literal initializers, open
  question for non-literal initializers (see below).
- [x] `func` detailed parameter parsing — resolved in DD-16 v1.2.
  All parameters require type keywords. Bare symbols are a compile
  error. `:any` is the explicit opt-out.
- [x] `%` in `:post` contracts — resolved in DD-16. `~` is the
  return-value placeholder (not `%`). `%` was rejected as easily
  confused with modulo.
- [x] `ContractError` — resolved in DD-16. Contract violations use
  standard `Error` with structured message format. No custom class,
  no runtime dependency. Type violations use `TypeError`.
- [ ] Cell interop — should `express`, `swap!`, `reset!` work on
  any object with a `.value` property, or only on objects created
  by `cell`? Affects interop with reactive frameworks.
- [ ] `obj` with spread — syntax for spreading another object into
  an `obj` form. `(obj :name "Duncan" (spread defaults))`?
- [ ] `assoc`/`dissoc`/`conj` detailed semantics — shallow copy
  depth, nested updates, array operations. Needs own DD or section
  in a future DD.
- [ ] Rust surface compiler architecture — module structure, AST
  representation, interface with JS kernel compiler. Needs DD-20.
- [ ] How kernel forms in surface context interact with surface
  analysis — does the Rust compiler pass kernel forms through
  unanalyzed, or does it understand them?
- [x] Production vs dev mode — resolved in DD-16. `--strip-assertions`
  CLI flag. Default is dev mode (assertions enabled).
- [ ] `bind` type annotation enforcement for non-literal initializers —
  should `(bind b a)` (variable reference without type) be a compile
  error or a linter warning? Compile error is consistent with `func`
  and `type` (every value crossing a boundary has a type). Linter
  warning is less friction for local bindings. Deferred to Rust
  compiler design session (DD-20).

## Version History

### v1.1 — 2026-03-28

Added `bind` type annotation decision. Type annotations optional for
primitive literal initializers (self-typing), syntax is
`(bind :type name value)`. Whether non-literal initializers require
type annotations deferred as open question to Rust compiler design
session. Edge cases updated to reflect DD-16 v1.2 (required types on
`func`/`fn`/`lambda` params, no bare symbols) and DD-19 (single
expression `:pre`/`:post`, no vectors). Resolved open questions
marked with `[x]`: type annotation syntax for `bind`, `func`
parameter parsing, `%` → `~` placeholder, `ContractError` → standard
`Error`, `--strip-assertions` CLI flag.

### v1.0 — 2026-03-27

Initial version.