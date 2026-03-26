# lykn

[![][build-badge]][build]
[![][crate-badge]][crate]
[![][tag-badge]][tag]
[![][docs-badge]][docs]

[![][logo]][logo-large]

*S-expression syntax for JavaScript*

**lykn** is a lightweight Lisp that compiles to clean, readable JavaScript. No runtime, no dependencies in the output — just JS you'd write by hand, but expressed in s-expressions.

The name means *good luck* in Norwegian, *luck* in Swedish, and — if you
squint at the Icelandic — *closure*.

## Status

**v0.1.0** — Feature-complete compiler covering core JS: functions, classes, modules, destructuring, template literals, async/await, and more. 38KB browser bundle. 196 tests.

## Quick taste

```lisp
(import "node:fs" (read-file-sync))

(const greet (=> (name)
  (console:log (template "hello, " name "!"))))

(greet "world")

(class Dog (Animal)
  (field -name)
  (constructor (name)
    (super name)
    (= this:-name name))
  (speak ()
    (console:log (template this:-name " says woof!"))))

(const (object name (default age 0)) (get-user))
```

Compiles to:

```js
import {readFileSync} from "node:fs";
const greet = name => console.log(`hello, ${name}!`);
greet("world");
class Dog extends Animal {
  #_name;
  constructor(name) {
    super(name);
    this.#_name = name;
  }
  speak() {
    console.log(`${this.#_name} says woof!`);
  }
}
const {name, age = 0} = getUser();
```

## Architecture

lykn has two implementations that share a common s-expression syntax:

- **JS compiler** (`src/`) — reads `.lykn` files, emits ESTree AST, generates
  JS via [astring](https://github.com/davidbonnet/astring). Publishable to
  npm and jsr.io. Also targets in-browser compilation for `<script
  type="text/lykn">` workflows.

- **Rust tools** (`crates/lykn-cli/`) — linter, formatter, syntax checker, and
  eventually a REPL. Single binary, no runtime dependencies. Publishable to crates.io.

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
# JS
deno test

# Rust
cargo test
```

## Usage

### Browser

```html
<script src="dist/lykn-browser.js"></script>
<script type="text/lykn">
  (const el (document:query-selector "#output"))
  (= el:text-content "Hello from lykn!")
</script>
```

Or use the API directly:

```js
lykn.compile('(+ 1 2)')   // → "1 + 2;\n"
lykn.run('(+ 1 2)')       // → 3
await lykn.load('/app.lykn')
```

### Build Browser Bundle

```sh
deno task build:browser
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

### Basics

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

## Design principles

- **Thin skin over JS.** lykn is not a new language. It's a syntax for the
  language you already have. The output should look like code you'd write.
- **No runtime.** Compiled lykn is just JS. Nothing extra ships to the
  browser.
- **Small tools.** The compiler is ~1,500 lines. The browser bundle is 38KB
  minified. The formatter is ~80 lines.
- **Two worlds.** Use Rust for dev-side tooling (fast, single binary). Use JS
  for the compiler (because it targets JS and can run in the browser).

## References

- [ESTree spec](https://github.com/estree/estree) — the AST format lykn
  targets
- [astring](https://github.com/davidbonnet/astring) — ESTree to JS code
  generation
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
