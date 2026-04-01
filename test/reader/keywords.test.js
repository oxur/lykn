import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";

Deno.test("keyword: simple keyword", () => {
  const result = read(":name");
  assertEquals(result.length, 1);
  assertEquals(result[0], { type: "keyword", value: "name" });
});

Deno.test("keyword: kebab-case keyword", () => {
  const result = read(":first-name");
  assertEquals(result.length, 1);
  assertEquals(result[0], { type: "keyword", value: "first-name" });
});

Deno.test("keyword: keyword in list", () => {
  const result = read("(:name :age)");
  assertEquals(result[0].type, "list");
  assertEquals(result[0].values[0], { type: "keyword", value: "name" });
  assertEquals(result[0].values[1], { type: "keyword", value: "age" });
});

Deno.test("keyword: keyword with value", () => {
  const result = read('(:name "Duncan")');
  assertEquals(result[0].values[0], { type: "keyword", value: "name" });
  assertEquals(result[0].values[1], { type: "string", value: "Duncan" });
});

Deno.test("keyword: bare colon is atom", () => {
  const result = read(":");
  assertEquals(result[0], { type: "atom", value: ":" });
});

Deno.test("keyword: multiple keywords in obj form", () => {
  const result = read('(obj :name "Duncan" :age 42)');
  const list = result[0];
  assertEquals(list.values[0], { type: "atom", value: "obj" });
  assertEquals(list.values[1], { type: "keyword", value: "name" });
  assertEquals(list.values[2], { type: "string", value: "Duncan" });
  assertEquals(list.values[3], { type: "keyword", value: "age" });
  assertEquals(list.values[4], { type: "number", value: 42 });
});
