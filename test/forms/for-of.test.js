import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("for-of: basic", () => {
  const result = lykn("(for-of item items (console:log item))");
  assertEquals(result.includes("for (const item of items)"), true);
});

Deno.test("for-of: missing args throws", () => {
  assertThrows(() => lykn("(for-of item)"));
});
