import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, expandExpr, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("macro: simple when macro", () => {
  const result = lykn(`
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))
    (when (> x 0) (console:log "positive"))
  `);
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("x > 0"), true);
  assertEquals(result.includes("positive"), true);
});

Deno.test("macro: unless via when", () => {
  const result = lykn(`
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))
    (macro unless (test (rest body))
      \`(when (! ,test) ,@body))
    (unless (=== x 0) (console:log "nonzero"))
  `);
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("!"), true);
});

Deno.test("macro: erased from output", () => {
  const result = ex(`
    (macro noop () \`null)
    (const x 42)
  `);
  // Should only have the const form, not the macro
  assertEquals(result.length, 1);
  assertEquals(result[0].values[0].value, "const");
});

Deno.test("macro: duplicate name throws", () => {
  assertThrows(() => ex(`
    (macro foo () \`null)
    (macro foo () \`null)
  `), Error, "duplicate");
});

Deno.test("macro: no body throws", () => {
  assertThrows(() => ex("(macro foo)"), Error);
});

Deno.test("macro: produces correct expansion", () => {
  const result = ex(`
    (macro double (x)
      \`(+ ,x ,x))
    (double y)
  `);
  // Should produce (+ y y)
  assertEquals(result[0].values[0].value, "+");
  assertEquals(result[0].values[1].value, "y");
  assertEquals(result[0].values[2].value, "y");
});

Deno.test("macro: body without quasiquote", () => {
  const result = ex(`
    (macro identity (x) x)
    (identity (+ 1 2))
  `);
  // Should pass through (+ 1 2)
  assertEquals(result[0].values[0].value, "+");
});
