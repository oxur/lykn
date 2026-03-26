import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("for-in: basic", () => {
  const result = lykn("(for-in key obj (console:log key))");
  assertEquals(result.includes("for (const key in obj)"), true);
});

Deno.test("for-in: missing args throws", () => {
  assertThrows(() => lykn("(for-in key)"));
});
