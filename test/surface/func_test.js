import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("func: zero-arg shorthand", () => {
  const r__gensym0 = compile("(func make-ts (Date:now))");
  assertEquals(r__gensym0.trim(), "function makeTs() {\n  return Date.now();\n}");
});
Deno.test("func: zero-arg multi-expression", () => {
  const result = compile("(func init (console:log \"start\") 42)");
  assertStringIncludes(result, "console.log(\"start\")");
  assertStringIncludes(result, "return 42");
});
Deno.test("func: single clause with typed args", () => {
  const result = compile("(func add :args (:number a :number b) :returns :number :body (+ a b))");
  assertStringIncludes(result, "function add(a, b)");
  assertStringIncludes(result, "typeof a !== \"number\"");
  assertStringIncludes(result, "typeof b !== \"number\"");
  assertStringIncludes(result, "return");
});
Deno.test("func: single clause with :any args", () => {
  const result = compile("(func identity :args (:any x) :returns :any :body x)");
  assertStringIncludes(result, "function identity(x)");
  assertStringIncludes(result, "return x");
});
Deno.test("func: pre-condition", () => {
  const result = compile("(func positive :args (:number x) :returns :number :pre (> x 0) :body x)");
  assertStringIncludes(result, "pre-condition failed");
  assertStringIncludes(result, "caller blame");
  assertStringIncludes(result, "(> x 0)");
});
Deno.test("func: post-condition with tilde", () => {
  const result = compile("(func abs :args (:number x) :returns :number :post (>= ~ 0) :body (if (< x 0) (- 0 x) x))");
  assertStringIncludes(result, "post-condition failed");
  assertStringIncludes(result, "callee blame");
  assertStringIncludes(result, "(>= ~ 0)");
});
Deno.test("func: void return (no return statement)", () => {
  const result = compile("(func log-it :args (:any msg) :returns :void :body (console:log msg))");
  assertStringIncludes(result, "console.log(msg)");
});
Deno.test("func: multi-clause dispatch", () => {
  const result = compile("(func greet\n    (:args (:string name)\n     :returns :string\n     :body (+ \"Hello, \" name))\n    (:args (:string greeting :string name)\n     :returns :string\n     :body (+ greeting \", \" name)))");
  assertStringIncludes(result, "...");
  assertStringIncludes(result, ".length === 1");
  assertStringIncludes(result, ".length === 2");
  assertStringIncludes(result, "no matching clause");
});
Deno.test("func: multi-clause ordering (longer arity first)", () => {
  const result = compile("(func f\n    (:args (:any a) :returns :any :body a)\n    (:args (:any a :any b) :returns :any :body (+ a b)))");
  assertStringIncludes(result, ".length === 2");
  assertStringIncludes(result, ".length === 1");
});
Deno.test("func: multi-clause with type dispatch", () => {
  const result = compile("(func describe\n    (:args (:number n) :returns :string :body (+ \"number: \" n))\n    (:args (:string s) :returns :string :body (+ \"string: \" s)))");
  assertStringIncludes(result, "typeof");
  assertStringIncludes(result, "\"number\"");
  assertStringIncludes(result, "\"string\"");
});
