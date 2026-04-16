import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("try: catch only", () => {
  const result = lykn("(try (do-something) (catch e (handle e)))");
  assertEquals(result.includes("try"), true);
  assertEquals(result.includes("catch"), true);
});

Deno.test("try: finally only", () => {
  const result = lykn("(try (do-something) (finally (cleanup)))");
  assertEquals(result.includes("finally"), true);
});

Deno.test("try: catch + finally", () => {
  const result = lykn("(try (do-something) (catch e (handle e)) (finally (cleanup)))");
  assertEquals(result.includes("catch"), true);
  assertEquals(result.includes("finally"), true);
});

Deno.test("try: no catch or finally throws", () => {
  assertThrows(() => lykn("(try (do-something))"));
});
