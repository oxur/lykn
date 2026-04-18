# lykn Surface Forms Reference

The complete catalog of lykn syntax. Every form compiles to
kernel forms, which compile to JavaScript. No runtime dependencies.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

All examples verified against **lykn 0.4.0-dev** compiler output (DD-22).

---

## How to Read This Guide

- **lykn column**: the surface syntax you write
- **JS column**: what the compiler produces
- **Notes**: caveats, edge cases, related forms
- Forms marked **(surface)** are high-level forms that expand to kernel forms
- Forms marked **(kernel)** are low-level forms that map directly to JS
- All compiled JS output shown was captured from `./bin/lykn compile`

---

## Syntax Fundamentals

### Colon Syntax (member access)

Colons in identifiers compile to `.` property access at compile time. No
runtime cost.

| lykn | JS | Notes |
|---|---|---|
| `console:log` | `console.log` | Member access |
| `items:length` | `items.length` | Property access |
| `user:to-string` | `user.toString` | camelCase applied |
| `Math:floor` | `Math.floor` | Stdlib access |
| `this:-name` | `this.#_name` | Private field (leading `-`) |

Colons split left-to-right. `a:b:c` becomes `a.b.c`. The leading `-`
prefix on the last segment maps to a `#_` private field.

### Keywords (leading colon)

A leading colon makes an atom a keyword. Keywords serve two purposes:

1. **Object keys** in `obj`, `assoc`, `dissoc` — `:name` becomes the key `"name"`
2. **Type annotations** in `func`, `fn`, `type` — `:number`, `:string`, `:any`, etc.

Keywords are not values at runtime. They are compile-time markers that the
surface macros consume.

### lisp-case to camelCase Conversion

Hyphens in identifiers are automatically converted to camelCase:

| lykn | JS |
|---|---|
| `my-function` | `myFunction` |
| `first-name` | `firstName` |
| `method-result` | `methodResult` |
| `abs-val` | `absVal` |

This applies to bindings, function names, object keys, and all
identifiers. The conversion is purely lexical.

### Comments

| Syntax | Scope | Notes |
|---|---|---|
| `; text` | Line | Everything after `;` to end of line |
| `;; text` | Line | Convention for top-level comments |
| `#; expr` | Expression | Discards the next form (JS compiler only; see Known Gaps) |
| `#\| ... \|#` | Block | Nestable block comments (JS compiler only; see Known Gaps) |

---

## Bindings & Mutation

### bind (surface)

Immutable binding. Compiles to `const`.

```lykn
(bind x 42)
```
```js
const x = 42;
```

Type annotation with runtime enforcement (DD-24):

```lykn
(bind :number age 42)
```
```js
const age = 42;
```

For literal values, the type is verified at compile time (no runtime
cost). Type-incompatible literals are compile errors. For non-literal
initializers, a runtime type check is emitted:

```lykn
(bind :number result (compute-something))
```
```js
const result = computeSomething();
if (typeof result !== "number" || Number.isNaN(result))
  throw new TypeError("bind: binding 'result' expected number, ...");
```

Runtime checks can be stripped with `--strip-assertions`.

`bind` always produces `const`. For mutable state, use `cell`.

### cell / express / swap! / reset! (surface)

Controlled mutation via cell containers. A cell wraps a value in an
object with a `.value` property, giving you explicit, visible mutation
points.

**cell** — create a mutable container:

```lykn
(bind counter (cell 0))
```
```js
const counter = {value: 0};
```

**express** — read the cell's current value:

```lykn
(express counter)
```
```js
counter.value
```

**swap!** — update the cell by applying a function to the current value:

```lykn
(swap! counter (=> (n) (+ n 1)))
```
```js
counter.value = ((n) => n + 1)(counter.value);
```

`swap!` accepts extra arguments that are passed after the current value:

```lykn
(swap! counter f a b)
```
```js
counter.value = f(counter.value, a, b);
```

**reset!** — set the cell to a new value directly:

```lykn
(reset! counter 0)
```
```js
counter.value = 0;
```

---

## Functions

### func (surface) — named, with contracts

`func` defines named functions with optional typed parameters, return
type checks, and pre/post contracts.

**Zero-arg shorthand** — last expression is implicit return:

```lykn
(func now (Date:now))
```
```js
function now() {
  return Date.now();
}
```

Multi-expression body:

