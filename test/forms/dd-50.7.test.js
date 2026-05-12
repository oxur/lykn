import { assertEquals, assertStringIncludes, assertThrows } from "jsr:@std/assert";
import { lykn } from "../../packages/lang/mod.js";

function compileJS(source) {
  return lykn(source).trim();
}

function compileRust(source) {
  const tmpPath = Deno.makeTempFileSync({ suffix: ".lykn" });
  try {
    Deno.writeTextFileSync(tmpPath, source);
    const lyknBin = Deno.env.get("LYKN_BIN") || "./bin/lykn";
    const proc = new Deno.Command(lyknBin, {
      args: ["compile", tmpPath],
      stdout: "piped",
      stderr: "piped",
    }).outputSync();
    if (!proc.success) {
      const stderr = new TextDecoder().decode(proc.stderr);
      throw new Error(`Rust compiler failed:\n${stderr}`);
    }
    let rustOut = new TextDecoder().decode(proc.stdout).trim();
    return rustOut.split("\n")
      .filter((l) => !l.includes(": warning:") && !l.startsWith("  suggestion:"))
      .join("\n").trim();
  } finally {
    try { Deno.removeSync(tmpPath); } catch { /* ignore */ }
  }
}

function assertNoBugSignatures(output, label) {
  assertEquals(output.includes("() => if"), false, `${label}: contains '() => if'`);
  assertEquals(output.includes("return throw"), false, `${label}: contains 'return throw'`);
  assertEquals(output.includes("COMPILE_ERROR"), false, `${label}: contains 'COMPILE_ERROR'`);
}

// ── Cluster 1: nested if with statement branches in for-of body ──────

const CLUSTER_1_SRC =
  `(for-of (array k v) (Object:entries x)
     (validate k)
     (if (= v true) (swap! parts (fn (:array p) (conj p k)))
       (if (or (= v false) (js:eq v null))
         undefined
         (if (= (js:typeof v) "string")
           (swap! parts (fn (:array p) (conj p k)))
           (throw (new Error "bad value"))))))`;

Deno.test("DD-50.7 cluster 1 (JS): nested if chain in for-of body", () => {
  const r = compileJS(CLUSTER_1_SRC);
  assertNoBugSignatures(r, "JS");
  assertStringIncludes(r, "if (v === true)");
});

Deno.test("DD-50.7 cluster 1 (Rust): nested if chain in for-of body", () => {
  const r = compileRust(CLUSTER_1_SRC);
  assertNoBugSignatures(r, "Rust");
  assertStringIncludes(r, "if (v === true)");
});

Deno.test("DD-50.7 cluster 1 (JS): multi-body for-of preserves statement context", () => {
  const src = `(for-of item items
     (console:log item)
     (if (= item null) (throw (new Error "null")))
     (process item))`;
  const r = compileJS(src);
  assertNoBugSignatures(r, "JS");
  assertStringIncludes(r, "if (item === null)");
});

Deno.test("DD-50.7 cluster 1 (Rust): multi-body for-of preserves statement context", () => {
  const src = `(for-of item items
     (console:log item)
     (if (= item null) (throw (new Error "null")))
     (process item))`;
  const r = compileRust(src);
  assertNoBugSignatures(r, "Rust");
  assertStringIncludes(r, "if (item === null)");
});

// ── Cluster 2: no-else if in block body in func body ─────────────────

const CLUSTER_2A_SRC =
  `(func handle :args (:array x) :returns :string :body
     (bind tag (get x 0))
     (if (= tag "br")
       (block
         (if (> x:length 1)
           (throw (new Error "void")))
         (return (template "<" tag ">"))))
     (template "<" tag ">done</" tag ">"))`;

Deno.test("DD-50.7 cluster 2 (JS): no-else if in block inside func body", () => {
  const r = compileJS(CLUSTER_2A_SRC);
  assertNoBugSignatures(r, "JS");
  assertStringIncludes(r, "if (x.length > 1)");
});

Deno.test("DD-50.7 cluster 2 (Rust): no-else if in block inside func body", () => {
  const r = compileRust(CLUSTER_2A_SRC);
  assertNoBugSignatures(r, "Rust");
  assertStringIncludes(r, "if (x.length > 1)");
});

const CLUSTER_2B_SRC =
  `(func check :args (:any x) :returns :string :body
     (if (= x null) (throw (new Error "null")))
     (if (= x 0) (return "zero"))
     "other")`;

Deno.test("DD-50.7 cluster 2 (JS): mid-body no-else if in func body", () => {
  const r = compileJS(CLUSTER_2B_SRC);
  assertNoBugSignatures(r, "JS");
  assertStringIncludes(r, "if (x === null)");
});

