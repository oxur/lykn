# Phase 2 — Core Forms: Implementation Guide

**For**: Claude Code
**Scope**: Phase 2 of lykn v0.1.0 — all core language forms
**Where you're working**: `src/compiler.js` — adding entries to the `macros` object and supporting utilities
**Prerequisite**: Phase 1 must be complete (`toCamelCase`, colon syntax, `get` form, `.` removed)
**Design authority**: The decision docs listed per-section below, all in `crates/design/dev/lang/`

---

## Overview: What Phase 2 Is

Phase 2 adds 24 new forms to the compiler. Every one is a new entry in the `macros` object (or a bulk registration like the existing binary/unary operators). The items are **independent of each other** — you can build them in any order.

These forms fall into five natural groups:

| Group | Forms | Design Doc |
|-------|-------|------------|
| A: Functions | `function`, `async`, `await` | DD-02, DD-03 |
| B: Modules | `import`, `export`, `alias`, `dynamic-import` | DD-04 |
| C: Control Flow | `throw`, `try`, `while`, `do-while`, `for`, `for-of`, `for-in`, `break`, `continue`, `switch` | DD-08, gap analysis |
| D: Operators | `?`, `++`, `--`, `**`, compound assignments | DD-08 |
| E: Miscellaneous | `label`, `debugger`, `seq`, `regex` | DD-08 |

**Recommended build order**: A → D → E → C → B. Functions first because they're simple and highly visible; operators because they're mechanical; misc because they're trivial; control flow because `try` and `switch` have structural complexity; modules last because `import`/`export` have the most dispatch variants.

But any order works. All 24 forms depend only on Phase 1 infrastructure (camelCase + colon syntax), not on each other.

---

## How Macros Work (Refresher)

Every macro is a function `(args) => ESTreeNode` in the `macros` object. When the compiler encounters a list `(foo a b c)`, it checks if `foo` is in `macros`. If so, it calls `macros['foo']([a_node, b_node, c_node])` where each arg is a raw reader AST node (`{type: 'atom' | 'string' | 'number' | 'list', ...}`).

**Critical**: The `args` array contains **raw reader nodes**, not compiled ESTree nodes. You must call `compileExpr(args[i])` to compile each argument. The only exception is when you need to inspect the raw structure (e.g., checking if an arg is an atom with a specific value, or a list you want to destructure yourself).

**Pattern for inspecting vs compiling**:
```js
// Inspecting raw structure (DON'T compile first):
if (args[0].type === 'atom' && args[0].value === 'default') { ... }

// Using as a compiled expression (DO compile):
const test = compileExpr(args[0]);
```

---

## Group A: Functions (DD-02, DD-03)

### 2.1 `function` — Function Declarations

**Design doc**: `0002-dd-02-function-forms-declaration-vs-expression.md`

**Syntax**: `(function name (params) body...)`

**What it produces**: `FunctionDeclaration`

**Key decisions from DD-02**:
- `function` is ALWAYS a declaration (produces `FunctionDeclaration`, not `FunctionExpression`)
- Name is REQUIRED — `(function (params) body)` with no name is a compile error. Use `lambda` for anonymous function expressions.
- `lambda` is KEPT as the expression form (already implemented in v0.0.1)
- Named function expressions are DEFERRED to v0.2.0

**Implementation**:

```js
'function'(args) {
  // args[0] = name (atom), args[1] = params (list), args[2..] = body
  if (args.length < 3) {
    throw new Error('function requires a name, params list, and body: (function name (params) body...)');
  }
  if (args[0].type !== 'atom') {
    throw new Error('function name must be an identifier, not a ' + args[0].type);
  }
  if (args[1].type !== 'list') {
    throw new Error('function params must be a list: (function name (params) body...)');
  }

  const params = args[1].values.map(compileExpr);
  const bodyExprs = args.slice(2);

  return {
    type: 'FunctionDeclaration',
    id: { type: 'Identifier', name: toCamelCase(args[0].value) },
    params,
    body: {
      type: 'BlockStatement',
      body: bodyExprs.map(e => toStatement(compileExpr(e))),
    },
    async: false,
    generator: false,
  };
},
```

**Compiler pitfall — `function` vs `lambda` vs `=>`**:

These three forms map to three different ESTree nodes:

| lykn | ESTree | JS output | Hoisted? |
|------|--------|-----------|----------|
| `(function name (params) body...)` | `FunctionDeclaration` | `function name(params) { body }` | Yes |
| `(lambda (params) body...)` | `FunctionExpression` (anonymous) | `function(params) { body }` | No |
| `(=> (params) body...)` | `ArrowFunctionExpression` | `(params) => body` | No |

`lambda` and `=>` are already implemented. You're only adding `function`. Don't modify `lambda` or `=>`.

**Compiler pitfall — `toStatement` wrapping**:

Both `function` and `lambda` need their body expressions wrapped via `toStatement()`. This is already handled correctly in the existing `lambda` implementation — follow the same pattern. Each body expression is compiled and then wrapped so that expressions become `ExpressionStatement` nodes, while statements (like `return`, `if`) pass through.

**Compiler pitfall — Name goes through `toCamelCase`**:

`(function my-handler (req) ...)` → `function myHandler(req) { ... }`. The function name is an identifier and gets camelCased. Same for parameter names — they're compiled via `compileExpr`, which hits the atom branch where `toCamelCase` is already applied.

**Compiler pitfall — `FunctionDeclaration` is already a statement**:

Unlike most macro results (which are expressions that need `ExpressionStatement` wrapping), `FunctionDeclaration` has a type ending in `Declaration`. The existing `toStatement()` function already checks for this suffix and passes it through. So `(function ...)` at top level works correctly without any special handling.

---

### 2.2 `async` — Async Wrapper

**Design doc**: `0003-dd-03-async-await.md`

**Syntax**: `(async (function-form ...))`

**What it produces**: The same node as the wrapped function form, but with `async: true`

