import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";
import { expandExpr } from "lykn/expander.js";

function ex(source) {
  return expandExpr(read(source)[0]);
}

Deno.test("quote: atom preserved", () => {
  const result = ex("'foo");
  assertEquals(result.type, "list");
  assertEquals(result.values[0].value, "quote");
  assertEquals(result.values[1].value, "foo");
});

Deno.test("quote: list preserved", () => {
  const result = ex("'(a b c)");
  assertEquals(result.values[0].value, "quote");
  assertEquals(result.values[1].type, "list");
});

Deno.test("quote: nested quote preserved", () => {
  const result = ex("'(quote x)");
  assertEquals(result.values[0].value, "quote");
  assertEquals(result.values[1].values[0].value, "quote");
});

Deno.test("quote: number preserved", () => {
  const result = ex("'42");
  assertEquals(result.values[0].value, "quote");
  assertEquals(result.values[1].value, 42);
});

Deno.test("quote: sugar inside quote NOT expanded", () => {
  const result = ex("'(car x)");
  // car should NOT be desugared — quote stops recursion
  assertEquals(result.values[1].values[0].value, "car");
});
