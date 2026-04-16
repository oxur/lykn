import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";

Deno.test("backtick: wraps atom in quasiquote", () => {
  assertEquals(read("`foo"), [
    { type: 'list', values: [{ type: 'atom', value: 'quasiquote' }, { type: 'atom', value: 'foo' }] }
  ]);
});

Deno.test("backtick: wraps list in quasiquote", () => {
  const result = read("`(a b)");
  assertEquals(result[0].type, 'list');
  assertEquals(result[0].values[0].value, 'quasiquote');
  assertEquals(result[0].values[1].type, 'list');
  assertEquals(result[0].values[1].values.length, 2);
});

Deno.test("backtick: wraps number", () => {
  const result = read("`42");
  assertEquals(result[0].values[0].value, 'quasiquote');
  assertEquals(result[0].values[1].value, 42);
});

Deno.test("backtick: wraps string", () => {
  const result = read('`"hello"');
  assertEquals(result[0].values[0].value, 'quasiquote');
  assertEquals(result[0].values[1].value, 'hello');
});

Deno.test("comma: wraps atom in unquote", () => {
  assertEquals(read(",x"), [
    { type: 'list', values: [{ type: 'atom', value: 'unquote' }, { type: 'atom', value: 'x' }] }
  ]);
});

Deno.test("comma-at: wraps atom in unquote-splicing", () => {
  assertEquals(read(",@xs"), [
    { type: 'list', values: [{ type: 'atom', value: 'unquote-splicing' }, { type: 'atom', value: 'xs' }] }
  ]);
});

Deno.test("quote: wraps atom in quote", () => {
  assertEquals(read("'foo"), [
    { type: 'list', values: [{ type: 'atom', value: 'quote' }, { type: 'atom', value: 'foo' }] }
  ]);
});

Deno.test("quote: wraps list in quote", () => {
  const result = read("'(a b)");
  assertEquals(result[0].values[0].value, 'quote');
  assertEquals(result[0].values[1].type, 'list');
});

Deno.test("backtick: nested unquote in list", () => {
  const result = read("`(a ,b)");
  const inner = result[0].values[1]; // the (a ,b) list
  assertEquals(inner.values[1].values[0].value, 'unquote');
  assertEquals(inner.values[1].values[1].value, 'b');
});

Deno.test("backtick: nested splice in list", () => {
  const result = read("`(a ,@b)");
  const inner = result[0].values[1];
  assertEquals(inner.values[1].values[0].value, 'unquote-splicing');
});

Deno.test("comma: inside list context", () => {
  const result = read("(a ,b c)");
  assertEquals(result[0].values[1].type, 'list');
  assertEquals(result[0].values[1].values[0].value, 'unquote');
});

Deno.test("comma-at: inside list context", () => {
  const result = read("(a ,@b c)");
  assertEquals(result[0].values[1].values[0].value, 'unquote-splicing');
});

Deno.test("backtick: multiple top-level", () => {
  const result = read("`a `b");
  assertEquals(result.length, 2);
  assertEquals(result[0].values[0].value, 'quasiquote');
  assertEquals(result[1].values[0].value, 'quasiquote');
});
