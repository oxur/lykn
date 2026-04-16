import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros, resetModuleCache } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros(); resetGensym(); resetModuleCache();
  return compile(expand(read(source))).trim();
}

Deno.test("integration: cons/list/car/cdr desugaring", () => {
  const result = lykn(`
    (const pair (cons 1 2))
    (const my-list (list 10 20 30))
    (console:log (car pair))
    (console:log (cdr pair))
    (console:log (cadr my-list))
    (console:log (cddr my-list))
  `);

  assertEquals(result.includes("[1, 2]"), true);
  assertEquals(result.includes("[10, [20, [30, null]]]"), true);
  assertEquals(result.includes("pair[0]"), true);
  assertEquals(result.includes("pair[1]"), true);
  assertEquals(result.includes("myList[1][0]"), true);
  assertEquals(result.includes("myList[1][1]"), true);
});

Deno.test("integration: radix literals", () => {
  const result = lykn(`
    (const mask #2r11110000)
    (const color #16rff8800)
  `);
  assertEquals(result.includes("240"), true);
  assertEquals(result.includes("16746496"), true);
});

Deno.test("integration: #a and #o data literals", () => {
  const result = lykn(`
    (const nums #a(1 2 3))
    (const person #o((name "Duncan") (age 42)))
  `);
  assertEquals(result.includes("[1, 2, 3]"), true);
  assertEquals(result.includes("Duncan"), true);
});
