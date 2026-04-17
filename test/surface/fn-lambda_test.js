import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("fn: typed params with type checks", () => {
  const result = compile("(fn (:number x) (+ x 1))");
  assertStringIncludes(result, "typeof x !== \"number\"");
  assertStringIncludes(result, "Number.isNaN(x)");
  assertStringIncludes(result, "return x + 1");
});
Deno.test("fn: :any params skip type check", () => {
  const r__gensym0 = compile("(fn (:any x) x)");
  assertEquals(r__gensym0.trim(), "(x => x);");
});
Deno.test("fn: zero params", () => {
  const r__gensym1 = compile("(fn () (Date:now))");
  assertEquals(r__gensym1.trim(), "(() => Date.now());");
});
Deno.test("fn: multi-expression body", () => {
  const result = compile("(fn (:number x) (console:log x) (+ x 1))");
  assertStringIncludes(result, "console.log(x)");
  assertStringIncludes(result, "return x + 1");
});
Deno.test("fn: string type check", () => {
  const result = compile("(fn (:string s) s)");
  assertStringIncludes(result, "typeof s !== \"string\"");
  assertStringIncludes(result, "return s");
});
Deno.test("fn: array type check", () => {
  const result = compile("(fn (:array a) a)");
  assertStringIncludes(result, "Array.isArray(a)");
  assertStringIncludes(result, "return a");
});
Deno.test("fn: object type check", () => {
  const result = compile("(fn (:object o) o)");
  assertStringIncludes(result, "typeof o !== \"object\"");
  assertStringIncludes(result, "o === null");
  assertStringIncludes(result, "return o");
});
Deno.test("lambda: alias for fn", () => {
  const result = compile("(lambda (:number x) (+ x 1))");
  assertStringIncludes(result, "typeof x !== \"number\"");
  assertStringIncludes(result, "return x + 1");
});
Deno.test("fn: multiple typed params", () => {
  const result = compile("(fn (:number x :string y) (+ x y))");
  assertStringIncludes(result, "typeof x !== \"number\"");
  assertStringIncludes(result, "typeof y !== \"string\"");
  assertStringIncludes(result, "return x + y");
});
Deno.test("fn: typed fn returns last expression", () => {
  const result = compile("(bind f (fn (:number x) (* x 2)))");
  assertStringIncludes(result, "return x * 2");
});
Deno.test("fn: multi-expression typed fn returns last", () => {
  const result = compile("(bind f (fn (:number x) (bind y (+ x 1)) (* y 2)))");
  assertStringIncludes(result, "return y * 2");
  assertStringIncludes(result, "const y = x + 1");
});
Deno.test("fn: :any fn still uses concise arrow", () => {
  const r__gensym2 = compile("(bind f (fn (:any x) x))");
  assertEquals(r__gensym2.trim(), "const f = x => x;");
});
