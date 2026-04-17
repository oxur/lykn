import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
import {expand, resetGensym, resetMacros} from "../../packages/lang/expander.js";
import {read} from "../../packages/lang/reader.js";
Deno.test("assoc: single key-value", () => {
  const r__gensym0 = compile("(assoc obj :age 43)");
  assertEquals(r__gensym0.trim(), "({\n  ...obj,\n  age: 43\n});");
});
Deno.test("assoc: multiple key-values", () => {
  const r__gensym1 = compile("(assoc obj :a 1 :b 2)");
  assertEquals(r__gensym1.trim(), "({\n  ...obj,\n  a: 1,\n  b: 2\n});");
});
Deno.test("assoc: kebab-case key", () => {
  const r__gensym2 = compile("(assoc obj :first-name \"Duncan\")");
  assertEquals(r__gensym2.trim(), "({\n  ...obj,\n  firstName: \"Duncan\"\n});");
});
Deno.test("assoc: expression value", () => {
  const r__gensym3 = compile("(assoc obj :score (+ base bonus))");
  assertEquals(r__gensym3.trim(), "({\n  ...obj,\n  score: base + bonus\n});");
});
Deno.test("dissoc: single key", () => {
  const result = compile("(dissoc obj :key)");
  assertStringIncludes(result, "...");
  assertStringIncludes(result, "obj");
});
Deno.test("dissoc: multiple keys", () => {
  const result = compile("(dissoc obj :a :b)");
  assertStringIncludes(result, "...");
  assertStringIncludes(result, "obj");
});
Deno.test("conj: append to array", () => {
  const r__gensym4 = compile("(conj arr 42)");
  assertEquals(r__gensym4.trim(), "[...arr, 42];");
});
Deno.test("conj: append expression", () => {
  const r__gensym5 = compile("(conj items (+ 1 2))");
  assertEquals(r__gensym5.trim(), "[...items, 1 + 2];");
});
Deno.test("assoc: expansion produces object with spread", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(assoc obj :age 43)"));
  const expanded = result[0];
  const head = expanded.values[0];
  const spreadPair = expanded.values[1];
  const spreadOp = spreadPair.values[0];
  const spreadArg = spreadPair.values[1];
  const kvPair = expanded.values[2];
  const kvKey = kvPair.values[0];
  const kvVal = kvPair.values[1];
  assertEquals(head.value, "object");
  assertEquals(spreadOp.value, "spread");
  assertEquals(spreadArg.value, "obj");
  assertEquals(kvKey.value, "age");
  assertEquals(kvVal.value, 43);
});
Deno.test("dissoc: expansion produces IIFE", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(dissoc obj :key)"));
  const expanded = result[0];
  assertEquals(expanded.type, "list");
  const arrow = expanded.values[0];
  const arrowHead = arrow.values[0];
  assertEquals(arrowHead.value, "=>");
});
Deno.test("conj: expansion produces array with spread", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(conj arr val)"));
  const expanded = result[0];
  const head = expanded.values[0];
  const spreadPair = expanded.values[1];
  const spreadOp = spreadPair.values[0];
  const spreadArg = spreadPair.values[1];
  const valNode = expanded.values[2];
  assertEquals(head.value, "array");
  assertEquals(spreadOp.value, "spread");
  assertEquals(spreadArg.value, "arr");
  assertEquals(valNode.value, "val");
});
