# Core lykn Idioms

Essential lykn idioms for writing clean, idiomatic surface code. These
patterns leverage lykn's surface language — immutable bindings, required
type annotations, controlled mutation via cells, strict equality by
default, and short-circuit logical operators — to produce clean, safe
JavaScript with no runtime dependencies.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: `bind` by Default, `cell` When Mutation Is Needed

**Strength**: MUST (compiler-enforced)

**Summary**: Use `bind` for all values. When you genuinely need mutable
state, wrap the initial value in `cell` and mutate via `swap!` or
`reset!`.

```lykn
;; Good — immutable bindings
(bind max-retries 3)
(bind users #a())

;; Good — controlled mutation via cell
(bind counter (cell 0))
(swap! counter (fn (:number n) (+ n 1)))
(console:log (express counter))
```

```
1
```

**Rationale**: lykn eliminates JS's `const`/`let`/`var` distinction
entirely. `bind` always emits `const` — the binding is immutable. When
you need state that changes over time, `cell` makes mutation explicit
and auditable: every mutation site is marked with `!` (`swap!`,
`reset!`), and reading a cell requires `express`. There is no silent
reassignment.

**See also**: ID-14, `04-values-references.md` ID-01

---

## ID-02: `=` Means Equality, Not Assignment

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, confusing `=` (assignment) with `===`
(equality) is a pervasive source of bugs.

lykn eliminates this entirely: `(= a b)` compiles to `a === b` (strict
equality). There is no assignment operator in surface syntax — all
mutation goes through named forms (`bind`, `reset!`, `swap!`). Loose
equality (`==`) is only available via the `js:eq` escape hatch for the
`== null` idiom. See DD-22 and `00-lykn-surface-forms.md`.

---

## ID-03: Use `??` for Nullish Defaults

**Strength**: SHOULD

**Summary**: Use nullish coalescing `??` when providing default values.
Use `or` only for boolean logic.

```lykn
;; Good — ?? only triggers on null/undefined
(bind timeout (?? options:timeout 5000))
(bind title (?? options:title "Untitled"))
(bind verbose (?? options:verbose true))

;; Bad — or treats 0, "", false as falsy
(bind timeout (or options:timeout 5000))
```

**Rationale**: `or` compiles to `||`, which returns the right-hand side
for any falsy value (including `0`, `""`, `false`, `NaN`). `??` returns
the right-hand side only for `null` or `undefined`. When `0`, `""`, or
`false` are legitimate values, `or` silently discards them.

**See also**: ID-04, ID-22

---

## ID-04: Use `some->` for Safe Property Access

**Strength**: SHOULD

**Summary**: Use `some->` (nil-safe thread-first) to safely traverse
potentially nullish values. Combine with `??` for defaults.

```lykn
;; Good — nil-safe threading
(bind street (?? (some-> person :address :street) "(unknown)"))
(bind len (some-> arr :length))
```

```lykn
;; Bad — verbose guard chains in kernel syntax
(if (and person person:address)
  person:address:street
  null)
```

**Threading forms** (see `00-lykn-surface-forms.md`):

| Form | Inserts at | Null-safe |
|------|-----------|-----------|
| `->` | First arg | No |
| `->>` | Last arg | No |
| `some->` | First arg | Yes (`== null` check at each step) |
| `some->>` | Last arg | Yes |

**Do not overuse** — scattering `some->` throughout code can silently
swallow nulls. Prefer normalizing data shapes at ingestion boundaries.

**See also**: `00-lykn-surface-forms.md` Threading Macros

---

## ID-05: Use `template` for String Interpolation

**Strength**: SHOULD

**Summary**: Use the `template` form for any string that embeds
expressions. Use plain strings for static text.

```lykn
;; Good — template with interpolation
(bind msg (template "Hello, " name "! You have " count " items."))

;; Good — plain string for static text
(bind greeting "Hello, world")
```

```lykn
;; Bad — string concatenation via +
(bind msg (+ "Hello, " name "! You have " count " items."))
```

```lykn
(bind name "world")
(bind count 3)
(bind msg (template "Hello, " name "! You have " count " items."))
(console:log msg)
```

```
Hello, world! You have 3 items.
```

**Rationale**: `template` compiles to JS template literals, which are
more readable and support any expression in interpolation positions.
Concatenation with `+` is error-prone (the `+` operator doubles as
numeric addition) and harder to scan visually.

**See also**: `00-lykn-surface-forms.md` Expressions

---

## ID-06: Destructure at the Point of Use

**Strength**: SHOULD

**Summary**: Extract needed properties using destructuring patterns in
`const` (kernel) or in function parameters.

```lykn
;; Good — destructure in binding
(const (object name email (default role "member")) user)

;; Good — destructure array
(const (array first (rest tail)) items)

;; Good — destructure in loop
(for-of (array index value) (arr:entries)
  (console:log (template index ": " value)))
```

```lykn
;; Bad — manual extraction
(bind name user:name)
(bind email user:email)
(bind role (or user:role "member"))
```

**Destructuring patterns** (`00-lykn-surface-forms.md`):

