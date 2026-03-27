import { assertThrows } from "https://deno.land/std/assert/mod.ts";
import { read } from "../../src/reader.js";
import { expand, resetGensym, resetMacros } from "../../src/expander.js";

function ex(source) {
  resetMacros();
  resetGensym();
  return expand(read(source));
}

Deno.test("macro error: duplicate definition throws", () => {
  assertThrows(() => ex(`
    (macro foo () \`null)
    (macro foo () \`null)
  `), Error, "duplicate");
});

Deno.test("macro error: macro form in pass 2 throws", () => {
  // A macro that expands to a macro definition should error
  // (This tests the register-macro sentinel in the dispatch table)
  assertThrows(() => ex(`
    (macro bad () \`(macro inner () \`null))
    (bad)
  `), Error);
});

Deno.test("macro error: circular dependency throws", () => {
  // Two macros that reference each other in their bodies
  assertThrows(() => ex(`
    (macro a () \`(b))
    (macro b () \`(a))
    (a)
  `), Error, "circular");
});
