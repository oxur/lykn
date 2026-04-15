import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- function* ---

Deno.test("function*: basic generator", () => {
  const result = lykn("(function* gen () (yield 1) (yield 2) (yield 3))");
  assertEquals(result.includes("function* gen()"), true);
  assertEquals(result.includes("yield 1"), true);
  assertEquals(result.includes("yield 2"), true);
  assertEquals(result.includes("yield 3"), true);
});

Deno.test("function*: with params", () => {
  const result = lykn("(function* range (start end) (for (let i start) (< i end) (+= i 1) (yield i)))");
  assertEquals(result.includes("function* range(start, end)"), true);
  assertEquals(result.includes("yield i"), true);
});

// --- yield ---

Deno.test("yield: with argument", () => {
  const result = lykn("(function* f () (yield 42))");
  assertEquals(result.includes("yield 42"), true);
});

Deno.test("yield: no argument", () => {
  const result = lykn("(function* f () (yield))");
  assertEquals(result.includes("yield;"), true);
});

// --- yield* ---

Deno.test("yield*: delegation", () => {
  const result = lykn("(function* f () (yield* other))");
  assertEquals(result.includes("yield* other"), true);
});

Deno.test("yield*: delegation to array", () => {
  const result = lykn("(function* f () (yield* #a(1 2 3)))");
  assertEquals(result.includes("yield* [1, 2, 3]"), true);
});

// --- for-await-of ---

Deno.test("for-await-of: basic", () => {
  const result = lykn("(async (function process () (for-await-of item stream (console:log item))))");
  assertEquals(result.includes("for await (const item of stream)"), true);
  assertEquals(result.includes("console.log(item)"), true);
});

// --- async generator ---

Deno.test("async function*: basic", () => {
  const result = lykn("(async (function* fetch-pages (url) (yield 1) (yield 2)))");
  assertEquals(result.includes("async function* fetchPages(url)"), true);
  assertEquals(result.includes("yield 1"), true);
});
