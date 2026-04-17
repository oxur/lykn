import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("func: object destructuring single clause", () => {
  const result = compile("(func process :args ((object :string name :number age) :string action) :returns :string :body (template name \" (\" age \") — \" action))");
  assertStringIncludes(result, "function process({name, age}, action)");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "typeof age !== \"number\"");
  assertStringIncludes(result, "Number.isNaN(age)");
  assertStringIncludes(result, "typeof action !== \"string\"");
  assertStringIncludes(result, "return");
});
Deno.test("func: object destructuring with :any field", () => {
  const result = compile("(func f :args ((object :any name :number age)) :body (console:log name age))");
  assertStringIncludes(result, "{name, age}");
});
Deno.test("func: array destructuring single clause", () => {
  const result = compile("(func f :args ((array :number first :number second)) :body (+ first second))");
  assertStringIncludes(result, "[first, second]");
  assertStringIncludes(result, "typeof first !== \"number\"");
  assertStringIncludes(result, "typeof second !== \"number\"");
});
Deno.test("func: array destructuring with rest", () => {
  const result = compile("(func head-tail :args ((array :number first (rest :number remaining))) :body (console:log first remaining))");
  assertStringIncludes(result, "[first, ...remaining]");
  assertStringIncludes(result, "typeof first !== \"number\"");
});
Deno.test("func: array destructuring with skip", () => {
  const result = compile("(func f :args ((array :number first _ :number third)) :body (+ first third))");
  assertStringIncludes(result, "[first, , third]");
});
Deno.test("func: mixed destructured + simple params", () => {
  const result = compile("(func handler :args ((object :string method :string url) :any body) :body (console:log method url body))");
  assertStringIncludes(result, "function handler({method, url}, body)");
  assertStringIncludes(result, "typeof method !== \"string\"");
  assertStringIncludes(result, "typeof url !== \"string\"");
});
Deno.test("fn: object destructuring", () => {
  const result = compile("(bind f (fn ((object :string name :number age)) (console:log name age)))");
  assertStringIncludes(result, "({name, age})");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "typeof age !== \"number\"");
});
Deno.test("fn: all :any destructured fields — concise arrow", () => {
  const result = compile("(bind f (fn ((object :any x :any y)) (+ x y)))");
  assertStringIncludes(result, "({x, y})");
});
Deno.test("func: multi-clause object destructured vs string dispatch", () => {
  const result = compile("(func process (:args ((object :string name) :string action) :body (template name \": \" action)) (:args (:string raw :string action) :body (template raw \" — \" action)))");
  assertStringIncludes(result, "=== \"object\"");
  assertStringIncludes(result, "=== \"string\"");
  assertStringIncludes(result, "const {name}");
  assertStringIncludes(result, "const raw");
});
Deno.test("func: multi-clause object vs array destructuring", () => {
  const result = compile("(func transform (:args ((object :string name)) :body name) (:args ((array :number first)) :body first))");
  assertStringIncludes(result, "=== \"object\"");
  assertStringIncludes(result, "Array.isArray(");
  assertStringIncludes(result, "const {name}");
  assertStringIncludes(result, "const [first]");
});
Deno.test("func: error on empty object pattern", () => assertThrows(() => compile("(func f :args ((object)) :body 1)"), Error, "empty destructuring pattern"));
Deno.test("func: error on empty array pattern", () => assertThrows(() => compile("(func f :args ((array)) :body 1)"), Error, "empty destructuring pattern"));
Deno.test("func: error on bare name without type in object", () => assertThrows(() => compile("(func f :args ((object name)) :body 1)"), Error, "missing type annotation"));
Deno.test("func: nested object with alias", () => {
  const result = compile("(func f :args ((object :string id (alias :any c (object :string name :string email)))) :body (console:log id name email))");
  assertStringIncludes(result, "{id, c: {name, email}}");
  assertStringIncludes(result, "typeof id !== \"string\"");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "typeof email !== \"string\"");
});
Deno.test("func: nested object in array (positional)", () => {
  const result = compile("(func f :args ((array (object :string name) :number score)) :body (console:log name score))");
  assertStringIncludes(result, "[{name}, score]");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "typeof score !== \"number\"");
});
Deno.test("func: two levels deep nesting", () => {
  const result = compile("(func f :args ((object (alias :any a (object :string city (alias :any g (object :number lat :number lng)))))) :body (console:log city lat lng))");
  assertStringIncludes(result, "{a: {city, g: {lat, lng}}}");
  assertStringIncludes(result, "typeof city !== \"string\"");
  assertStringIncludes(result, "typeof lat !== \"number\"");
});
Deno.test("func: nested + default combined", () => {
  const result = compile("(func f :args ((object (default :string name \"anon\") (alias :any addr (object :string city)))) :body (console:log name city))");
  assertStringIncludes(result, "{name = \"anon\", addr: {city}}");
});
Deno.test("fn: nested destructuring", () => {
  const result = compile("(bind f (fn ((object (alias :any c (object :string name)))) (console:log name)))");
  assertStringIncludes(result, "{c: {name}}");
  assertStringIncludes(result, "typeof name !== \"string\"");
});
Deno.test("func: error on nested without alias in object", () => assertThrows(() => compile("(func f :args ((object (object :string name))) :body 1)"), Error, "must use alias"));
Deno.test("func: error on alias missing inner pattern", () => assertThrows(() => compile("(func f :args ((object (alias :any name))) :body 1)"), Error, "requires"));
Deno.test("func: object destructuring with default", () => {
  const result = compile("(func f :args ((object :string name (default :number age 0))) :body (console:log name age))");
  assertStringIncludes(result, "{name, age = 0}");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "typeof age !== \"number\"");
});
Deno.test("func: object destructuring with multiple defaults", () => {
  const result = compile("(func f :args ((object (default :string name \"anon\") (default :number age 0))) :body (console:log name age))");
  assertStringIncludes(result, "{name = \"anon\", age = 0}");
});
Deno.test("func: mixed default + non-default fields", () => {
  const result = compile("(func f :args ((object :string name (default :number age 0) :string email)) :body 1)");
  assertStringIncludes(result, "{name, age = 0, email}");
});
Deno.test("func: default with :any — no type check", () => {
  const result = compile("(func f :args ((object (default :any name \"anon\") :number age)) :body (console:log name age))");
  assertStringIncludes(result, "{name = \"anon\", age}");
});
Deno.test("func: array destructuring with default", () => {
  const result = compile("(func f :args ((array :number first (default :number second 0))) :body (+ first second))");
  assertStringIncludes(result, "[first, second = 0]");
});
Deno.test("func: default value is expression", () => {
  const result = compile("(func f :args ((object (default :number x (+ 1 2)))) :body x)");
  assertStringIncludes(result, "x = 1 + 2");
});
Deno.test("fn: with default in destructured", () => {
  const result = compile("(bind f (fn ((object :string name (default :number age 0))) (console:log name age)))");
  assertStringIncludes(result, "{name, age = 0}");
  assertStringIncludes(result, "typeof name !== \"string\"");
});
Deno.test("func: default + rest in array", () => {
  const result = compile("(func f :args ((array (default :number first 0) (rest :number others))) :body (console:log first others))");
  assertStringIncludes(result, "[first = 0, ...others]");
});
Deno.test("func: error on default missing value", () => assertThrows(() => compile("(func f :args ((object (default :number age))) :body 1)"), Error, "requires 3 arguments"));
Deno.test("func: error on default missing type", () => assertThrows(() => compile("(func f :args ((object (default age 0 1))) :body 1)"), Error, "must be a type keyword"));
Deno.test("func: error on rest not last in array", () => assertThrows(() => compile("(func f :args ((array (rest :number r) :number x)) :body 1)"), Error, "rest element must be last"));
Deno.test("func: top-level default param", () => {
  const result = compile("(func greet :args (:string name (default :string greeting \"Hello\")) :returns :string :body (template greeting \", \" name \"!\"))");
  assertStringIncludes(result, "greeting = \"Hello\"");
  assertStringIncludes(result, "typeof name !== \"string\"");
  assertStringIncludes(result, "typeof greeting !== \"string\"");
});
Deno.test("func: multiple top-level defaults", () => {
  const result = compile("(func f :args ((default :number x 0) (default :number y 0)) :body (+ x y))");
  assertStringIncludes(result, "x = 0");
  assertStringIncludes(result, "y = 0");
});
Deno.test("func: top-level rest param :any — no per-element check", () => {
  const result = compile("(func log-all :args (:string level (rest :any messages)) :body (console:log level messages))");
  assertStringIncludes(result, "...messages");
  assertStringIncludes(result, "typeof level !== \"string\"");
});
Deno.test("func: top-level rest param :number — per-element check", () => {
  const result = compile("(func sum :args ((rest :number nums)) :returns :number :body (nums:reduce (fn (:number acc :number n) (+ acc n)) 0))");
  assertStringIncludes(result, "...nums");
  assertStringIncludes(result, "for (const");
  assertStringIncludes(result, "of nums");
});
Deno.test("fn: top-level default", () => {
  const result = compile("(bind f (fn ((default :string name \"anon\")) (console:log name)))");
  assertStringIncludes(result, "name = \"anon\"");
  assertStringIncludes(result, "typeof name !== \"string\"");
});
Deno.test("fn: top-level rest :any", () => {
  const result = compile("(bind f (fn ((rest :any args)) (console:log args)))");
  assertStringIncludes(result, "...args");
});
Deno.test("fn: top-level rest :string — per-element check", () => {
  const result = compile("(bind f (fn ((rest :string items)) (console:log items)))");
  assertStringIncludes(result, "...items");
  assertStringIncludes(result, "for (const");
});
Deno.test("func: mixed destructured + default + rest", () => {
  const result = compile("(func handle :args ((object :string method :string url) (default :number timeout 5000) (rest :any middleware)) :body (console:log method url timeout middleware))");
  assertStringIncludes(result, "{method, url}");
  assertStringIncludes(result, "timeout = 5000");
  assertStringIncludes(result, "...middleware");
});
Deno.test("func: default with :any — no type check (top-level)", () => {
  const result = compile("(func f :args ((default :any name \"anon\")) :body (console:log name))");
  assertStringIncludes(result, "name = \"anon\"");
});
Deno.test("func: error on rest not last in :args", () => assertThrows(() => compile("(func f :args ((rest :any r) :string x) :body 1)"), Error, "rest parameter must be the last parameter"));
Deno.test("func: error on multiple rest in :args", () => assertThrows(() => compile("(func f :args ((rest :any a) (rest :any b)) :body 1)"), Error, "only one rest parameter allowed"));
Deno.test("func: error on default missing value (top-level)", () => assertThrows(() => compile("(func f :args ((default :number x)) :body 1)"), Error, "requires exactly 3 elements"));