| Pattern | lykn | JS |
|---------|------|-----|
| Object | `(object name age)` | `{name, age}` |
| Alias | `(object (alias data items))` | `{data: items}` |
| Default | `(object (default x 0))` | `{x = 0}` |
| Array | `(array first second)` | `[first, second]` |
| Rest | `(array first (rest tail))` | `[first, ...tail]` |
| Skip | `(array _ _ third)` | `[, , third]` |

**Rationale**: Destructuring is concise, self-documenting (names are
visible in the pattern), and enables default values. Defaults trigger
only on `undefined`, not `null`.

---

## ID-07: Use Named Exports, Avoid Default Exports

**Strength**: SHOULD

**Summary**: Prefer named exports. Reserve default exports only for
modules with a single, obvious purpose.

```lykn
;; Good — named exports
(export (func format-date
  :args (:any d) :returns :string
  :body (d:toISOString)))

(export (func parse-date
  :args (:string s) :returns :any
  :body (new Date s)))

(export (bind DATE-FORMAT "YYYY-MM-DD"))
```

```lykn
;; Bad — default export
(export default format-date)
;; Importers can name it anything — format-date, fmt, x...
```

**Rationale**: Named exports enforce a consistent name across the
codebase, enable tree-shaking, and support IDE autocomplete. Default
exports let importers choose arbitrary names, making search and
refactoring harder.

---

## ID-08: ESM Only — No CommonJS, No `require()`

**Strength**: MUST

**Summary**: Use ECMAScript modules exclusively. lykn compiles to ESM.

```lykn
;; Good — ESM imports
(import "./utils.js" (format-date))
(import "@std/path" (join))

(bind data (await (Deno:readTextFile (join "config" "app.json"))))
(export (func process-data :args (:any input) :returns :any :body input))
```

**Module characteristics**:
- Automatic strict mode
- Own scope (top-level declarations are not global)
- Singleton execution (code runs once, on first import)
- Static structure enables tree-shaking
- Live bindings (imports reflect current export values)

**Rationale**: ESM is the JavaScript standard. lykn only produces ESM
output. CommonJS is not supported.

---

## ID-09: `func` for Named Functions, `fn` for Callbacks

**Strength**: SHOULD

**Summary**: Use `func` for named, module-level functions with type
annotations and contracts. Use `fn` (or `lambda`) for inline callbacks.

```lykn
;; Good — func for named functions (typed, can have contracts)
(func parse-config
  :args (:string raw)
  :returns :object
  :body (JSON:parse raw))

;; Good — fn for inline callbacks (typed arrow)
(bind doubled (items:map (fn (:number x) (* x 2))))

;; Good — fn for event handlers
(button:addEventListener "click"
  (fn (:any e) (handle-click e)))
```

**Function forms** (`00-lykn-surface-forms.md`):

| Form | Use for | Output |
|------|---------|--------|
| `func` | Named functions with types/contracts | `function` declaration |
| `fn` | Typed anonymous functions | Arrow function |
| `lambda` | Alias for `fn` | Arrow function |
| `=>` (kernel) | Untyped anonymous functions | Arrow function |

```lykn
(func double
  :args (:number x)
  :returns :number
  :body (* x 2))
(console:log (double 21))

(bind items #a(1 2 3 4))
(bind doubled (items:map (fn (:number x) (* x 2))))
(console:log doubled)
```

```
42
[ 2, 4, 6, 8 ]
```

**Rationale**: `func` provides runtime type checking, contracts
(`:pre`/`:post`), and multi-clause dispatch. `fn` is concise for
callbacks while still enforcing type annotations. Use `:any` as the
explicit opt-out when types are impractical.

**See also**: `06-functions-closures.md`

---

## ID-10: No `this` in Surface Code

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, `this` binding is one of the most common
sources of bugs — its value depends on how a function is called, not
where it is defined.

lykn eliminates this entirely: `this` does not exist in the surface
language vocabulary. State is managed through `cell` containers (for
local state) or through `func` parameters (for passed state). When
interacting with JS APIs that require `this` (e.g., class methods),
use the kernel `class` form where `this` is available. See
`00-lykn-surface-forms.md` Classes.

---

## ID-11: Document All Magic Values

**Strength**: MUST

**Summary**: Every literal number, string, or value that isn't
self-evident must be named via `bind`.

```lykn
;; Good — named constants with comments
;; Maximum retry attempts before circuit-breaker opens.
(bind MAX-RETRIES 3)

;; HTTP 429 Too Many Requests — back off and retry.
(bind TOO-MANY-REQUESTS 429)

(if (= response:status TOO-MANY-REQUESTS)
  (await (backoff attempt MAX-RETRIES)))
```

```lykn
;; Bad — unexplained magic numbers
(if (= response:status 429)
  (await (backoff attempt 3)))
```

**Rationale**: Magic values obscure intent, make code harder to search,
and resist refactoring. Named bindings are self-documenting, searchable,
and changeable in one place.

---

## ID-12: Naming — `lisp-case` for Values, `PascalCase` for Types

**Strength**: MUST

**Summary**: Use lisp-case for all identifiers. The compiler auto-converts
to camelCase in the JS output. Use PascalCase only for `type`
constructors and `class` names.

