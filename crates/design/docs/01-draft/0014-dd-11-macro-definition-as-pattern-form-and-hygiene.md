---
number: 14
title: "DD-11: Macro Definition, `as` Pattern Form, and Hygiene"
author: "the
expansion"
component: All
tags: [change-me]
created: 2026-03-26
updated: 2026-03-26
state: Draft
supersedes: null
superseded-by: null
version: 1.0
---

# DD-11: Macro Definition, `as` Pattern Form, and Hygiene

**Status**: Decided
**Date**: 2026-03-26
**Session**: v0.2.0 macro system design — topic 2 of 5

## Summary

Macros are defined with `macro` (not `defmacro`), following the naming
convention established by `function` (not `defun`). Macro parameter
lists use CL-heritage destructuring, reusing DD-06's existing pattern
forms. A new universal pattern form `as` is introduced for
whole-and-destructure bindings; it subsumes `alias` as the user-facing
form across imports, destructuring, function params, and macro params.
`alias` is retained as a core form that `as` desugars to. Hygiene
follows Fennel's enforced gensym model with `#gen` suffix for
auto-gensym, `(gensym)` for programmatic use, and `(sym)` as the
intentional-capture escape hatch. Compile-time evaluation uses
`new Function()` with a sandboxed macro environment API.

## Decisions

### `macro` form syntax

**Decision**: Macros are defined with `(macro name params body...)`.
The macro receives the call-site *arguments* (not the whole form
including the macro name), following CL convention. The macro body
is lykn code that returns an s-expression.

**Syntax**:

```lisp
;; Definition
(macro when (test (rest body))
  `(if ,test (do ,@body)))

;; Call site
(when (> x 0)
  (console:log "positive")
  (console:log x))

;; Expansion
(if (> x 0) (do (console:log "positive") (console:log x)))
```

```javascript
// Compiled output (what astring emits)
if (x > 0) {
  console.log("positive");
  console.log(x);
}
```

**ESTree nodes**: None — `macro` is an expansion-time form. It
produces no ESTree nodes and is erased from output. The expanded
core forms produce ESTree nodes per DD-01 through DD-09.

**Rationale**: `macro` is consistent with `function` (not `defun`),
`lambda` (not `fn`), and `class` (not `defclass`). CL-heritage
argument-only binding (not whole-form) is the clearest and most
natural parameter convention, with `as` available for whole-form
access when needed.

### CL-heritage destructuring parameter lists

**Decision**: Macro parameter lists use the same destructuring
forms as DD-06: `rest`, `default`, `object`, `array`, `as`, and
`_` skip. The parameter list is a destructuring pattern applied
to the call-site arguments.

**Syntax**:

```lisp
;; Positional + rest
(macro when (test (rest body))
  `(if ,test (do ,@body)))
;; test binds to first arg, body binds to remaining args

;; With defaults
(macro my-assert (test (default msg "assertion failed"))
  `(if (not ,test) (throw (new Error ,msg))))