**Key decisions from DD-03**:
- `async` is a WRAPPER, not a prefix or flag. It takes a single argument which must be a function form.
- The child must be `function`, `lambda`, or `=>`. Anything else is a compile error.
- `async` simply compiles the child function form, then sets `async: true` on the result.
- Top-level `await` is free (no special handling needed — lykn already sets `sourceType: "module"`)

**Implementation**:

```js
'async'(args) {
  if (args.length !== 1) {
    throw new Error('async takes exactly one argument: (async (function/lambda/=> ...))');
  }

  const child = args[0];
  if (child.type !== 'list' || child.values.length === 0) {
    throw new Error('async argument must be a function form: (async (function/lambda/=> ...))');
  }

  const head = child.values[0];
  if (head.type !== 'atom' || !['function', 'lambda', '=>'].includes(head.value)) {
    throw new Error(
      'async can only wrap function, lambda, or =>: got ' +
      (head.type === 'atom' ? head.value : head.type)
    );
  }

  // Compile the inner function form normally
  const compiled = compileExpr(child);
  // Set the async flag
  compiled.async = true;
  return compiled;
},
```

**Compiler pitfall — `async` must compile the WHOLE child form, not just call the macro**:

Don't do `macros[head.value](child.values.slice(1))` — use `compileExpr(child)` instead. `compileExpr` handles the full list dispatch including the macro lookup. This keeps the code path consistent and means if `function`/`lambda`/`=>` implementations change, `async` doesn't break.

**Compiler pitfall — mutation of the compiled node is intentional**:

Setting `compiled.async = true` mutates the ESTree node returned by `compileExpr`. This is fine — ESTree nodes are plain objects, and each `compileExpr` call creates fresh nodes. There's no shared mutable state. The existing `lambda` and `=>` handlers already create nodes with `async` defaulting to `undefined` (which astring treats as falsy), so setting it to `true` is sufficient.

**Compiler pitfall — `async` with `function` produces `async function` declaration**:

```lisp
(async (function fetch-data ()
  (const data (await (fetch url)))
  (return data)))
```
→
```js
async function fetchData() {
  const data = await fetch(url);
  return data;
}
```

The compiled node is a `FunctionDeclaration` with `async: true`. astring handles the `async` keyword output.

**Important: add `async: false` to the `function` macro (2.1)** so the flag exists on the node before `async` sets it. Also add `async: false` to the existing `lambda` handler if it doesn't have it. The `=>` handler may also need it. Check each one.

---

### 2.3 `await` — Await Expression

**Design doc**: `0003-dd-03-async-await.md`

**Syntax**: `(await expr)`

**What it produces**: `AwaitExpression`

**Implementation**:

```js
'await'(args) {
  if (args.length !== 1) {
    throw new Error('await takes exactly one argument');
  }
  return {
    type: 'AwaitExpression',
    argument: compileExpr(args[0]),
  };
},
```

**Key decision**: No context validation. The compiler does NOT check whether `await` appears inside an `async` function. The JS engine will report that error at runtime. This is the same approach used for `return` — the compiler doesn't validate that `return` is inside a function.

---

## Group B: Modules (DD-04)

**Design doc**: `0004-dd-04-modules-import-export.md`

### Understanding ESTree Module Nodes

Before implementing `import` and `export`, you need to understand the ESTree nodes they produce. This is the most structurally complex part of Phase 2.

**Import nodes**:
```
ImportDeclaration {
  type: "ImportDeclaration",
  specifiers: [ImportSpecifier | ImportDefaultSpecifier],
  source: Literal (string)
}

ImportDefaultSpecifier {
  type: "ImportDefaultSpecifier",
  local: Identifier          // the local binding name
}

ImportSpecifier {
  type: "ImportSpecifier",
  imported: Identifier,      // the name in the source module
  local: Identifier          // the local binding name (same as imported if no rename)
}
```

**Export nodes**:
```
ExportNamedDeclaration {
  type: "ExportNamedDeclaration",
  declaration: Declaration | null,   // when exporting a declaration
  specifiers: [ExportSpecifier],     // when exporting existing bindings
  source: Literal | null             // when re-exporting from another module
}

ExportDefaultDeclaration {
  type: "ExportDefaultDeclaration",
  declaration: Expression | Declaration
}

ExportSpecifier {
  type: "ExportSpecifier",
  local: Identifier,         // the local name
  exported: Identifier       // the exported name
}
```

---

### 2.4 `import` — Unified Import Form

**Syntax variants** (module path is ALWAYS first):

```lisp
(import "mod")                         ;; side-effect import
(import "mod" name)                    ;; default import
(import "mod" (a b))                   ;; named imports
(import "mod" (a (alias b local)))     ;; named with rename
(import "mod" name (a b))              ;; default + named
```

**Banned**: `import *` (namespace imports). No `ImportNamespaceSpecifier`.

**Implementation strategy**: Dispatch on the shape of args after the source string.

```js
'import'(args) {
  if (args.length === 0) {
    throw new Error('import requires at least a module path');
  }

  // First arg MUST be a string (module path)
  if (args[0].type !== 'string') {
    throw new Error('import: first argument must be a module path string');
  }

  const source = { type: 'Literal', value: args[0].value };
  const specifiers = [];

  if (args.length === 1) {
    // (import "mod") → side-effect import
    // No specifiers — just the source
  } else if (args.length === 2) {
    if (args[1].type === 'atom') {
      // (import "mod" name) → default import
      specifiers.push({
        type: 'ImportDefaultSpecifier',
        local: { type: 'Identifier', name: toCamelCase(args[1].value) },
      });
    } else if (args[1].type === 'list') {
      // (import "mod" (a b)) → named imports
      for (const spec of args[1].values) {
        specifiers.push(buildImportSpecifier(spec));
      }
    } else {
      throw new Error('import: second argument must be a name or list of names');
    }
  } else if (args.length === 3) {
    // (import "mod" name (a b)) → default + named
    if (args[1].type !== 'atom') {
      throw new Error('import: default import name must be an identifier');
    }
    if (args[2].type !== 'list') {
      throw new Error('import: named imports must be a list');
    }
    specifiers.push({
      type: 'ImportDefaultSpecifier',
      local: { type: 'Identifier', name: toCamelCase(args[1].value) },
    });
    for (const spec of args[2].values) {
      specifiers.push(buildImportSpecifier(spec));
    }
  } else {
    throw new Error('import: too many arguments');
  }

  return {
    type: 'ImportDeclaration',
    specifiers,
    source,
  };
},
```