```lykn
(func pick
  (bind idx (Math:floor (* (Math:random) taglines:length)))
  (taglines:at idx))
```
```js
function pick() {
  const idx = Math.floor(Math.random() * taglines.length);
  return taglines.at(idx);
}
```

**Single clause with type annotations:**

```lykn
(func add
  :args (:number a :number b)
  :returns :number
  :body (+ a b))
```
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

`:returns :void` suppresses return value and return-type check:

```lykn
(func greet
  :args (:string name)
  :returns :void
  :body (console:log name))
```
```js
function greet(name) {
  if (typeof name !== "string")
    throw new TypeError("greet: arg 'name' expected string, got " + typeof name);
  console.log(name);
}
```

**Supported type annotations:**

| Annotation | JS check |
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
| `:<UserType>` | `typeof !== "object" \|\| null \|\| !("tag" in ...)` |

**Pre-conditions** (`:pre`) — caller blame:

```lykn
(func pos
  :args (:number x) :returns :number
  :pre (> x 0)
  :body x)
```
```js
function pos(x) {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("pos: arg 'x' expected number, got " + typeof x);
  if (!(x > 0))
    throw new Error("pos: pre-condition failed: (> x 0) — caller blame");
  const result__gensym0 = x;
  if (typeof result__gensym0 !== "number" || Number.isNaN(result__gensym0))
    throw new TypeError("pos: return value expected number, got " + typeof result__gensym0);
  return result__gensym0;
}
```

**Post-conditions** (`:post`) — callee blame. Use `~` (tilde) to
reference the return value:

```lykn
(func abs-val
  :args (:number x) :returns :number
  :post (>= ~ 0)
  :body (if (< x 0) (- 0 x) x))
```
```js
function absVal(x) {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("abs-val: arg 'x' expected number, got " + typeof x);
  const result__gensym1 = x < 0 ? 0 - x : x;
  if (!(result__gensym1 >= 0))
    throw new Error("abs-val: post-condition failed: (>= ~ 0) — callee blame");
  return result__gensym1;
}
```

**Multi-clause** — overloaded on arity and types:

```lykn
(func greet
  (:args (:string name) :returns :string
   :body (+ "Hello, " name))
  (:args (:string g :string name) :returns :string
   :body (+ g ", " name)))
```
```js
function greet(...args) {
  if (args.length === 2 && typeof args[0] === "string" && typeof args[1] === "string") {
    const g = args[0];
    const name = args[1];
    // type checks ...
    return g + ", " + name;
  }
  if (args.length === 1 && typeof args[0] === "string") {
    const name = args[0];
    // type checks ...
    return "Hello, " + name;
  }
  throw new TypeError("greet: no matching clause for arguments");
}
```

**`--strip-assertions` flag** removes all type checks and contracts:

```sh
lykn compile file.lykn --strip-assertions
```

```lykn
(func add :args (:number a :number b) :returns :number :body (+ a b))
```
```js
function add(a, b) {
  return a + b;
}
```

**Destructured parameters** (DD-25) — a destructuring pattern
(`object` or `array`) can appear in `:args` where a `:type name`
pair would go. Every field inside the pattern requires a type keyword.

```lykn
;; Object destructuring — fields are typed
(func greet
  :args ((object :string name :number age))
  :returns :string
  :body (template name " is " age))
```
```js
function greet({name, age}) {
  if (typeof name !== "string")
    throw new TypeError("greet: arg 'name' expected string, got " + typeof name);
  if (typeof age !== "number" || Number.isNaN(age))
    throw new TypeError("greet: arg 'age' expected number, got " + typeof age);
  return `${name} is ${age}`;
}
```

```lykn
;; Array destructuring with rest
(func first-and-rest
  :args ((array :number head (rest :number tail)))
  :body (console:log head tail))

;; Mixed destructured + simple
(func handler
  :args ((object :string method :string url) :any body)
  :body (console:log method url body))
```

**Default values** (DD-25.1) — `(default :type name value)` inside
a destructuring pattern:

```lykn
(func connect
  :args ((object :string host
                 :number port
                 (default :boolean ssl true)))
  :body (open-connection host port ssl))
```
```js
function connect({host, port, ssl = true}) {
  // per-field type checks ...
  openConnection(host, port, ssl);
}
```

