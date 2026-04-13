---
number: 8
title: "DD-08: Special Atoms, Update Operators, and Miscellaneous Forms"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-03-24
updated: 2026-04-12
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# DD-08: Special Atoms, Update Operators, and Miscellaneous Forms

**Status**: Decided
**Date**: 2026-03-24
**Session**: (this chat)

## Summary

Quick decisions on eleven smaller topics. `this` and `super` emit proper ESTree nodes. Prefix `++`/`--` supported, postfix deferred. Ternary uses `?` (avoiding colon conflict with DD-01). `do-while` puts the test first for internal consistency. All compound assignment operators included. `**` added to binary operators. `debugger`, `label`, `seq`, and `regex` are straightforward.

## Decisions

### A: `this` → `ThisExpression`

**Decision**: The atom `this` compiles to `{ type: "ThisExpression" }` instead of `Identifier("this")`. `this:name` via colon syntax produces `MemberExpression(ThisExpression, "name")`.

**Syntax**:

```lisp
this
(this:name)
(= this:age 42)
```

```javascript
this
this.name
this.age = 42
```

**ESTree nodes**: `ThisExpression`

**Rationale**: Implementation fix. ESTree defines `ThisExpression` as the correct node. No design implications — `this` has no hyphens (camelCase is a no-op), and arrow function `this` binding is a runtime behavior, not a compiler concern. Already covered in DD-01 and DD-07.

### B: `super` → `Super`

**Decision**: The atom `super` compiles to `{ type: "Super" }`. The compiler does not validate whether `super` appears in a valid context — the JS engine reports errors.

**Syntax**:

```lisp
;; constructor delegation
(super name)

;; parent method call
(super:speak)
(super:method arg1 arg2)
```

```javascript
// constructor delegation
super(name);

// parent method call
super.speak();
super.method(arg1, arg2);
```

**ESTree nodes**: `Super`, `CallExpression`, `MemberExpression`

**Rationale**: Same approach as DD-01 — the compiler emits the node, the JS engine validates context. Already covered in DD-01 and DD-07.

### C: Update operators — prefix only

**Decision**: `(++ x)` and `(-- x)` emit prefix `UpdateExpression`. Postfix is deferred to post-v0.1.0.

**Syntax**:

```lisp
(++ x)
(-- x)
```

```javascript
++x
--x
```

**ESTree nodes**: `UpdateExpression` (with `prefix: true`)

**Rationale**: Prefix `++`/`--` is natural as an s-expression form. Postfix is only needed when the return value matters (e.g., `arr[i++]`), which is rare and arguably an antipattern. `(+= x 1)` covers the common "just increment" case. Deferring postfix avoids the naming question (`post++`? `_++`?) for something that's rarely needed.

### D: Ternary conditional — `?`

**Decision**: `(? test consequent alternate)` emits `ConditionalExpression`. Three required arguments.

**Syntax**:

```lisp
(const label (? (> x 0) "positive" "non-positive"))
(const value (? condition a b))
(console:log (? (done? item) "yes" "no"))
```

```javascript
const label = x > 0 ? "positive" : "non-positive";
const value = condition ? a : b;
console.log(done(item) ? "yes" : "no");
```

**ESTree nodes**: `ConditionalExpression`

**Rationale**: `?:` was the initial proposal but the colon would trigger DD-01's colon splitting, breaking the atom into `?` and an empty string. `?` alone avoids the conflict, is visually connected to JS's `?:` operator, and is terse. It doesn't conflict with `?` used in predicate names (e.g., `done?`) because those are different atoms — `?` is standalone, `done?` has characters before the `?`. Lykn's `if` is a statement (`IfStatement`); `?` is the expression form (`ConditionalExpression`).

### E: `debugger`

**Decision**: `(debugger)` emits `DebuggerStatement`. No arguments.

**Syntax**:

```lisp
(debugger)
```

```javascript
debugger;
```

**ESTree nodes**: `DebuggerStatement`

**Rationale**: Direct 1:1 mapping. Nothing to design.

### F: Labeled statements

**Decision**: `(label name body)` emits `LabeledStatement`. The label name goes through camelCase conversion.

**Syntax**:

```lisp
(label outer
  (for-of item items
    (if (done? item) (break outer))))

(label my-loop
  (while true
    (if (finished) (break my-loop))))
```

```javascript
outer:
  for (const item of items) {
    if (done(item)) break outer;
  }

myLoop:
  while (true) {
    if (finished()) break myLoop;
  }
```

**ESTree nodes**: `LabeledStatement`

**Rationale**: Labels are identifiers in JS, so camelCase conversion applies for consistency with DD-01. `label` as the form name is clear and JS-aligned.

### G: `do-while` — test first

**Decision**: `(do-while test body...)` puts the test first, consistent with `while` and all other conditional forms in lykn.

**Syntax**:

```lisp
(do-while (> x 0)
  (-= x 1)
  (console:log x))
```

```javascript
do {
  x -= 1;
  console.log(x);
} while (x > 0);
```

**ESTree nodes**: `DoWhileStatement`

