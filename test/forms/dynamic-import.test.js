import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("dynamic-import: basic", () => {
  assertEquals(lykn('(dynamic-import "./mod.js")'), 'import("./mod.js");');
});

Deno.test("dynamic-import: with await", () => {
  assertEquals(lykn('(const mod (await (dynamic-import "./mod.js")))'),
    'const mod = await import("./mod.js");');
});

Deno.test("dynamic-import: wrong arity throws", () => {
  assertThrows(() => lykn("(dynamic-import)"));
});
