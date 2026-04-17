import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("integration: bind + obj", () => {
  const r__gensym0 = compile("(bind user (obj :name \"Duncan\" :age 42))");
  assertEquals(r__gensym0.trim(), "const user = {\n  name: \"Duncan\",\n  age: 42\n};");
});
Deno.test("integration: bind + obj + assoc", () => {
  const result = compile("(bind user (obj :name \"Duncan\" :age 42))\n    (bind updated (assoc user :age 43))");
  assertStringIncludes(result, "name: \"Duncan\"");
  assertStringIncludes(result, "...user");
  assertStringIncludes(result, "age: 43");
});
Deno.test("integration: bind + cell + swap! + express", () => {
  const result = compile("(bind counter (cell 0))\n    (swap! counter inc)\n    (console:log (express counter))");
  assertStringIncludes(result, "value: 0");
  assertStringIncludes(result, "counter.value = inc(counter.value);");
  assertStringIncludes(result, "console.log(counter.value);");
});
Deno.test("integration: bind + conj", () => {
  const result = compile("(bind items (array 1 2 3))\n    (bind more (conj items 4))");
  assertStringIncludes(result, "const items = [1, 2, 3];");
  assertStringIncludes(result, "const more = [...items, 4];");
});
Deno.test("integration: thread-first with surface forms", () => {
  const r__gensym1 = compile("(-> x f g h)");
  assertEquals(r__gensym1.trim(), "h(g(f(x)));");
});
Deno.test("integration: bind + cell + reset!", () => {
  const result = compile("(bind state (cell (obj :count 0)))\n    (reset! state (obj :count 1))");
  assertStringIncludes(result, "const state");
  assertStringIncludes(result, "state.value =");
});
