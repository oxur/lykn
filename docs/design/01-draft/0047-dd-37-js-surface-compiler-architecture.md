---
number: 47
title: "DD-37: JS Surface Compiler Architecture"
author: "surface macros"
component: All
tags: [change-me]
created: 2026-04-18
updated: 2026-04-18
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-37: JS Surface Compiler Architecture

**Status**: Draft
**Date**: 2026-04-18
**Session**: Post-0.5.0 QA analysis — kernel/surface boundary
**Depends on**: DD-13 (macro expansion pipeline), DD-15 (language
architecture), DD-20 (Rust surface compiler architecture), DD-36
(kernel/surface compiler split — this is its JS-side Phase 0
prerequisite)
**Targets**: 0.6.0 (same release as DD-36)

## Summary

The JS compiler is today a three-file pipeline — `reader.js →
expander.js → compiler.js` — that conflates kernel compilation,
surface expansion, user-macro expansion, module loading, and codegen
into a shared dispatch table. This DD proposes a **six-module
decomposition** that mirrors DD-20's Rust architecture while
acknowledging the JS compiler's distinct constraints: it must run in
the browser, cannot shell out to Deno, trades static analysis for
deployability, and is a dependency of `@lykn/testing` and the test
runner.

The target shape: `reader → classifier → (macro expander | static
surface transforms) → kernel emitter → kernel compiler → astring`,
with the surface AST as the lingua franca between modules. Built-in
surface forms become static transforms (not macros); user-defined
macros retain the DD-13 pipeline via `new Function()`. The `_kernel`
marker and `kernelArray()` helper — the two visible scars of the
current design — are retired. The kernel path (DD-36) becomes a
direct bypass: `reader → kernel compiler → astring`, skipping
classification and expansion entirely.

