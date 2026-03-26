import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("function: basic declaration", () => {
  assertEquals(lykn("(function add (a b) (return (+ a b)))"),
    "function add(a, b) {\n  return a + b;\n}");
});

Deno.test("function: camelCase name", () => {
  assertEquals(lykn("(function my-handler (req) (return req))"),
    "function myHandler(req) {\n  return req;\n}");
});

Deno.test("function: no params", () => {
  assertEquals(lykn("(function init () (return 42))"),
    "function init() {\n  return 42;\n}");
});

Deno.test("function: multi-statement body", () => {
  const result = lykn("(function setup () (const x 1) (const y 2) (return (+ x y)))");
  assertEquals(result.includes("const x = 1;"), true);
  assertEquals(result.includes("const y = 2;"), true);
  assertEquals(result.includes("return x + y;"), true);
});

Deno.test("function: missing name throws", () => {
  assertThrows(() => lykn("(function (a b) (return 1))"));
});
