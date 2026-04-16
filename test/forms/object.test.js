import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("object: grouped pairs", () => {
  const result = lykn('(object (name "Duncan") (age 42))');
  assertEquals(result.includes('name'), true);
  assertEquals(result.includes('"Duncan"'), true);
  assertEquals(result.includes('age'), true);
  assertEquals(result.includes('42'), true);
});

Deno.test("object: shorthand", () => {
  const result = lykn('(object name age)');
  assertEquals(result.includes('name') && result.includes('age'), true);
});

Deno.test("object: spread", () => {
  const result = lykn('(object (name "x") (spread defaults))');
  assertEquals(result.includes('...defaults'), true);
});

Deno.test("object: camelCase keys", () => {
  const result = lykn('(object (my-name "Duncan"))');
  assertEquals(result.includes('myName'), true);
});

Deno.test("object: empty", () => {
  assertEquals(lykn('(object)'), '({});');
});

Deno.test("object: single-element sub-list throws", () => {
  assertThrows(() => lykn('(object (name))'));
});

Deno.test("object: computed key", () => {
  const result = lykn('(object ((computed key) "value"))');
  assertEquals(result.includes('[key]'), true);
});

Deno.test("object: string key", () => {
  const result = lykn('(object ("content-type" "text/plain"))');
  assertEquals(result.includes('"content-type"'), true);
});

Deno.test("object: mixed shorthand and pairs", () => {
  const result = lykn('(object (name "Duncan") age)');
  assertEquals(result.includes('"Duncan"'), true);
  assertEquals(result.includes('age'), true);
});
