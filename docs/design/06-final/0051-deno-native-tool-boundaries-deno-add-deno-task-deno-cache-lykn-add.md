---
number: 51
title: "Deno-Native Tool Boundaries (`deno add`, `deno task`, `deno cache`, `lykn add`)"
author: "Duncan McGreggor"
component: All
tags: [change-me]
created: 2026-05-03
updated: 2026-05-13
state: Final
supersedes: null
superseded-by: null
version: 1.1
---


# Deno-Native Tool Boundaries (`deno add`, `deno task`, `deno cache`, `lykn add`)

## Status

Proposed (pending Duncan/CDC review).

Scope expanded during 2026-05-04 CDC review from "`deno add`
adjudication" to "lykn-only-tooling vs deno-native-tooling
boundaries." The original framing was a single-tool yes/no question;
the resolved scope commits decisions on the four Deno-native tools
that abut the lykn-project layer (`deno add`, `deno task`, `deno
cache`, plus the `lykn add` future-work item).

## Context

- `docs/guides/14-no-node-boundary.md` ID-07 ("No `npm install` /
  `npm run` — Use `deno add` / `deno task`") recommends `deno add`
  and `deno task` as replacements for npm commands.
- `assets/ai/SKILL.md` "Before You Do Anything" Principle 1 explicitly
  bans `deno add`: the bypass table says
  `npm install <x>` → "add to `project.json` `imports`, let Deno
  cache it."
- These contradict each other inside the project's own ground-truth
  corpus.
- First surfaced in M2's inventory as `needs-adjudication`; carried
  forward through M3.5 and into Phase 2.
- Two coherent readings exist:
  - **Reading A (SKILL is canonical):** Lykn projects use
    `project.json` workspace-level import map. `deno add` writes to a
    package-level `deno.json`, which lykn's workspace resolution
    doesn't read for import-map entries. Recommending `deno add` for
    lykn projects misdirects users.
  - **Reading B (layer separation):** Guide 14 speaks to Deno-native
    projects (no Lykn); SKILL speaks to the Lykn-project layer. Both
    are correct in their respective layers, but need disambiguation.
- M2 CDC review's prior weak lean: Reading A. Guide 14's title is
  "No-Node Boundary" — its purpose is the boundary Lykn projects
  draw, which is the layer SKILL applies to.
- **Adjacent prior decision:** M9 / DD-48 (the V-08 import-macros fix)
  adopted `deno cache <specifier>` as the official offline-prefetch
  tool. The error messages emitted by the import-macros resolver
  point users at `deno cache` for offline scenarios. This DD makes
  that adoption explicit (Rule 3 below).

## Options analyzed

### Option A: SKILL is canonical; update guide 14

Update guide 14 ID-07 to match SKILL's directive:

- Replace `deno add` recommendation with "add to `project.json`
  `imports`."
- Add `deno add` to the MUST-AVOID table alongside `npm publish`
  (ID-27).
- Keep `deno task` as acceptable for project scripts defined in the
  user's `deno.json` (it doesn't conflict with the workspace import
  map).
- Document `deno cache` as the offline-prefetch tool (already adopted
  in DD-48).

**Pros:**

- One source of truth — SKILL and guide 14 agree.
- Matches philosophy Principle 2: Lykn projects use lykn-fronted
  operations. Adding dependencies is a project-level operation that
  should go through the project's import map, not Deno's per-package
  mechanism.
- Users of guide 14 ARE Lykn users (the guide is in the Lykn project's
  `docs/guides/`).
- Eliminates the contradiction entirely.

**Cons:**

- `deno add` is a real Deno capability that some users know and
  expect. Banning it may feel heavy-handed.
- The `project.json` imports approach is manual (no version
  resolution, no lock-file integration). `deno add` provides version
  resolution from JSR/npm. The manual approach is less ergonomic.
- Future work: when `lykn add` exists (a Lykn-fronted equivalent),
  this becomes cleaner. Until then, "edit project.json manually" is
  the user experience (mitigated by the worked example in the
  Implementation outline).

### Option B: Layer separation; update both

- Update guide 14 to scope its recommendations explicitly: "When
  working with Deno directly (without the Lykn CLI), use `deno add`
  for package management."
- Update SKILL Principle 1 to footnote: "The `deno add` ban applies
  to Lykn projects using `project.json` workspace imports. For
  Deno-native projects without Lykn, `deno add` is the correct tool."

**Pros:**

- Preserves more of guide 14's existing content.
- Honest about the two-layer reality (Deno-native vs Lykn-project).

**Cons:**

- Users must distinguish which layer they're in. In practice, Lykn
  users are ALWAYS in the Lykn-project layer — that's the whole point
  of guide 14 living inside the lykn project.
- Adds cognitive overhead: "am I in Deno-native mode or Lykn-project
  mode?"
