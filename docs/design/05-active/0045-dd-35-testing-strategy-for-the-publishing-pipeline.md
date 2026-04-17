---
number: 45
title: "DD-35: Testing Strategy for the Publishing Pipeline"
author: "publishing to"
component: All
tags: [change-me]
created: 2026-04-17
updated: 2026-04-17
state: Active
supersedes: null
superseded-by: null
version: 1.0
---

# DD-35: Testing Strategy for the Publishing Pipeline

**Status**: Decided
**Date**: 2026-04-17
**Session**: Publishing infrastructure design conversation (2026-04-17),
follow-up to DD-33 and DD-34
**Depends on**: DD-30 (testing DSL), DD-31 (test runner CLI),
DD-33 (publishing and `dist/` boundary), DD-34 (cross-package
`import-macros` resolution)
**Blocks**: Implementation completion of DD-33 Phase 2+ and all of
DD-34

## Summary

The publishing pipeline introduced by DD-33 and DD-34 is tested
through **five layers**, each catching a different class of failure:
unit tests for pure logic, integration tests with real Deno against
local fixtures, snapshot tests for generated configs, external
consumer smoke tests, and Deno-version compatibility tests. Each
DD-33/DD-34 implementation phase has explicit "done" criteria tied
to specific test cases. Synthetic fixture packages provide per-kind
coverage; the three real `@lykn/*` packages provide realism
coverage. The test suite deliberately does **not** include a local
registry simulation at this stage; `--dry-run` is trusted as the
authoritative acceptance check, and a separate consumer-project
repository provides the realistic post-publish verification.

## Motivation

1. **Publishing pipelines are famous for "works on my machine"
   failures.** The gap between "my build produces a `dist/`
   directory" and "a downstream consumer can actually use my
   published package" is exactly where publishing bugs live.
   Without deliberate test layering, the first time we discover a
   bug is when a real user tries to consume `@lykn/testing`.

2. **DD-34's resolver has an irreducible dependency on Deno's
   behaviour.** Tests for the three-tier resolution must prove not
   only that our code is correct, but that our code correctly
   interprets what Deno does. Deno is an external dependency
   whose behaviour can drift between versions.

3. **Unit tests alone are insufficient; real-registry tests alone
   are too expensive.** Unit tests miss integration bugs (wrong
   `deno.json` fields, missing files, mis-rewritten imports).
   Real-registry tests are slow, flaky, and can't run on every
   commit. A layered strategy puts the cheap tests in the hot
   loop and the expensive tests at release boundaries.

4. **DD-33 and DD-34 are multi-phase implementations with clear
   phase boundaries.** Specifying which tests gate which phase
   turns "when is this phase done?" into a concrete, reviewable
   question.

5. **Test infrastructure is itself code.** Decisions about fixture
   location, snapshot format, CI integration, and tool choice
   need to be made deliberately, not drift into the codebase
   through a series of small PRs.

## Decisions

### 1. Five-layer test strategy

**Decision**: Tests for the publishing pipeline are organized into
five numbered layers, each with a defined scope, speed, and
failure class.

| Layer | Scope | Speed | Runs on |
|-------|-------|-------|---------|
| **1. Unit** | Pure logic: config parsing, resolver dispatch, import rewriting, error formatting | <10s total | Every commit |
| **2. Integration (local)** | Full pipeline against synthetic and real fixture packages, with real Deno, no network | <90s total | Every commit |
| **3. Snapshot** | Golden-file tests of generated `deno.json` and `package.json` | <5s total | Every commit |
| **4. Consumer smoke** | External repo consuming `@lykn/*` packages from real JSR/npm | ~2min | Release candidates, weekly cron |
| **5. Deno compatibility** | Resolver assumptions verified against supported Deno versions | ~30s per version | Deno version bumps, weekly cron |

Each layer is additive; each DD-33 or DD-34 implementation phase
declares which layers it adds to (Decision 7).