**Helper function** — add this as a module-level function near `toCamelCase`:

```js
function buildImportSpecifier(node) {
  if (node.type === 'atom') {
    // Simple named import: a → import { a }
    const name = toCamelCase(node.value);
    return {
      type: 'ImportSpecifier',
      imported: { type: 'Identifier', name },
      local: { type: 'Identifier', name },
    };
  }
  if (node.type === 'list' && node.values.length >= 2 &&
      node.values[0].type === 'atom' && node.values[0].value === 'alias') {
    // (alias imported local) → import { imported as local }
    return {
      type: 'ImportSpecifier',
      imported: { type: 'Identifier', name: toCamelCase(node.values[1].value) },
      local: { type: 'Identifier', name: toCamelCase(node.values[2].value) },
    };
  }
  throw new Error('import: each specifier must be a name or (alias original local)');
}
```

**Compiler pitfall — camelCase on import names but NOT on module paths**:

`(import "node:fs" (read-file-sync))` → `import { readFileSync } from "node:fs";`

The module path `"node:fs"` is a string literal — it passes through unchanged. The import name `read-file-sync` is an atom that becomes an `Identifier`, so it gets camelCased to `readFileSync`. This happens naturally because `toCamelCase` is only called on atoms, never on strings.

**Compiler pitfall — `ImportDeclaration` is NOT an `ExpressionStatement`**:

`ImportDeclaration` ends in "Declaration", so `toStatement()` passes it through. Good. But note: `ImportDeclaration` can only appear at the top level of a module. The compiler doesn't enforce this — if someone writes `(if true (import "mod"))`, they'll get an ESTree that astring will try to generate, and the JS engine will reject. This is fine — we don't validate statement-level legality.

**Compiler pitfall — `imported` vs `local` on `ImportSpecifier`**:

For a plain `(import "mod" (foo))`, both `imported` and `local` must be set to the same `Identifier`. ESTree requires both fields even when there's no rename. When there IS a rename via `(alias foo bar)`, `imported` is `foo` (the name in the source module) and `local` is `bar` (the name in our code).

---

### 2.5 `export` — Unified Export Form

**Syntax variants**:

```lisp
(export (const x 42))                  ;; export declaration
(export (function foo () ...))         ;; export function decl
(export default expr)                  ;; export default
(export (names a b))                   ;; export existing bindings
(export (names (alias a ext)))         ;; export with rename
(export "mod" (names a b))             ;; re-export named
```

**Banned**: `export *` (re-export all). No `ExportAllDeclaration`.

**Implementation strategy**: Dispatch on the shape of args.

```js
'export'(args) {
  if (args.length === 0) {
    throw new Error('export requires an argument');
  }

  // Case 1: (export default expr)
  if (args[0].type === 'atom' && args[0].value === 'default') {
    if (args.length !== 2) {
      throw new Error('export default takes exactly one expression');
    }
    return {
      type: 'ExportDefaultDeclaration',
      declaration: compileExpr(args[1]),
    };
  }

  // Case 2: (export "mod" (names ...)) → re-export
  if (args[0].type === 'string') {
    if (args.length !== 2 || args[1].type !== 'list') {
      throw new Error('export re-export: (export "mod" (names ...))');
    }
    return buildExportNames(args[1], args[0]);
  }

  // Case 3: (export (names ...)) → export existing bindings
  if (args[0].type === 'list' && args[0].values.length > 0 &&
      args[0].values[0].type === 'atom' && args[0].values[0].value === 'names') {
    return buildExportNames(args[0], null);
  }

  // Case 4: (export (const/let/var/function/class ...)) → export declaration
  if (args.length === 1) {
    const decl = compileExpr(args[0]);
    return {
      type: 'ExportNamedDeclaration',
      declaration: decl,
      specifiers: [],
      source: null,
    };
  }

  throw new Error('export: unrecognized form');
},
```

**Helper function** for the `(names ...)` sub-form:

```js
function buildExportNames(namesNode, sourceNode) {
  // namesNode is a list: (names a b (alias c ext) ...)
  // sourceNode is a raw reader node (string) or null
  const items = namesNode.values.slice(1); // skip the 'names' head
  const specifiers = items.map(item => {
    if (item.type === 'atom') {
      const name = toCamelCase(item.value);
      return {
        type: 'ExportSpecifier',
        local: { type: 'Identifier', name },
        exported: { type: 'Identifier', name },
      };
    }
    if (item.type === 'list' && item.values.length >= 2 &&
        item.values[0].type === 'atom' && item.values[0].value === 'alias') {
      return {
        type: 'ExportSpecifier',
        local: { type: 'Identifier', name: toCamelCase(item.values[1].value) },
        exported: { type: 'Identifier', name: toCamelCase(item.values[2].value) },
      };
    }
    throw new Error('export names: each item must be a name or (alias local exported)');
  });

  return {
    type: 'ExportNamedDeclaration',
    declaration: null,
    specifiers,
    source: sourceNode ? { type: 'Literal', value: sourceNode.value } : null,
  };
}
```

**Compiler pitfall — `export default` vs `export declaration` dispatch**:

The `default` atom must be checked FIRST. If you check for "is args[0] a list?" first, you'd miss `(export default ...)` because `default` is an atom. The dispatch order matters:

```
1. args[0] is atom "default" → ExportDefaultDeclaration
2. args[0] is string → re-export
3. args[0] is list with head "names" → export existing bindings
4. args[0] is list (anything else) → export declaration
```