```lykn
;; Good
(bind max-retries 3)
(bind MAX-CONNECTIONS 100)
(func compute-total :args (:array items) :returns :number :body
  (items:reduce (fn (:number a :number b) (+ a b)) 0))
(type HttpClient (Client :string base-url))

;; Bad
(bind maxRetries 3)       ;; camelCase — let the compiler do this
(bind max_retries 3)      ;; snake_case — not lykn convention
(func ComputeTotal ...)   ;; PascalCase implies a type constructor
```

**Naming conventions**:

| Element | Convention | Example |
|---------|-----------|---------|
| Bindings, functions | lisp-case | `get-user-name`, `item-count` |
| Type constructors | PascalCase | `HttpClient`, `Some`, `None` |
| Module-level constants | UPPER-lisp-case | `MAX-RETRIES`, `API-BASE-URL` |
| Module filenames | kebab-case | `date-utils.lykn` (surface), `helpers.lyk` (kernel) |
| Predicates | `?` suffix | `even?`, `valid?`, `has-items?` |
| Mutation operations | `!` suffix | `swap!`, `reset!` |

**Rationale**: lykn automatically converts `lisp-case` to `camelCase`
in compiled output. Writing `camelCase` in lykn source produces
double-cased output. PascalCase is reserved for constructors created
by `type` — using it for functions sends a false signal.

---

## ID-13: Avoid Weasel Words in Names

**Strength**: MUST

**Summary**: Use specific, descriptive names. Avoid generic terms that
add syllables without meaning.

```lykn
;; Good — specific names
(func validate-email :args (:string input) :returns :boolean :body
  (input:includes "@"))
(func build-user-query :args (:object filters) :returns :string :body
  (template "SELECT * FROM users WHERE " (serialize filters)))

;; Bad — weasel words
(func handle-data :args (:any data) :returns :any :body data)
(func process-info :args (:any info) :returns :any :body info)
```

**Words to avoid as primary identifiers**: `Manager`, `Service`,
`Handler`, `Helper`, `Utils`, `Data`, `Info`, `Process`, `Handle`,
`Do`, `Perform`, `Execute` (when used generically).

**Rationale**: Vague names force readers to inspect the implementation
to understand the code. Specific names enable scanning and reasoning
without reading function bodies.

---

## ID-14: `bind` IS Immutable — Use `cell` for Mutable State

**Strength**: MUST (compiler-enforced)

**Summary**: Unlike JS's `const` (which freezes the binding but not the
value), lykn's `bind` produces a truly immutable binding. For mutable
state, use `cell` explicitly.

```lykn
;; bind is immutable — the binding cannot change
(bind config (obj :debug false))
;; config cannot be reassigned — there is no assignment operator

;; For mutable state, use cell
(bind counter (cell 0))
(swap! counter (fn (:number n) (+ n 1)))
(console:log (express counter))

;; For object updates, use assoc (returns a new object)
(bind updated-config (assoc config :debug true))
```

**Three strategies for state**:

| Strategy | Form | When to use |
|----------|------|-------------|
| Immutable binding | `bind` | Default — most values |
| Immutable update | `assoc`/`dissoc`/`conj` | Updating objects/arrays |
| Controlled mutation | `cell` + `swap!`/`reset!` | Counters, accumulators, caches |

**Rationale**: In JS, `const config = { debug: false }` allows
`config.debug = true` — the value is still mutable. lykn's `bind`
compiles to `const`, but the surface language steers you toward
immutable updates (`assoc`) instead of property mutation. When you
genuinely need mutation, `cell` makes every mutation site auditable
with `!`-suffixed operators.

**See also**: ID-01, `04-values-references.md`

---

## ID-15: Prefer `Object:freeze` for Truly Constant Objects

**Strength**: CONSIDER

**Summary**: When you need a JS object whose properties cannot be
modified at runtime, use `Object:freeze`.

```lykn
;; Good — truly immutable configuration
(bind CONFIG (Object:freeze (obj
  :max-retries 3
  :timeout 5000
  :base-url "https://api.example.com")))
```

**Rationale**: `bind` prevents reassignment of the binding, but the
underlying JS object can still be mutated by code that receives it.
`Object:freeze` prevents property modification at the JS level. Note
it is shallow — nested objects are not frozen. For most lykn code,
prefer `assoc`/`dissoc` for updates rather than mutating objects
in place.

**See also**: ID-14

---

## ID-16: Prefer `Map`/`Set` over `obj` for Dynamic Collections

**Strength**: SHOULD

**Summary**: Use `Map` for key-value collections and `Set` for
unique-value collections when keys are dynamic or non-string.

```lykn
;; Good — Map for dynamic keys
(bind cache (new Map))
(cache:set user-obj (compute-expensive-result user-obj))
(cache:set 42 "forty-two")

;; Good — Set for uniqueness
(bind visited (new Set))
(visited:add url)
(if (visited:has url) (console:log "skip"))
```

**When to use `obj`**: For fixed-shape records with known string keys
(configuration, DTOs, JSON-compatible data).

**Rationale**: `Map` accepts any key type, provides a `:size` property,
iterates in insertion order, and has no prototype pollution risk. `obj`
coerces all keys to strings via the keyword system.

