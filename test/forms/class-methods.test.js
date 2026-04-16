import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("class method: constructor", () => {
  const result = lykn('(class Foo () (constructor (x) (= this:x x)))');
  assertEquals(result.includes('constructor(x)'), true);
  assertEquals(result.includes('this.x = x'), true);
});

Deno.test("class method: getter", () => {
  const result = lykn('(class C () (get area () (return 42)))');
  assertEquals(result.includes('get area'), true);
});

Deno.test("class method: setter", () => {
  const result = lykn('(class C () (set radius (r) (= this:r r)))');
  assertEquals(result.includes('set radius'), true);
});

Deno.test("class method: static method", () => {
  const result = lykn('(class C () (static (create () (return (new C)))))');
  assertEquals(result.includes('static'), true);
  assertEquals(result.includes('create'), true);
});

Deno.test("class method: static field", () => {
  const result = lykn('(class C () (static (field count 0)))');
  assertEquals(result.includes('static'), true);
  assertEquals(result.includes('count'), true);
});

Deno.test("class method: camelCase method name", () => {
  const result = lykn('(class C () (get-data () (return 1)))');
  assertEquals(result.includes('getData'), true);
});
