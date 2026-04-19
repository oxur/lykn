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
  resetGensym,
  resetMacros,
  resetModuleCache,
} from "lang/expander.js";
import { lykn } from "lang/mod.js";

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

// Re-export commonly needed functions so test files don't need
// separate imports from lang/*.js
export { read } from "lang/reader.js";
export { expand, resetGensym, resetMacros, resetModuleCache } from "lang/expander.js";
