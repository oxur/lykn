# lykn

[![][build-badge]][build]
[![][crate-badge]][crate]
[![][tag-badge]][tag]
[![][docs-badge]][docs]

[![][logo]][logo-large]

*S-expression syntax for JavaScript*

**lykn** is a lightweight Lisp that compiles to clean, readable JavaScript. No runtime, no dependencies in the output — just JS you'd write by hand, but expressed in s-expressions.

lykn has two syntax layers: **surface syntax** for everyday code (typed
functions, algebraic data types, pattern matching, immutable bindings) and
**kernel syntax** for low-level control. Both are s-expressions; surface forms
compile to kernel forms, which compile to JavaScript.

The name means *good luck* in Norwegian, *luck* in Swedish, and — if you
squint at the Icelandic — *closure*.

## Status

**v0.5.0** — Workspace toolchain, testing DSL, and publishing pipeline.
`lykn` is now a unified CLI wrapping Deno: `lykn new`, `lykn run`,
`lykn test`, `lykn build --dist`, `lykn publish`. The language gains typed
bindings, destructured parameters, generator functions, surface forms in
class bodies, and cross-package macro resolution via `jsr:` / `npm:`
specifiers. Three published packages: `@lykn/lang`, `@lykn/browser`,
`@lykn/testing`. Both `.lykn` (surface) and `.lyk` (kernel) file extensions
are supported.

## Quick Start

```sh
# Install
brew install deno
cargo install lykn  # or: cargo build --release && cp target/release/lykn bin/

# Create a project
lykn new my-app
cd my-app

# Run it
lykn run packages/my-app/mod.lykn

# Run tests
lykn test
```

## Quick taste

```lisp
;; Immutable bindings
(bind greeting "hello, world")

;; Typed functions with runtime safety
(func greet
  :args (:string name)
  :returns :string
  :body (+ greeting ", " name "!"))

;; Threading macros
(bind result (-> 5 (+ 3) (* 2)))

;; Objects with keyword syntax
(bind user (obj :name "lykn" :version "0.5.0"))

;; Controlled mutation via cells
(bind counter (cell 0))
(swap! counter (=> (n) (+ n 1)))
(console:log (express counter))

;; Generator with typed yields
(genfunc range
  :args (:number start :number end)
  :yields :number
  :body
  (for (let i start) (< i end) (+= i 1)
    (yield i)))

;; Macros still work — define your own forms
(macro when (test (rest body))
  `(if ,test (block ,@body)))

(when (> result 0)
  (console:log "positive"))
