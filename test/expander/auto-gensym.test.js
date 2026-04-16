import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

Deno.test("auto-gensym: temp#gen in let binding", () => {
  const result = lykn(`
    (macro swap (a b)
      \`(block
        (let temp#gen ,a)
        (= ,a ,b)
        (= ,b temp#gen)))
    (swap x y)
  `);
  // temp#gen should resolve to temp__gensym0
  assertEquals(result.includes("temp__gensym0"), true);
});

Deno.test("auto-gensym: same prefix → same name within template", () => {
  const result = lykn(`
    (macro swap (a b)
      \`(block
        (let temp#gen ,a)
        (= ,a ,b)
        (= ,b temp#gen)))
    (swap x y)
  `);
  // All temp#gen occurrences → same gensym
  const matches = result.match(/temp__gensym0/g);
  assertEquals(matches !== null && matches.length >= 2, true);
});

Deno.test("auto-gensym: different prefixes → different names", () => {
  const result = lykn(`
    (macro two-lets (x y)
      \`(block
        (let a#gen ,x)
        (let b#gen ,y)))
    (two-lets 1 2)
  `);
  assertEquals(result.includes("a__gensym0"), true);
  assertEquals(result.includes("b__gensym1"), true);
});

Deno.test("auto-gensym: each macro invocation gets fresh gensyms", () => {
  const result = ex(`
    (macro with-temp (val (rest body))
      \`(block (let t#gen ,val) ,@body))
    (with-temp 1 (console:log t#gen))
    (with-temp 2 (console:log t#gen))
  `);
  // Two separate expansions should get different gensyms
  assertEquals(result.length, 2);
});
