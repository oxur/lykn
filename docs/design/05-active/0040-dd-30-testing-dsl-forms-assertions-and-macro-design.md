---
number: 40
title: "DD-30: Testing DSL — Forms, Assertions, and Macro Design"
author: "macro expansion"
component: All
tags: [change-me]
created: 2026-04-17
updated: 2026-04-17
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-30: Testing DSL — Forms, Assertions, and Macro Design

**Status**: Decided
**Date**: 2026-04-16
**Session**: Testing infrastructure design conversations (2026-04-12, 2026-04-16)

## Summary

lykn's testing infrastructure is a macro module (`packages/testing/`)
that provides test definition and assertion forms. Tests written in lykn
compile to `Deno.test()` + `@std/assert` calls — no runtime dependency,
no custom test runner. The DSL borrows uvu's minimalist naming philosophy
while exposing the full capability of Deno's test runner through
keyword-labeled clauses inspired by `func`'s contract syntax (DD-16).

## Decisions

### 1. Architecture: macro module over Deno's test runner

**Decision**: The testing DSL is a lykn macro module at
`packages/testing/`. Test macros expand to JS that imports from
`Deno.test` and `jsr:@std/assert`. No custom test runner, no additional
runtime dependency. Deno handles discovery, execution, parallelism,
coverage, reporters, and sanitizers.

**Rationale**: Thin skin over Deno. Deno already provides test discovery
(`{*_,*.,}test.{js,ts,...}`), parallel execution, watch mode, `--filter`,
`--fail-fast`, coverage via `--coverage`, four reporters (pretty, dot,
TAP, JUnit), and resource/op/exit sanitizers. Reimplementing any of
this would violate lykn's "don't reinvent what the platform gives you"
principle. The only missing piece is the *authoring surface* — a way
to write tests in lykn syntax.

**Import**:

```lisp
;; Test files import the testing macros
(import-macros "testing"
  (test test-async suite step
   is is-equal is-not-equal is-strict-equal
   ok is-thrown is-thrown-async
   matches includes has obj-matches))
```

**Compiled output imports** (emitted by macro expansion):

```javascript
import { assertEquals, assertNotEquals, assertStrictEquals,
         assertExists, assertThrows, assertRejects,
         assertMatch, assertStringIncludes, assertArrayIncludes,
         assertObjectMatch, assert } from "jsr:@std/assert";
```

**ESTree nodes**: `ImportDeclaration`, `CallExpression`

### 2. Test definition form: `test`

**Decision**: `test` is the primary test definition form. It takes a
string name and a body. It supports optional keyword clauses `:setup`,
`:teardown`, and `:body`.

**Syntax — minimal form**:

```lisp
;; Simple test — name and body expressions
(test "addition works"
  (is-equal (+ 1 2) 3)
  (is-equal (* 3 4) 12))
```

```javascript
Deno.test("addition works", () => {
  assertEquals(1 + 2, 3);
  assertEquals(3 * 4, 12);
});
```

**Syntax — keyword-clause form**:

```lisp
;; Structured test with setup and teardown
(test "database query"
  :setup    (bind db (create-temp-db))
  :teardown (close db)
  :body
    (bind result (query db "SELECT 1"))
    (is-equal result 1))
```

```javascript
Deno.test("database query", () => {
  const db = createTempDb();
  try {
    const result = query(db, "SELECT 1");
    assertEquals(result, 1);
  } finally {
    close(db);
  }
});
```

**ESTree nodes**: `CallExpression` (Deno.test), `ArrowFunctionExpression`,
`TryStatement` (when `:teardown` present)

**Rationale**: The keyword-clause syntax mirrors `func`'s `:args`/`:body`
pattern (DD-16), maintaining internal consistency. When `:teardown` is
present, the macro wraps the body in `try { ... } finally { teardown }`.
When only `:setup` is used, the setup expressions are prepended to the
body with no wrapping.

### 3. Async test form: `test-async`

**Decision**: `test-async` is the explicit async test form. The `test`
macro also auto-detects `await` in the body and emits an `async`
function when found.

**Syntax — explicit**:

```lisp
(test-async "fetches data"
  (bind result (await (fetch-data)))
  (is-equal (get result :status) :ok))
```

```javascript
Deno.test("fetches data", async () => {
  const result = await fetchData();
  assertEquals(result.status, "ok");
});
```

**Syntax — auto-detected**:

```lisp
;; `test` detects `await` and emits async automatically
(test "also fetches data"
  (bind result (await (fetch-data)))
  (is-equal (get result :status) :ok))
```

```javascript
// Same output — `test` macro walked the body, found `await`
Deno.test("also fetches data", async () => {
  const result = await fetchData();
  assertEquals(result.status, "ok");
});
```

