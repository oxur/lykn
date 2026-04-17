import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
import {expand, resetGensym, resetMacros} from "../../packages/lang/expander.js";
import {read} from "../../packages/lang/reader.js";
Deno.test("bind: simple binding", () => {
  const r__gensym0 = compile("(bind x 42)");
  assertEquals(r__gensym0.trim(), "const x = 42;");
});
Deno.test("bind: string value", () => {
  const r__gensym1 = compile("(bind name \"Duncan\")");
  assertEquals(r__gensym1.trim(), "const name = \"Duncan\";");
});
Deno.test("bind: expression value", () => {
  const r__gensym2 = compile("(bind result (+ 1 2))");
  assertEquals(r__gensym2.trim(), "const result = 1 + 2;");
});
Deno.test("bind: with type annotation", () => {
  const r__gensym3 = compile("(bind :number age 42)");
  assertEquals(r__gensym3.trim(), "const age = 42;");
});
Deno.test("bind: with type annotation and expression — emits runtime check", () => {
  const result = compile("(bind :string name (get-name user))");
  assertStringIncludes(result, "const name = getName(user);");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "TypeError");
});
Deno.test("bind: type annotation on literal — no runtime check", () => {
  const r__gensym4 = compile("(bind :string name \"hello\")");
  assertEquals(r__gensym4.trim(), "const name = \"hello\";");
});
Deno.test("bind: :any annotation — no runtime check", () => {
  const r__gensym5 = compile("(bind :any x (compute))");
  assertEquals(r__gensym5.trim(), "const x = compute();");
});
Deno.test("bind: type mismatch on literal — compile error", () => assertThrows(() => compile("(bind :number x \"hello\")"), Error));
Deno.test("bind: kebab-case name", () => {
  const r__gensym6 = compile("(bind my-value 10)");
  assertEquals(r__gensym6.trim(), "const myValue = 10;");
});
Deno.test("bind: expansion produces const form", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(bind x 42)"));
  const form = result[0];
  const head = form.values[0];
  const nameNode = form.values[1];
  const valNode = form.values[2];
  assertEquals(head.value, "const");
  assertEquals(nameNode.value, "x");
  assertEquals(valNode.value, 42);
});
Deno.test("bind: typed expansion drops type keyword", () => {
  resetMacros();
  resetGensym();
  const result = expand(read("(bind :number x 42)"));
  const form = result[0];
  const head = form.values[0];
  const nameNode = form.values[1];
  const valNode = form.values[2];
  assertEquals(head.value, "const");
  assertEquals(nameNode.value, "x");
  assertEquals(valNode.value, 42);
});
