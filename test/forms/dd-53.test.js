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

function tryCompileRust(source) {
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, source);
    const lyknBin = Deno.env.get("LYKN_BIN") || "./bin/lykn";
    const proc = new Deno.Command(lyknBin, {
      args: ["compile", tmpPath],
      stdout: "piped",
      stderr: "piped",
    }).outputSync();
    return {
      success: proc.success,
      stderr: new TextDecoder().decode(proc.stderr).trim(),
      stdout: new TextDecoder().decode(proc.stdout).trim(),
    };
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
}

// ── R-3: Bad-form validation ─────────────────────────────────────────

Deno.test("DD-53 R-3: (surface-macros 42) produces validation error", () => {
  const tmpDir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(tmpDir + "/mod.lykn", "(surface-macros 42)");
    const src = `(import-macros "${tmpDir}" ())`;
    const result = tryCompileRust(src);
    assertEquals(result.success, false, "compilation should fail for bad-form surface-macros");
    assertStringIncludes(result.stderr, "surface-macros argument must be a string literal");
  } finally {
    try { Deno.removeSync(tmpDir, { recursive: true }); } catch { /* ignore */ }
  }
});

Deno.test("DD-53 R-3: (surface-macros) with no arg produces validation error", () => {
  const tmpDir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(tmpDir + "/mod.lykn", "(surface-macros)");
    const src = `(import-macros "${tmpDir}" ())`;
    const result = tryCompileRust(src);
    assertEquals(result.success, false, "compilation should fail for no-arg surface-macros");
    assertStringIncludes(result.stderr, "surface-macros expects exactly one string argument, got 0");
  } finally {
    try { Deno.removeSync(tmpDir, { recursive: true }); } catch { /* ignore */ }
  }
});

Deno.test("DD-53 R-3: (surface-macros \"a\" \"b\") with extra args produces validation error", () => {
  const tmpDir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(tmpDir + "/mod.lykn", '(surface-macros "a" "b")');
    const src = `(import-macros "${tmpDir}" ())`;
    const result = tryCompileRust(src);
    assertEquals(result.success, false, "compilation should fail for multi-arg surface-macros");
    assertStringIncludes(result.stderr, "surface-macros expects exactly one string argument, got 2");
  } finally {
    try { Deno.removeSync(tmpDir, { recursive: true }); } catch { /* ignore */ }
  }
});

// ── R-4: Q5 path validation for local paths ─────────────────────────

Deno.test("DD-53 R-4: (surface-macros \"../escape.js\") is rejected for local paths", () => {
  const tmpDir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(tmpDir + "/mod.lykn", '(surface-macros "../escape.js")');
    const src = `(import-macros "${tmpDir}" ())`;
    const result = tryCompileRust(src);
    assertEquals(result.success, false, "compilation should fail for .. path");
    assertStringIncludes(result.stderr, "'..' and empty segments not allowed");
  } finally {
    try { Deno.removeSync(tmpDir, { recursive: true }); } catch { /* ignore */ }
  }
});

Deno.test("DD-53 R-4: (surface-macros \"/etc/passwd\") is rejected for local paths", () => {
  const tmpDir = Deno.makeTempDirSync();
  try {
    Deno.writeTextFileSync(tmpDir + "/mod.lykn", '(surface-macros "/etc/passwd")');
    const src = `(import-macros "${tmpDir}" ())`;
    const result = tryCompileRust(src);
    assertEquals(result.success, false, "compilation should fail for absolute path");
    assertStringIncludes(result.stderr, "absolute paths not allowed");
  } finally {
    try { Deno.removeSync(tmpDir, { recursive: true }); } catch { /* ignore */ }
  }
});

// ── R-5 Round 3: JSR end-to-end via production code path ─────────────
// Invokes `lykn compile` from a temp dir outside the workspace so
// jsr:@lykn/testing@0.5.2 flows through the production JSR fetch path
// (resolve-macro-source → sibling-fetch → cache → load_surface_macros).