**Rationale**: The layers correspond to genuinely different failure
classes. Unit tests catch "I wrote the wrong `if` branch."
Integration tests catch "I forgot to copy `README.md` into
`dist/`." Snapshot tests catch "I accidentally dropped a field
from the generated `package.json`." Consumer smoke tests catch
"JSR strips files matching this pattern and now my package doesn't
work." Deno compatibility tests catch "Deno 1.47 changed the path
of the npm cache." A single test tier cannot catch all five.

### 2. Unit tests (Layer 1)

**Decision**: Unit tests live in standard Rust `#[cfg(test)]`
modules alongside their source. They exercise pure functions with
in-memory inputs, use `tempfile::tempdir()` for filesystem
scenarios, and never spawn a Deno subprocess.

**Required coverage** (the implementation is complete when these
pass):

| Test case | Gates which phase |
|-----------|-------------------|
| `PackageConfig` deserializes every field shape including optional `lykn.macroEntry` | DD-33 Phase 1 |
| `ProjectConfig` workspace array parsed correctly (array, single string, empty) | DD-33 Phase 1 |
| `ImportMap` exact-match lookup | DD-33 Phase 1 / DD-34 Phase 2 |
| `ImportMap` prefix-match lookup (`"foo/"` matches `"foo/bar"`) | DD-34 Phase 2 |
| `ImportMap` cycle detection (max depth 8) | DD-34 Phase 2 |
| `PackageKind` inference (`.lykn` present → runtime, else tooling) | DD-33 Phase 2 |
| Per-kind staging produces expected file list (dry-run, mock FS) | DD-33 Phase 2 |
| Generated `mod.js` stub for macro modules exports `VERSION` | DD-33 Phase 2 |
| Import rewriter handles every shape in the "torture test" fixture | DD-33 Phase 3 |
| `FakeDenoSubprocess` + resolver: tier 1 dispatches on scheme prefixes | DD-34 Phase 3 |
| `FakeDenoSubprocess` + resolver: tier 2 consults import map | DD-34 Phase 3 |
| `FakeDenoSubprocess` + resolver: tier 3 falls through to filesystem | DD-34 Phase 3 |
| `macroEntry` fallback chain: explicit field → `mod.lykn` → `macros.lykn` → `index.lykn` → `exports` → error | DD-34 Phase 3 |
| `ResolutionError` for each failure mode renders the expected tiers in the message | DD-34 Phase 4 |

**Import-rewriter torture fixture** (committed under
`test/fixtures/publishing/import-rewriter-input.js`):

```javascript
import { a } from 'lang/reader.js';
import { b } from "lang/compiler.js";
import * as c from 'lang/namespace.js';
import { d as da, e as ea } from "lang/aliased.js";
import {
  multi,
  line,
} from 'lang/multiline.js';
/* import { ignored } from 'lang/in-comment.js'; */
// import { also } from 'lang/line-comment.js';
const template = `import { x } from 'lang/in-template.js';`;
export { reexport } from 'lang/reexport.js';
```

All `lang/` specifiers except the two in comments and the one in
the template literal must be rewritten to `@lykn/lang/`; the
commented and template-embedded specifiers must be preserved
verbatim. This fixture's expected output is a committed snapshot
(Layer 3).

**Rationale**: Layer 1 catches the bulk of logic errors with
millisecond runtime. The enumerated test cases serve as a concrete
completion criterion for each implementation phase, and the torture
fixture prevents anyone from ever "simplifying" the parser-based
import rewriter back to string replacement.

### 3. Integration tests with real Deno (Layer 2)

**Decision**: Layer 2 tests live under `test/integration/publishing/`
and exercise the full pipeline against real fixture packages using
a real Deno subprocess. They never contact the network and never
publish to real registries.

**Directory structure**:

```
test/integration/publishing/
  fixtures/
    synthetic/
      pkg-runtime-minimal/         # smallest valid runtime package
      pkg-runtime-with-imports/    # uses workspace imports
      pkg-macro-module/             # synthetic macro-module package
      pkg-tooling/                  # synthetic tooling package
      pkg-invalid-no-config/        # error-path fixture
    consumer-local/                 # consumes fixtures via file: specifiers
      project.json
      packages/app/mod.lykn
  scenarios/
    build_each_kind.rs              # per-kind build correctness
    dry_run_publish.rs              # deno publish + npm pack dry runs
    cross_package_consumption.rs    # consumer resolves fixtures
    real_packages_build.rs          # @lykn/lang, browser, testing
```

