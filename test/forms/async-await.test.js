import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("async: wraps function declaration", () => {
  const result = lykn("(async (function fetch-data () (return 1)))");
  assertEquals(result.startsWith("async function fetchData"), true);
});

Deno.test("async: wraps lambda", () => {
  const result = lykn("(const f (async (lambda () (return 1))))");
  assertEquals(result.includes("async function"), true);
});

Deno.test("async: wraps arrow", () => {
  const result = lykn("(const f (async (=> () 1)))");
  assertEquals(result.includes("async"), true);
});

Deno.test("async: rejects non-function", () => {
  assertThrows(() => lykn("(async 42)"));
});

Deno.test("await: basic", () => {
  assertEquals(lykn("(const data (await (fetch url)))"),
    "const data = await fetch(url);");
});

Deno.test("await: wrong arity throws", () => {
  assertThrows(() => lykn("(await a b)"));
});
