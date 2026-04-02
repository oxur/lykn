import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- Zero-arg shorthand ---

Deno.test("func: zero-arg shorthand", () => {
  const result = lykn("(func make-ts (Date:now))");
  assertEquals(result, "function makeTs() {\n  return Date.now();\n}");
});

Deno.test("func: zero-arg multi-expression", () => {
  const result = lykn('(func init (console:log "start") 42)');
  assertEquals(result.includes('console.log("start")'), true);
  assertEquals(result.includes("return 42"), true);
});

// --- Single clause ---

Deno.test("func: single clause with typed args", () => {
  const result = lykn("(func add :args (:number a :number b) :returns :number :body (+ a b))");
  assertEquals(result.includes("function add(a, b)"), true);
  assertEquals(result.includes("typeof a !== \"number\""), true);
  assertEquals(result.includes("typeof b !== \"number\""), true);
  assertEquals(result.includes("return"), true);
});

Deno.test("func: single clause with :any args", () => {
  const result = lykn("(func identity :args (:any x) :returns :any :body x)");
  assertEquals(result.includes("function identity(x)"), true);
  assertEquals(result.includes("typeof"), false);
  assertEquals(result.includes("return x"), true);
});

Deno.test("func: pre-condition", () => {
  const result = lykn("(func positive :args (:number x) :returns :number :pre (> x 0) :body x)");
  assertEquals(result.includes("pre-condition failed"), true);
  assertEquals(result.includes("caller blame"), true);
  assertEquals(result.includes("(> x 0)"), true);
});

Deno.test("func: post-condition with tilde", () => {
  const result = lykn("(func abs :args (:number x) :returns :number :post (>= ~ 0) :body (if (< x 0) (- 0 x) x))");
  assertEquals(result.includes("post-condition failed"), true);
  assertEquals(result.includes("callee blame"), true);
  assertEquals(result.includes("(>= ~ 0)"), true);
});

Deno.test("func: void return (no return statement)", () => {
  const result = lykn('(func log-it :args (:any msg) :returns :void :body (console:log msg))');
  assertEquals(result.includes("console.log(msg)"), true);
  assertEquals(result.includes("return"), false);
});

// --- Multi-clause ---

Deno.test("func: multi-clause dispatch", () => {
  const result = lykn(`(func greet
    (:args (:string name)
     :returns :string
     :body (+ "Hello, " name))
    (:args (:string greeting :string name)
     :returns :string
     :body (+ greeting ", " name)))`);
  assertEquals(result.includes("..."), true); // rest params
  assertEquals(result.includes(".length === 1"), true);
  assertEquals(result.includes(".length === 2"), true);
  assertEquals(result.includes("no matching clause"), true);
});

Deno.test("func: multi-clause ordering (longer arity first)", () => {
  const result = lykn(`(func f
    (:args (:any a) :returns :any :body a)
    (:args (:any a :any b) :returns :any :body (+ a b)))`);
  // Longer arity (2) should appear before shorter (1) in output
  const idx2 = result.indexOf(".length === 2");
  const idx1 = result.indexOf(".length === 1");
  assertEquals(idx2 < idx1, true);
});

Deno.test("func: multi-clause with type dispatch", () => {
  const result = lykn(`(func describe
    (:args (:number n) :returns :string :body (+ "number: " n))
    (:args (:string s) :returns :string :body (+ "string: " s)))`);
  assertEquals(result.includes('typeof'), true);
  assertEquals(result.includes('"number"'), true);
  assertEquals(result.includes('"string"'), true);
});
