import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
import {expand, resetGensym, resetMacros} from "../../packages/lang/expander.js";
import {read} from "../../packages/lang/reader.js";
Deno.test("obj: simple object", () => {
  const r__gensym0 = compile("(obj :name \"Duncan\" :age 42)");
  assertEquals(r__gensym0.trim(), "({\n  name: \"Duncan\",\n  age: 42\n});");
});
Deno.test("obj: single property", () => {
  const r__gensym1 = compile("(obj :active true)");
  assertEquals(r__gensym1.trim(), "({\n  active: true\n});");
});
Deno.test("obj: kebab-case key", () => {
  const r__gensym2 = compile("(obj :first-name \"Duncan\")");
  assertEquals(r__gensym2.trim(), "({\n  firstName: \"Duncan\"\n});");
});
Deno.test("obj: computed value", () => {
  const r__gensym3 = compile("(obj :score (* base multiplier))");
  assertEquals(r__gensym3.trim(), "({\n  score: base * multiplier\n});");
});
Deno.test("obj: empty object", () => {
  const r__gensym4 = compile("(obj)");
  assertEquals(r__gensym4.trim(), "({});");
});
Deno.test("obj: variable value", () => {
  const r__gensym5 = compile("(obj :name user-name)");
  assertEquals(r__gensym5.trim(), "({\n  name: userName\n});");
});
Deno.test("obj: expansion produces object form", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(obj :name \"Duncan\")"));
  const expanded = result[0];
  const head = expanded.values[0];
  const pair = expanded.values[1];
  const pairKey = pair.values[0];
  const pairVal = pair.values[1];
  assertEquals(head.value, "object");
  assertEquals(pair.type, "list");
  assertEquals(pairKey.value, "name");
  assertEquals(pairVal.value, "Duncan");
});
