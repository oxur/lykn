import {assert, assertEquals, assertNotEquals, assertStrictEquals, assertExists, assertThrows, assertRejects, assertMatch, assertStringIncludes, assertArrayIncludes, assertObjectMatch} from "jsr:@std/assert";
import {lykn as compile} from "../../packages/lang/mod.js";
Deno.test("if-let: simple binding with else", () => {
  const result = compile("(if-let (user (find-user id)) (greet user) (console:log \"not found\"))");
  assertStringIncludes(result, "!= null");
  assertStringIncludes(result, "const user");
  assertStringIncludes(result, "greet(user)");
  assertStringIncludes(result, "not found");
});
Deno.test("if-let: simple binding without else", () => {
  const result = compile("(if-let (user (find-user id)) (greet user))");
  assertStringIncludes(result, "!= null");
  assertStringIncludes(result, "greet(user)");
});
Deno.test("if-let: ADT constructor pattern", () => {
  const result = compile("(type Option (Some :any value) None)\n    (if-let ((Some v) (find-user id)) (greet v) (console:log \"none\"))");
  assertStringIncludes(result, ".tag === \"Some\"");
  assertStringIncludes(result, ".value");
  assertStringIncludes(result, "greet(v)");
});
Deno.test("if-let: structural obj pattern", () => {
  const result = compile("(if-let ((obj :name n :age a) data) (console:log n a) (console:log \"bad\"))");
  assertStringIncludes(result, "\"name\" in");
  assertStringIncludes(result, "\"age\" in");
  assertStringIncludes(result, "const n");
  assertStringIncludes(result, "const a");
});
Deno.test("if-let: always produces IIFE", () => {
  const result = compile("(if-let (x (get-val)) x 0)");
  assertStringIncludes(result, "(() =>");
  assertStringIncludes(result, "return");
});
Deno.test("when-let: simple binding", () => {
  const result = compile("(when-let (user (find-user id)) (greet user))");
  assertStringIncludes(result, "!= null");
  assertStringIncludes(result, "greet(user)");
});
Deno.test("when-let: ADT pattern", () => {
  const result = compile("(type Option (Some :any value) None)\n    (when-let ((Some v) (find-user id)) (greet v))");
  assertStringIncludes(result, ".tag === \"Some\"");
  assertStringIncludes(result, "greet(v)");
});
Deno.test("when-let: multiple body expressions", () => {
  const result = compile("(when-let (user (find-user id)) (console:log user) (greet user))");
  assertStringIncludes(result, "console.log(user)");
  assertStringIncludes(result, "greet(user)");
});
Deno.test("when-let: produces IIFE", () => {
  const result = compile("(when-let (x (get-val)) x)");
  assertStringIncludes(result, "(() =>");
});
