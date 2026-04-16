import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("sym escape: creates exact named symbol", () => {
  const result = lykn(`
    (macro aif (test then else-branch)
      \`(block
        (let it#gen ,test)
        (if it#gen ,then ,else-branch)))
    (aif (find-user id)
      (console:log "found")
      (console:log "not found"))
  `);
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("findUser"), true);
});
