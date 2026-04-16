import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("macro params: rest captures remaining", () => {
  const result = lykn(`
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))
    (when true (console:log "a") (console:log "b"))
  `);
  assertEquals(result.includes("a"), true);
  assertEquals(result.includes("b"), true);
});

Deno.test("macro params: single param", () => {
  const result = lykn(`
    (macro not-null (x) \`(!== ,x null))
    (not-null y)
  `);
  assertEquals(result.includes("!=="), true);
  assertEquals(result.includes("null"), true);
});

Deno.test("macro params: skip with unused param", () => {
  const result = lykn(`
    (macro second (unused x) x)
    (second foo bar)
  `);
  assertEquals(result, "bar;");
});
