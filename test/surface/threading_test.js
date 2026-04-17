import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
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