---

## ID-17: Use `rest` for Variadic Parameters

**Strength**: MUST

**Summary**: Use `(rest ...)` in parameter lists for variadic functions.
The `arguments` object does not exist in lykn.

```lykn
;; Good — rest parameter produces a real Array
(function find-max (first (rest others))
  (bind result (cell first))
  (for-of n others
    (if (> n (express result))
      (reset! result n)))
  (return (express result)))
```

**Rationale**: Rest parameters produce a real `Array`, are visible in
the function signature, and work in all function forms. The `arguments`
object does not exist in lykn — it is a JS legacy that is unavailable
in arrow functions and hides arity.

---

## ID-18: Use `spread`, `assoc`, and `conj` for Non-Destructive Updates

**Strength**: SHOULD

**Summary**: Use `assoc` for immutable object updates, `conj` for
immutable array appends, and `spread` (kernel) for lower-level
expansion.

```lykn
;; Good — immutable object update
(bind defaults (obj :timeout 5000 :retries 3))
(bind config (assoc defaults :timeout 10000 :debug true))

;; Good — immutable array append
(bind items #a(1 2 3))
(bind more-items (conj items 4))

;; Good — merge arrays via kernel spread
(bind merged (array (spread arr1) (spread arr2)))
```

**Rationale**: `assoc` and `conj` are the idiomatic surface forms for
non-destructive updates. They make the intent clear — a new value is
being created, not mutating the original. For spread in function calls,
use the kernel `spread` form inside `array`.

**See also**: `04-values-references.md`

---

## ID-19: Implicit Returns in `fn` and `func`

**Strength**: SHOULD

**Summary**: In `func` and `fn`, the last expression is automatically
returned. Do not add explicit `return` — it is a kernel form.

```lykn
;; Good — implicit return in fn
(bind double (fn (:number x) (* x 2)))
(bind is-even (fn (:number n) (= (% n 2) 0)))

;; Good — implicit return in func
(func get-name :args (:object user) :returns :string
  :body user:name)

;; Good — multi-expression body, last is returned
(func process-item :args (:string item) :returns :string :body
  (bind normalized (item:trim))
  (bind lower (normalized:to-lower-case))
  lower)
```

**Rationale**: lykn's surface functions handle returns automatically.
The last expression in `:body` becomes the return value. In `fn`, the
body expression(s) are placed in the arrow function body. Explicit
`return` is only needed in kernel forms like `function` and `lambda`.

---

## ID-20: No Node.js — Use Deno APIs and Web Platform APIs

**Strength**: MUST

**Summary**: Target Deno as the runtime. Use Web Platform APIs (`fetch`,
`URL`, `crypto`, etc.) and Deno namespace APIs.

```lykn
;; Good — Deno/Web Platform APIs
(bind response (await (fetch "https://api.example.com/data")))
(bind text (await (Deno:readTextFile "./config.json")))
(bind url (new URL "/path" "https://example.com"))
```

**Deno-specific conventions**:
- File extensions required on local imports: `(import "./calc.js" (add))`
- Permissions are explicit: `deno run --allow-read --allow-net script.js`
- Configuration in `deno.json`
- Use `jsr:` specifiers for JSR packages, `npm:` for npm packages
- lykn source compiles to JS first: `lykn compile main.lykn -o main.js`

**See also**: `12-deno/01-runtime-basics.md`, `14-no-node-boundary.md`

---

## ID-21: `bind` in Loops — `for-of` Is Always Immutable

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, you must choose between `const` and `let`
in loop heads, and using `const` in a `for` loop counter is an error.

lykn eliminates this entirely: `for-of` always creates an immutable
binding for each iteration. For C-style `for` loops (which use `let`
counters), use the kernel `for` form. In practice, `for-of` covers
the vast majority of iteration needs.

```lykn
;; for-of — each iteration gets a fresh immutable binding
(for-of item items
  (console:log item))

;; C-style for loop (kernel form — let counter is implicit)
(for (let i 0) (< i items:length) (++ i)
  (console:log (get items i)))
```

---

## ID-22: Prefer Explicit Checks over Truthiness

**Strength**: SHOULD

**Summary**: Use truthiness checks (`if x`) only when all falsy values
should be excluded. Use explicit checks when `0`, `""`, or `false` are
valid.

```lykn
;; Good — truthiness is appropriate (exclude null, undefined, "")
(if error-message
  (display-error error-message))

;; Bad — truthiness discards valid values
(func set-count :args (:any count) :returns :number :body
  (if (not count) 10 count))
;; fails when count is 0!

;; Good — explicit nullish check preserves 0
(func set-count :args (:any count) :returns :number :body
  (?? count 10))
```

**The 7 falsy values** (these are JS facts — lykn compiles to JS):
`false`, `0`, `-0`, `0n`, `NaN`, `""`, `null`, `undefined`

**Everything else is truthy**, including: `#a()` (empty array), `(obj)`
(empty object), `"0"`, `"false"`.

**Rationale**: Truthiness checks are concise but imprecise. When `0`,
`""`, or `false` are valid domain values, a truthiness check silently
discards them. Use `??` or explicit `(= x null)` checks instead.