**Compiler pitfall — `ExportSpecifier` `local` vs `exported` semantics are REVERSED from `ImportSpecifier`**:

This is an ESTree design that trips people up:

| Node | `local` means | `imported`/`exported` means |
|------|---------------|-----------------------------|
| `ImportSpecifier` | the name in YOUR code | the name in the SOURCE module |
| `ExportSpecifier` | the name in YOUR code | the name CONSUMERS see |

For `(export (names (alias my-func external-name)))`:
- `local` = `myFunc` (your internal binding)
- `exported` = `externalName` (what consumers import)

This is the opposite direction from import. Get it backwards and the generated JS will have the names swapped.

**Compiler pitfall — `export declaration` wraps the compiled child**:

`(export (const x 42))` compiles `(const x 42)` first (producing a `VariableDeclaration`), then wraps it in `ExportNamedDeclaration`. The compiled declaration goes in the `declaration` field, and `specifiers` is empty.

`(export (function foo () ...))` does the same — compiles the `function` form (producing `FunctionDeclaration`), wraps it.

---

### 2.6 `alias` — Renaming Sub-form

`alias` is NOT a standalone macro. It's a structural pattern recognized by `import` and `export` (and later by destructuring in Phase 4). You don't add it to the `macros` object.

The `buildImportSpecifier` and `buildExportNames` helpers above already handle `alias` by checking for lists whose first element is the atom `alias`. No additional code needed.

**For Phase 4**: `alias` will also appear in destructuring patterns: `(const (object (alias old-name new-name)) obj)`. That's handled in Phase 4's pattern compiler, not here.

---

### 2.7 `dynamic-import` — Dynamic Import Expression

**Syntax**: `(dynamic-import expr)`

**What it produces**: `ImportExpression`

```js
'dynamic-import'(args) {
  if (args.length !== 1) {
    throw new Error('dynamic-import takes exactly one argument');
  }
  return {
    type: 'ImportExpression',
    source: compileExpr(args[0]),
  };
},
```

**Why it's separate from `import`**: `import` is a declaration (top-level only). `dynamic-import` is an expression (can appear anywhere — inside functions, in `await`, etc.). They produce completely different ESTree nodes: `ImportDeclaration` vs `ImportExpression`.

```lisp
;; Static import (declaration, top-level)
(import "mod" (foo))

;; Dynamic import (expression, anywhere)
(const mod (await (dynamic-import "./mod.js")))
```

---

## Group C: Control Flow

### 2.8 `throw`

**Syntax**: `(throw expr)`

```js
'throw'(args) {
  if (args.length !== 1) {
    throw new Error('throw takes exactly one argument');
  }
  return {
    type: 'ThrowStatement',
    argument: compileExpr(args[0]),
  };
},
```

---

### 2.9 `try` / `catch` / `finally`

**Syntax**:
```lisp
(try
  body...
  (catch e body...)
  (finally body...))
```

**What it produces**: `TryStatement` with optional `CatchClause` and `BlockStatement` finalizer.

**The tricky part**: `catch` and `finally` are NOT separate macros. They're structural elements recognized within the `try` form by checking whether the last element(s) of the body are lists whose head atom is `catch` or `finally`.

**Implementation**:

```js
'try'(args) {
  if (args.length === 0) {
    throw new Error('try requires a body');
  }

  let handler = null;
  let finalizer = null;
  let bodyEnd = args.length;

  // Scan from the end for catch/finally clauses
  // Check last arg for finally
  const lastArg = args[args.length - 1];
  if (lastArg.type === 'list' && lastArg.values.length > 0 &&
      lastArg.values[0].type === 'atom' && lastArg.values[0].value === 'finally') {
    finalizer = {
      type: 'BlockStatement',
      body: lastArg.values.slice(1).map(e => toStatement(compileExpr(e))),
    };
    bodyEnd--;
  }

  // Check the (possibly new) last arg for catch
  if (bodyEnd > 0) {
    const catchArg = args[bodyEnd - 1];
    if (catchArg.type === 'list' && catchArg.values.length > 0 &&
        catchArg.values[0].type === 'atom' && catchArg.values[0].value === 'catch') {
      const catchParam = catchArg.values[1];
      handler = {
        type: 'CatchClause',
        param: compileExpr(catchParam),
        body: {
          type: 'BlockStatement',
          body: catchArg.values.slice(2).map(e => toStatement(compileExpr(e))),
        },
      };
      bodyEnd--;
    }
  }

  if (!handler && !finalizer) {
    throw new Error('try requires at least a catch or finally clause');
  }

  const body = {
    type: 'BlockStatement',
    body: args.slice(0, bodyEnd).map(e => toStatement(compileExpr(e))),
  };

  return {
    type: 'TryStatement',
    block: body,
    handler,
    finalizer,
  };
},
```

**Compiler pitfall — scan order matters**:

You MUST check for `finally` FIRST (at the very end), then check for `catch` (at the new end). If a `try` has both:

```lisp
(try
  (do-something)
  (catch e (handle e))
  (finally (cleanup)))
```

Args are: `[(do-something), (catch e (handle e)), (finally (cleanup))]`. Check index 2 for `finally` → found. Then check index 1 for `catch` → found. Body is `[index 0]`.

If you checked `catch` first at the end, you'd find `(finally ...)` and think it's not a catch.

**Compiler pitfall — `handler` vs `block` vs `finalizer` field names**:

ESTree uses `block` (not `body`) for the try body, `handler` (not `catch`) for the catch clause, and `finalizer` (not `finally`) for the finally block. Get these field names wrong and astring will silently drop the content.

---

### 2.10 `while`

**Syntax**: `(while test body...)`

```js
'while'(args) {
  if (args.length < 2) {
    throw new Error('while requires a test and body');
  }
  return {
    type: 'WhileStatement',
    test: compileExpr(args[0]),
    body: {
      type: 'BlockStatement',
      body: args.slice(1).map(e => toStatement(compileExpr(e))),
    },
  };
},
```

---

