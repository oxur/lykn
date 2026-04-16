import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("spread: in array", () => {
  assertEquals(lykn('(array 1 2 (spread rest))'), '[1, 2, ...rest];');
});

Deno.test("spread: in function call", () => {
  assertEquals(lykn('(foo (spread args))'), 'foo(...args);');
});

Deno.test("spread: in array at start", () => {
  assertEquals(lykn('(array (spread first) 4 5)'), '[...first, 4, 5];');
});
