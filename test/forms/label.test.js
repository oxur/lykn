import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("label: basic", () => {
  const result = lykn("(label my-loop (while true (break my-loop)))");
  assertEquals(result.includes("myLoop:"), true);
  assertEquals(result.includes("break myLoop"), true);
});

Deno.test("label: wrong arity throws", () => {
  assertThrows(() => lykn("(label foo)"));
});
