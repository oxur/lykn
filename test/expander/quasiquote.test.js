import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expandExpr, expand } from "lykn/expander.js";

function ex(source) {
  return expandExpr(read(source)[0]);
}

// --- Atom/self-evaluating cases ---

Deno.test("qq: atom", () => {
  const result = ex("`foo");
  // Should produce (quote foo) which expansion preserves
  assertEquals(result.type, "list");
  assertEquals(result.values[0].value, "quote");
  assertEquals(result.values[1].value, "foo");
});

Deno.test("qq: number self-evaluating", () => {
  const result = ex("`42");
  assertEquals(result.type, "number");
  assertEquals(result.value, 42);
});

Deno.test("qq: string self-evaluating", () => {
  const result = ex('`"hello"');
  assertEquals(result.type, "string");
  assertEquals(result.value, "hello");
});

Deno.test("qq: empty list", () => {
  const result = ex("`()");
  assertEquals(result.type, "list");
  assertEquals(result.values.length, 0);
});

// --- DD-10 examples (optimized output) ---

Deno.test("qq: DD-10 ex1 — no unquotes (all literal)", () => {
  // `(if true (console:log "yes"))
  // All literal → quoteLiteral produces (array (quote if) (quote true) ...)
  const result = ex('`(if true (console:log "yes"))');
  assertEquals(result.type, "list");
  // Head is 'array' (from quoteLiteral optimization)
  assertEquals(result.values[0].value, "array");
  // (quote if)
  assertEquals(result.values[1].values[0].value, "quote");
  assertEquals(result.values[1].values[1].value, "if");
});

Deno.test("qq: DD-10 ex2 — unquote, no splice", () => {
  // `(if ,test (console:log "yes"))
  // No splices → (array (quote if) test (array (quote console:log) (quote "yes")))
  const result = ex("`(if ,test (console:log \"yes\"))");
  assertEquals(result.values[0].value, "array");
  // Second element: (quote if)
  assertEquals(result.values[1].values[0].value, "quote");
  assertEquals(result.values[1].values[1].value, "if");
  // Third element: test (the unquoted variable)
  assertEquals(result.values[2].value, "test");
});

Deno.test("qq: DD-10 ex3 — has splice", () => {
  // `(if ,test ,@body)
  // Has splice → (append (array (quote if)) (array test) body)
  const result = ex("`(if ,test ,@body)");
  assertEquals(result.values[0].value, "append");
});

Deno.test("qq: all unquoted, no splice", () => {
  // `(,a ,b ,c) → (array a b c) (optimized)
  const result = ex("`(,a ,b ,c)");
  assertEquals(result.values[0].value, "array");
  assertEquals(result.values[1].value, "a");
  assertEquals(result.values[2].value, "b");
  assertEquals(result.values[3].value, "c");
});

Deno.test("qq: splice at start", () => {
  // `(,@xs y) → (append xs (array (quote y)))
  const result = ex("`(,@xs y)");
  assertEquals(result.values[0].value, "append");
});

Deno.test("qq: splice at middle", () => {
  // `(a ,@xs b) → (append (array (quote a)) xs (array (quote b)))
  const result = ex("`(a ,@xs b)");
  assertEquals(result.values[0].value, "append");
});

Deno.test("qq: multiple splices", () => {
  // `(,@xs ,@ys) → (append xs ys)
  const result = ex("`(,@xs ,@ys)");
  assertEquals(result.values[0].value, "append");
  assertEquals(result.values[1].value, "xs");
  assertEquals(result.values[2].value, "ys");
});

Deno.test("qq: colon syntax preserved", () => {
  // `(console:log ,x) → (array (quote console:log) x)
  const result = ex("`(console:log ,x)");
  assertEquals(result.values[0].value, "array");
  // (quote console:log)
  assertEquals(result.values[1].values[1].value, "console:log");
  // x
  assertEquals(result.values[2].value, "x");
});

// --- Error cases ---

Deno.test("qq: unquote outside quasiquote throws", () => {
  assertThrows(() => ex(",foo"), Error, "unquote outside");
});

Deno.test("qq: splice outside quasiquote throws", () => {
  assertThrows(() => ex(",@foo"), Error, "unquote-splicing outside");
});

Deno.test("qq: splice not in list throws", () => {
  // `,@foo — quasiquote wrapping a splice directly (not in a list)
  assertThrows(() => ex("`,@foo"), Error, "not inside a list");
});

// --- Nested quasiquote ---

Deno.test("qq: nested quasiquote preserves structure", () => {
  // ``(a ,,b) — nested quasiquote
  const result = ex("``(a ,,b)");
  // Should produce structure containing (quote quasiquote)
  assertEquals(result.values[0].value, "array");
});