- Guide 14 becomes conditionally-correct, which is harder to follow
  than unconditionally-correct.

### Option C: Clarify guide 14's audience (minimal)

Add a preamble to guide 14 stating its audience is Lykn projects,
then defer to SKILL for the authoritative guidance on each item.
Leave ID-07 as-is but add a note: "In Lykn projects, prefer editing
`project.json` `imports` directly (per SKILL.md Principle 1)."

**Pros:**

- Least invasive change.
- Acknowledges the contradiction without fully resolving it.

**Cons:**

- The contradiction still exists in the body of the guide. A user
  reading ID-07 without the preamble gets the wrong advice.
- Doesn't resolve the MUST-AVOID question (should `deno add` be in
  the MUST-AVOID table?).
- "Prefer" is weaker than SKILL's "Never" — leaving ambiguity.

## Decision

**Option A: SKILL is canonical; update guide 14.** Plus four
companion rules committing the layer boundary across all four
Deno-native tools.

### Rule 1: `deno add` is banned in lykn projects

`deno add` writes to `deno.json` in Deno's per-package format. Lykn
projects use `project.json` workspace-level imports, which Deno's
`deno add` does not read or write. Recommending `deno add` to Lykn
users misdirects them: the resulting state would not be honoured by
the workspace's import-map resolver.

The user-facing alternative is to edit `project.json` `imports`
directly. See the Implementation outline for the worked example.

### Rule 2: `deno task` is acceptable

`deno task` reads project scripts from `deno.json`'s `tasks` field.
This does not conflict with Lykn's workspace import-map mechanism —
it's a separate file, separate purpose. Lykn projects can use
`deno task` for project-specific scripts (linters, custom build
helpers, ad-hoc commands) alongside the lykn-fronted commands
(`lykn build`, `lykn test`, `lykn publish`). The guide should keep
`deno task` listed as acceptable; not promoted, not banned.

### Rule 3: `deno cache` is acceptable (and is the official offline-prefetch tool)

`deno cache <specifier>` populates Deno's global module cache without
modifying project configuration. It's an infrastructure command in
the same category as `git fetch` — it brings remote artefacts to the
local cache so offline compilation works.

DD-48 (the V-08 import-macros JSR/npm cache resolution fix) already
adopted `deno cache` as the official offline-prefetch tool. The
import-macros resolver's error messages explicitly point users at
`deno cache jsr:@scope/name` when the cache is empty and the network
is unreachable.

This DD makes that adoption explicit: `deno cache` is acceptable, is
not banned, and is the documented escape hatch for offline scenarios.
It is not promoted as a default workflow (online builds auto-cache on
first import; see "Online vs offline" below) — it's the offline
fallback.

### Rule 4: `lykn add` is tracked as future work

The ergonomic gap left by banning `deno add` (no version resolution,
no lockfile integration, manual edit of `project.json`) is a real
cost. The intended future answer is `lykn add` — a Lykn-fronted
command that writes to `project.json` `imports` with version
resolution from JSR/npm.

`lykn add` is **not** part of this DD's scope. It is logged as
future work for 0.6.x or later. The `deno add` ban does not block
on `lykn add` — the manual workflow is correct (if less ergonomic),
and shipping it now is preferable to leaving the contradiction in
place.

### Online vs offline caching

The phrase "let Deno cache it" in SKILL.md Principle 1 is correct
**for online builds**: Deno auto-caches modules on first import when
the network is reachable. For **offline builds**, users must prefetch
via `deno cache <specifier>` before the offline session — same
pattern as `git fetch` before working without network.

The Implementation outline below specifies that guide 14 ID-07 needs
to clarify both cases: online (auto-cached) and offline (prefetch via
`deno cache`).

### Why other options rejected

- **Option B:** Adds layer-distinction overhead for an audience
  that's always in one layer. Over-specified.
- **Option C:** Doesn't resolve the contradiction; leaves "prefer"
  vs "Never" ambiguity.

## Implementation outline

The actual edit scope is **smaller than the original DD draft
suggested**. Guide 14 ID-07 is just a heading + `**Strength**:
MUST-AVOID` tag — there is no body content today. The fix is three
small edits to one file plus optional supporting content.

### Edits to `docs/guides/14-no-node-boundary.md`

**Edit 1 (line 86 — ID-07 heading):**

Old:
```
## ID-07: No `npm install` / `npm run` — Use `deno add` / `deno task`
```

New:
```
## ID-07: No `npm install` — Use `project.json` `imports`
```

(`npm run` is removed from the heading since `deno task` is no
longer the recommended replacement; it stays acceptable but isn't
the primary cue. Lykn-fronted commands like `lykn test`, `lykn
build`, `lykn run` cover the common cases.)

**Edit 2 (line 86 body — add content):**

Add a body explaining the Lykn import-map approach and the
online/offline caching distinction:

