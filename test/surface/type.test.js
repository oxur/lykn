import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

Deno.test("type: single constructor with field", () => {
  const result = lykn("(type Wrapper (Wrap :any value))");
  assertEquals(result.includes("function Wrap(value)"), true);
  assertEquals(result.includes('tag: "Wrap"'), true);
  assertEquals(result.includes("return"), true);
});

Deno.test("type: zero-field constructor", () => {
  const result = lykn("(type Unit Empty)");
  assertEquals(result.includes('const Empty = {\n  tag: "Empty"\n}'), true);
});

Deno.test("type: Option with Some and None", () => {
  const result = lykn("(type Option (Some :any value) None)");
  assertEquals(result.includes("function Some(value)"), true);
  assertEquals(result.includes('tag: "Some"'), true);
  assertEquals(result.includes('tag: "None"'), true);
  assertEquals(result.includes("const None"), true);
});

Deno.test("type: typed fields emit type checks", () => {
  const result = lykn("(type Point (Pt :number x :number y))");
  assertEquals(result.includes("typeof x !== \"number\""), true);
  assertEquals(result.includes("typeof y !== \"number\""), true);
});

Deno.test("type: :any fields skip type checks", () => {
  const result = lykn("(type Box (Box :any value))");
  assertEquals(result.includes("typeof"), false);
});

Deno.test("type: multi-field constructor", () => {
  const result = lykn("(type Pair (Pair :any first :any second))");
  assertEquals(result.includes("function Pair(first, second)"), true);
  assertEquals(result.includes("first: first"), true);
  assertEquals(result.includes("second: second"), true);
});

Deno.test("type: Result with Ok and Err", () => {
  const result = lykn("(type Result (Ok :any value) (Err :any error))");
  assertEquals(result.includes("function Ok(value)"), true);
  assertEquals(result.includes("function Err(error)"), true);
  assertEquals(result.includes('tag: "Ok"'), true);
  assertEquals(result.includes('tag: "Err"'), true);
});
