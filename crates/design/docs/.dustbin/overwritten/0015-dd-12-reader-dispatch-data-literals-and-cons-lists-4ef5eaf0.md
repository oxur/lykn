---
number: 15
title: "DD-12: Reader `#` Dispatch, Data Literals, and Cons Lists"
author: "nested two"
component: All
tags: [change-me]
created: 2026-03-26
updated: 2026-03-26
state: Overwritten
supersedes: null
superseded-by: null
version: 1.0
---

# DD-12: Reader `#` Dispatch, Data Literals, and Cons Lists

**Status**: Decided
**Date**: 2026-03-26
**Session**: v0.2.0 macro system design — topic 3 of 5

## Summary

The reader's `#` character (reserved since v0.1.0) triggers a fixed
dispatch table with five entries: expression comments (`#;`), array
literals (`#(...)`), object literals (`#s(...)`), arbitrary-radix
numerics (`#NNr`), and nestable block comments (`#|...|#`). Three
new expansion-time forms — `cons`, `list`, and `car`/`cdr`/`cadr`/`cddr` —
introduce classic Lisp cons-cell data structures backed by nested
two-element JS arrays. Dotted-pair syntax `(,head . ,tail)` enables
head/tail destructuring on cons lists via quasiquote patterns. The
internal AST representation remains flat JS arrays; cons lists are a
user-facing data structure only. The dispatch table is fixed and not
user-extensible.

## Decisions

### Fixed dispatch table

**Decision**: The `#` dispatch table is hardcoded in the reader.
Five entries are defined. No mechanism is provided for users to
extend the table. The reader recognizes `#` at the start of a
token (not mid-atom — `temp#gen` from DD-11 is a single atom
containing `#`, not a dispatch trigger).

| Dispatch | Meaning | Reader action |
|---|---|---|
| `#;` | Expression comment | Discard next form |
| `#(...)` | Array literal | Expand to `(array ...)` |
| `#s(...)` | Object literal | Expand to `(object ...)` |
| `#NNr` | Radix literal | Parse value in base NN, emit numeric literal |
| `#\|...\|#` | Nestable block comment | Discard contents, track nesting depth |

**Rationale**: A fixed table avoids the complexity of user-extensible
reader macros (Racket-level machinery). lykn's "dumb reader"
philosophy favors a small, predictable set of reader extensions.
The five entries cover the concrete needs identified: expression
comments (debugging), data literals (construction and pattern
matching), numeric bases (expressiveness), and nestable comments
(ergonomics). Additional entries can be added in future versions
without design changes — the dispatch mechanism is extensible
internally, just not user-facing.

### `#;` expression comment

**Decision**: `#;` causes the reader to read and discard the next
complete form. No AST node is produced. This works on any form —
atoms, lists, nested structures.

**Syntax**:

```lisp
;; Comment out a single expression
(do
  (console:log "before")
  #;(console:log "this is commented out")
  (console:log "after"))
```

```javascript
{
  console.log("before");
  console.log("after");
}
```

```lisp
;; Comment out an argument
(my-function arg1 #;arg2 arg3)
```

```javascript
myFunction(arg1, arg3);
```

**ESTree nodes**: None — the form is discarded before any
processing.

**Rationale**: Standard Scheme/Racket feature. Universally useful
for debugging — comment out a whole s-expression without manually
matching parentheses. Simpler than wrapping in `#|...|#` for
single expressions. `;` comments only work to end-of-line; `#;`
works structurally.

### `#(...)` array literal

**Decision**: `#(...)` is reader sugar that expands to `(array ...)`.
It follows all the same rules as the `array` core form — the reader
performs a mechanical expansion, and the compiler/expansion pass
handles the rest.

**Syntax — construction**:

```lisp
#(1 2 3)
;; reader expands to:
(array 1 2 3)
```

```javascript
[1, 2, 3]
```

**Syntax — quasiquote pattern (destructuring)**:

```lisp
(let ((`#(,first ,second ,third) my-array))
  (console:log first second third))
