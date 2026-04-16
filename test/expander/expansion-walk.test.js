import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expand, expandExpr } from "lykn/expander.js";

function ex(source) {
  return expand(read(source));
}

function first(source) {
  return ex(source)[0];
}

Deno.test("walk: atom passes through", () => {
  assertEquals(first("x"), { type: "atom", value: "x" });
});

Deno.test("walk: number passes through", () => {
  assertEquals(first("42"), { type: "number", value: 42 });
});

Deno.test("walk: string passes through", () => {
  assertEquals(first('"hello"'), { type: "string", value: "hello" });
});

Deno.test("walk: empty list passes through", () => {
  assertEquals(first("()").type, "list");
  assertEquals(first("()").values.length, 0);
});

Deno.test("walk: unknown head expands sub-forms", () => {
  const result = first("(if (car x) (cdr y))");
  assertEquals(result.values[0].value, "if");
  assertEquals(result.values[1].values[0].value, "get");
  assertEquals(result.values[2].values[0].value, "get");
});

Deno.test("walk: nested sugar desugared", () => {
  const result = first("(const z (car (cons a b)))");
  assertEquals(result.values[0].value, "const");
  // (get (array a b) 0)
  assertEquals(result.values[2].values[0].value, "get");
  assertEquals(result.values[2].values[1].values[0].value, "array");
});

Deno.test("walk: macro form is consumed in pass 1", () => {
  // Macros are now processed in Pass 1 and erased, not errored
  const result = ex("(macro noop () \`null) (const x 1)");
  assertEquals(result.length, 1);
  assertEquals(result[0].values[0].value, "const");
});

Deno.test("walk: macroexpand erased from output", () => {
  const result = ex("(macroexpand '(+ 1 2)) (const x 1)");
  assertEquals(result.length, 1);
  assertEquals(result[0].values[0].value, "const");
});

Deno.test("walk: quote stops recursion", () => {
  const result = first("'(car x)");
  // car should NOT be desugared
  assertEquals(result.values[1].values[0].value, "car");
});

Deno.test("walk: deeply nested expansion", () => {
  const result = first("(if (car (cdr (cons a b))) c)");
  // (car (cdr (cons a b)))
  // → (get (get (array a b) 1) 0)
  assertEquals(result.values[1].values[0].value, "get");
  assertEquals(result.values[1].values[1].values[0].value, "get");
  assertEquals(result.values[1].values[1].values[1].values[0].value, "array");
});

Deno.test("walk: cons node expands recursively", () => {
  const result = expandExpr(read("((car x) . (cdr y))")[0]);
  assertEquals(result.type, "cons");
  assertEquals(result.car.values[0].value, "get");
  assertEquals(result.cdr.values[0].value, "get");
});

Deno.test("walk: expand top-level array of forms", () => {
  const result = ex("(const x (car y)) (const z (cdr w))");
  assertEquals(result.length, 2);
  assertEquals(result[0].values[2].values[0].value, "get");
  assertEquals(result[1].values[2].values[0].value, "get");
});
