import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, resetGensym, resetMacros } from "lykn/expander.js";
import { compile } from "lykn/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

// --- js:call ---

Deno.test("js:call: method dispatch", () => {
  assertEquals(lykn('(js:call console:log "hello")'), 'console.log("hello");');
});

Deno.test("js:call: with multiple args", () => {
  assertEquals(lykn("(js:call Math:max 1 2 3)"), "Math.max(1, 2, 3);");
});

Deno.test("js:call: strips js:call head", () => {
  const result = lykn("(js:call arr:push 42)");
  assertEquals(result, "arr.push(42);");
});

// --- js:bind ---

Deno.test("js:bind: method binding", () => {
  assertEquals(lykn("(js:bind obj:method obj)"), "obj.method.bind(obj);");
});

Deno.test("js:bind: nested method", () => {
  assertEquals(lykn("(js:bind el:addEventListener el)"), "el.addEventListener.bind(el);");
});

// --- js:eval ---

Deno.test("js:eval: simple eval", () => {
  assertEquals(lykn('(js:eval "1 + 2")'), 'eval("1 + 2");');
});

Deno.test("js:eval: with variable", () => {
  assertEquals(lykn("(js:eval code)"), "eval(code);");
});

// --- js:eq ---

Deno.test("js:eq: loose equality", () => {
  assertEquals(lykn("(js:eq a b)"), "a == b;");
});

Deno.test("js:eq: with null", () => {
  assertEquals(lykn("(js:eq x null)"), "x == null;");
});

// --- js:typeof ---

Deno.test("js:typeof: typeof operator", () => {
  assertEquals(lykn("(js:typeof x)"), "typeof x;");
});

// --- Nested in bind ---

Deno.test("js:eq: nested in bind", () => {
  assertEquals(lykn("(bind is-nil (js:eq x null))"), "const isNil = x == null;");
});

Deno.test("js:typeof: nested in bind", () => {
  assertEquals(lykn("(bind t (js:typeof x))"), "const t = typeof x;");
});

// --- Error cases ---

Deno.test("js:call: no args throws", () => {
  assertThrows(() => lykn("(js:call)"), Error, "at least a method");
});

Deno.test("js:bind: wrong arg count throws", () => {
  assertThrows(() => lykn("(js:bind obj:method)"), Error, "2 arguments");
});

Deno.test("js:eq: wrong arg count throws", () => {
  assertThrows(() => lykn("(js:eq a)"), Error, "2 arguments");
});
