import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("js:call: method dispatch", () => {
  const r__gensym0 = compile("(js:call console:log \"hello\")");
  assertEquals(r__gensym0.trim(), "console.log(\"hello\");");
});
Deno.test("js:call: with multiple args", () => {
  const r__gensym1 = compile("(js:call Math:max 1 2 3)");
  assertEquals(r__gensym1.trim(), "Math.max(1, 2, 3);");
});
Deno.test("js:call: strips js:call head", () => {
  const r__gensym2 = compile("(js:call arr:push 42)");
  assertEquals(r__gensym2.trim(), "arr.push(42);");
});
Deno.test("js:bind: method binding", () => {
  const r__gensym3 = compile("(js:bind obj:method obj)");
  assertEquals(r__gensym3.trim(), "obj.method.bind(obj);");
});
Deno.test("js:bind: nested method", () => {
  const r__gensym4 = compile("(js:bind el:addEventListener el)");
  assertEquals(r__gensym4.trim(), "el.addEventListener.bind(el);");
});
Deno.test("js:eval: simple eval", () => {
  const r__gensym5 = compile("(js:eval \"1 + 2\")");
  assertEquals(r__gensym5.trim(), "eval(\"1 + 2\");");
});
Deno.test("js:eval: with variable", () => {
  const r__gensym6 = compile("(js:eval code)");
  assertEquals(r__gensym6.trim(), "eval(code);");
});
Deno.test("js:eq: loose equality", () => {
  const r__gensym7 = compile("(js:eq a b)");
  assertEquals(r__gensym7.trim(), "a == b;");
});
Deno.test("js:eq: with null", () => {
  const r__gensym8 = compile("(js:eq x null)");
  assertEquals(r__gensym8.trim(), "x == null;");
});
Deno.test("js:typeof: typeof operator", () => {
  const r__gensym9 = compile("(js:typeof x)");
  assertEquals(r__gensym9.trim(), "typeof x;");
});
Deno.test("js:eq: nested in bind", () => {
  const r__gensym10 = compile("(bind is-nil (js:eq x null))");
  assertEquals(r__gensym10.trim(), "const isNil = x == null;");
});
Deno.test("js:typeof: nested in bind", () => {
  const r__gensym11 = compile("(bind t (js:typeof x))");
  assertEquals(r__gensym11.trim(), "const t = typeof x;");
});
Deno.test("js:call: no args throws", () => assertThrows(() => compile("(js:call)"), Error, "at least a method"));
Deno.test("js:bind: wrong arg count throws", () => assertThrows(() => compile("(js:bind obj:method)"), Error, "2 arguments"));
Deno.test("js:eq: wrong arg count throws", () => assertThrows(() => compile("(js:eq a)"), Error, "2 arguments"));
