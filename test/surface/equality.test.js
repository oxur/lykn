import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

// --- = (strict equality) ---

Deno.test("=: binary strict equality", () => {
  assertEquals(lykn("(= a b)"), "a === b;");
});

Deno.test("=: variadic pairwise chain", () => {
  assertEquals(lykn("(= a b c)"), "a === b && b === c;");
});

Deno.test("=: four-way pairwise chain", () => {
  assertEquals(lykn("(= a b c d)"), "a === b && b === c && c === d;");
});

Deno.test("=: with literals", () => {
  assertEquals(lykn("(= x 42)"), "x === 42;");
});

Deno.test("=: with null", () => {
  assertEquals(lykn("(= x null)"), "x === null;");
});

Deno.test("=: with expressions", () => {
  assertEquals(lykn("(= (+ a 1) (+ b 2))"), "a + 1 === b + 2;");
});

Deno.test("=: error on single arg", () => {
  assertThrows(() => lykn("(= x)"), Error, "at least 2 arguments");
});

// --- != (strict inequality) ---

Deno.test("!=: binary strict inequality", () => {
  assertEquals(lykn("(!= a b)"), "a !== b;");
});

Deno.test("!=: with literals", () => {
  assertEquals(lykn('(!= x "hello")'), 'x !== "hello";');
});

Deno.test("!=: error on wrong arity", () => {
  assertThrows(() => lykn("(!= a)"), Error, "exactly 2 arguments");
});

// --- and (logical AND) ---

Deno.test("and: binary", () => {
  assertEquals(lykn("(and x y)"), "x && y;");
});

Deno.test("and: variadic", () => {
  assertEquals(lykn("(and a b c d)"), "a && b && c && d;");
});

Deno.test("and: with expressions", () => {
  assertEquals(lykn("(and (> x 0) (< x 10))"), "x > 0 && x < 10;");
});

Deno.test("and: error on single arg", () => {
  assertThrows(() => lykn("(and x)"), Error, "at least 2 arguments");
});

// --- or (logical OR) ---

Deno.test("or: binary", () => {
  assertEquals(lykn("(or x y)"), "x || y;");
});

Deno.test("or: variadic", () => {
  assertEquals(lykn("(or a b c d)"), "a || b || c || d;");
});

Deno.test("or: with expressions", () => {
  assertEquals(lykn("(or (= x 0) (= x 1))"), "x === 0 || x === 1;");
});

Deno.test("or: error on single arg", () => {
  assertThrows(() => lykn("(or x)"), Error, "at least 2 arguments");
});

// --- not (logical NOT) ---

Deno.test("not: unary", () => {
  assertEquals(lykn("(not x)"), "!x;");
});

Deno.test("not: double negation", () => {
  assertEquals(lykn("(not (not x))"), "!!x;");
});

Deno.test("not: with expression", () => {
  assertEquals(lykn("(not (= a b))"), "!(a === b);");
});

Deno.test("not: error on multiple args", () => {
  assertThrows(() => lykn("(not x y)"), Error, "exactly 1 argument");
});

// --- Regression: surface macros that emit kernel = ---

Deno.test("regression: reset! still emits assignment", () => {
  const result = lykn("(bind c (cell 0))\n(reset! c 42)");
  assertEquals(result, "const c = {\n  value: 0\n};\nc.value = 42;");
});

Deno.test("regression: swap! still emits assignment", () => {
  const result = lykn("(bind c (cell 0))\n(swap! c f)");
  assertEquals(result, "const c = {\n  value: 0\n};\nc.value = f(c.value);");
});

// --- Composition: = inside other surface forms ---

Deno.test("=: inside bind", () => {
  assertEquals(lykn("(bind result (= a b))"), "const result = a === b;");
});

Deno.test("and: inside bind", () => {
  assertEquals(lykn("(bind result (and x y))"), "const result = x && y;");
});

Deno.test("or: inside if", () => {
  const result = lykn("(if (or a b) (console:log 1))");
  assertEquals(result, "if (a || b) console.log(1);");
});

Deno.test("=: inside match", () => {
  // = should work as equality inside surface forms
  assertEquals(lykn("(bind result (= 1 1))"), "const result = 1 === 1;");
});
