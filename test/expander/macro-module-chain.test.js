import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros, resetModuleCache } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";
import { resolve, dirname } from "node:path";
import { fromFileUrl } from "https://deno.land/std/path/mod.ts";

const testDir = dirname(fromFileUrl(import.meta.url));
const fixturesDir = resolve(testDir, "../fixtures/macros");

function lykn(source, filePath) {
  resetMacros();
  resetGensym();
  resetModuleCache();
  return compile(expand(read(source), { filePath })).trim();
}

Deno.test("macro-module-chain: A imports from B which imports from C", () => {
  // advanced-control imports from basic-control
  const result = lykn(
    `(import-macros "./advanced-control.lykn" (when-not))
     (when-not false (console:log "chain works"))`,
    resolve(fixturesDir, "test.lykn")
  );
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("chain works"), true);
});
