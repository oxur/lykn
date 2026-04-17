import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("some->: basic nil-safe threading", () => {
  const result = compile("(some-> x f g)");
  assertStringIncludes(result, "== null");
  assertStringIncludes(result, "f(");
  assertStringIncludes(result, "g(");
  assertStringIncludes(result, "return");
});
Deno.test("some->: with list forms (thread-first)", () => {
  const result = compile("(some-> x (f a) (g b))");
  assertStringIncludes(result, "== null");
  assertStringIncludes(result, "return");
});
Deno.test("some->: produces IIFE", () => {
  const result = compile("(some-> x f)");
  assertStringIncludes(result, "(() =>");
});
Deno.test("some->: uses loose equality for null check", () => {
  const result = compile("(some-> x f g)");
  assertStringIncludes(result, "== null");
});
Deno.test("some->>: basic nil-safe threading", () => {
  const result = compile("(some->> x f g)");
  assertStringIncludes(result, "== null");
  assertStringIncludes(result, "return");
});
Deno.test("some->>: with list forms (thread-last)", () => {
  const result = compile("(some->> x (f a) (g b))");
  assertStringIncludes(result, "== null");
});
Deno.test("some->>: produces IIFE", () => {
  const result = compile("(some->> x f)");
  assertStringIncludes(result, "(() =>");
});
