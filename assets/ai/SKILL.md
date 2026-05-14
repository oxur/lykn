---
name: lykn-language-guidelines
description: |
  lykn (Lisp Flavoured JavaScript) best practices, idioms, and anti-patterns.
  Use when: writing new lykn code, converting JS to lykn, reviewing lykn for
  issues, designing lykn module APIs, working with surface forms (bind, func,
  type, match, cell, threading macros), handling errors in lykn, writing tests
  for lykn code, understanding kernel vs surface syntax, using the lykn CLI
  (compile, check, fmt), configuring the lykn toolchain, or answering lykn
  design questions. lykn compiles to clean, readable JS with no runtime
  dependencies. All code uses lykn/surface syntax targeting Deno with
  ESM-only modules and Deno's built-in linter/formatter on compiled output.
---

# lykn Skill — Language Reference

lykn is a Lisp-flavoured JavaScript — s-expression syntax compiling to clean, readable JS with no runtime dependencies. lykn has two syntax layers: **surface syntax** (the recommended authoring language) and **kernel syntax** (the compilation target). All examples in this skill use surface syntax unless explicitly marked otherwise.

**Design principles**: thin skin over JS (no runtime, no invented semantics), JS-aligned naming over Lisp conventions, lisp-case→camelCase auto-conversion, functional by default (immutable bindings, controlled mutation), required type annotations on all boundaries, clean compiled output.

**Toolchain**: Deno (not Node.js), `deno lint` + `deno fmt` on compiled JS output, ESM-only modules, strictly no npm in development workflows.

**Strength indicators** used throughout:

| Indicator | Meaning | Action |
|-----------|---------|--------|
| **MUST** | Required / compiler-enforced | Always follow |
| **SHOULD** | Strong convention | Follow unless specific reason not to |
| **CONSIDER** | Context-dependent | Evaluate for situation |
| **AVOID** | Anti-pattern | Do not use |

---

## Before You Do Anything

These three rules are load-bearing — violating any of them produces broken builds, broken packages, or silent data loss.

> **1. Use the lykn CLI for everything. Never reach around it. (MUST)**
>
> Run `lykn <command>` for every project operation: build, test, lint,
> format, run, publish. **Never** invoke `npm`, `deno publish`,
> `deno add`, `cargo build` (for the project), or `node` directly. The
> lykn CLI wraps these. If the wrapper seems to be missing something,
> ask before reaching around it. Common bypass patterns and the correct
> lykn equivalents:
>
> | Tempted to run | Use instead |
> |----------------|-------------|
> | `npm publish` | `lykn publish --npm` |
> | `deno publish` | `lykn publish --jsr` |
> | `npm install <x>` | add to `project.json` `imports` — Deno auto-caches online; for offline use `deno cache <spec>` |
> | `cargo build` (in a lykn user project) | `lykn build --dist` |
> | `deno run main.lykn` | `lykn run main.lykn` |
> | `deno test` | `lykn test` |
> | `deno fmt main.lykn` | `lykn fmt main.lykn` |
> | `deno fmt dist/` or `deno lint dist/` | don't run these — compiler ensures output is formatted and lint-clean (Principle 3) |
>
> **2. Generated files are generated. Don't hand-write them. (MUST)**
>
> Never hand-write `package.json`, `jsr.json`, or any file inside
> `dist/`. These are produced by `lykn build --dist` from the source
> `deno.json`. Hand-written versions get overwritten on the next build
> and create silent divergence between what you committed and what
> ships. If you find yourself opening one of these in an editor, you
> are about to fight the build.
>
> **3. Source files at the package root. No `src/`. (MUST)**
>
> Place `.lykn` source files directly in the package directory:
> `packages/my-pkg/mod.lykn`, not `packages/my-pkg/src/mod.lykn`.
> `lykn build --dist` stages from the package root; files inside a
> top-level `src/` subdirectory are silently skipped. The dist will
> appear to build successfully but ship a package with no code. This
> is **not** stylistic — it is a load-bearing structural requirement.
>
> The user does not run `deno fmt` or `deno lint` against compiled
> output. The compiler is the only validator; bugs in compiled JS
> (malformed, ugly, or unidiomatic) are reported as compiler defects,
> not papered over post-hoc.

---

## Document Selection Guide

The inline content below is enough to write correct lykn code. Load the full guide files when you need deeper rationale, edge cases, or comprehensive examples.

Note: document paths are relative to the lykn project root.

