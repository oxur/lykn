import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("regex: pattern only", () => {
  assertEquals(lykn('(regex "^hello")'), "/^hello/;");
});

Deno.test("regex: pattern + flags", () => {
  assertEquals(lykn('(regex "^hello" "gi")'), "/^hello/gi;");
});

Deno.test("regex: wrong arity throws", () => {
  assertThrows(() => lykn("(regex)"));
});

Deno.test("regex: non-string pattern throws", () => {
  assertThrows(() => lykn("(regex foo)"));
});
