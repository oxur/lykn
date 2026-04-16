import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";

Deno.test("#a() empty array", () => {
  const result = read("#a()");
  assertEquals(result[0].type, 'list');
  assertEquals(result[0].values[0].value, 'array');
  assertEquals(result[0].values.length, 1);
});

Deno.test("#a(...) with elements", () => {
  const result = read("#a(1 2 3)");
  assertEquals(result[0].values[0].value, 'array');
  assertEquals(result[0].values.length, 4);
});

Deno.test("#a(...) with nested", () => {
  const result = read("#a(1 (+ 2 3))");
  assertEquals(result[0].values[0].value, 'array');
  assertEquals(result[0].values[2].type, 'list');
});

Deno.test("#o() empty object", () => {
  const result = read("#o()");
  assertEquals(result[0].values[0].value, 'object');
  assertEquals(result[0].values.length, 1);
});

Deno.test("#o(...) with pairs", () => {
  const result = read('#o((name "Duncan") (age 42))');
  assertEquals(result[0].values[0].value, 'object');
  assertEquals(result[0].values.length, 3);
});

Deno.test("#o(...) with shorthand", () => {
  const result = read("#o(name (age 42))");
  assertEquals(result[0].values[0].value, 'object');
  assertEquals(result[0].values[1].value, 'name');
});

Deno.test("#(...) without letter errors", () => {
  assertThrows(() => read("#(1 2 3)"), Error, "#a(...)");
});

Deno.test("# followed by unknown char errors", () => {
  assertThrows(() => read("#z"), Error, "unknown dispatch");
});

Deno.test("# at end of input errors", () => {
  assertThrows(() => read("#"), Error);
});

Deno.test("#a inside list", () => {
  const result = read("(const x #a(1 2))");
  assertEquals(result[0].values[2].values[0].value, 'array');
});

Deno.test("#o inside list", () => {
  const result = read('(const x #o((a 1)))');
  assertEquals(result[0].values[2].values[0].value, 'object');
});