**Nested destructuring** (DD-25.1) — use `alias` for object
nesting (names the intermediate property). Array nesting is
positional (no alias needed).

```lykn
(func f
  :args ((object :string name
                 (alias :any addr (object :string city :string zip))))
  :body (template name " in " city))
```
```js
function f({name, addr: {city, zip}}) {
  // type checks for name, city, zip ...
}
```

**Multi-clause dispatch**: destructured `object` params dispatch as
`:object`, destructured `array` as `:array`. Two clauses that both
destructure objects at the same position overlap — compile error.

### fn (surface) — anonymous, arrow output

`fn` creates typed arrow functions. Same typed parameter syntax as
`func`, including destructured params.

```lykn
(fn (:number x) (* x 2))
```
```js
(x) => {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("anonymous: arg 'x' expected number, got " + typeof x);
  x * 2;
}
```

Zero-arg:

```lykn
(fn () (Date:now))
```
```js
() => Date.now()
```

With `:any` (no type check):

```lykn
(fn (:any x) x)
```
```js
(x) => x
```

With destructured params (DD-25):

```lykn
(fn ((object :string name :number age)) (console:log name age))
```
```js
({name, age}) => {
  if (typeof name !== "string")
    throw new TypeError("anonymous: arg 'name' expected string, got " + typeof name);
  if (typeof age !== "number" || Number.isNaN(age))
    throw new TypeError("anonymous: arg 'age' expected number, got " + typeof age);
  console.log(name, age);
}
```

### lambda (surface) — alias for fn

`lambda` is an exact alias for `fn`. Same syntax, same output.

```lykn
(lambda (:number x) (+ x 1))
```
```js
(x) => {
  if (typeof x !== "number" || Number.isNaN(x))
    throw new TypeError("anonymous: arg 'x' expected number, got " + typeof x);
  x + 1;
}
```

### genfunc (surface) — named generator with typed yields

`genfunc` defines named generator functions with optional typed
parameters, `:yields` type checking, and contracts. Parallels `func`.

```lykn
(genfunc range
  :args (:number start :number end)
  :yields :number
  :body
  (for (let i start) (< i end) (+= i 1)
    (yield i)))
```
```js
function* range(start, end) {
  if (typeof start !== "number" || Number.isNaN(start))
    throw new TypeError("range: arg 'start' expected number, got " + typeof start);
  if (typeof end !== "number" || Number.isNaN(end))
    throw new TypeError("range: arg 'end' expected number, got " + typeof end);
  for (let i = start; i < end; i += 1) {
    yield (() => {
      const yv = i;
      if (typeof yv !== "number" || Number.isNaN(yv))
        throw new TypeError("range: yield expected number, got " + typeof yv);
      return yv;
    })();
  }
}
```

`:yields :type` emits per-yield runtime type checks in dev mode via
IIFE wrappers. `:yields :any` skips checks. Omitting `:yields`
also skips checks. `yield*` (delegation) is never instrumented —
the delegated generator is responsible for its own checks.

Zero-arg shorthand (no `:args`):

```lykn
(genfunc fibonacci
  :yields :number
  :body
  (let a 0) (let b 1)
  (while true
    (yield a)
    (let temp a) (= a b) (= b (+ temp b))))
```

Async generators: `(async (genfunc ...))` or
`(export (async (genfunc ...)))`.

### genfn (surface) — anonymous typed generator

`genfn` creates anonymous generator expressions. Same typed
parameter syntax as `fn`, with optional `:yields` annotation.

```lykn
(bind gen (genfn (:number start :number end)
  :yields :number
  (for (let i start) (< i end) (+= i 1)
    (yield i))))
```
```js
const gen = function* (start, end) {
  // param type checks ...
  for (let i = start; i < end; i += 1) {
    yield /* checked */;
  }
};
```

Without `:yields`:

```lykn
(bind gen (genfn () (yield 1) (yield 2)))
```

---

## Types & Pattern Matching

### type (surface) — algebraic data types

Define tagged constructors. Each constructor is a function (or constant
for zero-field variants) that returns `{ tag: "Name", ...fields }`.

```lykn
(type Option (Some :any value) None)
```
```js
{
  function Some(value) {
    return {tag: "Some", value: value};
  }
  const None = {tag: "None"};
}
```

With typed fields:

