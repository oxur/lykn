import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("tag: basic tagged template", () => {
  const result = lykn('(tag html (template "<div>" content "</div>"))');
  assertEquals(result.includes('html`'), true);
  assertEquals(result.includes('${content}'), true);
});

Deno.test("tag: tag is a member expression", () => {
  const result = lykn('(tag String:raw (template "\\n"))');
  assertEquals(result.includes('String.raw'), true);
  assertEquals(result.includes('`'), true);
});

Deno.test("tag: non-template second arg throws", () => {
  assertThrows(() => lykn('(tag html "not a template")'));
});
