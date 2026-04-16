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

// --- cell ---

Deno.test("cell: create cell with number", () => {
  assertEquals(lykn("(cell 0)"), "({\n  value: 0\n});");
});

Deno.test("cell: create cell with string", () => {
  assertEquals(lykn('(cell "hello")'), '({\n  value: "hello"\n});');
});

Deno.test("cell: expansion produces object form", () => {
  const result = ex("(cell 0)");
  const expanded = result[0];
  assertEquals(expanded.values[0].value, "object");
  assertEquals(expanded.values[1].values[0].value, "value");
  assertEquals(expanded.values[1].values[1].value, 0);
});

// --- express ---

Deno.test("express: read cell value", () => {
  assertEquals(lykn("(express counter)"), "counter.value;");
});

Deno.test("express: expansion produces colon syntax", () => {
  const result = ex("(express c)");
  assertEquals(result[0].type, "atom");
  assertEquals(result[0].value, "c:value");
});

// --- swap! ---

Deno.test("swap!: simple function", () => {
  assertEquals(lykn("(swap! counter inc)"), "counter.value = inc(counter.value);");
});

Deno.test("swap!: with extra args", () => {
  assertEquals(lykn("(swap! counter add 5)"), "counter.value = add(counter.value, 5);");
});

Deno.test("swap!: with multiple extra args", () => {
  assertEquals(lykn("(swap! counter f a b)"), "counter.value = f(counter.value, a, b);");
});

Deno.test("swap!: with lambda", () => {
  const result = lykn("(swap! counter (=> (n) (+ n 1)))");
  assertEquals(result, "counter.value = (n => n + 1)(counter.value);");
});

Deno.test("swap!: expansion produces assignment", () => {
  const result = ex("(swap! c f)");
  const expanded = result[0];
  assertEquals(expanded.values[0].value, "=");
  assertEquals(expanded.values[1].value, "c:value");
  assertEquals(expanded.values[2].values[0].value, "f");
  assertEquals(expanded.values[2].values[1].value, "c:value");
});

// --- reset! ---

Deno.test("reset!: simple value", () => {
  assertEquals(lykn("(reset! counter 0)"), "counter.value = 0;");
});

Deno.test("reset!: expression value", () => {
  assertEquals(lykn("(reset! counter (+ a b))"), "counter.value = a + b;");
});

Deno.test("reset!: expansion produces assignment", () => {
  const result = ex("(reset! c 42)");
  const expanded = result[0];
  assertEquals(expanded.values[0].value, "=");
  assertEquals(expanded.values[1].value, "c:value");
  assertEquals(expanded.values[2].value, 42);
});