;; Nested destructuring — macro takes structured args
(macro with-element ((id (default tag "div")) (rest body))
  `(let ((el (document:create-element ,tag)))
     (= el:id ,id)
     ,@body))

;; Usage: (with-element ("main" "section") ...)

;; Skip unused args
(macro third (_ _ x)
  x)
```

**ESTree nodes**: None — expansion-time only.

**Rationale**: Reusing DD-06's destructuring avoids inventing a
separate macro-specific parameter syntax. JS's native destructuring
(rest parameters, defaults, array/object patterns) maps naturally
to what macro parameter lists need. CL's full nested destructuring
in macro lambda lists is powerful for macros that accept structured
forms.

### `as` universal pattern form

**Decision**: `(as source target)` is a new expansion-time pattern
form meaning "source as target." It works everywhere destructuring
works: bindings (`const`, `let`, `var`), function parameters, macro
parameters, `for-of` bindings, and imports. `as` is resolved by the
expansion pass before the compiler sees it.

The two arguments are always `(as source target)` where the
meaning of `source` and `target` depends on context:

| Context | `source` | `target` | Meaning |
|---------|----------|----------|---------|
| Object pattern | Key name | Local name or pattern | Rename / deeper destructure |
| Binding position | Whole-value name | Pattern | Bind whole + destructure |
| Import | Imported name | Local name | Import rename |
| Macro params | Whole-args name | Pattern | Bind whole form + destructure args |

**Syntax — object key rename** (replaces `alias` in user code):

```lisp
(const (object (as payload p)) response)
```

```javascript
const { payload: p } = response;
```

**Syntax — object key rename with default** (composes with `default`):

```lisp
(const (object (as payload (default p 0))) response)
```

```javascript
const { payload: p = 0 } = response;
```

**Syntax — object key with deeper destructuring**:

```lisp
(const (object (as payload (array x y))) response)
```

```javascript
const { payload: [x, y] } = response;
```

**Syntax — whole-and-destructure in bindings**:

```lisp
(const (as whole (array first second (rest tail))) items)
```

```javascript
const whole = items;
const [first, second, ...tail] = whole;
```

**Syntax — whole-and-destructure in function params**:

```lisp
(function process ((as original (object name age)))
  (console:log "processing" original)
  (use name age))
```

```javascript
function process(original) {
  const { name, age } = original;
  console.log("processing", original);
  use(name, age);
}
```

**Syntax — whole-and-destructure in macro params**:

```lisp
(macro my-mac ((as form (test (rest body))))
  ;; form = entire argument list as a single AST node
  ;; test, body = destructured pieces
  ...)
```

**Syntax — import rename** (replaces `alias` in imports):

```lisp
(import "./utils.js" (as foo-bar baz))
```

```javascript
import { fooBar as baz } from "./utils.js";
```

**Syntax — `for-of` with whole-and-destructure**:

```lisp
(for-of (as entry (array key value)) (my-map:entries))
  (console:log entry key value))
```

```javascript
for (const entry of myMap.entries()) {
  const [key, value] = entry;
  console.log(entry, key, value);
}
```

**Syntax — nested `as`**:

```lisp
(const (object (as payload (as inner (array x y)))) msg)
```

```javascript
const { payload: inner } = msg;
const [x, y] = inner;
```

**ESTree nodes**: `as` produces no ESTree nodes of its own. In
object pattern context, it desugars to the core form `alias` which
produces `Property` nodes with different key/value `Identifier`s.
In whole-and-destructure context, it desugars to two binding
statements (one plain, one destructured).

**Rationale**: `as` unifies four previously distinct concepts —
object key rename, import rename, whole-value binding, and macro
whole-form access — into a single form with consistent syntax.
It reads as natural English ("payload as p", "items as whole").
It composes cleanly with other pattern forms (`default`, `rest`,
`array`, `object`). It eliminates the need for a separate `whole`
concept in macro parameters. The DD-06 three-arg
`(alias key local default)` is replaced by the more composable
`(as key (default local value))`.

### `alias` demoted to core form

**Decision**: `alias` remains as an internal core form that the
compiler understands. `as` is the user-facing form that desugars
to `alias` where appropriate (object key rename). Users rarely
write `alias` directly. The compiler's handling of `alias` is
unchanged from DD-06.

**Rationale**: The compiler already knows how to emit
`{ key: localName }` from `alias`. Rather than teaching the
compiler a new form, `as` desugars to `alias` in object contexts,
keeping the compiler simple. In non-object contexts
(whole-and-destructure), `as` desugars to two bindings — no core
form change needed.

### Expansion pass as pattern-and-macro expander

**Decision**: The expansion pass (inserted between reader and
compiler in v0.2.0) handles both macro expansion and pattern
desugaring. It resolves `as`, quasiquote, macros, and any future
pattern forms into core forms before the compiler sees them. The
compiler remains a thin s-expression-to-ESTree translator that
only understands DD-01 through DD-09 core forms plus `alias`.

**Rationale**: Pattern desugaring and macro expansion are both
s-expression → s-expression transformations. They belong in the
same pass. This keeps the compiler simple and makes the expansion
pass the single place where all "smarter than JS" features live.
Future pattern features (e.g., `match`/`case` expressions, guard
clauses) would also live here.

### Enforced gensym

**Decision**: Inside a quasiquote template, bare symbols in binding
position that are not macro parameters, known core form names, or
created via `#gen`/`gensym`/`sym` trigger a compile error. This
prevents accidental variable capture.

**Syntax**:

```lisp
;; REJECTED by enforced gensym check — bare `temp`
(macro swap (a b)
  `(let ((temp ,a))
     (= ,a ,b)
     (= ,b temp)))