```

Expansion: the quasiquote pattern `#(...)` expands to an `array`
pattern which uses DD-06 array destructuring:

```javascript
const [first, second, third] = myArray;
console.log(first, second, third);
```

**ESTree nodes**: None of its own — expands to `(array ...)` which
produces `ArrayExpression` (construction) or `ArrayPattern`
(destructuring) per DD-06.

**Rationale**: The primary purpose is not brevity over `(array ...)`.
It is to provide a literal form that works naturally in quasiquote
templates for both construction and pattern matching — the
constructor-as-destructor pattern from DD-06 and DD-11, applied at
the reader level.

### `#s(...)` object literal

**Decision**: `#s(...)` is reader sugar that expands to `(object ...)`.
It uses the same grouped `(key value)` pair syntax established in
DD-06's amended object construction syntax.

**Syntax — construction**:

```lisp
#s((name "Duncan") (age 42))
;; reader expands to:
(object (name "Duncan") (age 42))
```

```javascript
({ name: "Duncan", age: 42 })
```

All `object` features work: bare atoms for shorthand, `spread`,
computed keys:

```lisp
#s(name (age 42) (spread defaults))
;; reader expands to:
(object name (age 42) (spread defaults))
```

```javascript
({ name, age: 42, ...defaults })
```

**Syntax — quasiquote pattern (destructuring)**:

```lisp
(let ((`#s((name ,name) (address ,address)) personal-data))
  (console:log name address))
```

Expansion: the quasiquote pattern `#s(...)` expands to an `object`
pattern which uses DD-06 object destructuring:

```javascript
const { name, address } = personalData;
console.log(name, address);
```

**Syntax — nested patterns**:

```lisp
(let ((`#s((users `#(,first ,second))) response))
  (console:log first second))
```

```javascript
const { users: [first, second] } = response;
console.log(first, second);
```

**ESTree nodes**: None of its own — expands to `(object ...)` which
produces `ObjectExpression` (construction) or `ObjectPattern`
(destructuring) per DD-06.

**Rationale**: Same as `#(...)` — provides a literal form for
quasiquote-based construction and pattern matching of JS objects.
Uses grouped `(key value)` pairs to match `object`'s established
syntax (DD-06 amendment), maintaining internal consistency. The
lowercase `s` follows lykn's preference for lowercase forms.

### `#NNr` radix literal

**Decision**: `#NNr` reads a numeric literal in an arbitrary base.
The reader sees `#`, reads digits to determine the base (2–36),
sees `r` as the base terminator, then reads the value characters
until the next delimiter (whitespace, paren, etc.). The reader
computes the numeric value and emits it as a numeric AST node.

For bases with native JS literal syntax, the compiler emits the
JS literal form. For all other bases, it emits the computed
decimal value.

| Input | Base | JS output |
|---|---|---|
| `#2r11111111` | 2 | `0b11111111` |
| `#8r377` | 8 | `0o377` |
| `#16rff` | 16 | `0xff` |
| `#3r201` | 3 | `19` |
| `#36rzz` | 36 | `1295` |

**Syntax**:

```lisp
(const mask #2r11110000)
(const permissions #8r755)
(const color #16rff8800)
(const big #36rzz)
```

```javascript
const mask = 0b11110000;
const permissions = 0o755;
const color = 0xff8800;
const big = 1295;
```

**ESTree nodes**: `Literal` with numeric value.

**Rationale**: Follows CL's `#NNr` syntax exactly. Extends JS's
built-in base support (2, 8, 16 only) to arbitrary bases 2–36,
matching `parseInt`'s range. Emitting computed values rather than
`parseInt()` calls keeps compiled output clean and dependency-free.
Using native JS literal forms for bases 2/8/16 preserves
readability — a reader seeing `0xff` in compiled output
immediately understands the intent.

### `#|...|#` nestable block comment

