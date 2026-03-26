# Phase 6 — Browser Shim: Implementation Guide

**For**: Claude Code
**Scope**: Phase 6 of lykn v0.1.0 — bundle compiler for browsers, `<script type="text/lykn">` support, `window.lykn` API
**Where you're working**: New files — `src/lykn-browser.js` (entry point), `examples/browser.html` (test page), `dist/` (build output)
**Prerequisites**: Phases 1–5 must be complete (the compiler must be feature-complete)
**Design authority**: Project plan §6.1–6.6; research reference to BiwaScheme/Wisp shim patterns

---

## Overview: What Phase 6 Is

Phase 6 is NOT compiler work. You are not modifying `compiler.js` or adding ESTree forms. Instead, you're packaging the entire lykn compiler (reader + compiler + astring) as a single browser-loadable JavaScript file, and writing a small shim that:

1. Scans the DOM for `<script type="text/lykn">` tags
2. Compiles their content to JS
3. Executes the compiled JS
4. Exposes `window.lykn` for programmatic use

This is a thin integration layer — the compiler does all the real work. The shim just wires it up to the browser environment.

### Why This Works Without an Interpreter

Most Lisp-in-browser projects (BiwaScheme, etc.) include a full interpreter that evaluates s-expressions at runtime. Lykn doesn't need one — it compiles to plain JavaScript, so the browser's native JS engine runs the output. The flow is:

```
.lykn source → read() → compile() → JS string → eval() → done
```

No interpreter, no runtime, no AST walker. Just compile and eval.

---

## 6.1 Browser Entry Point: `src/lykn-browser.js`

Create a new file `src/lykn-browser.js`. This is the entry point that esbuild will bundle.

### What It Exports

The file re-exports the compiler API and adds the browser shim logic:

```js
import { read } from './reader.js';
import { compile } from './compiler.js';

/**
 * Compile lykn source to JavaScript string.
 * @param {string} source - lykn source text
 * @returns {string} JavaScript source text
 */
export function compileLykn(source) {
  return compile(read(source));
}

/**
 * Compile and execute lykn source.
 * @param {string} source - lykn source text
 * @returns {*} result of eval
 */
export function run(source) {
  const js = compileLykn(source);
  return (0, eval)(js);
}

/**
 * Fetch a .lykn file, compile, and execute.
 * @param {string} url - URL to fetch
 * @returns {Promise<*>} result of eval
 */
export async function load(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`[lykn] Failed to load ${url}: ${response.status}`);
  }
  const source = await response.text();
  return run(source);
}

/**
 * Process all <script type="text/lykn"> elements in document order.
 */
async function processScripts() {
  const scripts = document.querySelectorAll('script[type="text/lykn"]');

  for (const el of scripts) {
    const label = el.src || 'inline script';
    try {
      if (el.src) {
        await load(el.src);
      } else {
        run(el.textContent);
      }
    } catch (err) {
      console.error(`[lykn] Error in ${label}:`, err);
    }
  }
}

// Auto-run on DOMContentLoaded (only in browser environment)
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', processScripts);
  } else {
    // DOM already loaded (script loaded with defer, or dynamically)
    processScripts();
  }
}

// Expose public API
export { compileLykn as compile };
```

### Compiler Pitfall: Indirect Eval — `(0, eval)(js)`

The expression `(0, eval)(js)` is **indirect eval**. It looks weird but is critical:

- **Direct eval**: `eval(js)` executes in the LOCAL scope of the calling function. Variables declared in the compiled JS would be trapped inside `processScripts()` and invisible to subsequent scripts.
- **Indirect eval**: `(0, eval)(js)` executes in the GLOBAL scope. Variables declared with `var` become global, and all scripts share the same scope.

This is the standard pattern used by BiwaScheme, Wisp, CoffeeScript, and every other compile-to-JS language's browser shim.

**The `(0, eval)` trick**: The comma operator evaluates `0` (discarded), then `eval`, returning the `eval` function itself. Calling a function obtained this way (not directly by the name `eval`) makes it indirect. Any of these work: `(0, eval)`, `(1, eval)`, `window.eval`. The `(0, eval)` form is the conventional idiom.

### Compiler Pitfall: `const`/`let` in Compiled Output

There's a subtlety: `const` and `let` declarations in the compiled JS are block-scoped. Inside `eval()`, they're scoped to the eval block, not the global scope. Only `var` and `function` declarations create true globals.

