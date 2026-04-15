import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- genfunc ---

Deno.test("genfunc: basic with :yields :number", () => {
  const result = lykn(
    "(genfunc range :args (:number start :number end) :yields :number :body (for (let i start) (< i end) (+= i 1) (yield i)))",
  );
  assertEquals(result.includes("function* range(start, end)"), true);
  assertEquals(result.includes('typeof start !== "number"'), true);
  // Yield check IIFE
  assertEquals(result.includes("yield (() =>"), true);
});

Deno.test("genfunc: :yields :any — no yield check", () => {
  const result = lykn(
    "(genfunc gen :yields :any :body (yield 1) (yield 2))",
  );
  assertEquals(result.includes("function* gen()"), true);
  assertEquals(result.includes("yield 1"), true);
  assertEquals(result.includes("yield 2"), true);
  // No IIFE wrapper
  assertEquals(result.includes("(() =>"), false);
});

Deno.test("genfunc: no :yields — no yield check", () => {
  const result = lykn(
    "(genfunc gen :body (yield 1))",
  );
  assertEquals(result.includes("function* gen()"), true);
  assertEquals(result.includes("yield 1"), true);
  assertEquals(result.includes("(() =>"), false);
});

Deno.test("genfunc: yield* not instrumented", () => {
  const result = lykn(
    "(genfunc gen :yields :number :body (yield* other))",
  );
  assertEquals(result.includes("yield* other"), true);
  // yield* should NOT be wrapped in IIFE
  assertEquals(result.includes("(() =>"), false);
});

Deno.test("genfunc: with pre-condition", () => {
  const result = lykn(
    "(genfunc range :args (:number n) :yields :number :pre (> n 0) :body (for (let i 0) (< i n) (+= i 1) (yield i)))",
  );
  assertEquals(result.includes("pre-condition failed"), true);
  assertEquals(result.includes("function* range(n)"), true);
});

// --- genfn ---

Deno.test("genfn: anonymous with :yields", () => {
  const result = lykn(
    "(bind gen (genfn (:number start :number end) :yields :number (for (let i start) (< i end) (+= i 1) (yield i))))",
  );
  assertEquals(result.includes("function*"), true);
  assertEquals(result.includes('typeof start !== "number"'), true);
  assertEquals(result.includes("yield (() =>"), true);
});

Deno.test("genfn: no :yields", () => {
  const result = lykn(
    "(bind gen (genfn () (yield 1) (yield 2)))",
  );
  assertEquals(result.includes("function*"), true);
  assertEquals(result.includes("yield 1"), true);
  assertEquals(result.includes("(() =>"), false);
});

Deno.test("genfn: :yields :any — no check", () => {
  const result = lykn(
    "(bind gen (genfn () :yields :any (yield 1)))",
  );
  assertEquals(result.includes("function*"), true);
  assertEquals(result.includes("yield 1"), true);
  assertEquals(result.includes("(() =>"), false);
});

// --- composition ---

Deno.test("export genfunc", () => {
  const result = lykn(
    "(export (genfunc gen :yields :number :body (yield 42)))",
  );
  assertEquals(result.includes("export function* gen()"), true);
});

Deno.test("async genfunc", () => {
  const result = lykn(
    "(async (genfunc fetch-pages :body (yield 1)))",
  );
  assertEquals(result.includes("async function* fetchPages()"), true);
});

Deno.test("genfunc: error on missing :body", () => {
  assertThrows(
    () => lykn("(genfunc gen :yields :number)"),
    Error,
    ":body is required",
  );
});
