import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

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

// --- -> (thread-first) ---

Deno.test("->: bare symbols", () => {
  assertEquals(lykn("(-> x f g)"), "g(f(x));");
});

Deno.test("->: list forms", () => {
  assertEquals(lykn("(-> x (f a) (g b))"), "g(f(x, a), b);");
});

Deno.test("->: mixed bare and list", () => {
  assertEquals(lykn("(-> x f (g a))"), "g(f(x), a);");
});

Deno.test("->: single step", () => {
  assertEquals(lykn("(-> x f)"), "f(x);");
});

Deno.test("->: three steps", () => {
  assertEquals(lykn("(-> x f g h)"), "h(g(f(x)));");
});

Deno.test("->: expansion nests correctly", () => {
  const result = ex("(-> x f g)");
  const expanded = result[0];
  // Should be (g (f x))
  assertEquals(expanded.values[0].value, "g");
  assertEquals(expanded.values[1].values[0].value, "f");
  assertEquals(expanded.values[1].values[1].value, "x");
});

// --- ->> (thread-last) ---

Deno.test("->>: bare symbols", () => {
  assertEquals(lykn("(->> x f g)"), "g(f(x));");
});

Deno.test("->>: list forms", () => {
  assertEquals(lykn("(->> x (f a) (g b))"), "g(b, f(a, x));");
});

Deno.test("->>: mixed bare and list", () => {
  assertEquals(lykn("(->> x f (g a))"), "g(a, f(x));");
});

Deno.test("->>: single step", () => {
  assertEquals(lykn("(->> x f)"), "f(x);");
});

Deno.test("->>: three steps with args", () => {
  assertEquals(lykn("(->> items (filter pred) (map f) (reduce g init))"), "reduce(g, init, map(f, filter(pred, items)));");
});

Deno.test("->>: expansion nests correctly", () => {
  const result = ex("(->> x (f a))");
  const expanded = result[0];
  // Should be (f a x)
  assertEquals(expanded.values[0].value, "f");
  assertEquals(expanded.values[1].value, "a");
  assertEquals(expanded.values[2].value, "x");
});
