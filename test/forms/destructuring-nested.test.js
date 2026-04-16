import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("nested: object inside object via alias", () => {
  const result = lykn('(const (object (alias data (object name age))) response)');
  assertEquals(result.includes('data:'), true);
  assertEquals(result.includes('name'), true);
});

Deno.test("nested: array inside object via alias", () => {
  const result = lykn('(const (object (alias items (array first second))) response)');
  assertEquals(result.includes('items:'), true);
  assertEquals(result.includes('[first, second]'), true);
});

Deno.test("nested: array of arrays", () => {
  const result = lykn('(const (array (array a b) (array c d)) matrix)');
  assertEquals(result.includes('[['), true);
});

Deno.test("nested: deep object", () => {
  const result = lykn('(const (object (alias config (object (alias server (object host port))))) app)');
  assertEquals(result.includes('host'), true);
  assertEquals(result.includes('port'), true);
});