**ESTree nodes**: `CallExpression`, `ArrowFunctionExpression` with
`async: true`, `AwaitExpression`

**Rationale**: Auto-detection is ergonomic for the common case.
`test-async` exists for cases where the body delegates to an async
helper without a lexically visible `await`, or where the author wants
to be explicit. The auto-detection walks the body AST at compile time
looking for any `await` symbol in call position.

### 4. Suite form: `suite`

**Decision**: `suite` groups related tests with shared `:setup` and
`:teardown`. It compiles to a top-level `Deno.test()` with nested
`t.step()` calls for each child `test`.

**Syntax**:

```lisp
(suite "math operations"
  :setup    (bind fixtures (load-fixtures))
  :teardown (cleanup fixtures)

  (test "addition"
    (is-equal (+ 1 2) 3))

  (test "division by zero throws"
    (is-thrown (/ 1 0))))
```

```javascript
Deno.test("math operations", async (t) => {
  const fixtures = loadFixtures();
  try {
    await t.step("addition", () => {
      assertEquals(1 + 2, 3);
    });
    await t.step("division by zero throws", () => {
      assertThrows(() => 1 / 0);
    });
  } finally {
    cleanup(fixtures);
  }
});
```

**ESTree nodes**: `CallExpression` (Deno.test, t.step),
`ArrowFunctionExpression` with `async: true`, `AwaitExpression`,
`TryStatement`

**Rationale**: Deno's `t.step()` provides hierarchical test output,
shared setup/teardown context, and proper error isolation. The
`suite` always emits an `async` function because `t.step()` returns
a promise and must be awaited.

### 5. Step form: `step`

**Decision**: `step` defines a subtest within a `test` or `suite`.
It compiles to `await t.step()`.

**Syntax**:

```lisp
(test "user workflow"
  (step "create user"
    (bind user (await (create-user :name "Alice")))
    (is-equal (get user :name) "Alice"))
  (step "delete user"
    (await (delete-user 1))
    (is-equal (await (get-user 1)) null)))
```

```javascript
Deno.test("user workflow", async (t) => {
  await t.step("create user", async () => {
    const user = await createUser({ name: "Alice" });
    assertEquals(user.name, "Alice");
  });
  await t.step("delete user", async () => {
    await deleteUser(1);
    assertEquals(await getUser(1), null);
  });
});
```

**ESTree nodes**: `CallExpression` (t.step), `MemberExpression`,
`AwaitExpression`, `ArrowFunctionExpression`

**Rationale**: When `step` is present inside a `test`, the enclosing
`test` must receive the `t` parameter and be async. The `test` macro
detects the presence of `step` children and adjusts its output
accordingly. Each `step` independently auto-detects `await` for its
own async status.

### 6. Assertion forms

**Decision**: Named, explicit assertion forms. Each maps to a specific
`@std/assert` function. No "smart" dispatch — the form name determines
the assertion.

| lykn form | Compiles to | Purpose |
|-----------|------------|---------|
| `(is expr)` | `assert(expr)` | Truthiness |
| `(is-equal actual expected)` | `assertEquals(actual, expected)` | Deep equality |
| `(is-not-equal actual expected)` | `assertNotEquals(actual, expected)` | Deep inequality |
| `(is-strict-equal actual expected)` | `assertStrictEquals(actual, expected)` | Reference equality (`===`) |
| `(ok expr)` | `assertExists(expr)` | Not null/undefined |
| `(is-thrown body)` | `assertThrows(() => body)` | Expects throw |
| `(is-thrown body ErrorType)` | `assertThrows(() => body, ErrorType)` | Expects typed throw |
| `(is-thrown body ErrorType "msg")` | `assertThrows(() => body, ErrorType, "msg")` | Expects throw with message |
| `(is-thrown-async body)` | `assertRejects(async () => body)` | Expects async rejection |
| `(is-thrown-async body ErrorType)` | `assertRejects(async () => body, ErrorType)` | Expects typed rejection |
| `(matches str pattern)` | `assertMatch(str, pattern)` | Regex match |
| `(includes str substr)` | `assertStringIncludes(str, substr)` | String contains |
| `(has arr items)` | `assertArrayIncludes(arr, items)` | Array contains |
| `(obj-matches actual expected)` | `assertObjectMatch(actual, expected)` | Partial object match |

**Example — `is-thrown`**:

```lisp
(test "throws on invalid input"
  (is-thrown (validate nil) TypeError)
  (is-thrown (parse "{{") SyntaxError "unexpected token"))
```

```javascript
Deno.test("throws on invalid input", () => {
  assertThrows(() => validate(null), TypeError);
  assertThrows(() => parse("{{"), SyntaxError, "unexpected token");
});
```

**Example — `is-thrown-async`**:

