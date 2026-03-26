import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("array pattern: basic", () => {
  const result = lykn('(const (array first second) arr)');
  assertEquals(result.includes('[first, second]'), true);
});

Deno.test("array pattern: skip with _", () => {
  const result = lykn('(const (array _ second) pair)');
  assertEquals(result.includes('[, second]'), true);
});

Deno.test("array pattern: multiple skips", () => {
  const result = lykn('(const (array _ _ third) arr)');
  assertEquals(result.includes('[, , third]'), true);
});

Deno.test("array pattern: default", () => {
  const result = lykn('(const (array (default x 0) (default y 0)) point)');
  assertEquals(result.includes('x = 0'), true);
  assertEquals(result.includes('y = 0'), true);
});

Deno.test("array pattern: rest", () => {
  const result = lykn('(const (array head (rest tail)) list)');
  assertEquals(result.includes('...tail'), true);
});

Deno.test("array pattern: rest not last throws", () => {
  assertThrows(() => lykn('(const (array (rest head) tail) list)'));
});