```lykn
(type Shape
  (Circle :number radius)
  (Rect :number width :number height)
  (Point))
```
```js
{
  function Circle(radius) {
    if (typeof radius !== "number" || Number.isNaN(radius))
      throw new TypeError("Circle: field 'radius' expected number, got " + typeof radius);
    return {tag: "Circle", radius: radius};
  }
  function Rect(width, height) {
    if (typeof width !== "number" || Number.isNaN(width))
      throw new TypeError("Rect: field 'width' expected number, got " + typeof width);
    if (typeof height !== "number" || Number.isNaN(height))
      throw new TypeError("Rect: field 'height' expected number, got " + typeof height);
    return {tag: "Rect", width: width, height: height};
  }
  const Point = {tag: "Point"};
}
```

Zero-field constructors become constants (not functions). Call `Point`
directly, not `Point()`.

**Prelude types** — `Option` (`Some`/`None`) and `Result` (`Ok`/`Err`)
are pre-registered in the type registry so `match` can resolve their
fields without a preceding `type` declaration.

### match (surface) — exhaustive pattern matching

Pattern match on values. Always wraps in an IIFE.

**Literal patterns:**

```lykn
(match status
  (200 "ok")
  (404 "not found")
  (_ "unknown"))
```
```js
(() => {
  const target__gensym0 = status;
  if (target__gensym0 === 200) {
    return "ok";
  }
  if (target__gensym0 === 404) {
    return "not found";
  }
  {
    return "unknown";
  }
})()
```

**ADT constructor patterns:**

```lykn
(type Option (Some :any value) None)
(match opt
  ((Some v) v)
  (None 0))
```
```js
(() => {
  const target__gensym1 = opt;
  if (target__gensym1.tag === "Some") {
    const v = target__gensym1.value;
    return v;
  }
  if (target__gensym1.tag === "None") {
    return 0;
  }
  throw new Error("match: no matching pattern");
})()
```

**Structural object patterns:**

```lykn
(match resp
  ((obj :ok true :data d) d)
  (_ "error"))
```
```js
(() => {
  const target__gensym0 = resp;
  if (typeof target__gensym0 === "object" && target__gensym0 !== null
      && "ok" in target__gensym0 && "data" in target__gensym0) {
    const d = target__gensym0.data;
    return d;
  }
  {
    return "error";
  }
})()
```

**Guard clauses** (`:when`):

```lykn
(match opt
  ((Some v) :when (> v 10) "big")
  ((Some v) "small")
  (None "none"))
```
```js
(() => {
  const target__gensym0 = opt;
  if (target__gensym0.tag === "Some") {
    const v = target__gensym0.value;
    if (v > 10) {
      return "big";
    }
  }
  if (target__gensym0.tag === "Some") {
    const v = target__gensym0.value;
    return "small";
  }
  if (target__gensym0.tag === "None") {
    return "none";
  }
  throw new Error("match: no matching pattern");
})()
```

**Pattern types summary:**

| Pattern | Matches | Binds |
|---|---|---|
| `_` | Anything (wildcard) | Nothing |
| `42` | Number literal | Nothing |
| `"ok"` | String literal | Nothing |
| `true` / `false` / `null` | Literal | Nothing |
| `x` | Anything (binding) | `const x = target` |
| `None` | Zero-field ADT (PascalCase) | Nothing |
| `(Some v)` | ADT constructor | Named fields |
| `(obj :key v)` | Structural object | Selected fields |

Without a wildcard or binding as the last clause, `match` adds a
`throw new Error("match: no matching pattern")` fallback.

### if-let / when-let (surface) — conditional binding

**if-let** — bind + branch in one form. Always an IIFE.

Simple binding (null check):

```lykn
(if-let (user (find-user id))
  (greet user)
  "not found")
```
```js
(() => {
  const t__gensym0 = findUser(id);
  if (t__gensym0 != null) {
    const user = t__gensym0;
    return greet(user);
  } else {
    return "not found";
  }
})()
```

ADT pattern:

```lykn
(if-let ((Some v) (find-user id))
  (greet v)
  "none")
```
```js
(() => {
  const t__gensym1 = findUser(id);
  if (t__gensym1.tag === "Some") {
    const v = t__gensym1.value;
    return greet(v);
  } else {
    return "none";
  }
})()
```

**when-let** — same as `if-let` but without the else branch:

