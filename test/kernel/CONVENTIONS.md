# Kernel Test Conventions

Tests in `test/kernel/` exercise kernel-form semantics directly using
`.lyk` files. The test runner validates these files with kernel-only
classification — surface forms at the top level are rejected.

## File naming

- One test file per kernel form: `<form>_test.lyk`
- Example: `function_test.lyk`, `const_test.lyk`

## Test structure

Kernel test files use the testing DSL via `import-macros`:

```lyk
(import-macros "./packages/testing" (test is-equal includes))
(import "testing/helpers.js" ((alias compile-kernel compile)))

(test "const: basic declaration"
  (const result (compile "(const x 42)"))
  (includes result "const x = 42"))
```

After macro expansion, all top-level forms are kernel-compatible:
`import` (kernel form), expanded `test` → `Deno.test(...)` (function
call). The `const` bindings inside test bodies are inside arrow
function arguments, not at the classifier's top level.

## compileBoth usage

Use `compile-both` for kernel forms where both the JS and Rust
compilers implement them. Use single-compiler tests (`compile` or
`compile-kernel`) where only one compiler handles the form.

## Flavor (b) passthrough coverage

Flavor (b) passthroughs get a representative sample, not exhaustive
coverage. The sample covers one form per category:

- Arithmetic: `+`
- Control flow: `while`
- Module: `import`
- Async: `await`
- Literal constructor: `array`
- Comparison: `===`

Remaining passthrough coverage can expand in follow-up milestones.
