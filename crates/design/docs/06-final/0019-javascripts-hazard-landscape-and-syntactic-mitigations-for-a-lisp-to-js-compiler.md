---
number: 19
title: "JavaScript's hazard landscape and syntactic mitigations for a Lisp-to-JS compiler"
author: "type systems"
component: All
tags: [change-me]
created: 2026-03-27
updated: 2026-03-27
state: Final
supersedes: null
superseded-by: null
version: 1.0
---

# JavaScript's hazard landscape and syntactic mitigations for a Lisp-to-JS compiler

**JavaScript's most dangerous features fall into roughly 70 distinct hazard categories, but empirical research reveals a counterintuitive truth: only ~15% of real-world JS bugs are preventable by type systems, while the majority stem from specification misunderstandings, async complexity, and missing input validation.** A Lisp (s-expression) language compiling to JS can address significantly more than types alone — approximately 40–50% of cataloged hazards — through compile-time enforcement, macro-based safety patterns, and immutability-by-default semantics. The strongest leverage points are eliminating implicit coercion, enforcing immutability, providing exhaustive pattern matching on algebraic data types, and implementing nil-safe access chains — features that target the exact gaps empirical studies identify in TypeScript's coverage. This report catalogs every known JS hazard, maps each to a syntactic mitigation strategy, compares 9 compile-to-JS languages, and proposes a prioritized safety roadmap for lykn.

---

## Part 1: What the empirical evidence actually shows

Eight major empirical studies spanning 2011–2026 converge on a consistent picture of JavaScript's failure modes. The data contradicts several common assumptions about where bugs live and what fixes them.

### Coercion: ubiquitous but mostly harmless

Pradel and Sen's 2015 dynamic analysis of 132 programs (top-100 Alexa sites plus benchmarks) instrumented **138.9 million runtime events** across 321,711 unique source locations. Their central finding: **98.85% of all implicit type coercions are harmless**. The remaining 1.15% — the genuinely dangerous ones — cluster into five specific patterns:

- **Non-strict equality between different types** (excluding the null/undefined idiom) — the single most prevalent harmful coercion
- **String concatenation with undefined/null** via `+`, producing strings like `"undefinedabc"` instead of throwing
- **Arithmetic/bitwise operators on non-number types**, silently producing `NaN`
- **Relational comparisons between incomparable types** (e.g., arrays vs. functions), always yielding `false`
- **Wrapped primitives in conditionals** (e.g., `new Boolean(false)` evaluating as truthy)

Manual inspection of 30 harmful coercion sites found **1 clear bug, 3 probable bugs, and 22 intentional uses**. The paper recommends future language designs disallow non-strict equality between different types, concatenation of undefined/null to strings, and arithmetic on non-numbers. For every 1 explicit type conversion in real code, there are **269 implicit coercions** — making compile-time enforcement essential rather than relying on developer discipline.

### The bug pattern taxonomy from 105K commits

Hanam et al.'s BugAID tool (FSE 2016) mined **105,133 commits** from 134 Node.js projects and discovered **13 pervasive bug patterns** organized in 6 groups through unsupervised clustering of AST-level bug fixes:

**Group 1 — Dereferenced non-values** (the largest group by far): (1) protecting with falsy checks (`if (obj)`), (2) protecting with no-value checks (`!= null`), and (3) protecting with type checks (`typeof`/`===`). **Group 2 — Wrong equality operators**: (4) using `==` when `===` was needed, (5) comparing against wrong values. **Group 3 — Function argument errors**: (6) missing arguments, (7) wrong argument values. **Group 4 — Initialization errors**: (8) wrong variable assignments, (9) missing variable initialization. **Group 5 — Context/scoping**: (10) wrong `this` binding in callbacks, (11) variable scoping issues. **Group 6 — Other pervasive patterns**: (12) missing error handling (no try-catch or callback error checks), (13) wrong API method or property names.

Of these 13 patterns, **patterns 1–4, 9–10, and 12 are directly preventable by syntax/compiler design** — representing roughly half the discovered patterns. Patterns involving wrong values, wrong arguments, and wrong API names require understanding programmer intent and resist syntactic prevention.

### The 15% type system ceiling — and the 85% below it