This means if one `<script type="text/lykn">` declares `(const x 42)` and another tries to use `x`, it won't work — `const x` is scoped to the first eval call.

This is a known limitation of the compile-then-eval approach. Real-world browser lykn code will typically be self-contained within a single `<script>` tag, so this rarely matters. For multi-script pages that need shared state, `window.myVar = value` works (the programmer can assign to `window:my-var` in lykn).

For v0.1.0, document this limitation but don't try to solve it. It affects every compile-to-JS browser shim.

### Compiler Pitfall: `readyState` Check

The `document.readyState` check handles the case where the shim script is loaded AFTER the DOM is already parsed (e.g., if the script tag has `defer` or is loaded dynamically). If we only listened for `DOMContentLoaded`, we'd miss it in those cases.

### Compiler Pitfall: Processing Order

`querySelectorAll` returns elements in document order, and we process them sequentially with `for...of`. This matters because scripts may depend on each other — later scripts expect earlier ones to have already executed.

The `for...of` with `await` inside ensures `src`-based scripts complete (including fetch) before the next script runs.

### Compiler Pitfall: `typeof document !== 'undefined'`

The browser environment check ensures this module doesn't crash if imported in Deno/Node (which don't have `document`). The `compileLykn`, `run`, and `load` functions work in any JS environment; only the auto-scan requires a DOM.

---

## 6.2 Bundling with esbuild

### Install esbuild

esbuild is available as a standalone binary. For the Deno-based project, install it via npm (the only npm dependency, used as a build tool, not a runtime dependency):

```sh
# Using Deno to run esbuild (npx equivalent)
deno run -A npm:esbuild src/lykn-browser.js \
  --bundle \
  --format=iife \
  --global-name=lykn \
  --outfile=dist/lykn-browser.js \
  --minify
```

Or if esbuild is installed globally:

```sh
esbuild src/lykn-browser.js \
  --bundle \
  --format=iife \
  --global-name=lykn \
  --outfile=dist/lykn-browser.js \
  --minify
```

### What the Flags Mean

| Flag | Purpose |
|------|---------|
| `--bundle` | Resolves all imports and inlines them into one file |
| `--format=iife` | Wraps everything in an Immediately Invoked Function Expression — no ES module syntax in the output |
| `--global-name=lykn` | The IIFE assigns its exports to `window.lykn` (or `globalThis.lykn`) |
| `--outfile=dist/lykn-browser.js` | Output path |
| `--minify` | Minifies for production |

### Also Produce an Unminified Version

For debugging, build an unminified version too:

```sh
deno run -A npm:esbuild src/lykn-browser.js \
  --bundle \
  --format=iife \
  --global-name=lykn \
  --outfile=dist/lykn-browser.dev.js
```

### Create the `dist/` Directory

```sh
mkdir -p dist
```

### What esbuild Does With Imports

The source has:
```js
import { read } from './reader.js';
import { compile } from './compiler.js';
// compiler.js imports: import { generate } from 'astring';
```

esbuild follows the import chain:
1. `src/lykn-browser.js` → `src/reader.js` (inlined)
2. `src/lykn-browser.js` → `src/compiler.js` (inlined)
3. `src/compiler.js` → `astring` (resolved via `deno.json` import map → `npm:astring`, inlined)

The result is a single file with zero imports — pure self-contained JS.

### Compiler Pitfall: Import Map Resolution

esbuild needs to resolve the `astring` bare specifier. The `deno.json` maps `"astring"` → `"npm:astring@^1.9.0"`. esbuild may not read `deno.json` import maps automatically.

**Option A**: Install astring locally for the build:
```sh
# Create a minimal package.json for the build step
echo '{"dependencies":{"astring":"^1.9.0"}}' > /tmp/build-pkg.json
cd /tmp && npm install && cd -
# Then point esbuild to the node_modules
deno run -A npm:esbuild src/lykn-browser.js \
  --bundle --format=iife --global-name=lykn \
  --outfile=dist/lykn-browser.js --minify \
  --node-path=/tmp/node_modules
```

**Option B** (simpler): Use esbuild's `--alias` flag or `--external` to handle the mapping.

**Option C** (recommended): Since the project uses Deno, use Deno's own bundler if esbuild has trouble with the import map:
```sh
deno bundle src/lykn-browser.js dist/lykn-browser.js
```