Deno.test("DD-50.7 cluster 2 (Rust): mid-body no-else if in func body", () => {
  const r = compileRust(CLUSTER_2B_SRC);
  assertNoBugSignatures(r, "Rust");
  assertStringIncludes(r, "if (x === null)");
});

// ── Regressions: prior DD-50 machinery still works ──────────────────

Deno.test("DD-50.7 regression: both-branch if in expression position → ternary", () => {
  const r = compileJS("(bind x (if (= a b) 1 2))");
  assertStringIncludes(r, "?");
});

Deno.test("DD-50.7 regression: func with value-typed last still wraps in return", () => {
  const r = compileJS("(func add :args (:number a :number b) :returns :number :body (+ a b))");
  assertStringIncludes(r, "return ");
});

Deno.test("DD-50.7 regression: Q2=A still fires for :returns with statement-only last", () => {
  assertThrows(
    () => compileJS('(func bad :args (:any x) :returns :boolean :body (if (= x 1) (console:log "yes")))'),
    Error,
    "declared `:returns :boolean` but body ends with `if`"
  );
});

// ── Real downstream fixture compilation ──────────────────────────────

// ── DD-50.7 extension: emit_if_iife legitimate-IIFE patterns ─────────

Deno.test("DD-50.7 extension (JS): bind initializer with throw branch — IIFE valid JS", () => {
  const r = compileJS('(bind x (if (= a b) (throw (new Error "bad")) "value"))');
  assertNoBugSignatures(r, "JS bind-with-throw");
  assertStringIncludes(r, "() => {");
  assertStringIncludes(r, "throw new Error");
  assertEquals(r.includes("return throw"), false);
});

Deno.test("DD-50.7 extension (Rust): bind initializer with throw branch — IIFE valid JS", () => {
  const r = compileRust('(bind x (if (= a b) (throw (new Error "bad")) "value"))');
  assertNoBugSignatures(r, "Rust bind-with-throw");
  assertStringIncludes(r, "throw new Error");
  assertEquals(r.includes("return throw"), false);
});

Deno.test("DD-50.7 extension (JS): call argument with throw branch — IIFE valid JS", () => {
  const r = compileJS('(some-call (if cond (throw (new Error "x")) value))');
  assertNoBugSignatures(r, "JS call-arg-with-throw");
  assertStringIncludes(r, "throw new Error");
  assertEquals(r.includes("return throw"), false);
});

Deno.test("DD-50.7 extension (Rust): call argument with throw branch — IIFE valid JS", () => {
  const r = compileRust('(some-call (if cond (throw (new Error "x")) value))');
  assertNoBugSignatures(r, "Rust call-arg-with-throw");
  assertStringIncludes(r, "throw new Error");
  assertEquals(r.includes("return throw"), false);
});

Deno.test("DD-50.7 extension (JS): throw in else branch — IIFE valid JS", () => {
  const r = compileJS('(bind y (if ok "good" (throw (new Error "bad"))))');
  assertNoBugSignatures(r, "JS throw-in-else");
  assertStringIncludes(r, "throw new Error");
  assertStringIncludes(r, 'return "good"');
});

Deno.test("DD-50.7 extension (Rust): throw in else branch — IIFE valid JS", () => {
  const r = compileRust('(bind y (if ok "good" (throw (new Error "bad"))))');
  assertNoBugSignatures(r, "Rust throw-in-else");
  assertStringIncludes(r, "throw new Error");
  assertStringIncludes(r, 'return "good"');
});

// ── Real downstream fixture compilation ──────────────────────────────

Deno.test("DD-50.7 downstream (JS): nested-if-statement-branches fixture", () => {
  const src = Deno.readTextFileSync("test/regression/downstream-shapes/nested-if-statement-branches.lykn");
  const r = compileJS(src);
  assertNoBugSignatures(r, "JS fixture");
});

Deno.test("DD-50.7 downstream (Rust): nested-if-statement-branches fixture", () => {
  const src = Deno.readTextFileSync("test/regression/downstream-shapes/nested-if-statement-branches.lykn");
  const r = compileRust(src);
  assertNoBugSignatures(r, "Rust fixture");
});

Deno.test("DD-50.7 downstream (JS): no-else-if-in-block-body fixture", () => {
  const src = Deno.readTextFileSync("test/regression/downstream-shapes/no-else-if-in-block-body.lykn");
  const r = compileJS(src);
  assertNoBugSignatures(r, "JS fixture");
  assertStringIncludes(r, "throw new Error");
});

Deno.test("DD-50.7 downstream (Rust): no-else-if-in-block-body fixture", () => {
  const src = Deno.readTextFileSync("test/regression/downstream-shapes/no-else-if-in-block-body.lykn");
  const r = compileRust(src);
  assertNoBugSignatures(r, "Rust fixture");
  assertStringIncludes(r, "throw new Error");
});