### 2.11 `do-while`

**Design doc**: `0008-dd-08-special-atoms-update-operators-and-miscellaneous-forms.md` (Topic G)

**Syntax**: `(do-while test body...)`

**Key decision from DD-08**: Test comes FIRST in lykn (for internal consistency with `while`), even though JS puts the test last. The compiled output is `do { body } while (test)` — the reordering happens in the ESTree node.

```js
'do-while'(args) {
  if (args.length < 2) {
    throw new Error('do-while requires a test and body');
  }
  return {
    type: 'DoWhileStatement',
    test: compileExpr(args[0]),
    body: {
      type: 'BlockStatement',
      body: args.slice(1).map(e => toStatement(compileExpr(e))),
    },
  };
},
```

**Compiler pitfall — test position**:

The lykn programmer writes `(do-while (> x 0) (-= x 1))` — test first, body second.
The generated JS is `do { x -= 1; } while (x > 0)` — body first, test second.
The ESTree node just has `test` and `body` fields; astring handles the ordering in the output. So you just set the fields correctly and astring does the rest.

---

### 2.12 `for` — C-style For Loop

**Syntax**: `(for init test update body...)`

Init, test, and update can be `()` (empty list) for null/empty.

```js
'for'(args) {
  if (args.length < 4) {
    throw new Error('for requires init, test, update, and body: (for init test update body...)');
  }

  const init = args[0].type === 'list' && args[0].values.length === 0
    ? null
    : compileExpr(args[0]);
  const test = args[1].type === 'list' && args[1].values.length === 0
    ? null
    : compileExpr(args[1]);
  const update = args[2].type === 'list' && args[2].values.length === 0
    ? null
    : compileExpr(args[2]);

  return {
    type: 'ForStatement',
    init,
    test,
    update,
    body: {
      type: 'BlockStatement',
      body: args.slice(3).map(e => toStatement(compileExpr(e))),
    },
  };
},
```

**Compiler pitfall — `()` for null slots**:

An empty list `()` in lykn normally produces `ArrayExpression` with no elements. But in `for`, we special-case it as `null`. This is necessary for `for (;;)` (infinite loops). Check for `args[n].type === 'list' && args[n].values.length === 0` BEFORE calling `compileExpr`.

**Compiler pitfall — `init` can be a declaration or expression**:

`(for (let i 0) (< i 10) (++ i) ...)` — the init is `(let i 0)`, which compiles to a `VariableDeclaration`. ESTree's `ForStatement.init` accepts both `VariableDeclaration` and `Expression`. The existing `let` macro handles this, and `compileExpr` returns the right node. No special handling needed.

---

### 2.13 `for-of` — For...of Loop

**Syntax**: `(for-of binding iterable body...)`

```js
'for-of'(args) {
  if (args.length < 3) {
    throw new Error('for-of requires binding, iterable, and body');
  }

  const binding = compileExpr(args[0]);

  return {
    type: 'ForOfStatement',
    left: {
      type: 'VariableDeclaration',
      kind: 'const',
      declarations: [{
        type: 'VariableDeclarator',
        id: binding,
        init: null,
      }],
    },
    right: compileExpr(args[1]),
    body: {
      type: 'BlockStatement',
      body: args.slice(2).map(e => toStatement(compileExpr(e))),
    },
    await: false,
  };
},
```

**Compiler pitfall — the `left` side is a `VariableDeclaration`, not an `Identifier`**:

ESTree's `ForOfStatement.left` must be a `VariableDeclaration`, not a bare `Identifier`. The generated JS is `for (const item of items)`, not `for (item of items)`. We always use `const` as the kind.

**Compiler pitfall — destructuring in the binding**:

In Phase 4, the binding can be a destructuring pattern: `(for-of (object name age) people ...)` → `for (const { name, age } of people)`. This will work automatically once Phase 4 implements pattern detection in `compileExpr`. For now, it works with simple atom bindings.

---

### 2.14 `for-in` — For...in Loop

**Syntax**: `(for-in binding object body...)`

Structurally identical to `for-of`, but uses `ForInStatement`.

```js
'for-in'(args) {
  if (args.length < 3) {
    throw new Error('for-in requires binding, object, and body');
  }

  const binding = compileExpr(args[0]);

  return {
    type: 'ForInStatement',
    left: {
      type: 'VariableDeclaration',
      kind: 'const',
      declarations: [{
        type: 'VariableDeclarator',
        id: binding,
        init: null,
      }],
    },
    right: compileExpr(args[1]),
    body: {
      type: 'BlockStatement',
      body: args.slice(2).map(e => toStatement(compileExpr(e))),
    },
  };
},
```

---

### 2.15 `break` / `continue`

**Syntax**:
```lisp
(break)              ;; no label
(break my-label)     ;; with label (camelCased)
(continue)           ;; no label
(continue my-label)  ;; with label (camelCased)
```

```js
'break'(args) {
  return {
    type: 'BreakStatement',
    label: args.length > 0
      ? { type: 'Identifier', name: toCamelCase(args[0].value) }
      : null,
  };
},

'continue'(args) {
  return {
    type: 'ContinueStatement',
    label: args.length > 0
      ? { type: 'Identifier', name: toCamelCase(args[0].value) }
      : null,
  };
},
```

**Compiler pitfall — label names get camelCased**:

`(break my-loop)` → `break myLoop;`. This is consistent with DD-08's decision that labels are identifiers and all identifiers get camelCased.

---

### 2.16 `switch` / `case`

**Syntax**:
```lisp
(switch expr
  (test-val body... (break))
  (test-val body... (break))
  (default body...))
```

**What it produces**: `SwitchStatement` with `SwitchCase` entries.

Each child list after the discriminant is a case clause. The first element of each child is the test expression (or the atom `default`), the rest are body statements.