| Task | Load These Guides |
|------|-------------------|
| **Any lykn code** | `docs/guides/09-anti-patterns.md` (always load first) |
| **New module from scratch** | `docs/guides/01-core-idioms.md`, `docs/guides/10-project-structure.md`, `docs/guides/02-api-design.md` |
| **Learning surface syntax** | `docs/guides/00-lykn-surface-forms.md`, `docs/guides/01-core-idioms.md` |
| **API design** | `docs/guides/02-api-design.md`, `docs/guides/06-functions-closures.md`, `docs/guides/05-type-discipline.md` |
| **Error handling** | `docs/guides/03-error-handling.md`, `docs/guides/07-async-concurrency.md` |
| **Refactoring JS to lykn** | `docs/guides/00-lykn-surface-forms.md`, `docs/guides/01-core-idioms.md`, `docs/guides/09-anti-patterns.md` |
| **Code review / quality audit** | `docs/guides/09-anti-patterns.md`, `docs/guides/01-core-idioms.md`, `docs/guides/08-performance.md` |
| **Writing tests (lykn DSL)** | `docs/guides/16-testing.md`, `docs/guides/03-error-handling.md` |
| **Writing tests (JS on compiled output)** | `docs/guides/12-deno/12-02-testing.md`, `docs/guides/03-error-handling.md` |
| **Type system & contracts** | `docs/guides/05-type-discipline.md`, `docs/guides/06-functions-closures.md` |
| **Values, mutation, immutability** | `docs/guides/04-values-references.md`, `docs/guides/01-core-idioms.md` |
| **Async & concurrency** | `docs/guides/07-async-concurrency.md`, `docs/guides/03-error-handling.md` |
| **Performance review** | `docs/guides/08-performance.md`, `docs/guides/07-async-concurrency.md` |
| **Documentation** | `docs/guides/11-documentation.md`, `docs/guides/05-type-discipline.md` |
| **Deno runtime** | `docs/guides/12-deno/12-01-runtime-basics.md` |
| **Lint/format (Deno built-in)** | `docs/guides/12-deno/12-01-runtime-basics.md` |
| **No-Node boundary** | `docs/guides/14-no-node-boundary.md`, `docs/guides/12-deno/12-01-runtime-basics.md` |
| **lykn CLI (compile, fmt, check)** | `docs/guides/15-lykn-cli.md` |
| **Publishing a package** | `docs/guides/15-lykn-cli.md` (ID-04d, ID-04e), `docs/guides/10-project-structure.md` (package layout for publishing) |
| **Template / i18n** | `docs/guides/17-template-and-i18n.md`, `docs/guides/01-core-idioms.md` (ID-05) |

---

## Workflows

### Writing New lykn Code

1. **Create project** (if starting fresh): `lykn new my-project`
2. **Load anti-patterns first**: Read `docs/guides/09-anti-patterns.md` — know what to avoid
2. **Load core idioms**: Read `docs/guides/01-core-idioms.md` for `bind`, naming, control flow
3. **Load topic-specific docs**: Based on what you're building (API design, async, etc.)
4. **Structure the module**: Named exports, `bind` for all values, `func` for named functions
5. **Write code**: Type annotations on all `func` params, contracts for validation, `match` for branching on tagged data, threading macros for transformation pipelines
6. **Verify**: `lykn check <file>` for syntax, `lykn compile <file>` to inspect output
7. **Self-review**: Check against anti-patterns table before finishing
8. **When ready to publish**: see the **Publishing a Package** workflow below.

### Converting JS to lykn

1. **Load surface forms reference**: `docs/guides/00-lykn-surface-forms.md`
2. **Map declarations**: `const` → `bind`, `let` → `bind` + `cell`, remove all `var`
3. **Map functions**: `function` → `func` (add type annotations, contracts), arrows → `fn`/`lambda`
4. **Map control flow**: `if/else` → `(if ...)`, ternary → `(? ...)`, `switch` on tagged data → `match`
5. **Map objects**: `{ key: val }` → `(obj :key val)`, `{ ...a, key: val }` → `(assoc a :key val)`
6. **Map member access**: `obj.prop` → `obj:prop`, `obj[expr]` → `(get obj expr)`
7. **Eliminate `this`**: Restructure as pure functions or use `cell` for state
8. **Verify**: `lykn check`, `lykn compile`, inspect JS output for correctness

### Code Review / Quality Audit

