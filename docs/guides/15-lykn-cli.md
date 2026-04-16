# lykn CLI

The lykn command-line tool: compiling, formatting, and syntax checking
`.lykn` source files. The CLI is a single Rust binary with no runtime
dependencies.

---

## ID-00: `lykn new` — Create a New Project

**Strength**: SHOULD

**Summary**: Create a new lykn project with the workspace convention.

```sh
lykn new my-app
cd my-app
lykn run packages/my-app/mod.lykn
```

**Generated structure**:

```
my-app/
├── project.json              ← workspace root
├── packages/
│   └── my-app/
│       ├── deno.json          ← package config
│       └── mod.lykn           ← entry point
├── test/
│   └── mod.test.js            ← starter test
└── .gitignore
```

**Options**:

| Flag | Description |
|------|-------------|
| `--path DIR` | Create in a specific parent directory |

**Name rules**: kebab-case only — lowercase letters, digits, hyphens.
Must start with a letter.

The generated project is immediately runnable (`lykn run`) and
testable (`lykn test`). Git is initialized automatically.

---

## ID-01: Install — Build from Source

**Strength**: MUST

```sh
# Build the release binary
cargo build --release

# Copy to project bin/
mkdir -p bin/
cp target/release/lykn bin/

# Verify
./bin/lykn --version
```

The lykn binary is self-contained. No runtime dependencies, no Deno
or Node.js required for compilation.

---

## ID-02: `lykn compile` — Compile `.lykn` to JavaScript

**Strength**: MUST

```sh
# Output to stdout
lykn compile packages/myapp/main.lykn

# Output to file
lykn compile packages/myapp/main.lykn -o dist/main.js

# Strip type checks and contracts (production)
lykn compile packages/myapp/main.lykn --strip-assertions -o dist/main.js

# Output kernel JSON (debugging)
lykn compile packages/myapp/main.lykn --kernel-json
```

**Options**:

| Flag | Description |
|------|-------------|
| `-o`, `--output FILE` | Write to file (default: stdout) |
| `--strip-assertions` | Remove type checks and contracts |
| `--kernel-json` | Output kernel S-expression JSON |

**Note**: `lykn compile` operates on a single file. For multi-file
projects, use a Makefile or shell loop:

```sh
# Compile all .lykn files
for f in packages/myapp/**/*.lykn; do
  out="dist/${f#packages/myapp/}"
  out="${out%.lykn}.js"
  mkdir -p "$(dirname "$out")"
  lykn compile "$f" -o "$out"
done
```

---

## ID-03: `lykn fmt` — Format `.lykn` Source

**Strength**: SHOULD

```sh
# Preview formatted output (stdout)
lykn fmt packages/myapp/main.lykn

# Format in place
lykn fmt -w packages/myapp/main.lykn

# Format multiple files
lykn fmt -w packages/myapp/auth/*.lykn
```

The formatter handles S-expression indentation with 80-character line
width. This formats the `.lykn` source — for formatting compiled JS
output, use `biome format`.

**See also**: `13-biome/13-03-formatting.md` for JS output formatting.

---

## ID-04: `lykn check` — Syntax Check

**Strength**: SHOULD

```sh
# Check a single file
lykn check packages/myapp/main.lykn

# Check multiple files
lykn check packages/myapp/**/*.lykn
```

`lykn check` parses and analyzes the source without producing output.
It reports:
- Syntax errors
- Unused bindings (warnings)
- Missing type annotations
- Unknown surface forms

Use it in CI to catch issues before compilation.

---

## ID-04a: `lykn run` — Run `.lykn` or `.js` Files

**Strength**: SHOULD

**Summary**: Run a file directly via Deno. `.lykn` files are
compiled to a temp `.js` file first, then executed.

```sh
# Run a .lykn file (compile + execute)
lykn run packages/myapp/main.lykn

# Run a .js file directly
lykn run dist/main.js

# Pass arguments
lykn run packages/myapp/main.lykn -- --port 3000
```

The CLI auto-discovers `project.json` by walking up from the
current directory and passes `--config project.json` to Deno.

---

## ID-04b: `lykn test` — Run Tests

**Strength**: SHOULD

**Summary**: Run tests via Deno's test runner.

```sh
# Run all tests
lykn test

# Run specific test directory
lykn test test/forms/

# Run a single test file
lykn test test/surface/func.test.js
```

Wraps `deno test --config project.json --no-check -A`.

---

## ID-04c: `lykn lint` — Lint Compiled JS

**Strength**: SHOULD

