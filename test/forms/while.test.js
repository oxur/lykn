import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("while: basic", () => {
  const result = lykn("(while (> x 0) (-= x 1))");
  assertEquals(result.includes("while"), true);
  assertEquals(result.includes("x > 0"), true);
});

Deno.test("while: missing body throws", () => {
  assertThrows(() => lykn("(while true)"));
});
