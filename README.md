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

**v0.0.1** — This is an early proof of concept. The compiler handles core JS constructs. Design work is underway for the full language.

## Quick taste

```lisp
; main.lykn
(const greet (=> (name)
  ((. console log) (+ "hello, " name "!"))))

(greet "world")
```

Compiles to:

```js
const greet = name => console.log("hello, " + name + "!");
greet("world");
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

### Compile (JS)

```sh
# Install
npm install -g lykn      # or: deno install -g lykn

# Compile to stdout
lykn compile main.lykn

# Compile to file
lykn compile main.lykn -o main.js

# Pipe
echo '((. console log) "hi")' | lykn compile -
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

| lykn | JS |
|---|---|
| `(const x 1)` | `const x = 1;` |
| `(let x 1)` | `let x = 1;` |
| `(=> (a b) (+ a b))` | `(a, b) => a + b` |
| `(lambda (a) (return a))` | `function(a) { return a; }` |
| `((. console log) "hi")` | `console.log("hi");` |
| `(if cond a b)` | `if (cond) a; else b;` |
| `(+ a b c)` | `a + b + c` |
| `(array 1 2 3)` | `[1, 2, 3]` |
| `(object k1 v1 k2 v2)` | `{k1: v1, k2: v2}` |
| `(new Thing a b)` | `new Thing(a, b)` |

## Design principles

- **Thin skin over JS.** lykn is not a new language. It's a syntax for the
  language you already have. The output should look like code you'd write.
- **No runtime.** Compiled lykn is just JS. Nothing extra ships to the
  browser.
- **Small tools.** The compiler is ~400 lines. The formatter is ~80 lines.
  You can read the whole thing.
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
