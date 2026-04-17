---
number: 41
title: "DD-31: Test Runner CLI and Markdown/Literate Testing"
author: "fence annotation"
component: All
tags: [change-me]
created: 2026-04-17
updated: 2026-04-17
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-31: Test Runner CLI and Markdown/Literate Testing

**Status**: Decided
**Date**: 2026-04-16
**Session**: Testing infrastructure design conversations (2026-04-12, 2026-04-16)

## Summary

The `lykn test` CLI command compiles `.lykn` test files and invokes
Deno's test runner. It also supports testing lykn code blocks embedded
in Markdown files — verifying that book chapters, guides, and
documentation examples compile correctly and produce expected output.
This is a stepping stone toward full literate programming support.

## Decisions

### 1. `lykn test` command: compile then delegate

**Decision**: `lykn test` is a two-phase command: (1) compile all
matching `.lykn` test files to `.js`, then (2) invoke `deno test` on
the compiled output. The Rust binary handles compilation; Deno handles
execution.

**Usage**:

```sh
# Run all tests in current directory (recursive)
lykn test

# Run tests in a specific directory
lykn test src/

# Run a specific test file
lykn test src/math_test.lykn

# Pass flags through to Deno
lykn test --filter "addition" --fail-fast

# Run with coverage
lykn test --coverage

# Markdown testing (see Decision 5)
lykn test --docs docs/guides/
```

**Behavior**:

1. Discover files matching `**/*_test.lykn` and `**/*.test.lykn`
   (or explicit file arguments)
2. Compile each `.lykn` file to a `.js` file in the same directory
   (or a configurable output directory)
3. Invoke `deno test` on the compiled `.js` files, forwarding all
   unrecognized flags
4. Report exit code from Deno

**Rationale**: The Rust binary already has the full lykn compilation
pipeline. Deno already has the full test execution pipeline. `lykn test`
is the thin glue between them. No custom test runner logic in Rust
beyond compilation and Deno process spawning.

### 2. File discovery

**Decision**: `lykn test` discovers test files using the same glob
patterns as Deno, adapted for `.lykn`:

- `**/*_test.lykn`
- `**/*.test.lykn`
- `**/__tests__/**/*.lykn`

Explicit file/directory arguments override glob discovery.

**Rationale**: Matching Deno's conventions means users don't have to
learn new patterns. The `_test.lykn` suffix is preferred (Deno/Go
convention); `.test.lykn` is accepted (Jest/Vitest convention).

### 3. Compiled output location

**Decision**: By default, compiled test JS is written alongside the
source files (`math_test.lykn` → `math_test.js` in the same directory).
A `--out-dir` flag enables writing compiled files to a separate directory.

```sh
# Default: compile in-place
lykn test src/
# Produces src/math_test.js, runs deno test src/

# Separate output directory
lykn test src/ --out-dir .lykn-test-out/
# Produces .lykn-test-out/src/math_test.js, runs deno test .lykn-test-out/
```

**Rationale**: In-place compilation is simplest — import paths between
test files and source files work without rewriting. The `--out-dir`
option is for projects that want to keep compiled output out of their
source tree. The default `.lykn-test-out/` directory should be added
to `.gitignore`.

### 4. Flag passthrough to Deno

**Decision**: `lykn test` recognizes its own flags (`--docs`,
`--out-dir`) and passes all others through to `deno test`.

**lykn-specific flags**:

| Flag | Purpose |
|------|---------|
| `--docs <glob>` | Enable Markdown testing mode (Decision 5) |
| `--out-dir <dir>` | Write compiled JS to a separate directory |
| `--compile-only` | Compile but don't run (useful for CI caching) |

**Deno flags passed through** (non-exhaustive):

| Flag | Purpose |
|------|---------|
| `--filter <pattern>` | Run tests matching name pattern |
| `--fail-fast` | Stop on first failure |
| `--parallel` | Run test files in parallel |
| `--coverage` | Collect coverage data |
| `--reporter <name>` | Output format (pretty, dot, tap, junit) |
| `--watch` | Re-run on file changes |
| `--allow-*` | Deno permissions |

**Rationale**: lykn should not gatekeep Deno's features. Any new
Deno test flag works automatically without lykn CLI changes.

### 5. Markdown code block testing

**Decision**: `lykn test --docs <glob>` extracts lykn code blocks from
Markdown files and verifies them. This is the primary mechanism for
keeping the book and guide examples correct.

**Code block types** (determined by fence annotation):

