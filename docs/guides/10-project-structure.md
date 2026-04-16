# Project Structure

How to organize a lykn project for clarity, maintainability, and
tooling compatibility: directory layout, file naming, module dependency
flow, entry points, configuration, and test structure. lykn source
files (`.lykn`) compile to JavaScript (`.js`), adding a compilation
step to the development workflow. Module API design is in Guide 02;
Deno runtime details are in Guide 12.

Target environment: **Deno**, **ESM-only**, **Biome** on compiled
output, lykn/surface syntax throughout.

---

## ID-01: Flat-by-Feature, Not Nested-by-Type

**Strength**: SHOULD

**Summary**: Group files by feature/domain, not by technical role.

```
;; Good — flat-by-feature
project/
├── deno.json
├── mod.lykn               ;; library entry point
├── auth/
│   ├── mod.lykn           ;; public API: re-exports
│   ├── login.lykn
│   ├── session.lykn
│   ├── password.lykn
│   └── auth_test.js       ;; tests run on compiled output
├── users/
│   ├── mod.lykn
│   ├── repository.lykn
│   ├── validation.lykn
│   └── users_test.js
└── shared/
    ├── http.lykn
    └── constants.lykn

;; Bad — nested-by-type
project/
├── controllers/
├── models/
├── services/
└── utils/
```

**Rationale**: Adding or modifying a feature should touch files in one
directory. Flat-by-feature means adding a feature = adding a directory,
deleting a feature = deleting a directory.

---

## ID-02: Keep the Root Clean

**Strength**: SHOULD

**Summary**: The project root should contain only configuration files
and entry points. Source code goes in feature directories.

```
project/
├── deno.json               ;; config
├── deno.lock               ;; lockfile (auto-generated)
├── biome.json              ;; Biome config
├── Makefile                ;; build tasks
├── README.md
├── bin/                    ;; compiled binary (lykn CLI)
│   └── lykn
├── mod.lykn                ;; library entry point
├── main.lykn               ;; application entry point
├── auth/
├── users/
└── shared/
```

---

## ID-03: Reference Directory Structure for a lykn Project

**Strength**: SHOULD

**Summary**: A standard lykn project layout with feature directories,
compilation output, co-located tests, and centralized configuration.

```
my-project/
├── project.json             ;; workspace root (lykn CLI reads this)
├── deno.lock
├── Makefile                 ;; lykn commands + make tasks
├── bin/
│   └── lykn                 ;; lykn CLI binary
├── packages/
│   └── my-project/          ;; lykn source (workspace member)
│       ├── deno.json        ;; package config (name, version, exports)
│       ├── mod.lykn         ;; library entry point
│       ├── auth/
│       │   ├── mod.lykn
│       │   ├── login.lykn
│       │   └── session.lykn
│       ├── users/
│       │   ├── mod.lykn
│       │   ├── repository.lykn
│       │   └── validation.lykn
│       └── shared/
│           ├── http.lykn
│           └── constants.lykn
├── dist/                    ;; compiled JS output
│   ├── mod.js
│   ├── auth/
│   │   ├── mod.js
│   │   ├── login.js
│   │   └── session.js
│   └── ...
├── test/
│   ├── auth/
│   │   └── login_test.js    ;; tests run on compiled JS
│   └── users/
│       └── repository_test.js
└── docs/
    └── guides/              ;; lykn guides
```

**Conventions**:
- `project.json` at workspace root — import maps, tasks, workspace members
- `.lykn` source in `packages/<name>/` (workspace member)
- Each package has its own `deno.json` (name, version, exports)
- Compiled `.js` output in `dist/`
- Tests in `.js` (they import compiled output)
- `bin/lykn` for the CLI binary
- `lykn test`, `lykn lint`, `lykn run` wrap Deno with `--config project.json`

---

## ID-04: kebab-case for File and Directory Names

**Strength**: SHOULD

