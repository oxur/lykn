---
number: 42
title: "DD-32: Test Suite Migration — JS Tests to lykn"
author: "bootstrapping risk"
component: All
tags: [change-me]
created: 2026-04-17
updated: 2026-04-17
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-32: Test Suite Migration — JS Tests to lykn

**Status**: Decided
**Date**: 2026-04-17
**Session**: Testing infrastructure design conversations (2026-04-12, 2026-04-16, 2026-04-17)

## Summary

The existing 1,372-test suite is entirely JS and Rust. This DD defines
the plan, sequencing, and constraints for migrating the JS tests to
lykn using the DD-30 testing DSL. Migration proceeds in tiers ordered
by bootstrapping risk: surface tests first (lowest risk, highest
signal), expander tests last (highest risk — they test the macro
system that compiles the tests). Rust tests are out of scope.

## Decisions

### 1. Migration tiers and sequencing

**Decision**: JS tests migrate to lykn in five tiers, ordered by
bootstrapping distance — how many compilation layers separate the
test from the code being tested.

| Tier | Tests | Files | Count | Bootstrapping risk | Migration tool |
|------|-------|-------|-------|--------------------|----------------|
| 1 | Surface form tests | `test/surface/*.test.js` | 16 | Lowest — tests call `compile()` on surface input strings | `test-compiles` |
| 2 | Integration tests | `test/integration/*.test.js` | 4 | Low — test macro features end-to-end via `compile()` | `test-compiles` + custom assertions |
| 3 | Kernel form tests | `test/forms/*.test.js` | 30 | Low — same `compile()` pattern, kernel-level inputs | `test-compiles` |
| 4 | Reader tests | `test/reader/*.test.js` | 8 | Medium — test the reader that parses the test file itself | `test` + `is-equal` on reader output |
| 5 | Expander tests | `test/expander/*.test.js` | 17 | Highest — test the macro system that compiles test macros | `test` + custom assertions |

**Total JS tests in scope**: ~75 test files (the individual test case
count within them is higher).

**Out of scope**: Rust tests in `crates/lykn-lang/tests/` (3 files +
inline `#[cfg(test)]` blocks). These stay in Rust permanently.

**Rationale**: Tier 1 has near-zero bootstrapping risk because surface
tests treat the compiler as a black box — they pass a string to
`compile()` and check the output string. The test file's own
compilation is independent of the code being tested. Each subsequent
tier moves closer to testing the machinery that compiles the test
file itself.

### 2. Tier 1 — Surface form tests (first migration)

**Decision**: The 16 surface test files are the first to migrate.
They currently use JSON fixture files (`test/fixtures/surface/*.json`)
containing `{ input, output }` pairs.

**Current pattern** (JS):

```javascript
// test/surface/bind.test.js
import { assertEquals } from "jsr:@std/assert";
import { compileSurface } from "../helpers.js";

const fixtures = JSON.parse(
  Deno.readTextFileSync("test/fixtures/surface/bind.json")
);

for (const { input, output } of fixtures) {
  Deno.test(`bind: ${input}`, () => {
    assertEquals(compileSurface(input), output);
  });
}
```

**Migrated pattern** (lykn):

```lisp
;; test/surface/bind_test.lykn
(import-macros "lykn-testing" (test test-compiles is-equal))
(import "../../packages/lykn/mod.js" (compile))

(test-compiles "bind simple"
  "(bind x 1)" "const x = 1;")

(test-compiles "bind with array"
  "(bind :array users #a())" "const users = [];")

(test-compiles "bind with object destructuring"
  "(bind (object name age) person)"
  "const {name, age} = person;")
```

**Fixture file handling**: The JSON fixture files are *not* migrated
to a new format. Instead, each `{ input, output }` pair from the JSON
file becomes a `test-compiles` call in the `.lykn` test file. This
is a one-time expansion — the fixtures are inlined into the test source.

**Rationale**: JSON fixtures were a reasonable approach when tests were
in JS and lykn didn't exist yet. Now that tests can be written in lykn,
inlining the test cases is clearer, more maintainable, and doesn't
require a fixture-loading mechanism. Each test case has a descriptive
name instead of being an anonymous array entry.

**Migration per file**:

| JS test file | Fixture file | lykn test file |
|---|---|---|
| `test/surface/bind.test.js` | `test/fixtures/surface/bind.json` | `test/surface/bind_test.lykn` |
| `test/surface/func.test.js` | `test/fixtures/surface/func.json` | `test/surface/func_test.lykn` |
| `test/surface/match.test.js` | `test/fixtures/surface/match.json` | `test/surface/match_test.lykn` |
| `test/surface/type.test.js` | `test/fixtures/surface/type.json` | `test/surface/type_test.lykn` |
| `test/surface/cell.test.js` | `test/fixtures/surface/cell.json` | `test/surface/cell_test.lykn` |
| `test/surface/threading.test.js` | `test/fixtures/surface/threading.json` | `test/surface/threading_test.lykn` |
| `test/surface/equality.test.js` | `test/fixtures/surface/equality.json` | `test/surface/equality_test.lykn` |
| `test/surface/fn-lambda.test.js` | `test/fixtures/surface/fn-lambda.json` | `test/surface/fn-lambda_test.lykn` |
| `test/surface/obj.test.js` | `test/fixtures/surface/obj.json` | `test/surface/obj_test.lykn` |
| `test/surface/conditional-binding.test.js` | `test/fixtures/surface/conditional-binding.json` | `test/surface/conditional-binding_test.lykn` |
| `test/surface/some-threading.test.js` | `test/fixtures/surface/some-threading.json` | `test/surface/some-threading_test.lykn` |
| `test/surface/immutable-updates.test.js` | (inline) | `test/surface/immutable-updates_test.lykn` |
| `test/surface/js-interop.test.js` | (inline) | `test/surface/js-interop_test.lykn` |
| `test/surface/integration.test.js` | (inline) | `test/surface/integration_test.lykn` |
| `test/surface/genfunc.test.js` | (inline) | `test/surface/genfunc_test.lykn` |
| `test/surface/func-destructuring.test.js` | `test/fixtures/surface/func-destructuring.json` | `test/surface/func-destructuring_test.lykn` |

### 3. Tier 2 — Integration tests

**Decision**: The 4 integration test files migrate after Tier 1.
These test macro features end-to-end and may require more than
`test-compiles` — some test runtime behavior of compiled macros.

**Current files**:

| JS test file | lykn test file |
|---|---|
| `test/integration/control-flow-macros.test.js` | `test/integration/control-flow-macros_test.lykn` |
| `test/integration/data-structure-macros.test.js` | `test/integration/data-structure-macros_test.lykn` |
| `test/integration/gensym-hygiene.test.js` | `test/integration/gensym-hygiene_test.lykn` |
| `test/integration/macro-module.test.js` | `test/integration/macro-module_test.lykn` |

**Pattern**: These may use a mix of `test-compiles` (for output
verification) and `test`/`test-async` with direct assertions (for
runtime behavior checks).

### 4. Tier 3 — Kernel form tests

**Decision**: The 30 kernel form test files migrate after Tier 2.
Same `test-compiles` pattern — they pass kernel-level lykn input
to `compile()` and check JS output.

**Bootstrapping note**: Kernel form tests pass kernel syntax (e.g.,
`(const x 1)`) to the compiler, not surface syntax. The test file
itself is written in surface lykn. There is no conflict — the test
file's own compilation uses the surface compiler, while the strings
being tested are kernel-level input processed by the JS kernel
compiler.

**Example**:

```lisp
;; test/forms/async-await_test.lykn
(import-macros "lykn-testing" (test-compiles))
(import "../../packages/lykn/mod.js" (compile))

(test-compiles "async function"
  "(async (function fetchData () (await (fetch url))))"
  "async function fetchData() {\n  await fetch(url);\n}")

(test-compiles "top-level await"
  "(await (fetch url))"
  "await fetch(url);")
```

### 5. Tier 4 — Reader tests

**Decision**: The 8 reader test files migrate after Tier 3. These
test the reader/parser and have a different pattern — they call
reader functions and check the resulting AST structure.

**Bootstrapping consideration**: The reader being tested is the same
reader that parses the test file. However, this is not a practical
risk: the reader has been stable since v0.1.0, and a reader bug that
broke the test file would also break all other `.lykn` files,
making it immediately obvious.

**Pattern**:

```lisp
;; test/reader/keywords_test.lykn
(import-macros "lykn-testing" (test is-equal))
(import "../../packages/lykn/reader.js" (read-string))

(test "keyword reads as string"
  (is-equal (read-string ":name") "name"))

(test "keyword with hyphen converts to camelCase"
  (is-equal (read-string ":first-name") "firstName"))
```

### 6. Tier 5 — Expander tests (last migration)

**Decision**: The 17 expander test files migrate last. These test
the macro expansion pipeline — the very system that compiles the
`import-macros`, `test`, `is-equal`, etc. forms used in the test
file.

**Bootstrapping constraint**: If a macro expansion bug breaks the
test macros, the expander tests themselves would fail to compile,
not fail with incorrect assertions. This is actually *acceptable*
— a compile failure is a loud, obvious signal. The risk is not
silent incorrectness but noisy inability to run.

**Mitigation**: Keep the original JS expander tests as a fallback
during Tier 5 migration. Delete them only after the lykn versions
are green and stable for a full release cycle.

**Pattern**: Expander tests call internal functions like
`macroExpand`, `expandAll`, etc. These require importing expander
internals:

```lisp
;; test/expander/quasiquote_test.lykn
(import-macros "lykn-testing" (test is-equal))
(import "../../packages/lykn/expander.js" (macro-expand-all))

(test "quasiquote with unquote"
  (bind result (macro-expand-all (read-string "`(a ,b c)")))
  (is-equal result (read-string "(array (quote a) b (quote c))")))
