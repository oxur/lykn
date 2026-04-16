import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros, formatSExpr, sym, array } from "lang/expander.js";

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

// --- formatSExpr ---

Deno.test("formatSExpr: atom", () => {
  assertEquals(formatSExpr(sym("foo")), "foo");
});

Deno.test("formatSExpr: number", () => {
  assertEquals(formatSExpr({ type: "number", value: 42 }), "42");
});

Deno.test("formatSExpr: string", () => {
  assertEquals(formatSExpr({ type: "string", value: "hi" }), '"hi"');
});

Deno.test("formatSExpr: list", () => {
  assertEquals(formatSExpr(array(sym("if"), sym("x"), sym("y"))), "(if x y)");
});

Deno.test("formatSExpr: cons", () => {
  assertEquals(formatSExpr({ type: "cons", car: sym("a"), cdr: sym("b") }), "(a . b)");
});

Deno.test("formatSExpr: null", () => {
  assertEquals(formatSExpr(null), "null");
});

// --- macroexpand ---

Deno.test("macroexpand: fully expands macro call", () => {
  // macroexpand prints to stderr and returns null (erased)
  // We can verify it doesn't throw and produces no output forms
  const result = ex(`
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))
    (macroexpand '(when true (console:log "hi")))
    (const x 1)
  `);
  // macroexpand erased, only (const x 1) remains
  assertEquals(result.length, 1);
  assertEquals(result[0].values[0].value, "const");
});

Deno.test("macroexpand-1: one step expansion", () => {
  const result = ex(`
    (macro unless (test (rest body))
      \`(when (! ,test) ,@body))
    (macro when (test (rest body))
      \`(if ,test (block ,@body)))
    (macroexpand-1 '(unless true (foo)))
    (const x 1)
  `);
  // macroexpand-1 erased, only (const x 1) remains
  assertEquals(result.length, 1);
});

Deno.test("macroexpand: non-macro form prints unchanged", () => {
  const result = ex(`
    (macroexpand '(+ 1 2))
    (const x 1)
  `);
  assertEquals(result.length, 1);
});

Deno.test("macroexpand: wrong arity throws", () => {
  assertThrows(() => ex("(macroexpand)"), Error, "one argument");
});
