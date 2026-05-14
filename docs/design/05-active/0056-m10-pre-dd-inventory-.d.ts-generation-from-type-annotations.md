---
number: 56
title: "`.d.ts` Generation from `:type` Annotations"
author: "default for"
component: All
tags: [change-me]
created: 2026-05-13
updated: 2026-05-13
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# M10 Pre-DD Inventory — `.d.ts` Generation from `:type` Annotations

**Status:** Resolved — calls landed 2026-05-13. Ready for implementation.
**Thread:** cdc/dep-ergonomics
**Phase 2 plan reference:** M10 in `workbench/phase-2-plan.md` (2–4 iterations estimated)
**Bootstrap report tie-in:** issue #8 ("JSR publish requires `--allow-slow-types`" + "Feature: generate `.d.ts` from type annotations")
**Foundational reference:** DD-19 (Phase 3 — "`.d.ts` generation for TypeScript consumers"); guide 05 ID-20.

## Resolved design calls

All ten design questions resolved per Duncan's calls 2026-05-13. Pattern A confirmed: `.js` + `.d.ts` siblings (NOT switching to `.ts`-first output). Lykn is ECMAScript-2025-aligned, not TypeScript-aligned; `.d.ts` is auxiliary type information for TS consumers; `.js` remains the canonical compiled output.

| # | Question | Call | Detail |
|---|---|---|---|
| Q1 | `:any` → TS type | **A — `unknown`** | TS-idiomatic for library APIs; consumers narrow before use. |
| Q2 | `:array` element-type refinement | **A — `unknown[]`** | Bare `:array` → `unknown[]`; destructured array → typed tuple from the pattern's per-element types. |
| Q3 | `:object` shape inference | **A — `object`** | Bare `:object` → TS `object`; destructured object → shaped type `{ field: T; ... }`. |
| Q4 | `:function` first-class signatures | **A — `Function`** | Lossy but consistent. Typed callback support is Phase 3+ surface-syntax work. |
| Q5 | Undeclared `:returns` on exported funcs | **D — Warning + fallback to `unknown`** | Build emits warning; `.d.ts` gets `unknown` return type. Discipline shift toward declared return types for exports. Gentle (not hard error) for existing code. |
| Q6 | Hook point in build pipeline | **A — Emit `.d.ts` during `lykn compile`** | Parallel to `.js` emission. M11's `target/lykn/build/<pkg>/` accommodates the new sibling files. |
| Q7 | Opt-in vs opt-out | **C — Opt-out via `lykn.emitDts: false`** | On by default for packages with `:type` annotations; library authors disable per-package via `deno.json`. |
| Q8 | JSDoc-in-JS vs `.d.ts` | **A — `.d.ts` only for M10** | JSDoc-in-JS is future work; out of M10 scope. |
| Q9 | Multi-clause `func` | **A — TS overloads** | Multiple function declarations with same name; verbose but accurate. Standard TS library shape. |
| Q10 | `:pre`/`:post` conditions | **B — JSDoc `@requires`/`@ensures` tags** | Above the TS signature in the `.d.ts`. Documents intent without TS-typing implications. |

## Refinement log

- **2026-05-12 (drafting):** Pre-DD inventory written with 10 questions and proposed defaults.
- **2026-05-13 (calls):** Duncan reviewed; accepted all ten defaults. Substantive discussion on Pattern A vs B (TS-first output) before Q1; Pattern A confirmed because lykn is ECMAScript-aligned, not TypeScript-aligned. The `.d.ts` is auxiliary type information for the TS consumer story, not the primary publication artifact.

---

## Why a pre-DD memo (not a DD yet)

M10 has more design surface than DD-50.7 or DD-52. Six to ten design questions with multiple defensible answers each; type-mapping choices that compound; pipeline-hook decisions that interact with M11's just-landed `target/lykn/build/` and `target/lykn/dist/` structure. A DD that tries to answer them all in one pass produces a long writeup with weak commitment density.