```

Compiles to:

```js
const greeting = "hello, world";
function greet(name) {
  if (typeof name !== "string") throw new TypeError("greet: arg 'name' expected string, got " + typeof name);
  const result__gensym0 = greeting + ", " + name + "!";
  if (typeof result__gensym0 !== "string") throw new TypeError("greet: return value expected string, got " + typeof result__gensym0);
  return result__gensym0;
}
const result = (5 + 3) * 2;
const user = {name: "lykn", version: "0.5.0"};
const counter = {value: 0};
counter.value = ((n) => n + 1)(counter.value);
console.log(counter.value);
if (result > 0) {
  console.log("positive");
}
```

## Architecture

lykn has two compiler implementations sharing the same syntax and semantics:

**Rust compiler** (standalone binary, no runtime dependencies):

```
.lykn source → reader → expander → classifier → analyzer → emitter → codegen → JavaScript
```

**JS compiler** (browser bundle + Deno):

```
.lykn source → reader → surface macros → expander → compiler → astring → JavaScript
```

### Rust pipeline (`crates/`)

- **Reader** (`lykn-lang/reader`) — S-expression parser with source locations
- **Expander** (`lykn-lang/expander`) — macro expansion (user macros via Deno subprocess)
- **Classifier** (`lykn-lang/classifier`) — S-expressions → typed surface AST
- **Analyzer** (`lykn-lang/analysis`) — type registry, exhaustiveness checking,
  scope tracking, unused binding detection
- **Emitter** (`lykn-lang/emitter`) — surface forms → kernel S-expressions
- **Codegen** (`lykn-lang/codegen`) — kernel S-expressions → JavaScript text
  (pure Rust, no external dependencies)

### JS pipeline (`packages/lang/`)

- **Reader** (`packages/lang/reader.js`) — parses s-expressions, handles `#` dispatch
  (`` ` ``, `,`, `,@`, `#a(...)`, `#o(...)`, `#NNr`, `#;`, `#|...|#`),
  dotted pairs
- **Surface macros** (`packages/lang/surface.js`) — transforms high-level surface forms
  to kernel forms
- **Expander** (`packages/lang/expander.js`) — three-pass macro expansion pipeline
  (Bawden's quasiquote algorithm)
- **Compiler** (`packages/lang/compiler.js`) — kernel forms → ESTree AST → JS via
  [astring](https://github.com/davidbonnet/astring)
- **Browser shim** (`packages/browser/mod.js`) — 73KB bundle with `<script
  type="text/lykn">` support and `window.lykn` API

## Toolchain

```sh
brew install deno
cargo install lykn
```

### Test

```sh
lykn test                   # discover and run .lykn/.lyk tests
lykn test test/surface/     # test a specific directory
cargo test                  # Rust tests
```

### Lint & Format

```sh
lykn lint packages/         # lint compiled JS via Deno
cargo clippy                # lint Rust
cargo fmt                   # format Rust
lykn fmt main.lykn          # format lykn source
```

### Build & Publish

```sh
lykn build --dist           # stage all packages into dist/
lykn publish --jsr          # publish to JSR
lykn publish --npm          # publish to npm
lykn publish --jsr --dry-run  # verify without publishing
```

## Usage

### Browser

```html
<script src="dist/lykn-browser.js"></script>
<script type="text/lykn">
  ;; Macros work inline in the browser!
  (macro when (test (rest body))
    `(if ,test (block ,@body)))

  (bind el (document:query-selector "#output"))
  (when el
    (set! el:text-content "Hello from lykn!"))
</script>
```

Or use the API directly:

```js
lykn.compile('(+ 1 2)')   // → "1 + 2;\n"
lykn.run('(+ 1 2)')       // → 3
await lykn.load('/app.lykn')
```

> **Note:** `import-macros` is not available in the browser (no file system
> access). Inline `macro` definitions work.

### CLI

```sh
# Create a new project
lykn new my-app

# Compile .lykn to JavaScript
lykn compile main.lykn                    # output to stdout
lykn compile main.lykn -o main.js         # output to file
lykn compile main.lykn --strip-assertions # omit type checks / contracts

# Run directly
lykn run main.lykn

# Format
lykn fmt main.lykn                        # stdout
lykn fmt -w main.lykn                     # in place

# Syntax check
lykn check main.lykn

# Build browser bundle
lykn build --browser
```

Build from source:

```sh
mkdir -p ./bin
cargo build --release && cp ./target/release/lykn ./bin
```

### Run Examples

```sh
# Run a lykn file directly
lykn run examples/surface/main.lykn

# Or compile and run separately
lykn compile examples/surface/main.lykn -o /tmp/main.js && deno run /tmp/main.js

# Serve the browser examples
deno run --allow-net --allow-read jsr:@std/http@1/file-server --port 5099
# Then open http://localhost:5099/examples/surface/browser.html
```

Both `examples/surface/` (`.lykn`) and `examples/kernel/` (`.lyk`) are
available.

## Supported forms

### Surface forms

Surface syntax is the recommended way to write lykn. These forms expand to
kernel forms at compile time.

#### Bindings & mutation

| lykn | JS |
|---|---|
| `(bind x 1)` | `const x = 1;` |
| `(bind :number x (compute))` | `const x = compute();` with runtime type check |
| `(bind counter (cell 0))` | `const counter = { value: 0 };` |
| `(swap! counter f)` | `counter.value = f(counter.value);` |
| `(reset! counter 0)` | `counter.value = 0;` |
| `(express counter)` | `counter.value` |
| `(set! el:prop value)` | `el.prop = value;` |

#### Functions

| lykn | JS |
|---|---|
| `(func add :args (:number a :number b) :returns :number :body (+ a b))` | `function add(a, b) { ... return a + b; }` with type checks |
| `(func now (Date:now))` | `function now() { return Date.now(); }` |
| `(fn (:number x) (* x 2))` | `x => { ...; x * 2; }` with type check |
| `(func f :args ((object :string host :number port)) :body ...)` | Destructured params with per-field types |
| `(genfunc range :args (:number a :number b) :yields :number :body ...)` | Generator with typed yields |
| `(genfn () (yield 1) (yield 2))` | Anonymous generator |

#### Types & pattern matching

| lykn | JS |
|---|---|
| `(type Option (Some :any value) None)` | Constructor functions with `{ tag: "Some", value }` |
| `(match opt ((Some v) v) (None fallback))` | Exhaustive if-chain on `.tag` |
| `(if-let ((Some user) (find id)) (greet user) "none")` | Tag check + binding + branch |
| `(when-let ((Some user) (find id)) (greet user))` | Same without else branch |

#### Objects

| lykn | JS |
|---|---|
| `(obj :name "x" :age 42)` | `{ name: "x", age: 42 }` |
| `(assoc user :age 43)` | `{ ...user, age: 43 }` |
| `(dissoc user :password)` | Spread + delete |
| `(conj items new-item)` | `[...items, newItem]` |

#### Threading macros

| lykn | JS |
|---|---|
| `(-> x (+ 3) (* 2))` | `(x + 3) * 2` |
| `(->> items (filter even?) (map double))` | `map(filter(items, even), double)` |
| `(-> user (get :name) (:to-upper-case))` | `user["name"].toUpperCase()` |
| `(some-> user (get :name) (:to-upper-case))` | IIFE with null checks + method call |

#### Equality & logic

| lykn | JS |
|---|---|
| `(= a b)` | `a === b` (strict equality, not assignment) |
| `(!= a b)` | `a !== b` |
| `(= a b c)` | `a === b && b === c` (variadic pairwise) |
| `(and x y)` | `x && y` (short-circuit) |
| `(or x y)` | `x \|\| y` (short-circuit) |
| `(not x)` | `!x` |

### Kernel forms

Kernel forms are the compilation targets for surface macros. You can use
them directly for low-level control, JS interop, or when surface syntax
doesn't cover a specific JS feature.

#### Basics

| lykn | JS |
|---|---|
| `(const x 1)` | `const x = 1;` |
| `(let x 1)` | `let x = 1;` |
| `my-function` | `myFunction` |
| `console:log` | `console.log` |
| `this:-name` | `this.#_name` |
| `(get arr 0)` | `arr[0]` |

### Functions

| lykn | JS |
|---|---|
| `(=> (a b) (+ a b))` | `(a, b) => a + b` |
| `(function add (a b) (return (+ a b)))` | `function add(a, b) { return a + b; }` |
| `(lambda (a) (return a))` | `function(a) { return a; }` |
| `(async (=> () (await (fetch url))))` | `async () => await fetch(url)` |
| `(=> ((default x 0)) x)` | `(x = 0) => x` |
| `(function f (a (rest args)) ...)` | `function f(a, ...args) { ... }` |

### Modules

| lykn | JS |
|---|---|
| `(import "mod" (a b))` | `import {a, b} from "mod";` |
| `(import "mod" name)` | `import name from "mod";` |
| `(export (const x 42))` | `export const x = 42;` |
| `(export default my-fn)` | `export default myFn;` |
| `(dynamic-import "./mod.js")` | `import("./mod.js")` |

### Control flow

| lykn | JS |
|---|---|
| `(if cond a b)` | `if (cond) a; else b;` |
| `(? test a b)` | `test ? a : b` |
| `(for-of item items (f item))` | `for (const item of items) { f(item); }` |
| `(while cond body...)` | `while (cond) { body }` |
| `(try body (catch e ...) (finally ...))` | `try { body } catch(e) { ... } finally { ... }` |
| `(switch x ("a" (f) (break)) (default (g)))` | `switch(x) { case "a": f(); break; default: g(); }` |
| `(throw (new Error "oops"))` | `throw new Error("oops");` |

### Expressions

| lykn | JS |
|---|---|
| `(template "hi " name "!")` | `` `hi ${name}!` `` |
| `(tag html (template ...))` | `` html`...` `` |
| `(object (name "x") age)` | `{name: "x", age}` |
| `(array 1 2 (spread rest))` | `[1, 2, ...rest]` |
| `(regex "^hello" "gi")` | `/^hello/gi` |
| `(new Thing a b)` | `new Thing(a, b)` |

### Destructuring

| lykn | JS |
|---|---|
| `(const (object name age) person)` | `const {name, age} = person;` |
| `(const (array first (rest tail)) list)` | `const [first, ...tail] = list;` |
| `(const (object (alias data items)) obj)` | `const {data: items} = obj;` |
| `(const (object (default x 0)) point)` | `const {x = 0} = point;` |
| `(const (array _ _ third) arr)` | `const [, , third] = arr;` |

### Classes

Surface forms work inside class bodies — `bind`, `=` (equality), `set!`, threading macros all expand correctly.

| lykn | JS |
|---|---|
| `(class Dog (Animal) ...)` | `class Dog extends Animal { ... }` |
| `(assign this:name name)` | `this.name = name` (class body only) |
| `(field -count 0)` | `#_count = 0;` |
| `(get area () (return x))` | `get area() { return x; }` |
| `(static (field count 0))` | `static count = 0;` |
| `(async (fetch-data () ...))` | `async fetchData() { ... }` |

### Operators

| lykn | JS | Notes |
|---|---|---|
| `(+ a b c)` | `a + b + c` | Arithmetic |
| `(++ x)` | `++x` | Prefix increment |
| `(+= x 1)` | `x += 1` | Compound assignment |
| `(** base exp)` | `base ** exp` | Exponentiation |
| `(?? a b)` | `a ?? b` | Nullish coalescing |
| `(=== a b)` | `a === b` | Kernel strict equality |
| `(= x 1)` | `x = 1` | Kernel assignment (surface `=` is equality) |

### Macros

| lykn | What it does |
|---|---|
| `` (macro when (test (rest body)) `(if ,test (block ,@body))) `` | Define a macro with quasiquote template |
| `(import-macros "./lib.lykn" (when unless))` | Import macros from a local file |
| `(import-macros "jsr:@lykn/testing" (test is-equal))` | Import macros from a published package |
| `` `(if ,test ,@body) `` | Quasiquote with unquote and splicing |
| `temp#gen` | Auto-gensym (hygienic binding) |
| `(gensym "prefix")` | Programmatic gensym |

### Data literals and sugar

| lykn | JS |
|---|---|
| `#a(1 2 3)` | `[1, 2, 3]` |
| `#o((name "x") (age 42))` | `{name: "x", age: 42}` |
| `#16rff` | `255` (radix literal) |
| `#2r11110000` | `240` (binary) |
| `(cons 1 2)` | `[1, 2]` |
| `(list 1 2 3)` | `[1, [2, [3, null]]]` |
| `(car x)` / `(cdr x)` | `x[0]` / `x[1]` |
| `#; expr` | Expression comment (discards next form) |
| `#\| ... \|#` | Nestable block comment |

## Testing

lykn includes a testing DSL via the `@lykn/testing` macro module:

```lykn
(import-macros "jsr:@lykn/testing" (test suite is-equal ok))

(suite "arithmetic"
  (test "addition"
    (is-equal (+ 1 1) 2))
  (test "positive"
    (ok (> 5 0))))
```

Compiles to `Deno.test()` + `@std/assert` — no runtime dependency or custom
test runner. Available assertions: `is`, `is-equal`, `ok`, `is-thrown`,
`matches`.

Test files use `*_test.lykn` / `*.test.lykn` (or `.lyk`) naming. Run with
`lykn test`.

## Design principles

- **Thin skin over JS.** lykn is not a new language. It's a syntax for the
  language you already have. The output should look like code you'd write.
- **No runtime.** Compiled lykn is just JS. Nothing extra ships to the
  browser.
- **Self-contained.** The Rust compiler is a single binary with no runtime
  dependencies. The browser bundle is 73KB. You can read the whole thing.
- **Two worlds.** Rust for the compiler and dev-side tooling (fast, single
  binary). JS for the browser bundle and in-browser `<script>` workflow.

## References

- [EBNF grammar](https://github.com/lykn-lang/grammar/tree/main/ebnf) — formal
  grammar specification for lykn syntax
- [ESTree spec](https://github.com/estree/estree) — the AST format lykn
  targets
- [astring](https://github.com/davidbonnet/astring) — ESTree to JS code
  generation
- [Bawden 1999](https://citeseerx.ist.psu.edu/document?repid=rep1&type=pdf&doi=bc26d7c81e2db498dce94bd79fa5b6a8d68f3e45) — "Quasiquotation in Lisp", the algorithm
  behind lykn's macro expansion
- [Fennel](https://fennel-lang.org/) — inspiration for enforced gensym
  hygiene model
- [eslisp](https://github.com/anko/eslisp) — spiritual ancestor; reference
  implementation
- [BiwaScheme](https://www.biwascheme.org/) — inspiration for the in-browser
  `<script>` workflow

## License

Apache-2.0

[//]: ---Named-Links---

[logo]: assets/images/logo/v1-y250.png
[logo-large]: assets/images/logo/v1.png
[build]: https://github.com/oxur/lykn/actions/workflows/ci.yml
[build-badge]: https://github.com/oxur/lykn/actions/workflows/ci.yml/badge.svg
[crate]: https://crates.io/crates/lykn
[crate-badge]: https://img.shields.io/crates/v/lykn.svg
[docs]: https://docs.rs/lykn/
[docs-badge]: https://img.shields.io/badge/rust-documentation-blue.svg
[tag-badge]: https://img.shields.io/github/tag/oxur/lykn.svg
[tag]: https://github.com/oxur/lykn/tags
