import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("assignment: object destructuring", () => {
  const result = lykn('(= (object a b) obj)');
  assertEquals(result.includes('a'), true);
  assertEquals(result.includes('= obj'), true);
});

Deno.test("assignment: array destructuring", () => {
  const result = lykn('(= (array x y) pair)');
  assertEquals(result.includes('[x, y]'), true);
});

Deno.test("assignment: regular (non-destructuring) still works", () => {
  assertEquals(lykn('(= x 5)'), 'x = 5;');
});

Deno.test("assignment: member expression still works", () => {
  const result = lykn('(= this:count 0)');
  assertEquals(result.includes('this.count = 0'), true);
});