**Bundle-size caveat.** The JS compiler ships as `lykn-browser.js`
for in-browser compilation, which makes bundle size a user-visible
cost. The full architecture is estimated to add +8–20KB gzipped
(squarely in the project's "investigate" band). The migration
sequence is designed to fail fast on size regressions, and a
reduced-scope variant ("Alt C" — classifier-only, surface forms
stay as macros) is explicitly preserved as a pre-agreed fallback.
See "Bundle size considerations" for the budget, mitigations, and
decision rule.

This is a workbench draft, sibling to DD-36. It is a **target
architecture** — what the JS compiler should become — not a record
of what exists.

## Context: what the JS compiler is today

The lang package is 5,551 lines of JS across five files:

| File | Lines | Role |
|------|------:|------|
| `mod.js` | 19 | Public entry: `lykn(source) = compile(expand(read(source)))` |
| `reader.js` | 320 | S-expression reader — six node types, includes cons cells |
| `expander.js` | 1,350 | Macro system: DD-13 three-pass pipeline, surface macro registration, quasiquote, `new Function()` evaluation |
| `surface.js` | 2,262 | All surface forms as macros, registered into the expander's environment |
| `compiler.js` | 1,600 | Kernel compiler: `macros` dispatch table → ESTree via `compileExpr` |

The public entry `lykn(source)` composes `read → expand → compile`.
Compiled output flows through `astring` (npm dependency mapped
through Deno's import map) to produce JS text.

### What works

- **Reader is clean.** 320 lines with JSDoc-typed node variants. It
  does not know about surface or kernel semantics. DD-37 does not
  touch it.
- **Macro infrastructure is mature.** DD-13 (three-pass pipeline),
  DD-14 (macro modules), DD-11 (hygiene and gensym), DD-10
  (quasiquote) are all implemented in `expander.js`. User macros
  work and cross-package `import-macros` works (DD-34).
- **Compilation is deterministic.** Same input → same ESTree →
  same JS output, with astring handling formatting. DD-22's
  surface-equality change was implementable without new
  infrastructure.

### What doesn't scale

The `expander.js:731` comment describes it in one line:

```js
// Skip re-expansion of forms marked as kernel output by surface macros
if (head.type === 'atom' && macroEnv.has(head.value) && !form._kernel) {
```

Surface forms are implemented as macros in the same `macroEnv` as
user macros. Their output contains kernel forms that would be
re-expanded on the fixed-point walk if not marked. The marker
(`_kernel = true`) and its companion `kernelArray()` helper in
`surface.js:24` exist purely to prevent this re-entry. Grep finds
five sites in `surface.js` (lines 1026, 1133, 1148, 1167, 1181) that
use `kernelArray()`.

That is the central architectural debt: the JS compiler cannot
distinguish "surface macro output that happens to look like a surface
form name" from "actual surface form input." The Rust compiler
sidesteps this via the classifier + typed AST; the JS compiler
sidesteps it with a boolean tag on a tree node.

A second symptom: `compiler.js` has grown to 1,600 lines with a
single `macros` dispatch table conflating kernel primitives (`const`,
`=>`, `function`), syntactic sugar (`.`, `get`), and kernel
operators. There is no structural boundary between "things that
compile directly to one ESTree node" and "things that are small
transforms of other forms." Adding a new kernel primitive requires
editing the same table as adding a new operator shortcut.

A third symptom: `surface.js` is 2,262 lines of macros that share
state with user macros (the same `macroEnv`) and have no type
discipline. Every surface form independently re-validates its
argument shapes. Any mistake in a surface macro can silently produce
invalid kernel code that `compiler.js` will happily emit.

### Why this DD exists now

DD-36 proposes splitting kernel and surface compilation. The Rust
side is ~80% ready via DD-20; the JS side is the long pole. Before
we can enforce extension-based dispatch in the JS compiler, the JS
compiler needs enough internal structure to support it. That
structure is the subject of this DD.

## Decisions

### Six-module decomposition

**Decision**: The lang package is reorganised into six modules with
defined responsibilities and interfaces. Like DD-20, each module is
designed for reuse across consumers (the compiler, the browser
runtime, the test runner, future formatter and linter).

```
┌────────────────────────────────────────────┐
│           @lykn/lang (JS toolchain)        │
│                                            │
│  ┌────────┐   ┌─────────────┐              │
│  │ Reader │   │ Surface AST │              │
│  └────┬───┘   └──────┬──────┘              │
│       │              │                     │
│  ┌────▼──────────────▼───┐                 │
│  │     Classifier        │                 │
│  └────┬──────────────────┘                 │
│       │                                    │
│  ┌────▼───────────┐                        │
│  │ Macro Expander │                        │
│  │ (user macros   │                        │
│  │  only)         │                        │
│  └────┬───────────┘                        │
│       │                                    │
│  ┌────▼────────────────────┐               │
│  │   Kernel Emitter        │               │
│  │   (surface → kernel)    │               │
│  └────┬────────────────────┘               │
│       │                                    │
│  ┌────▼──────────────┐   ┌──────────────┐  │
│  │ Kernel Compiler   │   │ Diagnostics  │  │
│  │ (kernel → ESTree) │   │              │  │
│  └────┬──────────────┘   └──────────────┘  │
└───────┼────────────────────────────────────┘
        │
        ▼
      astring
        │
        ▼
   JavaScript output
```

**File layout** (initial proposal — see "Bundle size considerations"
for why the per-form split is an open design choice):

```
packages/lang/
  mod.js            # public API — re-exports, orchestration
  reader.js         # stays put, minimal changes
  surface-ast.js    # NEW — tagged node constructors + predicates
  classifier.js     # NEW — SExpr tree → surface AST
  expander.js       # TRIMMED — user macros + DD-13 pipeline only
  emitter.js        # NEW — surface AST → kernel SExpr (+ transforms)
  compiler.js       # TRIMMED — kernel SExpr → ESTree only
  diagnostics.js    # NEW — structured error objects
```

Per-form transforms live in `emitter.js` as a single flat dispatch
table by default. A per-file `surface/` directory is a **variant
layout** under consideration — it improves locality per form but
adds module boilerplate that costs bundle bytes. The choice is
deferred to implementation time and decided by measurement (see
Bundle size considerations below).

**Consumers**:

| Consumer | Modules used |
|----------|-------------|
| `lykn()` (convenience) | All six |
| Kernel-only compilation (`.lyk`) | Reader, kernel compiler, diagnostics |
| Browser runtime (`@lykn/browser`) | All six |
| Test runner (via `@lykn/testing`) | All six (surface used by test DSL) |
| Formatter (future) | Reader, diagnostics |
| Linter (future) | Reader, classifier, diagnostics |

**Rationale**: The decomposition matches DD-20 module-for-module
where possible, so the two implementations can be discussed in a
shared vocabulary. Three deviations from DD-20, covered in separate
decisions below: (a) no analysis module, (b) macro expander is
smaller and does not shell out to a subprocess, (c) kernel compiler
is a distinct module because the JS side emits ESTree + astring
rather than stopping at kernel SExpr.

### Surface AST as tagged object literals

**Decision**: The surface AST is represented as plain JS objects
with a `type` discriminator, following the existing reader
convention. JSDoc typedefs document each variant; `surface-ast.js`
exports constructor functions and type predicates.

```js
// surface-ast.js

/**
 * @typedef {Object} FuncNode
 * @property {'surface:func'} type
 * @property {AtomNode} name
 * @property {FuncClause[]} clauses
 * @property {Span} span
 */

export function mkFunc(name, clauses, span) {
  return { type: 'surface:func', name, clauses, span };
}

export function isFunc(node) {
  return node && node.type === 'surface:func';
}
```

Every surface AST node type gets a `surface:<form>` tag. This gives
us a clear namespace distinction: reader nodes have types like
`'atom'`, `'list'`, `'keyword'`; surface AST nodes have types like
`'surface:func'`, `'surface:match'`, `'surface:bind'`; kernel nodes
(when passed through) retain their reader types.

The variants mirror DD-20's enum: `Func`, `Bind`, `Match`, `Type`,
`Obj`, `Cell`, `Express`, `Swap`, `Reset`, `ThreadFirst`,
`ThreadLast`, `SomeThreadFirst`, `SomeThreadLast`, `IfLet`,
`WhenLet`, `Fn`, `Lambda`, `Macro`, `ImportMacros`, `Class`,
`SetBang` (formerly `SetSymbol` — DD-36 retires the split name),
and `KernelPassthrough`.

**Rationale**: Tagged objects are the idiomatic JS equivalent of a
Rust enum. They are serialisable (important for the JSON interface
with Rust), JSDoc-describable (acceptable static typing for a JS
codebase that rejects Node), and have zero runtime cost beyond a
string comparison. TypeScript is out of scope for this DD; the
project is deliberately Node-less and Deno-native, and the JSDoc
typedefs carry enough information for IDE hover to work.

### Two-level AST mirroring DD-20

**Decision**: The JS compiler operates on two AST levels — the
reader's generic tree and the classifier's typed surface AST —
exactly as DD-20 specifies for Rust. The reader is unaware of
surface forms. The classifier validates form structure and produces
typed nodes. Downstream modules consume typed nodes.

The reader's output remains what it is today: `AtomNode |
StringNode | NumberNode | ListNode | ConsNode | KeywordNode`.
Keyword support is already in the reader; DD-15's `:name` → keyword
rule is implemented.

**Rationale**: Two-level ASTs are a standard compiler pattern (DD-20
cites Rust/TypeScript; the JS compiler adopts the same shape for the
same reasons). The existing JS compiler collapses both levels into
one, and pays for it with the `_kernel` marker. Separating them here
pays down that debt.

### Built-in surface forms as static transforms, not macros

**Decision**: Every built-in surface form becomes a **static
transform function**, not a macro. The `registerSurfaceMacros()`
entry point (`surface.js:893`) is retired. The surface dispatch
moves from the macro environment into the classifier + emitter pair.

Flow for built-in surface forms:

```
(func foo :args (x :Int) (+ x 1))
    │
    ▼ reader
ListNode(atoms...)
    │
    ▼ classifier (recognises 'func' → mkFunc)
{type: 'surface:func', name, clauses: [...]}
    │
    ▼ emitter (static transform)
[['function', 'foo', ['x'], [[...type check...], ['return', ['+', 'x', 1]]]]]
    │
    ▼ kernel compiler → ESTree → astring
```

Flow for user-defined macros (unchanged in spirit from DD-13):

```
(my-macro a b)
    │
    ▼ reader
ListNode(atoms...)
    │
    ▼ classifier (unknown head → FunctionCall placeholder)
(carried through until expansion)
    │
    ▼ macro expander (invokes compiled user macro)
expanded SExpr
    │
    ▼ classifier (re-classify expanded output)
(could be any surface form or kernel form)
    │
    ▼ emitter → kernel compiler → ESTree → astring
```

The key move: **built-in surface forms never enter the macro
environment.** This is the JS-side analogue of DD-20's statement
that "Built-in surface forms are NOT macros — they are typed AST
nodes handled by the classifier and emitter directly."

Consequences:

- `_kernel` marker is **deleted**. It exists only because surface
  macros emit kernel forms that the expander might re-walk. With
  built-in surface as static transforms, the expander only sees
  user-macro output, and re-classification (not re-expansion) is
  the fixed-point step.
- `kernelArray()` is **deleted**. It has no callers once surface
  forms are static transforms.
- `macroEnv` contains only user macros. Its reset semantics become
  cleaner (no need to preserve built-ins across tests).
- Adding a new surface form means adding a classifier case and an
  emitter transform — not adding a macro. This is a non-trivial
  cost compared to today's macro-based approach, but it gains typed
  structure, clearer errors, and removes the re-entry problem.

**Rationale**: This is the same decision DD-20 made for Rust, for
the same reasons. The current JS design works because it is the
simpler path, but it is what produced the `_kernel` marker in the
first place. DD-36 cannot land cleanly without this change.

### User-defined macros via `new Function()` — no subprocess

**Decision**: User-defined macros are evaluated via `new Function()`
in the same runtime as the compiler. This preserves the DD-13
approach and is the JS compiler's principal architectural advantage
over the Rust compiler, which must shell out to Deno (DD-20).

The `@lykn/browser` compiler inherits this directly: user macros
work in the browser because `new Function()` is available everywhere
JS runs. No subprocess, no embedded V8 negotiation, no Deno
dependency at runtime.

Macro evaluation remains a three-pass pipeline:

- **Pass 0** — Resolve `import-macros` declarations. Load macro
  modules (may be `.lykn` source compiled recursively, or a
  pre-compiled `.js` module per DD-34).
- **Pass 1** — Compile local `macro` definitions (fixed-point for
  order independence).
- **Pass 2** — Expand macro invocations. Top-down recursive walk
  with per-node safety limit (currently `MAX_EXPAND_ITERATIONS =
  1000` in `expander.js:40`).

Pass 2 now operates on the **classified tree**: `FunctionCall`
nodes whose head matches a macro are rewritten via macro invocation,
the output is re-read/re-classified, and the walk continues. No
kernel-output marker is needed because re-classification is
idempotent — a `Bind` node classified as `Bind` stays a `Bind`; it
cannot be re-expanded.

**Rationale**: Keeping the subprocess-free JS expander is a
competitive asset for the browser path and the test runner. DD-20
already calls the JS surface compiler the "future browser-path
compiler"; this DD formalises that and explicitly retains
`new Function()` as the evaluation strategy.

### No static analysis module — explicit trade-off

**Decision**: The JS compiler does **not** ship the analysis module
that DD-20 specifies for Rust. No exhaustiveness checking, no
overlap detection, no unused-binding warnings, no type registry.

The JS compiler's role is expansion and codegen only. Static
analysis remains the Rust compiler's responsibility, where it belongs
(typed AST, borrow-checked visitors, mature diagnostic
infrastructure).

This has user-visible consequences:

- A `.lykn` file compiled via the JS path produces working JS without
  exhaustiveness diagnostics. Missing `match` variants fail at
  runtime with a `throw` that DD-17 already emits as the fall-through
  branch.
- A `.lykn` file compiled via the Rust path catches the same issue at
  compile time.
- The `lykn` CLI routes to the Rust compiler by default. The JS
  compiler is invoked in browser contexts (where Rust can't run) and
  in test-DSL compilation where the runtime throw is an acceptable
  failure mode because the tests are the thing being checked.

**Rationale**: DD-20 makes this decision explicitly — "In the JS
surface compiler, ALL surface forms are macros — there is no
classifier, no typed AST, no analysis passes. This is fine because
the JS compiler doesn't provide static analysis. It's a pure
expansion engine." This DD refines the decision: we adopt the typed
AST and classifier (because they pay down the `_kernel` debt) but
keep the analysis-free stance. The result is a JS compiler that is
structurally richer than DD-20 described but still scoped to
expansion and codegen.

One forward compatibility note: because the typed AST is now
available in the JS compiler, **incremental analysis adoption** is
possible if a specific check is useful in the browser (e.g.,
"warn on `match` without a wildcard"). The architecture does not
have to change to add it; the decision is scoping, not capability.

### Kernel compiler is its own module

**Decision**: `compiler.js` becomes the **kernel compiler** only. It
accepts kernel SExpr, produces ESTree, and does nothing else. The
current 1,600-line file shrinks as surface forms move out.

Public shape:

```js
// compiler.js

/**
 * Compile a kernel s-expression tree to ESTree.
 * @param {SExpr} node — must contain only kernel forms
 * @returns {ESTreeNode}
 */
export function compileExpr(node) { ... }

/**
 * Compile a top-level kernel module to ESTree Program node.
 * @param {SExpr[]} forms
 * @returns {Program}
 */
export function compile(forms) { ... }
```

The `macros` dispatch table inside `compiler.js` (currently
`compiler.js:148`) is kept but restricted to kernel primitives: the
declaration forms (`var`, `const`, `let`), control flow (`if`,
`while`, `for`, `try`, `throw`, `return`, `break`, `continue`,
`switch`, `label`), functions (`function`, `function*`, `=>`,
`lambda`, `async`, `await`, `yield`), operators (arithmetic,
comparison, bitwise, logical, update), literals (`array`, `object`,
`get`, template literals), class forms (`class`, `class-expr`,
method definitions), module forms (`import`, `export`), and the
handful of special atoms (`this`, `super`, `true`, `false`, `null`,
`undefined`).

Anything that is a syntactic transform of another kernel form
(e.g., `.` method call) remains here. Anything that is a surface
construct moves to `surface/`.

The ESTree output is fed to `astring.generate()` in `mod.js`, which
remains the entry point for text output. No changes to astring or
its Deno import-map wrapping.

**Rationale**: The kernel compiler is the single module that most
directly corresponds to "what lykn means semantically at the JS
layer." It should be small, boring, and stable. The current size is
an artifact of surface forms leaking in.

### Emitter module: surface AST → kernel SExpr

**Decision**: A new `emitter.js` module consumes the surface AST
produced by the classifier/expander and emits kernel SExpr. This is
the direct analogue of DD-20's kernel emitter.

Each surface AST variant has an emission rule. The table mirrors
DD-20's:

| Surface node | Kernel emission (SExpr) |
|-------------|-------------------------|
| `Bind` | `(const name value)` |
| `Func` (single clause) | `(function name (args...) body...)` |
| `Func` (multi-clause) | `(function name (...args) dispatch)` |
| `Match` (statement) | `(if t1 b1 (if t2 b2 ...))` |
| `Match` (value) | IIFE wrapper |
| `Type` | Constructor `function` declarations + `const` bindings |
| `Obj` | `(object (k1 v1) ...)` |
| `Cell` | `(object (value init))` |
| `Express` | `target:value` atom |
| `Swap` | `(= target:value (f target:value))` |
| `Reset` | `(= target:value new-value)` |
| `ThreadFirst` | Nested kernel calls (pure rewrite) |
| `SomeThreadFirst` | IIFE with `== null` checks |
| `IfLet` / `WhenLet` | IIFE or statement depending on context |
| `Fn` / `Lambda` | `(lambda …)` or `(=> …)` |
| `SetBang` | `(= obj:key value)` |
| `KernelPassthrough` | Raw SExpr unchanged |

The emitter is **not** responsible for context detection or IIFE
wrapping — those stay with individual surface transform functions in
`surface/`. The emitter is a dispatcher and a stable interface for
producing kernel SExpr.

Each `surface/<form>.js` exports its transform:

```js
// surface/bind.js
import { kList, kAtom, kConst } from '../kernel-sexpr.js';

export function emitBind(node) {
  // node: { type: 'surface:bind', name, typeAnnotation, value, span }
  return kConst(node.name, emitValue(node.value));
}
```

The emitter iterates classified forms, dispatches on type, and calls
the relevant transform. The output is a flat array of kernel SExpr
that the kernel compiler can consume directly.

**Rationale**: Separating the emitter from the surface transforms
keeps the dispatcher tiny and makes each transform independently
testable. It also mirrors DD-20 more closely, which matters for
cross-compiler parity (see "JSON interchange with Rust").

### Classifier module: dispatch and validation

**Decision**: A new `classifier.js` module dispatches reader output
into surface AST nodes or passes through kernel forms. Its dispatch
tables are the JS analogue of `crates/lykn-lang/src/classifier/
dispatch.rs`:

```js
// classifier.js
const SURFACE_FORMS = new Set([
  'bind', 'func', 'genfunc', 'genfn', 'fn', 'lambda',
  'match', 'type',
  'obj', 'cell', 'express',
  'swap!', 'reset!', 'set!', 'set-symbol!',  // set-symbol! retires per DD-36
  '->', '->>', 'some->', 'some->>',
  'if-let', 'when-let',
  'and', 'or', 'not',
  '=', '!=',
  'macro', 'import-macros',
  'conj', 'assoc', 'dissoc',
]);

const KERNEL_FORMS = new Set([
  'var', 'const', 'let',
  'function', 'function*', '=>', 'lambda',
  'if', 'block', 'while', 'for', 'for-of', 'for-in', 'for-await-of',
  'do-while', 'try', 'throw', 'return', 'break', 'continue',
  'switch', 'label', 'seq', 'debugger',
  'array', 'object', 'get', 'spread', 'rest', 'default', 'alias',
  'template', 'tag', 'regex',
  'new', 'delete', 'typeof', 'instanceof', 'in', 'void',
  'yield', 'yield*', 'await', 'async', 'dynamic-import',
  'class', 'class-expr',
  'import', 'export',
  'quote', 'quasiquote',
  // arithmetic, comparison, bitwise, logical, update operators listed
  // explicitly (matching the Rust side):
  '+', '-', '*', '/', '%', '**',
  '===', '!==', '==', '!=', '<', '>', '<=', '>=',
  '&&', '||', '??',
  '&', '|', '^', /* etc. — full list mirrors dispatch.rs */
  '=', '?',  // note: '=' overlaps with surface (DD-36 cleanup)
]);
```

Per DD-36, these sets will be disjoint after the kernel/surface
split lands. Today's overlaps (`=`, `!=`, `macro`, `import-macros`)
are explicitly called out in DD-36 for cleanup.

Dispatch logic:

- Head atom in `SURFACE_FORMS` → call the variant-specific parser
  (e.g., `parseFunc(args, span)` returns a `Func` node).
- Head atom in `KERNEL_FORMS` → wrap as `KernelPassthrough`.
- Head atom is `kernel:<form>` (DD-36 escape hatch) → strip prefix,
  validate `<form>` against `KERNEL_FORMS`, wrap as
  `KernelPassthrough`. (Alt B in DD-36 would replace this with the
  reader-level `#k(...)` tag; under that variant, the reader
  produces a `KernelTag` node directly and the classifier just
  accepts it.)
- Unknown head atom → `FunctionCall` (literal function call; may
  resolve to a user macro at expansion time).

Surface form parsers perform structural validation: a malformed
`func` produces a diagnostic with source location, not a silent
pass-through. This is where today's "validate every time I touch
it" scattered across `surface.js` gets centralised.

**Rationale**: The classifier is the module that most directly makes
the kernel/surface split visible in the JS source. Once it exists,
every subsequent module has typed input. Without it, the split
cannot be enforced.

### Diagnostics module

**Decision**: A new `diagnostics.js` module defines the structured
diagnostic type used across the toolchain. Structure matches DD-20:

```js
// diagnostics.js

/**
 * @typedef {Object} Diagnostic
 * @property {'error' | 'warning' | 'info'} severity
 * @property {string} message
 * @property {Span} location
 * @property {string} [sourceForm]   — for contract error messages
 * @property {string} [suggestion]   — optional fix suggestion
 */

export function error(message, location, opts = {}) { ... }
export function warning(message, location, opts = {}) { ... }

export class DiagnosticError extends Error {
  constructor(diagnostic) { ... }
}
```

The current JS compiler throws plain `Error` objects with composed
messages. Replacing these with structured diagnostics enables
machine-readable output for IDE integration and uniform formatting
across the Rust and JS tools.

**Rationale**: Matches DD-20 for parity; enables future LSP work
without another rewrite; replaces the current practice of embedding
error context into message strings with structured fields that the
caller can format.

### JSON interchange with Rust (kernel SExpr format)

**Decision**: The kernel SExpr format is the **shared contract**
between the Rust emitter and the JS kernel compiler. DD-20 defines
the format as nested arrays of primitives:

```json
[
  ["const", "name", "Duncan"],
  ["const", "age", 42],
  ["function", "greet", ["name"], ["console.log", ["template", "Hello, ", "name"]]]
]
```

The JS compiler's kernel SExpr format must match exactly. This DD
formalises the requirement:

- Atoms → strings.
- Numbers → JSON numbers.
- Booleans → JSON booleans.
- `null` → JSON null.
- Strings → JSON strings (distinguishable from atoms by context —
  the position in the kernel form determines whether a string is a
  string literal or an atom).
- Lists → JSON arrays.
- Keywords → strings (kept simple per DD-20).

In practice the JS compiler already produces this shape — the reader
emits node objects (with `type` tags) that need to be flattened when
crossing the Rust boundary, but pure in-process compilation uses the
tagged nodes. DD-37 introduces a `kernel-sexpr.js` module with
constructors (`kList`, `kAtom`, `kString`, `kNumber`, `kKeyword`)
that produce the canonical in-memory shape the kernel compiler
consumes, plus a serializer for the JSON interchange format.

**Rationale**: Without a formalised shared format, the Rust and JS
paths drift apart. DD-20 specifies the format; DD-37 ratifies it on
the JS side and adds the constructors that make it easy to produce
correctly.

### Kernel-only compilation path (DD-36 integration)

**Decision**: `.lyk` files (DD-36) compile through a direct path
that skips classification and expansion entirely:

```
.lyk file
    │
    ▼ reader
SExpr tree
    │
    ▼ kernel compiler (compileExpr)
ESTree
    │
    ▼ astring
JavaScript
```

This path is exposed as `compileKernel(source)` in `mod.js`:

```js
// mod.js
export function lykn(source, options = {}) {
  if (options.kernel) return compileKernel(source);
  return compileSurface(source);
}

export function compileKernel(source) {
  const forms = read(source);
  return generate(compile(forms));  // astring
}

export function compileSurface(source) {
  const forms = read(source);
  const classified = classify(forms);
  const expanded = expand(classified);
  const kernel = emit(expanded);
  return generate(compile(kernel));
}
```

Extension-based dispatch lives in the CLI (`lykn-cli`); the JS
compiler exposes both entry points and lets callers choose.

The kernel compiler **refuses** surface-only forms in kernel mode —
if a `.lyk` file contains `(bind x 42)`, compilation fails with a
diagnostic pointing at the offending form and suggesting either
renaming to `.lykn` or converting to `(const x 42)`.

**Rationale**: This is the contract DD-36 depends on. Without a
clean kernel-only path, DD-36's extension gating has nowhere to
land. The bypass is also a performance win — kernel files skip the
entire surface pipeline.

### Gradual migration, not a big-bang rewrite

**Decision**: The migration from the current three-file pipeline to
the six-module architecture happens in an explicit sequence, each
step independently releasable.

The sequence, each step independently testable:

0. **Establish bundle-size baseline.** Before touching any source,
   record the current minified + gzipped size of `lykn-browser.js`
   (and any other published bundles). Add a CI check that fails on
   growth beyond a per-PR threshold (suggested: +2KB gzipped
   warning, +5KB hard fail without explicit sign-off). This turns
   the ±KB bands Duncan flagged into guard rails and makes every
   later step self-measuring.
1. **Extract the reader's JSDoc typedefs into `surface-ast.js`.**
   Create constructor functions; initially unused. No behavior
   change.
2. **Create `classifier.js` as a pass-through.** It accepts SExpr,
   walks the tree, and currently produces only `FunctionCall` and
   `KernelPassthrough` nodes. Wire it in between `read` and
   `expand`; existing tests still pass because the expander still
   recognises everything by macro dispatch. No behavior change.
3. **Move built-in surface forms out of `surface.js` one at a time.**
   For each form: add a classifier case that produces the typed AST
   node; add an emitter transform; remove the macro registration.
   After each form, full test suite passes AND the CI bundle-size
   check passes. If the per-form cost is higher than the budget,
   revisit the layout choice (flat emitter vs per-file `surface/`)
   or escalate to the alternative in "Classifier-only, keep macros
   as fallback" below.
4. **Delete `_kernel` marker once the last built-in surface form is
   migrated.** The fixed-point walk no longer sees its own output.
5. **Delete `kernelArray()` helper.** No callers remain.
6. **Trim `compiler.js`** — move `.` method call, `get`, and any
   other surface-in-disguise forms out.
7. **Introduce `compileKernel` and `compileSurface` entry points.**
   Add extension dispatch to the CLI.
8. **Introduce structured diagnostics.** Convert throw-call sites
   to produce `DiagnosticError` objects. Update the CLI to render
   them.

Each step is a small PR. Steps 1–2 are pure additions. Steps 3–5
are the core transformation and can be ordered to keep risk per-PR
low (start with the smallest surface form — probably `not` or
`swap!` — before tackling `func` and `match`). Steps 6–8 are
cleanup.

**Rationale**: The current JS compiler is the compilation backend
for the test suite (`lykn-cli/src/main.rs:480` delegates to
`lang/mod.js`). A big-bang rewrite would break the test suite
during the refactor. Incremental migration keeps the full test
matrix green at every step.

## Rejected alternatives

### Rewrite in TypeScript

**What**: Port the JS compiler to TypeScript during the refactor,
taking advantage of nominal types for the surface AST.

**Why rejected**: The project is deliberately Deno-native and rejects
Node.js. TypeScript adds a build step (`tsc`) that conflicts with
the Deno runtime model (which runs `.ts` directly but imposes its
own opinions). JSDoc typedefs carry enough information for IDE hover
and are runtime-free. If TypeScript ever becomes worthwhile, it's a
separate decision with its own DD.

### Collapse the classifier and emitter into one module

**What**: Do the classification and emission in a single walk, as
today's `surface.js` does with its macro functions.

**Why rejected**: Defeats the reuse case. The classifier is shared
with the (future) linter and formatter; the emitter is compiler-only.
Merging them re-creates the current coupling. It also makes
diagnostics harder — "this form is malformed" (classifier) and "this
form emits invalid kernel" (emitter) are different error categories
that deserve different source locations.

### Classifier-only, keep macros as fallback ("Alt C")

**What**: Add a classifier pass for the purpose of typed errors
(and to enforce DD-36's `.lyk` vs `.lykn` split), but leave the
surface-as-macro implementation intact. The `_kernel` marker and
`kernelArray()` helper survive; the typed AST exists for error
reporting and for the kernel-path gating but is not the runtime
transformation carrier.

**Why deferred (not rejected)**: This is the reduced-scope variant
of DD-37 and is explicitly on the table as a bundle-size contingency.
It preserves the worst scar (`_kernel` marker) in exchange for a
smaller bundle delta — probably +3–5KB gzipped instead of the
full-split estimate of +8–20KB. It still delivers:

- DD-36's extension-based gating (classifier can enforce "no surface
  forms in `.lyk`").
- Structured diagnostics from the classifier layer.
- A typed AST that future tooling (linter, formatter) can consume.

What it does **not** deliver:

- Retirement of `_kernel` / `kernelArray()`.
- Structural parity with the Rust compiler's emitter layer.
- A clean kernel SExpr interchange boundary between the two
  compilers (surface macros still emit kernel in-place rather than
  through a dedicated emitter module).

**Decision rule**: Measure after Phase 1. If the per-form migration
costs push `lykn-browser.js` past the growth budget Duncan
specified (comfortable +1KB, investigate +20KB, reject at +100KB),
pivot to this variant. It is the same architecture seen from a
different altitude — the classifier is the load-bearing piece; the
emitter/per-form split is the optional second floor.

### Mirror DD-20's analysis module

**What**: Port the Rust compiler's exhaustiveness, overlap, and
unused-binding analysis to JS.

**Why rejected (deferred)**: The Rust compiler is the safety path;
the JS compiler is the deploy-everywhere path. Duplicating analysis
is expensive and the Rust compiler is already where the analysis
lives. If a specific check proves valuable in browser or REPL
contexts, it can be added to the JS compiler without changing the
architecture, because the typed AST is available.

### Drop `astring` and emit JS text directly from kernel SExpr

**What**: Build a pure-JS codegen that outputs JS text without the
ESTree intermediate step, matching DD-30's pure-Rust codegen.

**Why rejected (deferred)**: `astring` is already a dependency, is
well-tested, and handles formatting concerns (parenthesisation,
operator precedence) correctly. Replacing it is non-trivial and
does not align with DD-37's scope (compiler architecture, not
codegen strategy). DD-30 covers the Rust side; a JS analogue is a
future decision.

### Ship surface AST as a separate package

**What**: Publish `@lykn/surface-ast` as a standalone JSR/npm
package so third-party tools can depend on it without pulling in
the compiler.

**Why rejected (deferred)**: Premature. The AST shape is stable
enough to be internal to `@lykn/lang` today. When a third-party
consumer materialises (IDE plugin, docs generator), extraction is
straightforward because `surface-ast.js` is already its own module.

### Merge kernel compiler into kernel emitter

**What**: One module takes surface AST → ESTree directly, skipping
the kernel SExpr layer.

**Why rejected**: The kernel SExpr layer is the **interop
boundary** with the Rust compiler. DD-20 specifies JSON kernel AST
as the cross-language format. Keeping the JS kernel compiler as a
separate module preserves the contract: both Rust and JS paths
produce the same kernel SExpr, and the kernel SExpr → ESTree →
JS step is identical. Merging would make cross-compiler parity
testing impossible.

## Bundle size considerations

The JS compiler is shipped as `lykn-browser.js` for in-browser
compilation of `<script type="text/lykn">` tags. Bundle size is
therefore a **user-visible** cost, not just a build concern. This
section captures the size budget, the sources of growth this DD
introduces, and mitigations.

**Budget** (from project guidance):

- +1KB gzipped: acceptable.
- +20KB gzipped: worth genuinely investigating and designing
  against.
- +100KB gzipped: unacceptable; do over.

**Estimated growth under the full architecture** (all eight
migration steps completed):

This is an estimate derived from source line counts, not from
compiled output. Actual numbers depend on minification + gzip
interactions and should be measured before any of this lands.

| Source | Lines added | Notes |
|--------|------------:|-------|
| `surface-ast.js` constructors + predicates | ~250 | Thin tagged-object factories |
| `classifier.js` dispatch + per-form parsers | ~400 | Replaces the implicit validation in today's macro bodies |
| `emitter.js` dispatcher + per-form transforms | ~300 | Mostly relocated from `surface.js`, not duplicated |
| `diagnostics.js` structured diagnostic module | ~150 | New error infrastructure |
| Per-form boilerplate (classifier case + transform) | +30–50 per form × ~20 forms | The cost Duncan asked about — this is the growth that can be minimised by layout choices |
| **Rough total** | **+1,700–2,100 lines** | |

After minification + gzip, plausible growth is **+8–20KB gzipped**.
That lands squarely in the "investigate" band, not the "show-
stopper" band, but it is not free and not automatic.

**Mitigations, in order of expected bang-per-byte**:

1. **Keep AST constructors bare.** A surface AST node is a tagged
   object literal and nothing else: `{type: 'surface:func', name,
   clauses, span}`. No class hierarchy, no factory defaults, no
   runtime validation. The tag string is the whole data. This is
   the single biggest knob for keeping the typed-AST layer cheap.

2. **Flat emitter dispatch over per-file `surface/`.** The initial
   file-layout proposal above is the flat version; a per-file
   directory is tempting for locality but adds module boilerplate
   (one import block per file, one export block per file). For ~20
   forms, this can be a measurable fraction of the delta. Decide
   during implementation based on bundle measurements.

3. **Factor shared emitter helpers aggressively.** `wrapReturnLast`,
   `buildThread`, `buildSomeThread`, `compilePattern`,
   `andChain`, context-detection utilities — `surface.js` already
   shares these. Preserve that sharing in the new layout; do not
   inline them per form.

4. **Lean diagnostics mode for browser.** The diagnostics module's
   pretty-printer and source-excerpt formatter are the largest
   pieces. A browser-only build that keeps the structured diagnostic
   shape but drops the rich formatter can shave a few KB. The CLI
   keeps the full formatter.

5. **Per-PR CI size check.** As noted in the migration sequence,
   add the check in Phase 0. This is the meta-mitigation: it keeps
   every subsequent step honest.

6. **Revisit "Alt C" (classifier-only).** If the measurements
   through Phase 3 show the per-form split is pushing past the
   +20KB band, pivot to the classifier-only variant. It retains the
   structural error reporting and DD-36 gating at a reduced cost,
   at the price of keeping the `_kernel` marker.

**What this means for the decision**: The full architecture is the
right target, but it is not unconditional. The migration sequence
is designed to fail fast if the bundle growth exceeds the budget,
and the "Alt C" variant is a pre-agreed fallback rather than a
mid-project scramble. The DD commits to measurement, not to a
specific final shape.

**Open measurement tasks** (to complete before Phase 3):

- Record baseline `lykn-browser.js` size at the current `main`
  commit.
- Prototype one full-pipeline migration (probably the smallest
  surface form, e.g. `not` or `reset!`) and measure delta per form.
- Extrapolate to ~20 forms and compare to budget.
- Decide flat-emitter vs per-file layout on evidence.

## Edge cases

| Case | Behavior |
|------|----------|
| Empty `.lykn` file | Classifier returns `[]`, emitter returns `[]`, kernel compiler produces empty `Program` |
| `.lyk` file with only kernel forms | Kernel path; no classifier involvement |
| `.lyk` file containing a surface form | Diagnostic: "surface form '<name>' not permitted in kernel file" |
| `.lykn` file with `(kernel:if c t e)` (or `#k(if c t e)`) | Classifier recognises prefix/tag, emits `KernelPassthrough` |
| `.lykn` file with bare `(if c t e)` | Depends on DD-36 decision for auto-promoted set: if `if` is in the set, classifier rewrites to `KernelPassthrough`; otherwise diagnostic |
| User macro expanding to surface form | Classified after expansion; produces typed AST |
| User macro expanding to kernel form | Classified after expansion; produces `KernelPassthrough` |
| User macro expanding to another macro call | Re-expanded until fixed point; classifier is idempotent |
| `.lykn` file with syntax error | Reader throws; wrapped in diagnostic by the CLI |
| `.lykn` file with malformed surface form | Classifier produces diagnostic with source location |
| Surface form inside kernel passthrough | Not classified (passthrough is opaque); the surface form compiles as if it were kernel (which may produce wrong JS) — this is by design, matches DD-20's passthrough semantics |
| Browser compilation (no file system) | `import-macros` with filesystem paths fails with a clear diagnostic; inline macros work |
| Test DSL (`bind`, `test` etc.) | These are surface macros in `@lykn/testing`; they live in its own file and are loaded via `import-macros` |

## Dependencies

- **Depends on**: DD-10 (quasiquote) through DD-14 (macro modules) —
  the macro expander retains DD-13's three-pass pipeline verbatim.
  DD-15 — surface form vocabulary and `js:` interop. DD-16–DD-19 —
  surface form semantics that the classifier and emitter implement.
  DD-20 — sibling DD on the Rust side; this DD is its JS counterpart.
  DD-34 — cross-package `import-macros` resolution is the expander's
  concern. DD-36 — the kernel/surface split that this DD enables.
- **Affects**: DD-36 directly — this DD is the Phase 0 prerequisite
  called out there. Future DDs: browser-path features, JS linter,
  JS formatter.

## Open questions

- [ ] **Where does `@lykn/browser` sit?** Today it re-exports from
  `@lykn/lang`. Under this architecture, does the browser runtime
  import the full six-module toolchain, or only the subset that
  works without filesystem access? My lean: import the full
  toolchain, document filesystem-dependent features (cross-package
  `import-macros`) as unsupported in browser contexts. A separate
  "lean browser" build (diagnostics formatter stripped, maybe more)
  is a bundle-size lever if needed — see Bundle size considerations.
- [ ] **Bundle-size measurements.** Baseline `lykn-browser.js` at
  the current `main` commit, prototype one full-pipeline surface
  form, measure the delta, extrapolate, and compare against the
  project's size budget. This is a prerequisite for Phase 3 of
  the migration and for the flat-emitter vs per-file layout
  decision.
- [ ] **Source maps.** DD-20 lists source maps as an open question
  for Rust. The JS compiler has the same opportunity: reader attaches
  spans, classifier preserves them, emitter can propagate them to
  ESTree `loc` fields, and `astring` supports source-map output. A
  future DD scopes the design.
- [ ] **REPL support.** If a REPL ships in 0.6.x, it needs an entry
  point that compiles a single form and evaluates it. The
  `compileExpr` and `expandExpr` exports already support this; the
  REPL design is a separate DD.
- [ ] **Linter / formatter.** Both tools use the reader and
  classifier. The specific rules and formatting policies are their
  own DDs. This DD ensures they have the modules they need.
- [ ] **Incremental compilation.** The three-phase shape supports
  caching classified trees and re-running only the emitter when a
  downstream form changes. Useful for test-watch workflows. Not
  designed here.
- [ ] **Parity testing with the Rust compiler.** DD-20 proposes
  kernel JSON as canonical test fixtures. Once `emitter.js`
  produces deterministic kernel SExpr, a parity test suite that
  compiles the same `.lykn` input through both compilers and
  compares JSON output becomes a natural regression guard. Worth
  building; not designed here.
- [ ] **Surface form error-message style.** The current JS compiler
  throws `Error(message)` with composed strings. DD-37 introduces
  structured diagnostics but does not specify the message style
  guide. Worth consolidating with the Rust compiler's style.
- [ ] **Keyword handling.** The reader already produces a `keyword`
  node type, but DD-15 says keywords serialise to strings in kernel
  output. The classifier needs an explicit keyword-handling rule:
  in surface position, keywords are identifiers (`:name` → field
  name); in kernel position, they serialise to `"name"`. Worth
  pinning down in the emitter spec.

## Citations

All file references verified against the current workspace.

- `packages/lang/mod.js` — 20-line entry point; `lykn(source) =
  compile(expand(read(source)))`.
- `packages/lang/reader.js` — 320 lines; six node types
  (`atom`/`string`/`number`/`list`/`cons`, plus keyword handling).
- `packages/lang/expander.js` — 1,350 lines. Public entry `expand`
  at line 1,341. `_kernel` marker at line 731. `MAX_EXPAND_ITERATIONS
  = 1000` at line 40. Three-pass pipeline: `pass0ImportMacros` at
  line 1,122, `pass1RegisterMacros` at line 856, `pass2ExpandAll`
  at line 944.
- `packages/lang/surface.js` — 2,262 lines. `kernelArray()` helper
  at line 24 (used at lines 1026, 1133, 1148, 1167, 1181).
  `registerSurfaceMacros()` export at line 893.
- `packages/lang/compiler.js` — 1,600 lines. `macros` dispatch table
  at line 148. `compileExpr` at line 1,139. `compile` at line 1,593.
- `packages/browser/mod.js` — re-exports compile/run/load from
  lang via `compiler.js` wrapper.
- `docs/design/06-final/0025-dd-20-rust-surface-compiler-architecture.md`
  — DD-20, the Rust analogue this DD mirrors.
- `workbench/dd-36-kernel-surface-split.md` — DD-36, which depends
  on this DD as Phase 0.

## Verification notes

- I read `mod.js`, `reader.js` (header), the full definition lists
  of `expander.js` and `compiler.js`, the `macros` dispatch table
  header in `compiler.js`, and `surface.js` top-level exports and
  `registerSurfaceMacros` signature. I did **not** read the full
  2,262 lines of `surface.js`; my claims about its shape are
  extrapolated from its exports and the grep results for
  `kernelArray`/`_kernel`.
- The proposed file layout (`surface/bind.js`, `surface/func.js`,
  etc.) is a recommendation, not a reflection of current structure.
  The current `surface.js` is one file; the split into per-form
  files is part of the migration.
- The migration sequence is a design proposal, not a worked plan.
  Step-by-step PR sizing requires reading each surface form's
  current implementation to estimate extraction cost.
