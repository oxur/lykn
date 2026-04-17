import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("=: binary strict equality", () => {
  const r__gensym0 = compile("(= a b)");
  assertEquals(r__gensym0.trim(), "a === b;");
});
Deno.test("=: variadic pairwise chain", () => {
  const r__gensym1 = compile("(= a b c)");
  assertEquals(r__gensym1.trim(), "a === b && b === c;");
});
Deno.test("=: four-way pairwise chain", () => {
  const r__gensym2 = compile("(= a b c d)");
  assertEquals(r__gensym2.trim(), "a === b && b === c && c === d;");
});
Deno.test("=: with literals", () => {
  const r__gensym3 = compile("(= x 42)");
  assertEquals(r__gensym3.trim(), "x === 42;");
});
Deno.test("=: with null", () => {
  const r__gensym4 = compile("(= x null)");
  assertEquals(r__gensym4.trim(), "x === null;");
});
Deno.test("=: with expressions", () => {
  const r__gensym5 = compile("(= (+ a 1) (+ b 2))");
  assertEquals(r__gensym5.trim(), "a + 1 === b + 2;");
});
Deno.test("=: error on single arg", () => assertThrows(() => compile("(= x)"), Error));
Deno.test("!=: binary strict inequality", () => {
  const r__gensym6 = compile("(!= a b)");
  assertEquals(r__gensym6.trim(), "a !== b;");
});
Deno.test("!=: with literals", () => {
  const r__gensym7 = compile("(!= x \"hello\")");
  assertEquals(r__gensym7.trim(), "x !== \"hello\";");
});
Deno.test("!=: error on wrong arity", () => assertThrows(() => compile("(!= a)"), Error));
Deno.test("and: binary", () => {
  const r__gensym8 = compile("(and x y)");
  assertEquals(r__gensym8.trim(), "x && y;");
});
Deno.test("and: variadic", () => {
  const r__gensym9 = compile("(and a b c d)");
  assertEquals(r__gensym9.trim(), "a && b && c && d;");
});
Deno.test("and: with expressions", () => {
  const r__gensym10 = compile("(and (> x 0) (< x 10))");
  assertEquals(r__gensym10.trim(), "x > 0 && x < 10;");
});
Deno.test("and: error on single arg", () => assertThrows(() => compile("(and x)"), Error));
Deno.test("or: binary", () => {
  const r__gensym11 = compile("(or x y)");
  assertEquals(r__gensym11.trim(), "x || y;");
});
Deno.test("or: variadic", () => {
  const r__gensym12 = compile("(or a b c d)");
  assertEquals(r__gensym12.trim(), "a || b || c || d;");
});
Deno.test("or: with expressions", () => {
  const r__gensym13 = compile("(or (= x 0) (= x 1))");
  assertEquals(r__gensym13.trim(), "x === 0 || x === 1;");
});
Deno.test("or: error on single arg", () => assertThrows(() => compile("(or x)"), Error));
Deno.test("not: unary", () => {
  const r__gensym14 = compile("(not x)");
  assertEquals(r__gensym14.trim(), "!x;");
});
Deno.test("not: double negation", () => {
  const r__gensym15 = compile("(not (not x))");
  assertEquals(r__gensym15.trim(), "!!x;");
});
Deno.test("not: with expression", () => {
  const r__gensym16 = compile("(not (= a b))");
  assertEquals(r__gensym16.trim(), "!(a === b);");
});
Deno.test("not: error on multiple args", () => assertThrows(() => compile("(not x y)"), Error));
Deno.test("regression: reset! still emits assignment", () => {
  const r__gensym17 = compile("(bind c (cell 0))\n(reset! c 42)");
  assertEquals(r__gensym17.trim(), "const c = {\n  value: 0\n};\nc.value = 42;");
});
Deno.test("regression: swap! still emits assignment", () => {
  const r__gensym18 = compile("(bind c (cell 0))\n(swap! c f)");
  assertEquals(r__gensym18.trim(), "const c = {\n  value: 0\n};\nc.value = f(c.value);");
});
Deno.test("=: inside bind", () => {
  const r__gensym19 = compile("(bind result (= a b))");
  assertEquals(r__gensym19.trim(), "const result = a === b;");
});
Deno.test("and: inside bind", () => {
  const r__gensym20 = compile("(bind result (and x y))");
  assertEquals(r__gensym20.trim(), "const result = x && y;");
});
Deno.test("or: inside if", () => {
  const r__gensym21 = compile("(if (or a b) (console:log 1))");
  assertEquals(r__gensym21.trim(), "if (a || b) console.log(1);");
});
Deno.test("=: inside match", () => {
  const r__gensym22 = compile("(bind result (= 1 1))");
  assertEquals(r__gensym22.trim(), "const result = 1 === 1;");
});
