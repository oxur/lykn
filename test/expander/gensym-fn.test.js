import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("gensym function: available in macro body", () => {
  const result = lykn(`
    (macro with-temp (val (rest body))
      (const tmp ($gensym "tmp"))
      \`(block (let ,tmp ,val) ,@body))
    (with-temp 42 (console:log "done"))
  `);
  assertEquals(result.includes("tmp__gensym"), true);
});

Deno.test("gensym function: default prefix", () => {
  const result = lykn(`
    (macro with-gen (val)
      (const tmp ($gensym))
      \`(let ,tmp ,val))
    (with-gen 99)
  `);
  assertEquals(result.includes("g__gensym"), true);
});
