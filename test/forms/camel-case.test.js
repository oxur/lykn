import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

// Conversion table
Deno.test("camelCase: my-function → myFunction", () => {
  assertEquals(lykn("my-function"), "myFunction;");
});

Deno.test("camelCase: get-x → getX", () => {
  assertEquals(lykn("get-x"), "getX;");
});

Deno.test("camelCase: get-HTTP-response → getHTTPResponse", () => {
  assertEquals(lykn("get-HTTP-response"), "getHTTPResponse;");
});

Deno.test("camelCase: leading hyphen → underscore", () => {
  assertEquals(lykn("-foo"), "_foo;");
});

Deno.test("camelCase: double leading hyphens → double underscore", () => {
  assertEquals(lykn("--foo"), "__foo;");
});

Deno.test("camelCase: trailing hyphen → trailing underscore", () => {
  assertEquals(lykn("foo-"), "foo_;");
});

Deno.test("camelCase: consecutive mid hyphens → single boundary", () => {
  assertEquals(lykn("foo--bar"), "fooBar;");
});

Deno.test("camelCase: all-caps no hyphens → unchanged", () => {
  assertEquals(lykn("JSON"), "JSON;");
});

Deno.test("camelCase: existing underscore → unchanged", () => {
  assertEquals(lykn("_private"), "_private;");
});

Deno.test("camelCase: multiple segments", () => {
  assertEquals(lykn("my-var-name"), "myVarName;");
});

Deno.test("camelCase: single char no hyphens", () => {
  assertEquals(lykn("x"), "x;");
});

Deno.test("camelCase: all single-letter segments", () => {
  assertEquals(lykn("a-b-c"), "aBC;");
});

Deno.test("camelCase: get-element-by-id real-world", () => {
  assertEquals(lykn("get-element-by-id"), "getElementById;");
});

Deno.test("camelCase: inner-HTML preserves existing uppercase", () => {
  assertEquals(lykn("inner-HTML"), "innerHTML;");
});

// In context
Deno.test("camelCase: in variable declaration", () => {
  assertEquals(lykn("(const my-var 42)"), "const myVar = 42;");
});

Deno.test("camelCase: in function call arguments", () => {
  assertEquals(lykn("(foo my-arg)"), "foo(myArg);");
});
