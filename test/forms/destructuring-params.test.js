import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("params: object destructuring in arrow", () => {
  const result = lykn('(const f (=> ((object name age)) (console:log name)))');
  assertEquals(result.includes('{name, age}') || result.includes('{ name, age }'), true);
});

Deno.test("params: object destructuring in function", () => {
  const result = lykn('(function greet ((object name)) (return name))');
  assertEquals(result.includes('{name}') || result.includes('{ name }'), true);
});

Deno.test("params: mixed regular and destructured", () => {
  const result = lykn('(function handle (req (object data)) (return data))');
  assertEquals(result.includes('req'), true);
  assertEquals(result.includes('data'), true);
});

Deno.test("params: default + destructuring", () => {
  const result = lykn('(=> ((default x 0) (object name)) (+ x name))');
  assertEquals(result.includes('x = 0'), true);
  assertEquals(result.includes('name'), true);
});

Deno.test("params: rest parameter", () => {
  const result = lykn('(function f (a b (rest args)) (return args))');
  assertEquals(result.includes('...args'), true);
});

Deno.test("params: array destructuring in for-of", () => {
  const result = lykn('(for-of (array key value) entries (console:log key))');
  assertEquals(result.includes('[key, value]'), true);
});

Deno.test("backward compat: const with plain binding", () => {
  assertEquals(lykn('(const x 42)'), 'const x = 42;');
});

Deno.test("backward compat: let with plain binding", () => {
  assertEquals(lykn('(const my-var 42)'), 'const myVar = 42;');
});

Deno.test("backward compat: function with plain params", () => {
  const result = lykn('(function add (a b) (return (+ a b)))');
  assertEquals(result.includes('function add(a, b)'), true);
});

Deno.test("backward compat: arrow with plain params", () => {
  const result = lykn('(const f (=> (x) (* x 2)))');
  assertEquals(result.includes('x'), true);
});