```

### 7. Migration workflow per file

**Decision**: Each test file migrates via this process:

1. **Read the JS test file** and its fixture file (if any)
2. **Create the `.lykn` test file** with equivalent test cases
3. **Compile** the `.lykn` file: `lykn compile <file>`
4. **Run both** the original JS test and the new compiled JS test
5. **Verify identical results** — same number of tests, same
   pass/fail outcomes
6. **Delete the JS test file** and its fixture file (if any)
7. **Update `deno.json` task** if test paths changed

**Rationale**: Running both side-by-side before deleting ensures
no tests are lost or subtly changed during migration.

### 8. Fixture file disposition

**Decision**: JSON fixture files in `test/fixtures/surface/` are
deleted after their contents are inlined into `.lykn` test files.
The `test/fixtures/e2e/` directory (4 `.lykn` fixture files) is
retained — these are end-to-end test inputs, not fixture data.
The `test/fixtures/macros/` directory (5 `.lykn` files) is retained
— these are macro module fixtures imported by expander tests.

### 9. Test count verification

**Decision**: After each tier's migration is complete, run a test
count verification:

```sh
# Before migration: count JS test cases
deno test --reporter=tap 2>&1 | grep "^ok\|^not ok" | wc -l

# After migration: count should be identical
lykn test --reporter=tap 2>&1 | grep "^ok\|^not ok" | wc -l
```

No test cases may be dropped during migration. If a fixture pair
is genuinely redundant, document the removal explicitly in the
migration PR.

## Rejected Alternatives

### Automated JS-to-lykn test converter

**What**: Write a tool that automatically converts `.test.js` files
to `_test.lykn`.

**Why rejected**: The test patterns are varied enough (fixture-based,
inline, async, import-heavy) that an automated converter would
produce ugly, unidiomatic lykn. Hand migration produces better test
code and forces review of each test's continued relevance.

### Keeping both JS and lykn tests permanently

**What**: Maintain the JS tests alongside lykn tests as a redundant
safety net.

**Why rejected**: Duplication breeds drift. Two test suites testing
the same thing will diverge over time as one gets updated and the
other doesn't. The side-by-side verification during migration
(Decision 7) provides the safety net; after that, the JS version
is deleted.

### Migrating all tiers simultaneously

**What**: Convert all 75 test files in one effort.

**Why rejected**: Too large a changeset to review. Tier-by-tier
migration produces reviewable PRs, catches problems early, and
lets each tier's lessons inform the next. It also validates the
testing DSL incrementally — if the macros have design issues,
Tier 1 reveals them before we've converted everything.

### Starting with expander tests

**What**: Migrate the most complex tests first to stress-test the
DSL.

**Why rejected**: Maximum bootstrapping risk for minimum learning.
Start with the simplest tier (surface tests) to validate that the
DSL works at all, then work toward complexity.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Fixture pair with no descriptive name | Generate name from input: `"bind: (bind x 1)"` | Fixture pairs are anonymous in JSON |
| Test that uses Deno-specific APIs | Use `js:` interop for Deno API calls in lykn | `(js:Deno.readTextFileSync path)` |
| Test with dynamic test generation (loop) | Use lykn's `for` or convert to individual `test-compiles` calls | Fixture-loading loops become inlined tests |
| Test that imports test helpers | Migrate helpers to `.lykn` or keep as `.js` with `import` | Case-by-case decision |
| Multiline expected output in `test-compiles` | Use lykn template literals: `` (template "line1\nline2") `` | Function body output |
| Test that asserts on error messages | Use `is-thrown` with message argument | `(is-thrown (compile bad-input) CompileError "msg")` |
| Skipped/ignored tests in JS | Use Deno's test options via passthrough (pending DD-30 open question) | Skip annotation TBD |

## Dependencies

- **Depends on**: DD-30 (testing DSL must be implemented first),
  DD-31 (`lykn test` CLI for running compiled tests)
- **Affects**: CI configuration (test commands change), contributor
  documentation (how to write/run tests), the lykn book (testing
  chapter examples)

## Open Questions

- [ ] **Helper module migration**: Several JS test files import
  shared helpers (e.g., `compileSurface`, fixture loaders). Should
  these helpers be migrated to lykn, kept as JS with cross-language
  import, or eliminated by inlining their logic into `test-compiles`?

- [ ] **CI pipeline ordering**: Should CI run Rust tests first (fast,
  no bootstrapping risk), then lykn tests? This provides a safety
  net — if the Rust tests pass but lykn tests fail to compile, the
  issue is in the JS compilation layer, not the core.

- [ ] **Migration timeline**: Should all five tiers be completed
  before the public launch, or is Tier 1 + Tier 2 sufficient for
  launch with remaining tiers as fast-follows?
