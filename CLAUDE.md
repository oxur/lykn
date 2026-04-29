# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is lykn?

lykn is a lightweight Lisp that compiles S-expressions to clean, readable JavaScript. It has two implementations sharing a common syntax:

- **JS compiler** (`packages/lang/`) — reads `.lykn` source, emits ESTree AST, generates JS via astring
- **Rust CLI tools** (`crates/`) — compiler, formatter, syntax checker, and Deno wrapper — single binary

Zero runtime dependencies in compiled output.

## Writing Code

### JavaScript

This project does not use Node.js.

**For JavaScript Code Quality:**

1. **`assets/ai/ai-design/skills/nodeless-js/SKILL.md`** - Advanced JavaScript programming skill (**use this**)
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

### Lykn

**For Lykn Code Quality:**

1. **`assets/ai/SKILL.md`** - Advanced Lykn programming skill (**use this**)
2. **`docs/guides/*`** - Comprehensive Lykn guidelines referenced by the skill

## Build commands

### Project setup

```sh
lykn new my-project            # create new project
```

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
deno lint packages/          # lint JS
deno test --config project.json -A test/  # test JS
deno publish                 # publish to jsr.io
```

### Snapshot testing (insta)

Publishing pipeline tests use `insta` for golden-file snapshot testing.
Snapshot files live under `crates/lykn-cli/src/snapshots/` and are
committed. When generated output changes, tests fail with a diff.

```sh
cargo insta test                # run tests, show pending snapshots
cargo insta review              # interactive review of snapshot changes
cargo insta test --review       # run tests then review
```

Never auto-accept snapshots — review each change to verify it is
intentional. See DD-35 (`workbench/dd-35-testing-strategy-publishing-pipeline.md`)
for the full testing strategy.

## Lykn CLI safety gates

The Lykn CLI wraps underlying tools (deno, cargo, npm, git). Wrapping
must not weaken those tools' safety guarantees. Specifically:

**Never auto-pass safety-bypass flags to underlying tools.** The
Lykn CLI must NOT silently inject `--allow-dirty`, `--force`,
`--no-verify`, `--unsafe`, `--skip-checks`, or any equivalent
skip-gate flag when invoking deno, cargo, npm, or git. Safety gates
from underlying tools are part of the user's expected experience; the
Lykn CLI does not own the authority to bypass them on the user's
behalf.

**When an underlying gate fires, the resolution is to satisfy the
gate's condition.** If `cargo publish` rejects a dirty working tree,
the answer is `git commit` or `git stash`, not auto-passing
`--allow-dirty`. If `deno publish` rejects untracked files, the
answer is to track or remove them, not to add a wrapper flag that
silently includes `--allow-dirty`. The user retains the choice; the
Lykn CLI surfaces the gate honestly.

**If a gate's default behaviour is genuinely wrong for the project's
use case, that is a methodology question, not a wrapper patch.**
Raise it as a design discussion in the philosophy doc or as a DD,
not as a quiet behavioural change in the CLI. The 0.6.0 direction
(per `docs/philosophy.md` §Decided design questions #4) is to *more
strongly* enforce the uncommitted-changes check, not less — adding
an opt-in `--allow-dirty` flag at the lykn level for users who
explicitly accept the risk, with the default being a hard fail.

**Examples of operations this rule covers:**

- `lykn publish` (any subcommand) must not auto-pass `--allow-dirty`
  to deno/cargo/npm.
- `lykn publish` must not auto-pass `--no-verify` (which would skip
  signing / signature checks).
- `lykn run` / `lykn test` must not auto-pass deno permission flags
  (`--allow-all`, `--allow-net`, etc.) the user didn't request.
- `lykn build --dist` must not silently overwrite tracked files in
  ways the user can't undo with `git checkout`.
- Any future Lykn CLI subcommand wrapping a tool with safety gates
  must respect them by default.

Origin: this rule was crystallized after a M3.5 incident where the
Lykn CLI was patched to auto-pass `--allow-dirty` to `deno publish`
during dry-runs (commit `64bb301`, subsequently reverted). The
reasoning at the time ("dry-runs are verification, not publication —
untracked files should not block publishability checks") was
rhetorically clean but operationally wrong: dry-runs against an
uncommitted tree don't tell the user what would actually publish, so
weakening the gate defeats the purpose.

See also: `docs/philosophy.md` Principle 2 (Lykn-only tooling); the
"Snapshot testing (insta)" rule above; LEDGER_DISCIPLINE.md (the
discipline of not silently rewriting safety-relevant defaults).

### Makefile

`make help` lists all targets. Key ones: `make build`, `make build-release`, `make test`, `make lint`, `make format`, `make check` (build+lint+test), `make push` (pushes to all remotes).

## Architecture

### JS compiler pipeline (`packages/lang/`)

`reader.js` → parse source into S-expression AST (`{type: 'atom'|'string'|'number'|'list', value}`) → `compiler.js` → transform to ESTree nodes via built-in macros → `astring.generate()` → JS output.

`mod.js` re-exports `read`, `compile`, `compileExpr` and provides a convenience `lykn(source)` function.

The `astring` dependency is mapped via import map in `project.json` (`"astring"` → `"npm:astring@^1.9.0"`) so the source uses bare imports while Deno resolves through npm without node_modules.

### Rust workspace (`crates/`)

- **`lykn-cli`** — binary (`lykn`) + library. Contains `reader.rs` (S-expression parser, `SExpr` enum) and `formatter.rs` (pretty-printer, 80-char line width). CLI commands: `fmt`, `check`.
- **`lykn`** — umbrella library crate, re-exports `lykn_cli::reader` and `lykn_cli::formatter`.

Both crates use Rust edition 2024.

### Shared pattern

The JS reader and Rust reader are parallel implementations of the same S-expression grammar. Changes to the grammar should be reflected in both.

## Publishing

All JS packages publish from `dist/` (never directly from source). `lykn build --dist` stages each workspace member into `dist/<name>/` with generated `deno.json` and `package.json`. Three package kinds: `runtime` (compile .lykn → .js), `macro-module` (copy .lykn + .js, generate stub), `tooling` (copy .js).

```sh
lykn build --dist              # stage all packages into dist/
lykn publish --jsr             # publish to JSR from dist/
lykn publish --npm             # publish to npm from dist/
lykn publish --jsr --dry-run   # verify without publishing
make publish                   # publishes to JSR, npm, and crates.io
```

- **crates.io**: `make publish-crates` (publishes in dependency order with rate-limit delays) or `make publish-one CRATE=lykn-cli`

## Git remotes

The project pushes to multiple remotes (macpro, github, codeberg). `make push` handles all three. `make remotes` configures them.
