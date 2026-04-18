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

Compiles to:

```js
const maxRetries = 3;
const users = [];
const counter = {value: 0};
counter.value = ((n) => {
  if (typeof n !== "number" || Number.isNaN(n))
    throw new TypeError("anonymous: arg 'n' expected number, got " + typeof n);
  return n + 1;
})(counter.value);
console.log(counter.value);
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

Compiles to:

```js
const timeout = options.timeout ?? 5000;
const title = options.title ?? "Untitled";
const verbose = options.verbose ?? true;
const timeout = options.timeout || 5000;
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

Compiles to:

```js
const street = (() => {
  const t__gensym0 = person;
  if (t__gensym0 == null) return t__gensym0;
  const t__gensym1 = t__gensym0.address;
  if (t__gensym1 == null) return t__gensym1;
  return t__gensym1.street;
})() ?? "(unknown)";
const len = (() => {
  const t__gensym2 = arr;
  if (t__gensym2 == null) return t__gensym2;
  return t__gensym2.length;
})();
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

Compiles to:

```js
const msg = `Hello, ${name}! You have ${count} items.`;
const greeting = "Hello, world";
```

```lykn
;; Bad — string concatenation via +
(bind msg (+ "Hello, " name "! You have " count " items."))
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
(for-of (const (array index value) (arr:entries))
  (console:log (template index ": " value)))
```

Compiles to:

```js
const {name, email, role = "member"} = user;
const [first, ...tail] = items;
for (const [index, value] of arr.entries()) {
  console.log(`${index}: ${value}`);
}
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

Compiles to:

```js
export function formatDate(d) {
  return d.toISOString();
}
export function parseDate(s) {
  return new Date(s);
}
export const DATE_FORMAT = "YYYY-MM-DD";
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

Compiles to:

```js
import {formatDate} from "./utils.js";
import {join} from "@std/path";
const data = await Deno.readTextFile(join("config", "app.json"));
export function processData(input) {
  return input;
}
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

Compiles to:

```js
function parseConfig(raw) {
  if (typeof raw !== "string")
    throw new TypeError("parse-config: arg 'raw' expected string, got " + typeof raw);
  const result__gensym0 = JSON.parse(raw);
  /* return type check ... */
  return result__gensym0;
}
const doubled = items.map((x) => {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("anonymous: arg 'x' expected number, got " + typeof x);
  return x * 2;
});
button.addEventListener("click", (e) => {
  return handleClick(e);
});
```

**Function forms** (`00-lykn-surface-forms.md`):

| Form | Use for | Output |
|------|---------|--------|
| `func` | Named functions with types/contracts | `function` declaration |
| `fn` | Typed anonymous functions | Arrow function |
| `lambda` | Alias for `fn` | Arrow function |
| `=>` (kernel) | Untyped anonymous functions | Arrow function |

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

Compiles to:

```js
const MAX_RETRIES = 3;
const TOO_MANY_REQUESTS = 429;
if (response.status === TOO_MANY_REQUESTS) await backoff(attempt, MAX_RETRIES);
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

Compiles to:

```js
const config = {debug: false};
const counter = {value: 0};
counter.value = ((n) => {
  if (typeof n !== "number" || Number.isNaN(n))
    throw new TypeError("anonymous: arg 'n' expected number, got " + typeof n);
  return n + 1;
})(counter.value);
console.log(counter.value);
const updatedConfig = {...config, debug: true};
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

Compiles to:

```js
const CONFIG = Object.freeze({maxRetries: 3, timeout: 5000, baseUrl: "https://api.example.com"});
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

Compiles to:

```js
const cache = new Map();
cache.set(userObj, computeExpensiveResult(userObj));
cache.set(42, "forty-two");
const visited = new Set();
visited.add(url);
if (visited.has(url)) console.log("skip");
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

Compiles to:

```js
function findMax(first, ...others) {
  const result = {value: first};
  for (const n of others) {
    if (n > result.value) result.value = n;
  }
  return result.value;
}
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

Compiles to:

```js
const defaults = {timeout: 5000, retries: 3};
const config = {...defaults, timeout: 10000, debug: true};
const items = [1, 2, 3];
const moreItems = [...items, 4];
const merged = [...arr1, ...arr2];
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