**Required scenarios**:

1. **Per-kind build**: for each fixture kind (`runtime`,
   `macro-module`, `tooling`), `lykn build --dist` produces a
   `dist/<pkg>/` matching the committed expectation.

2. **JSR dry-run acceptance**: `deno publish --dry-run --config dist/project.json`
   exits zero for every valid fixture and non-zero for every
   invalid fixture with the expected error category.

3. **npm dry-run acceptance**: `npm pack --dry-run` inside each
   `dist/<pkg>/` produces a tarball file list matching the
   expectation.

4. **Cross-package consumption via `file:`**: the consumer fixture
   uses `(import-macros "file:<path-to-built-dist>" ...)` and
   `lykn compile` succeeds with expected output.

5. **Real packages round-trip**: the three real workspace members
   (`@lykn/lang`, `@lykn/browser`, `@lykn/testing`) build cleanly
   and dry-run-publish to both JSR and npm.

**Fixture strategy**: synthetic fixtures provide minimal,
purpose-built coverage per kind — they do not change when real
packages evolve. Real packages provide realism coverage — they
catch issues that synthetic minimalism misses. Both are required.

**Rationale**: Layer 2 is the primary defense against integration
bugs and the place where most high-value tests live. Using real
Deno (not a mock) is non-negotiable here because Deno's actual
behaviour is what we need to verify against. `--dry-run` gives
authoritative "would this be accepted?" answers without network
dependency.

### 4. Snapshot tests for generated configs (Layer 3)

**Decision**: Layer 3 uses the `insta` crate for golden-file
snapshot testing. Every code path that generates a config file
(`dist/<pkg>/deno.json`, `dist/<pkg>/package.json`,
`dist/project.json`, the `mod.js` stub for macro modules) has a
committed snapshot file.

**Structure**: snapshots committed under `test/snapshots/` with
`.snap` extension, reviewed as part of normal PR review.

**Tool choice**: `insta` (adding it as a dev-dependency to
`lykn-cli`). Rationale for locking in this specific tool:

- Standard in the Rust ecosystem; widely understood.
- `cargo insta review` workflow makes snapshot updates reviewable
  rather than auto-accepted.
- Integrates with `cargo test` — no separate runner.
- Alternatives (`expect-test`, hand-rolled string comparison) lack
  the review workflow.

**Required snapshots**:

| Snapshot | Source |
|----------|--------|
| `runtime_pkg_deno_json` | Generated `dist/<runtime-pkg>/deno.json` |
| `runtime_pkg_package_json` | Generated `dist/<runtime-pkg>/package.json` |
| `macro_module_deno_json` | Generated `dist/<macro-pkg>/deno.json` with `lykn.*` preserved |
| `macro_module_package_json` | Generated `dist/<macro-pkg>/package.json` |
| `macro_module_mod_js_stub` | Generated `dist/<macro-pkg>/mod.js` VERSION stub |
| `workspace_project_json` | Generated `dist/project.json` |
| `import_rewriter_output` | Result of rewriting the Layer 1 torture fixture |

**Rationale**: Config generation is deterministic, high-churn, and
a place where "helpful cleanups" silently break downstream
consumers. Snapshot tests make every change to generated output
visible in code review. This is orthogonal to Layer 1 (which
verifies logic) and Layer 2 (which verifies acceptance) — it
verifies *exact output*, which neither other layer does.

### 5. Consumer smoke tests (Layer 4)

**Decision**: A separate repository (`oxur/lykn-consumer-tests` or
equivalent, TBD on final naming) contains consumer projects that
actually depend on published `@lykn/*` packages. This repo runs
**after** publishing, not as part of lykn's own PR checks.

**Structure**:

```
lykn-consumer-tests/
  pin-latest/                    # tracks the latest published version
    project.json
    packages/demo/
      mod.lykn                    # uses (import-macros "jsr:@lykn/testing" ...)
      mod_test.lykn
  pin-0.5.0/                     # pinned to a specific version
    ...
  pin-0.6.0/                     # added when 0.6.0 releases
    ...
  ci/
    run-smoke.sh                  # orchestrates lykn test across every pin-*
```