Gao et al. (ICSE 2017) conducted the most rigorous quantification of type system effectiveness: they manually annotated **400 real bugs** from 398 GitHub projects with both Flow and TypeScript types. **Both detected exactly 15% of bugs** (60 out of 400, 95% CI: 11.5%–18.5%). The two type systems detected nearly identical bugs — 57 shared, 3 unique to each.

The **85% of bugs undetectable by types** breaks down as: specification errors at **55%** (the code doesn't match what it should do), branch/logic errors, wrong predicates, URI handling errors, **string content errors** (the second most common undetectable category — wrong URLs, malformed SQL, incorrect string content), UI errors, regex errors, and API misuse. TypeScript 2.0's `strictNullChecks` delivered a **58% improvement** over TypeScript 1.8, demonstrating that null-safety is the single highest-leverage type system feature.

### What persists even with TypeScript

Tang et al.'s 2026 study of **633 bugs** across 16 TypeScript repositories reveals a paradigm shift. The dominant bug category is no longer logic errors but **tooling and configuration failures at 27.8%** — a category virtually absent from JavaScript-era studies. API misuse accounts for 14.5%, and notably, **type errors still constitute 12.4%** of TypeScript bugs, caused by unsafe casts, missing annotations, reliance on `any`, and "type erosion" where guarantees are relaxed for JS interop. Async/event bugs persist at 7% regardless of the type system. The study identifies two principal fault axes in modern TypeScript: an integration axis (async + API + error-handling) and a toolchain axis (config + build + dependency).

### Concurrency: the event loop is not a silver bullet

Wang et al.'s analysis of **57 real Node.js concurrency bugs** demolishes the assumption that single-threaded execution prevents concurrency issues. **65% are atomicity violations** — multi-event sequences that should be atomic but aren't. 30% are ordering violations. 70% stem from non-deterministic event triggering, and crucially, **only 23% were fixable by adding synchronization**. The remaining 77% required semantic fixes like switching to atomic APIs, bypassing the race condition, or data privatization. Resources contended include shared variables (54%), databases (26%), and files (14%) — a distribution unique to server-side JavaScript.

### DLint: what static analysis misses

Gong et al.'s DLint (ISSTA 2015) implemented **28 dynamic checkers** across 200+ popular websites, analyzing 4 million lines of JavaScript over 178 million runtime operations. The tool found **9,018 warnings**, of which **49 per site on average were missed by JSHint**. Notable real-world bugs discovered include `$NaN` displayed as product prices on IKEA and eBay (NaN propagation from undefined values in arithmetic), a futile write on Twitch (`window.onbeforeunload = "string"` silently ignored by the DOM), and a style object compared as a string on Craigslist.

### BugsJS: server-side patterns

Gyimesi et al.'s BugsJS benchmark of **453 validated bugs** from 10 Node.js projects found **missing input validation** as the most prevalent category at 32%, followed by wrong/missing conditions, variable initialization bugs, and missing type conversions. The dominance of input validation failures suggests that contract/assertion systems (Racket-style `define/contract` or Clojure's `spec`) could address the single largest bug category in server-side JavaScript.

---

## Part 2: The complete JavaScript footgun catalog

The following catalog synthesizes ~70 distinct hazards across 10 categories, with severity ratings derived from empirical frequency data and security impact.

### Type system hazards dominate frequency rankings

The most frequently encountered hazards involve JavaScript's implicit type coercion system. The `+` operator's overloading — performing addition on numbers but concatenation when either operand is a string — is the root of the most common type confusion bugs. The expression `'3' + 1` yields `'31'` while `'3' - 1` yields `2`. Arrays coerced to strings produce results like `[] + {} === "[object Object]"` and `[] + [] === ""`. The `==` operator's coercion table creates equivalences that violate transitivity: `"" == false` and `"0" == false` are both true, but `"" == "0"` is false.

**`typeof null === "object"`** is a 1995 implementation bug that became specification. Combined with **`typeof NaN === "number"`** and **`NaN !== NaN`** (IEEE 754), these create a minefield for type checking code. The `parseInt` function produces surprising results when passed to `.map()` — `["1","2","3"].map(parseInt)` yields `[1, NaN, NaN]` because `map` passes the index as `parseInt`'s radix parameter. Floating-point arithmetic means `0.1 + 0.2 !== 0.3`, a problem affecting every financial calculation.

### Scoping and `this` binding cause the most frustrating bugs

