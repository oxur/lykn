# Functions & Closures

The heart of lykn composition. Covers function forms (`func`, `fn`,
`lambda`), closure mechanics, scope, higher-order functions, threading
macros (lykn's native pipe/compose), partial application, generators,
and pure function discipline. lykn eliminates `this` from the surface
language entirely, removing an entire category of JavaScript bugs.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: `func` for Named Module-Level Functions

**Strength**: SHOULD

**Summary**: Use `func` for named, module-level functions. `func`
provides type annotations, contracts, and multi-clause dispatch.

```lykn
;; Good — named function with types
(export (func parse-config
  :args (:string raw)
  :returns :object
  :body (JSON:parse raw)))

;; Good — zero-arg shorthand (last expression is returned)
(func generate-id
  (crypto:randomUUID))

;; Good — multi-expression body
(func parse-line
  :args (:string line)
  :returns :array
  :body
  (bind parts (line:split "="))
  (bind key ((get parts 0):trim))
  (bind val ((parts:slice 1):join "="))
  #a(key (val:trim))))
```

**Rationale**: `func` is the primary function form in lykn. It emits
`function` declarations, provides runtime type checking on parameters
and return values, supports `:pre`/`:post` contracts, and supports
multi-clause dispatch for overloading.

**See also**: `01-core-idioms.md` ID-09

---

## ID-02: `fn` for Callbacks and Inline Functions

**Strength**: SHOULD

**Summary**: Use `fn` (or `lambda`) for inline callbacks. `fn` produces
typed arrow functions.

```lykn
;; Good — concise, typed
(bind doubled (items:map (fn (:number x) (* x 2))))
(bind adults (users:filter (fn (:any u) (>= u:age 18))))

;; Good — zero-arg
(bind timestamp (fn () (Date:now)))

;; Good — :any for untyped
(bind identity (fn (:any x) x))
```

Compiles to:

```js
const doubled = items.map((x) => {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("anonymous: arg 'x' expected number, got " + typeof x);
  return x * 2;
});
const adults = users.filter((u) => {
  return u.age >= 18;
});
```

**`fn` constraints**: No own `this`, cannot be used with `new`. For
untyped kernel-level arrows (no type checks), use the kernel `=>` form.

**See also**: `01-core-idioms.md` ID-09

---

## ID-02a: Destructured Parameters — Named Params Pattern

**Strength**: SHOULD (for 3+ related params from the same source)

**Summary**: `func` and `fn` accept destructured parameters — an
`(object ...)` or `(array ...)` pattern in `:args` position with
per-field type annotations. This is the idiomatic way to implement
named/keyword parameters in lykn.

### Idiom 1: Named parameters via destructured `func` args

**The pattern**: A function accepts a single object with typed
fields instead of positional parameters. The caller uses `obj`
with keyword syntax.

```lykn
;; IDIOMATIC — named params with per-field types
(func connect
  :args ((object :string host
                 :number port
                 (default :boolean ssl true)))
  :body (open-connection host port ssl))

;; Caller — self-documenting, order-independent
(connect (obj :host "localhost" :port 5432))
(connect (obj :host "db.prod.internal" :port 5432 :ssl true))
```

```js
function connect({host, port, ssl = true}) {
  openConnection(host, port, ssl);
}
connect({ host: "localhost", port: 5432 });
```

**Why it's idiomatic**: The lykn caller side is `(obj :host "localhost"
:port 5432)` — keyword-value pairs that read like a config. The
function definition names and types each field. The JS output is
clean destructured params. No runtime overhead. Per-field type
safety. Self-documenting call sites.

**Contrast with positional params** (less idiomatic for 3+ params):

```lykn
;; LESS IDIOMATIC — positional, hard to read at call site
(func connect
  :args (:string host :number port :boolean ssl)
  :body (open-connection host port ssl))

(connect "localhost" 5432 true)  ;; what's true? ssl? verbose?
```

**Contrast with pre-DD-25 workaround** (type safety gap):

```lykn
;; PRE-DD-25 — type safety gap at the boundary
(func connect
  :args (:object opts)
  :body
  (bind (object host port ssl) opts)  ;; no per-field type checks!
  (open-connection host port ssl))