---

## ID-23: Use `structuredClone` for Deep Copies

**Strength**: SHOULD

**Summary**: Use `structuredClone` for deep copies. Use `assoc` or
spread for shallow copies.

```lykn
;; Good — shallow copy via spread
(bind copy (object (spread original)))

;; Good — deep copy
(bind deep (structuredClone original))
```

**Shallow vs. deep**:

```lykn
;; Shallow — nested objects are shared
(bind original (obj :work (obj :employer "Acme")))
(bind shallow (object (spread original)))
;; shallow:work is the same reference as original:work

;; Deep — fully independent
(bind deep (structuredClone original))
```

**Rationale**: `structuredClone` handles circular references, `Date`,
`RegExp`, `Map`, `Set`, and more. `(object (spread obj))` produces a
shallow copy via spread.

**See also**: ID-24, `04-values-references.md`

---

## ID-24: Immutability Is the Default — Embrace It

**Strength**: SHOULD

**Summary**: lykn makes immutability the default. Lean into it — use
`assoc`/`dissoc`/`conj` for updates instead of mutating in place.

```lykn
;; Good — non-destructive update (original unchanged)
(bind original (obj :city "Berlin" :country "Germany"))
(bind updated (assoc original :city "Munich"))

;; Good — remove a field without mutation
(bind sanitized (dissoc user :password))

;; Good — append without mutation
(bind with-new (conj items new-item))
```

**When mutation is appropriate**: Use `cell` for counters,
accumulators, and caches where the performance cost of immutable
updates is unacceptable or the code becomes unreadable.

**Rationale**: Shared mutable state is the root of many bugs. lykn's
surface language eliminates accidental mutation: `bind` is immutable,
`assoc`/`dissoc`/`conj` create new values, and `cell` makes mutation
explicit. When one function receives data from another, the data
cannot be silently modified.

**See also**: ID-14, `04-values-references.md`

---

## ID-25: Prefer `for-of` for Iteration

**Strength**: SHOULD

**Summary**: Use `for-of` for iteration. It supports `break`,
`continue`, and `await` — `.forEach()` does not.

```lykn
;; Good — for-of supports break, continue
(for-of item items
  (if item:skip (continue))
  (if item:done (break))
  (process item))

;; Good — for-of with index via entries
(for-of (array i item) (items:entries)
  (console:log (template i ": " item)))

;; Good — for-of with await (sequential execution)
(for-of item items
  (await (process item)))
```

**Rationale**: `for-of` is a statement, not a method call — it supports
`break`, `continue`, `return`, and `await`. `.forEach()` cannot be
terminated early, does not support `await` correctly, and adds a
function scope that obscures control flow.

---

## ID-26: Prefer Early Returns to Reduce Nesting

**Strength**: SHOULD

**Summary**: Use guard clauses with early returns to handle edge cases
at the top, keeping the main logic at a low nesting level.

```lykn
;; Good — guard clauses, flat main logic
(func process-user :args (:any user) :returns :any :body
  (if (not user) (throw (new Error "user is required")))
  (if (not user:active) null)
  (if (not user:email) null)
  (bind normalized (user:email:to-lower-case))
  (send-welcome normalized))
```

```lykn
;; Bad — deeply nested conditionals
(func process-user :args (:any user) :returns :any :body
  (if user
    (if user:active
      (if user:email
        (send-welcome (user:email:to-lower-case))
        null)
      null)
    null))
```

**Rationale**: Each level of nesting increases cognitive load. Guard
clauses invert conditions and exit early, leaving the happy path at
the top level of indentation.

---

## ID-27: Use Keywords for Object Keys

**Strength**: MUST

**Summary**: In `obj`, `assoc`, and `dissoc`, always use keywords
(`:name`) for keys. Keywords auto-convert to camelCase string keys.

```lykn
;; Good — keywords for keys
(bind user (obj :first-name "Alice" :last-name "Smith" :age 30))
(console:log user:first-name)
(bind updated (assoc user :age 31))
(console:log updated:age)
(bind public (dissoc user :age))
(console:log public:first-name)
(console:log public:age)
```

```
Alice
31
Alice
undefined
```

**Rationale**: Keywords are compile-time markers, not runtime values.
They provide a clean, consistent syntax for object construction and
update. The lisp-case to camelCase conversion applies to keywords
just as it does to identifiers.

**See also**: `00-lykn-surface-forms.md` Keywords

---

## ID-28: Colon Syntax for Member Access

**Strength**: MUST

**Summary**: Use colon syntax (`obj:prop`) for property access. This
compiles to `.` access with automatic camelCase conversion.

```lykn
;; Good — colon syntax
(console:log "hello")
(bind len items:length)
(bind name user:first-name)
(bind result (Math:floor (* (Math:random) 100)))

;; For computed access, use get
(bind item (get items 0))
(bind val (get obj key))
```

**Rationale**: Colon syntax is lykn's replacement for JS's `.` operator.
It is more visually distinct in s-expression syntax and automatically
applies camelCase conversion. Use `get` for computed (dynamic) access
where the property name is a variable or expression.

**See also**: `00-lykn-surface-forms.md` Colon Syntax

