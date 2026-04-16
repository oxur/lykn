import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("do-while: basic", () => {
  const result = lykn("(do-while (> x 0) (-= x 1))");
  assertEquals(result.includes("do"), true);
  assertEquals(result.includes("while"), true);
  assertEquals(result.includes("x > 0"), true);
});

Deno.test("do-while: missing body throws", () => {
  assertThrows(() => lykn("(do-while true)"));
});
