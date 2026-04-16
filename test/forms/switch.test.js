import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("switch: basic with default", () => {
  const result = lykn('(switch x ("a" (do-a) (break)) ("b" (do-b) (break)) (default (do-default)))');
  assertEquals(result.includes("switch"), true);
  assertEquals(result.includes('case "a"'), true);
  assertEquals(result.includes("default:"), true);
});

Deno.test("switch: missing cases throws", () => {
  assertThrows(() => lykn("(switch x)"));
});
