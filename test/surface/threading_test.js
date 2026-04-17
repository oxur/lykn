import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
import {expand, resetGensym, resetMacros} from "../../packages/lang/expander.js";
import {read} from "../../packages/lang/reader.js";
Deno.test("->: bare symbols", () => {
  const r__gensym0 = compile("(-> x f g)");
  assertEquals(r__gensym0.trim(), "g(f(x));");
});
Deno.test("->: list forms", () => {
  const r__gensym1 = compile("(-> x (f a) (g b))");
  assertEquals(r__gensym1.trim(), "g(f(x, a), b);");
});
Deno.test("->: mixed bare and list", () => {
  const r__gensym2 = compile("(-> x f (g a))");
  assertEquals(r__gensym2.trim(), "g(f(x), a);");
});
Deno.test("->: single step", () => {
  const r__gensym3 = compile("(-> x f)");
  assertEquals(r__gensym3.trim(), "f(x);");
});
Deno.test("->: three steps", () => {
  const r__gensym4 = compile("(-> x f g h)");
  assertEquals(r__gensym4.trim(), "h(g(f(x)));");
});
Deno.test("->>: bare symbols", () => {
  const r__gensym5 = compile("(->> x f g)");
  assertEquals(r__gensym5.trim(), "g(f(x));");
});
Deno.test("->>: list forms", () => {
  const r__gensym6 = compile("(->> x (f a) (g b))");
  assertEquals(r__gensym6.trim(), "g(b, f(a, x));");
});
Deno.test("->>: mixed bare and list", () => {
  const r__gensym7 = compile("(->> x f (g a))");
  assertEquals(r__gensym7.trim(), "g(a, f(x));");
});
Deno.test("->>: single step", () => {
  const r__gensym8 = compile("(->> x f)");
  assertEquals(r__gensym8.trim(), "f(x);");
});
Deno.test("->>: three steps with args", () => {
  const r__gensym9 = compile("(->> items (filter pred) (map f) (reduce g init))");
  assertEquals(r__gensym9.trim(), "reduce(g, init, map(f, filter(pred, items)));");
});
Deno.test("->: expansion nests correctly", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(-> x f g)"));
  const expanded = result[0];
  const outerFn = expanded.values[0];
  const inner = expanded.values[1];
  const innerFn = inner.values[0];
  const innerArg = inner.values[1];
  assertEquals(outerFn.value, "g");
  assertEquals(innerFn.value, "f");
  assertEquals(innerArg.value, "x");
});
Deno.test("->>: expansion nests correctly", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(->> x (f a))"));
  const expanded = result[0];
  const fnNode = expanded.values[0];
  const argA = expanded.values[1];
  const argX = expanded.values[2];
  assertEquals(fnNode.value, "f");
  assertEquals(argA.value, "a");
  assertEquals(argX.value, "x");
});