Better shape: this memo surfaces the questions and proposed defaults; Duncan/CDC review and call them or push back; then a focused DD with the calls baked in proceeds to draft → implementation. Same pattern as DD-50.6's pre-implementation prompt — design first, scope second.

---

## What M10 delivers

A `.d.ts` file emitted alongside the compiled `.js` in `target/lykn/dist/<pkg>/` for any lykn package with `:type` annotations on exported forms. Consumed by TypeScript projects importing the lykn package; gives them full autocomplete, type-narrowing, and `tsc`-level checks against the lykn-exported API surface.

What this does NOT deliver:
- Source-map generation (separate; tracked elsewhere; depends partly on .d.ts shape).
- JSDoc-in-JS annotation (DD-19's "primary" option; held as future work — see Q9 below).
- TypeScript-source `.lykn` interop (lykn-from-TS imports work today via plain `.js`; this is enriching what TS sees, not adding a new direction).
- A TS-source-aware LSP (philosophy.md §Decided design questions #5 — separate Phase 3 work).

---

## Current substrate (what's already in the codebase)

The Rust compiler has structured AST that captures everything `.d.ts` generation needs:

- `TypeAnnotation { name: String, span }` — keyword names like `"number"`, `"string"`, `"Option"`. Source: `crates/lykn-lang/src/ast/surface.rs:5`.
- `TypedParam { type_ann, name, name_span, default_value, is_rest }` — function parameter shape.
- `ParamShape::{Simple, DestructuredObject, DestructuredArray}` — nested destructured patterns with inner type annotations.
- `FuncClause { args, returns, pre, post, body, span }` — function signature, possibly one of several in a multi-clause `func`.
- `TypeDef { name, module_path, constructors }` — ADT definitions.
- `ConstructorDef { name, fields, owning_type, span }` — ADT variants.
- `FieldDef { name, type_keyword }` — ADT variant fields.
  - Source: `crates/lykn-lang/src/analysis/type_registry.rs:7-32`.

Build pipeline hook: `dist::build_dist(Path::new("."))` invoked from `crates/lykn-cli/src/main.rs` (post-M11 location).

Surface forms supporting `:type` annotations (per `docs/guides/00-lykn-surface-forms.md:185–360`):
- `func` / `fn` / `lambda` / `=>` — typed args + returns
- `bind` — typed value (`:bind :number x 42`)
- Multi-clause `func` — overloaded on arity/types
- Destructured `:args` — object/array patterns with per-field typing
- Default values — `(default :type name value)` inside destructured patterns
- Aliased destructured — `(alias :type alias-name pattern)`
- Generators — `:yields :type` for per-yield type checks
- `type` ADT declarations — `(type Option (Some :any value) None)`
- Pre/post conditions — `:pre <expr>` / `:post <expr>` (orthogonal to types but co-located on `func`)

---

## Proposed type-mapping table

| lykn surface | TypeScript | Notes |
|---|---|---|
| `:number` | `number` | |
| `:string` | `string` | |
| `:boolean` | `boolean` | |
| `:function` | `Function` | Lossy; no signature info. See Q4. |
| `:object` | `object` | For destructured: `{ field: type; ... }` |
| `:array` | `unknown[]` | For destructured: tuple `[type, type, ...]` |
| `:symbol` | `symbol` | |
| `:bigint` | `bigint` | |
| `:any` | `unknown` | TS-idiomatic. See Q1. |
| `:void` | `void` | |
| `:UserType` | `UserType` | Discriminated union from `(type ...)` |
| `:promise` (if supported) | `Promise<unknown>` | Verify whether this is a lykn type. |

ADT example — `(type Option (Some :any value) None)`:

```typescript
export type Option =
  | { tag: "Some"; value: unknown }
  | { tag: "None" };
```

Multi-clause `func` — TypeScript overloads:

```lykn
(func greet
  (:args (:string name) :returns :string :body ...)
  (:args (:string g :string name) :returns :string :body ...))
```

```typescript
export function greet(name: string): string;
export function greet(g: string, name: string): string;
```

Destructured `:args` — emitted as a single param with structured type:

```lykn
(func render-config
  :args ((object :string host :number port (default :boolean ssl true)))
  :returns :string :body ...)
```

```typescript
export function renderConfig(arg: {
  host: string;
  port: number;
  ssl?: boolean;
}): string;
```

(With `ssl?:` because `default` makes the field optional from the caller's perspective.)

---

## Design questions for Duncan/CDC

### Q1: `:any` → `unknown` or `any`?

`:any` in lykn means "no runtime type check." In TS terms, `unknown` is the type-safe equivalent ("could be anything, must be narrowed before use"); `any` is the type-unsafe equivalent ("skip the type checker").

**Options:**

- **A. `unknown`** (TS-idiomatic, type-safe). TS consumers of lykn packages will need to narrow before using `:any`-typed values. This matches what they'd do anyway for runtime-validated input. Recommended by TS style guides for library APIs.
- **B. `any`** (TS-loose). Matches lykn's "no check" semantics literally. TS consumers can use values freely without narrowing.
- **C. Configurable per-package via `deno.json`** `"lykn": { "anyAs": "unknown" | "any" }`. Library authors choose for their consumers.

**Proposed default: A (`unknown`).** Matches TS library-author idiom. Library authors who insist on `any` are an edge case; we can add the config flag later if real demand surfaces. **Trade-off:** TS consumers see more "type narrowing required" friction, but the API is more correct.

### Q2: `:array` — element-type refinement?

Bare `:array` annotation carries no element type info. The destructured array form (`(array :number head (rest :number tail))`) does.

**Options:**

- **A. Bare `:array` → `unknown[]`; destructured array → typed tuple/rest.** Loses element typing for bare `:array`; preserves it where the source has it.
- **B. Bare `:array` → `unknown[]`; analyze surrounding code to infer element type.** Compiler-side inference; complex and potentially wrong.

**Proposed default: A.** No inference. Library authors who want typed arrays use destructured patterns or accept `unknown[]`.

### Q3: `:object` — shape inference?

Bare `:object` annotation gives no field info. Destructured object form does.

**Options:**

- **A. Bare `:object` → `object`; destructured → shaped type.** Same shape as Q2.
- **B. Bare `:object` → `Record<string, unknown>`** (TS-narrower). Slightly more useful than bare `object` for some consumers; still no field-level info.

**Proposed default: A.** Symmetric with Q2. `Record<string, unknown>` is also defensible if Duncan prefers; minor.

### Q4: `:function` — first-class signatures?

`:function` annotation carries no signature info. TS `Function` is essentially `(...args: any[]) => any` — almost useless for typed consumers.

**Options:**

- **A. `:function` → `Function`.** Lossy but consistent with lykn surface.
- **B. Introduce a new lykn annotation form for typed function values, e.g., `:fn(:number -> :string)`.** New surface syntax; requires DD scope expansion.
- **C. Inspect surrounding context (callbacks passed to `:args`) and infer signature from usage.** Complex; potentially wrong.

**Proposed default: A.** Don't expand surface syntax in M10's scope. If users want typed callbacks, they can use ADTs or accept lossy `Function`. **Future:** option B as a Phase 3+ enhancement.

### Q5: Undeclared `:returns` — `unknown` or refuse to emit?

Some `func` declarations don't have `:returns`. Per DD-50.6 Q1=A, body's last expression's value flows out (or `undefined` if statement-only). What does the `.d.ts` say?

**Options:**

- **A. `unknown`** — generic "you can use the result but check before relying on it."
- **B. `void`** — assume statement-only body intent.
- **C. `unknown | void`** — explicit "could be either."
- **D. Refuse to emit `.d.ts` for the function; emit a warning during build.** Forces library authors to declare `:returns` for exported functions.

**Proposed default: D for exported functions; A for non-exported.** Exported APIs deserve explicit return types — that's the API surface library authors should design intentionally. Non-exported helpers can stay loose. The build-time warning gives concrete feedback. **Caveat:** this is a behavioral discipline shift; ensure DD calls it out clearly so library authors know what's expected.

### Q6: Hook point in build pipeline

The `lykn build --dist` pipeline (post-M11) stages packages into `target/lykn/dist/<pkg>/`. M10 adds a `.d.ts` emission step somewhere in this flow.

**Options:**

- **A. Emit `.d.ts` during `lykn compile` (alongside `.js`).** Files appear in `target/lykn/build/<pkg>/`. `lykn build --dist` then stages both. Clean separation.
- **B. Emit `.d.ts` only during `lykn build --dist`.** Faster `lykn compile` (no `.d.ts` overhead for inner-loop work); `lykn build --dist` does the full pass. Build-dist becomes more substantial.
- **C. Both — `lykn compile` produces fast lossy `.d.ts`; `lykn build --dist` produces canonical `.d.ts`.** Two passes; complex; unclear value.

**Proposed default: A.** Symmetric with `.js` emission. Inner-loop overhead is small (walking already-parsed AST). Easier to reason about. M11 lands the build dir; M10 fits cleanly into it.

### Q7: Opt-in vs always-on?

Should every lykn package emit `.d.ts` by default, or only packages that opt in?

**Options:**

- **A. Always-on for any package with `:type` annotations.** Zero-config; users who don't want `.d.ts` annotate less or use `:any` everywhere.
- **B. Opt-in via `deno.json` `"lykn": { "emitDts": true }`.** Explicit; users acknowledge they want `.d.ts`.
- **C. Opt-out via `deno.json` `"lykn": { "emitDts": false }`.** On by default; users disable when they don't want it.

**Proposed default: C (opt-out).** TypeScript-consumer-friendliness is a goal — turning it on by default gives downstream TS users the best experience. Opt-out is escape hatch for the rare lykn package that ships to non-TS consumers and doesn't want `.d.ts` overhead.

### Q8: JSDoc-in-JS vs separate `.d.ts`?

DD-19 named JSDoc-in-JS as "primary" (for `tsc --checkJs`) and `.d.ts` as "secondary" (for TypeScript-source consumers). Both are valuable; both are work.

**Options:**

- **A. `.d.ts` only for M10; JSDoc as future work.** Smaller M10 scope; the more common "consumer is a TS project that imports the published package" path is covered.
- **B. JSDoc only for M10; `.d.ts` as future work.** Lighter; integrates with `tsc --checkJs` for projects that don't generate TS source. But the dominant consumer pattern is `.d.ts`-using TS projects.
- **C. Both.** Doubles complexity; defensible if both are valuable enough to warrant the cost.

**Proposed default: A (`.d.ts` only).** The dominant consumer pattern is `.d.ts`. JSDoc is a follow-up if demand surfaces.

### Q9: Multi-clause functions — overloads vs union?

When `func` has multiple `:args`/`:returns` clauses (overloaded on arity/types), the TS representation has two natural shapes.

**Options:**

- **A. TS function overloads.** Verbose but accurate. Standard TS library shape.
- **B. Single signature with union types.** More compact: `(name: string, g?: string) => string`. Less accurate when arg structures differ.
- **C. Both — overloads in `.d.ts`; union in JSDoc.** If we go with Q8=C eventually.

**Proposed default: A.** Standard library TS shape. The added verbosity is the right cost for accuracy.

### Q10: Pre/post conditions — preserve in `.d.ts`?

`:pre` and `:post` conditions are lykn-specific contract annotations. TS has no analog.

**Options:**

- **A. Drop entirely from `.d.ts`.** Contracts are runtime, not type-level. TS sees the function signature only.
- **B. Emit as JSDoc `@requires` / `@ensures` tags above the TS signature.** Documents intent; doesn't enforce.
- **C. Comment block above the signature.** Plain text; preserves intent without TS analog.

**Proposed default: B.** Documents intent in a TS-tooling-friendly way (JSDoc tags). Doesn't bleed into types. Implementation: emit `/** @requires <pre> @ensures <post> */` above each function signature. Light, useful, low risk.

---

## Iteration estimate

3 iterations (within Phase 2 plan's 2–4 range):

- **Iter 1: DD finalized + type-mapping implementation.**
  - DD with the 10 questions resolved.
  - Rust-side `.d.ts` emitter walking `TypeDef`, `FuncClause`, exported `bind`s.
  - Type-mapping table implemented as a function.
  - Hook point in pipeline established (Q6=A).
- **Iter 2: ADTs, destructured patterns, multi-clause, edge cases.**
  - ADT emission per Q-section above.
  - Destructured `:args` → structured types.
  - Multi-clause → overloads (Q9=A).
  - Pre/post → JSDoc tags (Q10=B).
- **Iter 3: Scaffold, opt-out config, tests, real-downstream gate.**
  - `lykn new` scaffold reflects opt-out config (Q7=C).
  - `deno.json` `lykn.emitDts` flag wired up.
  - Tests: synthetic per-pattern + real-downstream against mycelium.
  - Real-downstream gate: compile mycelium's mycl-html through `lykn build --dist`, verify generated `.d.ts` passes `tsc --noEmit`.

---

## Risk profile

- **Higher design surface than DD-50.7 / DD-52.** Ten design questions, several with multiple defensible answers. Pre-DD step is load-bearing.
- **Moderate implementation surface.** Mostly AST-walking + string formatting. No new architectural primitives.
- **Cross-cutting awareness:** must integrate cleanly with M11's just-landed `target/lykn/build/` structure. Must not regress `lykn publish` flow.
- **Empirical validation gate (per the DD-50.7 methodology):** real-downstream test in iter 3 (mycelium's `.d.ts` passes `tsc --noEmit`).

---

## What this memo does NOT cover

- JSDoc-in-JS (Q8 deferred to A — `.d.ts` only for M10).
- Source-map generation (separate; depends partly on `.d.ts` shape).
- TS-source-aware lykn LSP (Phase 3+ per philosophy.md).
- `tsc --noEmit`-as-a-test-gate in the lykn test harness itself (the gate is invoked manually in iter 3 per closing report).

---

## Discussion points for Duncan

The ten design questions above are the substantive calls. Each has a proposed default; flag any you'd call differently. Trade-offs worth surfacing:

- **Q1 (`:any` → `unknown` vs `any`):** affects every TS consumer's narrowing burden. Defaults are defensible either way; lean strongly to `unknown` for library-author hygiene.
- **Q5 (undeclared `:returns` → warning vs accommodation):** behavioral discipline shift. Library authors who don't declare `:returns` on exports get a warning. Worth Duncan's call on tone (warning vs hard error).
- **Q7 (opt-in vs opt-out):** philosophical — is `.d.ts` emission default-on TS-friendly behavior, or default-off lykn-purist behavior? Lean opt-out (default-on) for downstream-consumer experience.
- **Q10 (pre/post in `.d.ts`):** preserves contract intent; doesn't enforce types. JSDoc tags are the least-intrusive way to surface this; defensible to drop entirely if Duncan prefers minimal `.d.ts`.

If the proposed defaults are accepted, a DD draft can be next. If any are called differently, the DD updates first.

After DD finalization, an implementation prompt patterned on DD-50.6 / DD-50.7 follows; iterations land per the 3-iter outline above; closing report includes the real-downstream `tsc --noEmit` gate.

---

## Sequencing with the rest of dep-ergonomics scope

M10 can run independently of:
- DD-50.7 (Finding E) fix — orthogonal compiler concern; no overlap.
- DD-52 (surface-macros gap) — orthogonal expander concern; no overlap.
- Finding D (exports field) — both touch package publishing but at different layers.

Order recommendation:
1. **DD-50.7 first** (blocks 0.6.0 ship per Duncan's call).
2. **DD-52 second** (future-foundational for mycelium per Duncan's framing).
3. **M10 third** (largest, highest user-facing value, no current blocker).
4. **Finding D fix** can land anywhere — it's a 2-line patch; opportune with whatever release lands first.

All four can ship as part of 0.6.0 if DD-50.7's fix is the gating item and 0.6.0 doesn't ship until that's done. If Duncan wants a 0.5.3 patch for any of them, Finding D + DD-52 are the small enough candidates.
