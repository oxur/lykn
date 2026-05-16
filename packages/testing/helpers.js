// Testing helper functions for lykn test files.
//
// These provide the `compile` function that `test-compiles` expects,
// plus variants for kernel-only and stateful (macro-resetting) compilation.
//
// Usage in test files:
//   (import "testing/helpers.js" (compile))           ;; surface tests
//   (import "testing/helpers.js" (compile-kernel))     ;; kernel form tests
//   (import "testing/helpers.js" (compile-all))        ;; integration tests

import { read } from "lang/reader.js";
import { compile as rawCompile } from "lang/compiler.js";
import {
  expand,
  resetGensym,
  resetMacros,
  resetModuleCache,
} from "lang/expander.js";

// Local `lykn(source)` — same definition as @lykn/lang's mod.js.
// Reconstructed here to decouple from the @lykn/lang sub-path exports
// (Finding D — see workbench/finding-d-lang-exports-gap-2026-05-12.md).
function lykn(source) {
  return rawCompile(expand(read(source)));
}

/**
 * Compile lykn surface source to JavaScript (trimmed).
 * This is what `test-compiles` calls as `compile`.
 * @param {string} source - lykn source text
 * @returns {string} compiled JavaScript
 */
export function compile(source) {
  return lykn(source).trim();
}

/**
 * Compile kernel-only lykn source to JavaScript (trimmed).
 * Bypasses the surface macro expansion pass.
 * @param {string} source - kernel lykn source text
 * @returns {string} compiled JavaScript
 */
export function compileKernel(source) {
  return rawCompile(read(source)).trim();
}

/**
 * Compile with full state reset (macros, gensym, module cache).
 * Use for integration tests that define inline macros.
 * @param {string} source - lykn source text
 * @returns {string} compiled JavaScript
 */
export function compileAll(source) {
  resetMacros();
  resetGensym();
  resetModuleCache();
  return lykn(source).trim();
}

/**
 * Compile via BOTH compilers (JS and Rust) and verify convergent output.
 *
 * Compiles `source` through the JS pipeline (reader → expander →
 * compiler → astring) and independently through the Rust binary
 * (`lykn compile`). Normalizes both outputs and asserts equality.
 * Returns the JS output on success; throws on divergence.
 *
 * ## Source-context-path mechanism
 *
 * The Rust compiler is invoked with `--source-context-path` set to
 * `Deno.cwd()`. This tells the Rust compiler to resolve relative
 * imports (e.g., `(import-macros "./packages/testing" ...)`) from
 * the project root rather than from the temp file's directory.
 *
 * ASSUMPTION: callers MUST invoke from the project root (the
 * directory containing `project.json`). `lykn test` always sets
 * cwd to the project root, so this holds for all standard test
 * invocations.
 *
 * ## Normalizer
 *
 * The comparison normalizes known-benign differences between the
 * two compilers' outputs. See the normalizer policy comment above
 * the `normalize` function for the full list of transformations,
 * rationales, and the forbidden-extension policy.
 *
 * Default action on a new divergence: FIX the divergence in one
 * compiler, do not extend the normalizer. A normalizer extension
 * hides a real difference; a compiler fix eliminates it. Normalizer
 * extensions require explicit rationale in the policy comment AND
 * a reference to the closing-report that surfaced the need.
 *
 * ## Known reliable convergence
 *
 * compileBoth reliably converges on:
 * - Simple kernel forms (for, while, if, try, etc.)
 * - Surface forms (bind, func, match, type, etc.)
 * - Sources using (import-macros ...) with runtime-import
 *   declarations (as of the runtime-import Rust fix)
 * - Destructuring (object, array, nested)
 * - Class expressions, class fields, class async methods
 * - Export, import, colon-syntax, camelCase conversion
 *
 * Known divergence classes (formatting, not semantic) that prevent
 * compileBoth for some forms as of M16-2:
 * - Tagged templates, generators, some async wrapping patterns,
 *   certain object literal / class method formatting, default
 *   parameters with multiple defaults, some destructuring-
 *   assignment patterns. These are tracked for resolution.
 *
 * @param {string} source - lykn source text
 * @returns {string} compiled JavaScript (from JS compiler; Rust
 *   verified to match)
 */
export function compileBoth(source) {
  const jsOut = lykn(source).trim();

  // Write source to temp file for the Rust compiler
  const tmpPath = Deno.makeTempFileSync({ suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, source);
    const lyknBin = Deno.env.get("LYKN_BIN") || "./bin/lykn";
    const projectRoot = Deno.cwd();
    const proc = new Deno.Command(lyknBin, {
      args: ["compile", "--source-context-path", projectRoot, tmpPath],
      stdout: "piped",
      stderr: "piped",
    }).outputSync();

    if (!proc.success) {
      const stderr = new TextDecoder().decode(proc.stderr);
      throw new Error(
        `Rust compiler failed:\n${stderr}\n--- JS output was ---\n${jsOut}`
      );
    }

    let rustOut = new TextDecoder().decode(proc.stdout).trim();
    // Strip Rust-side warnings (lines containing ": warning:" or "  suggestion:")
    rustOut = rustOut
      .split("\n")
      .filter((l) => !l.includes(": warning:") && !l.startsWith("  suggestion:"))
      .join("\n")
      .trim();

    // ── Normalizer — cross-compiler output comparison ──
    //
    // Each transformation below exists because the JS and Rust compilers
    // produce structurally equivalent but textually different output in
    // specific, understood ways. The normalizer makes these known-benign
    // differences invisible to the equality check.
    //
    // Transformations (each with rationale):
    //
    // 1. Whitespace collapse (\s+ → " "): the two codegen backends
    //    (astring for JS, emit.rs for Rust) use different indentation
    //    and newline strategies. The compiled JS is semantically
    //    identical regardless of whitespace.
    //
    // 2. Trailing-semicolon canonicalization (;\s*} → "; }"): astring
    //    sometimes omits the semicolon before a closing brace; Rust's
    //    codegen always includes it. Both are valid JS.
    //
    // 3. Gensym-counter canonicalization (__gensymN): the JS compiler's
    //    gensym counter is process-global (increments across tests in a
    //    single Deno process), while Rust's resets per invocation. The
    //    generated names are internal and never user-visible.
    //
    // 4. Strip trailing ';' after '}' (}\s*; → }): Rust's codegen
    //    appends a semicolon after function declarations and other
    //    brace-terminated statements. Syntactically valid but unnecessary;
    //    JS (via astring) omits it. Rationale: M16-2 C-3 fast-follow.
    //
    // ── Normalizer extension policy ──
    //
    // Default action: FIX the divergence in one compiler, do not extend
    // the normalizer. A normalizer extension hides a real difference;
    // a compiler fix eliminates it.
    //
    // Any new normalizer transformation MUST be added with:
    // (a) an explicit rationale comment in this list, AND
    // (b) a reference to the closing-report or fast-follow note that
    //     surfaced the need.
    //
    const normalize = (s) =>
      s.replace(/\s+/g, " ")
        .replace(/;\s*}/g, "; }")
        .replace(/}\s*;/g, "}")
        .replace(/__gensym\d+/g, "__gensymN")
        .trim();

    if (normalize(jsOut) !== normalize(rustOut)) {
      throw new Error(
        `cross-compiler divergence:\n--- JS ---\n${jsOut}\n--- Rust ---\n${rustOut}`
      );
    }
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }

  return jsOut;
}

// Re-export commonly needed functions so test files don't need
// separate imports from lang/*.js
export { read } from "lang/reader.js";
export { expand, resetGensym, resetMacros, resetModuleCache } from "lang/expander.js";