```lisp
(test-async "rejects on network error"
  (is-thrown-async (await (fetch-data "bad-url")) NetworkError))
```

```javascript
Deno.test("rejects on network error", async () => {
  await assertRejects(async () => await fetchData("bad-url"), NetworkError);
});
```

**Rationale**: Explicit form names over smart dispatch. Each assertion
is a simple macro that emits the corresponding `@std/assert` call.
This is more predictable, more debuggable, and avoids compile-time
AST introspection complexity. The names follow lykn's "named English
words" convention: `is-equal` not `eq`, `is-thrown` not `throws`.

### 7. Error messages — good-enough v1

**Decision**: For v1, assertion macros pass no custom message argument.
Deno's `@std/assert` provides built-in diff output showing actual vs
expected values. Enhanced source-expression capture is deferred to a
future source-mapping initiative.

**Example failure output** (provided by Deno, not lykn):

```
error: AssertionError: Values are not equal:

    [Diff] Actual / Expected

-   4
+   3

  at assertEquals (jsr:@std/assert/equals)
  at file:///path/to/math_test.js:5:3
```

**Rationale**: Deno's built-in diff output is already high quality —
it shows actual vs expected with color-coded diffs. Custom source
capture (serializing the lykn AST into the message string) requires
compile-time AST-to-string serialization infrastructure that overlaps
with the broader source-mapping work. Shipping with Deno's defaults
is "good enough" for initial use; source-mapping will upgrade this
to show `.lykn` file lines and original s-expressions.

**Future upgrade path**: When source mapping lands, assertion macros
can optionally serialize the source expression at compile time and
pass it as the `msg` parameter to each `@std/assert` function:

```javascript
// Future: with source capture
assertEquals(1 + 2, 4, `Assertion failed: (is-equal (+ 1 2) 4)`);
```

### 8. Convenience macro: `test-compiles`

**Decision**: A `test-compiles` macro for testing compiler output.
This is the primary pattern for lykn's own test suite — verifying
that lykn source compiles to expected JS.

**Syntax**:

```lisp
(import "../../packages/lykn/mod.js" (compile))

(test-compiles "bind compiles correctly"
  "(bind x 1)" "const x = 1;")

(test-compiles "func basic"
  "(func add :args (:number a :number b) :body (+ a b))"
  "function add(a, b) {\n  return a + b;\n}")
```

```javascript
import { compile } from "../../packages/lykn/mod.js";

Deno.test("bind compiles correctly", () => {
  assertEquals(compile("(bind x 1)"), "const x = 1;");
});

Deno.test("func basic", () => {
  assertEquals(
    compile("(func add :args (:number a :number b) :body (+ a b))"),
    "function add(a, b) {\n  return a + b;\n}"
  );
});
```

**ESTree nodes**: `CallExpression`, `ImportDeclaration`

**Rationale**: The existing test suite has 14 surface test files and
30 kernel form test files, all following the `{ input, output }` fixture
pattern. `test-compiles` captures this pattern in a single macro,
eliminating boilerplate. The `compile` import is not part of the macro
— the test file imports it explicitly, keeping the testing module
decoupled from the compiler.

### 9. Module structure: `packages/testing/`

**Decision**: The testing module lives at `packages/testing/` and
has its own `deno.json` for package configuration. It exports macros
only — no runtime code ships.

**File layout**:

```
packages/testing/
├── deno.json          # Package config, version, exports
├── mod.lykn           # Re-exports all testing macros
├── test.lykn          # test, test-async, suite, step macros
├── assert.lykn        # Assertion macros (is, is-equal, etc.)
└── convenience.lykn   # test-compiles and other helper macros
```

**Rationale**: Separate package within the monorepo, consistent with
`packages/lykn/`. Macro modules are `.lykn` files compiled and
evaluated at compile time (DD-14). The module is published to jsr.io
as `@lykn/testing` for external consumers.

### 10. Test file conventions

**Decision**: lykn test files use the `_test.lykn` suffix. They are
compiled to `_test.js` and discovered by Deno's standard glob.

**Example file structure**:

```
my-project/
├── src/
│   ├── math.lykn
│   └── math_test.lykn    ;; Co-located test
├── test/
│   └── integration_test.lykn
└── project.json
```

**Workflow**:

```sh
# Compile test files
lykn compile src/math_test.lykn -o src/math_test.js

# Run with Deno
deno test src/

# Or future: lykn test (DD-31) handles compile + run
lykn test src/
```

**Rationale**: Co-location of tests with source follows Deno's
recommendation and Rust convention. The `_test.lykn` suffix matches
Deno's discovery glob after compilation to `_test.js`. The compile
step is explicit for now; DD-31 will integrate it into `lykn test`.