```

**When to use named params**: 3+ parameters, optional/defaulted
fields, config-style interfaces, any function where the caller
benefits from labeled arguments.

**When to keep positional**: 1–2 params with obvious meaning
(e.g., `(func add :args (:number a :number b) ...)`), callbacks
where brevity matters.

### Idiom 2: Handler/callback destructuring

**The pattern**: Event handlers and callbacks destructure their
argument to extract the fields they need, with type annotations
on each.

```lykn
;; IDIOMATIC — destructure the request, type each field
(func handle-login
  :args ((object :string method :string url :any body) :any res)
  :returns :void
  :body
  (if (= method "POST")
    (authenticate body res)
    (res:status 405)))

;; IDIOMATIC — DOM event handler
(button:add-event-listener "click"
  (fn ((object :string type :any target :boolean shift-key))
    (if shift-key
      (handle-shift-click target)
      (handle-click target))))
```

**Contrast with `:any` param + body access**:

```lykn
;; LESS IDIOMATIC — no type info, field access scattered through body
(func handle-login
  :args (:any req :any res)
  :body
  (if (= req:method "POST")
    (authenticate req:body res)
    (res:status 405)))
```

**Why destructuring is better**: Fields used by the function are
declared upfront in the param list — visible, typed, documented.
The body doesn't need colon-access chains into an opaque `:any`
parameter. A reader can see at a glance what the function uses
from its argument.

### Idiom 3: Component/widget props

**The pattern**: UI components receive typed props via
destructured params. Especially natural with `fn` for inline
component definitions.

```lykn
;; IDIOMATIC — typed props in fn
(bind UserCard
  (fn ((object :string name :string email :number age))
    (template
      "<div class='card'>"
      "<h2>" name "</h2>"
      "<p>" email " — age " age "</p>"
      "</div>")))

;; Usage
(UserCard (obj :name "Duncan" :email "d@example.com" :age 42))
```

**Contrast**: Without DD-25, you'd take `:any props` and access
`props:name`, `props:email`, `props:age` throughout the body —
losing both type checking and the upfront declaration of which
props the component uses.

### Idiom 4: Multi-clause structural dispatch

**The pattern**: Different clauses handle different *shapes* of
input — one accepts an object, another accepts a string. The
dispatch type is implicit from the destructuring pattern.

```lykn
;; IDIOMATIC — structural dispatch via destructuring
(func process-input
  (:args ((object :string name :string email) :string action)
   :returns :string
   :body (template name " (" email ") — " action))

  (:args (:string raw-input :string action)
   :returns :string
   :body (template raw-input " — " action)))

;; Caller
(process-input (obj :name "Alice" :email "a@b.com") "signup")
(process-input "raw data" "import")
```

**How it works**: Clause 1 dispatches on `:object` at position 0.
Clause 2 dispatches on `:string` at position 0. No overlap. The
reader sees the function accepts either a user object or a raw
string — two calling conventions, one function name.

Note: two clauses that both destructure objects at the same
position DO overlap (both match `:object`) — this is a compile
error. Structural dispatch is on the outer type, not on internal
field shapes.

### Idiom 5: `fn` in pipelines with destructured items

**The pattern**: Threading macros processing collections of
objects, where the `fn` destructures each item.

```lykn
;; IDIOMATIC — destructure each item in the pipeline
(->> users
  (filter (fn ((object :boolean active)) active))
  (map (fn ((object :string name :number age))
    (obj :display-name (string:to-upper-case name)
         :birth-year (- 2026 age)))))
```

**Contrast with `:any` + colon access**:

```lykn
;; LESS IDIOMATIC — opaque items, scattered access
(->> users
  (filter (fn (:any u) u:active))
  (map (fn (:any u)
    (obj :display-name (string:to-upper-case u:name)
         :birth-year (- 2026 u:age)))))
