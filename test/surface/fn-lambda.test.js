import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("fn: typed params with type checks", () => {
  const result = lykn("(fn (:number x) (+ x 1))");
  assertEquals(result.includes("typeof x !== \"number\""), true);
  assertEquals(result.includes("Number.isNaN(x)"), true);
  assertEquals(result.includes("x + 1"), true);
});

Deno.test("fn: :any params skip type check", () => {
  const result = lykn("(fn (:any x) x)");
  assertEquals(result.includes("typeof"), false);
  assertEquals(result.includes("=> x"), true);
});

Deno.test("fn: zero params", () => {
  const result = lykn("(fn () (Date:now))");
  assertEquals(result, "(() => Date.now());");
});

Deno.test("fn: multi-expression body", () => {
  const result = lykn("(fn (:number x) (console:log x) (+ x 1))");
  assertEquals(result.includes("console.log(x)"), true);
  assertEquals(result.includes("x + 1"), true);
});

Deno.test("fn: string type check", () => {
  const result = lykn("(fn (:string s) s)");
  assertEquals(result.includes("typeof s !== \"string\""), true);
});

Deno.test("fn: array type check", () => {
  const result = lykn("(fn (:array a) a)");
  assertEquals(result.includes("Array.isArray(a)"), true);
});

Deno.test("fn: object type check", () => {
  const result = lykn("(fn (:object o) o)");
  assertEquals(result.includes("typeof o !== \"object\""), true);
  assertEquals(result.includes("o === null"), true);
});

Deno.test("lambda: alias for fn", () => {
  const result = lykn("(lambda (:number x) (+ x 1))");
  assertEquals(result.includes("typeof x !== \"number\""), true);
  assertEquals(result.includes("x + 1"), true);
});

Deno.test("fn: multiple typed params", () => {
  const result = lykn("(fn (:number x :string y) (+ x y))");
  assertEquals(result.includes("typeof x !== \"number\""), true);
  assertEquals(result.includes("typeof y !== \"string\""), true);
});
