import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("throw: basic", () => {
  assertEquals(lykn('(throw (new Error "oops"))'), 'throw new Error("oops");');
});

Deno.test("throw: wrong arity throws", () => {
  assertThrows(() => lykn("(throw)"));
});