```js
'switch'(args) {
  if (args.length < 2) {
    throw new Error('switch requires a discriminant and at least one case');
  }

  const discriminant = compileExpr(args[0]);
  const cases = args.slice(1).map(caseNode => {
    if (caseNode.type !== 'list' || caseNode.values.length === 0) {
      throw new Error('switch: each case must be a list (test body...)');
    }

    const headNode = caseNode.values[0];
    const isDefault = headNode.type === 'atom' && headNode.value === 'default';
    const test = isDefault ? null : compileExpr(headNode);
    const consequent = caseNode.values.slice(isDefault ? 1 : 1)
      .map(e => toStatement(compileExpr(e)));

    return {
      type: 'SwitchCase',
      test,
      consequent,
    };
  });

  return {
    type: 'SwitchStatement',
    discriminant,
    cases,
  };
},
```

**Compiler pitfall — `default` is NOT a separate macro**:

`default` inside a switch case is detected by checking if the head atom of a case list has the value `"default"`. It's not a standalone form — don't add it to `macros`.

**Compiler pitfall — fallthrough**:

Switch cases in JS fall through unless there's a `break`. lykn doesn't change this behavior — the programmer must include `(break)` explicitly. The compiler doesn't auto-insert breaks.

**Compiler pitfall — `SwitchCase.consequent` is an array, not a `BlockStatement`**:

Unlike `if` or `while` bodies which use `BlockStatement`, switch case bodies are flat arrays of statements. This is an ESTree quirk — cases don't create a new block scope (unless you explicitly wrap in `{}`). Match the spec exactly: `consequent` is `Statement[]`, not `{ type: 'BlockStatement', body: [...] }`.

---

## Group D: Operators (DD-08)

**Design doc**: `0008-dd-08-special-atoms-update-operators-and-miscellaneous-forms.md`

### 2.17 `?` — Ternary Conditional

**Key decision from DD-08**: The form name is `?`, NOT `?:`. Using `?:` would trigger colon splitting in the atom path. `?` is safe because the reader treats it as part of an atom.

**Syntax**: `(? test consequent alternate)`

```js
'?'(args) {
  if (args.length !== 3) {
    throw new Error('? (ternary) requires exactly 3 arguments: (? test then else)');
  }
  return {
    type: 'ConditionalExpression',
    test: compileExpr(args[0]),
    consequent: compileExpr(args[1]),
    alternate: compileExpr(args[2]),
  };
},
```

**Compiler pitfall — `?` is an expression, `if` is a statement**:

`(? test a b)` → `ConditionalExpression` (can be used as a value).
`(if test a b)` → `IfStatement` (cannot be used as a value).

Both exist and serve different purposes. This is the JS distinction between `x > 0 ? "yes" : "no"` and `if (x > 0) { ... } else { ... }`.

**Compiler pitfall — reader must accept `?` in atoms**:

Check that the reader doesn't stop on `?`. Looking at `reader.js`, the `readAtomOrNumber` function breaks on ` `, `\t`, `\n`, `\r`, `(`, `)`, and `;`. It does NOT break on `?`, so `?` passes through as part of an atom. You're good.

---

### 2.18 `++` / `--` — Prefix Update Operators

**Key decision from DD-08**: Prefix only for v0.1.0. Postfix deferred to v0.2.0.

```js
'++'(args) {
  if (args.length !== 1) {
    throw new Error('++ takes exactly one argument');
  }
  return {
    type: 'UpdateExpression',
    operator: '++',
    argument: compileExpr(args[0]),
    prefix: true,
  };
},

'--'(args) {
  if (args.length !== 1) {
    throw new Error('-- takes exactly one argument');
  }
  return {
    type: 'UpdateExpression',
    operator: '--',
    argument: compileExpr(args[0]),
    prefix: true,
  };
},
```

**Compiler pitfall — `UpdateExpression` vs `UnaryExpression`**:

`++x` is NOT a `UnaryExpression` — it's an `UpdateExpression`. These are different ESTree node types. `UnaryExpression` is for `!x`, `typeof x`, `-x`, etc. `UpdateExpression` is specifically for `++` and `--`. If you put `++` in the unary operator bulk registration, you'll get the wrong node type and astring may not handle it correctly.

---

### 2.19 `**` — Exponentiation

Add `**` to the existing binary operator table.

Find the `binaryOps` array near the bottom of the existing code:

```js
const binaryOps = ['+', '-', '*', '/', '%', '===', '!==', '==', '!=',
                    '<', '>', '<=', '>=', '&&', '||', '??',
                    '&', '|', '^', '<<', '>>', '>>>'];
```

Add `'**'` to this array. That's it — the existing bulk registration loop handles the rest.

**BUT WAIT** — check the operator classification. `**` is a `BinaryExpression` operator, not a `LogicalExpression`. The existing loop checks for `&&`, `||`, `??` to determine the type. `**` is not in that list, so it correctly falls through to `BinaryExpression`. No changes to the loop needed.

---

### 2.20 All Compound Assignment Operators

These are similar to the plain assignment `=` but with a different operator.

**Registration** — add this block near the existing binary/unary operator registrations:

```js
// Compound assignment operators
const compoundAssignOps = [
  '+=', '-=', '*=', '/=', '%=', '**=',
  '<<=', '>>=', '>>>=',
  '&=', '|=', '^=',
  '&&=', '||=', '??=',
];
for (const op of compoundAssignOps) {
  macros[op] = (args) => {
    if (args.length !== 2) {
      throw new Error(op + ' takes exactly 2 arguments');
    }
    return {
      type: 'AssignmentExpression',
      operator: op,
      left: compileExpr(args[0]),
      right: compileExpr(args[1]),
    };
  };
}
```

**Compiler pitfall — compound assignments are NOT binary expressions**:

`(+= x 1)` → `AssignmentExpression`, NOT `BinaryExpression`. The distinction matters: `x += 1` modifies `x`, while `x + 1` doesn't. ESTree models these as different node types.

**Compiler pitfall — the existing `=` macro**:

Plain `=` is already implemented as `AssignmentExpression` with operator `'='`. The compound assignment operators are the same ESTree node type, just with different operators. They coexist without conflict.

