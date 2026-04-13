import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros, resetModuleCache } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros(); resetGensym(); resetModuleCache();
  return compile(expand(read(source))).trim();
}

Deno.test("integration: swap macro with auto-gensym", () => {
  // DD-22: surface `=` is now strict equality. User macros that need
  // assignment must use kernel operators directly. This test verifies
  // gensym hygiene using a read-only swap (captures values, doesn't assign).
  const result = lykn(`
    (macro swap-log (a b)
      \`(block
        (let temp#gen ,a)
        (console:log temp#gen ,b)))
    (swap-log x y)
  `);
  assertEquals(result.includes("temp__gensym0"), true);
  assertEquals(result.includes("let temp__gensym0 = x"), true);
  assertEquals(result.includes("console.log(temp__gensym0, y)"), true);
});

Deno.test("integration: full end-to-end with macros + classes + destructuring", () => {
  const result = lykn(`
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))

    (class Handler ()
      (field -count 0)
      (handle (req)
        (++ this:-count)
        (when req
          (console:log (template "handled #" this:-count)))))

    (const (object name (default age 0)) (get-user))
    (const items (list 1 2 3))
    (console:log (car items))
  `);

  assertEquals(result.includes("class Handler"), true);
  assertEquals(result.includes("#_count"), true);
  assertEquals(result.includes("if (req)"), true);
  assertEquals(result.includes("{name, age = 0}"), true);
  assertEquals(result.includes("[1, [2, [3, null]]]"), true);
  assertEquals(result.includes("items[0]"), true);
});