**What `run-smoke.sh` does for each pin**:

1. `cd pin-<version>`
2. `lykn build --dist`
3. `lykn test`
4. Assert exit code zero and expected output fragments present.

**When it runs**:

- **Release candidate workflow**: before a real publish, publish
  to a `@lykn-rc/*` scoped staging namespace (or equivalent),
  point `pin-latest` at the RC, run the smoke. If green, promote
  the RC to real.
- **Weekly cron**: run the entire consumer repo against whatever
  is currently on JSR/npm, catching external regressions (Deno
  updates, registry behaviour changes) between releases.

**Release gating**: the consumer-smoke for the current release
candidate MUST pass before promoting that RC. This is the bet-
the-farm test.

**The `@lykn/testing` dogfood pin**: `pin-latest` must include a
project that consumes `@lykn/testing` via JSR and uses its macros
in real test code. If DD-33 and DD-34 have failed, this pin will
catch it.

**Rationale**: Layer 4 is the only layer that tests against the
real registry infrastructure. It is slow and gated, but necessary
— without it, every release is a gamble. Running the same tests
on cron catches external drift even when lykn itself hasn't
changed.

### 6. Deno compatibility tests (Layer 5)

**Decision**: A matrix of supported Deno versions is maintained
in `.github/workflows/deno-compat.yml`. The matrix runs a
dedicated subset of Layer 2 tests — specifically those that
exercise resolver assumptions about Deno's behaviour — against
each Deno version.

**Matrix**:

- **Minimum supported Deno version** — declared in `project.json`
  under a new `engines.deno` field (initial value: `">=1.40"`,
  TBD based on `import.meta.resolve()` availability).
- **Current stable Deno version**.
- **Current canary Deno version** (allowed to fail; reports drift).

**What the compat test actually verifies**:

| Assumption | Test |
|------------|------|
| `import.meta.resolve("jsr:@std/assert")` returns a `file://` URL | Direct subprocess call, assert URL shape |
| JSR cache path location matches expected pattern | Resolve a known JSR package, assert path structure |
| npm cache path location matches expected pattern | Resolve a known npm package, assert path structure |
| `deno publish --dry-run` accepts the runtime fixture | Layer 2 fixture, run per Deno version |
| `--config` flag accepts the generated `dist/project.json` | Layer 2 fixture, run per Deno version |

**On failure**: A compat failure is a clear signal that either (a)
Deno has made a breaking change we need to accommodate, or (b)
our minimum-Deno-version declaration is wrong. Either way, it
surfaces before users hit it.

**CI integration**: the matrix runs on every PR that touches
`crates/lykn-lang/src/expander/` or `crates/lykn-cli/src/`.
Weekly cron runs it against all current Deno versions.

**Rationale**: DD-34 delegates resolution to Deno. That delegation
is correct, but it means Deno's behaviour is part of our surface
contract. Pinning a minimum version and actively verifying
assumptions against new versions is how we stay honest about that
contract.

### 7. Phase-gating map

**Decision**: Each DD-33 and DD-34 implementation phase declares
which tests from which layers must pass for it to be considered
complete. CC prompts reference this table.

