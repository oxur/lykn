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
  assertEquals(result.includes("return x + 1"), true);
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
  assertEquals(result.includes("return x + 1"), true);
});

Deno.test("fn: string type check", () => {
  const result = lykn("(fn (:string s) s)");
  assertEquals(result.includes("typeof s !== \"string\""), true);
  assertEquals(result.includes("return s"), true);
});

Deno.test("fn: array type check", () => {
  const result = lykn("(fn (:array a) a)");
  assertEquals(result.includes("Array.isArray(a)"), true);
  assertEquals(result.includes("return a"), true);
});

Deno.test("fn: object type check", () => {
  const result = lykn("(fn (:object o) o)");
  assertEquals(result.includes("typeof o !== \"object\""), true);
  assertEquals(result.includes("o === null"), true);
  assertEquals(result.includes("return o"), true);
});

Deno.test("lambda: alias for fn", () => {
  const result = lykn("(lambda (:number x) (+ x 1))");
  assertEquals(result.includes("typeof x !== \"number\""), true);
  assertEquals(result.includes("return x + 1"), true);
});

Deno.test("fn: multiple typed params", () => {
  const result = lykn("(fn (:number x :string y) (+ x y))");
  assertEquals(result.includes("typeof x !== \"number\""), true);
  assertEquals(result.includes("typeof y !== \"string\""), true);
  assertEquals(result.includes("return x + y"), true);
});

Deno.test("fn: typed fn returns last expression", () => {
  const result = lykn("(bind f (fn (:number x) (* x 2)))");
  // Must contain "return x * 2" not bare "x * 2;"
  assertEquals(result.includes("return x * 2"), true);
});

Deno.test("fn: multi-expression typed fn returns last", () => {
  const result = lykn("(bind f (fn (:number x) (bind y (+ x 1)) (* y 2)))");
  assertEquals(result.includes("return y * 2"), true);
  assertEquals(result.includes("const y = x + 1"), true);
});

Deno.test("fn: :any fn still uses concise arrow", () => {
  assertEquals(lykn("(bind f (fn (:any x) x))"), "const f = x => x;");
});
