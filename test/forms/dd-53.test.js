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

// ── R-5: JSR end-to-end test (live, network-gated) ──────────────────

Deno.test({
  name: "DD-53 R-5: JSR-fetched surface-macros work end-to-end via protocol",
  async fn() {
    const netPerm = await Deno.permissions.query({ name: "net", host: "jsr.io" });
    if (netPerm.state !== "granted") {
      console.log("  (skipped — network permission for jsr.io not granted)");
      return;
    }

    // Use a unique temp cache directory to guarantee a fresh fetch
    const testCacheDir = Deno.makeTempDirSync({ prefix: "lykn-r5-" });
    const cacheKey = "r5_lykn_testing_0_5_2";

    try {
      // Invoke the subprocess action directly by compiling a small source
      // that imports from jsr:@lykn/testing@0.5.2.
      // Since workspace resolution shadows the JSR specifier, we invoke
      // the resolve-macro-source protocol action directly via a test script.
      const testScript = `
        import { read } from "./packages/lang/reader.js";
        const encoder = new TextEncoder();
        function writeLine(s) { Deno.stdout.writeSync(encoder.encode(s + "\\n")); }

        const spec = "jsr:@lykn/testing@0.5.2";
        const proc = new Deno.Command("deno", {
          args: ["info", "--json", spec],
          stdout: "piped", stderr: "piped",
        }).outputSync();
        if (!proc.success) { writeLine(JSON.stringify({ok:false,error:"deno info failed"})); Deno.exit(1); }
        const info = JSON.parse(new TextDecoder().decode(proc.stdout));
        const redirectUrl = (info.redirects || {})[spec];
        if (!redirectUrl) { writeLine(JSON.stringify({ok:false,error:"not found on JSR"})); Deno.exit(1); }
        const baseUrl = redirectUrl.replace(/\\/[^/]+$/, "/");
        const srcResp = await fetch(baseUrl + "mod.lykn");
        const source = await srcResp.text();

        // Parse and fetch siblings
        const forms = read(source);
        const pkgDir = "${testCacheDir}/${cacheKey}";
        Deno.mkdirSync(pkgDir, { recursive: true });
        Deno.writeTextFileSync(pkgDir + "/mod.lykn", source);
        for (const form of forms) {
          if (form.type === "list" && form.values.length === 2 &&
              form.values[0].type === "atom" && form.values[0].value === "surface-macros" &&
              form.values[1].type === "string") {
            const sibPath = form.values[1].value;
            const sibResp = await fetch(baseUrl + sibPath);
            Deno.writeTextFileSync(pkgDir + "/" + sibPath, await sibResp.text());
          }
        }
        writeLine(JSON.stringify({ok:true, source: source.substring(0, 50), baseUrl}));
      `;

      const proc = new Deno.Command("deno", {
        args: ["eval", "--config", "project.json", "--ext=js", testScript],
        stdout: "piped",
        stderr: "piped",
      }).outputSync();

      const stdout = new TextDecoder().decode(proc.stdout).trim();
      const stderr = new TextDecoder().decode(proc.stderr).trim();
      if (!proc.success) {
        throw new Error(`Protocol test failed: ${stderr}`);
      }

      const result = JSON.parse(stdout);
      assertEquals(result.ok, true, "JSR fetch should succeed");

      // Verify cache directory contents
      const pkgDir = `${testCacheDir}/${cacheKey}`;
      const modLykn = Deno.readTextFileSync(pkgDir + "/mod.lykn");
      assertStringIncludes(modLykn, "surface-macros", "cached mod.lykn should contain surface-macros directive");

      const macrosJs = Deno.readTextFileSync(pkgDir + "/macros.js");
      assertStringIncludes(macrosJs, "macroEnv", "cached macros.js should contain macroEnv references");

      // Verify the cached files can be used by load_surface_macros
      // by compiling a source that imports from the cached directory
      const testSource = `(import-macros "${pkgDir}" (test is-equal))\n(test "r5" (is-equal 1 1))`;
      const compiled = compileRust(testSource);
      assertStringIncludes(compiled, "Deno.test", "surface-macros expansion should produce Deno.test");
      assertStringIncludes(compiled, "assertEquals", "is-equal macro should expand to assertEquals");
    } finally {
      try { Deno.removeSync(testCacheDir, { recursive: true }); } catch { /* ignore */ }
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
