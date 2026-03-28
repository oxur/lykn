---
number: 17
title: "DD-14: Macro Modules and ESM Interaction"
author: "file path"
component: All
tags: [change-me]
created: 2026-03-26
updated: 2026-03-27
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-14: Macro Modules and ESM Interaction

**Status**: Decided
**Date**: 2026-03-26
**Session**: v0.2.0 macro system design, conversation 5

## Summary

Macro modules are regular `.lykn` files. The import site (`import-macros` vs `import`) determines how a file is used, not anything about the file itself. `import-macros` is compile-time only, erased from output, uses explicit binding lists (no import-all), and supports `as` renaming. Macro modules are compiled via the full three-pass pipeline, executed synchronously via `new Function()`, and cached by path + mtime. Cross-module macro composition is supported; circular dependencies are a hard error.

## Decisions

### `import-macros` syntax

**Decision**: `import-macros` follows DD-04's module-path-first convention with an explicit binding list. It is compile-time only and produces no JS output.

```lisp
;; Import specific macros
(import-macros "./control-flow.lykn" (unless when-let))
```

```javascript
// no output — erased
```

```lisp
;; Renaming with `as`
(import-macros "./control-flow.lykn" ((as unless my-unless) when-let))
```

```javascript
// no output — erased
```

The path points to the `.lykn` source file (the compiler needs to compile it). The binding list names the exported macros to import. Renaming uses `as` (DD-11), which desugars to `alias` in the macro environment registration.

**Rationale**: Consistent with DD-04 `import` syntax. Module-path-first is already established. Explicit binding list makes macro provenance visible at the import site.

### No import-all

**Decision**: `import-macros` requires a binding list. Import-all (no binding list) is not supported.

```lisp
;; Error: import-macros requires explicit binding list
(import-macros "./control-flow.lykn")
```

**Rationale**: DD-04 banned `export *` and namespace imports for runtime. The same reasoning applies: you want to know exactly which names in your file are macros and where they came from. Explicit imports make macro provenance grep-able and prevent silent name collisions.

### Relative paths only

**Decision**: `import-macros` paths must be relative (starting with `./` or `../`). Bare specifiers (package-style imports) are not supported in v0.2.0. File extension is required.

```lisp
;; OK
(import-macros "./macros/control-flow.lykn" (when unless))
(import-macros "../shared/macros.lykn" (with-bindings))

;; Error: bare specifier not supported
(import-macros "control-flow" (when unless))

;; Error: file extension required
(import-macros "./control-flow" (when unless))
```

**Rationale**: No magic resolution. Consistent with DD-04. Package-style macro imports can come in a future version when there's a real use case.

### Macro module format

**Decision**: A macro module is a regular `.lykn` file. There is no dedicated macro-only file format. The import site determines how a file is used — `import-macros` extracts exported macros, `import` extracts runtime exports.

A single file can export both macros and runtime functions:

```lisp
;; utils.lykn
(export (macro unless (test (rest body))
  `(if (not ,test) (do ,@body))))

(export (function format-name (first last)
  (template first " " last)))
```

```lisp
;; consumer.lykn

;; Gets the macro — compile-time, erased
(import-macros "./utils.lykn" (unless))

;; Gets the runtime function — appears in JS output
(import "./utils.js" (format-name))
```

```javascript
// compiled consumer.js
import { formatName } from "./utils.js";
```

Note the path difference: `import-macros` points to `.lykn` source; `import` points to `.js` output. The compiler doesn't need to know or care that both reference the same source file.

**Rationale**: No file-type specialization. The file is just a file; the import site carries the semantics. This is the simplest model and avoids inventing a new file category.

### Macro module compilation and execution

**Decision**: When the compiler encounters `import-macros` in Pass 0, it compiles the target file through the full three-pass pipeline, wraps the compiled JS in `new Function()`, executes it synchronously, and registers the exported macros.

Steps:

1. **Resolve path**, check cache (path + mtime)
2. **Read** the file through the reader → s-expressions
3. **Run the full three-pass pipeline** on the target file (recursive — the target may have its own `import-macros` and local macros)
4. **Compile** all expanded forms to JS. The compiled JS is wrapped as a module-pattern function: all forms compile into the function body, and a return statement exposes only the exported macros as an object.
5. **Execute** via `new Function()` with the macro environment API as parameters. Returns an object mapping macro names to functions.
6. **Register** the requested macros (from the binding list) in the importing file's macro environment.

```javascript
// What the compiler generates internally for a macro module:
const moduleFn = new Function(
  // Macro environment API — same sandbox as inline macros (DD-11)
  "array", "sym", "gensym",
  "isArray", "isSymbol", "isNumber", "isString",
  "first", "rest", "concat", "nth", "length",
  `
  // compiled helper functions, constants, etc.
  const makeBindings = (pairs) => { ... };

  // compiled macro functions
  const unless = (test, ...body) => { ... };
  const whenLet = (binding, ...body) => { ... };

  // only exported macros in the return object
  return { unless, whenLet };
  `
);

