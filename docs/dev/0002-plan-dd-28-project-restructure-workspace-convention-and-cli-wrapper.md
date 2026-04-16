# Plan: DD-28 — Project Restructure, Workspace Convention, and CLI Wrapper

## Context

Restructure lykn project: `src/` → `packages/lykn/`, `deno.json` → `project.json`, delete npm artifacts from git, add CLI wrappers for Deno commands. Establishes conventions for `lykn new` (DD-29). Import map `"lykn/"` for clean test imports.

## Phase 1: File migration + project.json

**Commit separately. Verify all tests pass before proceeding.**

### 1a. Create directory structure

```
mkdir -p packages/lykn
```

### 1b. Move JS source files

```
git mv src/reader.js packages/lykn/reader.js
git mv src/compiler.js packages/lykn/compiler.js
git mv src/expander.js packages/lykn/expander.js
git mv src/surface.js packages/lykn/surface.js
git mv src/lykn-browser.js packages/lykn/browser.js
```

### 1c. Create `packages/lykn/mod.js`

New entry point (replaces `src/index.js`). Same content but renamed:

```javascript
import { read } from './reader.js';
import { expand, expandExpr } from './expander.js';
import { compile, compileExpr } from './compiler.js';

export { read, expand, expandExpr, compile, compileExpr };

export function lykn(source) {
  return compile(expand(read(source)));
}
```

### 1d. Create `project.json` (workspace root)

```json
{
  "workspaces": ["./packages/lykn"],
  "imports": {
    "lykn/": "./packages/lykn/"
  },
  "tasks": {
    "test": "deno test -A test/",
    "test:unit": "deno test test/forms/ test/reader/ test/expander/",
    "test:integration": "deno test -A test/integration/",
    "build:browser": "deno run -A build.js"
  }
}
```

### 1e. Create `packages/lykn/deno.json` (package config)

```json
{
  "name": "@lykn/lykn",
  "version": "0.5.0",
  "exports": "./mod.js",
  "imports": {
    "astring": "npm:astring@^1.9.0"
  }
}
```

### 1f. Delete old files

```
git rm deno.json
git rm package.json
git rm src/index.js
git rm src/index.d.ts
```

### 1g. Update all test imports (82 files)

Replace `../../src/` with `lykn/` using import map:

```
../../src/reader.js      →  lykn/reader.js
../../src/expander.js    →  lykn/expander.js
../../src/compiler.js    →  lykn/compiler.js
```

Mechanical find-and-replace across all 82 test files.

### 1h. Update internal cross-imports

Files in `packages/lykn/` use relative `./` imports to siblings — these stay unchanged since the whole directory moved together. Only verify:

- `expander.js` imports `./compiler.js`, `./reader.js`, `./surface.js` ✓
- `browser.js` (was `lykn-browser.js`) imports `./reader.js`, `./expander.js`, `./compiler.js` ✓
- `mod.js` imports `./reader.js`, `./expander.js`, `./compiler.js` ✓

### 1i. Update `build.js`

Line 28: `"src/lykn-browser.js"` → `"packages/lykn/browser.js"`

### 1j. Update Rust `bridge.rs`

- Line 54: `./src/compiler.js` → `./packages/lykn/compiler.js`
- Line 91: `src/compiler.js` → `packages/lykn/compiler.js`
- Also update `deno.json` check to `project.json`

### 1k. Update Rust `e2e_tests.rs`

- Line 71: `./src/reader.js` → `./packages/lykn/reader.js`
- Line 72: `./src/expander.js` → `./packages/lykn/expander.js`

### 1l. Update `.gitignore`

