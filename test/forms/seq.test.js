import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("seq: two expressions", () => {
  assertEquals(lykn("(seq a b)"), "(a, b);");
});

Deno.test("seq: three expressions", () => {
  assertEquals(lykn("(seq a b c)"), "(a, b, c);");
});

Deno.test("seq: too few args throws", () => {
  assertThrows(() => lykn("(seq a)"));
});
