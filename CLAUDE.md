# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is lykn?

lykn is a lightweight Lisp that compiles S-expressions to clean, readable JavaScript. It has two implementations sharing a common syntax:

- **JS compiler** (`src/`) — reads `.lykn` source, emits ESTree AST, generates JS via astring
- **Rust CLI tools** (`crates/`) — formatter and syntax checker, single binary

Zero runtime dependencies in compiled output.

## Writing Code

### JavaScript

This project does not use Node.js.

**For JavaScript Code Quality:**

1. **`assets/ai/ai-design/skills/nodeless-js/SKILL.md`** - Advanced Rust programming skill (**use this**)
2. **`assets/ai/ai-design/guides/js/*.md`** - Comprehensive JavaScript guidelines referenced by the skill

**Important:** Note that `assets/ai/ai-design` may be a synlink; check to be sure, before assuming there's no directory if a directory check failes. If a symlink check fails and you have confirmed that `assets/ai/ai-design` does not exist on the file system, ask permission to clone it:

```bash
git clone https://github.com/cnbb-design/ai-design assets/ai/ai-design
```

### Rust

**For Rust Code Quality:**

1. **`assets/ai/ai-rust/skills/claude/SKILL.md`** - Advanced Rust programming skill (**use this**)
2. **`assets/ai/ai-rust/guides/*.md`** - Comprehensive Rust guidelines referenced by the skill
3. **`assets/ai/CLAUDE-CODE-COVERAGE.md`** - Comprehensive test coverage guide
4. **This file (CLAUDE.md)** - Project-specific conventions only

**Important:** Note that `assets/ai/ai-rust` may be a synlink; check to be sure, before assuming there's no directory if a directory check failes. If a symlink check fails and you have confirmed that `assets/ai/ai-rust` does not exist on the file system, ask permission to clone it:

```bash
git clone https://github.com/oxur/ai-rust assets/ai/ai-rust
```

## Build commands

### Rust (Cargo workspace at project root)

```sh
cargo build --release        # build all crates
cargo clippy                 # lint
cargo fmt                    # format
cargo test                   # test
cargo publish --dry-run      # verify crates.io packaging
```

### JavaScript (Deno)

```sh
deno lint src/               # lint JS
deno test                    # test JS
deno publish --allow-slow-types  # publish to jsr.io (browser auth)
```

### Biome (JS formatting)

```sh
biome format src/            # check
biome format --write src/    # fix in place
```

### Makefile

`make help` lists all targets. Key ones: `make build`, `make build-release`, `make test`, `make lint`, `make format`, `make check` (build+lint+test), `make push` (pushes to all remotes).

## Architecture

### JS compiler pipeline (`src/`)

`reader.js` → parse source into S-expression AST (`{type: 'atom'|'string'|'number'|'list', value}`) → `compiler.js` → transform to ESTree nodes via built-in macros → `astring.generate()` → JS output.

`index.js` re-exports `read`, `compile`, `compileExpr` and provides a convenience `lykn(source)` function.

The `astring` dependency is mapped via import map in `deno.json` (`"astring"` → `"npm:astring@^1.9.0"`) so the source uses bare imports while Deno resolves through npm without node_modules.

### Rust workspace (`crates/`)

- **`lykn-cli`** — binary (`lykn`) + library. Contains `reader.rs` (S-expression parser, `SExpr` enum) and `formatter.rs` (pretty-printer, 80-char line width). CLI commands: `fmt`, `check`.
- **`lykn`** — umbrella library crate, re-exports `lykn_cli::reader` and `lykn_cli::formatter`.

Both crates use Rust edition 2024.

### Shared pattern

The JS reader and Rust reader are parallel implementations of the same S-expression grammar. Changes to the grammar should be reflected in both.

## Publishing

- **crates.io**: `make publish` (publishes in dependency order with rate-limit delays) or `make publish-one CRATE=lykn-cli`
- **jsr.io**: `deno publish --allow-slow-types` (config in `deno.json`)
- **npm**: `npm publish --access public` (scoped as `@lykn/lykn` in `package.json`)

## Git remotes

The project pushes to multiple remotes (macpro, github, codeberg). `make push` handles all three. `make remotes` configures them.
