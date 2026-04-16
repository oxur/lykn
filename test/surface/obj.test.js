import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros } from "lang/expander.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  resetMacros();
  resetGensym();
  return compile(expand(read(source))).trim();
}

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

Deno.test("obj: simple object", () => {
  const result = lykn('(obj :name "Duncan" :age 42)');
  assertEquals(result, '({\n  name: "Duncan",\n  age: 42\n});');
});

Deno.test("obj: single property", () => {
  const result = lykn('(obj :active true)');
  assertEquals(result, "({\n  active: true\n});");
});

Deno.test("obj: kebab-case key", () => {
  const result = lykn('(obj :first-name "Duncan")');
  assertEquals(result, '({\n  firstName: "Duncan"\n});');
});

Deno.test("obj: computed value", () => {
  const result = lykn("(obj :score (* base multiplier))");
  assertEquals(result, "({\n  score: base * multiplier\n});");
});

Deno.test("obj: expansion produces object form", () => {
  const result = ex('(obj :name "Duncan")');
  const expanded = result[0];
  assertEquals(expanded.values[0].value, "object");
  assertEquals(expanded.values[1].type, "list");
  assertEquals(expanded.values[1].values[0].value, "name");
  assertEquals(expanded.values[1].values[1].value, "Duncan");
});

Deno.test("obj: empty object", () => {
  const result = lykn("(obj)");
  assertEquals(result, "({});");
});

Deno.test("obj: variable value", () => {
  const result = lykn("(obj :name user-name)");
  assertEquals(result, "({\n  name: userName\n});");
});
