import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

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

Deno.test("bind: with type annotation and expression", () => {
  assertEquals(lykn("(bind :string name (get-name user))"), "const name = getName(user);");
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