```

**When each style wins**: For simple single-field access
(`u:active`), the `:any` style is fine — the access is trivial
and type checking adds little value. For multi-field access where
the body uses 2+ fields, destructuring wins because it documents
what the function needs and checks the types at the boundary.

### Best practices

**Prefer destructured params for 3+ fields**: When a function takes
3 or more related parameters from the same source (user data, config,
request fields), prefer a single destructured object param over
multiple positional params. The call site becomes self-documenting.

**Type every field, even in destructured params**: `:any` is the
explicit opt-out. Don't use it reflexively — type as specifically as
you can. The type checks fire in dev mode and catch bugs at the
boundary. A field typed `:any` in a destructured param is a field
without a safety net.

**Destructure in the param list, not the body**: Pre-DD-25, the
pattern was:

```lykn
(func f :args (:object opts) :body (bind (object x y) opts) ...)
```

Post-DD-25, prefer:

```lykn
(func f :args ((object :string x :number y)) :body ...)
```

The param-list version is shorter, documents the interface in the
signature, and gets per-field type checks. The body version loses
type safety and buries the interface.

**Use `:any` for interop boundaries, types for domain code**: When
destructuring a JS library response (Express `req`, DOM event,
fetch response), `:any` fields are acceptable — you don't control
the shape. When destructuring your own domain objects, type every
field.

```lykn
;; Interop: :any is fine — we don't control Express's types
(func handle
  :args ((object :any method :any url :any body) :any res)
  :body ...)

;; Domain: type everything — we control this shape
(func process-order
  :args ((object :string id :number total :boolean paid))
  :body ...)
```

---

## ID-03: No `this` in Surface Code — Callback Safety for Free

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, using an ordinary function as a callback
loses `this` in strict mode — one of the most common JS bugs.

lykn eliminates this entirely: `this` does not exist in the surface
language. `fn` produces arrow functions that inherit `this` from the
enclosing scope automatically. There is no "loses `this`" bug to worry
about. When you need `this` (e.g., in class methods), use the kernel
`class` form where `this` is available.

---

## ID-04: Method Syntax in Classes

**Strength**: SHOULD

**Summary**: `class` bodies expand surface forms (DD-27). `bind`
produces `const`, `=` is equality (`===`), `set!` does property
mutation, threading macros work, and all other surface forms expand
normally inside methods, constructors, getters, and setters.

Use `assign` for `this`-property assignment in constructors:

```lykn
(class Dog (Animal)
  (constructor (name breed)
    (super name)
    (assign this:breed breed))

  (speak ()
    (bind greeting (template this:name " says woof"))
    (if (= this:breed "poodle")
      (return (template greeting " (fancy)"))
      (return greeting)))

  (fetch-toy (toy-name)
    (return (template this:name " fetches " toy-name))))
```

```js
class Dog extends Animal {
  constructor(name, breed) {
    super(name);
    this.breed = breed;
  }
  speak() {
    const greeting = `${this.name} says woof`;
    if (this.breed === "poodle") return `${greeting} (fancy)`;
    return greeting;
  }
  fetchToy(toyName) {
    return `${this.name} fetches ${toyName}`;
  }
}
```

**Key forms inside class bodies**:
- `assign` — `this`-property assignment, class body only (compile error elsewhere)
- `bind` → `const` — immutable binding
- `=` → `===` — equality (not assignment)
- `set!` → property mutation (`obj.prop = value`)
- All surface forms (`obj`, threading macros, `match`, etc.) work

**Rationale**: `class` is available in lykn but de-emphasized. Prefer
`type` + `func` for data and operations. Use `class` when you need
`instanceof`, shared prototype methods, `Symbol:iterator`, or JS API
interop.

**See also**: `02-api-design.md` ID-28

---

## ID-05: Generator Functions for Lazy Sequences

**Strength**: SHOULD

**Summary**: Use `genfunc` (surface) for typed generators with
`:yields` runtime checks, or the kernel `function*` form when you
don't need per-yield type checking.

```lykn
;; PREFERRED — surface genfunc with typed yields
(genfunc naturals
  :yields :number
  :body
  (let n 0)
  (while true
    (yield n)
    (+= n 1)))

;; PREFERRED — typed lazy transformation
(genfunc lazy-map
  :args (:any iterable :function f)
  :yields :any
  :body
  (for-of x iterable (yield (f x))))

