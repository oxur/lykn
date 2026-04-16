import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("class async: async method", () => {
  const result = lykn('(class C () (async (fetch-data () (return (await (get-it))))))');
  assertEquals(result.includes('async'), true);
  assertEquals(result.includes('fetchData'), true);
});

Deno.test("class async: static async", () => {
  const result = lykn('(class C () (static (async (load () (return 1)))))');
  assertEquals(result.includes('static'), true);
  assertEquals(result.includes('async'), true);
});

Deno.test("class async: async private method", () => {
  const result = lykn('(class C () (async (-do-work () (return 1))))');
  assertEquals(result.includes('async'), true);
  assertEquals(result.includes('#_doWork'), true);
});