Compiles to:

```js
const double = (x) => {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("anonymous: arg 'x' expected number, got " + typeof x);
  return x * 2;
};
const isEven = (n) => {
  if (typeof n !== "number" || Number.isNaN(n))
    throw new TypeError("anonymous: arg 'n' expected number, got " + typeof n);
  return n % 2 === 0;
};
function getName(user) {
  return user.name;
}
function processItem(item) {
  if (typeof item !== "string")
    throw new TypeError("process-item: arg 'item' expected string, got " + typeof item);
  const normalized = item.trim();
  const lower = normalized.toLowerCase();
  return lower;
}
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

Compiles to:

```js
const response = await fetch("https://api.example.com/data");
const text = await Deno.readTextFile("./config.json");
const url = new URL("/path", "https://example.com");
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
;; Good — shallow copy via assoc (new object)
(bind copy (assoc original))

;; Good — deep copy
(bind deep (structuredClone original))
```

Compiles to:

```js
const copy = {...original};
const deep = structuredClone(original);
```

**Shallow vs. deep**:

```lykn
;; Shallow — nested objects are shared
(bind original (obj :work (obj :employer "Acme")))
(bind shallow (assoc original))
;; shallow:work is the same reference as original:work

;; Deep — fully independent
(bind deep (structuredClone original))
```

**Rationale**: `structuredClone` handles circular references, `Date`,
`RegExp`, `Map`, `Set`, and more. `assoc` with no extra key-value
pairs produces a shallow copy via spread.

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

Compiles to:

```js
const original = {city: "Berlin", country: "Germany"};
const updated = {...original, city: "Munich"};
const sanitized = (() => {
  const {password: ___gensym0, ...rest__gensym1} = user;
  return rest__gensym1;
})();
const withNew = [...items, newItem];
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
(for-of (const (array i item) (items:entries))
  (console:log (template i ": " item)))

;; Good — for-of with await (sequential execution)
(for-of item items
  (await (process item)))
```

Compiles to:

```js
for (const item of items) {
  if (item.skip) continue;
  if (item.done) break;
  process(item);
}
for (const [i, item] of items.entries()) {
  console.log(`${i}: ${item}`);
}
for (const item of items) {
  await process(item);
}
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

Compiles to:

```js
function processUser(user) {
  if (!user) throw new Error("user is required");
  if (!user.active) return null;
  if (!user.email) return null;
  const normalized = user.email.toLowerCase();
  return sendWelcome(normalized);
}
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
(bind updated (assoc user :age 31))
(bind public (dissoc user :age))

;; The keyword :first-name becomes the JS key "firstName"
```

Compiles to:

```js
const user = {firstName: "Alice", lastName: "Smith", age: 30};
const updated = {...user, age: 31};
const public = (() => {
  const {age: ___gensym0, ...rest__gensym1} = user;
  return rest__gensym1;
})();
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

Compiles to:

```js
console.log("hello");
const len = items.length;
const name = user.firstName;
const result = Math.floor(Math.random() * 100);
const item = items[0];
const val = obj[key];
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

Compiles to:

```js
if (x > 0 && x < 100) console.log("in range");
const result = cachedValue || computeFresh();
if (!valid?(input)) throw new Error("invalid input");
if (((a && b) && c) && d) console.log("all truthy");
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

Compiles to:

```js
{
  function Ok(value) {
    return {tag: "Ok", value: value};
  }
  function Err(message) {
    if (typeof message !== "string")
      throw new TypeError("Err: field 'message' expected string, got " + typeof message);
    return {tag: "Err", message: message};
  }
}
function handleResult(r) {
  const result__gensym1 = (() => {
    const target__gensym0 = r;
    if (target__gensym0.tag === "Ok") {
      const v = target__gensym0.value;
      return `Success: ${v}`;
    }
    if (target__gensym0.tag === "Err") {
      const msg = target__gensym0.message;
      return `Error: ${msg}`;
    }
    throw new Error("match: no matching pattern");
  })();
  return result__gensym1;
}
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
