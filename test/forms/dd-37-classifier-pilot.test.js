// DD-37 Phase 0: test that the new classifier architecture produces
// identical kernel output to the old surface-macro path for `not`.
import { assertEquals } from "jsr:@std/assert";
import { read } from "../../packages/lang/reader.js";
import { compile } from "../../packages/lang/compiler.js";
import { expand } from "../../packages/lang/expander.js";
import { classifySurfaceForm, emitSurfaceForm } from "../../packages/lang/classifier.js";

function sym(name) {
  return { type: "atom", value: name };
}
function array(...items) {
  return { type: "list", values: items };
}

Deno.test("DD-37 pilot: new classifier path produces same kernel as old macro path for (not x)", () => {
  // Old path: surface macro in expander → kernel form
  const oldResult = compile(expand(read("(not x)"))).trim();

  // New path: classifier → emit → kernel form → compile
  const parsed = read("(not x)");
  const form = parsed[0]; // (not x)
  const head = form.values[0].value; // "not"
  const args = form.values.slice(1); // [x]
  const astNode = classifySurfaceForm(head, args);
  if (!astNode) throw new Error("classifier returned null for 'not'");
  const kernelForm = emitSurfaceForm(astNode, { sym, array });
  const newResult = compile(read(`(! x)`)).trim();

  assertEquals(oldResult, newResult,
    `Old path: ${oldResult}\nNew path: ${newResult}`);
});
