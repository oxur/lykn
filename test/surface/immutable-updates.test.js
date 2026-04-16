import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

// --- assoc ---

Deno.test("assoc: single key-value", () => {
  const result = lykn("(assoc obj :age 43)");
  assertEquals(result, "({\n  ...obj,\n  age: 43\n});");
});

Deno.test("assoc: multiple key-values", () => {
  const result = lykn("(assoc obj :a 1 :b 2)");
  assertEquals(result, "({\n  ...obj,\n  a: 1,\n  b: 2\n});");
});

Deno.test("assoc: kebab-case key", () => {
  const result = lykn('(assoc obj :first-name "Duncan")');
  assertEquals(result, '({\n  ...obj,\n  firstName: "Duncan"\n});');
});

Deno.test("assoc: expression value", () => {
  const result = lykn("(assoc obj :score (+ base bonus))");
  assertEquals(result, "({\n  ...obj,\n  score: base + bonus\n});");
});

Deno.test("assoc: expansion produces object with spread", () => {
  const result = ex("(assoc obj :age 43)");
  const expanded = result[0];
  assertEquals(expanded.values[0].value, "object");
  // First child is (spread obj)
  assertEquals(expanded.values[1].values[0].value, "spread");
  assertEquals(expanded.values[1].values[1].value, "obj");
  // Second child is (age 43)
  assertEquals(expanded.values[2].values[0].value, "age");
  assertEquals(expanded.values[2].values[1].value, 43);
});

// --- dissoc ---

Deno.test("dissoc: single key", () => {
  const result = lykn("(dissoc obj :key)");
  // IIFE pattern: ((=> () (const {key: _, ...rest} = obj) rest))
  assertEquals(result.includes("..."), true);
  assertEquals(result.includes("obj"), true);
});

Deno.test("dissoc: multiple keys", () => {
  const result = lykn("(dissoc obj :a :b)");
  assertEquals(result.includes("..."), true);
  assertEquals(result.includes("obj"), true);
});

Deno.test("dissoc: expansion produces IIFE", () => {
  const result = ex("(dissoc obj :key)");
  const expanded = result[0];
  // Outer form is a call (list with one element — the IIFE)
  assertEquals(expanded.type, "list");
  // The called expression is an arrow function
  const arrow = expanded.values[0];
  assertEquals(arrow.values[0].value, "=>");
});

// --- conj ---

Deno.test("conj: append to array", () => {
  const result = lykn("(conj arr 42)");
  assertEquals(result, "[...arr, 42];");
});

Deno.test("conj: append expression", () => {
  const result = lykn("(conj items (+ 1 2))");
  assertEquals(result, "[...items, 1 + 2];");
});

Deno.test("conj: expansion produces array with spread", () => {
  const result = ex("(conj arr val)");
  const expanded = result[0];
  assertEquals(expanded.values[0].value, "array");
  assertEquals(expanded.values[1].values[0].value, "spread");
  assertEquals(expanded.values[1].values[1].value, "arr");
  assertEquals(expanded.values[2].value, "val");
});
