import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("genfunc: basic with :yields :number", () => {
  const result = compile("(genfunc range :args (:number start :number end) :yields :number :body (for (let i start) (< i end) (+= i 1) (yield i)))");
  assertStringIncludes(result, "function* range(start, end)");
  assertStringIncludes(result, "typeof start !== \"number\"");
  assertStringIncludes(result, "yield (() =>");
});
Deno.test("genfunc: :yields :any — no yield check", () => {
  const result = compile("(genfunc gen :yields :any :body (yield 1) (yield 2))");
  assertStringIncludes(result, "function* gen()");
  assertStringIncludes(result, "yield 1");
  assertStringIncludes(result, "yield 2");
});
Deno.test("genfunc: no :yields — no yield check", () => {
  const result = compile("(genfunc gen :body (yield 1))");
  assertStringIncludes(result, "function* gen()");
  assertStringIncludes(result, "yield 1");
});
Deno.test("genfunc: yield* not instrumented", () => {
  const result = compile("(genfunc gen :yields :number :body (yield* other))");
  assertStringIncludes(result, "yield* other");
});
Deno.test("genfunc: with pre-condition", () => {
  const result = compile("(genfunc range :args (:number n) :yields :number :pre (> n 0) :body (for (let i 0) (< i n) (+= i 1) (yield i)))");
  assertStringIncludes(result, "pre-condition failed");
  assertStringIncludes(result, "function* range(n)");
});
Deno.test("genfn: anonymous with :yields", () => {
  const result = compile("(bind gen (genfn (:number start :number end) :yields :number (for (let i start) (< i end) (+= i 1) (yield i))))");
  assertStringIncludes(result, "function*");
  assertStringIncludes(result, "typeof start !== \"number\"");
  assertStringIncludes(result, "yield (() =>");
});
Deno.test("genfn: no :yields", () => {
  const result = compile("(bind gen (genfn () (yield 1) (yield 2)))");
  assertStringIncludes(result, "function*");
  assertStringIncludes(result, "yield 1");
});
Deno.test("genfn: :yields :any — no check", () => {
  const result = compile("(bind gen (genfn () :yields :any (yield 1)))");
  assertStringIncludes(result, "function*");
  assertStringIncludes(result, "yield 1");
});
Deno.test("export genfunc", () => {
  const result = compile("(export (genfunc gen :yields :number :body (yield 42)))");
  assertStringIncludes(result, "export function* gen()");
});
Deno.test("async genfunc", () => {
  const result = compile("(async (genfunc fetch-pages :body (yield 1)))");
  assertStringIncludes(result, "async function* fetchPages()");
});
Deno.test("genfunc: error on missing :body", () => assertThrows(() => compile("(genfunc gen :yields :number)"), Error, ":body is required"));
