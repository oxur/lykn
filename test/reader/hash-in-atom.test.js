import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";

Deno.test("# mid-atom is not dispatch", () => {
  assertEquals(read("temp#gen"), [{ type: 'atom', value: 'temp#gen' }]);
});

Deno.test("# mid-atom in list", () => {
  const result = read("(foo temp#gen)");
  assertEquals(result[0].values[1].value, 'temp#gen');
});

Deno.test("# at end of atom", () => {
  assertEquals(read("foo#"), [{ type: 'atom', value: 'foo#' }]);
});

Deno.test("multiple # in atom", () => {
  assertEquals(read("a#b#c"), [{ type: 'atom', value: 'a#b#c' }]);
});

Deno.test("# at start of token triggers dispatch", () => {
  assertEquals(read("#;foo bar"), [{ type: 'atom', value: 'bar' }]);
});
