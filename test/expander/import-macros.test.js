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

Deno.test("import-macros: basic import and use", () => {
  const result = lykn(
    `(import-macros "./basic-control.lykn" (when))
     (when true (console:log "yes"))`,
    resolve(fixturesDir, "test.lykn")
  );
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("yes"), true);
});

Deno.test("import-macros: multiple bindings", () => {
  const result = lykn(
    `(import-macros "./basic-control.lykn" (when unless))
     (when true (console:log "a"))
     (unless false (console:log "b"))`,
    resolve(fixturesDir, "test.lykn")
  );
  assertEquals(result.includes("a"), true);
  assertEquals(result.includes("b"), true);
});

Deno.test("import-macros: with as renaming", () => {
  const result = lykn(
    `(import-macros "./basic-control.lykn" ((as when my-when)))
     (my-when true (console:log "renamed"))`,
    resolve(fixturesDir, "test.lykn")
  );
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("renamed"), true);
});

Deno.test("import-macros: erased from output", () => {
  const result = lykn(
    `(import-macros "./basic-control.lykn" (when))
     (const x 1)`,
    resolve(fixturesDir, "test.lykn")
  );
  // No import-macros in output
  assertEquals(result.includes("import"), false);
  assertEquals(result, "const x = 1;");
});