**Summary**: Use `kebab-case` for all file and directory names. lykn
files use `.lykn` extension.

```
;; Good
auth/
  password-hash.lykn
  session-store.lykn

;; Bad
Auth/
  PasswordHash.lykn
  sessionStore.lykn
```

**Exception**: Test files use `*_test.js` per Deno convention.

---

## ID-05: One Module, One Purpose

**Strength**: SHOULD

**Summary**: Each `.lykn` file should have a single clear
responsibility.

```lykn
;; Good — focused module: password.lykn
(export (func hash-password
  :args (:string plain) :returns :string
  :body (do-hash plain)))

(export (func verify-password
  :args (:string plain :string hash) :returns :boolean
  :body (do-verify plain hash)))
```

**See also**: `02-api-design.md` ID-06

---

## ID-06: Name Files After Their Primary Export

**Strength**: SHOULD

**Summary**: A file named `session.lykn` should export session-related
functions.

```
;; Good
auth/
  login.lykn          ;; exports login, logout
  session.lykn        ;; exports create-session, destroy-session
  password.lykn       ;; exports hash-password, verify-password

;; Bad
auth/
  controller.lykn     ;; controller for what?
  service.lykn        ;; service for what?
```

---

## ID-07: Test Files Use `*_test.js` for Deno Auto-Discovery

**Strength**: SHOULD

**Summary**: Tests run on compiled JS output and use Deno's test
runner. Name test files `*_test.js`.

```
auth/
  login.lykn           ;; source
  login.js             ;; compiled output
test/
  auth/
    login_test.js      ;; deno test finds this
```

---

## ID-08: Entry Points — `mod.lykn` for Libraries, `main.lykn` for Apps

**Strength**: SHOULD

**Summary**: Use `mod.lykn` as the library entry point (compiles to
`mod.js`, the Deno convention). Use `main.lykn` for applications.

```lykn
;; mod.lykn — library public API
(export "./create.js" (names create))
(export "./parse.js" (names parse))
```

---

## ID-09: Barrel Files — Selective Re-Exports Only

**Strength**: SHOULD

**Summary**: Use selective re-exports in `mod.lykn` to define the
public API.

```lykn
;; Good — selective re-exports
;; auth/mod.lykn
(export "./login.js" (names login logout))
(export "./session.js" (names create-session destroy-session))
(export "./password.js" (names hash-password))
```

**See also**: `02-api-design.md` ID-08

---

## ID-10: Avoid Deep Imports into Internal Modules

**Strength**: SHOULD

**Summary**: Import from a feature's `mod.js`, not its internal files.

```lykn
;; Good — import from public API
(import "./auth/mod.js" (login create-session))

;; Bad — reaching into internals
(import "./auth/password.js" (hash-password))
```

---

## ID-11: Dependency Direction — Depend Inward, Not Outward

**Strength**: MUST

**Summary**: Inner/core modules should not import from outer/feature
modules. Dependencies flow inward.

```
auth/login.lykn       → imports from shared/http.lykn    ✓
shared/http.lykn      → imports nothing outside            ✓
shared/http.lykn      → imports from auth/session.lykn   ✗
```

---

## ID-12: No Circular Imports

**Strength**: MUST

**Summary**: Circular imports cause bindings to be `undefined` at
access time. Extract shared dependencies into a third module.

---

## ID-13: Limit Import Depth — Use Import Maps

**Strength**: SHOULD

**Summary**: If you're writing deep relative paths, use import map
aliases in `deno.json`.

```lykn
;; Good — import map alias
;; deno.json: { "imports": { "@shared/": "./shared/" } }
(import "@shared/strings.js" (slugify))
```

---

## ID-14: Separate Pure Logic from I/O

**Strength**: SHOULD

**Summary**: Keep core business logic in pure modules. Push I/O to
the edges.

