import { assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros, resetModuleCache } from "lykn/expander.js";
import { resolve, dirname } from "node:path";
import { fromFileUrl } from "https://deno.land/std/path/mod.ts";

const testDir = dirname(fromFileUrl(import.meta.url));
const fixturesDir = resolve(testDir, "../fixtures/macros");

function ex(source, filePath) {
  resetMacros();
  resetGensym();
  resetModuleCache();
  return expand(read(source), { filePath });
}

Deno.test("import-macros error: file not found", () => {
  assertThrows(
    () => ex('(import-macros "./nonexistent.lykn" (foo))', resolve(fixturesDir, "test.lykn")),
    Error, "not found"
  );
});

Deno.test("import-macros error: macro not exported", () => {
  assertThrows(
    () => ex('(import-macros "./unexported.lykn" (internal-only))', resolve(fixturesDir, "test.lykn")),
    Error, "not exported"
  );
});

Deno.test("import-macros error: circular dependency", () => {
  assertThrows(
    () => ex('(import-macros "./circular-a.lykn" (foo))', resolve(fixturesDir, "test.lykn")),
    Error, "circular"
  );
});

Deno.test("import-macros error: duplicate import of same path", () => {
  assertThrows(
    () => ex(
      `(import-macros "./basic-control.lykn" (when))
       (import-macros "./basic-control.lykn" (unless))`,
      resolve(fixturesDir, "test.lykn")
    ),
    Error, "duplicate"
  );
});

Deno.test("import-macros error: missing binding list", () => {
  assertThrows(
    () => ex('(import-macros "./basic-control.lykn")', resolve(fixturesDir, "test.lykn")),
    Error, "binding list"
  );
});

Deno.test("import-macros error: invalid path (not relative)", () => {
  assertThrows(
    () => ex('(import-macros "basic-control.lykn" (when))', resolve(fixturesDir, "test.lykn")),
    Error, "relative"
  );
});

Deno.test("import-macros error: invalid path (no .lykn extension)", () => {
  assertThrows(
    () => ex('(import-macros "./basic-control.js" (when))', resolve(fixturesDir, "test.lykn")),
    Error, ".lykn"
  );
});

Deno.test("import-macros error: shadowing imported macro", () => {
  assertThrows(
    () => ex(
      `(import-macros "./basic-control.lykn" (when))
       (macro when (test (rest body)) \`(if ,test (block ,@body)))`,
      resolve(fixturesDir, "test.lykn")
    ),
    Error, "duplicate"
  );
});
