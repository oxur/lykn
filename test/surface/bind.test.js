import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

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

Deno.test("bind: simple binding", () => {
  assertEquals(lykn("(bind x 42)"), "const x = 42;");
});

Deno.test("bind: string value", () => {
  assertEquals(lykn('(bind name "Duncan")'), 'const name = "Duncan";');
});

Deno.test("bind: expression value", () => {
  assertEquals(lykn("(bind result (+ 1 2))"), "const result = 1 + 2;");
});

Deno.test("bind: with type annotation", () => {
  assertEquals(lykn("(bind :number age 42)"), "const age = 42;");
});

Deno.test("bind: with type annotation and expression — emits runtime check", () => {
  const result = lykn("(bind :string name (get-name user))");
  assertEquals(result.includes("const name = getName(user);"), true);
  assertEquals(result.includes("typeof name !== \"string\""), true);
  assertEquals(result.includes("TypeError"), true);
});

Deno.test("bind: type annotation on literal — no runtime check", () => {
  assertEquals(lykn('(bind :string name "hello")'), 'const name = "hello";');
});

Deno.test("bind: :any annotation — no runtime check", () => {
  assertEquals(lykn("(bind :any x (compute))"), "const x = compute();");
});

Deno.test("bind: type mismatch on literal — compile error", () => {
  let error;
  try { lykn('(bind :number x "hello")'); } catch (e) { error = e; }
  assertEquals(error instanceof Error, true);
  assertEquals(error.message.includes("type annotation is :number but initializer is a string"), true);
});

Deno.test("bind: expansion produces const form", () => {
  const result = ex("(bind x 42)");
  assertEquals(result[0].values[0].value, "const");
  assertEquals(result[0].values[1].value, "x");
  assertEquals(result[0].values[2].value, 42);
});

Deno.test("bind: typed expansion drops type keyword", () => {
  const result = ex("(bind :number x 42)");
  assertEquals(result[0].values[0].value, "const");
  assertEquals(result[0].values[1].value, "x");
  assertEquals(result[0].values[2].value, 42);
});

Deno.test("bind: kebab-case name", () => {
  assertEquals(lykn("(bind my-value 10)"), "const myValue = 10;");
});
