import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("export: declaration", () => {
  assertEquals(lykn("(export (const x 42))"), "export const x = 42;");
});

Deno.test("export: function declaration", () => {
  const result = lykn("(export (function foo () (return 1)))");
  assertEquals(result.includes("export function foo"), true);
});

Deno.test("export: default expression", () => {
  assertEquals(lykn("(export default my-fn)"), "export default myFn;");
});

Deno.test("export: named bindings", () => {
  const result = lykn("(export (names a b))");
  assertEquals(result.includes("a"), true);
  assertEquals(result.includes("b"), true);
});

Deno.test("export: named with alias", () => {
  const result = lykn("(export (names (alias my-func external-name)))");
  assertEquals(result.includes("myFunc as externalName"), true);
});

Deno.test("export: re-export from module", () => {
  const result = lykn('(export "mod" (names foo bar))');
  assertEquals(result.includes('"mod"'), true);
  assertEquals(result.includes("foo"), true);
});

Deno.test("export: no args throws", () => {
  assertThrows(() => lykn("(export)"));
});