;; PREFERRED — typed lazy filter
(genfunc lazy-filter
  :args (:any iterable :function pred)
  :yields :any
  :body
  (for-of x iterable
    (if (pred x) (yield x))))
```

When you need per-yield type checking:

```lykn
;; :yields :number — each yield is runtime-checked in dev mode
(genfunc range
  :args (:number start :number end)
  :yields :number
  :body
  (for (let i start) (< i end) (+= i 1)
    (yield i)))
```

Anonymous generators via `genfn`:

```lykn
(bind gen (genfn (:number n)
  :yields :number
  (for (let i 0) (< i n) (+= i 1)
    (yield i))))
```

Kernel `function*` is still available when you don't need type
annotations:

```lykn
(function* simple () (yield 1) (yield 2) (yield 3))
```

**Key rules**: `yield` cannot appear inside nested callbacks —
use `for-of` inside the generator. `yield*` delegates to another
iterable and is not type-checked (the delegated generator handles
its own checks). Async generators: `(async (genfunc ...))`.

**See also**: `02-api-design.md` ID-20, ID-21

---

## ID-06: Closures Capture Variables, Not Values

**Strength**: MUST

**Summary**: A closure holds a live reference to the variable's binding.
It sees the current value, not a snapshot at creation time.

```lykn
;; Closures see the current value of closed-over bindings
(func make-counter
  (bind count (cell 0))
  (obj
    :increment (fn () (swap! count (fn (:number n) (+ n 1))) (express count))
    :get (fn () (express count))))

(bind c (make-counter))
((get c :increment))   ;; 1
((get c :increment))   ;; 2
((get c :get))          ;; 2
```

**In lykn**: Because `bind` is immutable, the "stale closure" problem
is reduced. Closures over `bind` values always see the value at
creation time (it cannot change). Closures over `cell` values see the
current `.value` — this is explicit and expected.

**Rationale**: Understanding that closures hold bindings, not values,
prevents an entire category of bugs. lykn's immutable-by-default
design means most closures close over fixed values; only `cell`
closures have mutable state.

---

## ID-07: No `var`-in-Loop Bug — `for-of` Is Always Immutable

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, `var` shares one binding across all loop
iterations, causing closures to see the final value. `let` creates a
fresh binding per iteration.

lykn eliminates this entirely: `var` does not exist, and `for-of`
always creates an immutable binding per iteration. The classic
closure-in-loop bug cannot occur.

---

## ID-08: `cell` for Encapsulated Mutable State

**Strength**: SHOULD

**Summary**: Use `cell` inside functions to create encapsulated mutable
state. This replaces the JS pattern of closures over `let` variables.

```lykn
;; Good — cell for private mutable state
(func create-counter
  :args (:number initial)
  :returns :object
  :body
  (bind count (cell initial))
  (obj
    :increment (fn () (swap! count (fn (:number n) (+ n 1))) (express count))
    :decrement (fn () (swap! count (fn (:number n) (- n 1))) (express count))
    :reset (fn () (reset! count initial))
    :value (fn () (express count))))
```

**Key insight**: Multiple closures returned from the same invocation
share the same `cell`. Different invocations produce independent cells.

**Rationale**: `cell` makes mutable state explicit — every read is
`express`, every write is `swap!` or `reset!`. This is more visible
than JS's `let` inside a closure where mutation happens via `=`.

---

## ID-09: Factory Functions That Return Closures

**Strength**: SHOULD

**Summary**: Use factory functions to create specialized closures with
captured configuration.

```lykn
;; Good — factory captures configuration
(func create-logger
  :args (:string prefix)
  :returns :function
  :body (fn (:string message)
    (console:log (template "[" prefix "] " message))))

(bind db-log (create-logger "DB"))
(bind api-log (create-logger "API"))
(db-log "Connected")   ;; [DB] Connected
(api-log "Request")    ;; [API] Request

;; Good — factory for specialization
(func create-multiplier
  :args (:number factor)
  :returns :function
  :body (fn (:number x) (* x factor)))

