import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

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

// --- Nested destructuring ---

Deno.test("func: nested object with alias", () => {
  const result = lykn(
    "(func f :args ((object :string id (alias :any c (object :string name :string email)))) :body (console:log id name email))",
  );
  assertEquals(result.includes("{id, c: {name, email}}"), true);
  assertEquals(result.includes('typeof id !== "string"'), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
  assertEquals(result.includes('typeof email !== "string"'), true);
});

Deno.test("func: nested object in array (positional)", () => {
  const result = lykn(
    "(func f :args ((array (object :string name) :number score)) :body (console:log name score))",
  );
  assertEquals(result.includes("[{name}, score]"), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
  assertEquals(result.includes('typeof score !== "number"'), true);
});

Deno.test("func: two levels deep nesting", () => {
  const result = lykn(
    "(func f :args ((object (alias :any a (object :string city (alias :any g (object :number lat :number lng)))))) :body (console:log city lat lng))",
  );
  assertEquals(result.includes("{a: {city, g: {lat, lng}}}"), true);
  assertEquals(result.includes('typeof city !== "string"'), true);
  assertEquals(result.includes('typeof lat !== "number"'), true);
});

Deno.test("func: nested + default combined", () => {
  const result = lykn(
    '(func f :args ((object (default :string name "anon") (alias :any addr (object :string city)))) :body (console:log name city))',
  );
  assertEquals(result.includes('{name = "anon", addr: {city}}'), true);
});

Deno.test("fn: nested destructuring", () => {
  const result = lykn(
    "(bind f (fn ((object (alias :any c (object :string name)))) (console:log name)))",
  );
  assertEquals(result.includes("{c: {name}}"), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
});

Deno.test("func: error on nested without alias in object", () => {
  assertThrows(
    () => lykn("(func f :args ((object (object :string name))) :body 1)"),
    Error,
    "must use alias",
  );
});

Deno.test("func: error on alias missing inner pattern", () => {
  assertThrows(
    () => lykn("(func f :args ((object (alias :any name))) :body 1)"),
    Error,
    "requires",
  );
});

// --- Default values in destructured params ---

Deno.test("func: object destructuring with default", () => {
  const result = lykn(
    '(func f :args ((object :string name (default :number age 0))) :body (console:log name age))',
  );
  assertEquals(result.includes("{name, age = 0}"), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
  assertEquals(result.includes('typeof age !== "number"'), true);
});

Deno.test("func: object destructuring with multiple defaults", () => {
  const result = lykn(
    '(func f :args ((object (default :string name "anon") (default :number age 0))) :body (console:log name age))',
  );
  assertEquals(result.includes('{name = "anon", age = 0}'), true);
});

Deno.test("func: mixed default + non-default fields", () => {
  const result = lykn(
    '(func f :args ((object :string name (default :number age 0) :string email)) :body 1)',
  );
  assertEquals(result.includes("{name, age = 0, email}"), true);
});

Deno.test("func: default with :any — no type check", () => {
  const result = lykn(
    '(func f :args ((object (default :any name "anon") :number age)) :body (console:log name age))',
  );
  assertEquals(result.includes('{name = "anon", age}'), true);
  // :any default — no type check for name
  assertEquals(result.includes('typeof name'), false);
  // :number still checked
  assertEquals(result.includes('typeof age !== "number"'), true);
});

Deno.test("func: array destructuring with default", () => {
  const result = lykn(
    "(func f :args ((array :number first (default :number second 0))) :body (+ first second))",
  );
  assertEquals(result.includes("[first, second = 0]"), true);
});

Deno.test("func: default value is expression", () => {
  const result = lykn(
    "(func f :args ((object (default :number x (+ 1 2)))) :body x)",
  );
  assertEquals(result.includes("x = 1 + 2"), true);
});

Deno.test("fn: with default in destructured", () => {
  const result = lykn(
    '(bind f (fn ((object :string name (default :number age 0))) (console:log name age)))',
  );
  assertEquals(result.includes("{name, age = 0}"), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
});

Deno.test("func: default + rest in array", () => {
  const result = lykn(
    "(func f :args ((array (default :number first 0) (rest :number others))) :body (console:log first others))",
  );
  assertEquals(result.includes("[first = 0, ...others]"), true);
});

Deno.test("func: error on default missing value", () => {
  assertThrows(
    () => lykn("(func f :args ((object (default :number age))) :body 1)"),
    Error,
    "requires 3 arguments",
  );
});

Deno.test("func: error on default missing type", () => {
  assertThrows(
    () => lykn("(func f :args ((object (default age 0 1))) :body 1)"),
    Error,
    "must be a type keyword",
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

// --- Top-level default and rest in :args (DD-25.2) ---

Deno.test("func: top-level default param", () => {
  const result = lykn(
    '(func greet :args (:string name (default :string greeting "Hello")) :returns :string :body (template greeting ", " name "!"))',
  );
  assertEquals(result.includes('greeting = "Hello"'), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
  assertEquals(result.includes('typeof greeting !== "string"'), true);
});

Deno.test("func: multiple top-level defaults", () => {
  const result = lykn(
    "(func f :args ((default :number x 0) (default :number y 0)) :body (+ x y))",
  );
  assertEquals(result.includes("x = 0"), true);
  assertEquals(result.includes("y = 0"), true);
});

Deno.test("func: top-level rest param :any — no per-element check", () => {
  const result = lykn(
    "(func log-all :args (:string level (rest :any messages)) :body (console:log level messages))",
  );
  assertEquals(result.includes("...messages"), true);
  assertEquals(result.includes('typeof level !== "string"'), true);
  // :any rest — no for-of check
  assertEquals(result.includes("for (const"), false);
});

Deno.test("func: top-level rest param :number — per-element check", () => {
  const result = lykn(
    "(func sum :args ((rest :number nums)) :returns :number :body (nums:reduce (fn (:number acc :number n) (+ acc n)) 0))",
  );
  assertEquals(result.includes("...nums"), true);
  // Per-element type check via for-of
  assertEquals(result.includes("for (const"), true);
  assertEquals(result.includes("of nums"), true);
});

Deno.test("fn: top-level default", () => {
  const result = lykn(
    '(bind f (fn ((default :string name "anon")) (console:log name)))',
  );
  assertEquals(result.includes('name = "anon"'), true);
  assertEquals(result.includes('typeof name !== "string"'), true);
});

Deno.test("fn: top-level rest :any", () => {
  const result = lykn(
    "(bind f (fn ((rest :any args)) (console:log args)))",
  );
  assertEquals(result.includes("...args"), true);
  assertEquals(result.includes("for (const"), false);
});

Deno.test("fn: top-level rest :string — per-element check", () => {
  const result = lykn(
    "(bind f (fn ((rest :string items)) (console:log items)))",
  );
  assertEquals(result.includes("...items"), true);
  assertEquals(result.includes("for (const"), true);
});

Deno.test("func: mixed destructured + default + rest", () => {
  const result = lykn(
    "(func handle :args ((object :string method :string url) (default :number timeout 5000) (rest :any middleware)) :body (console:log method url timeout middleware))",
  );
  assertEquals(result.includes("{method, url}"), true);
  assertEquals(result.includes("timeout = 5000"), true);
  assertEquals(result.includes("...middleware"), true);
});

Deno.test("func: default with :any — no type check", () => {
  const result = lykn(
    '(func f :args ((default :any name "anon")) :body (console:log name))',
  );
  assertEquals(result.includes('name = "anon"'), true);
  assertEquals(result.includes("typeof name"), false);
});

Deno.test("func: error on rest not last in :args", () => {
  assertThrows(
    () => lykn("(func f :args ((rest :any r) :string x) :body 1)"),
    Error,
    "rest parameter must be the last parameter",
  );
});

Deno.test("func: error on multiple rest in :args", () => {
  assertThrows(
    () => lykn("(func f :args ((rest :any a) (rest :any b)) :body 1)"),
    Error,
    "only one rest parameter allowed",
  );
});

Deno.test("func: error on default missing value", () => {
  assertThrows(
    () => lykn("(func f :args ((default :number x)) :body 1)"),
    Error,
    "requires exactly 3 elements",
  );
});