Remove `/workbench` if desired (it's a working dir). Ensure `dist/` stays ignored.

### 1m. Update `README.md`

- Architecture section: `src/` → `packages/lykn/`
- File descriptions: update paths
- Toolchain section: already updated for Deno-only

### 1n. Update `CLAUDE.md`

- Pipeline paths: `src/` → `packages/lykn/`
- Build commands section

### Verification

```bash
deno test --config project.json -A test/
cargo test
cargo clippy
```

## Phase 2: CLI subcommands (run, test, lint)

**Commit separately after Phase 1 is green.**

### 2a. Add new subcommands to `crates/lykn-cli/src/main.rs`

Extend the `Commands` enum:

```rust
#[derive(Subcommand)]
enum Commands {
    Fmt { ... },
    Check { ... },
    Compile { ... },
    Run {
        /// File to run (.lykn or .js)
        file: PathBuf,
        /// Arguments to pass to the script
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    Test {
        /// Test file patterns (default: all tests)
        #[arg(default_value = "test/")]
        patterns: Vec<String>,
    },
    Lint {
        /// Paths to lint (default: packages/)
        #[arg(default_value = "packages/")]
        paths: Vec<String>,
    },
}
```

### 2b. Implement `cmd_run`

```rust
fn cmd_run(file: &Path, args: &[String]) {
    let project_root = find_project_root().unwrap_or_else(|| ".".into());
    let config = project_root.join("project.json");

    if file.extension().map_or(false, |e| e == "lykn") {
        // Compile .lykn to temp .js, then run
        let temp = std::env::temp_dir().join("lykn_run.js");
        // Use the existing compile pipeline
        compile_to_file(file, &temp, false);
        exec_deno(&["run", "--config", config_str, "-A", temp_str, ...args]);
    } else {
        exec_deno(&["run", "--config", config_str, "-A", file_str, ...args]);
    }
}
```

### 2c. Implement `cmd_test`

```rust
fn cmd_test(patterns: &[String]) {
    let config = find_config_path();
    let mut args = vec!["test", "--config", &config, "-A"];
    args.extend(patterns.iter().map(|s| s.as_str()));
    exec_deno(&args);
}
```

### 2d. Implement `cmd_lint`

```rust
fn cmd_lint(paths: &[String]) {
    let config = find_config_path();
    let mut args = vec!["lint", "--config", &config];
    args.extend(paths.iter().map(|s| s.as_str()));
    exec_deno(&args);
}
```

### 2e. Helper: `exec_deno`

```rust
fn exec_deno(args: &[&str]) {
    let status = Command::new("deno")
        .args(args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run deno: {e}");
            eprintln!("is deno installed? try: brew install deno");
            process::exit(1);
        });
    process::exit(status.code().unwrap_or(1));
}
```

### 2f. Helper: `find_config_path`

Update `find_project_root` in `bridge.rs` to look for `project.json` first, then `deno.json` as fallback.

### Verification

```bash
cargo build --release
lykn test                    # → deno test --config project.json -A test/
lykn lint packages/          # → deno lint --config project.json packages/
lykn run test.lykn           # compile + run
```

## Phase 3: npm publish via dnt

**Commit separately.**

### 3a. Create `build_npm.ts`

```typescript
import { build } from "jsr:@nicetry/dnt";

await build({
  entryPoints: ["./packages/lykn/mod.js"],
  outDir: "./dist/npm",
  shims: { deno: false },
  package: {
    name: "@lykn/lykn",
    version: Deno.readTextFileSync("packages/lykn/deno.json")
      |> JSON.parse |> (c => c.version),
    description: "S-expression syntax for JavaScript",
    license: "Apache-2.0",
    repository: { type: "git", url: "https://github.com/oxur/lykn" },
  },
});
```

### 3b. Add `publish` subcommand to CLI

```rust
Publish {
    #[arg(long)]
    jsr: bool,
    #[arg(long)]
    npm: bool,
    #[arg(long)]
    crates: bool,
    #[arg(long)]
    dry_run: bool,
}
```

- `--jsr`: `deno publish --config project.json`
- `--npm`: `deno run -A build_npm.ts && cd dist/npm && npm publish`
- `--crates`: existing crates.io logic from Makefile
- No flags: default to `--jsr`

### 3c. Update Makefile

Replace `deno task publish:*` with `lykn publish --*`.

### Verification

```bash
lykn publish --jsr --dry-run
lykn publish --npm --dry-run
ls dist/npm/package.json     # generated, not tracked
```

## Phase 4: Docs update

### Files to update

| File | Change |
|------|--------|
| `README.md` | Architecture paths, CLI commands, toolchain |
| `CLAUDE.md` | Pipeline paths, build commands |
| `assets/ai/SKILL.md` | Document selection paths, CLI section, no-Node table |
| `docs/guides/00-lykn-surface-forms.md` | Version reference if needed |
| `docs/guides/15-lykn-cli.md` | New subcommands (if exists) |

## Key files modified per phase

| Phase | Files |
|-------|-------|
| 1 | 82 test files, `build.js`, `bridge.rs`, `e2e_tests.rs`, `project.json` (new), `packages/lykn/deno.json` (new), `packages/lykn/mod.js` (new), `.gitignore`, `README.md`, `CLAUDE.md` |
| 2 | `crates/lykn-cli/src/main.rs`, `crates/lykn-cli/src/bridge.rs` |
| 3 | `build_npm.ts` (new), `crates/lykn-cli/src/main.rs`, `Makefile` |
| 4 | `README.md`, `CLAUDE.md`, `SKILL.md`, guides |

## Verification (end-to-end)

```bash
# Phase 1
deno test --config project.json -A test/
cargo test
cargo clippy

# Phase 2
cargo build --release
lykn test
lykn lint packages/

# Phase 3
lykn publish --npm --dry-run

# Phase 4
# Manual review of docs
```