| Phase | Description | Gating tests |
|-------|-------------|--------------|
| DD-33 Phase 1 | `serde_json` adoption + config types | Layer 1 config tests |
| DD-33 Phase 2 | `lykn build --dist` core + per-kind staging | Layer 1 kind-dispatch, Layer 2 per-kind build, Layer 3 config snapshots |
| DD-33 Phase 3 | Import rewriting | Layer 1 torture-fixture tests, Layer 3 rewriter snapshot |
| DD-33 Phase 4 | Publish pipeline integration | Layer 2 dry-run publish scenarios |
| DD-33 Phase 5 | Update workspace members (`lang`, `browser`, `testing`) | Layer 2 real-package build + dry-run |
| DD-33 Phase 6 | `lykn new` template update | Layer 2 scenario: `lykn new demo && cd demo && lykn build --dist` succeeds and matches snapshot |
| DD-33 Phase 7 | Documentation | No test gating (documentation review) |
| DD-34 Phase 1 | Deno subprocess `"resolve"` action | Layer 2 direct-subprocess tests, Layer 5 compat tests |
| DD-34 Phase 2 | Import-map parsing | Layer 1 map-lookup tests |
| DD-34 Phase 3 | Three-tier resolver in Pass 0 | Layer 1 resolver tests (FakeDeno), Layer 2 resolver tests (real Deno + `file:`), Layer 5 compat |
| DD-34 Phase 4 | Error diagnostics | Layer 1 error-rendering tests |
| DD-34 Phase 5 | Integration tests | Layer 2 cross-package-consumption scenario |
| DD-34 Phase 6 | Documentation | No test gating |

**Release gating** (in addition to all phase gating):

1. All Layer 1–3 tests pass in CI.
2. Layer 4 consumer-smoke passes against the release candidate.
3. Layer 5 compat passes against all supported Deno versions.

**Rationale**: Making test completion criteria explicit per phase
turns CC prompts from "write tests for this" into "these specific
tests must pass before the phase is accepted." This is enforceable
review criteria rather than good intentions.

### 8. Fixture management

**Decision**: Fixtures live under `test/fixtures/publishing/`
(for Layer 1 inputs) and `test/integration/publishing/fixtures/`
(for Layer 2 projects). Both are committed. Neither is generated
at test time.

**Naming convention**: `pkg-<kind>-<variant>` for synthetic
package fixtures. Example: `pkg-runtime-minimal`,
`pkg-macro-module-with-deps`.

**Fixture versioning**: fixtures carry their own `version` field
independent of `@lykn/*` versions. Bumping fixture versions is a
deliberate signal that the test surface has changed.

**When to add a fixture**: every bug that slips past the test
suite must have a regression fixture added as part of the fix.
This is a hard rule — if a bug reached a user, the test suite
had a gap.

**Rationale**: Committed fixtures are reviewable, stable, and
make test behaviour reproducible. Generated fixtures can drift
silently. The versioning convention catches cases where a fixture
change is really a silent test-surface change.

### 9. Test runtime targets (non-binding)

**Decision**: The following runtime targets are aspirational, not
enforced. If a layer exceeds its target, that's a signal to
investigate, not a test failure.

| Layer | Target | Enforcement |
|-------|--------|-------------|
| 1 (unit) | <10s total | none; CI reports |
| 2 (integration) | <90s total | none; CI reports |
| 3 (snapshot) | <5s total | none; CI reports |
| 4 (consumer smoke) | ~2min per pin | none; separate workflow |
| 5 (Deno compat) | ~30s per version | none; separate workflow |

**Rationale**: Hard enforcement of runtime limits creates pressure
to skip tests when times drift up. Aspirational targets with CI
reporting give visibility without perverse incentives.

### 10. What the test suite deliberately does NOT cover

**Decision**: The following are explicitly out of scope for
DD-35's test strategy:

- **Local registry simulation** (verdaccio for npm, similar for
  JSR). Considered and rejected — duplicates Layer 4 coverage with
  more mechanism.
- **Testing by publishing to real registries from CI.** Rate
  limits, version churn, and cleanup complexity make this a bad
  idea. `--dry-run` is trusted instead.
- **Performance benchmarks of the build pipeline.** Out of scope;
  revisit if pipeline latency becomes a user-visible problem.
- **Fuzz testing of the import rewriter.** The torture fixture
  provides adversarial coverage of known edge cases; full fuzz
  testing is overkill for the scope of rewriting.
- **Testing `lykn publish` without `--dry-run`.** Real publishes
  from automated tests are explicitly banned.

**Rationale**: Every "not covered" above was considered. Listing
them explicitly prevents future debate about why they're absent.

## Rejected Alternatives

### Single-layer test strategy ("just integration tests")

**What**: Skip the layer distinction; write integration tests
that cover everything end-to-end.

