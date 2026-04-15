# CC Task: Update Guides and Idioms for DD-25 / DD-25.1

## Prerequisites

READ FIRST:
- `dd-25-destructured-func-params.md` — the full DD
- `dd-25.1-nested-and-defaults.md` — the deferred follow-up
- `~/lab/cnbb/ai-design/guides/lykn/00-surface-forms.md` — current surface reference
- `~/lab/cnbb/ai-design/guides/lykn/SKILL.md` — CC entry point

This task updates five documents and adds new idiomatic patterns
that DD-25 enables. Execute after DD-25 implementation is merged
and all tests pass. If DD-25.1 is also merged, include those
patterns too; otherwise mark them as "available after DD-25.1."

---

## Part A: New idiomatic patterns

DD-25 doesn't just add syntax — it enables patterns that are
more concise and safer than their alternatives. These patterns
should be documented as **idiomatic Lykn** and preferred over
the alternatives.

### Idiom 1: Named parameters via destructured `func` args

**The pattern**: A function accepts a single object with typed
fields instead of positional parameters. The caller uses `obj`
with keyword syntax.

```lisp
;; IDIOMATIC — named params with per-field types
(func connect
  :args ((object :string host
                 :number port
                 (default :boolean ssl true)))    ;; DD-25.1
  :body (open-connection host port ssl))

;; Caller — self-documenting, order-independent
(connect (obj :host "localhost" :port 5432))
(connect (obj :host "db.prod.internal" :port 5432 :ssl true))
```

```javascript
function connect({host, port, ssl = true}) {
  openConnection(host, port, ssl);
}
connect({ host: "localhost", port: 5432 });
```

**Why it's idiomatic**: The Lykn caller side is `(obj :host "localhost"
:port 5432)` — keyword-value pairs that read like a config. The
function definition names and types each field. The JS output is
clean destructured params. No runtime overhead. Per-field type
safety. Self-documenting call sites.

**Contrast with positional params**:

```lisp
;; LESS IDIOMATIC — positional, hard to read at call site
(func connect
  :args (:string host :number port :boolean ssl)
  :body (open-connection host port ssl))

(connect "localhost" 5432 true)  ;; what's true? ssl? verbose?
```

**Contrast with pre-DD-25 workaround**:

```lisp
;; PRE-DD-25 — type safety gap at the boundary
(func connect
  :args (:object opts)
  :body
  (bind (object host port ssl) opts)  ;; no per-field type checks!
  (open-connection host port ssl))
```

**When to use named params**: 3+ parameters, optional/defaulted
fields, config-style interfaces, any function where the caller
benefits from labeled arguments.

**When to keep positional**: 1–2 params with obvious meaning
(e.g., `(func add :args (:number a :number b) ...)`), callbacks
where brevity matters.

### Idiom 2: Handler/callback destructuring

**The pattern**: Event handlers and callbacks destructure their
argument to extract the fields they need, with type annotations
on each.

```lisp
;; IDIOMATIC — destructure the request, type each field
(func handle-login
  :args ((object :string method :string url :any body) :any res)
  :returns :void
  :body
  (if (= method "POST")
    (authenticate body res)
    (res:status 405)))

;; IDIOMATIC — DOM event handler
(button:add-event-listener "click"
  (fn ((object :string type :any target :boolean shift-key))
    (if shift-key
      (handle-shift-click target)
      (handle-click target))))
```

**Contrast with `:any` param + body access**:

```lisp
;; LESS IDIOMATIC — no type info, field access scattered through body
(func handle-login
  :args (:any req :any res)
  :body
  (if (= req:method "POST")
    (authenticate req:body res)
    (res:status 405)))
```

**Why destructuring is better**: Fields used by the function are
declared upfront in the param list — visible, typed, documented.
The body doesn't need colon-access chains into an opaque `:any`
parameter. A reader can see at a glance what the function uses
from its argument.

### Idiom 3: Component/widget props

**The pattern**: UI components receive typed props via
destructured params. Especially natural with `fn` for inline
component definitions.

```lisp
;; IDIOMATIC — typed props in fn
(bind UserCard
  (fn ((object :string name :string email :number age))
    (template
      "<div class='card'>"
      "<h2>" name "</h2>"
      "<p>" email " — age " age "</p>"
      "</div>")))

