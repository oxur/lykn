import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { compile } from "../../src/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

Deno.test("colon: simple member access", () => {
  assertEquals(lykn("console:log"), "console.log;");
});

Deno.test("colon: method call", () => {
  assertEquals(lykn('(console:log "hi")'), 'console.log("hi");');
});

Deno.test("colon: three-segment chain", () => {
  assertEquals(lykn("process:argv:length"), "process.argv.length;");
});

Deno.test("colon: this member access", () => {
  assertEquals(lykn("this:name"), "this.name;");
});

Deno.test("colon: this with camelCase", () => {
  assertEquals(lykn("this:my-name"), "this.myName;");
});

Deno.test("colon: super member access", () => {
  assertEquals(lykn("super:constructor"), "super.constructor;");
});

Deno.test("colon: camelCase each segment independently", () => {
  assertEquals(lykn("my-obj:get-value"), "myObj.getValue;");
});

Deno.test("colon: Math:PI no camelCase effect", () => {
  assertEquals(lykn("Math:PI"), "Math.PI;");
});

Deno.test("colon: bare this", () => {
  assertEquals(lykn("(return this)"), "return this;");
});

Deno.test("colon: bare super", () => {
  assertEquals(lykn("super"), "super;");
});

// Keywords (leading colon)
Deno.test("colon: keyword compiles to string literal", () => {
  assertEquals(lykn(":foo"), '"foo";');
});

Deno.test("colon: keyword with kebab-case compiles to camelCase string", () => {
  assertEquals(lykn(":first-name"), '"firstName";');
});

Deno.test("colon: trailing colon throws", () => {
  assertThrows(() => lykn("foo:"), Error);
});

Deno.test("colon: numeric segment throws", () => {
  assertThrows(() => lykn("obj:0"), Error, "get");
});

Deno.test("colon: consecutive colons throws", () => {
  assertThrows(() => lykn("foo::bar"), Error, "Empty segment");
});

Deno.test("colon: bare colon throws", () => {
  assertThrows(() => lykn(":"), Error);
});
