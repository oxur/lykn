import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

function expandOnly(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

// --- Object destructuring ---

Deno.test("func: object destructuring single clause", () => {
  const result = lykn(
    '(func process :args ((object :string name :number age) :string action) :returns :string :body (template name " (" age ") — " action))',
  );
  assertEquals(result.includes("function process({name, age}, action)"), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
  assertEquals(result.includes('typeof age !== "number"'), true);
  assertEquals(result.includes("Number.isNaN(age)"), true);
  assertEquals(result.includes('typeof action !== "string"'), true);
  assertEquals(result.includes("return"), true);
});

Deno.test("func: object destructuring with :any field", () => {
  const result = lykn(
    "(func f :args ((object :any name :number age)) :body (console:log name age))",
  );
  assertEquals(result.includes("{name, age}"), true);
  // :any field should not produce a type check
  assertEquals(result.includes('typeof name !== "'), false);
  // :number field should
  assertEquals(result.includes('typeof age !== "number"'), true);
});

// --- Array destructuring ---

Deno.test("func: array destructuring single clause", () => {
  const result = lykn(
    "(func f :args ((array :number first :number second)) :body (+ first second))",
  );
  assertEquals(result.includes("[first, second]"), true);
  assertEquals(result.includes('typeof first !== "number"'), true);
  assertEquals(result.includes('typeof second !== "number"'), true);
});

Deno.test("func: array destructuring with rest", () => {
  const result = lykn(
    "(func head-tail :args ((array :number first (rest :number remaining))) :body (console:log first remaining))",
  );
  assertEquals(result.includes("[first, ...remaining]"), true);
  assertEquals(result.includes('typeof first !== "number"'), true);
});

Deno.test("func: array destructuring with skip", () => {
  const result = lykn(
    "(func f :args ((array :number first _ :number third)) :body (+ first third))",
  );
  assertEquals(result.includes("[first, , third]"), true);
});

// --- Mixed params ---

Deno.test("func: mixed destructured + simple params", () => {
  const result = lykn(
    '(func handler :args ((object :string method :string url) :any body) :body (console:log method url body))',
  );
  assertEquals(result.includes("function handler({method, url}, body)"), true);
  assertEquals(result.includes('typeof method !== "string"'), true);
  assertEquals(result.includes('typeof url !== "string"'), true);
  // :any body should not produce a type check for body
  assertEquals(result.includes('typeof body !== "'), false);
});

// --- fn/lambda ---

Deno.test("fn: object destructuring", () => {
  const result = lykn(
    "(bind f (fn ((object :string name :number age)) (console:log name age)))",
  );
  assertEquals(result.includes("({name, age})"), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
  assertEquals(result.includes('typeof age !== "number"'), true);
});

Deno.test("fn: all :any destructured fields — concise arrow", () => {
  const result = lykn(
    "(bind f (fn ((object :any x :any y)) (+ x y)))",
  );
  assertEquals(result.includes("({x, y})"), true);
  // No type checks, so concise arrow body
  assertEquals(result.includes("typeof"), false);
});

// --- Multi-clause dispatch ---

Deno.test("func: multi-clause object destructured vs string dispatch", () => {
  const result = lykn(
    '(func process (:args ((object :string name) :string action) :body (template name ": " action)) (:args (:string raw :string action) :body (template raw " — " action)))',
  );
  // Should produce a multi-clause function with dispatch
  assertEquals(result.includes('=== "object"'), true);
  assertEquals(result.includes('=== "string"'), true);
  assertEquals(result.includes("const {name}"), true);
  assertEquals(result.includes("const raw"), true);
});

Deno.test("func: multi-clause object vs array destructuring", () => {
  const result = lykn(
    '(func transform (:args ((object :string name)) :body name) (:args ((array :number first)) :body first))',
  );
  // Object dispatch
  assertEquals(result.includes('=== "object"'), true);
  // Array dispatch
  assertEquals(result.includes("Array.isArray("), true);
  // Destructuring bindings
  assertEquals(result.includes("const {name}"), true);
  assertEquals(result.includes("const [first]"), true);
});

// --- Error cases ---

Deno.test("func: error on empty object pattern", () => {
  assertThrows(
    () => lykn("(func f :args ((object)) :body 1)"),
    Error,
    "empty destructuring pattern",
  );
});

Deno.test("func: error on empty array pattern", () => {
  assertThrows(
    () => lykn("(func f :args ((array)) :body 1)"),
    Error,
    "empty destructuring pattern",
  );
});

Deno.test("func: error on bare name without type in object", () => {
  assertThrows(
    () => lykn("(func f :args ((object name)) :body 1)"),
    Error,
    "missing type annotation",
  );
});

Deno.test("func: error on nested destructuring", () => {
  assertThrows(
    () =>
      lykn(
        "(func f :args ((object :string name (alias :any addr (object :string city)))) :body 1)",
      ),
    Error,
    "nested destructuring",
  );
});

Deno.test("func: error on default in destructured", () => {
  assertThrows(
    () =>
      lykn(
        '(func f :args ((object (default :string name "anon") :number age)) :body 1)',
      ),
    Error,
    "default values in destructured params",
  );
});

Deno.test("func: error on rest not last in array", () => {
  assertThrows(
    () =>
      lykn(
        "(func f :args ((array (rest :number r) :number x)) :body 1)",
      ),
    Error,
    "rest element must be last",
  );
});
