import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("ternary: basic", () => {
  assertEquals(lykn('(? (> x 0) "yes" "no")'), 'x > 0 ? "yes" : "no";');
});

Deno.test("ternary: nested in const", () => {
  assertEquals(lykn('(const result (? flag 1 0))'), "const result = flag ? 1 : 0;");
});

Deno.test("ternary: wrong arity throws", () => {
  assertThrows(() => lykn("(? a b)"));
});
