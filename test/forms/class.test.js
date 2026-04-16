import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("class: basic no extends", () => {
  const result = lykn('(class Foo () (constructor () (return)))');
  assertEquals(result.includes('class Foo'), true);
  assertEquals(result.includes('constructor'), true);
});

Deno.test("class: with extends", () => {
  const result = lykn('(class Dog (Animal) (constructor (name) (super name)))');
  assertEquals(result.includes('extends Animal'), true);
  assertEquals(result.includes('super(name)'), true);
});

Deno.test("class: empty body", () => {
  const result = lykn('(class Empty ())');
  assertEquals(result.includes('class Empty'), true);
});

Deno.test("class: method with this", () => {
  const result = lykn('(class Greeter () (greet () (return this:name)))');
  assertEquals(result.includes('this.name'), true);
});
