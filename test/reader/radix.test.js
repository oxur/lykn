import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";

Deno.test("#2r binary", () => {
  assertEquals(read("#2r11111111")[0], { type: 'number', value: 255, base: 2 });
});

Deno.test("#8r octal", () => {
  assertEquals(read("#8r377")[0], { type: 'number', value: 255, base: 8 });
});

Deno.test("#16r hex lowercase", () => {
  assertEquals(read("#16rff")[0], { type: 'number', value: 255, base: 16 });
});

Deno.test("#16r hex uppercase", () => {
  assertEquals(read("#16rFF")[0], { type: 'number', value: 255, base: 16 });
});

Deno.test("#16r hex mixed case", () => {
  assertEquals(read("#16rFf")[0], { type: 'number', value: 255, base: 16 });
});

Deno.test("#3r base 3", () => {
  assertEquals(read("#3r201")[0], { type: 'number', value: 19, base: 3 });
});

Deno.test("#36r base 36", () => {
  assertEquals(read("#36rzz")[0], { type: 'number', value: 1295, base: 36 });
});

Deno.test("#2r single bit", () => {
  assertEquals(read("#2r1")[0], { type: 'number', value: 1, base: 2 });
});

Deno.test("#10r decimal", () => {
  assertEquals(read("#10r42")[0], { type: 'number', value: 42, base: 10 });
});

Deno.test("#16r zero", () => {
  assertEquals(read("#16r0")[0], { type: 'number', value: 0, base: 16 });
});

Deno.test("radix inside list", () => {
  const result = read("(const x #16rff)");
  assertEquals(result[0].values[2].value, 255);
});

Deno.test("radix followed by paren", () => {
  const result = read("(#2r101)");
  assertEquals(result[0].values[0].value, 5);
});

Deno.test("#0r base too low throws", () => {
  assertThrows(() => read("#0r10"), Error, "2-36");
});

Deno.test("#1r base too low throws", () => {
  assertThrows(() => read("#1r0"), Error, "2-36");
});

Deno.test("#37r base too high throws", () => {
  assertThrows(() => read("#37r10"), Error, "2-36");
});

Deno.test("#100r base too high throws", () => {
  assertThrows(() => read("#100r10"), Error, "2-36");
});

Deno.test("#2r invalid digit throws", () => {
  assertThrows(() => read("#2r29"), Error, "not a valid digit");
});

Deno.test("#8r invalid digit throws", () => {
  assertThrows(() => read("#8r89"), Error, "not a valid digit");
});

Deno.test("#16r invalid digit throws", () => {
  assertThrows(() => read("#16rGG"), Error, "not a valid digit");
});

Deno.test("#16r no value throws", () => {
  assertThrows(() => read("#16r"), Error, "missing value");
});

Deno.test("#2r no value throws", () => {
  assertThrows(() => read("#2r"), Error, "missing value");
});

Deno.test("#r without base is unknown dispatch", () => {
  assertThrows(() => read("#r10"), Error, "unknown dispatch");
});
