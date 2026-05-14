import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { parseIcu, collectSlotNames, IcuParseError } from "lang/icu-parser.js";

// --- Simple slots ---

Deno.test("icu: plain literal, no slots", () => {
  assertEquals(parseIcu("hello world"), [
    { type: "literal", value: "hello world" },
  ]);
});

Deno.test("icu: single slot", () => {
  assertEquals(parseIcu("{name}"), [
    { type: "slot", name: "name" },
  ]);
});

Deno.test("icu: slot surrounded by text", () => {
  assertEquals(parseIcu("Hello, {name}!"), [
    { type: "literal", value: "Hello, " },
    { type: "slot", name: "name" },
    { type: "literal", value: "!" },
  ]);
});

Deno.test("icu: multiple different slots", () => {
  assertEquals(parseIcu("{a} and {b}"), [
    { type: "slot", name: "a" },
    { type: "literal", value: " and " },
    { type: "slot", name: "b" },
  ]);
});

Deno.test("icu: same slot used multiple times", () => {
  assertEquals(parseIcu("{name} is {name}"), [
    { type: "slot", name: "name" },
    { type: "literal", value: " is " },
    { type: "slot", name: "name" },
  ]);
});

Deno.test("icu: slot at start", () => {
  assertEquals(parseIcu("{x} end"), [
    { type: "slot", name: "x" },
    { type: "literal", value: " end" },
  ]);
});

Deno.test("icu: slot at end", () => {
  assertEquals(parseIcu("start {x}"), [
    { type: "literal", value: "start " },
    { type: "slot", name: "x" },
  ]);
});

Deno.test("icu: only slot, no text", () => {
  assertEquals(parseIcu("{x}"), [
    { type: "slot", name: "x" },
  ]);
});

Deno.test("icu: empty string", () => {
  assertEquals(parseIcu(""), []);
});

Deno.test("icu: hyphenated slot name", () => {
  assertEquals(parseIcu("{first-name}"), [
    { type: "slot", name: "first-name" },
  ]);
});

Deno.test("icu: underscore slot name", () => {
  assertEquals(parseIcu("{user_id}"), [
    { type: "slot", name: "user_id" },
  ]);
});

// --- Escape sequences ---

Deno.test("icu: escaped left brace", () => {
  assertEquals(parseIcu("a '{' b"), [
    { type: "literal", value: "a { b" },
  ]);
});

Deno.test("icu: escaped right brace", () => {
  assertEquals(parseIcu("a '}' b"), [
    { type: "literal", value: "a } b" },
  ]);
});

Deno.test("icu: escaped apostrophe", () => {
  assertEquals(parseIcu("it''s"), [
    { type: "literal", value: "it's" },
  ]);
});

Deno.test("icu: lone apostrophe is literal", () => {
  assertEquals(parseIcu("it's fine"), [
    { type: "literal", value: "it's fine" },
  ]);
});

Deno.test("icu: escaped braces and slot together", () => {
  const result = parseIcu("'{' {name} '}'");
  assertEquals(result, [
    { type: "literal", value: "{ " },
    { type: "slot", name: "name" },
    { type: "literal", value: " }" },
  ]);
});

// --- Plural ---

Deno.test("icu: basic plural one/other", () => {
  const result = parseIcu("{count, plural, one {1 item} other {# items}}");
  assertEquals(result.length, 1);
  assertEquals(result[0].type, "plural");
  assertEquals(result[0].name, "count");
  assertEquals(result[0].branches.length, 2);
  assertEquals(result[0].branches[0].key, "one");
  assertEquals(result[0].branches[0].body, [{ type: "literal", value: "1 item" }]);
  assertEquals(result[0].branches[1].key, "other");
  assertEquals(result[0].branches[1].body, [
    { type: "slot", name: "count" },
    { type: "literal", value: " items" },
  ]);
});

Deno.test("icu: plural with explicit =0 branch", () => {
  const result = parseIcu("{n, plural, =0 {none} one {one} other {many}}");
  assertEquals(result[0].branches[0].key, "=0");
  assertEquals(result[0].branches[0].body, [{ type: "literal", value: "none" }]);
});

Deno.test("icu: plural with # shorthand resolves to slot", () => {
  const result = parseIcu("{n, plural, one {# thing} other {# things}}");
  assertEquals(result[0].branches[0].body, [
    { type: "slot", name: "n" },
    { type: "literal", value: " thing" },
  ]);
  assertEquals(result[0].branches[1].body, [
    { type: "slot", name: "n" },
    { type: "literal", value: " things" },
  ]);
});

Deno.test("icu: plural with slot inside branch", () => {
  const result = parseIcu("{n, plural, one {1 {unit}} other {# {unit}s}}");
  assertEquals(result[0].branches[0].body, [
    { type: "literal", value: "1 " },
    { type: "slot", name: "unit" },
  ]);
  assertEquals(result[0].branches[1].body, [
    { type: "slot", name: "n" },
    { type: "literal", value: " " },
    { type: "slot", name: "unit" },
    { type: "literal", value: "s" },
  ]);
});

Deno.test("icu: plural with text around it", () => {
  const result = parseIcu("You have {n, plural, one {1 msg} other {# msgs}}.");
  assertEquals(result.length, 3);
  assertEquals(result[0], { type: "literal", value: "You have " });
  assertEquals(result[1].type, "plural");
  assertEquals(result[2], { type: "literal", value: "." });
});