**Why rejected**: Single-layer strategies optimize for realism at
the cost of everything else. Unit tests run in seconds; full
integration tests run in minutes. On a codebase where the build
pipeline is iterated on frequently, forcing every test through
the slow path would either cause tests to be skipped locally or
cause CI latency that kills iteration speed. The layered approach
keeps the hot path fast.

### Test via publishing to real registries

**What**: CI publishes to JSR/npm as part of every PR, with
cleanup afterwards.

**Why rejected**: Rate limits, version naming conflicts, cleanup
race conditions, and the risk of accidentally publishing a
malformed package to the real registry. `--dry-run` is the
authoritative "would this be accepted?" answer without any of
these hazards.

### Snapshot everything via a single mega-snapshot

**What**: One giant snapshot of the entire `dist/` tree per
fixture, rather than per-file snapshots.

**Why rejected**: Mega-snapshots produce huge, unreviewable
diffs when anything changes. Per-file snapshots keep change
footprints minimal and reviewable.

### Include the general lykn test suite strategy in DD-35

**What**: Document not just publishing tests but the whole test
philosophy for the project.

**Why rejected**: Scope explosion. The general test suite
already exists and works; retroactively codifying it is a
different problem with different stakes. DD-35 is scoped to
publishing and resolution testing. A future "DD-N: general
testing strategy" can pick up the broader question if/when it
becomes worth doing.

### Omit Layer 5 (Deno compatibility)

**What**: Trust that Deno doesn't break compatibility and don't
test against it explicitly.

**Why rejected**: DD-34's resolver is a delegation contract with
Deno. Delegating without verifying the contract means breakage
will surface in user bug reports, which is the worst place to
find it. Layer 5 is cheap insurance against an external
dependency we don't control.

### Local registry simulation as a required layer

**What**: Run `verdaccio` (and JSR equivalent) in CI as a
required layer.

**Why rejected**: Duplicates Layer 4 coverage. The value-add
over `--dry-run` + external consumer repo is marginal. The setup
cost is not. Revisit only if Layer 4 proves insufficient.

### Auto-accept snapshots on CI

**What**: Let CI auto-update snapshot files when they change,
committing back to the branch.

**Why rejected**: Snapshot changes are where silent regressions
hide. Requiring explicit `cargo insta review` and a code review
of the resulting diff is the whole point of snapshot testing.
Auto-accept would defeat this.

## Edge Cases

| Case | Behavior |
|------|----------|
| Test fixture becomes stale because DD-33 / DD-34 behaviour legitimately changed | Update fixture + snapshot, document in PR that this is intentional |
| Layer 4 fails but only on one Deno version | Layer 5 was the right place to catch this; investigate what moved |
| A user-reported bug is not reproducible with any existing fixture | Add a regression fixture as part of the fix (mandatory per Decision 8) |
| `insta` snapshot review workflow blocks a PR due to unrelated snapshot changes | Treat it as a signal the PR's scope is larger than intended |
| Layer 2 test takes >90 seconds | Investigate; if legitimate growth, update target. If accidental, fix before merge. |
| Deno releases a version that breaks resolver assumptions | Layer 5 fails first; pin to a compatible version, update `engines.deno`, file upstream issue if warranted |
| Consumer-smoke repo drifts from lykn's current API | Caught by weekly cron; fix at next release cycle |
| New `PackageKind` added (e.g., `linter-plugin` in a future DD) | Every layer needs new coverage for that kind; DD introducing the kind lists the required tests |
| A fixture happens to hit a real bug in Deno | File upstream; pin around it; document in the fixture's README why |

## Dependencies

- **Depends on**:
  - DD-30 (testing DSL — the tests written in lykn for Layer 4
    consumer smoke will use its forms)
  - DD-31 (test runner CLI — `lykn test` is how Layer 4 runs the
    consumer projects)
  - DD-33 (the pipeline being tested)
  - DD-34 (the resolver being tested)
- **Affects**:
  - `lykn-cli`'s dev-dependencies (adds `insta`, `tempfile`)
  - CI configuration (new workflows for Layer 4 and Layer 5)
  - A new `oxur/lykn-consumer-tests` repository (or subdirectory,
    final location TBD)
  - Book chapter "CI/CD and Publishing" (should reference Layer
    4 / Layer 5 workflows as CI patterns)

