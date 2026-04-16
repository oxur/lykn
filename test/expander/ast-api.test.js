import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import {
  sym, gensym, resetGensym, array, append,
  isArray, isSymbol, isNumber, isString,
  first, rest, concat, length, nth,
} from "lang/expander.js";

Deno.test("sym: creates atom node", () => {
  assertEquals(sym("foo"), { type: "atom", value: "foo" });
});

Deno.test("sym: preserves colon syntax", () => {
  assertEquals(sym("console:log"), { type: "atom", value: "console:log" });
});

Deno.test("gensym: creates unique symbols", () => {
  resetGensym();
  const a = gensym();
  const b = gensym();
  assertEquals(a.type, "atom");
  assertEquals(a.value, "g__gensym0");
  assertEquals(b.value, "g__gensym1");
});

Deno.test("gensym: uses custom prefix", () => {
  resetGensym();
  const s = gensym("tmp");
  assertEquals(s.value, "tmp__gensym0");
});

Deno.test("array: empty", () => {
  assertEquals(array(), { type: "list", values: [] });
});

Deno.test("array: single element", () => {
  assertEquals(array(sym("x")), { type: "list", values: [{ type: "atom", value: "x" }] });
});

Deno.test("array: multiple elements", () => {
  const result = array(sym("a"), sym("b"), sym("c"));
  assertEquals(result.values.length, 3);
});

Deno.test("append: two lists", () => {
  const result = append(array(sym("a")), array(sym("b")));
  assertEquals(result.values.length, 2);
  assertEquals(result.values[0].value, "a");
  assertEquals(result.values[1].value, "b");
});

Deno.test("append: empty lists", () => {
  const result = append(array(), array(sym("a")));
  assertEquals(result.values.length, 1);
});

Deno.test("append: throws on non-list", () => {
  assertThrows(() => append(sym("x"), array()), Error, "expected list");
});

Deno.test("isArray: true for list node", () => {
  assertEquals(isArray(array()), true);
});

Deno.test("isArray: false for atom", () => {
  assertEquals(isArray(sym("x")), false);
});

Deno.test("isArray: false for null", () => {
  assertEquals(isArray(null), false);
});

Deno.test("isSymbol: true for atom", () => {
  assertEquals(isSymbol(sym("x")), true);
});

Deno.test("isSymbol: false for number", () => {
  assertEquals(isSymbol({ type: "number", value: 42 }), false);
});

Deno.test("isNumber: true for number", () => {
  assertEquals(isNumber({ type: "number", value: 42 }), true);
});

Deno.test("isString: true for string", () => {
  assertEquals(isString({ type: "string", value: "hi" }), true);
});

Deno.test("first: of non-empty list", () => {
  assertEquals(first(array(sym("a"), sym("b"))), sym("a"));
});

Deno.test("first: of empty list", () => {
  assertEquals(first(array()), undefined);
});

Deno.test("rest: of list", () => {
  const result = rest(array(sym("a"), sym("b"), sym("c")));
  assertEquals(result.values.length, 2);
  assertEquals(result.values[0].value, "b");
});

Deno.test("rest: of single-element list", () => {
  assertEquals(rest(array(sym("a"))).values.length, 0);
});

Deno.test("concat: alias for append", () => {
  const result = concat(array(sym("a")), array(sym("b")));
  assertEquals(result.values.length, 2);
});

Deno.test("length: of list", () => {
  assertEquals(length(array(sym("a"), sym("b"))), 2);
});

Deno.test("length: of empty list", () => {
  assertEquals(length(array()), 0);
});

Deno.test("nth: valid index", () => {
  assertEquals(nth(array(sym("a"), sym("b")), 1), sym("b"));
});

Deno.test("nth: out of bounds", () => {
  assertEquals(nth(array(sym("a")), 5), undefined);
});
