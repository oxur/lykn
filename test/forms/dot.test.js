import { assertEquals } from "jsr:@std/assert";
import { compile } from "lang/compiler.js";

// Helper: build AST nodes directly (the . form is kernel-only, not readable from source)
const atom = (v) => ({ type: "atom", value: v });
const str = (v) => ({ type: "string", value: v });
const num = (v) => ({ type: "number", value: v });
const list = (...vals) => ({ type: "list", values: vals });

function compileDot(...args) {
  return compile([list(atom("."), ...args)]).trim();
}

Deno.test(".: method call with no args", () => {
  const result = compileDot(str("hello"), atom("to-upper-case"));
  assertEquals(result, '("hello").toUpperCase();');
});

Deno.test(".: method call with args", () => {
  const result = compileDot(atom("arr"), atom("slice"), num(0), num(10));
  assertEquals(result, "arr.slice(0, 10);");
});

Deno.test(".: method call with one arg", () => {
  const result = compileDot(atom("arr"), atom("push"), num(42));
  assertEquals(result, "arr.push(42);");
});

Deno.test(".: camelCase conversion on method name", () => {
  const result = compileDot(atom("el"), atom("get-element-by-id"), str("output"));
  assertEquals(result, 'el.getElementById("output");');
});

Deno.test(".: nested object as receiver", () => {
  const result = compileDot(
    list(atom("get"), atom("user"), str("name")),
    atom("to-upper-case"),
  );
  assertEquals(result, 'user["name"].toUpperCase();');
});

Deno.test(".: chained dot calls", () => {
  const inner = list(atom("."), str("hello"), atom("to-upper-case"));
  const result = compile([list(atom("."), inner, atom("slice"), num(0), num(3))]).trim();
  assertEquals(result, '("hello").toUpperCase().slice(0, 3);');
});
