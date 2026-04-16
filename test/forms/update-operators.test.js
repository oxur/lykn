import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("++: prefix increment", () => {
  assertEquals(lykn("(++ x)"), "++x;");
});

Deno.test("--: prefix decrement", () => {
  assertEquals(lykn("(-- x)"), "--x;");
});

Deno.test("++: wrong arity throws", () => {
  assertThrows(() => lykn("(++ x y)"));
});