---

## ID-29: Use `and`/`or`/`not` for Logic

**Strength**: MUST (compiler-enforced)

**Summary**: Use `and`, `or`, and `not` for logical operations. These
are short-circuit operators, not function calls.

```lykn
;; Good — surface logical operators
(if (and (> x 0) (< x 100))
  (console:log "in range"))

(bind result (or cached-value (compute-fresh)))

(if (not (valid? input))
  (throw (new Error "invalid input")))

;; Variadic — chain left-to-right
(if (and a b c d)
  (console:log "all truthy"))
```

**Rationale**: Every Lisp dialect uses `and`/`or`/`not` for logical
operations. These compile to `&&`/`||`/`!` with proper short-circuit
semantics. The kernel operators `&&`, `||`, `!` remain available but
are not idiomatic in surface code. See DD-22.

**See also**: ID-02, `00-lykn-surface-forms.md` Logical operators

---

## ID-30: Use `type` + `match` for Structured Data

**Strength**: SHOULD

**Summary**: Use `type` to define algebraic data types and `match` for
exhaustive pattern matching.

```lykn
;; Define a tagged union
(type Result
  (Ok :any value)
  (Err :string message))

;; Exhaustive pattern matching
(func handle-result :args (:any r) :returns :string :body
  (match r
    ((Ok v) (template "Success: " v))
    ((Err msg) (template "Error: " msg))))
```

**Pattern types** (`00-lykn-surface-forms.md`):

| Pattern | Example | Matches |
|---------|---------|---------|
| Wildcard | `_` | Anything |
| Literal | `42`, `"ok"` | Exact value |
| Binding | `x` | Anything, binds to `x` |
| ADT | `(Ok v)` | Tagged value, binds fields |
| Structural | `(obj :ok true :data d)` | Object shape, binds fields |
| Guard | `((Ok v) :when (> v 0) ...)` | ADT + condition |

```lykn
(type Result
  (Ok :any value)
  (Err :string message))

(func handle-result :args (:any r) :returns :string :body
  (match r
    ((Ok v) (template "Success: " v))
    ((Err msg) (template "Error: " msg))))

(console:log (handle-result (Ok 42)))
(console:log (handle-result (Err "not found")))
```

```
Success: 42
Error: not found
```

**Rationale**: `type` + `match` replaces the common JS pattern of
switch-on-string or if-chains with runtime-checked, exhaustive
matching. The compiler adds a `throw` fallback when patterns are
not exhaustive (unless the last pattern is `_` or a binding).

**See also**: `05-type-discipline.md`, `00-lykn-surface-forms.md`

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | `bind` default, `cell` when needed | MUST | `bind` is immutable; `cell` for controlled mutation |
| 02 | `=` means equality | ELIMINATED | DD-22: `(= a b)` compiles to `===` |
| 03 | `??` for nullish defaults | SHOULD | `or` discards `0`, `""`, `false` |
| 04 | `some->` for safe access | SHOULD | Nil-safe threading with `== null` checks |
| 05 | `template` for interpolation | SHOULD | Compiles to template literals |
| 06 | Destructure at point of use | SHOULD | `object`/`array` patterns in binding position |
| 07 | Named exports | SHOULD | Tree-shakeable, refactor-safe |
| 08 | ESM only | MUST | lykn compiles to ESM exclusively |
| 09 | `func` for named, `fn` for callbacks | SHOULD | `func` has contracts; `fn` is concise |
| 10 | No `this` | ELIMINATED | No `this` in surface language |
| 11 | Document magic values | MUST | Named `bind`s are searchable and changeable |
| 12 | lisp-case / PascalCase | MUST | lisp-case auto-converts to camelCase |
| 13 | Avoid weasel words | MUST | Specific names enable scanning |
| 14 | `bind` IS immutable | MUST | Use `cell` for mutation, `assoc` for updates |
| 15 | `Object:freeze` for constant objects | CONSIDER | Shallow — freeze recursively for deep |
| 16 | `Map`/`Set` over `obj` for dynamic keys | SHOULD | Any key type, no prototype pollution |
| 17 | `rest` parameters | MUST | Real Array, visible in signature |
| 18 | `assoc`/`conj`/`spread` for updates | SHOULD | Non-destructive, original unchanged |
| 19 | Implicit returns | SHOULD | Last expression is returned automatically |
| 20 | Deno APIs, not Node.js | MUST | Web Platform APIs, explicit permissions |
| 21 | `bind` in loops | ELIMINATED | `for-of` always creates immutable bindings |
| 22 | Careful truthiness checks | SHOULD | `0`, `""`, `false` are falsy but often valid |
| 23 | `structuredClone` for deep copies | SHOULD | Handles circular refs, Date, Map, Set |
| 24 | Immutability is the default | SHOULD | `assoc`/`dissoc`/`conj` for updates |
| 25 | `for-of` for iteration | SHOULD | Supports `break`, `continue`, `await` |
| 26 | Early returns to reduce nesting | SHOULD | Guard clauses keep the happy path flat |
| 27 | Keywords for object keys | MUST | `:name` in `obj`, `assoc`, `dissoc` |
| 28 | Colon syntax for member access | MUST | `obj:prop` compiles to `obj.prop` |
| 29 | `and`/`or`/`not` for logic | MUST | Short-circuit operators, not function calls |
| 30 | `type` + `match` for data | SHOULD | Exhaustive pattern matching on tagged unions |