```lykn
;; Good — pure core
;; shared/transform.lykn
(export (func transform :args (:any data) :returns :any :body
  (data:map normalize)))

;; I/O at the edge
;; main.lykn
(bind raw (await (Deno:readTextFile "data.csv")))
(bind result (transform raw))
(await (Deno:writeTextFile "output.json" (JSON:stringify result)))
```

**See also**: `06-functions-closures.md` ID-29

---

## ID-15: `deno.json` as the Single Config Source

**Strength**: SHOULD

**Summary**: Centralize imports, tasks, and compiler options in
`deno.json`. Biome config goes in `biome.json`.

```json
{
  "imports": {
    "@std/assert": "jsr:@std/assert@^1.0.0",
    "@std/path": "jsr:@std/path@^1.0.0",
    "@shared/": "./shared/"
  },
  "tasks": {
    "build": "make build",
    "dev": "deno run --watch --allow-net --allow-read dist/main.js",
    "test": "make build && deno test --allow-all",
    "check": "make build && deno lint dist/ && deno test --allow-all"
  },
  "compilerOptions": {
    "checkJs": true
  }
}
```

---

## ID-16: Import Maps for Dependency Aliases

**Strength**: SHOULD

**Summary**: Use the `imports` field in `deno.json` for bare specifier
mapping.

```lykn
;; Source files use clean bare specifiers
(import "@std/assert" (assert-equals))
(import "@std/path" (join))
(import "@shared/strings.js" (slugify))
```

---

## ID-17: Environment Variables for Runtime Config

**Strength**: SHOULD

**Summary**: Use environment variables for runtime configuration.

```lykn
(bind PORT (Number (?? (Deno:env:get "PORT") "8080")))
(bind DB-URL (?? (Deno:env:get "DATABASE_URL") "sqlite:./dev.db"))
(Deno:serve (obj :port PORT) handler)
```

---

## ID-18: Centralize Shared Constants

**Strength**: SHOULD

**Summary**: Put constants used across features in `shared/constants.lykn`.

```lykn
;; shared/constants.lykn
(export (bind MAX-RETRIES 3))
(export (bind DEFAULT-TIMEOUT 5000))
(export (bind API-VERSION "v2"))
```

**See also**: `01-core-idioms.md` ID-11

---

## ID-19: lykn Type Annotations Replace JSDoc `@typedef`

**Strength**: SHOULD

**Summary**: In lykn, `type` constructors replace JSDoc `@typedef` for
shared type definitions. For compiled JS output consumed by other JS
code, JSDoc annotations may still be useful.

```lykn
;; Good — type constructors for shared shapes
;; shared/types.lykn
(export (type User (Usr :string id :string name :string email)))
(export (type ApiResult
  (Ok :any data)
  (Err :string message)))
```

**See also**: `05-type-discipline.md` ID-03, ID-17

---

## ID-20: Co-Locate Tests with Source

**Strength**: SHOULD

**Summary**: Place test files next to the compiled output they test,
or in a mirrored `test/` directory.

---

## ID-21: Separate Test Utilities

**Strength**: CONSIDER

**Summary**: Shared test helpers go in a `testing/` directory, not in
`shared/`.

---

## ID-22: Pin Dependencies — Use `deno.lock`

**Strength**: SHOULD

**Summary**: Commit `deno.lock` to version control for reproducible
builds.

---

## ID-23: Prefer `jsr:` Specifiers over `npm:`

**Strength**: CONSIDER

**Summary**: Use `jsr:` for Deno standard library and JSR packages.
Use `npm:` only for npm-only packages.

---

## ID-24: Vendor Dependencies for Offline/CI Stability

**Strength**: CONSIDER

**Summary**: Use `"vendor": true` in `deno.json` for air-gapped builds.

---

## ID-25: Deno Workspaces for Multi-Package Projects

**Strength**: CONSIDER

**Summary**: Use the `workspace` field for monorepos with multiple
packages. lykn projects with separate crate-style packages benefit
from workspace organization.

---

## ID-26: The lykn Compilation Pipeline

**Strength**: MUST