;; Usage
(UserCard (obj :name "Duncan" :email "d@example.com" :age 42))
```

**Contrast**: Without DD-25, you'd take `:any props` and access
`props:name`, `props:email`, `props:age` throughout the body —
losing both type checking and the upfront declaration of which
props the component uses.

### Idiom 4: Multi-clause structural dispatch

**The pattern**: Different clauses handle different *shapes* of
input — one accepts an object, another accepts a string. The
dispatch type is implicit from the destructuring pattern.

```lisp
;; IDIOMATIC — structural dispatch via destructuring
(func process-input
  (:args ((object :string name :string email) :string action)
   :returns :string
   :body (template name " (" email ") — " action))

  (:args (:string raw-input :string action)
   :returns :string
   :body (template raw-input " — " action)))

;; Caller
(process-input (obj :name "Alice" :email "a@b.com") "signup")
(process-input "raw data" "import")
```

**How it works**: Clause 1 dispatches on `:object` at position 0.
Clause 2 dispatches on `:string` at position 0. No overlap. The
reader sees the function accepts either a user object or a raw
string — two calling conventions, one function name.

Note: two clauses that both destructure objects at the same
position DO overlap (both match `:object`) — this is a compile
error. Structural dispatch is on the outer type, not on internal
field shapes.

### Idiom 5: `fn` in pipelines with destructured items

**The pattern**: Threading macros processing collections of
objects, where the `fn` destructures each item.

```lisp
;; IDIOMATIC — destructure each item in the pipeline
(->> users
  (filter (fn ((object :boolean active)) active))
  (map (fn ((object :string name :number age))
    (obj :display-name (string:to-upper-case name)
         :birth-year (- 2026 age)))))
```

**Contrast with `:any` + colon access**:

```lisp
;; LESS IDIOMATIC — opaque items, scattered access
(->> users
  (filter (fn (:any u) u:active))
  (map (fn (:any u)
    (obj :display-name (string:to-upper-case u:name)
         :birth-year (- 2026 u:age)))))
```

The destructured version is *slightly* more verbose in the param
list but dramatically more readable in the body — and it gets
per-field type checking.

**When each style wins**: For simple single-field access
(`u:active`), the `:any` style is fine — the access is trivial
and type checking adds little value. For multi-field access where
the body uses 2+ fields, destructuring wins because it documents
what the function needs and checks the types at the boundary.

---

## Part B: Document updates

### Update 1: Lykn Guide 00 — Surface Forms Reference

File: `~/lab/cnbb/ai-design/guides/lykn/00-surface-forms.md`

In the `func` section, add after the existing `:args` documentation:

**Destructured parameters (DD-25)**:

A destructuring pattern (`object` or `array`) can appear in
`:args` where a `:type name` pair would go. Every field inside
the pattern requires a type keyword.

```lisp
;; Object destructuring — fields are typed
(func greet
  :args ((object :string name :number age))
  :returns :string
  :body (template name " is " age))

;; Array destructuring
(func first-and-rest
  :args ((array :number head (rest :number tail)))
  :body (console:log head tail))

;; Mixed destructured + simple
(func handler
  :args ((object :string method :string url) :any body)
  :body ...)
```

Compiled output: JS destructured params + per-field type checks
in dev mode. `--strip-assertions` removes checks, preserves
destructuring.

Multi-clause dispatch: destructured `object` params have dispatch
type `:object`. Destructured `array` params have dispatch type
`:array`. Two clauses that both destructure objects at the same
position overlap — compile error.

In the `fn`/`lambda` section, add:

```lisp
;; fn with destructured params
(fn ((object :string name :number age)) (console:log name age))
```

Same rules: every field typed, `:any` opt-out.

If DD-25.1 is implemented, also add:

**Nested destructuring** (DD-25.1): nested patterns require
`alias` for object nesting (to name the intermediate property).
Array nesting is positional.

```lisp
(func f
  :args ((object :string name
                 (alias :any addr (object :string city :string zip))))
  :body (template name " in " city))
