import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("break: no label", () => {
  assertEquals(lykn("(break)"), "break;");
});

Deno.test("break: with label", () => {
  assertEquals(lykn("(break my-loop)"), "break myLoop;");
});

Deno.test("continue: no label", () => {
  assertEquals(lykn("(continue)"), "continue;");
});

Deno.test("continue: with label", () => {
  assertEquals(lykn("(continue my-loop)"), "continue myLoop;");
});
