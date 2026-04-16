import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("default: in arrow params", () => {
  const result = lykn('(const f (=> ((default x 0)) x))');
  assertEquals(result.includes('x = 0'), true);
});

Deno.test("default: in function params", () => {
  const result = lykn('(function greet ((default name "world")) (return name))');
  assertEquals(result.includes('name = "world"'), true);
});

Deno.test("default: multiple defaults", () => {
  const result = lykn('(=> ((default x 0) (default y 0)) (+ x y))');
  assertEquals(result.includes('x = 0'), true);
  assertEquals(result.includes('y = 0'), true);
});