**Summary**: lykn source (`.lykn`) must be compiled to JavaScript
(`.js`) before execution. The compilation step fits into the build
pipeline alongside Biome formatting.

```sh
# Compile lykn source to JavaScript
lykn compile src/main.lykn -o dist/main.js

# Format compiled output with Biome
biome format --write dist/

# Run with Deno
deno run --allow-net dist/main.js

# Or combine in Makefile / deno tasks
make build    # compile + format
make test     # compile + test
make check    # compile + lint + test
```

**Pipeline**:
```
.lykn source → lykn compile → .js output → biome format → deno run/test
```

**Development workflow**:
1. Write `.lykn` source files
2. `lykn compile` to produce `.js` output
3. `biome format --write` on compiled output
4. `deno test` to run tests against compiled JS
5. `deno run` to execute the application

**Rationale**: lykn compiles to clean, readable JavaScript. The
compiled output is the artifact that Deno runs, Biome formats, and
tests exercise. The `.lykn` source is the authoritative code.

---

---

## Best Practices Summary

### Quick Reference Table

| ID | Pattern | Strength | Key Insight |
|----|---------|----------|-------------|
| 01 | Flat-by-feature layout | SHOULD | Group by domain, not by type |
| 02 | Clean root | SHOULD | Config at root, source in directories |
| 03 | Reference directory structure | SHOULD | `.lykn` source, `dist/` output, tests |
| 04 | kebab-case file names | SHOULD | `.lykn` extension, cross-OS safe |
| 05 | One module, one purpose | SHOULD | Focused modules |
| 06 | Name files after primary export | SHOULD | `login.lykn` exports `login` |
| 07 | `*_test.js` for tests | SHOULD | Deno auto-discovery |
| 08 | `mod.lykn` / `main.lykn` | SHOULD | Library vs application entry |
| 09 | Selective re-exports | SHOULD | Explicit barrel files |
| 10 | No deep imports | SHOULD | Import from `mod.js` |
| 11 | Depend inward | MUST | `shared/` doesn't import features |
| 12 | No circular imports | MUST | Extract shared dependencies |
| 13 | Import map aliases | SHOULD | Eliminate deep relative paths |
| 14 | Separate pure from I/O | SHOULD | Pure core, I/O at edges |
| 15 | `deno.json` config | SHOULD | Single source of truth |
| 16 | Import maps | SHOULD | Centralized versions |
| 17 | Env vars for config | SHOULD | Runtime configuration |
| 18 | Centralize constants | SHOULD | One place to change |
| 19 | `type` replaces `@typedef` | SHOULD | Runtime-validated types |
| 20 | Co-locate tests | SHOULD | Visible alongside code |
| 21 | Test utilities separate | CONSIDER | Keep out of production |
| 22 | `deno.lock` | SHOULD | Reproducible builds |
| 23 | `jsr:` over `npm:` | CONSIDER | Native Deno registry |
| 24 | Vendor for offline | CONSIDER | Air-gapped builds |
| 25 | Workspaces for monorepos | CONSIDER | Shared config |
| 26 | lykn compilation pipeline | MUST | `.lykn` → `.js` → run/test |

---

## Related Guidelines

- **Core Idioms**: See `01-core-idioms.md` for ESM (ID-08), named
  exports (ID-07), magic values (ID-11)
- **API Design**: See `02-api-design.md` for module design (ID-06-10)
- **Functions & Closures**: See `06-functions-closures.md` for module
  scope (ID-15), pure functions (ID-28-29)
- **Performance**: See `08-performance.md` for tree shaking (ID-30)
- **Deno**: See `12-deno/01-runtime-basics.md` for runtime details
- **Biome**: See `13-biome/01-setup.md` for formatting configuration
- **No-Node Boundary**: See `14-no-node-boundary.md`
- **lykn CLI**: See `15-lykn-cli.md` for compiler usage
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for
  `import`, `export`, `bind`