```lykn
(when-let (user (find-user id))
  (greet user))
```
```js
(() => {
  const t__gensym2 = findUser(id);
  if (t__gensym2 != null) {
    const user = t__gensym2;
    return greet(user);
  }
})()
```

---

## Objects & Immutable Updates

### obj (surface) — construction

Build objects from keyword-value pairs. Keywords become camelCase keys.

```lykn
(obj :name "Duncan" :age 42)
```
```js
{name: "Duncan", age: 42}
```

Kebab-case keys auto-convert:

```lykn
(obj :first-name "Duncan")
```
```js
{firstName: "Duncan"}
```

Empty object:

```lykn
(obj)
```
```js
{}
```

### assoc (surface) — immutable field update

Returns a new object with updated fields via spread:

```lykn
(assoc user :age 43)
```
```js
{...user, age: 43}
```

Multiple fields:

```lykn
(assoc obj :a 1 :b 2)
```
```js
{...obj, a: 1, b: 2}
```

### dissoc (surface) — immutable field removal

Returns a new object without the specified keys. Compiles to an IIFE
with destructuring:

```lykn
(dissoc obj :password)
```
```js
(() => {
  const {password: ___gensym0, ...rest__gensym1} = obj;
  return rest__gensym1;
})()
```

Multiple keys:

```lykn
(dissoc obj :a :b)
```
```js
(() => {
  const {a: ___gensym2, b: ___gensym3, ...rest__gensym4} = obj;
  return rest__gensym4;
})()
```

### conj (surface) — immutable collection append

Returns a new array with an item appended:

```lykn
(conj items 42)
```
```js
[...items, 42]
```

With an expression:

```lykn
(conj items (+ 1 2))
```
```js
[...items, 1 + 2]
```

---

## Threading Macros

### -> (thread-first)

Threads a value through a series of forms, inserting it as the **first**
argument.

```lykn
(-> 5 (+ 3) (* 2))
```
```js
(5 + 3) * 2
```

With bare functions (not lists):

```lykn
(-> x f g)
```
```js
g(f(x))
```

**Keyword method calls** — a keyword step calls a method on the threaded value:

```lykn
(-> user (get :name) (:to-upper-case))
```
```js
user["name"].toUpperCase()
```

### ->> (thread-last)

Threads a value as the **last** argument:

```lykn
(->> items (filter even?) (map double))
```
```js
map(double, filter(even?, items))
```

With bare functions, `->` and `->>` behave identically (single-arg calls):

```lykn
(->> x f g)
```
```js
g(f(x))
```

### some-> / some->> (nil-safe threading)

Like `->` / `->>` but short-circuits on `null`/`undefined`. Compiles to
an IIFE with null checks at each step.

```lykn
(some-> user (get :name) (:to-upper-case))
```
```js
(() => {
  const t__gensym0 = user;
  if (t__gensym0 == null) return t__gensym0;
  const t__gensym1 = t__gensym0["name"];
  if (t__gensym1 == null) return t__gensym1;
  return t__gensym1.toUpperCase();
})()
```

```lykn
(some->> items (filter even?) (map double))
```
```js
(() => {
  const t__gensym2 = items;
  if (t__gensym2 == null) return t__gensym2;
  const t__gensym3 = filter(even?, t__gensym2);
  if (t__gensym3 == null) return t__gensym3;
  return map(double, t__gensym3);
})()
```

The null check uses `== null` (loose equality), which catches both
`null` and `undefined`.

---

## Modules (kernel)

### import

```lykn
(import "./utils.js" (add subtract))
```
```js
import {add, subtract} from "./utils.js";
```

Default import:

```lykn
(import "./config.js" config)
```
```js
import config from "./config.js";
```

### export

```lykn
(export (const VERSION "1.0"))
```
```js
export const VERSION = "1.0";
```

Default export:

```lykn
(export default main-fn)
```
```js
export default mainFn;
```

### dynamic-import

```lykn
(dynamic-import "./mod.js")
```
```js
import("./mod.js")
```

---

## Control Flow (kernel forms)

### if / ? (ternary)

```lykn
(if (> x 0)
  (console:log "positive")
  (console:log "non-positive"))
```
```js
if (x > 0) console.log("positive");
 else console.log("non-positive");
```

Ternary expression:

```lykn
(? (> x 0) "yes" "no")
```
```js
x > 0 ? "yes" : "no"
```

### for-of / while / for / do-while

