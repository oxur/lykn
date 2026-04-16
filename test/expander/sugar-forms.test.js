import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand } from "lykn/expander.js";

function ex(source) {
  return expand(read(source));
}

function first(source) {
  return ex(source)[0];
}

// --- cons ---

Deno.test("cons: basic pair", () => {
  const result = first("(cons x y)");
  assertEquals(result.values[0].value, "array");
  assertEquals(result.values[1].value, "x");
  assertEquals(result.values[2].value, "y");
});

Deno.test("cons: nested", () => {
  const result = first("(cons (cons a b) c)");
  assertEquals(result.values[0].value, "array");
  assertEquals(result.values[1].values[0].value, "array");
});

Deno.test("cons: too few args throws", () => {
  assertThrows(() => ex("(cons x)"), Error, "2 arguments");
});

Deno.test("cons: too many args throws", () => {
  assertThrows(() => ex("(cons x y z)"), Error, "2 arguments");
});

// --- list ---

Deno.test("list: three elements", () => {
  const result = first("(list a b c)");
  // (array a (array b (array c null)))
  assertEquals(result.values[0].value, "array");
  assertEquals(result.values[1].value, "a");
  const inner = result.values[2];
  assertEquals(inner.values[0].value, "array");
  assertEquals(inner.values[1].value, "b");
  const innermost = inner.values[2];
  assertEquals(innermost.values[0].value, "array");
  assertEquals(innermost.values[1].value, "c");
  assertEquals(innermost.values[2].value, "null");
});

Deno.test("list: single element", () => {
  const result = first("(list a)");
  assertEquals(result.values[0].value, "array");
  assertEquals(result.values[1].value, "a");
  assertEquals(result.values[2].value, "null");
});

Deno.test("list: empty", () => {
  const result = first("(list)");
  assertEquals(result.type, "atom");
  assertEquals(result.value, "null");
});

Deno.test("list: nested list call", () => {
  const result = first("(list (list a) b)");
  assertEquals(result.values[0].value, "array");
  // first element is (array a null)
  assertEquals(result.values[1].values[0].value, "array");
});

// --- car ---

Deno.test("car: basic", () => {
  const result = first("(car x)");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].value, "x");
  assertEquals(result.values[2].value, 0);
});

Deno.test("car: nested", () => {
  const result = first("(car (car x))");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].values[0].value, "get");
});

Deno.test("car: wrong arity throws", () => {
  assertThrows(() => ex("(car)"), Error, "1 argument");
});

// --- cdr ---

Deno.test("cdr: basic", () => {
  const result = first("(cdr x)");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].value, "x");
  assertEquals(result.values[2].value, 1);
});

Deno.test("cdr: wrong arity throws", () => {
  assertThrows(() => ex("(cdr x y)"), Error, "1 argument");
});

// --- cadr ---

Deno.test("cadr: basic", () => {
  const result = first("(cadr x)");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].values[0].value, "get");
  assertEquals(result.values[1].values[2].value, 1);
  assertEquals(result.values[2].value, 0);
});

// --- cddr ---

Deno.test("cddr: basic", () => {
  const result = first("(cddr x)");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].values[2].value, 1);
  assertEquals(result.values[2].value, 1);
});

// --- car/cdr of cons ---

Deno.test("car of cons", () => {
  const result = first("(car (cons a b))");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].values[0].value, "array");
});

Deno.test("cdr of cons", () => {
  const result = first("(cdr (cons a b))");
  assertEquals(result.values[0].value, "get");
  assertEquals(result.values[1].values[0].value, "array");
  assertEquals(result.values[2].value, 1);
});