```

**Defaults in destructured params** (DD-25.1):

```lisp
(func f
  :args ((object :string name (default :number age 0)))
  :body (template name " age " age))
```

### Update 2: Lykn SKILL.md

File: `~/lab/cnbb/ai-design/guides/lykn/SKILL.md`

If the SKILL.md has a quick-reference or examples section showing
`func` syntax, add one destructured param example. Keep it brief —
the SKILL.md is an entry point, not a reference.

### Update 3: JS Guide 04 — Functions

File: `~/lab/cnbb/ai-design/guides/js/04-functions.md`

Find the section on "named parameters via destructuring" (or
"destructuring in parameters" or similar). Add a Lykn annotation:

> **Lykn note**: Surface `func` and `fn` accept destructured
> patterns in parameter position with per-field type annotations.
> This is the idiomatic way to implement named parameters in Lykn:
> the caller passes `(obj :key value ...)` and the function
> destructures with typed fields. See DD-25.

### Update 4: Conversation bootstrap v7

File: `~/lab/oxur/lykn/workbench/conversation-bootstrap-v7.md`

This update is deferred until the full bootstrap rewrite happens.
When it does, ensure:

- DD-16 summary mentions destructured params as valid in `:args`
- DD-25 and DD-25.1 appear in the DD table
- Surface form vocabulary table notes that `func`/`fn` accept
  destructured patterns
- The deferred features table no longer lists destructured func
  params
- The new idioms (named params, handler destructuring, component
  props) are listed as idiomatic patterns

### Update 5: Book chapter prompts

**Ch 7 prompt** (`lykn-book-cc-ch07-prompt.md`):
- Section 7.6 (Parameters in depth): update the forward reference
  to Ch 15. Can now show a simple destructured param example inline
  as a preview: "Destructured parameters let you type each field
  individually — see Chapter 15 for the full story."

**Ch 8 prompt** (`lykn-book-cc-ch08-prompt.md`):
- Section 8.4 (Overlap is a compile error): add a note that
  destructured object params have dispatch type `:object`. Two
  clauses that both destructure objects overlap. Show one example.

**Ch 15 prompt** (`lykn-book-cc-ch15-prompt.md`):
- Section 15.3 (Destructuring in function parameters): this is
  the main update. Remove any "current limitation" or "workaround"
  language. Show the clean one-step pattern with typed fields.
  Show the named-params idiom as the primary use case. Show the
  compiled JS output (destructured params + type checks).
- Add a subsection: "Named parameters — the Lykn way." Feature
  the `obj` keyword syntax on the caller side as the complement
  to destructured params on the definition side.

---

## Part C: Style guide / best practices additions

Add these to the lykn guide or a new idioms section:

### Best practice: Prefer destructured params for 3+ fields

When a function takes 3 or more related parameters from the same
source (user data, config, request fields), prefer a single
destructured object param over multiple positional params. The
call site becomes self-documenting.

### Best practice: Type every field, even in destructured params

`:any` is the explicit opt-out. Don't use it reflexively — type
as specifically as you can. The type checks fire in dev mode and
catch bugs at the boundary. A field typed `:any` in a destructured
param is a field without a safety net.

### Best practice: Destructure in the param list, not the body

Pre-DD-25, the pattern was:
```lisp
(func f :args (:object opts) :body (bind (object x y) opts) ...)
```

Post-DD-25, prefer:
```lisp
(func f :args ((object :string x :number y)) :body ...)
```

The param-list version is shorter, documents the interface in the
signature, and gets per-field type checks. The body version loses
type safety and buries the interface.

### Best practice: Use `:any` for interop boundaries, types for domain code

When destructuring a JS library response (Express `req`, DOM
event, fetch response), `:any` fields are acceptable — you don't
control the shape. When destructuring your own domain objects,
type every field.

```lisp
;; Interop: :any is fine — we don't control Express's types
(func handle
  :args ((object :any method :any url :any body) :any res)
  :body ...)

;; Domain: type everything — we control this shape
(func process-order
  :args ((object :string id :number total :boolean paid))
  :body ...)
```