```lykn
(for-of item items (console:log item))
```
```js
for (const item of items) {
  console.log(item);
}
```

```lykn
(while (> n 0) (console:log n) (= n (- n 1)))
```
```js
while (n > 0) {
  console.log(n);
  n = n - 1;
}
```

C-style for:

```lykn
(for (let i 0) (< i 10) (++ i) (console:log i))
```
```js
for (let i = 0; i < 10; ++i) {
  console.log(i);
}
```

do-while:

```lykn
(do-while (console:log n) (> n 0))
```
```js
do {
  console.log(n);
} while (n > 0);
```

### try / catch / finally

```lykn
(try
  (risky)
  (catch e (console:log e))
  (finally (cleanup)))
```
```js
try {
  risky();
} catch (e) {
  console.log(e);
} finally {
  cleanup();
}
```

### throw

```lykn
(throw (new Error "oops"))
```
```js
throw new Error("oops");
```

### switch / break / continue

```lykn
(switch status
  ("ok" (console:log "good") (break))
  (default (console:log "bad")))
```
```js
switch (status) {
  case "ok":
    console.log("good");
    break;
  default:
    console.log("bad");
}
```

---

## Expressions (kernel forms)

### template (template literals)

```lykn
(template "hi " name "!")
```
```js
`hi ${name}!`
```

Strings become literal text segments; everything else becomes
`${...}` interpolations.

### tag (tagged templates)

```lykn
(tag html (template "<p>" text "</p>"))
```
```js
html`<p>${text}</p>`
```

### regex

```lykn
(regex "^hello" "gi")
```
```js
/^hello/gi
```

### new

```lykn
(new Thing 1 2)
```
```js
new Thing(1, 2)
```

### get (computed access)

```lykn
(get arr 0)
```
```js
arr[0]
```

```lykn
(get user :name)
```
```js
user["name"]
```

---

## Data Literals

### #a(...) — array

```lykn
#a(1 2 3)
```
```js
[1, 2, 3]
```

### #o(...) — object

```lykn
#o((name "x") (age 42))
```
```js
{name: "x", age: 42}
```

### #NNr — radix literals

```lykn
#16rff
```
```js
255
```

```lykn
#2r11110000
```
```js
240
```

Radix literals are evaluated at read time and compiled as plain numbers.

---

## Operators

### Arithmetic (kernel)

| lykn | JS | Notes |
|---|---|---|
| `(+ a b c)` | `a + b + c` | Variadic |
| `(- a b)` | `a - b` | |
| `(* x y)` | `x * y` | |
| `(/ a b)` | `a / b` | |
| `(% a b)` | `a % b` | Remainder |
| `(** base exp)` | `base ** exp` | Exponentiation |

### Equality (surface — DD-22)

| lykn | JS | Notes |
|---|---|---|
| `(= a b)` | `a === b` | **Strict equality** (not assignment!) |
| `(!= a b)` | `a !== b` | Strict inequality |
| `(= a b c)` | `a === b && b === c` | Variadic pairwise chain |

`=` means equality in surface lykn — matching every Lisp dialect.
There is no assignment operator in surface syntax; all mutation goes
through named forms: `bind`, `reset!`, `swap!`, `assoc`, `dissoc`,
`conj`.

For the rare `== null` idiom, use the `js:eq` escape hatch:

```lykn
(js:eq x null)
```
```js
x == null
```

### Logical (surface — DD-22)

| lykn | JS | Notes |
|---|---|---|
| `(and a b)` | `a && b` | Short-circuit AND |
| `(or a b)` | `a \|\| b` | Short-circuit OR |
| `(not x)` | `!x` | Logical NOT (unary only) |
| `(and a b c d)` | `a && b && c && d` | Variadic left-to-right |
| `(or a b c d)` | `a \|\| b \|\| c \|\| d` | Variadic left-to-right |
| `(not (not x))` | `!!x` | Nested |

`and`/`or` are short-circuit operators (not function calls). `not`
accepts exactly one argument.

### Comparison (kernel)

| lykn | JS | Notes |
|---|---|---|
| `(< a b)` | `a < b` | |
| `(> a b)` | `a > b` | |
| `(<= a b)` | `a <= b` | |
| `(>= a b)` | `a >= b` | |

### Nullish (kernel)

| lykn | JS |
|---|---|
| `(?? a b)` | `a ?? b` |

