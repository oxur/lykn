import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- some-> ---

Deno.test("some->: basic nil-safe threading", () => {
  const result = lykn("(some-> x f g)");
  assertEquals(result.includes("== null"), true);
  assertEquals(result.includes("f("), true);
  assertEquals(result.includes("g("), true);
  assertEquals(result.includes("return"), true);
});

Deno.test("some->: with list forms (thread-first)", () => {
  const result = lykn("(some-> x (f a) (g b))");
  // Thread-first: x is first arg
  assertEquals(result.includes("== null"), true);
  assertEquals(result.includes("return"), true);
});

Deno.test("some->: produces IIFE", () => {
  const result = lykn("(some-> x f)");
  assertEquals(result.includes("(() =>"), true);
});

Deno.test("some->: uses loose equality for null check", () => {
  const result = lykn("(some-> x f g)");
  assertEquals(result.includes("== null"), true);
  // Should NOT use ===
  assertEquals(result.includes("=== null"), false);
});

// --- some->> ---

Deno.test("some->>: basic nil-safe threading", () => {
  const result = lykn("(some->> x f g)");
  assertEquals(result.includes("== null"), true);
  assertEquals(result.includes("return"), true);
});

Deno.test("some->>: with list forms (thread-last)", () => {
  const result = lykn("(some->> x (f a) (g b))");
  assertEquals(result.includes("== null"), true);
});

Deno.test("some->>: produces IIFE", () => {
  const result = lykn("(some->> x f)");
  assertEquals(result.includes("(() =>"), true);
});
