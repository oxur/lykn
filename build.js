// Build script for browser bundle — run with: deno run -A build.js
import * as esbuild from "npm:esbuild";

// Resolve astring's package directory from Deno's npm cache
const astringMeta = import.meta.resolve("astring");
// e.g. file:///.../.../astring/1.9.0/dist/astring.mjs → parent of the package
const astringPkg = astringMeta.replace("file://", "").replace(/\/dist\/.*$/, "");

// Provide a stub for node:path in browser builds (import-macros not available)
const nodePathShimPlugin = {
  name: "node-path-shim",
  setup(build) {
    build.onResolve({ filter: /^node:path$/ }, () => ({
      path: "node:path",
      namespace: "node-path-shim",
    }));
    build.onLoad({ filter: /.*/, namespace: "node-path-shim" }, () => ({
      contents: `
        export function resolve() { throw new Error("import-macros not available in browser"); }
        export function dirname() { throw new Error("import-macros not available in browser"); }
      `,
      loader: "js",
    }));
  },
};

const shared = {
  entryPoints: ["packages/lykn/browser.js"],
  bundle: true,
  format: "iife",
  globalName: "lykn",
  alias: {
    "astring": astringPkg,
  },
  plugins: [nodePathShimPlugin],
};

await esbuild.build({
  ...shared,
  outfile: "dist/lykn-browser.js",
  minify: true,
});

await esbuild.build({
  ...shared,
  outfile: "dist/lykn-browser.dev.js",
  minify: false,
});

console.log("Build complete: dist/lykn-browser.js and dist/lykn-browser.dev.js");
esbuild.stop();