### Assignment (kernel — not available in surface)

These operators are used internally by the compiler and in kernel
passthrough. Surface code should use `bind`, `reset!`, `swap!` instead.

| lykn | JS | Notes |
|---|---|---|
| `(= x 1)` | `x = 1` | Kernel assignment (surface `=` is equality) |
| `(+= x 1)` | `x += 1` | Compound assignment |
| `(-= x 1)` | `x -= 1` | |
| `(++ x)` | `++x` | Prefix increment |
| `(-- x)` | `--x` | Prefix decrement |

> **Note:** In surface lykn, `(= a b)` compiles to `a === b` (strict
> equality). The kernel `=` (assignment) is only used internally by
> surface macros like `reset!` and `swap!`, and in kernel passthrough
> for classes and `for` loops. See DD-22.

---

## Macros

### macro (definition)

Define compile-time macros with quasiquote templates:

```lykn
(macro when (test (rest body))
  `(if ,test (block ,@body)))
```

After definition, `when` expands at compile time:

```lykn
(when (> x 0)
  (console:log "positive")
  (console:log "very positive"))
```
```js
if (x > 0) {
  console.log("positive");
  console.log("very positive");
}
```

### import-macros

Import macros from a local file or published package. Only the listed
macros are imported.

**Local file** (relative path, must end with `.lykn` or `.lyk`):

```lykn
(import-macros "./lib.lykn" (when unless))
```

**Published package** via `jsr:` or `npm:` specifier (DD-34):

```lykn
(import-macros "jsr:@lykn/testing" (test is-equal ok))
```

**Bare name** resolved via the project import map in `project.json`:

```lykn
(import-macros "my-macros" (my-form))
```

Requires a matching entry in `project.json`:
```json
{ "imports": { "my-macros": "./packages/my-macros/" } }
```

Resolution uses a three-tier scheme:
1. **Scheme-prefixed** (`jsr:`, `npm:`, `file:`, `https:`) — delegated
   to Deno's module resolver
2. **Bare names** — looked up in the project import map
3. **Filesystem paths** (`./`, `../`) — relative to the importing file

The macro entry point in a resolved package is found via:
1. `deno.json` field `lykn.macroEntry`
2. Fallback files: `mod.lykn`, `mod.lyk`, `macros.lykn`, `macros.lyk`,
   `index.lykn`, `index.lyk`
3. `deno.json` field `exports` if it points to a `.lykn`/`.lyk` file

### Quasiquote (`` ` ``, `,`, `,@`)

| Syntax | Meaning |
|---|---|
| `` `(if ,test ,body) `` | Template with holes |
| `,expr` | Unquote — insert value |
| `,@expr` | Splice — insert list elements |

### gensym / #gen

Hygienic symbol generation for macros:

| Syntax | Purpose |
|---|---|
| `(gensym "prefix")` | Programmatic gensym |
| `temp#gen` | Auto-gensym suffix — each unique `name#gen` in a macro body gets a fresh symbol |

---

## JS Interop

### js: namespace

Surface forms for explicit JS interop:

| lykn | JS | Notes |
|---|---|---|
| `(js:typeof x)` | `typeof x` | typeof operator |
| `(js:eq a b)` | `a == b` | Loose equality |
| `(js:call console:log "hi")` | `console.log("hi")` | Explicit method call |
| `(js:bind obj:method obj)` | `obj.method.bind(obj)` | Bind method |
| `(js:eval code)` | `eval(code)` | eval |

### Kernel form passthrough

Any kernel form can be used directly in surface code. Surface macros only
transform recognized surface form heads; everything else passes through
to the kernel compiler unchanged.

---

## Kernel Functions

| lykn | JS | Notes |
|---|---|---|
| `(=> (a b) (+ a b))` | `(a, b) => a + b` | Arrow function |
| `(function add (a b) (return (+ a b)))` | `function add(a, b) { return a + b; }` | Declaration |
| `(lambda (a) (return a))` | `function(a) { return a; }` | Anonymous function expression |
| `(async (=> () (await (fetch url))))` | `async () => await fetch(url)` | Async wrapper |
| `(=> ((default x 0)) x)` | `(x = 0) => x` | Default parameter |
| `(function f (a (rest args)) (return args))` | `function f(a, ...args) { return args; }` | Rest parameter |
| `(function* gen () (yield 1) (yield 2))` | `function* gen() { yield 1; yield 2; }` | Generator |
| `(yield expr)` | `yield expr` | Yield value |
| `(yield* other)` | `yield* other` | Delegate to iterable |
| `(for-await-of item stream (process item))` | `for await (const item of stream) { process(item); }` | Async iteration |
| `(async (function* gen () ...))` | `async function* gen() { ... }` | Async generator |
| `(assign this:x value)` | `this.x = value` | Class body only (DD-27) |