---

## Group E: Miscellaneous (DD-08)

### 2.21 `label` — Labeled Statements

**Syntax**: `(label name body)`

```js
'label'(args) {
  if (args.length !== 2) {
    throw new Error('label requires a name and body: (label name body)');
  }
  return {
    type: 'LabeledStatement',
    label: { type: 'Identifier', name: toCamelCase(args[0].value) },
    body: toStatement(compileExpr(args[1])),
  };
},
```

**Note**: Label name gets camelCased. `(label my-loop ...)` → `myLoop: ...`.

---

### 2.22 `debugger`

**Syntax**: `(debugger)`

```js
'debugger'(args) {
  if (args.length !== 0) {
    throw new Error('debugger takes no arguments');
  }
  return {
    type: 'DebuggerStatement',
  };
},
```

---

### 2.23 `seq` — Sequence Expression

**Syntax**: `(seq expr1 expr2 ...)`

```js
'seq'(args) {
  if (args.length < 2) {
    throw new Error('seq requires at least 2 expressions');
  }
  return {
    type: 'SequenceExpression',
    expressions: args.map(compileExpr),
  };
},
```

---

### 2.24 `regex` — Regular Expression Literals

**Syntax**: `(regex pattern)` or `(regex pattern flags)`

```js
'regex'(args) {
  if (args.length < 1 || args.length > 2) {
    throw new Error('regex takes 1 or 2 arguments: (regex pattern) or (regex pattern flags)');
  }
  if (args[0].type !== 'string') {
    throw new Error('regex pattern must be a string');
  }
  const pattern = args[0].value;
  const flags = args.length === 2
    ? (args[1].type === 'string' ? args[1].value : String(args[1].value))
    : '';

  return {
    type: 'Literal',
    value: null,  // Some tools expect this
    regex: { pattern, flags },
  };
},
```

**Compiler pitfall — `Literal` with `regex` property**:

Regex literals in ESTree are a special case of `Literal`. The `value` field can be `null` (or a RegExp instance). The `regex` property holds the pattern and flags as strings. astring reads the `regex` property to generate `/pattern/flags`. Make sure you include the `regex` field — without it, astring will try to generate the `value` and produce `null` in the output.

---

## Tests (2.25)

### File Organization

Create one test file per form (or per logical group):

```
test/
  forms/
    function.test.js
    async-await.test.js
    import.test.js
    export.test.js
    dynamic-import.test.js
    throw.test.js
    try-catch.test.js
    while.test.js
    do-while.test.js
    for.test.js
    for-of.test.js
    for-in.test.js
    break-continue.test.js
    switch.test.js
    ternary.test.js
    update-operators.test.js
    compound-assignment.test.js
    label.test.js
    debugger.test.js
    seq.test.js
    regex.test.js
```

### Test Pattern (Same as Phase 1)

```js
import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}
```

### Key Test Cases Per Form

Here are the tests that exercise critical behavior and catch likely bugs:

**`function.test.js`**:
```js
Deno.test("function: basic declaration", () => {
  assertEquals(lykn('(function add (a b) (return (+ a b)))'),
    'function add(a, b) {\n  return a + b;\n}');
});

Deno.test("function: camelCase name", () => {
  assertEquals(lykn('(function my-handler (req) (return req))'),
    'function myHandler(req) {\n  return req;\n}');
});

Deno.test("function: no params", () => {
  assertEquals(lykn('(function init () (return 42))'),
    'function init() {\n  return 42;\n}');
});

Deno.test("function: multi-statement body", () => {
  // Verify multiple body expressions become multiple statements
  const result = lykn('(function setup () (const x 1) (const y 2) (return (+ x y)))');
  // Check it contains all three statements
  assertEquals(result.includes('const x = 1;'), true);
  assertEquals(result.includes('const y = 2;'), true);
  assertEquals(result.includes('return x + y;'), true);
});

Deno.test("function: missing name throws", () => {
  assertThrows(() => lykn('(function (a b) (return 1))'));
});
```

**`async-await.test.js`**:
```js
Deno.test("async: wraps function declaration", () => {
  const result = lykn('(async (function fetch-data () (return 1)))');
  assertEquals(result.startsWith('async function fetchData'), true);
});

Deno.test("async: wraps lambda", () => {
  const result = lykn('(const f (async (lambda () (return 1))))');
  assertEquals(result.includes('async function'), true);
});

Deno.test("async: wraps arrow", () => {
  const result = lykn('(const f (async (=> () 1)))');
  assertEquals(result.includes('async'), true);
});

Deno.test("async: rejects non-function", () => {
  assertThrows(() => lykn('(async 42)'));
});

Deno.test("await: basic", () => {
  assertEquals(lykn('(const data (await (fetch url)))'),
    'const data = await fetch(url);');
});
```

**`import.test.js`**:
```js
Deno.test("import: side-effect", () => {
  assertEquals(lykn('(import "mod")'), 'import "mod";');
});

Deno.test("import: default", () => {
  assertEquals(lykn('(import "express" express)'), 'import express from "express";');
});

Deno.test("import: named", () => {
  assertEquals(lykn('(import "fs" (read-file write-file))'),
    'import {readFile, writeFile} from "fs";');
});

Deno.test("import: named with alias", () => {
  const result = lykn('(import "mod" ((alias foo bar)))');
  assertEquals(result.includes('foo as bar'), true);
});

Deno.test("import: default + named", () => {
  const result = lykn('(import "react" React (use-state use-effect))');
  assertEquals(result.includes('React'), true);
  assertEquals(result.includes('useState'), true);
});

Deno.test("import: camelCase on names not paths", () => {
  const result = lykn('(import "node:fs" (read-file-sync))');
  assertEquals(result.includes('readFileSync'), true);
  assertEquals(result.includes('"node:fs"'), true);
});
```

