import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("template: string only", () => {
  assertEquals(lykn('(template "hello")'), '`hello`;');
});

Deno.test("template: expression only", () => {
  assertEquals(lykn('(template name)'), '`${name}`;');
});

Deno.test("template: string-expr-string", () => {
  assertEquals(lykn('(template "Hello, " name "!")'), '`Hello, ${name}!`;');
});

Deno.test("template: two adjacent expressions", () => {
  assertEquals(lykn('(template a b)'), '`${a}${b}`;');
});

Deno.test("template: expression at start", () => {
  assertEquals(lykn('(template name " is here")'), '`${name} is here`;');
});

Deno.test("template: expression at end", () => {
  assertEquals(lykn('(template "value: " x)'), '`value: ${x}`;');
});

Deno.test("template: multiple expressions with strings", () => {
  assertEquals(lykn('(template "a=" a ", b=" b)'), '`a=${a}, b=${b}`;');
});

Deno.test("template: three adjacent expressions", () => {
  assertEquals(lykn('(template a b c)'), '`${a}${b}${c}`;');
});

Deno.test("template: empty", () => {
  assertEquals(lykn('(template)'), '``;');
});

Deno.test("template: expression is a call", () => {
  assertEquals(lykn('(template "Result: " (compute x))'), '`Result: ${compute(x)}`;');
});

Deno.test("template: nested template", () => {
  const result = lykn('(template "outer " (template "inner " x) " end")');
  assertEquals(result.includes('`outer ${`inner ${x}`} end`'), true);
});