**Class bodies** (DD-27): Surface forms expand inside `class`
method and constructor bodies. `=` is equality, `bind` produces
`const`, `set!` works, threading macros work. Use `assign` for
`this`-property assignment in constructors.

```lykn
(class Dog ()
  (constructor (name)
    (assign this:name name))
  (greet ()
    (bind msg (template "Hi, I'm " this:name))
    (return msg)))
```
```js
class Dog {
  constructor(name) { this.name = name; }
  greet() {
    const msg = `Hi, I'm ${this.name}`;
    return msg;
  }
}
```

---

## Destructuring (kernel forms)

### Object patterns

```lykn
(const (object name age) person)
```
```js
const {name, age} = person;
```

### Alias

```lykn
(const (object (alias data items)) obj)
```
```js
const {data: items} = obj;
```

### Default

```lykn
(const (object (default x 0)) point)
```
```js
const {x = 0} = point;
```

### Array patterns

```lykn
(const (array first (rest tail)) list)
```
```js
const [first, ...tail] = list;
```

### Skip with _

```lykn
(const (array _ _ third) arr)
```
```js
const [, , third] = arr;
```

---

## Classes (kernel forms)

```lykn
(class Dog (Animal)
  (field -count 0)
  (constructor ((name))
    (super name)
    (= this:-count (+ this:-count 1)))
  (speak () (return (+ this:name " barks")))
  (static (field species "Canine")))
```
```js
class Dog extends Animal {
  #_count = 0;
  constructor(name) {
    super(name);
    this.#_count = this.#_count + 1;
  }
  speak() {
    return this.name + " barks";
  }
  static species = "Canine";
}
```

| Feature | lykn | JS |
|---|---|---|
| Extends | `(class Dog (Animal) ...)` | `class Dog extends Animal { ... }` |
| Private field | `(field -count 0)` | `#_count = 0` |
| Static | `(static (field count 0))` | `static count = 0` |
| Private access | `this:-count` | `this.#_count` |

---

## Misc Kernel Forms

| lykn | JS | Notes |
|---|---|---|
| `(object (name "x") age)` | `{name: "x", age}` | Object literal (kernel) |
| `(array 1 2 (spread rest))` | `[1, 2, ...rest]` | Array literal (kernel) |
| `(seq a b c)` | `a, b, c` | Comma expression |
| `(typeof x)` | `typeof x` | typeof operator |
| `(debugger)` | `debugger` | Debugger statement |
| `(block stmts...)` | `{ stmts }` | Block scope |

---

## Known Gaps

The following forms appear in design documents (DD-15 through DD-21) or
the README but behave differently than documented, or are only available
in one compiler:

### JS compiler only (not in Rust compiler)

- **`#;` (expression comment)** — The `#;` reader dispatch for discarding
  the next form is implemented in the JS reader but not yet in the Rust
  reader. The Rust compiler reports `unknown dispatch character after #`.
- **`#|...|#` (block comment)** — Nestable block comments are
  implemented in the JS reader but not the Rust reader.
- **`cons`, `list`, `car`, `cdr`** — Listed in the README as data
  literals producing linked-list structures (`[1, 2]`, `[1, [2, [3, null]]]`,
  `x[0]`, `x[1]`). In the Rust compiler, these compile as ordinary
  function calls. They require user-defined runtime functions.

### Fixed in v0.4.0 (DD-22)

The following v0.3.0 issues have been resolved:

- **`=` is now strict equality** — `(= a b)` compiles to `a === b`,
  matching every Lisp dialect. Assignment is handled by named forms
  (`bind`, `reset!`, `swap!`).
- **`!=` is now strict inequality** — `(!= a b)` compiles to `a !== b`.
- **`and`/`or`/`not` are now logical operators** — `(and x y)` compiles
  to `x && y` (short-circuit), not `and(x, y)` (function call).