```markdown
**Summary:** Lykn projects manage dependencies via the workspace-level
`project.json` `imports` map, not Deno's per-package `deno.json` or
`deno add`. The latter writes to a file Lykn's workspace resolver
doesn't read.

**Adding a dependency** — edit `project.json` directly:

```json
{
  "imports": {
    "@std/path": "jsr:@std/path@^1.0.0",
    "lodash": "npm:lodash@^4.17.21"
  }
}
```

Online builds: Deno auto-caches on first import. No prefetch needed.

Offline builds: prefetch the new dependency once before going
offline, using Deno's cache infrastructure command:

```sh
deno cache jsr:@std/path
```

`deno cache` is acceptable in lykn projects (Rule 3 of DD-51) — it's
infrastructure, not project configuration. `deno add` is **not**
acceptable (it writes to the wrong file).

**Counter-cue (read this if you're tempted to bypass):** `deno add`
is a real Deno command, but its target is `deno.json` per-package
imports, which lykn's workspace resolver doesn't honour. Editing
`project.json` directly is the correct workflow.
```

**Edit 3 (line 310 — summary table):**

Old:
```
| 07 | `npm install`/`npm run` | MUST-AVOID | `deno add`/`deno task` |
```

New:
```
| 07 | `npm install` | MUST-AVOID | edit `project.json` `imports`; `deno cache` to prefetch for offline |
```

**Edit 4 (line 343 — replacement table):**

Old:
```
| | `npm install` | `deno add` |
```

New:
```
| | `npm install` | edit `project.json` `imports` (offline: `deno cache <spec>`) |
```

**Edit 5 (MUST-AVOID side — add `deno add` row):**

Add a new ID entry banning `deno add` explicitly:

```markdown
## ID-NN: No `deno add` — Edit `project.json` `imports` Directly

**Strength**: MUST-AVOID

**Summary**: `deno add` writes to `deno.json` per-package imports,
which lykn's workspace resolver does not read. Edit `project.json`
`imports` directly. See ID-07 for the workflow.
```

(CC chooses the next available ID number when implementing.)

### Edits to `assets/ai/SKILL.md`

**Edit 6 (Principle 1 bypass-table line 51):**

The current entry already matches the Decision:

```
| `npm install <x>` | add to `project.json` `imports`, let Deno cache it |
```

Augment with the offline note:

```
| `npm install <x>` | add to `project.json` `imports` — Deno auto-caches online; for offline use `deno cache <spec>` |
```

(Verify the exact phrasing fits the existing table format. If not,
adjust the cell content while preserving the table's column
structure.)

### Optional: Philosophy doc decided-question entry

Per the pattern of M3.5 / M5 / M9 closing reports, significant
tooling decisions get a one-line entry in `docs/philosophy.md`'s
"Decided design questions" list. Add:

```markdown
N. **`deno add` is banned in lykn projects; `deno task`/`deno cache`
   are acceptable.** Lykn projects use `project.json` workspace-level
   imports, not Deno's per-package `deno.json` mechanism. `deno cache`
   is the offline-prefetch tool (per DD-48). `lykn add` is tracked as
   future work for ergonomic dependency addition; the absence of
   `lykn add` does not block the `deno add` ban.
```

(The exact entry number depends on what's already in the list at
implementation time.)

### No source code changes

This is purely documentation alignment. No compiler, CLI, or test
changes.

## Relationship to philosophy

- **Principle 2 (lykn-only tooling):** `deno add` is a Deno-native
  tool that writes to files Lykn doesn't own. Banning it for Lykn
  projects is consistent with the principle. `deno task` and `deno
  cache` are acceptable because they don't conflict with Lykn-owned
  configuration — `deno task` reads user scripts, `deno cache`
  populates the global module cache.
- **Future `lykn add`:** When implemented, `lykn add` will write to
  `project.json` `imports` with version resolution from JSR/npm. This
  closes the ergonomic gap. The DD-51 ban does not depend on `lykn
  add` shipping first; the manual workflow is correct now and
  improves later.

## Future work / backlog

- **`lykn add`** — Lykn-fronted dependency addition. Writes to
  `project.json` `imports` with JSR/npm version resolution. Tracked
  for 0.6.x or later. Not blocking on this DD.
- **`lykn cache`** — possible Lykn-fronted wrapper around `deno
  cache` for consistency with `lykn build`/`lykn test`/`lykn run`
  patterns. Open question for future design; `deno cache` works
  today.
- **Lock-file integration** — when `lykn add` lands, lock-file
  semantics (akin to `package-lock.json` or `deno.lock`) need
  designing. Out of scope for DD-51.

## What this DD did NOT cover

- The future `lykn add` design (deferred — Rule 4 logs it as
  future work).
- Lock-file semantics for lykn projects (deferred — see Future
  work).
- General `deno.json` vs `project.json` boundaries beyond the four
  tools named in this DD (separate concern).