**Decision**: `#|` opens a block comment, `|#` closes it. Nesting
is tracked with a depth counter — inner `#|...|#` pairs are
allowed. The reader discards all content between balanced delimiters.

**Syntax**:

```lisp
#|
  This entire block is commented out.
  (console:log "not compiled")
  #|
    Nested comments work too.
    (console:log "also not compiled")
  |#
  (console:log "still commented")
|#
(console:log "this compiles")
```

```javascript
console.log("this compiles");
```

**ESTree nodes**: None — content is discarded at read time.

**Rationale**: JS's `/* ... */` does not nest — commenting out a
block that contains a `/* */` comment causes a parse error. CL's
`#|...|#` solves this cleanly. Extremely useful for temporarily
disabling large sections of code during development. Standard
feature in CL, Scheme (SRFI-30), and LFE.

### `cons` form

**Decision**: `(cons x y)` is expansion-time sugar that desugars
to `(array x y)`, producing a two-element JS array. This is a
classic Lisp cons cell represented as a JS array where index 0
is `car` and index 1 is `cdr`.

**Syntax**:

```lisp
(cons 1 2)
```

```javascript
[1, 2]
```

```lisp
;; Building a proper list
(cons 1 (cons 2 (cons 3 null)))
```

```javascript
[1, [2, [3, null]]]
```

**ESTree nodes**: None of its own — desugars to `(array x y)` →
`ArrayExpression`.

**Rationale**: Classic cons semantics using JS's native array type.
No runtime library needed — cons cells are just two-element arrays.
The representation is immediately recognizable to anyone from a
Lisp background while remaining plain JS data. `null` serves as
the natural nil terminator, matching JS convention.

### `list` form

**Decision**: `(list ...)` is expansion-time sugar that desugars
to nested `cons` calls terminated by `null`. It builds a proper
cons list.

**Syntax**:

```lisp
(list 1 2 3)
;; desugars to:
(cons 1 (cons 2 (cons 3 null)))
```

```javascript
[1, [2, [3, null]]]
```

```lisp
(list)
;; empty list
```

```javascript
null
```

**ESTree nodes**: None of its own — desugars through `cons` →
`(array x y)` → `ArrayExpression`. Empty `(list)` produces
`null` `Literal`.

**Rationale**: `list` is the standard Lisp function for building
proper lists from elements. The desugaring to nested `cons` is
mechanical and produces the expected nested-pair structure. An
empty list is `null`, consistent with the nil terminator convention.

**Important distinction**: `(list ...)` builds cons-cell data
structures (nested two-element arrays). `(array ...)` builds flat
JS arrays. These are different data structures for different
purposes. The user chooses which is appropriate for their use case.

### `car`, `cdr`, `cadr`, `cddr` accessors

**Decision**: These are expansion-time sugar that desugar to
indexed access via `get`:

| Form | Desugars to | JS output |
|---|---|---|
| `(car x)` | `(get x 0)` | `x[0]` |
| `(cdr x)` | `(get x 1)` | `x[1]` |
| `(cadr x)` | `(get (get x 1) 0)` | `x[1][0]` |
| `(cddr x)` | `(get (get x 1) 1)` | `x[1][1]` |

**Syntax**:

```lisp
(const my-list (list 10 20 30))
(console:log (car my-list))
(console:log (cadr my-list))
(console:log (cddr my-list))
```

```javascript
const myList = [10, [20, [30, null]]];
console.log(myList[0]);
console.log(myList[1][0]);
console.log(myList[1][1]);
```

Output: `10`, `20`, `[30, null]`

**ESTree nodes**: None of their own — desugar to `get` →
`MemberExpression` with computed property.

**Rationale**: The four classic accessors cover the most common
cons-list operations. Deeper compositions (`caddr`, `cdddr`, etc.)
can be expressed with nesting: `(car (cddr x))`. All four compile
to simple indexed access — no runtime function calls, no
dependencies.

### Dotted-pair pattern for cons list destructuring

