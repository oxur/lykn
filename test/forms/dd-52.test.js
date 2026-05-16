import { assertEquals, assertStringIncludes } from "jsr:@std/assert";
import { lykn } from "../../packages/lang/mod.js";

const CWD = Deno.cwd();

function compileRust(source) {
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, source);
    return compileRustFile(tmpPath);
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
}

function compileRustFile(path) {
  const lyknBin = Deno.env.get("LYKN_BIN") || "./bin/lykn";
  const proc = new Deno.Command(lyknBin, {
    args: ["compile", path],
    stdout: "piped",
    stderr: "piped",
  }).outputSync();
  if (!proc.success) {
    const stderr = new TextDecoder().decode(proc.stderr);
    throw new Error(stderr.trim());
  }
  let rustOut = new TextDecoder().decode(proc.stdout).trim();
  return rustOut.split("\n")
    .filter((l) => !l.includes(": warning:") && !l.startsWith("  suggestion:"))
    .join("\n").trim();
}

function tryCompileRustFile(path) {
  const lyknBin = Deno.env.get("LYKN_BIN") || "./bin/lykn";
  const proc = new Deno.Command(lyknBin, {
    args: ["compile", path],
    stdout: "piped",
    stderr: "piped",
  }).outputSync();
  return {
    success: proc.success,
    stderr: new TextDecoder().decode(proc.stderr).trim(),
    stdout: new TextDecoder().decode(proc.stdout).trim(),
  };
}

function compileJS(source, { cwd } = {}) {
  if (cwd) {
    const tmpPath = Deno.makeTempFileSync({ dir: cwd, suffix: ".lykn" });
    try {
      Deno.writeTextFileSync(tmpPath, source);
      return lykn(Deno.readTextFileSync(tmpPath)).trim();
    } finally {
      try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
    }
  }
  return lykn(source).trim();
}

// ── DD-52: surface-macros via Rust expander ──────────────────────────

// Happy path: bundled-impl fixture (use absolute path to avoid temp-dir issues)
Deno.test("DD-52 (Rust): bundled-impl surface-macros loads and registers macro", () => {
  const src = `(import-macros "${CWD}/test/regression/surface-macros/bundled-impl" (greet))\n(greet "world")`;
  const r = compileRust(src);
  assertStringIncludes(r, "console.log");
  assertStringIncludes(r, "Hello");
});

Deno.test("DD-52 (JS): bundled-impl surface-macros loads and registers macro", () => {
  const src = `(import-macros "./test/regression/surface-macros/bundled-impl/mod.lykn" (greet))\n(greet "world")`;
  const r = compileJS(src);
  assertStringIncludes(r, "console.log");
  assertStringIncludes(r, "Hello");
});

// Empty-impl: no macros registered
Deno.test("DD-52 (Rust): empty-impl surface-macros succeeds with no registered names", () => {
  const src = `(import-macros "${CWD}/test/regression/surface-macros/empty-impl" ())\n(console:log "ok")`;
  const r = compileRust(src);
  assertStringIncludes(r, "console.log");
});

// Throwing-impl: fail-fast on JS error (compile the test source that imports from throwing-impl)
Deno.test("DD-52 (Rust): throwing-impl surface-macros fails fast", () => {
  const src = `(import-macros "${CWD}/test/regression/surface-macros/throwing-impl" ())`;
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, src);
    const result = tryCompileRustFile(tmpPath);
    assertEquals(result.success, false);
    assertStringIncludes(result.stderr, "surface-macros");
    assertStringIncludes(result.stderr, "intentional error");
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
});

// Missing-file: file not found error
Deno.test("DD-52 (Rust): missing-file surface-macros reports file not found", () => {
  const src = `(import-macros "${CWD}/test/regression/surface-macros/missing-file" ())`;
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, src);
    const result = tryCompileRustFile(tmpPath);
    assertEquals(result.success, false);
    assertStringIncludes(result.stderr, "file not found");
    assertStringIncludes(result.stderr, "nonexistent.js");
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
});

// ── Real-world: @lykn/testing surface macros ─────────────────────────

Deno.test("DD-52 (Rust): @lykn/testing surface macros expand correctly", () => {
  const src = `(import-macros "../packages/testing" (test is-equal ok))\n(test "basic" (is-equal 1 1))`;
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, src);
    const r = compileRustFile(tmpPath);
    assertStringIncludes(r, "Deno.test");
    assertStringIncludes(r, "assertEquals");
    assertStringIncludes(r, "basic");
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
});

Deno.test("DD-52 (JS): @lykn/testing surface macros expand correctly", () => {
  const src = `(import-macros "./packages/testing/mod.lykn" (test is-equal ok))\n(test "basic" (is-equal 1 1))`;
  const r = compileJS(src);
  assertStringIncludes(r, "Deno.test");
  assertStringIncludes(r, "assertEquals");
  assertStringIncludes(r, "basic");
});

// ── Regression: DD-50.7 still works ──────────────────────────────────

Deno.test("DD-52 regression: DD-50.7 patterns still compile correctly (Rust)", () => {
  const r = compileRust(
    `(for-of item items
       (console:log item)
       (if (= item null) (throw (new Error "null")))
       (process item))`
  );
  assertEquals(r.includes("COMPILE_ERROR"), false);
  assertStringIncludes(r, "if (item === null)");
});

// ── A-3: bad-form fixture validation ────────────────────────────────

Deno.test("DD-52 (Rust): bad-form surface-macros directive produces validation error", () => {
  const src = `(import-macros "${CWD}/test/regression/surface-macros/bad-form" ())`;
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, src);
    const result = tryCompileRustFile(tmpPath);
    assertEquals(result.success, false);
    assertStringIncludes(result.stderr, "surface-macros");
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
});
