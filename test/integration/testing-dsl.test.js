import { assertEquals } from "https://deno.land/std/assert/mod.ts";
import { read } from "lang/reader.js";
import { expand, resetGensym, resetMacros, resetModuleCache } from "lang/expander.js";
import { compile } from "lang/compiler.js";
import { resolve, dirname } from "node:path";
import { fromFileUrl } from "https://deno.land/std/path/mod.ts";

const testingDir = resolve(dirname(fromFileUrl(import.meta.url)), "../../packages/testing");

function lykn(source) {
  resetMacros(); resetGensym(); resetModuleCache();
  return compile(expand(read(source), { filePath: resolve(testingDir, "consumer_test.lykn") })).trim();
}

const importAll = `(import-macros "./mod.lykn"
  (test test-async suite step
   is is-equal is-not-equal is-strict-equal
   ok is-thrown is-thrown-async
   matches includes has obj-matches
   test-compiles))`;

// --- runtime import injection ---

Deno.test("testing: runtime import emitted for @std/assert", () => {
  const result = lykn(`${importAll}
    (test "x" (is true))`);
  assertEquals(result.includes('import {'), true);
  assertEquals(result.includes('jsr:@std/assert'), true);
  assertEquals(result.includes('assertEquals'), true);
  assertEquals(result.includes('assert,'), true);
});

// --- assertion macros ---

Deno.test("testing: is → assert", () => {
  const result = lykn(`${importAll}
    (test "x" (is true))`);
  assertEquals(result.includes('assert(true)'), true);
});

Deno.test("testing: is-equal → assertEquals", () => {
  const result = lykn(`${importAll}
    (test "x" (is-equal (+ 1 2) 3))`);
  assertEquals(result.includes('assertEquals(1 + 2, 3)'), true);
});

Deno.test("testing: is-not-equal → assertNotEquals", () => {
  const result = lykn(`${importAll}
    (test "x" (is-not-equal a b))`);
  assertEquals(result.includes('assertNotEquals(a, b)'), true);
});

Deno.test("testing: is-strict-equal → assertStrictEquals", () => {
  const result = lykn(`${importAll}
    (test "x" (is-strict-equal a b))`);
  assertEquals(result.includes('assertStrictEquals(a, b)'), true);
});

Deno.test("testing: ok → assertExists", () => {
  const result = lykn(`${importAll}
    (test "x" (ok value))`);
  assertEquals(result.includes('assertExists(value)'), true);
});

Deno.test("testing: is-thrown → assertThrows", () => {
  const result = lykn(`${importAll}
    (test "x" (is-thrown (bad-call)))`);
  assertEquals(result.includes('assertThrows(() => badCall())'), true);
});

Deno.test("testing: is-thrown with error type", () => {
  const result = lykn(`${importAll}
    (test "x" (is-thrown (validate nil) TypeError))`);
  assertEquals(result.includes('assertThrows('), true);
  assertEquals(result.includes('TypeError'), true);
});

Deno.test("testing: is-thrown with error type and message", () => {
  const result = lykn(`${importAll}
    (test "x" (is-thrown (parse "{{") SyntaxError "unexpected"))`);
  assertEquals(result.includes('assertThrows('), true);
  assertEquals(result.includes('SyntaxError'), true);
  assertEquals(result.includes('"unexpected"'), true);
});

Deno.test("testing: is-thrown-async → assertRejects", () => {
  const result = lykn(`${importAll}
    (test-async "x" (is-thrown-async (fetch-data "bad")))`);
  assertEquals(result.includes('assertRejects('), true);
  assertEquals(result.includes('async'), true);
});

Deno.test("testing: matches → assertMatch", () => {
  const result = lykn(`${importAll}
    (test "x" (matches str pattern))`);
  assertEquals(result.includes('assertMatch(str, pattern)'), true);
});

Deno.test("testing: includes → assertStringIncludes", () => {
  const result = lykn(`${importAll}
    (test "x" (includes str "hello"))`);
  assertEquals(result.includes('assertStringIncludes(str, "hello")'), true);
});

Deno.test("testing: has → assertArrayIncludes", () => {
  const result = lykn(`${importAll}
    (test "x" (has arr items))`);
  assertEquals(result.includes('assertArrayIncludes(arr, items)'), true);
});

Deno.test("testing: obj-matches → assertObjectMatch", () => {
  const result = lykn(`${importAll}
    (test "x" (obj-matches actual expected))`);
  assertEquals(result.includes('assertObjectMatch(actual, expected)'), true);
});

// --- test form ---

