import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- Literal patterns ---

Deno.test("match: number literals", () => {
  const result = lykn('(match status (200 "ok") (404 "not found") (_ "unknown"))');
  assertEquals(result.includes("=== 200"), true);
  assertEquals(result.includes("=== 404"), true);
  assertEquals(result.includes('"ok"'), true);
  assertEquals(result.includes('"not found"'), true);
  assertEquals(result.includes('"unknown"'), true);
});

Deno.test("match: string literals", () => {
  const result = lykn('(match cmd ("start" 1) ("stop" 0) (_ -1))');
  assertEquals(result.includes('=== "start"'), true);
  assertEquals(result.includes('=== "stop"'), true);
});

Deno.test("match: boolean literals", () => {
  const result = lykn('(match flag (true "yes") (false "no"))');
  assertEquals(result.includes("=== true"), true);
  assertEquals(result.includes("=== false"), true);
});

// --- Wildcard ---

Deno.test("match: wildcard default", () => {
  const result = lykn('(match x (1 "one") (_ "other"))');
  assertEquals(result.includes("=== 1"), true);
  assertEquals(result.includes('"other"'), true);
  // Should not have "no matching pattern" throw since wildcard covers all
  assertEquals(result.includes("no matching pattern"), false);
});

// --- ADT patterns ---

Deno.test("match: ADT constructor with field", () => {
  const result = lykn(`
    (type Option (Some :any value) None)
    (match opt
      ((Some v) v)
      (None 0))`);
  assertEquals(result.includes('.tag === "Some"'), true);
  assertEquals(result.includes('.tag === "None"'), true);
  assertEquals(result.includes(".value"), true);
});

Deno.test("match: zero-field ADT constructor", () => {
  const result = lykn(`
    (type Option (Some :any value) None)
    (match opt
      ((Some v) (use v))
      (None (default-val)))`);
  assertEquals(result.includes('.tag === "None"'), true);
});

// --- Structural obj patterns ---

Deno.test("match: structural obj pattern", () => {
  const result = lykn(`(match response
    ((obj :ok true :data d) (process d))
    (_ (handle-error)))`);
  assertEquals(result.includes('typeof'), true);
  assertEquals(result.includes('"ok" in'), true);
  assertEquals(result.includes('"data" in'), true);
  assertEquals(result.includes('.ok === true'), true);
});

// --- Guards ---

Deno.test("match: guarded pattern", () => {
  const result = lykn(`
    (type Option (Some :any value) None)
    (match opt
      ((Some v) :when (> v 0) (use-positive v))
      ((Some v) (use-other v))
      (None 0))`);
  assertEquals(result.includes('.tag === "Some"'), true);
  assertEquals(result.includes("> 0"), true);
});

// --- IIFE wrapper ---

Deno.test("match: always produces IIFE", () => {
  const result = lykn('(match x (1 "one") (_ "other"))');
  assertEquals(result.includes("(() =>"), true);
  assertEquals(result.includes("return"), true);
});

// --- No matching pattern throw ---

Deno.test("match: throws on no match without wildcard", () => {
  const result = lykn('(match x (1 "one") (2 "two"))');
  assertEquals(result.includes("no matching pattern"), true);
});

// --- Simple binding ---

Deno.test("match: simple symbol binding", () => {
  const result = lykn('(match x (y (+ y 1)))');
  assertEquals(result.includes("const y ="), true);
  assertEquals(result.includes("y + 1"), true);
});
