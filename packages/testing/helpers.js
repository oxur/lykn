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
 * Compile via BOTH compilers (JS and Rust) and verify convergence.
 * The Rust compiler is invoked via the `lykn` binary. If the binary is
 * not available, throws a clear error.
 * @param {string} source - lykn source text
 * @returns {string} compiled JavaScript (from JS compiler; Rust verified to match)
 */
export function compileBoth(source) {
  const jsOut = lykn(source).trim();

  // Write source to temp file for the Rust compiler
  const tmpPath = Deno.makeTempFileSync({ suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, source);
    const lyknBin = Deno.env.get("LYKN_BIN") || "./bin/lykn";
    const proc = new Deno.Command(lyknBin, {
      args: ["compile", tmpPath],
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

    // Normalize whitespace for comparison: collapse runs of whitespace,
    // strip trailing semicolons differences, etc.
    const normalize = (s) =>
      s.replace(/\s+/g, " ").replace(/;\s*}/g, "; }").trim();

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