## Rejected Alternatives

### Smart `is` macro with pattern dispatch

**What**: A single `(is expr)` macro that inspects its argument: `(is (= a b))`
dispatches to `assertEquals`, `(is (> a b))` dispatches to `assert(a > b)`,
etc.

**Why rejected**: Clever but opaque. The user has to know the dispatch
rules to predict which assertion fires. Explicit form names (`is-equal`,
`is-strict-equal`) are more predictable, easier to document, and produce
clearer error messages. Each macro is also simpler to implement — no
compile-time AST walking for dispatch.

### uvu as a runtime dependency

**What**: Import uvu as a dev dependency and compile lykn test macros
to uvu API calls instead of Deno's built-in test runner.

**Why rejected**: Adds an external dependency when Deno already provides
everything. The lykn project has bought into Deno's test runner — all
1,372 existing tests use it. uvu's value was its API naming philosophy,
which we adopted; the runtime itself is unnecessary.

### `deftest` / `defsuite` naming

**What**: Use Lisp-convention `def*` prefix for test definitions.

**Why rejected**: lykn doesn't use `def*` convention anywhere. The
language uses `bind` (not `defvar`), `func` (not `defun`), `type`
(not `deftype`). `test` and `suite` are consistent with this.

### `describe`/`it` BDD-style API

**What**: Use Mocha/Jest-style `(describe "thing" (it "should work" ...))`.

**Why rejected**: BDD naming adds verbosity without value in s-expression
syntax. `(it "should add numbers" ...)` reads worse than
`(test "addition" ...)` in a Lisp context. The `describe`/`it` style
was designed for JS's statement-oriented syntax, not for expressions.

### Custom test runner in Rust

**What**: Build `lykn test` as a complete test runner in the Rust binary,
bypassing Deno's test infrastructure.

**Why rejected**: Massive scope increase. Would need to reimplement
discovery, parallel execution, reporters, coverage, sanitizers, watch
mode, filtering. All of this exists in Deno and works well. The Rust
binary's role in testing is limited to compilation (DD-31).

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Empty test body | Compiles to empty function (passes) | `(test "placeholder")` → `Deno.test("placeholder", () => {});` |
| Nested suites | Inner `suite` becomes nested `t.step` group | `(suite "outer" (suite "inner" (test "x" ...)))` |
| `step` outside `suite`/`test` | Compile error — `step` requires enclosing test context | N/A |
| `is-thrown` with multiple expressions | Only last expression is wrapped | `(is-thrown (setup) (bad-call))` → `assertThrows(() => { setup(); return badCall(); })` |
| `:setup` without `:body` | All expressions after `:setup` value are body | `(test "x" :setup (init) (is-equal a b))` |
| `:teardown` without `:setup` | Valid — teardown wraps body in try/finally | `(test "x" :teardown (cleanup) :body (is-equal a b))` |
| Async `:setup` | Auto-detected; enclosing test becomes async | `(test "x" :setup (bind db (await (connect))) ...)` |
| `test-compiles` with multiline output | String comparison is exact, including newlines | Use template literals or escaped newlines |
| Suite with no child tests | Compiles to empty `Deno.test` (passes) | Allowed but useless |
| `test` with both `step` children and direct assertions | Direct assertions run before steps | Both coexist in the same function body |

## Dependencies

- **Depends on**: DD-10 (quasiquote), DD-11 (macro definition), DD-13
  (expansion pipeline), DD-14 (macro modules), DD-15 (keywords),
  DD-16 (`func` keyword-clause pattern)
- **Affects**: DD-31 (test runner CLI), future source-mapping DD,
  future book chapter on testing

## Open Questions

- [ ] **Source mapping integration**: When source mapping lands, how
  do assertion macros incorporate source expressions into error
  messages? The mechanism (compile-time AST serialization passed as
  `msg` parameter) is sketched but the implementation depends on
  the source-mapping infrastructure design.

- [ ] **Fixture file support**: Should there be a `test-fixture` macro
  that loads JSON fixture files (the current `test/fixtures/surface/*.json`
  pattern)? Or is `test-compiles` sufficient for all fixture-based tests?

- [ ] **Test configuration passthrough**: Deno's `Deno.test()` accepts
  options like `{ permissions, sanitizeOps, sanitizeResources, only,
  ignore }`. Should `test` support these via additional keyword clauses
  (e.g., `:only true`, `:skip true`, `:permissions (read true)`)? Or
  defer until needed?

- [ ] **`@std/expect` compatibility**: Deno also provides `@std/expect`
  for Jest-style `expect(x).toBe(y)` chains. Should testing offer
  an alternative set of macros that emit `expect` chains? Probably not
  — the function-call assertion style is the natural Lisp fit — but
  worth noting.
