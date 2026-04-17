import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
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