Deno.test("icu: plural missing other branch → error", () => {
  assertThrows(
    () => parseIcu("{n, plural, one {x}}"),
    IcuParseError,
    "missing required 'other' branch",
  );
});

Deno.test("icu: plural unknown category → error", () => {
  assertThrows(
    () => parseIcu("{n, plural, weird {x} other {y}}"),
    IcuParseError,
    "unknown plural category 'weird'",
  );
});

// --- Select ---

Deno.test("icu: basic select", () => {
  const result = parseIcu("{role, select, owner {You own it.} other {Read only.}}");
  assertEquals(result.length, 1);
  assertEquals(result[0].type, "select");
  assertEquals(result[0].name, "role");
  assertEquals(result[0].branches.length, 2);
  assertEquals(result[0].branches[0].key, "owner");
  assertEquals(result[0].branches[0].body, [{ type: "literal", value: "You own it." }]);
  assertEquals(result[0].branches[1].key, "other");
});

Deno.test("icu: select with three branches", () => {
  const result = parseIcu(
    "{role, select, owner {Own} member {Mem} other {Guest}}"
  );
  assertEquals(result[0].branches.length, 3);
  assertEquals(result[0].branches[0].key, "owner");
  assertEquals(result[0].branches[1].key, "member");
  assertEquals(result[0].branches[2].key, "other");
});

Deno.test("icu: select missing other branch → error", () => {
  assertThrows(
    () => parseIcu("{role, select, owner {x}}"),
    IcuParseError,
    "missing required 'other' branch",
  );
});

Deno.test("icu: select with slot in branch", () => {
  const result = parseIcu("{role, select, owner {Hi {name}} other {Hello}}");
  assertEquals(result[0].branches[0].body, [
    { type: "literal", value: "Hi " },
    { type: "slot", name: "name" },
  ]);
});

// --- Nesting ---

Deno.test("icu: plural inside select", () => {
  const result = parseIcu(
    "{role, select, owner {{n, plural, one {1 item} other {# items}}} other {N/A}}"
  );
  assertEquals(result[0].type, "select");
  const ownerBody = result[0].branches[0].body;
  assertEquals(ownerBody.length, 1);
  assertEquals(ownerBody[0].type, "plural");
  assertEquals(ownerBody[0].name, "n");
});

Deno.test("icu: select inside plural", () => {
  const result = parseIcu(
    "{n, plural, one {{role, select, admin {Admin task} other {Task}}} other {# tasks}}"
  );
  assertEquals(result[0].type, "plural");
  const oneBranch = result[0].branches[0].body;
  assertEquals(oneBranch.length, 1);
  assertEquals(oneBranch[0].type, "select");
});

// --- Error cases ---

Deno.test("icu: unclosed slot → error", () => {
  assertThrows(
    () => parseIcu("{name"),
    IcuParseError,
    "expected '}'",
  );
});

Deno.test("icu: empty slot name → error", () => {
  assertThrows(
    () => parseIcu("{}"),
    IcuParseError,
    "expected slot name",
  );
});

Deno.test("icu: unknown format type → error", () => {
  assertThrows(
    () => parseIcu("{n, number, short}"),
    IcuParseError,
    "unknown format type 'number'",
  );
});

// --- collectSlotNames ---

Deno.test("collectSlotNames: simple slots", () => {
  const nodes = parseIcu("Hello {name}, you have {count} items");
  const names = collectSlotNames(nodes);
  assertEquals(names, new Set(["name", "count"]));
});

Deno.test("collectSlotNames: deduplicates multi-use", () => {
  const nodes = parseIcu("{name} is {name}");
  const names = collectSlotNames(nodes);
  assertEquals(names, new Set(["name"]));
});

Deno.test("collectSlotNames: includes plural selector", () => {
  const nodes = parseIcu("{n, plural, one {1} other {#}}");
  const names = collectSlotNames(nodes);
  assertEquals(names, new Set(["n"]));
});

Deno.test("collectSlotNames: includes select selector", () => {
  const nodes = parseIcu("{role, select, admin {A} other {O}}");
  const names = collectSlotNames(nodes);
  assertEquals(names, new Set(["role"]));
});

Deno.test("collectSlotNames: slots inside branches", () => {
  const nodes = parseIcu("{role, select, admin {Hi {name}} other {Hello}}");
  const names = collectSlotNames(nodes);
  assertEquals(names, new Set(["role", "name"]));
});

Deno.test("collectSlotNames: nested plural+select", () => {
  const nodes = parseIcu(
    "{role, select, owner {{n, plural, one {1 {unit}} other {# {unit}s}}} other {N/A}}"
  );
  const names = collectSlotNames(nodes);
  assertEquals(names, new Set(["role", "n", "unit"]));
});

// --- Composed example from DD-54 ---

Deno.test("icu: marketing screenshot example parses", () => {
  const input =
    "{role, select, " +
    "owner {Welcome back, {name}! You have {count, plural, " +
    "=0 {no pending tasks} " +
    "one {1 pending task} " +
    "other {# pending tasks}}.} " +
    "member {Hi {name}. You have {count, plural, " +
    "=0 {no items to review} " +
    "one {1 item to review} " +
    "other {# items to review}}.} " +
    "other {Hello, guest.}}";

  const result = parseIcu(input);
  assertEquals(result.length, 1);
  assertEquals(result[0].type, "select");
  assertEquals(result[0].name, "role");
  assertEquals(result[0].branches.length, 3);

  const names = collectSlotNames(result);
  assertEquals(names, new Set(["role", "name", "count"]));
});