The `this` keyword's behavior — determined by call-site, not definition — is responsible for BugAID's pattern #10 and one of the most frequently reported JavaScript confusions. Extracting a method loses its context: `const fn = obj.greet; fn()` produces `undefined` for `this.name`. The `var` keyword's function-scoped hoisting enables the classic closure-over-loop-variable bug where `for(var i = 0; i < 5; i++) { setTimeout(() => console.log(i), 100) }` prints `5` five times. Implicit global creation (assigning to an undeclared variable in non-strict mode) silently creates properties on the global object.

### Security hazards carry critical severity

**Prototype pollution** is JavaScript's most impactful security-specific vulnerability class. Attackers inject `__proto__` keys through deep merge functions or `JSON.parse`, modifying `Object.prototype` and affecting every object in the runtime. Real CVEs exist in Lodash, jQuery, dot-prop, and dozens of other libraries. The attack vector is deceptively simple: `JSON.parse('{"__proto__":{"isAdmin":true}}')` followed by a naive merge operation.

**ReDoS** (Regular Expression Denial of Service) exploits JavaScript's backtracking NFA regex engine with patterns containing nested quantifiers. The pattern `/(a+)+$/` exhibits catastrophic backtracking, causing exponential time on inputs like `"aaaaaaaaaaX"`. Real incidents include a 34-minute Stack Overflow outage (2016) and a 27-minute Cloudflare outage (2019).

**Supply chain attacks** represent the fastest-growing threat: malicious npm packages surged from 38 reports in 2018 to over **3,000 in 2024**. The September 2025 Shai-Hulud attack compromised 18 packages with **2.6 billion combined weekly downloads** through a single phishing vector.

### Async hazards resist all current mitigation strategies

Unhandled promise rejections, the inability of `try/catch` to capture errors from non-awaited promises, and the silent error swallowing of `async` callbacks passed to `.forEach` form a triad of async hazards that persist across JavaScript, TypeScript, and every compile-to-JS language studied. The expression `items.forEach(async (item) => { await fn(item) })` looks sequential but runs all iterations in parallel, with errors becoming unhandled rejections. The `return` vs `return await` distinction inside `try/catch` blocks silently changes whether rejections are caught.

### Full severity breakdown

Across all categories, the catalog identifies **5 critical-severity** hazards (prototype pollution, eval injection, DOM XSS, supply chain attacks, arbitrary code execution via `eval`), **24 high-severity** hazards (implicit coercion, NaN propagation, floating-point errors, `var` hoisting, `this` binding loss, implicit globals, closure-over-loop-variable, `Array.sort` string comparison, `forEach` ignoring async, unhandled rejections, `try/catch` async failure, silent non-strict failures, ASI with `return`, ReDoS, template injection, race conditions, stale closures, and others), **30 medium-severity** hazards, and **11 low-severity** quirks.

---

## Part 3: How other languages address these problems

### The soundness spectrum determines real-world safety

Languages targeting JavaScript arrange on a spectrum from no type safety to full soundness. **Elm, ReScript, and PureScript** provide sound type systems — if it compiles, runtime type errors are impossible in pure code. **TypeScript** is intentionally unsound, with seven documented sources of unsoundness including the `any` escape hatch, covariant array assignability, out-of-bounds array access returning the element type rather than `undefined`, and type assertions that bypass checking entirely. **ClojureScript and CoffeeScript** provide no compile-time type guarantees.

