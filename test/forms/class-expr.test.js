import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("class-expr: basic", () => {
  const result = lykn('(const MyClass (class-expr () (constructor () (return))))');
  assertEquals(result.includes('class'), true);
  assertEquals(result.includes('constructor'), true);
});

Deno.test("class-expr: with extends", () => {
  const result = lykn('(const Sub (class-expr (Base) (constructor () (super))))');
  assertEquals(result.includes('extends Base'), true);
});
