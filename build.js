// Build script for browser bundle — run with: deno run -A build.js
import * as esbuild from "npm:esbuild";

// Resolve astring's package directory from Deno's npm cache
const astringMeta = import.meta.resolve("astring");
// e.g. file:///.../.../astring/1.9.0/dist/astring.mjs → parent of the package
const astringPkg = astringMeta.replace("file://", "").replace(/\/dist\/.*$/, "");

const shared = {
  entryPoints: ["src/lykn-browser.js"],
  bundle: true,
  format: "iife",
  globalName: "lykn",
  alias: {
    "astring": astringPkg,
  },
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