| Fence | Behavior |
|-------|----------|
| `` ```lykn `` | **Compile check**: parse and compile; assert no errors |
| `` ```lykn,run `` | **Execute**: compile and run; assert no runtime errors |
| `` ```lykn,compile-fail `` | **Expect failure**: assert compilation fails (for anti-pattern examples) |
| `` ```lykn,skip `` | **Skip**: do not test this block |
| `` ```lykn,fragment `` | **Skip**: this is a partial expression, not compilable standalone |

**Output matching**: When a `` ```lykn `` block is immediately followed
by a `` ```js `` block (with optional "Compiles to:" text between them),
the tool compiles the lykn block and asserts the output matches the JS
block.

**Example Markdown**:

````markdown
```lykn
(bind max-retries 3)
```

Compiles to:

```js
const maxRetries = 3;
```
````

**Generated test** (internal, not visible to the user):

```javascript
Deno.test("docs/guides/01-core-idioms.md block 3", () => {
  const result = compile("(bind max-retries 3)");
  assertEquals(result.trim(), "const maxRetries = 3;");
});
```

**Discovery**: The Markdown tester scans for fenced code blocks
with `lykn` as the language identifier. Each block becomes a
`Deno.test()` case named `<file> block <n>` (1-indexed).

**Rationale**: The book has 37 chapters and the guides have 18 files.
All contain lykn code examples. Manual verification (the current CC
workflow of extracting each block, writing a temp file, running
`lykn compile`, comparing output) doesn't scale and breaks silently
when the compiler changes. Automated Markdown testing catches
regressions in CI.

### 6. Block accumulation: independent by default

**Decision**: Each Markdown code block is compiled independently.
Blocks do not share state or accumulate bindings.

**Exception — `lykn,continue` annotation**: When a block is annotated
with `` ```lykn,continue ``, it is concatenated with all preceding
`continue` blocks in the same document section (delimited by `##`
headings). This handles the common documentation pattern where a
type is defined in one block and used in a later block.

**Example**:

````markdown
```lykn,continue
(type Color Red Green Blue)
```

Later in the same section:

```lykn,continue
(match my-color
  Red   "stop"
  Green "go"
  Blue  "sky")
```
````

These two blocks are concatenated and compiled as a single unit.

**Rationale**: Independent blocks are simpler, more robust, and
match user expectations — each example should stand alone. The
`continue` escape hatch handles the legitimate case where a section
builds up a program incrementally. Section boundaries (`##` headings)
reset the accumulator, preventing cross-section coupling.

### 7. Output matching semantics

**Decision**: When comparing compiled lykn output to an expected JS
block, the comparison is **whitespace-normalized**: leading/trailing
whitespace is trimmed, and internal whitespace sequences are collapsed
to single spaces, UNLESS the expected block contains explicit newlines
(detected by the presence of `\n` or multi-line content), in which
case exact matching is used after trimming.

**Rationale**: Many Markdown examples show single-line output where
the compiler might emit trailing newlines or semicolons. Whitespace
normalization prevents false failures from formatting differences.
Multi-line expected output (function bodies, etc.) uses exact matching
because formatting is semantically meaningful to the reader.

### 8. Markdown tester implementation

**Decision**: The Markdown tester is implemented in Rust as part of
the `lykn-cli` crate. It parses Markdown (simple fence-block
extraction, not full Markdown parsing), compiles extracted blocks
using the existing compilation pipeline, and generates a temporary
`.js` test file that Deno executes.

**Workflow**:

```
lykn test --docs docs/guides/01-core-idioms.md
  → Parse Markdown, extract fenced blocks
  → For each testable block, generate a Deno.test() call
  → Write temporary test file: .lykn-test-out/docs__guides__01-core-idioms.md.test.js
  → Invoke: deno test .lykn-test-out/docs__guides__01-core-idioms.md.test.js
  → Clean up (or keep with --no-cleanup for debugging)
```

**Rationale**: Rust handles the Markdown parsing and block extraction
(fast, no dependencies). The generated test file uses the same
`Deno.test()` + `@std/assert` pattern as hand-written tests, so
the output is familiar and debuggable. Temporary files are cleanable.

### 9. Literate programming — future direction

**Decision**: DD-31 establishes the *infrastructure* for literate
programming but does not define a full literate programming system.
The `lykn,continue` annotation and block accumulation provide the
foundation. A future DD will define:

- Tangle (extract runnable code from Markdown)
- Weave (generate documentation from annotated code)
- Named blocks and cross-references
- Execution order vs. document order

**Rationale**: Literate programming is a significant design surface.
Getting Markdown testing right is the prerequisite — it proves the
block extraction, compilation, and verification pipeline. Once that
works, literate programming features are incremental additions.