(bind double (create-multiplier 2))
(bind triple (create-multiplier 3))
(#a(1 2 3):map double)  ;; [2, 4, 6]
```

---

## ID-10: Beware Closing over `cell` — It Reflects Current State

**Strength**: SHOULD

**Summary**: Because `cell` holds mutable state, closures that read a
cell always reflect its current value, not the value at closure
creation time.

```lykn
;; cell closures always see the current value
(bind url (cell "/api/v1"))

;; This closure will fetch whatever url currently is
(bind handler (fn () (fetch (express url))))

(reset! url "/api/v2")
(handler)   ;; fetches /api/v2, not /api/v1

;; Good — capture at a specific point by binding to a new name
(bind captured-url (express url))
(bind handler2 (fn () (fetch captured-url)))
;; handler2 always fetches what url was at capture time
```

**Rationale**: This is the lykn equivalent of the "stale closure"
problem. Because `bind` is immutable, capturing a `bind` value freezes
it. But reading a `cell` via `express` always returns the current
value. To freeze a cell's value, copy it to a `bind` first.

---

## ID-11: Block Scope — `bind` Is Always Block-Scoped

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, `var` is function-scoped and leaks out of
blocks. `const` and `let` are block-scoped.

lykn eliminates this: `bind` always produces `const`, which is block-
scoped. There is no `var` in lykn. No scope leakage can occur.

---

## ID-12: No Temporal Dead Zone Confusion

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, `let` and `const` have a Temporal Dead Zone
between scope entry and declaration, while `var` hoists to `undefined`.

lykn eliminates this: `bind` produces `const`, and there is no `var`.
The TDZ still exists in the compiled JS, but lykn's structure
(definitions before use) naturally avoids it.

---

## ID-13: `func` Is Not Hoisted — Define Before Use

**Strength**: SHOULD

**Summary**: Unlike JS `function` declarations, `func` compiles to a
`function` declaration that IS hoisted. But lykn convention is to
define functions before their use site for readability.

```lykn
;; Good — define before use
(func parse-line :args (:string line) :returns :array :body
  (bind parts (line:split "="))
  #a((get parts 0) ((parts:slice 1):join "=")))

(func parse-config :args (:string raw) :returns :array :body
  (bind lines (raw:split "\n"))
  (lines:map parse-line))
```

**Rationale**: Top-down code organization (public API first, helpers
below) works because `func` emits hoisted `function` declarations.
But defining before use is the recommended style for clarity.

---

## ID-14: Shadowing — Inner Bindings Mask Outer Ones

**Strength**: SHOULD

**Summary**: An inner `bind` can shadow an outer one of the same name.
The inner binding hides the outer within its scope.

```lykn
(bind x "outer")
(block
  (bind x "inner")
  (console:log x))    ;; "inner"
(console:log x)       ;; "outer" — unaffected
```

**Caution**: Accidental shadowing can cause bugs when a developer
believes they are reading an outer value but are seeing an inner one.

---

## ID-15: Module Scope — Each File Has Its Own Scope

**Strength**: SHOULD

**Summary**: Module-level bindings are private by default. Nothing is
global unless explicitly exported.

```lykn
;; module-a.lykn
(bind SECRET "hidden")
(export (func get-secret :returns :string :body SECRET))

;; module-b.lykn
(import "./module-a.js" (get-secret))
(get-secret)   ;; "hidden"
;; SECRET is not accessible — not exported
```

---

## ID-16: No `this` Rules — `this` Does Not Exist in Surface lykn

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JavaScript, `this` has four binding rules (default,
implicit, explicit, `new`) determined at call time — one of the most
complex and bug-prone aspects of the language.

lykn eliminates `this` from the surface language entirely. State is
managed through `cell` containers, `func` parameters, or `type`
constructors. When JS interop requires `this` (class methods), use the
kernel `class` form.

---

## ID-17: No `this` Inheritance Issues

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: Arrow functions inherit `this` in JS. `fn` in lykn also
produces arrow functions, but since `this` doesn't exist in surface
lykn, the inheritance is irrelevant for surface code.

---

## ID-18: No Method Extraction Bug

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, storing a method in a variable loses `this`. In
lykn, there is no `this` to lose. Functions are values — extracting
them always works correctly.

---

## ID-19: No `call`/`apply`/`bind` Needed

**Status**: ELIMINATED BY LANGUAGE DESIGN

**Summary**: In JS, `.call()`, `.apply()`, and `.bind()` are needed to
set `this`. Without `this` in surface lykn, these are unnecessary.
For JS interop, use `js:bind`.

---

## ID-20: Prefer `:map`, `:filter`, `:reduce` for Transformations

**Strength**: SHOULD

**Summary**: Use functional array methods when building a new value
from an existing array.

```lykn
;; Good — :map for transformation
(bind names (users:map (fn (:any u) u:name)))

;; Good — :filter for selection
(bind active (users:filter (fn (:any u) u:active)))

;; Good — :reduce for aggregation
(bind total (prices:reduce (fn (:number sum :number p) (+ sum p)) 0))

;; Good — chaining
(bind active-emails
  (-> users
    (:filter (fn (:any u) u:active))
    (:map (fn (:any u) u:email))))
```

**Always provide an initial value for `:reduce`** — without it, an
empty array throws `TypeError`.

**Rationale**: Array methods express intent (transform, select,
aggregate) more clearly than imperative loops. They return new arrays
without mutating the original. Use them for building values; use
`for-of` for side effects.

---

## ID-21: Use `:find` / `:findIndex` for Single-Element Search

**Strength**: SHOULD

**Summary**: Use predicate-based search methods instead of filtering
and taking the first element.

```lykn
;; Good — :find returns first match or undefined
(bind admin (users:find (fn (:any u) (= u:role "admin"))))

;; Good — :findIndex returns index or -1
(bind idx (items:findIndex (fn (:any item) (= item:id target-id))))

;; Good — :findLast for reverse search (ES2023)
(bind last-error (logs:findLast (fn (:any entry) (= entry:level "error"))))
```

---

## ID-22: Use `:some` / `:every` for Boolean Checks

**Strength**: SHOULD

**Summary**: Use `:some` for "does any match?" and `:every` for "do
all match?" Both short-circuit.

```lykn
(bind has-errors (results:some (fn (:any r) (= r:status "error"))))
(bind all-valid (inputs:every (fn (:any input) (> input:length 0))))
```

---

## ID-23: Use `:flatMap` for Map-Then-Flatten

**Strength**: SHOULD

**Summary**: `:flatMap` maps each element to zero or more elements and
flattens one level.

```lykn
;; Filter + transform in one step
(bind fulfilled-values
  (results:flatMap (fn (:any r)
    (if (= r:status "fulfilled") #a(r:value) #a()))))

;; One-to-many expansion
(bind all-tags (posts:flatMap (fn (:any post) post:tags)))
```

---

## ID-24: Use `for-of` for Side Effects and Control Flow

**Strength**: SHOULD

**Summary**: Use `for-of` when you need `break`, `continue`, `await`,
or when the primary purpose is side effects.

```lykn
;; Good — for-of with break
(for-of item items
  (if item:done (break))
  (process item))

;; Good — for-of with sequential await
(for-of url urls
  (bind data (await (-> (fetch url) (:then (fn (:any r) (r:json))))))
  (await (save data)))

;; Good — for-of for side effects
(for-of user users
  (console:log user:name))
```

| Use case | Preferred |
|----------|-----------|
| Transform → new array | `:map` |
| Select subset | `:filter` |
| Aggregate → single value | `:reduce` |
| Side effects | `for-of` |
| `break` / `continue` | `for-of` |
| Sequential `await` | `for-of` |

**See also**: `01-core-idioms.md` ID-25

---

## ID-25: `->` Threading Is lykn's Native Pipe

**Strength**: SHOULD

**Summary**: The `->` (thread-first) macro replaces JS's `pipe()`
pattern. It is built into the language — no utility function needed.

```lykn
;; Good — threading replaces nested function calls
(bind result (-> name
  (:trim)
  (:to-lower-case)
  (:replace (regex "\\s+" "g") "-")))

;; Equivalent JS pipe pattern (verbose):
;; pipe(trim, toLowerCase, s => s.replace(/\s+/g, "-"))(name)
```

Compiles to:

```js
const result = name.trim().toLowerCase().replace(/\s+/g, "-");
```

```lykn
;; Good — ->> thread-last for data-last APIs
(bind result (->> items
  (filter even?)
  (map double)
  (take 5)))
```

**Rationale**: `->` and `->>` are built-in threading macros that
replace the `pipe()`/`compose()` utility pattern. They read left-to-
right in execution order, require no utility function, and compile
to clean JS method chains or nested calls.

**See also**: `00-lykn-surface-forms.md` Threading Macros

---

## ID-26: Closures for Partial Application

**Strength**: SHOULD

**Summary**: Use closures (factory functions) for partial application.
lykn's `fn` makes this concise.

```lykn
;; Good — explicit, named, arguments in any position
(bind double (fn (:number x) (* 2 x)))
(bind to-upper (fn (:string s) (s:to-upper-case)))
(bind get-age (fn (:any user) user:age))

;; Good — factory for configuration
(func create-filter
  :args (:function predicate)
  :returns :function
  :body (fn (:array items) (items:filter predicate)))

(bind get-adults (create-filter (fn (:any u) (>= u:age 18))))
(get-adults users)
```

**Rationale**: Closures are explicit, composable, and produce clear
stack traces. Combined with `->` threading, they enable powerful
function composition without dedicated utilities.

---

## ID-27: `->` and `->>` Replace `pipe()` and `compose()`

**Strength**: SHOULD

**Summary**: lykn has built-in threading macros. No `pipe()` or
`compose()` utility is needed.

```lykn
;; -> thread-first: value flows as first argument
(-> user
  (get :name)
  (:to-upper-case)
  (:slice 0 10))

;; ->> thread-last: value flows as last argument
(->> data
  (filter valid?)
  (map transform)
  (reduce combine initial))

;; some-> nil-safe: short-circuits on null
(some-> config
  :database
  :host
  (:to-upper-case))
```

| Macro | Inserts at | Nil-safe | Use for |
|-------|-----------|----------|---------|
| `->` | First arg | No | Method chains, property access |
| `->>` | Last arg | No | Data-last function pipelines |
| `some->` | First arg | Yes | Safe property traversal |
| `some->>` | Last arg | Yes | Safe data pipelines |

---

## ID-28: Prefer Pure Functions

**Strength**: SHOULD

**Summary**: A pure function has no side effects and always returns the
same output for the same input. lykn's immutable-by-default design
naturally encourages purity.

```lykn
;; Good — pure: testable, composable, predictable
(func calculate-tax
  :args (:number price :number rate)
  :returns :number
  :body (* price rate))

(func format-currency
  :args (:number cents)
  :returns :string
  :body (template "$" ((/ cents 100):toFixed 2)))

;; Bad — reads external mutable state
;; (bind tax-rate (cell 0.08))
;; (func calculate-tax :args (:number price) :returns :number
;;   :body (* price (express tax-rate)))
```

**Rationale**: Pure functions are trivially testable (call and assert),
safely composable in `->` pipelines, and can be memoized or reordered.
lykn's `bind` (immutable) and `assoc`/`conj` (non-destructive updates)
make purity the path of least resistance.

---

## ID-29: Isolate Side Effects at the Edges

**Strength**: SHOULD

**Summary**: Push side effects (I/O, logging, mutation) to the boundary.
Keep the core logic pure.

```lykn
;; Good — pure core
(func process-data :args (:string raw) :returns :array :body
  (bind records (parse raw))
  (bind filtered (records:filter valid?))
  (filtered:map normalize))

;; Side effects at the edge
(async (func main :returns :void :body
  (bind raw (await (Deno:readTextFile "data.csv")))
  (bind result (process-data raw))
  (await (Deno:writeTextFile "output.json" (JSON:stringify result)))
  (console:log (template "Processed " result:length " records"))))
```

**Rationale**: The goal is not purity for its own sake but containment
— knowing exactly where the world changes. The pure core is trivially
testable; the side-effect edges are where integration tests focus.

---

## ID-30: Multi-Clause `func` for Overloaded Dispatch

**Strength**: SHOULD

**Summary**: `func` supports multi-clause dispatch on arity and types.
This replaces JS overloading patterns.

```lykn
;; Good — multi-clause: different behavior by arity/type
(func greet
  (:args (:string name)
   :returns :string
   :body (template "Hello, " name "!"))
  (:args (:string greeting :string name)
   :returns :string
   :body (template greeting ", " name "!")))

(greet "world")         ;; "Hello, world!"
(greet "Howdy" "world") ;; "Howdy, world!"
```

Compiles to:

```js
function greet(...args) {
  if (args.length === 2 && typeof args[0] === "string" && typeof args[1] === "string") {
    const greeting = args[0];
    const name = args[1];
    return greeting + ", " + name + "!";
  }
  if (args.length === 1 && typeof args[0] === "string") {
    const name = args[0];
    return "Hello, " + name + "!";
  }
  throw new TypeError("greet: no matching clause for arguments");
}
```

**Rationale**: Multi-clause dispatch replaces the JS pattern of
checking `arguments.length` or `typeof` inside the function body. Each
clause has its own type annotations and contracts. The compiler
generates the dispatch logic.

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | `func` for named functions | SHOULD | Types, contracts, multi-clause |
| 02 | `fn` for callbacks | SHOULD | Typed arrow functions |
| 03 | No `this` in callbacks | ELIMINATED | No `this` in surface lykn |
| 04 | Method syntax in classes | SHOULD | Kernel form for JS interop |
| 05 | Generator functions | SHOULD | Lazy sequences, `yield*` |
| 06 | Closures capture variables | MUST | Live binding reference |
| 07 | No `var`-in-loop bug | ELIMINATED | `for-of` always immutable |
| 08 | `cell` for encapsulated state | SHOULD | Replaces closure-over-`let` |
| 09 | Factory functions | SHOULD | Configuration injection |
| 10 | `cell` closures reflect current state | SHOULD | Copy to `bind` to freeze |
| 11 | Block scope | ELIMINATED | `bind` is always block-scoped |
| 12 | No TDZ confusion | ELIMINATED | No `var`/`let` distinction |
| 13 | Define before use | SHOULD | `func` hoists, but convention is top-down |
| 14 | Shadowing | SHOULD | Inner masks outer |
| 15 | Module scope | SHOULD | Private by default |
| 16 | No `this` rules | ELIMINATED | `this` doesn't exist |
| 17 | No `this` inheritance | ELIMINATED | Arrow's `this` irrelevant |
| 18 | No method extraction bug | ELIMINATED | No `this` to lose |
| 19 | No `call`/`apply`/`bind` needed | ELIMINATED | Use `js:bind` for interop |
| 20 | `:map`/`:filter`/`:reduce` | SHOULD | Transform, select, aggregate |
| 21 | `:find`/`:findIndex` | SHOULD | Short-circuit search |
| 22 | `:some`/`:every` | SHOULD | Boolean aggregates |
| 23 | `:flatMap` | SHOULD | Map + flatten in one pass |
| 24 | `for-of` for control flow | SHOULD | `break`, `continue`, `await` |
| 25 | `->` is native pipe | SHOULD | Built-in, no utility needed |
| 26 | Closures for partial application | SHOULD | Named, flexible |
| 27 | `->` / `->>` replace pipe/compose | SHOULD | Threading macros |
| 28 | Pure functions | SHOULD | Same input → same output |
| 29 | Side effects at edges | SHOULD | Pure core, I/O at boundaries |
| 30 | Multi-clause `func` | SHOULD | Overloaded dispatch |

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for `func` vs `fn` (ID-09),
  `for-of` (ID-25), threading (ID-04)
- **API Design**: See `02-api-design.md` for iterator protocol
  (ID-20-22), factory patterns (ID-11, ID-13)
- **Error Handling**: See `03-error-handling.md` for async error
  patterns, contracts at boundaries
- **Values & References**: See `04-values-references.md` for `cell`
  model (ID-20), `assoc`/`conj` (ID-14)
- **Type Discipline**: See `05-type-discipline.md` for type annotations,
  contracts, `match` dispatch
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for
  `func`, `fn`, `lambda`, `->`, `->>`, `some->`, `some->>`