## Open Questions

- [ ] **Final location of the consumer-smoke repo.** Options:
  separate `oxur/lykn-consumer-tests` repo, subdirectory under
  the main repo (`consumer-tests/` at the root), or separate
  branch. My lean: separate repo, because the consumer tests
  should consume published packages exactly as an external user
  would, and a subdirectory in the main repo might tempt people
  to use relative paths. Confirm during DD-34 implementation.

- [ ] **Minimum Deno version for Layer 5 matrix.** Needs to be
  pinned before Layer 5 is implemented. The relevant feature is
  `import.meta.resolve()` which has been stable for some time;
  `1.40` is a safe floor but a more recent floor might be
  acceptable. Verify during DD-34 Phase 1.

- [ ] **Release-candidate staging mechanism for Layer 4.**
  Publishing `@lykn-rc/*` to JSR/npm requires the `@lykn-rc`
  scope to exist. Alternatives: use a git-tag-based pre-release
  workflow, or publish real RCs with version suffixes
  (`0.6.0-rc.1`) and rely on Deno's handling of pre-release
  semver. Decide before Layer 4 lands.

- [ ] **How aggressively to pin dev-dependencies.** `insta` is
  committed to; should we also pin the specific version? My
  default: loose pinning (`^1.x`), tighten only if reproducibility
  problems surface.

- [ ] **Should Layer 5 run `lykn` itself under different Node /
  Bun / Deno runtimes?** Out of scope for now — lykn's toolchain
  is Deno-only (DD-28). If Node support is ever added, Layer 5
  grows.

- [ ] **Test code in `.lykn` vs `.js`?** Layer 4 consumer tests
  should probably be authored in `.lykn` (dogfooding DD-30's
  testing DSL). Layer 1–3 tests are clearly Rust. Layer 2 is a
  mix — the scenarios themselves are Rust, but the fixture
  packages contain `.lykn` files. Confirm the Layer 4 convention
  during implementation.

## Implementation Phases

DD-35 itself is not a code-producing DD — it is the specification
that DD-33 and DD-34 implementation phases execute against.
However, some infrastructure setup is needed before DD-33/DD-34
tests can be written:

### Phase 0: Test infrastructure setup

1. Add `insta` as dev-dependency to `crates/lykn-cli/Cargo.toml`.
2. Create `test/integration/publishing/` directory structure.
3. Create `test/fixtures/publishing/` with the import-rewriter
   torture fixture and its committed snapshot.
4. Create synthetic fixture packages under
   `test/integration/publishing/fixtures/synthetic/`.
5. Write the skeleton of each Layer 2 scenario file with `#[ignore]`
   on individual tests until the corresponding phase lands.
6. Document the `cargo insta review` workflow in `CLAUDE.md`.

### Phase 1: CI workflows

1. Add GitHub Actions workflow for Layer 4 (manual trigger +
   weekly cron).
2. Add GitHub Actions workflow for Layer 5 (Deno version matrix,
   triggered on PR touching resolver/publish code and on cron).
3. Update the main PR workflow to run all of Layers 1–3 in parallel.

### Phase 2: Consumer smoke repo bootstrap

1. Create `oxur/lykn-consumer-tests` (or finalized location).
2. Set up `pin-latest/` referencing current JSR / npm versions
   of `@lykn/lang`, `@lykn/browser`, `@lykn/testing`.
3. Set up `ci/run-smoke.sh` with clear pass/fail semantics.
4. Verify the first smoke run against currently-published
   packages — this establishes a baseline before DD-33's new
   publishing pipeline replaces the packages.

### Phase 3: Ongoing maintenance (integrated into DD-33/DD-34 phases)

Phases 0–2 set up the infrastructure. The actual test cases are
written as part of DD-33 and DD-34 implementation phases, per
the phase-gating table in Decision 7. DD-35 does not have its
own "implementation complete" state — it is complete when
DD-33 and DD-34 are complete and the phase-gating table's
requirements have all been met.
