import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("get: numeric index", () => {
  assertEquals(lykn("(get arr 0)"), "arr[0];");
});

Deno.test("get: string key", () => {
  assertEquals(lykn('(get obj "name")'), 'obj["name"];');
});

Deno.test("get: variable key", () => {
  assertEquals(lykn("(get obj key)"), "obj[key];");
});

Deno.test("get: nested", () => {
  assertEquals(lykn("(get (get matrix 0) 1)"), "matrix[0][1];");
});

Deno.test("get: expression as key", () => {
  assertEquals(lykn("(get args (- len 1))"), "args[len - 1];");
});

Deno.test("get: wrong arity throws", () => {
  assertThrows(() => lykn("(get obj)"), Error, "2 arguments");
});
