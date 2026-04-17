import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
import {expand, resetGensym, resetMacros} from "../../packages/lang/expander.js";
import {read} from "../../packages/lang/reader.js";
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
Deno.test("cell: expansion produces object form", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(cell 0)"));
  const expanded = result[0];
  const head = expanded.values[0];
  const pair = expanded.values[1];
  const pairKey = pair.values[0];
  const pairVal = pair.values[1];
  assertEquals(head.value, "object");
  assertEquals(pairKey.value, "value");
  assertEquals(pairVal.value, 0);
});
Deno.test("express: expansion produces colon syntax", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(express c)"));
  const node = result[0];
  assertEquals(node.type, "atom");
  assertEquals(node.value, "c:value");
});
Deno.test("swap!: expansion produces assignment", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(swap! c f)"));
  const expanded = result[0];
  const op = expanded.values[0];
  const target = expanded.values[1];
  const call = expanded.values[2];
  const callFn = call.values[0];
  const callArg = call.values[1];
  assertEquals(op.value, "=");
  assertEquals(target.value, "c:value");
  assertEquals(callFn.value, "f");
  assertEquals(callArg.value, "c:value");
});
Deno.test("reset!: expansion produces assignment", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(reset! c 42)"));
  const expanded = result[0];
  const op = expanded.values[0];
  const target = expanded.values[1];
  const val = expanded.values[2];
  assertEquals(op.value, "=");
  assertEquals(target.value, "c:value");
  assertEquals(val.value, 42);
});
