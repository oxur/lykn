import { assertEquals, assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { compile } from "lang/compiler.js";

function lykn(source) {
  return compile(read(source)).trim();
}

// Evaluate compiled JS to check runtime output
async function evalLykn(source) {
  const js = compile(read(source));
  const fn = new Function(`"use strict"; return (${js.trim().replace(/;$/, '')})`);
  return fn();
}

// --- ICU simple slots ---

Deno.test("template-icu: single slot", () => {
  const result = lykn('(template "Hello, {name}!" :name n)');
  assertEquals(result, '`Hello, ${n}!`;');
});

Deno.test("template-icu: multiple slots", () => {
  const result = lykn('(template "{a} and {b}" :a x :b y)');
  assertEquals(result, '`${x} and ${y}`;');
});

Deno.test("template-icu: multi-use of same slot", () => {
  const result = lykn('(template "{name} is {name}" :name n)');
  assertEquals(result, '`${n} is ${n}`;');
});

Deno.test("template-icu: slot at start", () => {
  const result = lykn('(template "{x} end" :x a)');
  assertEquals(result, '`${a} end`;');
});

Deno.test("template-icu: slot at end", () => {
  const result = lykn('(template "start {x}" :x a)');
  assertEquals(result, '`start ${a}`;');
});

Deno.test("template-icu: only slot", () => {
  const result = lykn('(template "{x}" :x a)');
  assertEquals(result, '`${a}`;');
});

Deno.test("template-icu: no slots in string → simple literal", () => {
  const result = lykn('(template "hello world")');
  assertEquals(result, '`hello world`;');
});

// --- ICU escape sequences ---

Deno.test("template-icu: escaped braces in ICU string", () => {
  const result = lykn("(template \"'{' literal '}'\")");
  assertEquals(result, '`{ literal }`;');
});

Deno.test("template-icu: escaped apostrophe", () => {
  const result = lykn("(template \"it''s fine\")");
  assertEquals(result, "`it's fine`;");
});

// --- ICU plural ---

Deno.test("template-icu: basic plural", () => {
  const result = lykn('(template "{n, plural, one {1 item} other {# items}}" :n count)');
  // Should contain an IIFE with conditional logic
  assertEquals(result.includes('count'), true);
  assertEquals(result.includes('==='), true);
  assertEquals(result.includes('1 item'), true);
  assertEquals(result.includes('items'), true);
});

Deno.test("template-icu: plural runtime one", async () => {
  const js = compile(read('(template "{n, plural, one {1 item} other {# items}}" :n n)'));
  const fn = new Function('n', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn(1), "1 item");
});

Deno.test("template-icu: plural with =0 branch", () => {
  const result = lykn('(template "{n, plural, =0 {none} one {1 item} other {# items}}" :n count)');
  assertEquals(result.includes('=== 0'), true);
  assertEquals(result.includes('none'), true);
});

Deno.test("template-icu: plural with surrounding text", () => {
  const result = lykn('(template "You have {n, plural, one {1 msg} other {# msgs}}." :n count)');
  assertEquals(result.includes('You have'), true);
  assertEquals(result.includes('.'), true);
});

// --- ICU select ---

Deno.test("template-icu: basic select", () => {
  const result = lykn('(template "{role, select, owner {You own it.} other {Read only.}}" :role r)');
  assertEquals(result.includes('r'), true);
  assertEquals(result.includes('owner'), true);
  assertEquals(result.includes('You own it.'), true);
  assertEquals(result.includes('Read only.'), true);
});

Deno.test("template-icu: select with three branches", () => {
  const result = lykn('(template "{r, select, a {A} b {B} other {O}}" :r x)');
  assertEquals(result.includes('"a"'), true);
  assertEquals(result.includes('"b"'), true);
});

Deno.test("template-icu: select with slot in branch", () => {
  const result = lykn('(template "{role, select, admin {Hi {name}} other {Hello}}" :role r :name n)');
  assertEquals(result.includes('n'), true);
  assertEquals(result.includes('Hi '), true);
});

// --- Nesting ---

Deno.test("template-icu: plural inside select", () => {
  const result = lykn(
    '(template "{role, select, owner {{n, plural, one {1 item} other {# items}}} other {N/A}}" :role r :n count)'
  );
  assertEquals(result.includes('owner'), true);
  assertEquals(result.includes('1 item'), true);
});

// --- Dispatch edge cases ---

Deno.test("template-icu: dispatch — expr only stays concat", () => {
  const result = lykn('(template name)');
  assertEquals(result, '`${name}`;');
});

Deno.test("template-icu: dispatch — string-expr-string stays concat", () => {
  const result = lykn('(template "Hi, " name "!")');
  assertEquals(result, '`Hi, ${name}!`;');
});

Deno.test("template-icu: dispatch — ICU mode matched", () => {
  const result = lykn('(template "Hi, {name}!" :name n)');
  assertEquals(result, '`Hi, ${n}!`;');
});

Deno.test("template-icu: dispatch — ambiguous form throws", () => {
  assertThrows(
    () => lykn('(template "Hi, " :name)'),
    Error,
    "ambiguous form",
  );
});

// --- Compile-time errors ---

Deno.test("template-icu: missing kwarg → error", () => {
  assertThrows(
    () => lykn('(template "Hello {missing}!")'),
    Error,
    "no binding for slot",
  );
});

Deno.test("template-icu: unused kwarg → error", () => {
  assertThrows(
    () => lykn('(template "Hello {name}!" :name n :extra v)'),
    Error,
    "unused keyword argument :extra",
  );
});

Deno.test("template-icu: duplicate kwarg → error", () => {
  assertThrows(
    () => lykn('(template "{a}" :a x :a y)'),
    Error,
    "duplicate keyword argument :a",
  );
});

Deno.test("template-icu: plural missing other → error", () => {
  assertThrows(
    () => lykn('(template "{n, plural, one {x}}" :n count)'),
    Error,
    "missing required",
  );
});

Deno.test("template-icu: unknown plural category → error", () => {
  assertThrows(
    () => lykn('(template "{n, plural, weird {x} other {y}}" :n count)'),
    Error,
    "unknown plural category",
  );
});

// --- Runtime equivalence ---

Deno.test("template-icu: runtime — simple slot", async () => {
  const js = compile(read('(template "Hello, {name}!" :name name)'));
  const fn = new Function('name', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn("Duncan"), "Hello, Duncan!");
});

Deno.test("template-icu: runtime — multi-use slot", async () => {
  const js = compile(read('(template "{name} is {name}" :name name)'));
  const fn = new Function('name', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn("Bob"), "Bob is Bob");
});

Deno.test("template-icu: runtime — plural one", async () => {
  const js = compile(read('(template "You have {n, plural, one {1 item} other {# items}}." :n n)'));
  const fn = new Function('n', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn(1), "You have 1 item.");
});

Deno.test("template-icu: runtime — plural other", async () => {
  const js = compile(read('(template "You have {n, plural, one {1 item} other {# items}}." :n n)'));
  const fn = new Function('n', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn(5), "You have 5 items.");
});

Deno.test("template-icu: runtime — plural =0", async () => {
  const js = compile(read('(template "{n, plural, =0 {none} one {1 item} other {# items}}" :n n)'));
  const fn = new Function('n', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn(0), "none");
  assertEquals(fn(1), "1 item");
  assertEquals(fn(5), "5 items");
});

Deno.test("template-icu: runtime — select", async () => {
  const js = compile(read('(template "{role, select, owner {You own it.} member {You are a member.} other {Guest.}}" :role role)'));
  const fn = new Function('role', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn("owner"), "You own it.");
  assertEquals(fn("member"), "You are a member.");
  assertEquals(fn("viewer"), "Guest.");
});

Deno.test("template-icu: runtime — select with slot in branch", async () => {
  const js = compile(read('(template "{role, select, admin {Hi {name}} other {Hello}}" :role role :name name)'));
  const fn = new Function('role', 'name', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn("admin", "Alice"), "Hi Alice");
  assertEquals(fn("guest", "Alice"), "Hello");
});

Deno.test("template-icu: runtime — composed example from DD-54", async () => {
  const source =
    '(template ' +
    '"{role, select, owner {Welcome back, {name}! You have {count, plural, =0 {no pending tasks} one {1 pending task} other {# pending tasks}}.} member {Hi {name}. You have {count, plural, =0 {no items to review} one {1 item to review} other {# items to review}}.} other {Hello, guest.}}" ' +
    ':role role :name name :count count)';
  const js = compile(read(source));
  const fn = new Function('role', 'name', 'count', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn("member", "Bob", 3), "Hi Bob. You have 3 items to review.");
  assertEquals(fn("owner", "Alice", 0), "Welcome back, Alice! You have no pending tasks.");
  assertEquals(fn("owner", "Alice", 1), "Welcome back, Alice! You have 1 pending task.");
  assertEquals(fn("viewer", "X", 99), "Hello, guest.");
});

// --- Tag interaction ---

Deno.test("template-icu: tag with concat mode still works", () => {
  const result = lykn('(tag html (template "<div>" content "</div>"))');
  assertEquals(result.includes('html`'), true);
  assertEquals(result.includes('${content}'), true);
});

// --- Review regression tests (DD-54 review 2026-05-13) ---

Deno.test("template-icu: nested plural same selector — no TDZ at runtime", async () => {
  const src = '(template "{n, plural, one {{n, plural, one {a} other {b}}} other {c}}" :n n)';
  const js = compile(read(src));
  const fn = new Function('n', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn(1), "a");
  assertEquals(fn(2), "c");
});

Deno.test("template-icu: select inside plural same selector — no TDZ", async () => {
  const src = '(template "{x, plural, one {one} other {{x, select, a {A} other {O}}}}" :x x)';
  const js = compile(read(src));
  const fn = new Function('x', `"use strict"; return ${js.trim().replace(/;$/, '')}`);
  assertEquals(fn(1), "one");
  assertEquals(fn("a"), "A");
  assertEquals(fn("b"), "O");
});

Deno.test("template-icu: multi-use kwarg evaluates expression once", async () => {
  const js = lykn('(template "{x}-{x}" :x (next-id))');
  const matches = js.match(/nextId\(\)/g) ?? [];
  assertEquals(matches.length, 1, `expected nextId() to appear once after hoisting, got ${matches.length}: ${js}`);
});

Deno.test("template-icu: simple identifier kwarg is NOT hoisted (no IIFE)", () => {
  const js = lykn('(template "{x}-{x}" :x x)');
  assertEquals(js, '`${x}-${x}`;');
});

Deno.test("template-icu: select branches use captured selector value", () => {
  const js = lykn('(template "{role, select, owner {Owner: {role}} other {Guest: {role}}}" :role (lookup-role))');
  const matches = js.match(/lookupRole\(\)/g) ?? [];
  assertEquals(matches.length, 1, `expected lookupRole() called once; got ${matches.length}: ${js}`);
});

Deno.test("template-icu: =1 + one overlap → error", () => {
  assertThrows(
    () => lykn('(template "{n, plural, =1 {a} one {b} other {c}}" :n n)'),
    Error,
    "overlapping branches",
  );
});

Deno.test("template-icu: one then =1 (reverse order) → error", () => {
  assertThrows(
    () => lykn('(template "{n, plural, one {b} =1 {a} other {c}}" :n n)'),
    Error,
    "overlapping branches",
  );
});

Deno.test("template-icu: 'zero' category → error in English Phase A", () => {
  assertThrows(
    () => lykn('(template "{n, plural, zero {no} one {1} other {many}}" :n n)'),
    Error,
    "not valid under English plural rules",
  );
});

Deno.test("template-icu: 'two' category → error in English Phase A", () => {
  assertThrows(
    () => lykn('(template "{n, plural, two {pair} other {many}}" :n n)'),
    Error,
    "not valid under English plural rules",
  );
});

Deno.test("template-icu: 'few' category → error in English Phase A", () => {
  assertThrows(
    () => lykn('(template "{n, plural, few {a few} other {many}}" :n n)'),
    Error,
    "not valid under English plural rules",
  );
});

Deno.test("template-icu: 'many' category → error in English Phase A", () => {
  assertThrows(
    () => lykn('(template "{n, plural, many {lots} other {some}}" :n n)'),
    Error,
    "not valid under English plural rules",
  );
});

Deno.test("template-icu: error message has single 'template:' prefix", () => {
  try {
    lykn('(template "{a, plural, weird {x} other {y}}" :a 1)');
    throw new Error("should have thrown");
  } catch (e) {
    const matches = e.message.match(/template:/g) ?? [];
    assertEquals(matches.length, 0, `IcuParseError should not have 'template:' prefix; message: ${e.message}`);
  }
});