## Rejected Alternatives

### Separate `lykn-doctest` binary

**What**: A standalone tool for Markdown testing, separate from
`lykn test`.

**Why rejected**: Unnecessary complexity. `--docs` is a flag, not a
separate tool. Shares the same compilation pipeline, same Deno
delegation, same output format. One tool, one command.

### Full Markdown parser dependency

**What**: Use a Markdown parsing library (pulldown-cmark, comrak)
for proper AST-based block extraction.

**Why rejected**: Overkill for fence-block extraction. Fenced code
blocks have a trivial grammar: `` ``` `` on its own line opens, the
next `` ``` `` on its own line closes. A ~50-line scanner handles
this correctly. A full parser adds a Rust dependency for no benefit.
If edge cases arise later, we can upgrade.

### Block accumulation as default

**What**: All blocks in a document share state, building up a
program incrementally (like a REPL session).

**Why rejected**: Fragile. Moving, removing, or adding a block
changes the meaning of all subsequent blocks. Independent-by-default
with explicit `continue` opt-in is safer and matches how users
actually read documentation — each example should be self-contained.

### Virtual module accumulation

**What**: Maintain a "virtual module" that accumulates bindings across
blocks, simulating a persistent REPL.

**Why rejected**: Complex implementation (would need incremental
compilation or a module-level binding tracker). The `continue`
annotation achieves the same result with concatenation, which is
simple and predictable.

### String-exact output matching only

**What**: Always compare compiled output character-for-character.

**Why rejected**: Too brittle for documentation examples. A trailing
newline or semicolon difference causes false failures. Whitespace
normalization handles the common cases while preserving exactness
for multi-line output where formatting matters.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| No lykn blocks in Markdown file | Zero tests generated; file is skipped with info message | Guide with only prose |
| Empty lykn block | Skipped (no test generated) | `` ```lykn\n``` `` |
| `compile-fail` block that succeeds | Test fails — expected compilation error didn't occur | Anti-pattern example that becomes valid after compiler change |
| `continue` block after `##` heading | Accumulator resets; block is independent or starts new chain | Section boundary = fresh context |
| JS block without preceding lykn block | Ignored — JS blocks only matched when preceded by lykn block | Standalone JS example |
| Inline code (`` `(bind x 1)` ``) | Not tested — only fenced blocks | Inline examples are too short to test meaningfully |
| Multiple lykn blocks before one JS block | Only the immediately preceding lykn block is matched | Each lykn block is independently testable |
| lykn block in HTML comment | Not extracted — scanner only finds fenced blocks | `<!-- ```lykn ... ``` -->` |
| Windows line endings (CRLF) | Normalized to LF before comparison | Cross-platform compatibility |
| `--docs` combined with file arguments | File arguments are `.lykn` test files; `--docs` globs are Markdown files; both run | `lykn test src/ --docs docs/` |

## Dependencies

- **Depends on**: DD-30 (testing DSL — defines what test code looks
  like), the lykn compilation pipeline (Rust surface compiler +
  JS kernel compiler)
- **Affects**: Book CI workflow, guide CI workflow, future literate
  programming DD

## Open Questions

- [ ] **Watch mode for Markdown**: When `--watch` is combined with
  `--docs`, should file changes to `.md` files trigger recompilation
  and re-run? This requires Deno's watcher to know about `.md` files,
  which may need a `--watch` flag on the lykn side that watches `.md`
  files and re-invokes `deno test` on change.

- [ ] **Coverage for Markdown blocks**: Should `--coverage` work with
  `--docs`? The generated test file exercises `compile()`, so coverage
  would measure compiler code coverage, not the user code in the
  blocks. This is potentially useful for the lykn project itself but
  may be confusing for external users.

- [ ] **Error recovery in block extraction**: If one block fails to
  compile, should the tester continue with remaining blocks (collecting
  all failures) or stop? Collecting all failures is more useful for CI;
  stopping is more useful for interactive use. Probably controlled by
  `--fail-fast` passthrough.

- [ ] **`lykn,run` execution environment**: When a `lykn,run` block
  is compiled and executed, what permissions does it get? Probably
  `--allow-none` by default (pure computation only), with a way to
  annotate permissions in the Markdown. Syntax TBD.

- [ ] **Integration with mdbook**: mdbook has its own test command
  (`mdbook test`) for Rust code blocks. Should `lykn test --docs`
  output be compatible with mdbook's test infrastructure, or is it
  a completely separate tool? Probably separate — mdbook tests Rust,
  `lykn test --docs` tests lykn.
