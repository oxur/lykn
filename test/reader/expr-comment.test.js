import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";

Deno.test("#; discards next atom", () => {
  assertEquals(read("#;foo bar"), [{ type: 'atom', value: 'bar' }]);
});

Deno.test("#; discards next list", () => {
  assertEquals(read("#;(a b c) bar"), [{ type: 'atom', value: 'bar' }]);
});

Deno.test("#; inside list", () => {
  const result = read("(a #;b c)");
  assertEquals(result[0].values.length, 2);
  assertEquals(result[0].values[0].value, 'a');
  assertEquals(result[0].values[1].value, 'c');
});

Deno.test("#; before closing paren", () => {
  const result = read("(a #;b)");
  assertEquals(result[0].values.length, 1);
  assertEquals(result[0].values[0].value, 'a');
});

Deno.test("#; discards nested structure", () => {
  assertEquals(read("#;(a (b c) d) e"), [{ type: 'atom', value: 'e' }]);
});

Deno.test("#;#; double discard", () => {
  const result = read("(a #;#;b c d)");
  assertEquals(result[0].values.length, 2);
  assertEquals(result[0].values[0].value, 'a');
  assertEquals(result[0].values[1].value, 'd');
});

Deno.test("#; discards string", () => {
  assertEquals(read('#;"hello" world'), [{ type: 'atom', value: 'world' }]);
});

Deno.test("#; discards number", () => {
  assertEquals(read("#;42 true"), [{ type: 'atom', value: 'true' }]);
});

Deno.test("#; at end of input throws", () => {
  assertThrows(() => read("#;"), Error, "no form");
});

Deno.test("#; at end of list throws", () => {
  assertThrows(() => read("(a b #;)"), Error);
});

Deno.test("#; discards all top-level", () => {
  assertEquals(read("#;foo"), []);
});
