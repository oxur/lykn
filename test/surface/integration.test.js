import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("integration: bind + obj", () => {
  const result = lykn('(bind user (obj :name "Duncan" :age 42))');
  assertEquals(result, 'const user = {\n  name: "Duncan",\n  age: 42\n};');
});

Deno.test("integration: bind + obj + assoc", () => {
  const result = lykn(`
    (bind user (obj :name "Duncan" :age 42))
    (bind updated (assoc user :age 43))
  `);
  assertEquals(result.includes('name: "Duncan"'), true);
  assertEquals(result.includes("...user"), true);
  assertEquals(result.includes("age: 43"), true);
});

Deno.test("integration: bind + cell + swap! + express", () => {
  const result = lykn(`
    (bind counter (cell 0))
    (swap! counter inc)
    (console:log (express counter))
  `);
  assertEquals(result.includes("value: 0"), true);
  assertEquals(result.includes("counter.value = inc(counter.value);"), true);
  assertEquals(result.includes("console.log(counter.value);"), true);
});

Deno.test("integration: bind + conj", () => {
  const result = lykn(`
    (bind items (array 1 2 3))
    (bind more (conj items 4))
  `);
  assertEquals(result.includes("const items = [1, 2, 3];"), true);
  assertEquals(result.includes("const more = [...items, 4];"), true);
});

Deno.test("integration: thread-first with surface forms", () => {
  assertEquals(lykn("(-> x f g h)"), "h(g(f(x)));");
});

Deno.test("integration: bind + cell + reset!", () => {
  const result = lykn(`
    (bind state (cell (obj :count 0)))
    (reset! state (obj :count 1))
  `);
  assertEquals(result.includes("const state"), true);
  assertEquals(result.includes("state.value ="), true);
});
