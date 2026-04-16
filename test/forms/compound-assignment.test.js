import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("+=: basic", () => {
  assertEquals(lykn("(+= x 1)"), "x += 1;");
});

Deno.test("-=: basic", () => {
  assertEquals(lykn("(-= x 1)"), "x -= 1;");
});

Deno.test("*=: basic", () => {
  assertEquals(lykn("(*= x 2)"), "x *= 2;");
});

Deno.test("**=: exponentiation assignment", () => {
  assertEquals(lykn("(**= x 2)"), "x **= 2;");
});

Deno.test("&&=: logical and assignment", () => {
  assertEquals(lykn("(&&= x y)"), "x &&= y;");
});

Deno.test("||=: logical or assignment", () => {
  assertEquals(lykn("(||= x y)"), "x ||= y;");
});

Deno.test("??=: nullish coalescing assignment", () => {
  assertEquals(lykn("(??= x y)"), "x ??= y;");
});

Deno.test("+=: wrong arity throws", () => {
  assertThrows(() => lykn("(+= x)"));
});