1. **Scan for anti-patterns**: kernel forms where surface forms exist, missing type annotations, unnecessary `cell`, overuse of `js:` interop
2. **Check error handling**: No empty catches, errors wrapped with context, rejections handled
3. **Check API surfaces**: All `func` params typed, contracts for validation, consistent return types
4. **Check mutation discipline**: `cell` only where genuinely needed, `!` suffix on all mutating ops
5. **Check compiled output**: `lykn compile <file>` — output should be clean, readable JS

### Publishing a Package

1. **Confirm `deno.json` has the required fields**: `name`,
   `version`, `exports`, and `lykn.kind`. Either a `license` field
   in `deno.json` *or* a `LICENSE` file at the package root is
   required for JSR; `lykn new` generates the LICENSE file by
   default, so adding a `license` field is optional.
2. **Run `lykn build --dist`**: stages compiled `.js`, generated
   `package.json`, generated `jsr.json`, and copied `README.md`/
   `LICENSE` into `dist/<pkg>/`. Do **not** hand-write any file in
   `dist/`.
3. **Dry-run first**: `lykn publish --jsr --dry-run` then
   `lykn publish --npm --dry-run`. Read the output — JSR in particular
   has strict requirements that surface here.
4. **Publish**: `lykn publish --jsr` then `lykn publish --npm`.
5. **Counter-cue (read this if you're tempted to bypass):** if you
   reach for `npm publish` or `deno publish` directly, stop. Those
   commands skip `lykn build --dist` staging and produce a broken
   package. The lykn CLI wraps both — there is no scenario in normal
   project work where calling them directly is correct.

---

## Bindings & Mutation

- **`bind` for all values** — immutable by default. No `const`/`let`/`var` in surface syntax. **MUST**
- **Type annotation on `bind`** (DD-24): `(bind :number result (compute))`. Non-literal initializers get runtime type checks (same checks as `func`/`fn`). Literal initializers are verified at compile time (no runtime check). Type-incompatible literals are compile errors. Stripped by `--strip-assertions`. **SHOULD**
- **`cell` for controlled mutation**: `(bind counter (cell 0))`. **MUST** use `cell` when mutation is needed — never reach for kernel `let`.
- **`swap!` to update**: `(swap! counter (fn (:number n) (+ n 1)))`. The `!` suffix signals mutation. **MUST**
- **`reset!` to replace**: `(reset! counter 0)`. **MUST**
- **`express` to read**: `(express counter)` reads the cell's current value. **MUST**
- **Keywords for string values**: `:name` compiles to `"name"`. Use keywords as object keys, enum-like values, and form labels. **MUST**
- **lisp-case for all identifiers**: `my-function` → compiles to `myFunction`. **MUST**

```lykn
;; Immutable binding — no type annotation needed for literals
(bind greeting "hello")
(bind max-retries 3)

;; Type annotation — runtime check on non-literal (DD-24)
(bind :number result (compute-something))

;; Mutation via cell
(bind counter (cell 0))
(swap! counter (fn (:number n) (+ n 1)))
(console:log (express counter))  ;; => 1
(reset! counter 0)
```

Compiles to:

```js
const greeting = "hello";
const maxRetries = 3;
const result = computeSomething();
if (typeof result !== "number" || Number.isNaN(result))
  throw new TypeError("bind: binding 'result' expected number, got " + typeof result);

const counter = {value: 0};
counter.value = ((n) => {
  if (typeof n !== "number" || Number.isNaN(n))
    throw new TypeError("anonymous: arg 'n' expected number, got " + typeof n);
  return n + 1;
})(counter.value);
console.log(counter.value);
counter.value = 0;
```

> **Note (DD-24):** `bind` type annotations are enforced at runtime for
> non-literal initializers. Literal initializers are verified at compile
> time (no runtime check emitted). Type-incompatible literals are compile
> errors. All `bind` type checks are stripped by `--strip-assertions`.

---

## Functions

- **`func` for named functions**: keyword-labeled clauses, required type annotations, optional contracts. **MUST**
- **`fn` for anonymous functions**: positional typed params, arrow function output. **SHOULD**
- **`lambda` for anonymous functions**: exact alias for `fn` — same output (arrow function). **CONSIDER**
- **All params require type annotations**: bare symbols in param lists are compile errors. `:any` is the explicit opt-out. **MUST**
- **Contracts with `:pre` / `:post`**: runtime validation, stripped by `--strip-assertions`. **SHOULD**
- **`:returns` for return type checking**: runtime check on return value. **SHOULD**
- **Multi-clause `func`**: arity + type dispatch. Overlap is a compile error. **CONSIDER**
- **Zero-arg shorthand**: `(func name (body-expr))` for simple functions. **SHOULD**
- **Destructured params** (DD-25): `(object ...)` or `(array ...)` patterns in `:args` with per-field type annotations. Idiomatic for named/keyword parameters. **SHOULD** for 3+ related params.
- **Defaults in destructured params** (DD-25.1): `(default :type name value)` inside destructuring patterns. **SHOULD**
- **Nested destructuring** (DD-25.1): `(alias :type name (object/array ...))` for nesting in object params. Array nesting is positional. **CONSIDER**

```lykn
;; Full func with contracts
(func divide
  :args (:number a :number b)
  :returns :number
  :pre (!= b 0)
  :body (/ a b))

;; Zero-arg shorthand
(func now (Date:now))

;; Anonymous function
(bind doubled (fn (:number x) (* x 2)))

;; Destructured params — named/keyword argument pattern
(func connect
  :args ((object :string host :number port (default :boolean ssl true)))
  :body (open-connection host port ssl))
;; Called as: (connect (obj :host "localhost" :port 5432))

;; Multi-clause dispatch
(func describe
  :args (:string s)
  :body (template "string: " s))
(func describe
  :args (:number n)
  :body (template "number: " n))
```

**Built-in type keywords**: `:number` (excludes NaN), `:string`, `:boolean`, `:function`, `:object`, `:array`, `:symbol`, `:bigint`, `:any`, `:void`, `:promise`.

---

## Generators

- **`genfunc` for named generators**: keyword-labeled clauses like `func`, plus `:yields :type` for per-yield runtime checks. **SHOULD**
- **`genfn` for anonymous generators**: like `fn` but produces `function*`. Optional `:yields :type`. **SHOULD**
- **`yield` / `yield*`**: kernel forms, used inside generator bodies. `yield*` delegates to another iterable. **MUST** use inside `genfunc`/`genfn`/`function*` only.
- **`for-await-of`**: kernel form for async iteration. **MUST** use inside `async` context.
- **Async generators**: `(async (genfunc ...))` composes naturally. **CONSIDER**

```lykn
;; Typed generator with :yields runtime checks
(genfunc range
  :args (:number start :number end)
  :yields :number
  :body
  (for (let i start) (< i end) (+= i 1)
    (yield i)))

;; Anonymous generator
(bind gen (genfn () (yield 1) (yield 2)))

;; Async generator
(async (genfunc fetch-pages
  :args (:string url)
  :body
  (let page 1)
  (while true
    (bind response (await (fetch (template url "?page=" page))))
    (bind data (await (response:json)))
    (if (= data:results:length 0) (return))
    (yield data:results)
    (+= page 1))))
```

---

## Types & Pattern Matching

- **`type` for algebraic data types**: tagged objects with named fields, constructor validation. **SHOULD**
- **`match` for exhaustive pattern matching**: compiler verifies all constructors covered. **MUST** include all variants or a wildcard `_`.
- **`match` is an expression**: returns a value. **SHOULD** use as expression rather than statement when possible.
- **`if-let` / `when-let`**: conditional binding — test a pattern and bind in one step. **SHOULD**
- **Constructor patterns**: `(Some v)` matches `{ tag: "Some", value: v }`. **MUST** use constructor syntax in patterns, not raw object checks.

```lykn
;; Define a type
(type Option
  (Some :any value)
  None)

;; Construct values
(bind found (Some 42))
(bind missing None)

;; Exhaustive pattern matching
(bind result
  (match found
    ((Some v) (+ v 1))
    (None 0)))

;; Conditional binding
(if-let ((Some user) (find-user id))
  (greet user)
  "not found")

;; when-let (no else branch)
(when-let ((Some user) (find-user id))
  (console:log user:name))
```

---

## Objects & Immutable Updates

- **`obj` for object construction**: keyword-value pairs. **MUST**
- **`assoc` for immutable field update**: spread-based, original unchanged. **MUST**
- **`dissoc` for immutable field removal**: original unchanged. **SHOULD**
- **`conj` for immutable collection append**: works with arrays. **SHOULD**
- **Keywords as keys**: `:name` compiles to `"name"` — use keywords, not quoted strings. **MUST**

```lykn
;; Object construction
(bind user (obj :name "Alice" :age 30))

;; Immutable update — original unchanged
(bind updated (assoc user :age 31))

;; Remove a field
(bind safe (dissoc user :password))

;; Append to array
(bind items #a(1 2 3))
(bind more (conj items 4))
```

Compiles to:

```js
const user = {name: "Alice", age: 30};
const updated = {...user, age: 31};
```

---

## Threading Macros

- **`->` (thread-first)**: pipes a value through a series of transformations, inserting as first argument. **SHOULD**
- **`->>` (thread-last)**: inserts as last argument. **SHOULD**
- **`some->` / `some->>` (nil-safe threading)**: short-circuits on `null`/`undefined`. Replaces `?.` chaining. **SHOULD**
- **Method calls in threading**: `(:method-name)` calls a method on the threaded value. **MUST** use this syntax for method calls in threading position.

```lykn
;; Thread-first: x is inserted as first arg at each step
(-> 5 (+ 3) (* 2))   ;; => (5 + 3) * 2 = 16

;; Thread-last: items inserted as last arg
(->> items (filter even?) (map double))

;; Method call in threading position
(-> user (get :name) (:to-upper-case))

;; Nil-safe threading (replaces ?. chains)
(some-> user (get :address) (get :street))
```

---

## Colon Syntax & Member Access

- **Colon for member access**: `console:log` → `console.log`. **MUST**
- **Chained access**: `a:b:c` → `a.b.c`. **MUST**
- **`get` for computed access**: `(get obj expr)` → `obj[expr]`. **MUST**
- **Leading colon = keyword**: `:name` → `"name"`. Reader-level type. **MUST**
- **`js:` namespace for JS interop**: escape hatch for JS features not in surface syntax. Greppable, auditable. **SHOULD** use sparingly.

```lykn
;; Member access
(console:log "hello")           ;; => console.log("hello")
(bind len my-array:length)      ;; => const len = myArray.length

;; Computed access
(bind item (get my-array 0))    ;; => const item = myArray[0]

;; JS interop escape hatch
(js:eq value null)              ;; => value == null (loose equality)
```

---

## Modules

- **ESM only**: `import`/`export` forms. No `require()`, no CommonJS. **MUST**
- **Named exports**: `(export (func ...))` or `(export (bind ...))`. **MUST**
- **Import with binding list**: `(import "./module.js" (name1 name2))`. **MUST** — path first, then bindings.
- **`alias` for rename**: `(import "./module.js" ((alias original renamed)))`. **SHOULD**
- **File extensions required** on local imports. **MUST**
- **No `export *` or namespace imports**: banned by design. **MUST**

```lykn
;; Import — path first, then bindings
(import "@std/path" (join))
(import "./auth/mod.js" (login logout))

;; Export
(export (func greet
  :args (:string name)
  :returns :string
  :body (template "Hello, " name)))

;; Export a binding
(export (bind VERSION "0.4.0"))
```

---

## Error Handling

- **`throw` only `Error` instances**: `(throw (new Error "message"))`. **MUST**
- **`try`/`catch`/`finally`**: kernel forms used directly in surface code. **MUST**
- **`Error.cause` for chaining**: `(throw (new Error "msg" (obj :cause err)))`. **SHOULD**
- **Never empty catches**: handle, rethrow, or both. **MUST**
- **`match` for error dispatch**: pattern match on error type for typed error handling. **CONSIDER**

```lykn
;; Error handling
(try
  (bind raw (await (Deno:read-text-file path)))
  (JSON:parse raw)
  (catch err
    (throw (new Error
      (template "Failed to load config from " path)
      (obj :cause err)))))

;; Guard clause with contract
(func fetch-user
  :args (:string id)
  :pre ((not (js:eq id null)) "id is required")
  :body (await (fetch (template "/api/users/" id))))
```

---

## Async & Concurrency

- **`(async ...)` wrapper**: wraps any form to make it async. **MUST**
- **`(await expr)` unary**: await a promise. **MUST**
- **Top-level `await`**: works directly in ESM modules. **SHOULD**
- **`Promise:all` for parallel ops**: never sequential `await` on independent calls. **SHOULD**
- **`AbortController` for cancellation**: every `fetch` in production should accept a signal. **SHOULD**

```lykn
;; Async function
(export (async (func fetch-data
  :args (:string url)
  :returns :promise
  :body (bind response (await (fetch url)))
        (if (not response:ok)
          (throw (new Error (template "HTTP " response:status)))
          (await (response:json))))))

;; Parallel operations
(bind #a(users posts)
  (await (Promise:all #a((fetch-users) (fetch-posts)))))
```

---

## Classes

Classes are available but de-emphasized in surface lykn. Prefer `type` + `func` for new designs. Use `class` for JS interop, framework requirements, or when `instanceof` checking is needed.

- **Surface forms expand in class bodies** (DD-27): `bind`, `=` (equality), `set!`, threading macros, `obj`, and all other surface forms work inside methods and constructors. **MUST** understand this.
- **`assign` for this-property assignment** (class body only): `(assign this:x value)` → `this.x = value`. Use in constructors. Compile error outside class bodies — use `set!` for mutation elsewhere. **MUST** (not `=`, which is equality).
- **Private fields via `-` prefix**: `-count` compiles to `#_count`. **MUST** for encapsulation.
- **`this` available inside class bodies**: surface forms eliminate `this` elsewhere, but inside `class` bodies it's available for property access.

```lykn
;; Class with surface forms in methods (DD-27)
(class Dog (Animal)
  (constructor (name breed)
    (super name)
    (assign this:breed breed))
  (speak ()
    (bind greeting (template this:name " says woof"))
    (if (= this:breed "poodle")
      (return (template greeting " (fancy)"))
      (return greeting))))

;; Prefer type + func for new designs
(type Counter
  (Counter :number count))

(func increment
  :args ((Counter c))
  :body (Counter (+ c:count 1)))
```

---

## Data Literals & Sugar

| lykn | JS | Notes |
|------|-----|-------|
| `#a(1 2 3)` | `[1, 2, 3]` | Array literal |
| `#o((name "x") (age 42))` | `{name: "x", age: 42}` | Object literal (kernel-style) |
| `#16rff` | `255` | Hex radix literal |
| `#2r11110000` | `240` | Binary radix literal |
| `#; expr` | *(discarded)* | Expression comment (JS compiler only; not in Rust CLI) |
| `#\| ... \|#` | *(discarded)* | Nestable block comment (JS compiler only; not in Rust CLI) |
| `(template "hi " name)` | `` `hi ${name}` `` | Template literal |
| `(tag html (template ...))` | `` html`...` `` | Tagged template |
| `(regex "^hello" "gi")` | `/^hello/gi` | Regular expression |

---

## Anti-Patterns to AVOID

These are the most common mistakes in lykn code, especially when converting from JS or in AI-generated output.

| Anti-Pattern | Fix |
|---|---|
| Using kernel forms when surface forms exist (`const` instead of `bind`, `function` instead of `func`) | Always use surface forms for new code |
| Missing type annotations on `func`/`fn` params | All params require type keywords; `:any` is the opt-out |
| Forgetting `express` when reading a cell | `(express counter)`, not just `counter` (which is the cell object) |
| Using `cell` when immutability works | Prefer `bind` + `assoc`/`dissoc`/`conj` for data transforms |
| Overusing `js:` interop | `js:` is an escape hatch, not a primary tool; keep it greppable and rare |
| camelCase in lykn source | Use lisp-case: `my-function`, not `myFunction`. Compiler auto-converts. |
| Quoted strings as object keys | Use keywords: `(obj :name "x")`, not `(obj "name" "x")` |
| Using `===`/`==` kernel operators directly | Use `(= a b)` for equality — it compiles to `===`. Use `(js:eq x null)` for the `== null` idiom. |
| Using `&&`/`\|\|`/`!` kernel operators directly | Use `(and a b)`, `(or a b)`, `(not x)` — the surface operators. They compile to `&&`/`\|\|`/`!`. |
| Sequential `await` on independent ops | `(await (Promise:all #a(...)))` for parallel |
| Empty `catch` blocks | Handle, rethrow, or both |
| `import-macros` without explicit binding list | Always specify which macros you're importing |
| Bare symbols in destructuring without type context | Add type annotations where the compiler expects them |
| Using `object` (kernel) instead of `obj` (surface) for construction | `obj` uses keyword syntax; `object` is the kernel form |
| Assuming `bind` type annotations on literals generate runtime checks | Literal annotations are verified at compile time — no runtime check emitted. Non-literal annotations DO generate runtime checks (DD-24). |

**`?` vs `if` (DD-50 Rule 5):** prefer `?` for expression position; prefer `if` for statement position. The compiler treats them as functionally equivalent in expression position; the explicit form communicates intent at source level. For LLM-generated code, treat this as a hard rule (always use `?` in expression position, always use `if` in statement position).

---

## Naming Conventions

| Element | Convention | Example |
|---------|-----------|---------|
| Bindings, functions | lisp-case | `get-user-name`, `max-retries` |
| Type constructors | PascalCase | `Some`, `None`, `HttpError` |
| Type names | PascalCase | `Option`, `Result` |
| Keywords | lisp-case with `:` prefix | `:first-name` → `"firstName"` |
| Mutation ops | `!` suffix | `swap!`, `reset!` |
| Predicate functions | `?` suffix | `even?`, `valid?` |
| Private class fields | `-` prefix | `-count` → `#_count` |
| Module files | kebab-case `.lykn` | `http-client.lykn` |

**AVOID** weasel names: `Manager`, `Service`, `Handler`, `Utils`, `Data`, `process`, `handle`.

### Identifier Mapping Details (DD-49)

**Predicate prefix detection:** when a `?`-suffix identifier already starts with `is-`, `has-`, `can-`, `should-`, `will-`, `does-`, `was-`, or `had-`, the compiler strips the `?` and emits the camelCase form without prepending `is-`. So `has-items?` → `hasItems` (not `isHasItems`). Otherwise, the compiler prepends `is-` then camelCases: `valid?` → `isValid`.

**Embedded punctuation abbreviations:**

| Character | Abbreviation | Example |
|---|---|---|
| `?` (mid/leading) | `QMARK` | `func?-thing` → `funcQMARKThing` |
| `!` (mid/leading) | `BANG` | `set!-state` → `setBANGState` |
| `*` | `STAR` | `*globals*` → `STARGlobalsSTAR` |
| `+` | `PLUS` | `+constant+` → `PLUSConstantPLUS` |
| `=` (mid/leading) | `EQ` | (rare) |
| `<` / `>` | `LT` / `GT` | (rare in identifiers) |
| `&` | `AMP` | `&rest` → `AMPRest` |
| `%` | `PCT` | (rare) |
| `/` | `SLASH` | `path/to` → `pathSLASHTo` |
| `->` | `To` | `string->json` → `stringToJson` |
| `<-` | `From` | `json<-string` → `jsonFromString` |

Note: `$` is NOT in the abbreviation table — it's a valid JS identifier character (per ECMAScript spec) and passes through unchanged.

**Macro-name overrides:** the threading macros `->` and `->>` (the language-primitive forms, not embedded in longer identifiers) compile to `threadFirst` and `threadLast` respectively. These are language-design names rather than mechanical-rule outputs.

See DD-49 (`docs/design/05-active/0049-identifier-mapping-lykn-js.md`) for full rationale.

---

## lykn CLI

The `lykn` binary is the primary tool. Single binary wraps compilation, Deno test/lint/run, and publishing.

```sh
# Create a new project
lykn new my-app

# Compile to JS
lykn compile main.lykn -o main.js

# Strip type checks (production)
lykn compile main.lykn --strip-assertions -o main.js

# Run a .lykn file directly
lykn run packages/myapp/main.lykn

# Run tests
lykn test

# Lint compiled JS
lykn lint

# Syntax check
lykn check main.lykn

# Format
lykn fmt -w main.lykn

# Publish to JSR / npm
lykn publish --jsr
lykn publish --npm
lykn publish --npm --dry-run
```

### Build from source

```sh
mkdir -p ./bin
cargo build --release && cp ./target/release/lykn ./bin
```

### Project config

The CLI auto-discovers `project.json` by walking up from the current directory. This is the workspace root config that maps imports and defines tasks.

---

## Deno Runtime & Testing

lykn targets Deno exclusively. The compiled JS output runs in Deno with ESM-only modules.

- **Deno APIs**: `Deno.readTextFile`, `Deno.serve`, `Deno.env.get()`, etc. **MUST**
- **Web Platform APIs**: `fetch`, `Request`, `Response`, `URL`, `AbortController`, `structuredClone`. **SHOULD**
- **Permissions**: `--allow-net`, `--allow-read`, etc. Never `--allow-all` in production. **MUST**
- **Testing**: `Deno.test()` + `@std/assert`. Test files named `*_test.js` (on compiled output) or `*_test.lykn`. The lykn testing DSL (`import-macros "testing"`) provides `test`, `suite`, `step`, and assertion macros that compile to `Deno.test()` + `@std/assert`. See `docs/guides/16-testing.md`. **MUST**

---

## Linting & Formatting (Deno Built-in)

Deno's built-in `deno lint` and `deno fmt` operate on compiled JS output. The lykn formatter (`lykn fmt`) handles `.lykn` source formatting.

- **Pipeline**: `.lykn` → `lykn compile` → `.js` → `deno lint` + `deno fmt` → results. **MUST**
- **No external tools needed**: `deno lint` and `deno fmt` are built into the Deno binary. **MUST**
- **Configure via `deno.json`**: lint rule exclusions and format options go in `deno.json`. **SHOULD**

---

## No-Node Boundary

lykn targets Deno exclusively. The same no-Node boundary from the JS guides applies:

| Node.js Pattern | Replacement |
|----------------|-------------|
| `require()` | ESM `import` (lykn: `(import ...)`) |
| `module.exports` | ESM `export` (lykn: `(export ...)`) |
| `package.json` | `project.json` (workspace root) + `deno.json` (per package) |
| `node_modules` | Global cache via `jsr:`/`npm:` specifiers |
| `process.env` | `Deno:env:get` in lykn |
| `__dirname` | `import:meta:dirname` in lykn |
| `Buffer` | `Uint8Array` + `TextEncoder`/`TextDecoder` |
| Jest / Mocha | `Deno.test()` + `@std/assert` |
| ESLint + Prettier | `deno lint` + `deno fmt` on compiled output, `lykn fmt` on source |

---

## Kernel vs Surface: When to Use Kernel Forms

Surface syntax covers the vast majority of use cases. Drop to kernel forms only when:

1. **JS interop requires it**: a JS construct with no surface equivalent
2. **Class bodies**: `class` is a kernel form; method bodies use kernel syntax
3. **Low-level control flow**: `switch`, `for`, `do-while`, `label` — kernel forms
4. **Debugging**: `(debugger)` is a kernel form
5. **The `js:` escape hatch isn't enough**: very rare

When mixing, surface forms can contain kernel forms freely. The compiler handles the boundary.

---

## Worked Examples

### Task: "Write a module that fetches and caches API responses"

1. **Load**: `docs/guides/09-anti-patterns.md`, `docs/guides/01-core-idioms.md`, `docs/guides/07-async-concurrency.md`
2. **Apply**:
   - `(bind cache (new Map))` for the cache
   - `(export (async (func fetch-cached ...)))` with `:string url` param, typed
   - Contract: `:pre ((not (js:eq url null)) "url is required")`
   - `(if-let ((Some cached) (find-in-cache url)) cached (do-fetch url))` pattern
   - `(await ...)` for async, `AbortSignal` via options parameter
   - `(try ... (catch err (throw (new Error msg (obj :cause err)))))` for error chaining

### Task: "Define a result type with success/failure handling"

1. **Load**: `docs/guides/05-type-discipline.md`, `docs/guides/06-functions-closures.md`
2. **Apply**:
   - `(type Result (Ok :any value) (Err :string message))` — algebraic data type
   - `(func map-result :args ((Result r) :function f) :body (match r ((Ok v) (Ok (f v))) ((Err e) (Err e))))` — exhaustive match
   - `(func unwrap :args ((Result r)) :body (match r ((Ok v) v) ((Err e) (throw (new Error e)))))` — unwrap or throw

### Task: "Convert a JS options-object API to lykn"

1. **Load**: `docs/guides/00-lykn-surface-forms.md`, `docs/guides/02-api-design.md`
2. **Apply**:
   - JS `function createServer({ port = 8080, host = "localhost" } = {})` → lykn `(func create-server :args (:object opts) :body ...)` with `(bind :number port (?? opts:port 8080))`
   - Or use kernel destructuring for the options object
   - Named exports: `(export (func create-server ...))`
   - Type annotations on all parameters

### Task: "Refactor imperative JS with mutable state to lykn"

1. **Load**: `docs/guides/04-values-references.md`, `docs/guides/01-core-idioms.md`
2. **Apply**:
   - Replace `let count = 0; count++` → `(bind counter (cell 0)) (swap! counter (fn (:number n) (+ n 1)))`
   - Replace `arr.push(x)` → `(bind new-arr (conj arr x))` (immutable append)
   - Replace `obj.key = val` → `(bind updated (assoc obj :key val))` (immutable update)
   - Only use `cell` when the value genuinely needs to change over time
   - Thread pipelines replace intermediate mutable variables: `(-> data (transform-a) (transform-b) (transform-c))`

### Task: "Set up linting on lykn compiled output"

1. **Load**: `docs/guides/12-deno/12-01-runtime-basics.md`
2. **Apply**:
   - `deno lint dist/` on compiled JS output — no extra tools needed
   - `deno fmt dist/` for consistent formatting
   - Many JS anti-patterns (like `var`, `==`) are structurally impossible in lykn's output
   - `deno lint` catches issues in hand-written JS helpers or edge cases in compiled output
