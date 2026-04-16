import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("for: basic c-style", () => {
  const result = lykn("(for (let i 0) (< i 10) (++ i) (console:log i))");
  assertEquals(result.includes("for"), true);
  assertEquals(result.includes("i < 10"), true);
});

Deno.test("for: infinite loop with empty slots", () => {
  const result = lykn("(for () () () (break))");
  assertEquals(result.includes("for (; ; )"), true);
});

Deno.test("for: missing body throws", () => {
  assertThrows(() => lykn("(for (let i 0) (< i 10))"));
});
