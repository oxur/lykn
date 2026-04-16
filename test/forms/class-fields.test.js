import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("class field: with value", () => {
  const result = lykn('(class C () (field name "default"))');
  assertEquals(result.includes('name = "default"'), true);
});

Deno.test("class field: without value", () => {
  const result = lykn('(class C () (field items))');
  assertEquals(result.includes('items'), true);
});

Deno.test("class field: private", () => {
  const result = lykn('(class C () (field -count 0))');
  assertEquals(result.includes('#_count'), true);
});

Deno.test("class field: private access via this", () => {
  const result = lykn('(class C () (constructor () (= this:-count 0)))');
  assertEquals(result.includes('this.#_count'), true);
});

Deno.test("class method: private method", () => {
  const result = lykn('(class C () (-helper () (return 42)))');
  assertEquals(result.includes('#_helper'), true);
});

Deno.test("class field: private method reference via this", () => {
  const result = lykn('(class C () (run () (this:-helper)))');
  assertEquals(result.includes('this.#_helper'), true);
});
