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

**v0.3.0-dev** — Surface syntax with typed functions, ADTs, pattern matching, cells, threading macros. Full macro system with quasiquote, auto-gensym hygiene, and cross-module macro imports. 56KB browser bundle. 555 tests.

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
(bind user (obj :name "lykn" :version "0.3.0"))

;; Controlled mutation via cells
(bind counter (cell 0))
(swap! counter (=> (n) (+ n 1)))
(console:log (express counter))

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
  if (typeof result__gensym0 !== "string") throw new TypeError("greet: return 'result__gensym0' expected string, got " + typeof result__gensym0);
  return result__gensym0;
}
const result = (5 + 3) * 2;
const user = {
  name: "lykn",
  version: "0.3.0"
};
const counter = {
  value: 0
};
counter.value = (n => n + 1)(counter.value);
console.log(counter.value);
if (result > 0) {
  console.log("positive");
}
```

## Architecture

```
.lykn source → reader → surface macros → expander → compiler → astring → JavaScript
```

- **Reader** (`src/reader.js`) — parses s-expressions, handles `#` dispatch
  (`` ` ``, `,`, `,@`, `#a(...)`, `#o(...)`, `#NNr`, `#;`, `#|...|#`),
  dotted pairs

- **Surface macros** (`src/surface.js`) — transforms high-level surface forms
  (`bind`, `func`, `type`, `match`, `obj`, `cell`, threading macros) to
  kernel forms before macro expansion

- **Expander** (`src/expander.js`) — three-pass macro expansion pipeline.
  Resolves quasiquote (Bawden's algorithm), sugar forms (`cons`/`list`/
  `car`/`cdr`), user-defined macros, `import-macros`, `as` patterns

- **Compiler** (`src/compiler.js`) — transforms core forms to ESTree AST,
  generates JS via [astring](https://github.com/davidbonnet/astring)

- **Browser shim** (`src/lykn-browser.js`) — 56KB bundle with `<script
  type="text/lykn">` support and `window.lykn` API

- **Rust tools** (`crates/lykn-cli/`) — linter, formatter, syntax checker.
  Single binary, no runtime dependencies. Publishable to crates.io.

## Toolchain

```sh
brew install biome deno
```

### Lint

```sh
# JS (src/)
deno lint src/
biome lint src/

# Rust
cargo clippy
```

### Format

```sh
# JS (src/)
biome format src/
biome format --write src/    # fix in place

# Rust
cargo fmt
```

### Test

```sh
deno task test              # all tests
deno task test:unit         # unit tests only
deno task test:integration  # integration tests only
cargo test                  # Rust tests
```

## Usage

### Browser

```html
<script src="dist/lykn-browser.js"></script>
<script type="text/lykn">
  ;; Macros work inline in the browser!
  (macro when (test (rest body))
    `(if ,test (block ,@body)))

  (const el (document:query-selector "#output"))
  (when el
    (= el:text-content "Hello from lykn!"))
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

### Build Browser Bundle

```sh
deno task build
```

### Format (Rust)

```sh
# Build from source
mkdir -p ./bin
cargo build --release && cp ./target/release/lykn ./bin

# Format a file (stdout)
./target/release/lykn fmt main.lykn

# Format in place
./target/release/lykn fmt -w main.lykn

# Syntax check
./target/release/lykn check main.lykn
```

## Supported forms

### Surface forms

Surface syntax is the recommended way to write lykn. These forms expand to
kernel forms at compile time.

#### Bindings & mutation

| lykn | JS |
|---|---|
| `(bind x 1)` | `const x = 1;` |
| `(bind counter (cell 0))` | `const counter = { value: 0 };` |
| `(swap! counter f)` | `counter.value = f(counter.value);` |
| `(reset! counter 0)` | `counter.value = 0;` |
| `(express counter)` | `counter.value` |

#### Functions

| lykn | JS |
|---|---|
| `(func add :args (:number a :number b) :returns :number :body (+ a b))` | `function add(a, b) { ... return a + b; }` with type checks |
| `(func now (Date:now))` | `function now() { return Date.now(); }` |
| `(fn (:number x) (* x 2))` | `x => { ...; x * 2; }` with type check |

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
| `(some-> user (get :name) (str:to-upper-case))` | IIFE with null checks at each step |

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

| lykn | JS |
|---|---|
| `(class Dog (Animal) ...)` | `class Dog extends Animal { ... }` |
| `(field -count 0)` | `#_count = 0;` |
| `(get area () (return x))` | `get area() { return x; }` |
| `(static (field count 0))` | `static count = 0;` |
| `(async (fetch-data () ...))` | `async fetchData() { ... }` |

### Operators

| lykn | JS |
|---|---|
| `(+ a b c)` | `a + b + c` |
| `(++ x)` | `++x` |
| `(+= x 1)` | `x += 1` |
| `(** base exp)` | `base ** exp` |
| `(?? a b)` | `a ?? b` |

### Macros

| lykn | What it does |
|---|---|
| `` (macro when (test (rest body)) `(if ,test (block ,@body))) `` | Define a macro with quasiquote template |
| `(import-macros "./lib.lykn" (when unless))` | Import macros from another file |
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

## Design principles

- **Thin skin over JS.** lykn is not a new language. It's a syntax for the
  language you already have. The output should look like code you'd write.
- **No runtime.** Compiled lykn is just JS. Nothing extra ships to the
  browser.
- **Small tools.** The full pipeline (reader + expander + compiler) is ~3,000
  lines. The browser bundle is 56KB minified. You can read the whole thing.
- **Two worlds.** Use Rust for dev-side tooling (fast, single binary). Use JS
  for the compiler (because it targets JS and can run in the browser).

## References

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
[build]: https://github.com/oxur/lykn/actions/workflows/cicd.yml
[build-badge]: https://github.com/oxur/lykn/actions/workflows/cicd.yml/badge.svg
[crate]: https://crates.io/crates/lykn
[crate-badge]: https://img.shields.io/crates/v/lykn.svg
[docs]: https://docs.rs/lykn/
[docs-badge]: https://img.shields.io/badge/rust-documentation-blue.svg
[tag-badge]: https://img.shields.io/github/tag/oxur/lykn.svg
[tag]: https://github.com/oxur/lykn/tags