**Rationale**: Internal consistency wins over JS alignment. Every conditional form in lykn puts the test first: `if`, `while`, `?`. JS's test-last ordering for `do...while` is a syntactic quirk of C-family languages. In s-expressions, the natural reading is left to right: "do while this holds, do these things."

### H: Sequence expression

**Decision**: `(seq expr1 expr2 ...)` emits `SequenceExpression`.

**Syntax**:

```lisp
(seq a b c)
(seq (++ i) (console:log i))
```

```javascript
a, b, c
(++i, console.log(i))
```

**ESTree nodes**: `SequenceExpression`

**Rationale**: Same as eslisp. Rarely used but needed for completeness. `seq` is short and clear.

### I: Regex literals

**Decision**: `(regex pattern)` or `(regex pattern flags)` emits a regex `Literal`. Pattern and flags are both strings.

**Syntax**:

```lisp
(regex "^hello" "gi")
(regex "\\d+" "g")
(regex "^test$")
```

```javascript
/^hello/gi
/\d+/g
/^test$/
```

**ESTree nodes**: `Literal` (with `regex: { pattern, flags }`)

**Rationale**: Two-arg form for pattern + flags, one-arg form for no flags. Both are strings, keeping it simple.

### J: All compound assignment operators

**Decision**: All compound assignment operators are included in v0.1.0, including logical assignment (ES2021) and exponentiation assignment.

**Syntax**:

```lisp
;; arithmetic
(+= x 1)   (-= x 1)   (*= x 2)   (/= x 2)   (%= x 3)   (**= x 2)

;; bitwise
(<<= x 1)  (>>= x 1)  (>>>= x 1)
(&= x 1)   (|= x 1)   (^= x 1)

;; logical (ES2021)
(&&= x y)  (||= x y)  (??= x y)
```

```javascript
// arithmetic
x += 1;  x -= 1;  x *= 2;  x /= 2;  x %= 3;  x **= 2;

// bitwise
x <<= 1;  x >>= 1;  x >>>= 1;
x &= 1;  x |= 1;  x ^= 1;

// logical
x &&= y;  x ||= y;  x ??= y;
```

**ESTree nodes**: `AssignmentExpression` (with corresponding operator)

**Rationale**: Mechanical to implement — each is a one-line operator registration. No reason to defer any of them. All take exactly two arguments.

### K: `**` exponentiation operator

**Decision**: `**` is added to the binary operator table.

**Syntax**:

```lisp
(** 2 10)
(** base exponent)
```

```javascript
2 ** 10
base ** exponent
```

**ESTree nodes**: `BinaryExpression` (with `operator: "**"`)

**Rationale**: ES2016 operator. Easy addition to the existing operator table.

## Rejected Alternatives

### `?:` for ternary

**What**: `(?: test then else)` mirroring JS's ternary operator.

**Why rejected**: The colon in `?:` triggers DD-01's colon splitting, which would break the atom into `?` and empty string. `?` alone avoids the conflict and is still visually connected to JS's ternary.

### `if-expr` for ternary

**What**: `(if-expr test then else)` as a verbose but explicit conditional expression.

**Why rejected**: Unnecessarily verbose. `?` is terse, unambiguous, and JS-aligned.

### `cond` for ternary

**What**: `(cond test then else)` borrowing from Lisp tradition.

**Why rejected**: `cond` in CL/Scheme is a multi-clause conditional. Using it for a three-arg ternary would confuse Lisp programmers.

### Postfix `++`/`--` in v0.1.0

**What**: `(post++ x)` or `(_++ x)` for postfix update operators.

**Why rejected**: Postfix is only needed when the return value matters, which is rare. `(+= x 1)` covers the common case. Naming the postfix form (`post++`? `_++`?) is an unresolved question not worth blocking v0.1.0 for.

### `do-while` with test last

**What**: `(do-while body... test)` mirroring JS's `do { ... } while (test)` ordering.

**Why rejected**: Every other conditional form in lykn puts the test first (`if`, `while`, `?`). Internal consistency is more important than mirroring JS's C-family syntactic quirk.

## Edge Cases

| Case | Behavior | Example |
|------|----------|---------|
| `?` with wrong arity | Compile-time error | `(? test)` or `(? a b c d)` → error |
| `++`/`--` on non-lvalue | Compiler emits node, JS engine errors | `(++ 5)` → `++5` → JS runtime error |
| `debugger` with args | Compile-time error | `(debugger "foo")` → error |
| `label` with non-atom name | Compile-time error | `(label (foo) ...)` → error |
| `regex` with no args | Compile-time error | `(regex)` → error |
| `regex` with 3+ args | Compile-time error | `(regex "a" "g" "extra")` → error |
| `seq` with 0-1 args | Compile-time error | `(seq)` or `(seq a)` → error (need 2+) |
| `?` in predicate names | No conflict | `done?` is a different atom from `?` |
| `label` name with hyphens | camelCase applied | `(label my-loop ...)` → `myLoop: ...` |

## Dependencies

- **Depends on**: DD-01 (colon syntax, camelCase, `this`/`super` handling), DD-07 (class context for `this`/`super`)
- **Affects**: DD-09 (all items here factor into v0.1.0 scope)

## Open Questions

- [ ] Postfix `++`/`--` syntax — deferred, decide naming when the need arises