Deno.test({
  name: "DD-53 R-5 (Round 3): JSR end-to-end via production code path",
  async fn() {
    const netPerm = await Deno.permissions.query({ name: "net", host: "jsr.io" });
    if (netPerm.state !== "granted") {
      console.log("  (skipped — network permission for jsr.io not granted)");
      return;
    }

    const tempProject = Deno.makeTempDirSync({ prefix: "lykn-r5-r3-" });
    try {
      // Symlink packages/ so the subprocess can import the reader
      Deno.symlinkSync(`${CWD}/packages`, `${tempProject}/packages`);

      // Minimal project.json — no workspace shadowing of JSR specifiers
      Deno.writeTextFileSync(`${tempProject}/project.json`, JSON.stringify({
        imports: { "astring": "npm:astring@^1.9.0" },
      }));

      // Source with literal jsr: specifier
      Deno.writeTextFileSync(`${tempProject}/test.lykn`,
        '(import-macros "jsr:@lykn/testing@0.5.2" (test is-equal))\n(test "r5-r3" (is-equal 1 1))\n');

      // Invoke lykn compile from the temp dir
      const lyknBin = `${CWD}/bin/lykn`;
      const proc = new Deno.Command(lyknBin, {
        args: ["compile", "test.lykn"],
        cwd: tempProject,
        stdout: "piped",
        stderr: "piped",
      }).outputSync();

      const stdout = new TextDecoder().decode(proc.stdout).trim();
      const stderr = new TextDecoder().decode(proc.stderr).trim();

      // Assert compilation succeeded
      assertEquals(proc.success, true, `lykn compile failed: ${stderr}`);

      // Assert macro expansion shape
      assertStringIncludes(stdout, "Deno.test", "test macro should expand to Deno.test");
      assertStringIncludes(stdout, "assertEquals", "is-equal macro should expand to assertEquals");
      assertStringIncludes(stdout, "r5-r3", "test name should be in output");

      // Inspect cache directory — verify per-package layout with mod.lykn + macros.js
      const cacheRoot = `${Deno.env.get("HOME")}/.cache/lykn/macros`;
      const expectedKey = "jsr_@lykn_testing@0.5.2";
      const pkgDir = `${cacheRoot}/${expectedKey}`;
      const modLyknStat = Deno.statSync(`${pkgDir}/mod.lykn`);
      assertEquals(modLyknStat.isFile, true, "cache should contain mod.lykn");
      const macrosJsStat = Deno.statSync(`${pkgDir}/macros.js`);
      assertEquals(macrosJsStat.isFile, true, "cache should contain macros.js");
    } finally {
      try { Deno.removeSync(tempProject, { recursive: true }); } catch { /* ignore */ }
    }
  },
});

// ── Regression tests ─────────────────────────────────────────────────

Deno.test("DD-53 regression: DD-52 local-path surface-macros still work (Rust)", () => {
  const src = `(import-macros "${CWD}/test/regression/surface-macros/bundled-impl" (greet))\n(greet "world")`;
  const r = compileRust(src);
  assertStringIncludes(r, "console.log");
  assertStringIncludes(r, "Hello");
});

Deno.test("DD-53 regression: DD-52 @lykn/testing still works (Rust)", () => {
  const src = `(import-macros "../packages/testing" (test is-equal))\n(test "basic" (is-equal 1 1))`;
  const tmpPath = Deno.makeTempFileSync({ dir: "test", suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, src);
    const r = compileRustFile(tmpPath);
    assertStringIncludes(r, "Deno.test");
    assertStringIncludes(r, "assertEquals");
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
});

Deno.test("DD-53 regression: DD-50.7 patterns still compile correctly (Rust)", () => {
  const r = compileRust(
    `(for-of item items
       (console:log item)
       (if (= item null) (throw (new Error "null")))
       (process item))`
  );
  assertEquals(r.includes("COMPILE_ERROR"), false, "should not contain COMPILE_ERROR");
  assertStringIncludes(r, "if (item === null)");
});

Deno.test("DD-53 regression: mycelium render.lykn compiles without bugs (Rust)", () => {
  const renderPath = "/Users/oubiwann/lab/lykn/mycelium/packages/mycl-html/render.lykn";
  try {
    Deno.statSync(renderPath);
  } catch {
    console.log("  (skipped — mycelium not present at expected path)");
    return;
  }
  const r = compileRustFile(renderPath);
  assertEquals(r.includes("() => if"), false, "E.1 should not be present");
  assertEquals(r.includes("return throw"), false, "E.2 should not be present");
  assertEquals(r.includes("COMPILE_ERROR"), false, "Cluster 2 should not be present");
});