;; Error: bare symbol `temp` in binding position inside quasiquote.
;; Use temp#gen for auto-gensym, (gensym "temp") for manual,
;; or (sym "temp") for intentional capture.

;; ACCEPTED — auto-gensym
(macro swap (a b)
  `(let ((temp#gen ,a))
     (= ,a ,b)
     (= ,b temp#gen)))
```

**Rationale**: Fennel's enforced gensym model (since v0.3.0) catches
the most common macro hygiene bug — accidental name capture — at
compile time rather than producing mysterious runtime bugs. The error
message guides the user to the three available solutions.

### Auto-gensym via `#gen` suffix

**Decision**: The `#gen` suffix on a symbol inside a quasiquote
template triggers automatic gensym generation. The prefix carries
semantic meaning. All occurrences of the same `name#gen` within one
quasiquote template resolve to the same generated symbol. Different
prefixes generate different symbols. The expanded name follows the
pattern `name__gensymN` where N is a monotonic counter.

**Syntax**:

```lisp
;; Definition with auto-gensym
(macro swap (a b)
  `(let ((temp#gen ,a))
     (= ,a ,b)
     (= ,b temp#gen)))

;; Multiple distinct gensyms in one macro
(macro some-mac (expr)
  `(let ((temp#gen ,expr)
         (result#gen (process temp#gen)))
     result#gen))
```

Expansion of `(some-mac (compute x))`:

```lisp
(let ((temp__gensym0 (compute x))
      (result__gensym1 (process temp__gensym0)))
  result__gensym1)
```

```javascript
// Compiled output
{
  let temp__gensym0 = compute(x);
  let result__gensym1 = process(temp__gensym0);
}
```

**ESTree nodes**: None — `#gen` is resolved during expansion. The
generated `name__gensymN` symbols are ordinary identifiers by the
time the compiler sees them.

**Rationale**: The `#gen` suffix is explicit and instantly
recognizable in both source and debug output. The `name__gensymN`
expansion format is verbose by design — "flashing neon signs" that
make gensym-generated names unmistakable in `macroexpand` output.
The prefix preserves semantic intent (`temp`, `result`, etc.) while
the `__gensymN` suffix guarantees uniqueness. Bare `#` suffix
(ClojureScript/Fennel convention) is reserved for potential future
use.

### `(gensym)` for programmatic use

**Decision**: `(gensym "prefix")` is available in the macro
environment API for programmatic symbol generation. It returns a
symbol node with the name `prefix__gensymN`. The counter is shared
with the `#gen` suffix mechanism. `(gensym)` without a prefix
argument uses `"g"` as the default prefix.

**Syntax**:

```lisp
;; Programmatic gensym — useful when building AST nodes by hand
(macro make-bindings ((rest names))
  (let ((bindings (names:map
                    (=> (n) (list (gensym "tmp") n)))))
    `(let (,@bindings) ...)))
```

**Rationale**: `#gen` covers the common case (quasiquote templates).
`(gensym)` covers the programmatic case where macro bodies construct
AST nodes dynamically via the `list`/`sym` API. Both use the same
counter, producing the same `name__gensymN` format.

### `sym` escape hatch for intentional capture

**Decision**: `(sym "name")` creates a symbol node with the exact
given name, bypassing the enforced gensym check. This is the escape
hatch for macros that intentionally introduce named bindings
(anaphoric macros, macros that define specific variables).

**Syntax**:

```lisp
;; Anaphoric if — intentionally captures `it`
(macro aif (test then else)
  `(let (((sym "it") ,test))
     (if (sym "it") ,then ,else)))

;; Usage
(aif (find-user id)
  (console:log it:name)
  (console:log "not found"))

;; Expansion
(let ((it (find-user id)))
  (if it (console:log it:name) (console:log "not found")))
```

```javascript
{
  let it = findUser(id);
  if (it) {
    console.log(it.name);
  } else {
    console.log("not found");
  }
}
```

**Rationale**: Intentional capture is a legitimate macro technique
(anaphoric macros, binding macros). `(sym "name")` makes the capture
visible and deliberate — anyone reading the macro source sees the
explicit opt-out of hygiene. This follows Fennel's philosophy: make
the unsafe thing possible but loud.

### Compile-time evaluation via `new Function()`

**Decision**: When the expansion pass encounters a `macro` form, it:

1. Compiles the macro body from lykn s-expressions to JavaScript
   using the same compiler that handles regular code
2. Wraps the compiled JS in `new Function()` with the macro
   environment API functions as parameters
3. Stores the resulting function in the macro environment
4. Invokes the function each time the macro is called, passing
   the call-site arguments as s-expression AST nodes

```javascript
// What the expander does internally for:
// (macro when (test (rest body)) `(if ,test (do ,@body)))

const macroFn = new Function(
  // Macro environment API — the sandbox boundary
  "list", "sym", "gensym",
  "isList", "isSymbol", "isNumber", "isString",
  "first", "rest", "concat", "nth", "length",
  // Compiled macro body (returns s-expression builder)
  compiledBodyString
);

// Bind the API functions
const boundMacro = (...args) => macroFn.call(
  null,
  list, sym, gensym,
  isList, isSymbol, isNumber, isString,
  first, rest, concat, nth, length
)(...args);

macroEnv.set("when", boundMacro);
```

**Available at compile time**:

| Available | Examples |
|-----------|----------|
| Macro environment API | `list`, `sym`, `gensym`, `first`, `rest`, `concat`, `nth`, `length`, type predicates |
| Earlier macros in same file | Macros defined above the current `macro` form |
| JS built-ins | `Math`, `Array`, `String`, `Object`, `JSON`, etc. |

**Not available at compile time** (deferred to v0.3.0 `import-macros`):

| Not available | Reason |
|---------------|--------|
| Runtime imports | Requires `import-macros` (DD-14) |
| File system access | Sandboxing — macros are pure transformations |
| Compiler internals | Isolation via `new Function()` parameter list |

**Rationale**: `new Function()` is synchronous, requires no
Deno-specific APIs, and provides natural isolation — the macro body
can only access what is explicitly passed as parameters. The macro
environment API is the sandbox boundary. Macro bodies are compiled
by the same compiler that handles regular code, so all core forms
(`let`, `if`, `do`, loops, etc.) are available in macro bodies.

### Quasiquote in macro bodies

**Decision**: Quasiquote inside macro bodies is compiled into calls
to the macro environment API functions (`list`, `sym`, `append`,
etc.). The quasiquote is *not* resolved when the macro is defined —
it is compiled into code that *constructs* an s-expression when the
macro is invoked.

```lisp
;; This macro definition...
(macro when (test (rest body))
  `(if ,test (do ,@body)))

;; ...compiles the body to approximately:
;; function(test, ...body) {
;;   return list(sym("if"), test, concat(list(sym("do")), body));
;; }
```

**Rationale**: This is the standard Lisp macro compilation strategy.
The quasiquote template is compiled into AST-constructing code. Each
invocation of the macro evaluates that code with the actual arguments,
producing a fresh s-expression that is then further expanded and
compiled.

### Error reporting

**Decision**: If a macro function throws during expansion, the
expander catches the error and reports it with the macro call site
location (from source location metadata tracked per DD-10).

Format: `Error expanding macro \`name\` at file:line:col: <message>`

**Rationale**: Source location metadata is tracked by the reader and
preserved through expansion (DD-10). Showing the call site — not the
macro definition site — is what the user needs to find and fix the
problem in their code.

## Rejected Alternatives

### `defmacro` naming

**What**: Use `defmacro` following CL tradition.

**Why rejected**: lykn uses `function` not `defun`, `class` not
`defclass`. `macro` is consistent with this naming convention.
The `def-` prefix adds no information.

### `&`-prefixed lambda list keywords

**What**: Use `&rest`, `&body`, `&whole`, `&optional` as CL does.

**Why rejected**: Creates asymmetry with DD-06's core forms which
use structural wrappers: `(rest name)`, `(default name value)`.
Reusing the existing forms maintains one syntax throughout the
language. `&body` is synonymous with `&rest` in CL (indentation
hint only) — lykn has no editor protocol that uses the distinction.

### `&whole` as a separate concept

**What**: Introduce `whole` (or `&whole`) specifically for macro
parameter lists to bind the entire argument form.

**Why rejected**: The `as` pattern form subsumes this naturally:
`(macro my-mac ((as form (test (rest body)))) ...)`. No special
macro-only syntax needed.

### `x#` short suffix for auto-gensym

**What**: Use bare `#` suffix (ClojureScript/Fennel convention):
`temp#`, `result#`.

**Why rejected**: Reserving `x#` for potential future use. The
`#gen` suffix is more explicit, and since auto-gensym is almost
never typed by hand (and is primarily seen in `macroexpand` debug
output), brevity is less important than clarity.

### `gensym#` as a fixed single-token auto-gensym

**What**: Use `gensym#` as the only auto-gensym form, with no
user-specified prefix.

**Why rejected**: Cannot generate multiple distinct gensyms within
one macro. `temp#gen` vs `result#gen` produce distinct symbols
while preserving semantic meaning. A single `gensym#` token forces
fallback to manual `(gensym)` calls for any macro needing more
than one generated binding.

### `alias` as the user-facing form

**What**: Keep `alias` as the primary form users write for rename
and destructuring, add `whole` or `as` only for the new
whole-and-destructure case.

**Why rejected**: `as` unifies four concepts (object rename, import
rename, whole-and-destructure, macro whole-form) into one form. It
reads as natural English. It composes with `default` instead of
overloading `alias` arity. Having both `alias` and `as` as
user-facing forms with overlapping semantics creates confusion.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| Macro with no params | Empty param list | `(macro my-break () \`(break))` |
| Macro body with no quasiquote | Returns computed s-expression | Body uses `list`/`sym` API directly |
| Same `#gen` prefix used in nested quasiquotes | Each quasiquote level gets independent gensym scope | Prevents cross-template collision |
| `sym` with camelCase name | Name used as-is (no lisp-case conversion) | `(sym "innerHTML")` → `innerHTML` |
| `sym` with lisp-case name | DD-01 conversion still applies at compile time | `(sym "my-var")` → symbol `my-var` → compiles to `myVar` |
| `as` with two simple names in array pattern | First is whole, second is a one-element destructure | Unambiguous — context determines meaning |
| `as` in expression position (not pattern) | Expansion error | `as` is only valid in pattern/binding contexts |
| Macro expanding to another macro call | Outer expansion produces form, expander recurses | Standard fixed-point expansion per DD-10 research |
| Macro defined inside a macro expansion | Valid — sequential file processing applies | Expanded macro can contain `macro` forms |

## Dependencies

- **Depends on**: DD-01 through DD-09 (core forms), DD-06
  (destructuring — `rest`, `default`, `object`, `array`, `_`),
  DD-04 (imports — `alias` for rename), DD-10 (quasiquote —
  macro bodies use quasiquote to build templates, source location
  metadata for error reporting)
- **Affects**: DD-06 (adds `as` as expansion-time sugar over
  `alias` — no change to core form behavior), DD-04 (adds `as`
  as sugar over `alias` in imports — no change to core form
  behavior), DD-12 (`#` reader dispatch — reader must not
  conflict with `#gen` suffix), DD-13 (macro expansion
  pipeline — expansion algorithm must handle `macro` form
  registration, enforced gensym, `#gen` resolution, and `as`
  desugaring), DD-14 (macro modules — module macros use the
  same definition syntax)

## Open Questions

None.
