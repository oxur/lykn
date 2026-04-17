import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("match: number literals", () => {
  const result = compile("(match status (200 \"ok\") (404 \"not found\") (_ \"unknown\"))");
  assertStringIncludes(result, "=== 200");
  assertStringIncludes(result, "=== 404");
  assertStringIncludes(result, "\"ok\"");
  assertStringIncludes(result, "\"not found\"");
  assertStringIncludes(result, "\"unknown\"");
});
Deno.test("match: string literals", () => {
  const result = compile("(match cmd (\"start\" 1) (\"stop\" 0) (_ -1))");
  assertStringIncludes(result, "=== \"start\"");
  assertStringIncludes(result, "=== \"stop\"");
});
Deno.test("match: boolean literals", () => {
  const result = compile("(match flag (true \"yes\") (false \"no\"))");
  assertStringIncludes(result, "=== true");
  assertStringIncludes(result, "=== false");
});
Deno.test("match: wildcard default", () => {
  const result = compile("(match x (1 \"one\") (_ \"other\"))");
  assertStringIncludes(result, "=== 1");
  assertStringIncludes(result, "\"other\"");
});
Deno.test("match: ADT constructor with field", () => {
  const result = compile("\n    (type Option (Some :any value) None)\n    (match opt\n      ((Some v) v)\n      (None 0))");
  assertStringIncludes(result, ".tag === \"Some\"");
  assertStringIncludes(result, ".tag === \"None\"");
  assertStringIncludes(result, ".value");
});
Deno.test("match: zero-field ADT constructor", () => {
  const result = compile("\n    (type Option (Some :any value) None)\n    (match opt\n      ((Some v) (use v))\n      (None (default-val)))");
  assertStringIncludes(result, ".tag === \"None\"");
});
Deno.test("match: structural obj pattern", () => {
  const result = compile("(match response\n    ((obj :ok true :data d) (process d))\n    (_ (handle-error)))");
  assertStringIncludes(result, "typeof");
  assertStringIncludes(result, "\"ok\" in");
  assertStringIncludes(result, "\"data\" in");
  assertStringIncludes(result, ".ok === true");
});
Deno.test("match: guarded pattern", () => {
  const result = compile("\n    (type Option (Some :any value) None)\n    (match opt\n      ((Some v) :when (> v 0) (use-positive v))\n      ((Some v) (use-other v))\n      (None 0))");
  assertStringIncludes(result, ".tag === \"Some\"");
  assertStringIncludes(result, "> 0");
});
Deno.test("match: always produces IIFE", () => {
  const result = compile("(match x (1 \"one\") (_ \"other\"))");
  assertStringIncludes(result, "(() =>");
  assertStringIncludes(result, "return");
});
Deno.test("match: throws on no match without wildcard", () => {
  const result = compile("(match x (1 \"one\") (2 \"two\"))");
  assertStringIncludes(result, "no matching pattern");
});
Deno.test("match: simple symbol binding", () => {
  const result = compile("(match x (y (+ y 1)))");
  assertStringIncludes(result, "const y =");
  assertStringIncludes(result, "y + 1");
});