**Summary**: Lint JavaScript files via Deno's built-in linter.

```sh
# Lint all packages
lykn lint

# Lint specific directory
lykn lint packages/myapp/
```

Wraps `deno lint --config project.json`.

---

## ID-04d: `lykn publish` — Publish Packages

**Strength**: SHOULD

**Summary**: Publish to JSR, npm, or both.

```sh
# Publish to JSR (default)
lykn publish --jsr

# Build and publish to npm
lykn publish --npm

# Dry run (check without publishing)
lykn publish --npm --dry-run
lykn publish --jsr --dry-run
```

`--npm` builds the npm package in `dist/npm/` via `build_npm.ts`,
then publishes. No `package.json` tracked in git — generated at
publish time.

---

## ID-05: `--strip-assertions` for Production Builds

**Strength**: SHOULD

**Summary**: Remove all type checks and `:pre`/`:post` contracts from
compiled output for zero-overhead production builds.

```lykn
;; Source
(func add
  :args (:number a :number b)
  :returns :number
  :pre (and (>= a 0) (>= b 0))
  :body (+ a b))
```

**Development** (`lykn compile`):

```js
function add(a, b) {
  if (typeof a !== "number" || Number.isNaN(a))
    throw new TypeError("add: arg 'a' expected number, got " + typeof a);
  if (typeof b !== "number" || Number.isNaN(b))
    throw new TypeError("add: arg 'b' expected number, got " + typeof b);
  if (!(a >= 0 && b >= 0))
    throw new Error("add: pre-condition failed: ...");
  const result__gensym0 = a + b;
  if (typeof result__gensym0 !== "number" || Number.isNaN(result__gensym0))
    throw new TypeError("add: return value expected number, got " + typeof result__gensym0);
  return result__gensym0;
}
```

**Production** (`lykn compile --strip-assertions`):

```js
function add(a, b) {
  return a + b;
}
```

---

## ID-06: The Full Build Pipeline

**Strength**: MUST

```sh
# 1. Format lykn source
lykn fmt -w packages/myapp/main.lykn

# 2. Check syntax
lykn check packages/myapp/main.lykn

# 3. Compile to JS
lykn compile packages/myapp/main.lykn -o dist/main.js

# 4. Format compiled JS
biome format --write dist/

# 5. Lint compiled JS
biome lint dist/

# 6. Run tests
deno test test/

# 7. Run
deno run --allow-net dist/main.js
```

A typical `Makefile`:

```makefile
.PHONY: build test check fmt

build:
	lykn compile packages/myapp/main.lykn -o dist/main.js
	biome format --write dist/

test: build
	deno test --allow-all

check: build
	biome check dist/
	deno test --allow-all

fmt:
	lykn fmt -w packages/myapp/*.lykn
	biome format --write dist/
```

---

## ID-07: `--kernel-json` for Debugging

**Strength**: CONSIDER

```sh
# See the kernel S-expressions as JSON (before JS codegen)
lykn compile packages/myapp/main.lykn --kernel-json
```

Useful for debugging macro expansions and surface-to-kernel
transformations. The output shows the intermediate representation
that the JS codegen consumes.

---

---

## Quick Reference

| Command | Description |
|---------|-------------|
| `lykn new NAME` | Create new project |
| `lykn compile FILE` | Compile to JS (stdout) |
| `lykn compile FILE -o OUT` | Compile to file |
| `lykn compile FILE --strip-assertions` | Production build |
| `lykn compile FILE --kernel-json` | Debug kernel output |
| `lykn fmt FILE` | Preview formatted source |
| `lykn fmt -w FILE` | Format in place |
| `lykn check FILE` | Syntax check |
| `lykn run FILE` | Run .lykn or .js file |
| `lykn test [PATTERNS]` | Run tests via Deno |
| `lykn lint [PATHS]` | Lint JS via Deno |
| `lykn publish --jsr` | Publish to JSR |
| `lykn publish --npm` | Build + publish to npm |
| `lykn publish --dry-run` | Check without publishing |
| `lykn --version` | Show version |

---

## Related Guidelines

- **Project Structure**: See `10-project-structure.md` ID-26 for the
  compilation pipeline
- **Type Discipline**: See `05-type-discipline.md` ID-30 for
  `--strip-assertions`
- **Deno Runtime**: See `12-deno/12-01-runtime-basics.md` for
  `deno lint` and `deno fmt` on compiled output
- **Surface Forms Reference**: See `00-lykn-surface-forms.md` for the
  complete surface form catalog