**Note**: `deno bundle` is deprecated but still works. If it's unavailable, esbuild is the fallback. Try esbuild first — it usually handles npm specifiers when run via `deno run -A npm:esbuild`.

**Option D** (most reliable): Create a small build script that handles the import resolution explicitly:

```js
// build.js — run with: deno run -A build.js
import * as esbuild from "npm:esbuild";

await esbuild.build({
  entryPoints: ["src/lykn-browser.js"],
  bundle: true,
  format: "iife",
  globalName: "lykn",
  outfile: "dist/lykn-browser.js",
  minify: true,
});

await esbuild.build({
  entryPoints: ["src/lykn-browser.js"],
  bundle: true,
  format: "iife",
  globalName: "lykn",
  outfile: "dist/lykn-browser.dev.js",
  minify: false,
});

console.log("Build complete: dist/lykn-browser.js and dist/lykn-browser.dev.js");
esbuild.stop();
```

This approach uses Deno's npm specifier resolution for esbuild itself, which handles the `astring` import correctly because Deno resolves it via the `deno.json` import map.

---

## 6.3 The `window.lykn` API

After loading the bundled script, users have access to:

```js
// In browser console or other scripts:
window.lykn.compile('(console:log "hello")')
// Returns: 'console.log("hello");\n'

window.lykn.run('(console:log "hello")')
// Executes: console.log("hello") — prints "hello" to console

await window.lykn.load('/app.lykn')
// Fetches, compiles, and executes the file
```

### What esbuild's `--global-name=lykn` Does

With `--format=iife` and `--global-name=lykn`, esbuild wraps the bundle like:

```js
var lykn = (() => {
  // ... all bundled code ...
  return { compile: compileLykn, run, load };
})();
```

The exports from `src/lykn-browser.js` become properties on `window.lykn`. Make sure the export names in `lykn-browser.js` match the intended API:

- `export { compileLykn as compile }` → `window.lykn.compile`
- `export function run(source)` → `window.lykn.run`
- `export async function load(url)` → `window.lykn.load`

---

## 6.4 Error Handling

All errors during script processing are caught and logged with a `[lykn]` prefix:

```
[lykn] Error in inline script: SyntaxError: Unknown form: badform
[lykn] Error in https://example.com/app.lykn: TypeError: ...
[lykn] Failed to load https://example.com/missing.lykn: 404
```

Errors do NOT propagate — one broken script doesn't prevent subsequent scripts from running. Each script tag is independent.

The `processScripts` function already wraps each script in a try/catch. The `load` function throws on fetch failure, which is caught by the try/catch.

---

## 6.5 Test Pages

Create manual test pages. These aren't automated — they're HTML files you open in a browser to verify everything works.

### `examples/browser.html` — Inline Script

This is Integration Test 4 from the project plan:

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>lykn Browser Test — Inline</title>
  <script src="../dist/lykn-browser.dev.js"></script>
</head>
<body>
  <h1>lykn Browser Test</h1>
  <div id="output"></div>

  <script type="text/lykn">
  (const el (document:get-element-by-id "output"))
  (const items (array "one" "two" "three"))
  (const html (items:map (=> (item i)
    (template "<p>" (+ i 1) ". " item "</p>"))))
  (= el:inner-HTML (html:join ""))
  </script>

  <p>If you see a numbered list above, the inline script worked.</p>
</body>
</html>
```

### `examples/browser-src.html` — External Script via `src`

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>lykn Browser Test — External</title>
  <script src="../dist/lykn-browser.dev.js"></script>
</head>
<body>
  <h1>lykn Browser Test — External</h1>
  <div id="output"></div>

  <script type="text/lykn" src="browser-app.lykn"></script>

  <p>If you see content above, the external script loaded.</p>
</body>
</html>
```

### `examples/browser-app.lykn` — External Lykn Source

```lisp
;; External lykn file loaded via <script src="...">
(const el (document:get-element-by-id "output"))
(const now (new Date))
(= el:inner-HTML (template "<p>Loaded at " (now:to-locale-time-string) "</p>"))
```

### `examples/browser-api.html` — Programmatic API

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>lykn Browser Test — API</title>
  <script src="../dist/lykn-browser.dev.js"></script>
</head>
<body>
  <h1>lykn Browser Test — API</h1>
  <div id="output"></div>

  <script>
    // Test the programmatic API
    const js = lykn.compile('(+ 1 2)');
    document.getElementById('output').innerHTML = `
      <p>Compiled: <code>${js.trim()}</code></p>
      <p>Result: <code>${lykn.run('(+ 1 2)')}</code></p>
    `;
  </script>

  <p>If you see compiled output and "3" above, the API works.</p>
