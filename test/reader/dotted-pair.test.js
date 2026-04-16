import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";

Deno.test("dotted pair: basic", () => {
  assertEquals(read("(a . b)"), [
    { type: 'cons', car: { type: 'atom', value: 'a' }, cdr: { type: 'atom', value: 'b' } }
  ]);
});

Deno.test("dotted pair: with lists", () => {
  const result = read("(a . (b c))");
  assertEquals(result[0].type, 'cons');
  assertEquals(result[0].cdr.type, 'list');
});

Deno.test("dotted pair: nested", () => {
  const result = read("(a . (b . c))");
  assertEquals(result[0].type, 'cons');
  assertEquals(result[0].cdr.type, 'cons');
});

Deno.test("dotted pair: with numbers", () => {
  assertEquals(read("(1 . 2)"), [
    { type: 'cons', car: { type: 'number', value: 1 }, cdr: { type: 'number', value: 2 } }
  ]);
});

Deno.test("dotted pair: with strings", () => {
  const result = read('("a" . "b")');
  assertEquals(result[0].car.value, 'a');
  assertEquals(result[0].cdr.value, 'b');
});

Deno.test("dotted pair: with unquote", () => {
  const result = read("(,a . ,b)");
  assertEquals(result[0].type, 'cons');
  assertEquals(result[0].car.values[0].value, 'unquote');
  assertEquals(result[0].cdr.values[0].value, 'unquote');
});

Deno.test("dotted pair: car is list", () => {
  const result = read("((+ 1 2) . b)");
  assertEquals(result[0].car.type, 'list');
  assertEquals(result[0].cdr.value, 'b');
});

Deno.test("dot cannot be first", () => {
  assertThrows(() => read("(. a)"), Error, "first");
});

Deno.test("dot cannot be last", () => {
  assertThrows(() => read("(a .)"), Error, "nothing after dot");
});

Deno.test("dot with nothing after and close paren", () => {
  assertThrows(() => read("(a . )"), Error, "nothing after dot");
});

Deno.test("multiple dots error", () => {
  assertThrows(() => read("(a . b . c)"), Error, "one");
});

Deno.test("dot in nested list is independent", () => {
  const result = read("((a . b) (c . d))");
  assertEquals(result[0].type, 'list');
  assertEquals(result[0].values[0].type, 'cons');
  assertEquals(result[0].values[1].type, 'cons');
});

Deno.test("no dot produces normal list", () => {
  const result = read("(a b c)");
  assertEquals(result[0].type, 'list');
  assertEquals(result[0].values.length, 3);
});

Deno.test("multiple elements before dot throws", () => {
  assertThrows(() => read("(a b . c)"), Error, "one element");
});
