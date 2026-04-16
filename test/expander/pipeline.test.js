import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

// --- Three-pass pipeline ---

Deno.test("pipeline: pass 0 processes import-macros", () => {
  // import-macros now works — file not found is the expected error for a bad path
  assertThrows(() => ex('(import-macros "./nonexistent.lykn" (bar))'), Error, "not found");
});

Deno.test("pipeline: pass 1 processes macros before pass 2", () => {
  const result = lykn(`
    (macro double (x) \`(+ ,x ,x))
    (double 21)
  `);
  assertEquals(result, "21 + 21;");
});

Deno.test("pipeline: macros erased from output", () => {
  const result = ex(`
    (macro noop () \`null)
    (const x 1)
  `);
  assertEquals(result.length, 1);
});

Deno.test("pipeline: order-independent macro compilation", () => {
  // unless depends on when, but is defined before it
  const result = lykn(`
    (macro unless (test (rest body))
      \`(when (! ,test) ,@body))
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))
    (unless false (console:log "yes"))
  `);
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("!false"), true);
});

Deno.test("pipeline: circular dependency detected", () => {
  assertThrows(() => ex(`
    (macro a () \`(b))
    (macro b () \`(a))
    (a)
  `), Error, "circular");
});

Deno.test("pipeline: duplicate macro name detected", () => {
  assertThrows(() => ex(`
    (macro foo () \`null)
    (macro foo () \`null)
  `), Error, "duplicate");
});

Deno.test("pipeline: sugar forms expanded in pass 2", () => {
  const result = lykn("(car (cons 1 2))");
  assertEquals(result, "[1, 2][0];");
});

Deno.test("pipeline: full end-to-end with all features", () => {
  const result = lykn(`
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))

    (const data (cons 1 (cons 2 null)))
    (when (car data)
      (console:log (template "first: " (car data))))
  `);
  assertEquals(result.includes("if"), true);
  assertEquals(result.includes("data[0]"), true);
  assertEquals(result.includes("`first: ${data[0]}`"), true);
});
