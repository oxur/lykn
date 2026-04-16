import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- if-let: simple binding ---

Deno.test("if-let: simple binding with else", () => {
  const result = lykn('(if-let (user (find-user id)) (greet user) (console:log "not found"))');
  assertEquals(result.includes("!= null"), true);
  assertEquals(result.includes("const user"), true);
  assertEquals(result.includes("greet(user)"), true);
  assertEquals(result.includes("not found"), true);
});

Deno.test("if-let: simple binding without else", () => {
  const result = lykn("(if-let (user (find-user id)) (greet user))");
  assertEquals(result.includes("!= null"), true);
  assertEquals(result.includes("greet(user)"), true);
});

// --- if-let: ADT pattern ---

Deno.test("if-let: ADT constructor pattern", () => {
  const result = lykn(`
    (type Option (Some :any value) None)
    (if-let ((Some v) (find-user id)) (greet v) (console:log "none"))`);
  assertEquals(result.includes('.tag === "Some"'), true);
  assertEquals(result.includes(".value"), true);
  assertEquals(result.includes("greet(v)"), true);
});

// --- if-let: structural obj pattern ---

Deno.test("if-let: structural obj pattern", () => {
  const result = lykn('(if-let ((obj :name n :age a) data) (console:log n a) (console:log "bad"))');
  assertEquals(result.includes('"name" in'), true);
  assertEquals(result.includes('"age" in'), true);
  assertEquals(result.includes("const n"), true);
  assertEquals(result.includes("const a"), true);
});

// --- if-let: IIFE ---

Deno.test("if-let: always produces IIFE", () => {
  const result = lykn("(if-let (x (get-val)) x 0)");
  assertEquals(result.includes("(() =>"), true);
  assertEquals(result.includes("return"), true);
});

// --- when-let ---

Deno.test("when-let: simple binding", () => {
  const result = lykn("(when-let (user (find-user id)) (greet user))");
  assertEquals(result.includes("!= null"), true);
  assertEquals(result.includes("greet(user)"), true);
  // No else branch
  assertEquals(result.includes("else"), false);
});

Deno.test("when-let: ADT pattern", () => {
  const result = lykn(`
    (type Option (Some :any value) None)
    (when-let ((Some v) (find-user id)) (greet v))`);
  assertEquals(result.includes('.tag === "Some"'), true);
  assertEquals(result.includes("greet(v)"), true);
});

Deno.test("when-let: multiple body expressions", () => {
  const result = lykn("(when-let (user (find-user id)) (console:log user) (greet user))");
  assertEquals(result.includes("console.log(user)"), true);
  assertEquals(result.includes("greet(user)"), true);
});

Deno.test("when-let: produces IIFE", () => {
  const result = lykn("(when-let (x (get-val)) x)");
  assertEquals(result.includes("(() =>"), true);
});