The empirical evidence from NoRedInk is striking: **100,000+ lines of Elm in production since 2015 with zero runtime exceptions**. This demonstrates that full soundness is achievable in practice, though it comes at the cost of restricted JS interop (Elm's port system is deliberately limited) and a smaller ecosystem.

### Immutability-by-default is the highest-impact single feature

ClojureScript, Elm, ReScript, and PureScript all make immutability the default. ClojureScript's persistent data structures use structural sharing (hash array mapped tries) to make immutable updates efficient. Mutation requires explicit atoms with compare-and-swap semantics — `(swap! state update :count inc)` — making every state change visible and auditable. This single design decision eliminates entire categories of bugs: stale closure state, race conditions on shared variables, unexpected side effects from function calls, and prototype pollution through mutation.

ClojureScript's approach is most relevant to lykn: it demonstrates that Lisp syntax with immutability-by-default and atom-based controlled mutation compiles efficiently to JavaScript while eliminating a wide class of hazards. Value equality by default (`=` performs deep structural comparison) and simplified truthiness (only `false` and `nil` are falsy) remove additional footgun categories.

### CoffeeScript proved syntax matters, TypeScript proved types matter more

CoffeeScript's most impactful safety feature was compiling `==` to `===` unconditionally — eliminating the entire coercion equality table. Its existential operator (`?.`) for null-safe access and fat arrow (`=>`) for lexical `this` binding were so successful that JavaScript itself adopted them (optional chaining in ES2020, arrow functions in ES2015). CoffeeScript declined because ES6 absorbed its syntactic innovations and TypeScript addressed a more fundamental need (type safety), but its legacy proves that **compile-time syntax transformation for safety works and gets adopted upstream**.

### Dart's sound null safety is the gold standard for null handling

Dart's approach (required since Dart 3) makes all types non-nullable by default: `int count` cannot be null, while `int? maybeCount` explicitly opts in. Flow analysis automatically promotes types after null checks — after `if (x != null)`, the variable `x` is non-nullable in the branch body. The `!` operator serves as an explicit, documented escape hatch. Dart's guarantee is sound: "If an expression has a static type that does not permit null, then no possible execution can ever evaluate to null." This soundness enables compiler optimizations beyond what TypeScript's unsound null checking permits.

### Lisp dialects offer unique error-handling paradigms

Common Lisp's **condition/restart system** provides a fundamentally different approach to error handling. Unlike `try/catch` which immediately unwinds the stack, conditions signal errors while keeping the full call stack intact. Restarts define recovery strategies at the error site, while handlers at higher levels choose which restart to invoke — separating the decision of *how* to recover from *where* the error occurs. This enables recovery patterns impossible with exceptions, such as retrying with different input or substituting a default value without losing execution context.

**Racket's contract system** adds blame tracking to precondition/postcondition checking: when a contract is violated, the error identifies *which party* (caller or callee) broke the contract. Higher-order contracts wrap functions passed as arguments to check future invocations. **Clojure's spec** provides composable runtime validation with generative testing — specs automatically produce test data, and `fdef` specifications for function arguments, return values, and their relationships enable property-based testing from declarations alone.

### Comparative summary of hazard coverage

| Hazard category | TypeScript | Elm | ClojureScript | ReScript | PureScript |
|---|---|---|---|---|---|
| Type coercion | Mostly prevents | Fully prevents | No coercion issues | Fully prevents | Fully prevents |
| Null/undefined | strictNullChecks | Maybe/Result types | nil punning (partial) | Option type | Maybe type |
| Mutability bugs | Same as JS | All immutable | Persistent DS + atoms | Immutable default | Effect tracking |
| `this` confusion | Same as JS | No `this` | No `this` | No `this` | No `this` |
| Exhaustive matching | Non-exhaustive switch | Compiler-enforced | Not checked | Compiler-enforced | Compiler-enforced |
| Side effect tracking | None | TEA + Ports | None | Convention only | Effect monad |
| Runtime exceptions | Still possible | Near-zero | Still possible | Reduced | Eliminated in pure code |
| Async hazards | Partial (async/await) | Commands/Subscriptions | core.async (partial) | Promises (partial) | Aff monad |

---

## Part 4: Syntactic mitigation roadmap for lykn

Each hazard maps to one of three mitigation levels: **compile-time enforceable** (C) where the compiler rejects bad code, **macro-implementable** (M) where standard library macros transform code to be safe, or **convention-level** (V) where syntax encourages but doesn't enforce safety.

### Tier 1 — Zero-cost compile-time eliminations

These mitigations add no runtime overhead, require no opt-in, and eliminate hazards unconditionally through compiler output decisions.

**Strict equality only (C).** The compiler never emits `==`. The form `(= a b)` compiles to `a === b`. A separate `(deep= a b)` form provides structural comparison. This eliminates the entire coercion equality table, BugAID pattern #4, and the single most prevalent harmful coercion identified by Pradel and Sen. CoffeeScript proved this works and gets adopted.

**Immutability by default (C).** All `def` and `let` bindings emit `const`. Mutation requires explicit `def-atom` with `swap!` and `reset!` — exactly ClojureScript's model. This eliminates stale closure bugs, prevents accidental mutation, and makes state changes auditable. The form `(def x 42)` compiles to `const x = 42` while `(def-atom counter 0)` creates a managed mutable container.

**No `this` keyword (C).** The language has no `this` form. Object-oriented interop uses explicit self parameters: `(defn greet [self] (str "Hello, " (get self :name)))`. JS interop provides `(js/bind f obj)` when calling methods that require `this`. This eliminates BugAID pattern #10 and the entire `this`-binding hazard category.

**Safe iteration only (C).** The compiler never emits `for...in`. All iteration compiles to `for...of`, `Object.entries()`, or `Object.keys()`. The form `(for [x coll] (print x))` compiles to `for (const x of coll)`. This prevents inherited property iteration and the `for-in` on arrays antipattern detected by DLint.

**No implicit globals (C).** All code compiles in strict mode or as ES modules. No `var` emission — all locals use `const` or `let`. The closure-over-loop-variable bug is impossible because `let` is block-scoped and the compiler never emits `var`.

**Separated `+` operator (C).** Numeric addition uses `(+ a b)`, string concatenation uses `(str a b c)`. The compiler emits different operators for each. This eliminates the `+` overloading hazard and prevents the `"undefinedabc"` coercion pattern. If type information is available, `(+ "3" 1)` is a compile-time error.

### Tier 2 — Macro-powered safety patterns

These require standard library macros or compiler support beyond simple output transformation, but add powerful safety guarantees.

**Nil-safe access chains (M).** Threading macros with nil checking: `(some-> obj .foo .bar .baz)` compiles to `obj?.foo?.bar?.baz`. The `some->>` variant threads the value as the last argument with nil checking at each step. `(if-let [v (get obj :key)] (use v) (default))` binds only when non-nil. These macros directly target the most common BugAID pattern group (dereferenced non-values).

**Pattern matching with exhaustiveness (C+M).** Algebraic data types defined via `deftype` compile to tagged JS objects (`{tag: "Circle", radius: r}`). The `match` form with compiler-enforced exhaustiveness rejects code that doesn't handle all variants:

```
(deftype Shape
  (Circle :radius number)
  (Rect :w number :h number))

(match shape
  (Circle r) (* pi (* r r))
  (Rect w h) (* w h))
```

This addresses missing-case bugs (5% of TypeScript bugs per Tang et al.) and enables the `Result`/`Option` pattern that replaces exceptions for expected failures.

**Contract/assertion macros (M).** Racket-inspired contracts with blame tracking address the **32% of server-side bugs** caused by missing input validation (BugsJS's top category):

```
(defn/contract deposit [amount account]
  :pre  [(number? amount) (> amount 0)]
  :post [(= (balance %) (+ (balance account) amount))]
  (update account :balance + amount))
```

This compiles to runtime checks that can be stripped in production builds. The `:pre` and `:post` forms emit `ContractError` with source location and blame information identifying whether the caller or callee violated the contract.

**Spec-like runtime validation (M).** A `defspec` system inspired by Clojure.spec provides composable predicates, conformance checking, and generative testing:

```
(defspec :person (spec/keys :req [:name :age]))
(defspec :name string?)
(defspec :age pos-int?)
(spec/assert :person data)
```

**Type guard macros (M).** Forms like `(when-type [x :string] (str/upper x))` compile to `if (typeof x === 'string') { x.toUpperCase() }` with type narrowing in the body, bridging runtime checking and static type information.

### Tier 3 — Advanced safety features

**Algebraic data types (M+C).** Tagged unions via macros with compiler-enforced exhaustive matching. The `Result` and `Option` types are predefined:

```
(deftype Result (Ok value) (Err error))
(deftype Option (Some value) None)
```

These compile to `{tag: "Ok", value: x}` — zero-dependency, debuggable JavaScript. Combined with exhaustive `match`, they replace exception-based error handling for expected failure paths, addressing the error-swallowing hazards.

**Condition/restart system (M).** A simplified version of Common Lisp's three-layer error handling, compiled to JavaScript using a dynamic restart registry. Low-level code establishes restart points; high-level handlers choose recovery strategies without unwinding the call stack. While JS cannot preserve the full non-unwinding property, the separation of error detection from recovery strategy selection enables patterns impossible with `try/catch` — particularly valuable for the 77% of Node.js concurrency bugs that Wang et al. found unfixable by synchronization alone.

**Safe arithmetic macros (M).** Forms like `(money+ #M"10.50" #M"3.25")` for BigDecimal arithmetic, `(int+ a b)` compiling to `(a + b) | 0` for integer-only operations, and `(checked+ a b)` with overflow detection. Reader macros validate literal formats at compile time.

**Reader macros for validated literals (C).** `#rx"^[a-z]+$"` validates regex syntax at compile time and optionally analyzes for ReDoS susceptibility. `#date"2024-01-15"` and `#url"https://example.com"` validate format at read time, catching string-content errors — the second most common type-system-undetectable bug category per Gao et al.

**Import-time safety (C).** Module exports are frozen by default: `Object.freeze()` is emitted on all exported objects, preventing prototype pollution through monkey-patching. All imports are `const`.

**Controlled eval (C+M).** The language provides no `eval` form. A `safe-eval` macro interprets lykn s-expressions without executing arbitrary JavaScript. The compiler rejects any attempt to use `eval`, `new Function()`, or string-form `setTimeout`.

### Tier 4 — Convention-level and future features

**Effect tracking (V→C).** Initially convention-level: effectful functions use a `!` suffix (`save-to-db!`, `def-atom!`). With a type system, this graduates to compile-time enforcement distinguishing `Pure` and `IO` function types, following Coalton's model.

**Regular expression safety (C+M).** Compile-time regex validation catches syntax errors. Static analysis warns on patterns susceptible to catastrophic backtracking (nested quantifiers with overlapping character classes). An optional `#safe-rx` form restricts to a ReDoS-safe subset.

---

## Part 5: The type annotation question

### Six Lisp type systems offer different models

**Common Lisp's `declare`/`the` forms** are advisory hints: `(declare (type fixnum x))` tells the compiler the type but violations have undefined behavior. SBCL uses these for optimization and compile-time warnings. **Typed Racket** adds full Hindley-Milner-inspired types with occurrence typing — after `(string? x)` returns true, `x` is known to be a string in that branch. Typed/untyped interop uses contracts at module boundaries. **Shen's sequent calculus** allows users to define custom type rules via backward-chaining Prolog — the most powerful approach but impractical for a JS target. **Coalton** embeds Haskell-like types with full inference and type classes in Common Lisp. **Carp** adds Rust-like ownership tracking to a Lisp — inapplicable to GC'd JavaScript but inspiring for data-flow analysis. **LFE** wraps Erlang's Dialyzer-checked type specs in Lisp syntax.

### Recommended approach: Coalton-inspired gradual typing with JSDoc output

The most practical model for lykn combines **Coalton's type inference and type classes** with **Typed Racket's occurrence typing** for idiomatic conditional narrowing. The key architectural decision is output format:

**Primary: JSDoc annotations in emitted JavaScript.** The compiler emits standard JavaScript with JSDoc type comments, enabling TypeScript's checker (`tsc --checkJs --noEmit`) to validate without a compilation step. Projects like webpack, Svelte, and ESLint have proven this approach production-viable. The lykn form `(defn add ^(-> number number number) [x y] (+ x y))` emits:

```javascript
/** @param {number} x @param {number} y @returns {number} */
function add(x, y) { return x + y; }
```

**Secondary: `.d.ts` generation for TypeScript consumers.** The compiler generates TypeScript declaration files alongside JS output, allowing TypeScript projects to consume lykn libraries with full type information. ADTs emit discriminated union types: `type Shape = { tag: "Circle"; radius: number } | { tag: "Rect"; w: number; h: number }`.

**Optional: strippable runtime assertions.** Type annotations can compile to runtime `typeof` checks that are removed in production builds, providing development-time safety without production overhead. This follows Clojure.spec's `*compile-asserts*` pattern.

### Concrete syntax proposals for type annotations

```
;; Function types (Coalton-style)
(declare add (-> number number number))
(defn add [x y] (+ x y))

;; Parametric types
(declare map (-> (-> :a :b) (List :a) (List :b)))

;; ADT definitions with types
(deftype (Result :ok :err)
  (Ok :ok)
  (Err :err))

;; Type classes
(define-class (Eq :a)
  (== (-> :a :a boolean)))

;; Occurrence typing (automatic)
(defn process [x]
  (cond
    (string? x) (str/upper x)     ;; x is String here
    (number? x) (* x 2)           ;; x is Number here
    :else       (str x)))          ;; x is unknown
```

---

## Prioritized safety roadmap

The following priority ordering weighs three factors: severity of the JS hazard addressed (from empirical data), feasibility of syntactic mitigation in an s-expression language, and alignment with Lisp design philosophy.

### Phase 1 — Foundation (addresses ~30% of empirical bugs)

- **Strict equality only** — eliminates BugAID pattern #4, Pradel's #1 harmful coercion. Cost: zero. Benefit: immediate.
- **Immutability by default with atoms** — eliminates mutation-class bugs, race conditions on shared state, stale closures. Proven by ClojureScript at scale.
- **No `this`, no `var`, no implicit globals** — eliminates BugAID patterns #10-11, closure-over-loop-variable, implicit global creation.
- **Separated `+`/`str`** — eliminates the most common type confusion. ClojureScript proves this works idiomatically.
- **Safe iteration** — eliminates for-in hazards, DLint's L2 checker findings.
- **Strict mode / ES modules only** — eliminates silent failures in non-strict mode.

### Phase 2 — Structural safety (addresses ~15% more)

- **Nil-safe threading macros** — targets BugAID's largest pattern group (dereferenced non-values).
- **Result/Option ADTs with exhaustive match** — replaces exception-based error handling for expected failures.
- **Contract macros** — targets BugsJS's top category (32% missing input validation).
- **Frozen module exports** — prevents prototype pollution through import mutation.
- **No eval** — eliminates critical security hazards.

### Phase 3 — Type system (addresses ~15% more)

- **Gradual typing with JSDoc output** — captures the 15% type-detectable bugs from Gao et al.
- **Occurrence typing** — makes type narrowing after predicates automatic and idiomatic.
- **`.d.ts` generation** — enables TypeScript ecosystem interop.
- **Strippable runtime assertions** — development-time safety for untyped code.

### Phase 4 — Advanced (addresses remaining edge cases)

- **Condition/restart system** — novel error handling paradigm for complex recovery scenarios.
- **Reader macros for validated literals** — targets string-content errors (Gao's second most common undetectable category).
- **Safe arithmetic library** — BigDecimal, checked overflow, integer-only operations.
- **ReDoS-safe regex validation** — compile-time analysis of regex patterns.
- **Effect tracking** — pure/effectful distinction, initially convention-level, graduating to compile-time with mature type system.
- **Spec-like generative testing** — runtime validation with automatic test data generation.

### What syntax cannot fix

Certain hazard categories resist syntactic mitigation entirely. **Specification errors** (55% of bugs per Gao et al.) require understanding programmer intent. **Async ordering bugs** can be reduced by better abstractions but not eliminated by syntax alone — Wang et al.'s finding that 77% of Node.js concurrency bugs resist synchronization-based fixes applies equally to language-level mitigations. **Supply chain attacks** operate at the ecosystem level, beyond any single language's control. **DOM-related bugs** (65% of client-side faults per Ocariza et al.) involve the browser API surface, not the language. **Tooling and configuration bugs** (27.8% of TypeScript-era bugs per Tang et al.) arise from ecosystem complexity rather than language design. Acknowledging these limits is essential: lykn should aim to eliminate the ~40–50% of bugs that are syntactically preventable while providing better abstractions (contracts, specs, effect tracking) for the remaining categories.

---

## Conclusion

The empirical evidence points to a clear design thesis for lykn: **the highest-leverage safety features are not types but structural constraints**. Immutability-by-default, separated operators, eliminated `this`, and nil-safe access chains collectively address more empirically measured bugs than a type system alone. Types remain important — the 15% detection rate from Gao et al. is worth capturing — but they should be the *third* phase, not the first.

The Lisp tradition offers something no other compile-to-JS approach provides: **macro-powered safety patterns that operate at the syntactic level without runtime cost**. Contracts, specs, condition/restart systems, and reader macros for validated literals can address bug categories that neither TypeScript's types nor Elm's purity reach — particularly the 32% of server-side bugs from missing input validation and the string-content errors that are opaque to all type systems. ClojureScript has already proven that a Lisp compiling to JavaScript with immutable-by-default semantics is both practical and effective. Lykn's opportunity is to push further by adding exhaustive pattern matching, gradual typing with JSDoc output for ecosystem compatibility, and compile-time guarantees that ClojureScript leaves to runtime convention. The s-expression surface is not merely syntactic preference — it is the prerequisite for the macro system that makes these safety innovations possible without runtime dependencies.
