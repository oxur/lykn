import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("debugger: basic", () => {
  assertEquals(lykn("(debugger)"), "debugger;");
});

Deno.test("debugger: with args throws", () => {
  assertThrows(() => lykn("(debugger foo)"));
});
