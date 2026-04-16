import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, expandExpr } from "lykn/expander.js";

function ex(source) {
  return expand(read(source));
}

Deno.test("as: simple rename", () => {
  const result = ex("(as foo bar)");
  assertEquals(result[0].values[0].value, "alias");
  assertEquals(result[0].values[1].value, "foo");
  assertEquals(result[0].values[2].value, "bar");
});

Deno.test("as: in import context", () => {
  const result = ex('(import (as some-module sm) "./mod.js")');
  // The (as ...) inside import gets expanded to (alias ...)
  const importForm = result[0];
  assertEquals(importForm.values[0].value, "import");
  assertEquals(importForm.values[1].values[0].value, "alias");
});

Deno.test("as: whole-and-destructure with const", () => {
  const result = ex("(const (as (object a b) whole) expr)");
  // Should produce two forms
  assertEquals(result.length, 2);
  assertEquals(result[0].values[0].value, "const");
  assertEquals(result[0].values[1].value, "whole");
  assertEquals(result[1].values[0].value, "const");
  assertEquals(result[1].values[1].values[0].value, "object");
});

Deno.test("as: whole-and-destructure with let", () => {
  const result = ex("(let (as (object x) w) val)");
  assertEquals(result.length, 2);
  assertEquals(result[0].values[0].value, "let");
  assertEquals(result[1].values[0].value, "let");
});

Deno.test("as: wrong arity throws", () => {
  assertThrows(() => ex("(as x)"), Error, "2 arguments");
});

Deno.test("as: wrong arity 3 args throws", () => {
  assertThrows(() => ex("(as x y z)"), Error, "2 arguments");
});
