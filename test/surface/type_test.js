import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("type: single constructor with field", () => {
  const result = compile("(type Wrapper (Wrap :any value))");
  assertStringIncludes(result, "function Wrap(value)");
  assertStringIncludes(result, "tag: \"Wrap\"");
  assertStringIncludes(result, "return");
});
Deno.test("type: zero-field constructor", () => {
  const result = compile("(type Unit Empty)");
  assertStringIncludes(result, "const Empty = {\n  tag: \"Empty\"\n}");
});
Deno.test("type: Option with Some and None", () => {
  const result = compile("(type Option (Some :any value) None)");
  assertStringIncludes(result, "function Some(value)");
  assertStringIncludes(result, "tag: \"Some\"");
  assertStringIncludes(result, "tag: \"None\"");
  assertStringIncludes(result, "const None");
});
Deno.test("type: typed fields emit type checks", () => {
  const result = compile("(type Point (Pt :number x :number y))");
  assertStringIncludes(result, "typeof x !== \"number\"");
  assertStringIncludes(result, "typeof y !== \"number\"");
});
Deno.test("type: :any fields skip type checks", () => {
  const result = compile("(type Box (Box :any value))");
  assertStringIncludes(result, "function Box(value)");
  assertStringIncludes(result, "tag: \"Box\"");
});
Deno.test("type: multi-field constructor", () => {
  const result = compile("(type Pair (Pair :any first :any second))");
  assertStringIncludes(result, "function Pair(first, second)");
  assertStringIncludes(result, "first: first");
  assertStringIncludes(result, "second: second");
});
Deno.test("type: Result with Ok and Err", () => {
  const result = compile("(type Result (Ok :any value) (Err :any error))");
  assertStringIncludes(result, "function Ok(value)");
  assertStringIncludes(result, "function Err(error)");
  assertStringIncludes(result, "tag: \"Ok\"");
  assertStringIncludes(result, "tag: \"Err\"");
});