---

## ID-31: Intermediate `bind` When Chaining on `express`

**Strength**: MUST

**Summary**: Method calls cannot be chained directly on `(express cell)`.
Bind the expressed value first, then call the method on the binding.

```lykn
;; Bad — compiles to listeners.value("push", fn) (function call, not method)
((express listeners):push fn)

;; Good — bind first, then call
(bind ls (express listeners))
(ls:push fn)
```

**Rationale**: The surface compiler treats `(expr:method arg)` as a
method call on `expr`, but `(express cell)` is a special form that
the compiler doesn't recognize as a chainable expression target.
The intermediate `bind` gives the compiler a named value to chain on.

---

## ID-32: Implicit Return in `fn` — Type Annotations Required

**Strength**: MUST

**Summary**: Multi-statement `fn` (arrow function) only gets implicit
return when type annotations are present on the parameters. Without
type annotations, the last expression becomes a bare statement.
Never combine explicit `(return ...)` with typed `fn` parameters —
the compiler adds its own `return`, producing `return return expr`.

```lykn
;; Bad — explicit return + typed params → double return in compiled output
(bind make-link (fn (:string url :string text)
  (bind display (text:substring 0 30))
  (return (+ "<a>" display "</a>"))))  ;; compiles to: return return "<a>...";

;; Good — typed fn with implicit return (compiler adds return)
(bind make-link (fn (:string url :string text)
  (bind display (text:substring 0 30))
  (+ "<a>" display "</a>")))

;; Good — single-expression fn (implicit return via JS arrow syntax)
(bind double (fn (:number x) (* x 2)))

;; Good — func always implicitly returns the last expression
(func make-link :args (:string url :string text) :returns :string
  :body
  (bind display (text:substring 0 30))
  (+ "<a>" display "</a>"))
```

**Rationale**: When type annotations are present, the compiler wraps the
`fn` body in a block with type assertions and adds `return` before
the final expression. Without annotations and with `--strip-assertions`,
this implicit return is lost. Always add type annotations to `fn`
parameters when the function needs to return a value from a
multi-statement body.

---

## ID-33: Use Literal Unicode Characters, Not Escape Sequences

**Strength**: MUST

**Summary**: Lykn strings do not process `\uNNNN` escape sequences.
Use the literal Unicode character directly in the source.

```lykn
;; Bad — \u2026 compiles to literal string "u2026"
(bind ellipsis "\u2026")

;; Good — use the actual character
(bind ellipsis "…")
```

**Rationale**: The Lykn reader treats `\u` as two ordinary characters
in a string literal, not as a Unicode escape. Since Lykn source files
are UTF-8, embed the character directly.

---

## ID-35: Escape Forward Slashes in `regex` Patterns

**Strength**: MUST

**Summary**: The `(regex "pattern" "flags")` form wraps the pattern
in `/pattern/flags`. Forward slashes inside the pattern must be
escaped as `\\/` (written `\\\\/` in the Lykn string) to avoid
prematurely terminating the regex literal.

```lykn
;; Bad — unescaped // terminates the regex early
(regex "https?://test" "gi")
;; Compiles to: /https?://test/gi  (syntax error)

;; Good — escaped forward slashes
(regex "https?:\\/\\/test" "gi")
;; Compiles to: /https?:\/\/test/gi
```

**Rationale**: The Lykn `regex` form emits a JS regex literal
(`/.../flags`). Inside a regex literal, `/` must be escaped as `\/`
to avoid being parsed as the closing delimiter. Since Lykn strings
do process `\\` → `\`, use `\\/` in the Lykn string to produce `\/`
in the output.

---

## ID-37: `:object` Excludes Functions — Use `:any` for "Any Object-Like"

**Strength**: MUST

**Summary**: Lykn's `:object` type annotation compiles to
`typeof x !== "object"`, which rejects functions (since
`typeof fn === "function"`). In JavaScript, functions ARE objects
(they have properties, can be stamped, etc.), but `:object` doesn't
accept them. Use `:any` when a parameter accepts both plain objects
and functions.

```lykn
;; Bad — rejects functions even though they're valid JS objects
(func stamp :args (:object obj) :returns :number :body ...)
;; throws: "stamp: arg 'obj' expected object, got function"

;; Good — accepts any value that can have properties
(func stamp :args (:any obj) :returns :number :body ...)
```

**Rationale**: JavaScript's `typeof` has a well-known quirk:
`typeof null === "object"` and `typeof fn === "function"`, even
though functions are objects. Lykn's `:object` follows `typeof`
semantics, so it excludes functions. When migrating JS code that
accepts "anything with properties," use `:any`.

---

## ID-38: Kernel `=` Is Top-Level Equality, Block-Level Assignment

**Strength**: MUST

**Summary**: In kernel (`.lyk`) files, `(= x 5)` at module top
level compiles to `x === 5` (equality), but inside function bodies,
`for` loops, `if` blocks, and `(block ...)` wrappers it compiles to
`x = 5` (assignment). Wrap top-level assignments in `(block ...)`.

```lykn
;; Bad — top-level = is equality
(let x 0)
(= x 42)          ;; x === 42 (no-op comparison)

