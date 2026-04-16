import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros, resetModuleCache } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";
import { resolve, dirname } from "node:path";
import { fromFileUrl } from "https://deno.land/std/path/mod.ts";

const fixturesDir = resolve(dirname(fromFileUrl(import.meta.url)), "../fixtures/macros");

function lykn(source) {
  resetMacros(); resetGensym(); resetModuleCache();
  return compile(expand(read(source), { filePath: resolve(fixturesDir, "test.lykn") })).trim();
}

Deno.test("integration: import macros from module", () => {
  const result = lykn(`
    (import-macros "./basic-control.lykn" (when unless))
    (when (> x 0) (console:log "positive"))
    (unless (=== y 0) (console:log "nonzero"))
  `);

  assertEquals(result.includes("if (x > 0)"), true);
  assertEquals(result.includes("if (!(y === 0))"), true);
  // No import-macros in output
  assertEquals(result.includes("import"), false);
});

Deno.test("integration: cross-module macro chain", () => {
  const result = lykn(`
    (import-macros "./advanced-control.lykn" (when-not))
    (when-not false (console:log "works"))
  `);
  assertEquals(result.includes("if (!false)"), true);
});