**`try-catch.test.js`**:
```js
Deno.test("try: catch only", () => {
  const result = lykn('(try (do-something) (catch e (handle e)))');
  assertEquals(result.includes('try'), true);
  assertEquals(result.includes('catch'), true);
});

Deno.test("try: finally only", () => {
  const result = lykn('(try (do-something) (finally (cleanup)))');
  assertEquals(result.includes('finally'), true);
});

Deno.test("try: catch + finally", () => {
  const result = lykn('(try (do-something) (catch e (handle e)) (finally (cleanup)))');
  assertEquals(result.includes('catch'), true);
  assertEquals(result.includes('finally'), true);
});

Deno.test("try: no catch or finally throws", () => {
  assertThrows(() => lykn('(try (do-something))'));
});
```

**`switch.test.js`**:
```js
Deno.test("switch: basic", () => {
  const result = lykn('(switch x ("a" (do-a) (break)) ("b" (do-b) (break)) (default (do-default)))');
  assertEquals(result.includes('switch'), true);
  assertEquals(result.includes('case "a"'), true);
  assertEquals(result.includes('default:'), true);
});
```

**`regex.test.js`**:
```js
Deno.test("regex: pattern only", () => {
  assertEquals(lykn('(regex "^hello")'), '/^hello/;');
});

Deno.test("regex: pattern + flags", () => {
  assertEquals(lykn('(regex "^hello" "gi")'), '/^hello/gi;');
});
```

### Running All Tests

```sh
deno test test/
```

### Important Note on Expected Output

astring controls exact formatting. Run each test, see what astring actually produces, and match it. Common surprises:

- astring may or may not put spaces inside `{ }` for imports: `{readFile}` vs `{ readFile }`
- astring may use `var` keyword placement you don't expect
- Statement termination (semicolons) is consistent but worth verifying
- Block indentation uses the `indent: '  '` option from `compile()`

If a test fails on whitespace, check astring's actual output and adjust expectations to match.

---

## Summary of All Changes to `compiler.js`

| What | Where | Notes |
|------|-------|-------|
| `buildImportSpecifier()` function | Module level, near `toCamelCase` | Helper for import |
| `buildExportNames()` function | Module level, near above | Helper for export |
| `macros['function']` | In `macros` object | New |
| `macros['async']` | In `macros` object | New |
| `macros['await']` | In `macros` object | New |
| `macros['import']` | In `macros` object | New |
| `macros['export']` | In `macros` object | New |
| `macros['dynamic-import']` | In `macros` object | New |
| `macros['throw']` | In `macros` object | New |
| `macros['try']` | In `macros` object | New |
| `macros['while']` | In `macros` object | New |
| `macros['do-while']` | In `macros` object | New |
| `macros['for']` | In `macros` object | New |
| `macros['for-of']` | In `macros` object | New |
| `macros['for-in']` | In `macros` object | New |
| `macros['break']` | In `macros` object | New |
| `macros['continue']` | In `macros` object | New |
| `macros['switch']` | In `macros` object | New |
| `macros['?']` | In `macros` object | New |
| `macros['++']` | In `macros` object | New |
| `macros['--']` | In `macros` object | New |
| `'**'` added to `binaryOps` | In existing array | One string added |
| Compound assignment loop | After existing operator loops | New registration block |
| `macros['label']` | In `macros` object | New |
| `macros['debugger']` | In `macros` object | New |
| `macros['seq']` | In `macros` object | New |
| `macros['regex']` | In `macros` object | New |
| `async: false` on `function` | In new macro | Explicit flag |
| `async: false` on `lambda` | In existing macro | Add if missing |
| `async: false` on `=>` | In existing macro | Add if missing |

### Files Changed

| File | Action |
|------|--------|
| `src/compiler.js` | Add 24 macros, 2 helpers, 1 operator, 1 registration block; update `lambda`/`=>` with `async: false` |
| `test/forms/*.test.js` | ~20 new test files |

### What NOT to Do

- **Do not modify `src/reader.js`.** The reader is unchanged for all of v0.1.0.
- **Do not modify the `object` macro.** Phase 3 changes it to grouped pairs.
- **Do not implement destructuring.** That's Phase 4. `for-of` and `for-in` work with simple atom bindings for now.
- **Do not implement `class`.** That's Phase 5. `export` can declare `(export (class ...))` but only after Phase 5 adds the `class` macro.
- **Do not add `alias` to `macros`.** It's a structural pattern recognized inside `import`/`export`, not a standalone form.
- **Do not add `default` to `macros`.** Inside `switch`, it's a recognized atom. Inside `export`, it's a recognized atom. Neither is a standalone macro.

---

## Verification Checklist

When you're done, confirm:

- [ ] `(function add (a b) (return (+ a b)))` compiles to `function add(a, b) { return a + b; }`
- [ ] `(async (function fetch () (return (await (get-data)))))` compiles to `async function fetch() { ... }`
- [ ] `(import "node:fs" (read-file-sync))` compiles with `readFileSync` and `"node:fs"`
- [ ] `(export (const x 42))` compiles to `export const x = 42;`
- [ ] `(export default my-fn)` compiles to `export default myFn;`
- [ ] `(try (x) (catch e (y)) (finally (z)))` compiles with all three blocks
- [ ] `(for-of item items (console:log item))` compiles to `for (const item of items) { ... }`
- [ ] `(switch x ("a" (f) (break)) (default (g)))` compiles with cases
- [ ] `(? (> x 0) "yes" "no")` compiles to `x > 0 ? "yes" : "no"`
- [ ] `(++ x)` compiles to `++x`
- [ ] `(+= x 1)` compiles to `x += 1`
- [ ] `(**= x 2)` compiles to `x **= 2`
- [ ] `(regex "pattern" "gi")` compiles to `/pattern/gi`
- [ ] `(debugger)` compiles to `debugger;`
- [ ] `(do-while (> x 0) (-= x 1))` compiles to `do { x -= 1; } while (x > 0)`
- [ ] `(dynamic-import "./mod.js")` compiles to `import("./mod.js")`
- [ ] `deno test test/` passes all tests
- [ ] `deno lint src/` passes
