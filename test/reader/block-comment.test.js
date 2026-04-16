import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lykn/reader.js";

Deno.test("#|...|# discards content", () => {
  assertEquals(read("#| comment |# foo"), [{ type: 'atom', value: 'foo' }]);
});

Deno.test("#|...|# nested", () => {
  assertEquals(read("#| outer #| inner |# still |# foo"), [{ type: 'atom', value: 'foo' }]);
});

Deno.test("#|...|# empty", () => {
  assertEquals(read("#||# foo"), [{ type: 'atom', value: 'foo' }]);
});

Deno.test("#|...|# preserves surrounding", () => {
  const result = read("a #| comment |# b");
  assertEquals(result.length, 2);
  assertEquals(result[0].value, 'a');
  assertEquals(result[1].value, 'b');
});

Deno.test("#|...|# inside list", () => {
  const result = read("(a #| x |# b)");
  assertEquals(result[0].values.length, 2);
});

Deno.test("#|...|# multiline", () => {
  assertEquals(read("#|\nline1\nline2\n|# foo"), [{ type: 'atom', value: 'foo' }]);
});

Deno.test("#|...|# with code inside", () => {
  assertEquals(read('#|(console:log "hi")|# foo'), [{ type: 'atom', value: 'foo' }]);
});

Deno.test("unterminated block comment throws", () => {
  assertThrows(() => read("#| unclosed"), Error, "unterminated");
});

Deno.test("nested unterminated throws", () => {
  assertThrows(() => read("#| #| inner |#"), Error, "unterminated");
});

Deno.test("deeply nested block comment", () => {
  assertEquals(read("#| #| #| deep |# |# |# foo"), [{ type: 'atom', value: 'foo' }]);
});
