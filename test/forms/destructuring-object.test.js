import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("object pattern: shorthand", () => {
  const result = lykn('(const (object name age) person)');
  assertEquals(result.includes('{name, age}') || result.includes('{ name, age }'), true);
  assertEquals(result.includes('= person'), true);
});

Deno.test("object pattern: alias rename", () => {
  const result = lykn('(const (object (alias old-name new-name)) obj)');
  assertEquals(result.includes('oldName: newName'), true);
});

Deno.test("object pattern: default value", () => {
  const result = lykn('(const (object (default x 0)) point)');
  assertEquals(result.includes('x = 0'), true);
});

Deno.test("object pattern: alias with default", () => {
  const result = lykn('(const (object (alias name n "anon")) obj)');
  assertEquals(result.includes('name: n = "anon"'), true);
});

Deno.test("object pattern: rest", () => {
  const result = lykn('(const (object a (rest others)) obj)');
  assertEquals(result.includes('...others'), true);
});

Deno.test("object pattern: rest not last throws", () => {
  assertThrows(() => lykn('(const (object (rest others) a) obj)'));
});

Deno.test("object pattern: camelCase", () => {
  const result = lykn('(const (object my-name) person)');
  assertEquals(result.includes('myName'), true);
});

Deno.test("object pattern: mixed", () => {
  const result = lykn('(const (object name (alias data items) (default count 0)) resp)');
  assertEquals(result.includes('name'), true);
  assertEquals(result.includes('data: items'), true);
  assertEquals(result.includes('count = 0'), true);
});
