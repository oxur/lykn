# Template and i18n

The `template` form has two modes: **concat mode** (the original) and
**ICU mode** (new in 0.6.0). Concat mode concatenates fragments into a
JS template literal. ICU mode parses an ICU MessageFormat string at
compile time and emits JS that evaluates the template with named
parameters, plural/select branching, and zero runtime dependencies.

Both modes coexist. The dispatch is automatic: if arg 0 is a literal
string and arg 1 is a keyword, it's ICU mode; otherwise it's concat.

---

## Concat mode (unchanged)

```lykn
(template "Hello, " name "!")
;; → `Hello, ${name}!`

(template a b)
;; → `${a}${b}`

(template "value: " (compute x))
;; → `value: ${compute(x)}`
```

Each string argument becomes a template-literal segment; each
non-string argument becomes a `${expr}` interpolation. No runtime
overhead beyond what JS template literals already provide.

---

## ICU mode

### Simple slots

```lykn
(template "Hello, {name}!" :name user-name)
;; → `Hello, ${userName}!`
```

The ICU string is the first argument. Named slots (`{name}`) are
replaced with the value of the corresponding keyword argument
(`:name`). Slot names use the same lisp-case convention as lykn
identifiers.

### Multi-use slots

```lykn
(template "{name}, please review {name}'s changes." :name actor)
```

A slot can appear any number of times. The keyword argument is
evaluated once; each reference uses the same value.

### Plural

```lykn
(template "You have {count, plural, one {1 message} other {# messages}}."
  :count n)
```

Inside `{count, plural, ...}`, branches are selected by the CLDR
plural category of the value:

| Category | English rule |
|----------|-------------|
| `one`    | count == 1  |
| `other`  | everything else |

Use `=N` for exact-value branches:

```lykn
(template "{n, plural, =0 {No messages} one {1 message} other {# messages}}"
  :n count)
```

`=N` branches take priority over category branches. Inside any
branch, `#` is shorthand for the selector's value.

**Phase A (0.6.0)** supports English plural rules only (`one` and
`other`). The categories `zero`, `two`, `few`, and `many` are
recognized as valid CLDR categories but rejected at compile time with
a hint to use `=N` instead. Locale-aware plural rules are planned for
a future milestone.

### Select

```lykn
(template "{role, select, owner {You own this.} member {You are a member.} other {Read access.}}"
  :role user-role)
```

Select branches match by string equality. The `other` branch is
required and serves as the fallback.

### Nesting

Branches can contain further slots and selectors:

```lykn
(template "{role, select, owner {Welcome, {name}! {count, plural, =0 {No tasks.} one {1 task.} other {# tasks.}}} other {Hello, guest.}}"
  :role  role
  :name  user-name
  :count task-count)
```

### Escape sequences

| Sequence | Result |
|----------|--------|
| `'{'`    | literal `{` |
| `'}'`    | literal `}` |
| `''`     | literal `'` |
| lone `'` | literal `'` |

These follow ICU's quoting rules, not Python's `{{ }}`.

---

## Dispatch rules

| Form | Mode | Reason |
|------|------|--------|
| `(template "hello")` | ICU | Single literal string, no slots |
| `(template name)` | Concat | arg 0 is not a literal string |
| `(template "Hi, " name "!")` | Concat | arg 1 is not a keyword |
| `(template "Hi, {name}!" :name n)` | ICU | Literal string + keyword |
| `(template "Hi, " :name)` | Error | Ambiguous — add a value or use concat |

Every program that compiled before 0.6.0 continues to compile
unchanged. ICU mode is only activated by the literal-string-then-keyword
pattern, which was not a valid concat-mode form.

---

## Compile-time validation

All ICU errors are caught at compile time. No runtime ICU parser is
shipped.

### Missing keyword argument

```lykn
(template "Hello, {name}!")
;; ERROR: template: no binding for slot {name}
;;   hint: add :name <value> to the template call
```

### Unused keyword argument

```lykn
(template "Hello, {name}!" :name n :extra v)
;; ERROR: template: unused keyword argument :extra
;;   hint: remove :extra, or add a {extra} slot to the template
```

### Duplicate keyword argument

```lykn
(template "{a}" :a x :a y)
;; ERROR: template: duplicate keyword argument :a
```

### Missing `other` branch

```lykn
(template "{n, plural, one {x}}" :n count)
;; ERROR: template: plural block for {n} missing required 'other' branch
```

### Overlapping branches

```lykn
(template "{n, plural, =1 {a} one {b} other {c}}" :n count)
;; ERROR: template: plural block for {n} has overlapping branches
;;   '=1' and 'one' both match count == 1 under English plural rules.
```

### Non-English plural categories

```lykn
(template "{n, plural, zero {none} other {many}}" :n count)
;; ERROR: template: plural category 'zero' is not valid under English plural rules.
;;   hint: use '=0 {none}' for the n=0 case
```

---

## Multi-use and side effects

When a keyword argument expression is non-trivial (a function call,
property access, etc.) and the slot is referenced more than once, the
compiler hoists the expression into a local binding so it evaluates
exactly once:

```lykn
(template "{x}-{x}-{x}" :x (next-id))
;; Emits: (() => { const _x = nextId(); return `${_x}-${_x}-${_x}`; })()
```

Simple identifier or literal kwargs are not hoisted (re-referencing
them is side-effect-free).

---

## Multi-line ICU strings

ICU strings preserve whitespace verbatim, including newlines and
indentation. To keep templates readable without leaking whitespace
into the output, write them on a single line:

```lykn
(template "{role, select, owner {Welcome, {name}!} other {Hello.}}" :role r :name n)
```

---

## Slot name rules

Slot names accept `[a-zA-Z0-9_-]`. Lykn identifiers with `$` or
other special characters are not valid as slot names. Use a kwarg
rename if you need to thread such a value through a template:

```lykn
;; If you have a binding named $total:
(template "Total: {total}" :total $total)
```

---

## Tagged templates

`(tag fn (template ...))` works with both concat-mode and ICU-mode
inner templates. The tag function receives the post-expansion JS
template literal:

```lykn
(tag html (template "<div>{content}</div>" :content body))
```

---

## What's not in Phase A (0.6.0)

- Locale-aware date, time, or number formatting
- CLDR plural rules for non-English locales
- Translation-extraction tooling (gettext-style `.po` extraction)
- Runtime hot-swap of templates by locale

These are planned for future milestones. See DD-55 for the roadmap.

---

## Related guidelines

- **Core Idioms**: `01-core-idioms.md` ID-05 (`template` for interpolation)
- **Anti-Patterns**: `09-anti-patterns.md` (kernel forms in surface code)
- **Surface Forms**: `00-lykn-surface-forms.md` (complete form reference)