Deno.test("testing: simple test → Deno.test", () => {
  const result = lykn(`${importAll}
    (test "addition works"
      (is-equal (+ 1 2) 3)
      (is-equal (* 3 4) 12))`);
  assertEquals(result.includes('Deno.test("addition works"'), true);
  assertEquals(result.includes('assertEquals(1 + 2, 3)'), true);
  assertEquals(result.includes('assertEquals(3 * 4, 12)'), true);
});

Deno.test("testing: test is sync when no await", () => {
  const result = lykn(`${importAll}
    (test "sync" (is true))`);
  assertEquals(result.includes('async'), false);
});

Deno.test("testing: test auto-detects await → async", () => {
  const result = lykn(`${importAll}
    (test "async auto"
      (const result (await (fetch-data)))
      (is-equal result 42))`);
  assertEquals(result.includes('async () =>'), true);
});

Deno.test("testing: test-async always async", () => {
  const result = lykn(`${importAll}
    (test-async "explicit async"
      (is-equal 1 1))`);
  assertEquals(result.includes('async () =>'), true);
});

// --- keyword clauses ---

Deno.test("testing: test with :setup", () => {
  const result = lykn(`${importAll}
    (test "with setup"
      :setup (const db (create-db))
      :body (is-equal db 42))`);
  assertEquals(result.includes('createDb()'), true);
  assertEquals(result.includes('assertEquals(db, 42)'), true);
});

Deno.test("testing: test with :setup and :teardown → try/finally", () => {
  const result = lykn(`${importAll}
    (test "with teardown"
      :setup (const db (create-db))
      :teardown (close db)
      :body (is-equal (query db) 1))`);
  assertEquals(result.includes('try'), true);
  assertEquals(result.includes('finally'), true);
  assertEquals(result.includes('close(db)'), true);
});

Deno.test("testing: test with :teardown only → try/finally", () => {
  const result = lykn(`${importAll}
    (test "teardown only"
      :teardown (cleanup)
      :body (is-equal 1 1))`);
  assertEquals(result.includes('try'), true);
  assertEquals(result.includes('finally'), true);
  assertEquals(result.includes('cleanup()'), true);
});

// --- suite and step ---

Deno.test("testing: suite → Deno.test with t param", () => {
  const result = lykn(`${importAll}
    (suite "math"
      (test "add" (is-equal (+ 1 2) 3))
      (test "mul" (is-equal (* 2 3) 6)))`);
  assertEquals(result.includes('Deno.test("math"'), true);
  assertEquals(result.includes('async t =>'), true);
  assertEquals(result.includes('await t.step("add"'), true);
  assertEquals(result.includes('await t.step("mul"'), true);
});

Deno.test("testing: step → await t.step", () => {
  const result = lykn(`${importAll}
    (test "workflow"
      (step "create" (is-equal 1 1))
      (step "delete" (is-equal 2 2)))`);
  assertEquals(result.includes('await t.step("create"'), true);
  assertEquals(result.includes('await t.step("delete"'), true);
  assertEquals(result.includes('async t =>'), true);
});

Deno.test("testing: step auto-detects async in body", () => {
  const result = lykn(`${importAll}
    (test "async steps"
      (step "fetch" (const r (await (fetch-data))) (is-equal r 1)))`);
  assertEquals(result.includes('await t.step("fetch", async () =>'), true);
});

Deno.test("testing: suite with setup/teardown", () => {
  const result = lykn(`${importAll}
    (suite "db tests"
      :setup (const db (create-db))
      :teardown (close db)
      (test "query" (is-equal (query db) 1)))`);
  assertEquals(result.includes('try'), true);
  assertEquals(result.includes('finally'), true);
  assertEquals(result.includes('close(db)'), true);
});

// --- test-compiles ---

Deno.test("testing: test-compiles expands to compile + assertEquals with trim", () => {
  const result = lykn(`${importAll}
    (test-compiles "bind" "(bind x 1)" "const x = 1;")`);
  assertEquals(result.includes('Deno.test("bind"'), true);
  assertEquals(result.includes('compile("(bind x 1)")'), true);
  assertEquals(result.includes('.trim()'), true);
  assertEquals(result.includes('"const x = 1;"'), true);
});

// --- edge cases ---

Deno.test("testing: empty test body → empty function", () => {
  const result = lykn(`${importAll}
    (test "placeholder")`);
  assertEquals(result.includes('Deno.test("placeholder"'), true);
});

Deno.test("testing: test with step and direct assertions coexist", () => {
  const result = lykn(`${importAll}
    (test "mixed"
      (is-equal 1 1)
      (step "sub" (is-equal 2 2)))`);
  assertEquals(result.includes('assertEquals(1, 1)'), true);
  assertEquals(result.includes('await t.step("sub"'), true);
});