;; Good — wrap in block for assignment
(let x 0)
(block (= x 42))  ;; { x = 42; }

;; Good — = is assignment inside function bodies
(function init ()
  (= x 42))       ;; x = 42
```

**Rationale**: The kernel compiler's top-level context treats `=` as
equality to match the surface compiler's behavior. Assignment only
activates inside statement-level contexts (function bodies, loops,
conditionals, blocks).

---

## ID-39: Kernel Object Methods Need Separated Function Definitions

**Strength**: SHOULD

**Summary**: The kernel `(object ...)` form doesn't support inline
method definitions. Define methods as standalone `function`
declarations, then reference them by name in the object literal.

```lykn
;; Bad — inline function in object doesn't parse correctly
(const mixin (object
  (show (function (x) ...))))

;; Good — separate functions, assemble object
(function _show (x) ...)
(function _hide () ...)
(const mixin (object (show _show) (hide _hide)))
```

**Rationale**: The kernel `(object (key value) ...)` form expects
simple key-value pairs. Inline function expressions are not
recognized as values — they're parsed as nested s-expressions.

---

## ID-40: Kernel Prototype Methods Need Named Function Expressions

**Strength**: MUST

**Summary**: When assigning methods to prototypes in kernel, use
named function expressions (not anonymous). The kernel compiler
treats the first symbol after `function` as the name.

```lykn
;; Bad — anonymous function expression
(block (= Foo:prototype:bar (function (x) (return x))))
;; Produces: function x()(return, x) — broken

;; Good — named function expression
(block (= Foo:prototype:bar (function _bar (x) (return x))))
;; Produces: function _bar(x) { return x; }
```

---

## ID-41: Intermediate Binding for Method Calls on `(get ...)`

**Strength**: MUST

**Summary**: Method calls cannot be chained directly on `(get ...)`
results in kernel. `(get obj key):method` compiles to
`obj[key]("method")` (function call with string arg), not
`obj[key].method()`. Use an intermediate `const` binding.

```lykn
;; Bad — method call on (get ...) result
((get obj key):push 4)      ;; obj[key]("push", 4) — wrong

;; Good — intermediate binding
(const arr (get obj key))
(arr:push 4)                ;; arr.push(4) — correct
```

---

## ID-42: Threading Macros Are Surface-Only

**Strength**: MUST

**Summary**: The threading macros `->`, `->>`, and `some->` are
surface forms and do NOT work in kernel (`.lyk`) files. In kernel
files, use intermediate `const` bindings for method chaining.

```lykn
;; Bad — threading macro in .lyk file compiles to _>(expr, ...)
(-> str (:replace re1 "") (:replace re2 ""))

;; Good — intermediate bindings in .lyk file
(const step1 (str:replace re1 ""))
(const step2 (step1:replace re2 ""))
```

**Rationale**: Surface macros are expanded by the surface compiler
before reaching the kernel. Kernel files bypass the surface pipeline
entirely, so threading macros are passed through as-is and produce
invalid JS.

---

## ID-38: Use `.lyk` (Kernel) for Dynamic Property Assignment

**Strength**: SHOULD

**Summary**: Surface `set!` requires a static property path (e.g.,
`obj:prop`). When you need computed/dynamic property assignment
(`obj[key] = value`), use a `.lyk` kernel file where `(= ...)` is
assignment, not equality.

```lykn
;; Bad — set! does not accept (get obj key)
;; This is in a .lykn file (surface syntax)
(set! (get obj key) value)  ;; compile error

;; Good — use kernel syntax in a .lyk file
;; This is in a .lyk file (kernel syntax)
(= (get target:prototype name) (get source:prototype name))
```

**Rationale**: Surface lykn intentionally limits mutation to named
paths for auditability. Kernel files give full JS assignment
semantics when needed. Use `.lyk` sparingly for modules that
genuinely require dynamic property manipulation.

---

## Related Guidelines

- **API Design**: See `02-api-design.md` for `func` contracts, keyword
  args, and return types
- **Error Handling**: See `03-error-handling.md` for `try`/`catch` and
  `match`-based error dispatch
- **Values & References**: See `04-values-references.md` for the full
  mutation model (`cell`, `assoc`, `dissoc`, `conj`)
- **Type Discipline**: See `05-type-discipline.md` for type annotations,
  contracts, and constructor validation
- **Functions & Closures**: See `06-functions-closures.md` for `func`
  contracts, `fn`/`lambda`, and multi-clause dispatch
- **Async & Concurrency**: See `07-async-concurrency.md` for `async`,
  `await`, and concurrency patterns
- **Anti-Patterns**: See `09-anti-patterns.md` for common lykn mistakes
  and patterns to avoid
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for the
  complete surface form catalog
- **Deno**: See `12-deno/01-runtime-basics.md` for runtime configuration
- **No-Node Boundary**: See `14-no-node-boundary.md`
