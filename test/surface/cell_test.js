import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("cell: create cell with number", () => {
  const r__gensym0 = compile("(cell 0)");
  assertEquals(r__gensym0.trim(), "({\n  value: 0\n});");
});
Deno.test("cell: create cell with string", () => {
  const r__gensym1 = compile("(cell \"hello\")");
  assertEquals(r__gensym1.trim(), "({\n  value: \"hello\"\n});");
});
Deno.test("express: read cell value", () => {
  const r__gensym2 = compile("(express counter)");
  assertEquals(r__gensym2.trim(), "counter.value;");
});
Deno.test("swap!: simple function", () => {
  const r__gensym3 = compile("(swap! counter inc)");
  assertEquals(r__gensym3.trim(), "counter.value = inc(counter.value);");
});
Deno.test("swap!: with extra args", () => {
  const r__gensym4 = compile("(swap! counter add 5)");
  assertEquals(r__gensym4.trim(), "counter.value = add(counter.value, 5);");
});
Deno.test("swap!: with multiple extra args", () => {
  const r__gensym5 = compile("(swap! counter f a b)");
  assertEquals(r__gensym5.trim(), "counter.value = f(counter.value, a, b);");
});
Deno.test("swap!: with lambda", () => {
  const r__gensym6 = compile("(swap! counter (=> (n) (+ n 1)))");
  assertEquals(r__gensym6.trim(), "counter.value = (n => n + 1)(counter.value);");
});
Deno.test("reset!: simple value", () => {
  const r__gensym7 = compile("(reset! counter 0)");
  assertEquals(r__gensym7.trim(), "counter.value = 0;");
});
Deno.test("reset!: expression value", () => {
  const r__gensym8 = compile("(reset! counter (+ a b))");
  assertEquals(r__gensym8.trim(), "counter.value = a + b;");
});