**Decision**: Inside a quasiquote pattern, `(,head . ,tail)` is
the head/tail destructuring form for cons lists. The `.` (dot)
inside a list signals a cons pair — everything before the dot is
the `car` side, everything after is the `cdr` side.

The reader recognizes `.` as a special delimiter inside lists
(currently not special in the lykn reader since DD-01 removed
dots for member access). When the reader encounters `(a . b)`,
it produces a cons-pair AST node rather than a regular list node.

**Syntax — basic head/tail**:

```lisp
(let ((`(,head . ,tail) (list 1 2 3 4 5)))
  (console:log head)
  (console:log tail))
```

```javascript
const _list = [1, [2, [3, [4, [5, null]]]]];
const head = _list[0];
const tail = _list[1];
console.log(head);
console.log(tail);
```

Output: `1`, `[2, [3, [4, [5, null]]]]`

**Syntax — nested head/tail (implementing `cddr`)**:

```lisp
(let ((`(,first . (,second . ,rest)) (list 1 2 3 4)))
  (console:log first)
  (console:log second)
  (console:log rest))
```

```javascript
const _list = [1, [2, [3, [4, null]]]];
const first = _list[0];
const second = _list[1][0];
const rest = _list[1][1];
console.log(first);
console.log(second);
console.log(rest);
```

Output: `1`, `2`, `[3, [4, null]]`

**ESTree nodes**: None of its own — the pattern desugars to
indexed access (`MemberExpression` with computed property) in
binding statements.

**Rationale**: Dotted-pair destructuring is deeply familiar to
anyone from a Lisp background and is the natural complement to
`cons`/`list` construction — the constructor-as-destructor
pattern. It only applies to cons-list data structures (nested
two-element arrays), not flat JS arrays (which use DD-06's
`(array ... (rest ...))` destructuring). The dot syntax reuses
a character that was freed by DD-01's removal of dots for member
access.

### Internal AST representation unchanged

**Decision**: The reader, expansion pass, and compiler continue
to use flat JS arrays as the internal AST representation. The
macro environment API's `array` function (renamed from `list`
per DD-12 amendment to DD-10 and DD-11) constructs these flat
JS arrays. `cons`/`list` are user-facing data structure forms
only and do not affect compiler internals.

**Rationale**: Flat JS arrays are what the reader has always
produced. The pipeline — reader, expander, compiler — iterates
these with standard JS array operations. Changing the internal
representation to cons cells would add complexity throughout the
pipeline for no benefit, fighting against JS rather than leaning
into it. The "thin skin over JS" principle applies to the
compiler's own internals, not just its output.

## Rejected Alternatives

### `#t` / `#f` for booleans

**What**: Scheme-style boolean literals `#t` and `#f`.

**Why rejected**: JS already has `true` and `false` as keywords,
and lykn passes them through as atoms. Adding a second syntax for
the same values adds complexity without expressiveness. Violates
the "one syntax where possible" principle.

### User-extensible dispatch table

**What**: Allow users to register custom `#` dispatch characters
at read time.

**Why rejected**: User-extensible reader macros require
Racket-level machinery (reader language negotiation, phase
separation at the reader level). This contradicts lykn's "dumb
reader" philosophy. The fixed table can be extended in future
lykn versions if new entries are needed.

### `#js{...}` for JS object literals

**What**: A special `#js{...}` syntax using curly braces for
inline JS object construction.

**Why rejected**: Introduces curly braces into the reader, which
is otherwise paren-only for grouping. `#s(...)` achieves the same
goal using parentheses, consistent with lykn's s-expression
syntax. No justification for a second grouping character.

### Flat alternating key-value pairs for `#s`

**What**: Use `#s(name "Duncan" age 42)` with flat alternating
keys and values instead of grouped pairs.

**Why rejected**: DD-06 amended `object` to use grouped `(key value)`
pairs. `#s` expands to `(object ...)`, so it should use the same
grouping convention for internal consistency. Flat alternating
would create a divergence between `#s(...)` and `(object ...)`.

### `cons` as a core form

**What**: Have the compiler understand `cons` directly rather
than desugaring it.

**Why rejected**: `(cons x y)` is trivially `(array x y)`. Adding
a core form for something that desugars to an existing core form
adds compiler complexity for no benefit. The expansion pass handles
the desugaring; the compiler stays thin.

### Multiple-head cons patterns

**What**: Support `(,a ,b . ,rest)` for binding multiple elements
before the tail, similar to Erlang's `[A, B | Rest]`.

**Why rejected**: Deferred, not rejected outright. Classic Lisp
cons cells are strictly pairs — `car` and `cdr`. Multiple-head
patterns would require desugaring to nested pair access, which
users can already express with nested dotted pairs:
`(,a . (,b . ,rest))`. May be reconsidered as sugar in a future
version if demand arises.

### `parseInt` calls in radix output

**What**: Emit `parseInt("ff", 16)` instead of computing the
value at compile time.

**Why rejected**: Compiled output should be clean, readable JS
with no unnecessary function calls. The value is fully known at
compile time — emitting `0xff` or `255` is simpler, more readable,
and marginally more efficient than a runtime `parseInt` call.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `#;` before closing paren | Discards form, paren closes normally | `(a #;b c)` → `(a c)` |
| `#;` before `#;` | Each discards one form | `(a #;#;b c d)` → `(a d)` |
| `#;` at end of list | Reader error (no form to discard) | `(a b #;)` → error |
| Empty `#()` | Valid, produces `(array)` | `#()` → `[]` |
| Empty `#s()` | Valid, produces `(object)` | `#s()` → `({})` |
| `#s` with single-element sub-list | Follows `object` rules — compile error | `#s((name))` → error per DD-06 |
| `#0r` or `#1r` | Reader error — base must be 2–36 | `#1r0` → error |
| `#37r` or higher | Reader error — base must be 2–36 | `#37r10` → error |
| Invalid digit for base | Reader error | `#2r29` → error: `9` is not valid in base 2 |
| `#NNr` with no value | Reader error | `#16r` → error: missing value |
| Unbalanced `#\|` | Reader error at end of file | Missing `\|#` → error |
| `#` followed by unknown character | Reader error | `#z` → error: unknown dispatch character |
| Dot outside quasiquote pattern | Reader produces cons-pair node; expansion error if not in pattern context | Context determines validity |
| `(a . b . c)` | Reader error — only one dot allowed per list level | Multiple dots → error |
| `(. a)` | Reader error — dot cannot be first | Leading dot → error |
| `(a .)` | Reader error — dot cannot be last | Trailing dot → error |
| `(a . )` | Reader error — nothing after dot | Missing cdr → error |
| `car`/`cdr` on non-cons data | No compile-time check — runtime behavior of `x[0]`/`x[1]` on whatever `x` is | User responsibility |
| `(list)` with no args | Produces `null` | Empty list = nil |
| `#` mid-atom (`temp#gen`) | Not a dispatch trigger — reader treats as single atom | DD-11 auto-gensym unaffected |

## Dependencies

- **Depends on**: DD-01 (colon syntax — dot freed for reuse,
  `#` accepted as atom character), DD-06 (destructuring — `array`
  and `object` pattern forms, grouped `(key value)` pairs), DD-08
  (misc — reader character handling), DD-10 (quasiquote — patterns
  use quasiquote for construction/destructuring), DD-11 (`#gen`
  auto-gensym — dispatch must not conflict with `#` in atoms)
- **Affects**: DD-10 (amends `list` → `array` in Bawden's algorithm
  and macro environment API; updates rejected cons alternative
  rationale), DD-11 (amends `list` → `array` in macro environment
  API, `new Function` example, and quasiquote compilation section),
  DD-13 (macro expansion pipeline — expander must handle `cons`,
  `list`, `car`/`cdr`/`cadr`/`cddr` desugaring and dotted-pair
  pattern expansion), DD-14 (macro modules — module macros may
  use cons list operations)

## Open Questions

None.