</body>
</html>
```

### How to Test

You need a local HTTP server to test (browsers block `file://` fetch for `src` attributes). Use Deno:

```sh
# From the project root:
deno run --allow-net --allow-read https://deno.land/std/http/file_server.ts .
```

Or the short form:

```sh
deno run -A jsr:@std/http/file-server .
```

Then open `http://localhost:4507/examples/browser.html` in a browser.

---

## 6.6 Add Build Target to `deno.json`

Add a task for building the browser bundle:

```json
{
  "tasks": {
    "build:browser": "deno run -A build.js",
    "test": "deno test test/"
  }
}
```

Or if using esbuild directly:

```json
{
  "tasks": {
    "build:browser": "deno run -A npm:esbuild src/lykn-browser.js --bundle --format=iife --global-name=lykn --outfile=dist/lykn-browser.js --minify"
  }
}
```

Also add `dist/` to `.gitignore` if it isn't already there (build artifacts shouldn't be committed):

```
dist/
```

Or alternatively, INCLUDE `dist/` in the repo so users can grab the browser build without running a build step. This is a project decision — check with Duncan. Either way, add the task.

---

## Summary of All Changes

| File | Action | Notes |
|------|--------|-------|
| `src/lykn-browser.js` | **New** | Browser entry point with compile/run/load + auto-scan |
| `build.js` | **New** | esbuild build script (or use CLI in deno task) |
| `dist/lykn-browser.js` | **Generated** | Minified IIFE bundle |
| `dist/lykn-browser.dev.js` | **Generated** | Unminified IIFE bundle |
| `examples/browser.html` | **New** | Inline script test page |
| `examples/browser-src.html` | **New** | External src test page |
| `examples/browser-app.lykn` | **New** | External lykn source for src test |
| `examples/browser-api.html` | **New** | Programmatic API test page |
| `deno.json` | **Modify** | Add `build:browser` task |

### What NOT to Do

- **Do not modify `src/compiler.js` or `src/reader.js`.** The compiler is done. Phase 6 is packaging only.
- **Do not add a runtime library.** The compiled JS has zero dependencies — that's a core design principle.
- **Do not try to solve the `const`/`let` scoping limitation.** It's inherent to `eval()` and affects every compile-to-JS browser shim. Document it, don't fight it.
- **Do not use `document.write()`.** It's destructive after page load and breaks everything.
- **Do not use `new Function()` instead of `eval()`.** `new Function()` creates function scope, not global scope. Variables wouldn't be visible to subsequent code.
- **Do not use Node.js or npm for the build.** Use `deno run -A npm:esbuild` or the `build.js` script with Deno.

---

## Verification Checklist

- [ ] `src/lykn-browser.js` exists and imports from reader + compiler
- [ ] `deno run -A build.js` (or equivalent) produces `dist/lykn-browser.js`
- [ ] `dist/lykn-browser.js` is a single file with no `import`/`export` statements
- [ ] Loading `dist/lykn-browser.js` in a browser creates `window.lykn`
- [ ] `window.lykn.compile('(+ 1 2)')` returns a JS string containing `1 + 2`
- [ ] `window.lykn.run('(+ 1 2)')` returns `3`
- [ ] `examples/browser.html` renders a numbered list from inline lykn
- [ ] `examples/browser-src.html` loads and runs an external `.lykn` file
- [ ] `examples/browser-api.html` shows compiled output and eval result
- [ ] Errors in lykn scripts appear in console with `[lykn]` prefix
- [ ] A broken script doesn't prevent subsequent scripts from running
- [ ] The bundle works in Chrome, Firefox, and Safari (basic smoke test)
- [ ] ALL Phase 1–5 unit tests still pass (no regressions from new files)
- [ ] `deno lint src/` passes (including the new `lykn-browser.js`)

---

## What Comes After Phase 6

Phase 6 completes v0.1.0. After this, run the four integration tests from the project plan:

1. **HTTP Server** (Test 1) — compile and run with Deno/Node
2. **CLI Tool** (Test 2) — compile and run from command line
3. **Utility Module** (Test 3) — compile, import from another module
4. **Browser Page** (Test 4) — the `examples/browser.html` from this phase

All four must work. When they do, v0.1.0 is done.