const macroExports = moduleFn(
  array, sym, gensym,
  isArray, isSymbol, isNumber, isString,
  first, rest, concat, nth, length
);

// Register only the macros named in the binding list
macroEnv.set("unless", macroExports.unless);
```

Internal helper functions, unexported macros, constants, and runtime forms all compile into the function body and are available to macro code at compile time, but do not leak to the importing file.

**Rationale**: `new Function()` keeps the entire pipeline synchronous — no async/await contamination in the compiler. The same sandbox boundary established in DD-11 applies: macro modules can only access what is explicitly passed as parameters (the macro environment API plus JS built-ins). Compiling everything in the file (not just macros) means the compiler doesn't need dependency analysis to determine which non-macro forms are "needed by macros."

### Synchronous pipeline

**Decision**: The entire expansion pipeline (Pass 0, Pass 1, Pass 2) is synchronous. `new Function()` is used for both inline macros (DD-11) and macro modules. `dynamic import()` with data URIs is not used in v0.2.0.

**Rationale**: Synchronous compilation is the norm for compiled languages. `new Function()` is sufficient for v0.2.0 — macros are pure s-expression transformations that need only the macro environment API and JS built-ins. If a future version needs macro modules to import runtime JS libraries at compile time, `dynamic import()` can be introduced then. Keeping the pipeline synchronous avoids the colored-function problem in the compiler.

### Caching

**Decision**: Compiled macro modules are cached by file path + mtime. If the source file hasn't changed since last compilation, the cached compiled module is reused.

For v0.2.0, only leaf modules (those with no `import-macros` of their own) benefit from caching. If a macro module itself uses `import-macros`, it is recompiled every time. Full transitive cache invalidation (tracking dependency edges and their mtimes) is deferred.

**Rationale**: Simple and correct. Macro modules are small and compile fast. Deep macro dependency chains will be rare in early use. Transitive cache invalidation can be added in v0.2.x when real-world usage data shows whether it matters.

### Cross-module macro composition

**Decision**: Macro modules can use `import-macros` themselves. This triggers recursive compilation — the importing compiler runs the full three-pass pipeline on each transitively imported module.

```lisp
;; basic-control.lykn
(export (macro when (test (rest body))
  `(if ,test (do ,@body))))

(export (macro unless (test (rest body))
  `(if (not ,test) (do ,@body))))
```

```lisp
;; advanced-control.lykn
(import-macros "./basic-control.lykn" (when))

