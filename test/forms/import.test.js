import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("import: side-effect", () => {
  assertEquals(lykn('(import "mod")'), 'import "mod";');
});

Deno.test("import: default", () => {
  assertEquals(lykn('(import "express" express)'), 'import express from "express";');
});

Deno.test("import: named", () => {
  const result = lykn('(import "fs" (read-file write-file))');
  assertEquals(result.includes("readFile"), true);
  assertEquals(result.includes("writeFile"), true);
  assertEquals(result.includes('"fs"'), true);
});

Deno.test("import: named with alias", () => {
  const result = lykn('(import "mod" ((alias foo bar)))');
  assertEquals(result.includes("foo as bar"), true);
});

Deno.test("import: default + named", () => {
  const result = lykn('(import "react" React (use-state use-effect))');
  assertEquals(result.includes("React"), true);
  assertEquals(result.includes("useState"), true);
  assertEquals(result.includes("useEffect"), true);
});

Deno.test("import: camelCase on names not paths", () => {
  const result = lykn('(import "node:fs" (read-file-sync))');
  assertEquals(result.includes("readFileSync"), true);
  assertEquals(result.includes('"node:fs"'), true);
});

Deno.test("import: no args throws", () => {
  assertThrows(() => lykn("(import)"));
});

Deno.test("import: non-string path throws", () => {
  assertThrows(() => lykn("(import foo)"));
});