(export (macro when-let (binding (rest body))
  `(let (,binding)
     (when ,(first binding) ,@body))))
```

```lisp
;; app.lykn
(import-macros "./advanced-control.lykn" (when-let))

(when-let ((const user (get-user id)))
  (console:log user))
```

**Rationale**: Macro composition is essential for building macro libraries. The recursive compilation model handles this naturally — each module is a self-contained compilation unit processed by the same pipeline.

### Circular macro module dependencies

**Decision**: Circular dependencies between macro modules are a hard error. Detection uses a compilation stack: when Pass 0 begins compiling a macro module, its path is pushed onto the stack. If a path is encountered that is already on the stack, a circular dependency error is raised.

```
Error: circular macro module dependency:
  ./a.lykn imports macros from ./b.lykn
  ./b.lykn imports macros from ./a.lykn
```

**Rationale**: Circular macro dependencies are unresolvable — macro A can't exist until macro B is compiled, and vice versa. The compilation stack gives a clear error message showing the full cycle. Consistent with DD-13's circular dependency detection for file-local macros.

### Shadowing of imported macros by file-local macros

**Decision**: Defining a file-local macro with the same name as an imported macro is a hard error.

```lisp
(import-macros "./control-flow.lykn" (unless))

;; Error: macro 'unless' already defined (imported from ./control-flow.lykn)
(macro unless (test (rest body))
  `(if (not ,test) (do ,@body)))
```

If you want to replace an imported macro, don't import it.

**Rationale**: Consistent with DD-13's decision on duplicate file-local macro names. Silent shadowing discards work and makes debugging harder. Explicit error catches the problem immediately.

## Rejected Alternatives

### `dynamic import()` with data URIs for macro modules

**What**: Use `dynamic import()` with data URIs to load compiled macro modules as ESM, as proposed in the research document.

**Why rejected**: Makes Pass 0 async, which contaminates the entire pipeline with async/await. `new Function()` achieves the same result synchronously. `dynamic import()` is only needed if macro modules need to import runtime JS libraries at compile time, which is not a v0.2.0 requirement.

### Import-all for `import-macros`

**What**: Allow `(import-macros "./macros.lykn")` without a binding list to import all exported macros.

**Why rejected**: Same reasoning as DD-04's ban on namespace imports. Explicit binding lists make macro provenance visible and grep-able. Prevents silent name collisions when a macro module adds new exports.

### Macro-only file format

**What**: Require a dedicated file format or convention for macro modules (e.g., `.macro.lykn` extension, or a file-level declaration).

**Why rejected**: Unnecessary specialization. The import site (`import-macros` vs `import`) already determines how a file is used. A regular `.lykn` file can export both macros and runtime functions without ambiguity.

### Transitive cache invalidation in v0.2.0

**What**: Track dependency edges in the cache so that when module B changes, all modules that transitively import macros from B are invalidated.

**Why rejected**: Premature optimization. Macro modules are small and compile fast. Deep dependency chains will be rare in early use. Leaf-only caching is sufficient for v0.2.0. Full transitive invalidation can be added when real-world data shows it matters.

### Bare specifiers for macro imports

**What**: Allow package-style imports like `(import-macros "my-macro-lib" (when unless))`.

**Why rejected**: No macro package ecosystem exists yet. Relative paths are sufficient for v0.2.0. Package-style imports introduce resolution complexity (where to look, version management) that isn't justified yet.

### Separate sandbox for macro modules vs inline macros

**What**: Give macro modules a richer API than inline macros (e.g., file system access, network).

**Why rejected**: Macros are pure s-expression transformations. The same sandbox boundary (macro environment API + JS built-ins) applies regardless of where the macro is defined. A richer API can be added later if specific use cases emerge.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `import-macros` of non-existent file | Hard error with path shown | `"macro module not found: ./missing.lykn"` |
| `import-macros` requesting non-exported macro | Hard error listing available exports | `"macro 'foo' not exported by ./macros.lykn (available: when, unless)"` |
| `import-macros` requesting non-macro export | Hard error | `"'format-name' is not a macro in ./utils.lykn"` |
| Macro module with syntax errors | Hard error with source location in the macro module | `"./macros.lykn:5:3: unexpected token"` |
| Macro module with circular dependency | Hard error showing full cycle | See "Circular macro module dependencies" above |
| Same macro module imported by multiple files | Cached — compiled once, reused | Cache keyed on resolved path + mtime |
| Same macro module imported twice in one file | Hard error (duplicate import) | `"duplicate import-macros for ./macros.lykn"` |
| `import-macros` with `as` renaming to existing name | Hard error (name collision) | `"macro 'my-unless' already defined"` |
| Mixed file: `import-macros` and `import` of same source | Independent operations — both succeed | `import-macros` gets macros from `.lykn`, `import` gets runtime from `.js` |
| Macro module has runtime-only code | Compiled and executed (available to macro bodies) but not exported | Helper functions, constants accessible inside `new Function()` body |
| Deeply nested macro module imports | Recursive compilation with cycle detection | Compilation stack tracks full chain |

## Dependencies

- **Depends on**: DD-04 (module syntax, path conventions, `alias`), DD-11 (macro definition, `as` pattern form, `new Function()` compile-time eval, macro environment API), DD-13 (three-pass pipeline, Pass 0 defined as `import-macros` processing)
- **Affects**: DD-11 (v1.2 amendment: adds macro module compilation as a second use of `new Function()`, extends "not available at compile time" to note that `import-macros` now provides cross-module access), DD-13 (confirms Pass 0 semantics and adds detail on recursive compilation, cycle detection, and caching)

## Open Questions

- [ ] Package-style bare specifiers for macro imports (deferred — no macro package ecosystem yet)
- [ ] Transitive cache invalidation for macro module dependency chains (deferred to v0.2.x)
- [ ] Whether macro modules should have access to a richer compile-time API beyond the macro environment (deferred — no use case yet)
- [ ] `let` form (not yet defined) — macro module local bindings will use whatever binding forms are available
