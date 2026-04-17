use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

mod compile;
mod doctest;

#[derive(Parser)]
#[command(name = "lykn", version, about = "lykn language toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Format .lykn files
    Fmt {
        /// Files to format
        files: Vec<PathBuf>,
        /// Write formatted output back to file
        #[arg(short, long)]
        write: bool,
    },
    /// Check .lykn syntax
    Check {
        /// Files to check
        files: Vec<PathBuf>,
    },
    /// Compile .lykn to JavaScript
    Compile {
        /// Input .lykn file
        file: PathBuf,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Strip type checks and contracts
        #[arg(long)]
        strip_assertions: bool,
        /// Output kernel JSON instead of JS
        #[arg(long)]
        kernel_json: bool,
    },
    /// Run a .lykn or .js file
    Run {
        /// File to run
        file: PathBuf,
        /// Arguments to pass to the script
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Run tests via Deno
    Test {
        /// Test file/directory patterns (default: test/)
        #[arg(default_value = "test/")]
        patterns: Vec<String>,
        /// Test lykn code blocks in Markdown files
        #[arg(long)]
        docs: Option<String>,
        /// Write compiled JS to a separate directory
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Compile .lykn files but don't run tests
        #[arg(long)]
        compile_only: bool,
        /// Extra args passed through to Deno's test runner
        #[arg(last = true)]
        deno_args: Vec<String>,
    },
    /// Lint compiled JS via Deno
    Lint {
        /// Paths to lint (default: packages/)
        #[arg(default_value = "packages/")]
        paths: Vec<String>,
    },
    /// Create a new lykn project
    New {
        /// Project name (kebab-case)
        name: String,
        /// Parent directory (default: current directory)
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Build browser bundle or npm package
    Build {
        /// Build the browser bundle (dist/lykn-browser.js)
        #[arg(long)]
        browser: bool,
        /// Build the npm package (dist/npm/)
        #[arg(long)]
        npm: bool,
    },
    /// Publish package(s)
    Publish {
        /// Publish to JSR (JavaScript Registry)
        #[arg(long)]
        jsr: bool,
        /// Build and publish to npm
        #[arg(long)]
        npm: bool,
        /// Dry run (don't actually publish)
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fmt { files, write } => cmd_fmt(&files, write),
        Commands::Check { files } => cmd_check(&files),
        Commands::Compile {
            file,
            output,
            strip_assertions,
            kernel_json,
        } => cmd_compile(&file, output.as_deref(), strip_assertions, kernel_json),
        Commands::Run { file, args } => cmd_run(&file, &args),
        Commands::Test {
            patterns,
            docs,
            out_dir,
            compile_only,
            deno_args,
        } => cmd_test(&patterns, docs.as_deref(), out_dir.as_deref(), compile_only, &deno_args),
        Commands::Lint { paths } => cmd_lint(&paths),
        Commands::New { name, path } => cmd_new(&name, path.as_deref()),
        Commands::Build { browser, npm } => cmd_build(browser, npm),
        Commands::Publish { jsr, npm, dry_run } => cmd_publish(jsr, npm, dry_run),
    }
}

fn cmd_fmt(files: &[PathBuf], write: bool) {
    if files.is_empty() {
        eprintln!("Usage: lykn fmt <file.lykn>");
        process::exit(1);
    }

    for path in files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {e}", path.display());
                process::exit(1);
            }
        };

        let exprs = lykn_cli::reader::read(&source);
        let formatted = lykn_cli::formatter::format_exprs(&exprs, 0);

        if write {
            if let Err(e) = std::fs::write(path, &formatted) {
                eprintln!("error writing {}: {e}", path.display());
                process::exit(1);
            }
            eprintln!("{}: formatted", path.display());
        } else {
            print!("{formatted}");
        }
    }
}

fn cmd_check(files: &[PathBuf]) {
    if files.is_empty() {
        eprintln!("Usage: lykn check <file.lykn>");
        process::exit(1);
    }

    for path in files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {e}", path.display());
                process::exit(1);
            }
        };

        let exprs = lykn_cli::reader::read(&source);
        if exprs.is_empty() && !source.trim().is_empty() {
            eprintln!(
                "{}: warning: source is non-empty but parsed to zero expressions",
                path.display()
            );
        } else {
            eprintln!(
                "{}: ok ({} top-level expressions)",
                path.display(),
                exprs.len()
            );
        }
    }
}

fn cmd_compile(
    file: &std::path::Path,
    output: Option<&std::path::Path>,
    strip_assertions: bool,
    kernel_json: bool,
) {
    match compile::compile_file(file, strip_assertions, kernel_json) {
        Ok(result) => {
            if let Some(out_path) = output {
                if let Err(e) = std::fs::write(out_path, &result) {
                    eprintln!("error writing {}: {e}", out_path.display());
                    process::exit(1);
                }
            } else {
                print!("{result}");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Deno wrapper subcommands
// ---------------------------------------------------------------------------

/// Find the project config path by walking up from the current directory.
fn find_config() -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.as_path();
    loop {
        if dir.join("project.json").exists() {
            return dir.join("project.json").to_string_lossy().into_owned();
        }
        if dir.join("deno.json").exists() {
            return dir.join("deno.json").to_string_lossy().into_owned();
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                // Fallback — use project.json in current dir even if it doesn't exist
                return "project.json".to_string();
            }
        }
    }
}

/// Execute a deno command, exiting with its status code.
fn exec_deno(args: &[&str]) {
    let status = Command::new("deno")
        .args(args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run deno: {e}");
            eprintln!("is deno installed? try: brew install deno");
            process::exit(1);
        });
    process::exit(status.code().unwrap_or(1));
}

fn cmd_run(file: &std::path::Path, args: &[String]) {
    let config = find_config();

    if file.extension().is_some_and(|e| e == "lykn") {
        // Compile .lykn to temp .js, then run
        let temp = std::env::temp_dir().join("lykn_run.js");
        match compile::compile_file(file, false, false) {
            Ok(js) => {
                if let Err(e) = std::fs::write(&temp, &js) {
                    eprintln!("error writing temp file: {e}");
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        }
        let temp_str = temp.to_string_lossy();
        let mut deno_args = vec!["run", "--config", &config, "-A", &temp_str];
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        deno_args.extend(arg_refs);
        exec_deno(&deno_args);
    } else {
        let file_str = file.to_string_lossy();
        let mut deno_args = vec!["run", "--config", &config, "-A", &*file_str];
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        deno_args.extend(arg_refs);
        exec_deno(&deno_args);
    }
}

fn cmd_test(
    patterns: &[String],
    docs: Option<&str>,
    out_dir: Option<&Path>,
    compile_only: bool,
    extra_deno_args: &[String],
) {
    let config = find_config();

    // Handle --docs mode: extract and test Markdown code blocks
    if let Some(docs_path) = docs {
        // If there are also .lykn patterns, compile them first
        let lykn_files = discover_lykn_test_files(patterns);
        if !lykn_files.is_empty() {
            let compiled = compile_lykn_test_files(&lykn_files, out_dir);
            if compile_only {
                eprintln!("Compiled {} .lykn test file(s).", compiled.len());
                // Still run doc tests below (compile_only only affects .lykn files)
            } else {
                // Run .lykn tests first, then doc tests
                let test_paths: Vec<String> =
                    compiled.iter().map(|p| p.to_string_lossy().into_owned()).collect();
                run_deno_test(&config, &test_paths, extra_deno_args);
            }
        }
        // run_doc_tests exits the process
        doctest::run_doc_tests(docs_path, &config, extra_deno_args);
    }

    // Discover .lykn test files in the given patterns
    let lykn_files = discover_lykn_test_files(patterns);

    if lykn_files.is_empty() {
        // No .lykn test files found — delegate directly to Deno
        if compile_only {
            eprintln!("No .lykn test files found to compile.");
            return;
        }
        let mut deno_args = vec!["test", "--config", &config, "--no-check", "-A"];
        let refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
        deno_args.extend(refs);
        let extra_refs: Vec<&str> = extra_deno_args.iter().map(|s| s.as_str()).collect();
        deno_args.extend(extra_refs);
        exec_deno(&deno_args);
    } else {
        // Compile .lykn files, then optionally run them
        let compiled = compile_lykn_test_files(&lykn_files, out_dir);
        eprintln!("Compiled {} .lykn test file(s).", compiled.len());

        if compile_only {
            return;
        }

        // Build the list of paths to pass to deno test
        let test_paths = if let Some(od) = out_dir {
            // With --out-dir, test the output directory
            vec![od.to_string_lossy().into_owned()]
        } else {
            // Test the directories/files that contain the compiled output.
            // If patterns were directories, pass those. If individual files,
            // pass the compiled .js paths.
            let mut paths: Vec<String> = Vec::new();
            for pattern in patterns {
                let p = Path::new(pattern);
                if p.is_dir() {
                    paths.push(pattern.clone());
                }
            }
            if paths.is_empty() {
                // Individual files — pass compiled JS paths
                paths = compiled
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
            }
            paths
        };

        let path_refs: Vec<&str> = test_paths.iter().map(|s| s.as_str()).collect();
        let mut deno_args = vec!["test", "--config", &config, "--no-check", "-A"];
        deno_args.extend(path_refs);
        let extra_refs: Vec<&str> = extra_deno_args.iter().map(|s| s.as_str()).collect();
        deno_args.extend(extra_refs);
        exec_deno(&deno_args);
    }
}

/// Discover `.lykn` test files matching `*_test.lykn` or `*.test.lykn` in the
/// given patterns. Patterns may be files or directories (searched recursively).
fn discover_lykn_test_files(patterns: &[String]) -> Vec<PathBuf> {
    let mut results = Vec::new();
    for pattern in patterns {
        let path = Path::new(pattern);
        if path.is_file() {
            if is_lykn_test_file(path) {
                results.push(path.to_path_buf());
            }
        } else if path.is_dir() {
            collect_lykn_test_files(path, &mut results);
        }
    }
    results.sort();
    results
}

/// Check whether a path matches lykn test file naming conventions.
///
/// Matches: `*_test.lykn`, `*.test.lykn`, and any `.lykn` file inside a
/// `__tests__` directory.
fn is_lykn_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with("_test.lykn") || name.ends_with(".test.lykn") {
        return true;
    }
    // Also match any .lykn file inside a __tests__ directory
    if name.ends_with(".lykn")
        && let Some(parent) = path.parent()
    {
        return parent
            .components()
            .any(|c| c.as_os_str() == "__tests__");
    }
    false
}

/// Recursively collect lykn test files from a directory.
fn collect_lykn_test_files(dir: &Path, results: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lykn_test_files(&path, results);
        } else if is_lykn_test_file(&path) {
            results.push(path);
        }
    }
}

/// Compile a list of `.lykn` test files to JavaScript.
///
/// Returns the list of compiled `.js` file paths.
fn compile_lykn_test_files(files: &[PathBuf], out_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut compiled = Vec::new();

    for lykn_path in files {
        let js_path = if let Some(od) = out_dir {
            // Mirror directory structure under out_dir
            let relative = lykn_path
                .strip_prefix(".")
                .unwrap_or(lykn_path);
            let dest = od.join(relative).with_extension("js");
            if let Some(parent) = dest.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                eprintln!("error creating directory {}: {e}", parent.display());
                process::exit(1);
            }
            dest
        } else {
            // Write next to the source file
            lykn_path.with_extension("js")
        };

        match compile::compile_file(lykn_path, false, false) {
            Ok(js) => {
                if let Err(e) = fs::write(&js_path, &js) {
                    eprintln!("error writing {}: {e}", js_path.display());
                    process::exit(1);
                }
                eprintln!("  {} -> {}", lykn_path.display(), js_path.display());
                compiled.push(js_path);
            }
            Err(e) => {
                eprintln!("error compiling {}: {e}", lykn_path.display());
                process::exit(1);
            }
        }
    }

    compiled
}

/// Run `deno test` and return (do not exit the process).
fn run_deno_test(config: &str, paths: &[String], extra_args: &[String]) {
    let mut args: Vec<&str> = vec!["test", "--config", config, "--no-check", "-A"];
    let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    args.extend(path_refs);
    let extra_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
    args.extend(extra_refs);

    let status = Command::new("deno")
        .args(&args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run deno: {e}");
            eprintln!("is deno installed? try: brew install deno");
            process::exit(1);
        });

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
}

fn cmd_lint(paths: &[String]) {
    let config = find_config();
    let mut deno_args = vec!["lint", "--config", &config];
    let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    deno_args.extend(refs);
    exec_deno(&deno_args);
}

fn cmd_publish(jsr: bool, npm: bool, dry_run: bool) {
    // Default to JSR if no flags specified
    let do_jsr = jsr || !npm;
    let do_npm = npm;

    if do_jsr {
        let config = find_config();
        let mut args = vec!["publish", "--config", &config];
        if dry_run {
            args.push("--dry-run");
        }
        eprintln!("Publishing to JSR...");
        let status = Command::new("deno")
            .args(&args)
            .status()
            .unwrap_or_else(|e| {
                eprintln!("failed to run deno: {e}");
                process::exit(1);
            });
        if !status.success() {
            eprintln!("JSR publish failed");
            process::exit(status.code().unwrap_or(1));
        }
    }

    if do_npm {
        // Build both npm packages natively
        build_npm_package();

        // Publish all built npm packages (dist/npm-*)
        let npm_dirs: Vec<_> = fs::read_dir("dist")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_type().is_ok_and(|t| t.is_dir())
                    && e.file_name().to_string_lossy().starts_with("npm-")
            })
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect();
        for dir in &npm_dirs {
            if dry_run {
                eprintln!("npm dry run — checking {dir}...");
                let status = Command::new("npm")
                    .args(["pack", "--dry-run"])
                    .current_dir(dir)
                    .status()
                    .unwrap_or_else(|e| {
                        eprintln!("failed to run npm: {e}");
                        process::exit(1);
                    });
                if !status.success() {
                    process::exit(status.code().unwrap_or(1));
                }
            } else {
                eprintln!("Publishing {dir} to npm...");
                let status = Command::new("npm")
                    .args(["publish", "--access", "public"])
                    .current_dir(dir)
                    .status()
                    .unwrap_or_else(|e| {
                        eprintln!("failed to run npm: {e}");
                        process::exit(1);
                    });
                if !status.success() {
                    process::exit(status.code().unwrap_or(1));
                }
            }
        }
    }

    eprintln!("Done.");
}

// ---------------------------------------------------------------------------
// lykn new — project creation
// ---------------------------------------------------------------------------

fn validate_project_name(name: &str) {
    if name.is_empty() {
        eprintln!("error: project name cannot be empty");
        process::exit(1);
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        eprintln!("error: project name must be kebab-case (lowercase letters, digits, hyphens)");
        process::exit(1);
    }
    if name.starts_with('-') || name.starts_with(|c: char| c.is_ascii_digit()) {
        eprintln!("error: project name must start with a letter");
        process::exit(1);
    }
}

fn write_file(path: &Path, content: &str) {
    if let Err(e) = fs::write(path, content) {
        eprintln!("error writing {}: {e}", path.display());
        process::exit(1);
    }
}

fn project_json_template(name: &str) -> String {
    format!(
        r#"{{
    "workspace": ["./packages/{name}"],
    "imports": {{
        "{name}/": "./packages/{name}/"
    }},
    "lint": {{
        "rules": {{
            "exclude": ["no-slow-types"]
        }}
    }},
    "tasks": {{
        "test": "deno test -A test/"
    }}
}}
"#
    )
}

fn deno_json_template(name: &str) -> String {
    format!(
        r#"{{
    "name": "@{name}/{name}",
    "version": "0.1.0",
    "exports": "./mod.lykn"
}}
"#
    )
}

fn mod_lykn_template(name: &str) -> String {
    format!(
        r#";; {name} — created with lykn new

(bind greeting "Hello from {name}!")
(console:log greeting)
"#
    )
}

fn test_template(name: &str) -> String {
    format!(
        r#"import {{ assertEquals }} from "https://deno.land/std/assert/mod.ts";

Deno.test("{name}: placeholder test", () => {{
  assertEquals(1 + 1, 2);
}});
"#
    )
}

const GITIGNORE_TEMPLATE: &str = ".DS_Store
node_modules/
target/
dist/
bin/
*.js.map
";

fn cmd_new(name: &str, path: Option<&Path>) {
    validate_project_name(name);

    let base = path.unwrap_or(Path::new("."));
    let project_dir = base.join(name);

    if project_dir.exists() {
        eprintln!(
            "error: directory '{}' already exists",
            project_dir.display()
        );
        process::exit(1);
    }

    // Create directories
    if let Err(e) = fs::create_dir_all(project_dir.join("packages").join(name)) {
        eprintln!("error creating directories: {e}");
        process::exit(1);
    }
    if let Err(e) = fs::create_dir_all(project_dir.join("test")) {
        eprintln!("error creating directories: {e}");
        process::exit(1);
    }

    // Write template files
    write_file(
        &project_dir.join("project.json"),
        &project_json_template(name),
    );
    write_file(
        &project_dir.join("packages").join(name).join("deno.json"),
        &deno_json_template(name),
    );
    write_file(
        &project_dir.join("packages").join(name).join("mod.lykn"),
        &mod_lykn_template(name),
    );
    write_file(
        &project_dir.join("test").join("mod.test.js"),
        &test_template(name),
    );
    write_file(&project_dir.join(".gitignore"), GITIGNORE_TEMPLATE);

    // Git init (silent failure if git not installed)
    let _ = Command::new("git")
        .args(["init"])
        .current_dir(&project_dir)
        .stdout(process::Stdio::null())
        .stderr(process::Stdio::null())
        .status();

    eprintln!(
        "Created lykn project '{}' in {}",
        name,
        project_dir.display()
    );
    eprintln!();
    eprintln!("  cd {name}");
    eprintln!("  lykn run packages/{name}/mod.lykn");
    eprintln!();
    eprintln!("Happy hacking!");
}

// ---------------------------------------------------------------------------
// lykn build — build artifacts
// ---------------------------------------------------------------------------

fn cmd_build(browser: bool, npm: bool) {
    if !browser && !npm {
        eprintln!("Usage: lykn build --browser or lykn build --npm");
        process::exit(1);
    }
    if browser {
        build_browser_bundle();
    }
    if npm {
        build_npm_package();
    }
}

/// Build the browser bundle by invoking esbuild via Deno.
/// The build script is embedded in the binary.
fn build_browser_bundle() {
    eprintln!("Building browser bundle...");

    let script = r#"
import * as esbuild from "npm:esbuild";
const astringMeta = import.meta.resolve("astring");
const astringPkg = astringMeta.replace("file://", "").replace(/\/dist\/.*$/, "");
const nodePathShimPlugin = {
  name: "node-path-shim",
  setup(build) {
    build.onResolve({ filter: /^node:path$/ }, () => ({
      path: "node:path", namespace: "node-path-shim",
    }));
    build.onLoad({ filter: /.*/, namespace: "node-path-shim" }, () => ({
      contents: `
        export function resolve() { throw new Error("import-macros not available in browser"); }
        export function dirname() { throw new Error("import-macros not available in browser"); }
      `, loader: "js",
    }));
  },
};
const lyknImportPlugin = {
  name: "lykn-import-map",
  setup(build) {
    build.onResolve({ filter: /^lang\// }, (args) => {
      const rel = args.path.replace(/^lang\//, "packages/lang/");
      return { path: Deno.cwd() + "/" + rel };
    });
  },
};
const shared = {
  entryPoints: ["packages/browser/mod.js"],
  bundle: true, format: "iife", globalName: "lykn",
  alias: { "astring": astringPkg },
  plugins: [nodePathShimPlugin, lyknImportPlugin],
};
await Deno.mkdir("dist", { recursive: true });
await esbuild.build({ ...shared, outfile: "dist/lykn-browser.js", minify: true });
await esbuild.build({ ...shared, outfile: "dist/lykn-browser.dev.js", minify: false });
console.log("Build complete: dist/lykn-browser.js and dist/lykn-browser.dev.js");
esbuild.stop();
"#;

    let config = find_config();
    let status = Command::new("deno")
        .args(["eval", "--config", &config, "--ext=js", script])
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run deno: {e}");
            process::exit(1);
        });
    if !status.success() {
        eprintln!("Browser build failed");
        process::exit(status.code().unwrap_or(1));
    }
}

/// Build npm packages for all workspace members.
/// Reads project.json → workspace array → each package's deno.json.
fn build_npm_package() {
    eprintln!("Building npm packages...");

    let workspace_members = read_workspace_members();
    if workspace_members.is_empty() {
        eprintln!("error: no workspace members found in project.json");
        process::exit(1);
    }

    for pkg_dir in &workspace_members {
        build_npm_for_package(pkg_dir);
    }
}

/// Read the workspace member directories from project.json.
fn read_workspace_members() -> Vec<String> {
    let config_str = fs::read_to_string("project.json").unwrap_or_else(|e| {
        eprintln!("error reading project.json: {e}");
        process::exit(1);
    });
    // Extract workspace array entries: "workspace": ["./packages/foo", "./packages/bar"]
    let mut members = Vec::new();
    if let Some(start) = config_str.find("\"workspace\"") {
        let rest = &config_str[start..];
        if let Some(arr_start) = rest.find('[') {
            let arr_rest = &rest[arr_start + 1..];
            if let Some(arr_end) = arr_rest.find(']') {
                let arr_content = &arr_rest[..arr_end];
                for item in arr_content.split(',') {
                    let trimmed = item.trim().trim_matches('"').trim_matches('\'');
                    if !trimmed.is_empty() {
                        // Normalize: "./packages/lykn" → "packages/lykn"
                        let cleaned = trimmed.strip_prefix("./").unwrap_or(trimmed);
                        members.push(cleaned.to_string());
                    }
                }
            }
        }
    }
    members
}

/// Extract a string value for a key from a JSON string (simple, no serde).
fn json_extract(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let line = json.lines().find(|l| l.contains(&pattern))?;
    // Find value after the colon: "key": "value"
    let colon_pos = line.find(':')?;
    let after_colon = &line[colon_pos + 1..];
    let v_start = after_colon.find('"')? + 1;
    let v_rest = &after_colon[v_start..];
    let v_end = v_rest.find('"')?;
    Some(v_rest[..v_end].to_string())
}

/// Extract npm dependencies from deno.json "imports" field.
/// Maps "astring": "npm:astring@^1.9.0" → "astring": "^1.9.0"
/// Maps workspace imports like "lykn/": "./..." → "@lykn/lykn": "^version"
fn extract_npm_deps(json: &str, version: &str) -> Vec<(String, String)> {
    let mut deps = Vec::new();
    let mut in_imports = false;
    for line in json.lines() {
        if line.contains("\"imports\"") {
            in_imports = true;
            continue;
        }
        if in_imports {
            if line.contains('}') {
                break;
            }
            // Parse "pkg": "npm:pkg@^version"
            if let Some(colon) = line.find(':') {
                let key = line[..colon]
                    .trim()
                    .trim_matches('"')
                    .trim_matches(',')
                    .to_string();
                let val = line[colon + 1..]
                    .trim()
                    .trim_matches('"')
                    .trim_matches(',')
                    .to_string();
                if let Some(npm_spec) = val.strip_prefix("npm:") {
                    // npm:astring@^1.9.0 → astring, ^1.9.0
                    // strip "npm:"
                    if let Some(at) = npm_spec.rfind('@') {
                        let npm_name = &npm_spec[..at];
                        let npm_ver = &npm_spec[at + 1..];
                        deps.push((npm_name.to_string(), npm_ver.to_string()));
                    }
                } else if key.ends_with('/') && val.starts_with("./packages/") {
                    // Workspace import: "lykn/": "./packages/lang/"
                    // → depends on @lykn/<name>
                    let pkg_name = key.trim_end_matches('/');
                    deps.push((format!("@lykn/{pkg_name}"), format!("^{version}")));
                }
            }
        }
    }
    deps
}

/// Build an npm package for a single workspace member.
fn build_npm_for_package(pkg_dir: &str) {
    let deno_json_path = Path::new(pkg_dir).join("deno.json");
    let deno_json = fs::read_to_string(&deno_json_path).unwrap_or_else(|e| {
        eprintln!("error reading {}: {e}", deno_json_path.display());
        process::exit(1);
    });

    let name = json_extract(&deno_json, "name").unwrap_or_else(|| {
        eprintln!("error: no \"name\" in {}", deno_json_path.display());
        process::exit(1);
    });
    let version = json_extract(&deno_json, "version").unwrap_or("0.0.0".into());

    // npm package name: @lykn/lykn → dist dir: dist/npm-lykn
    // @lykn/browser → dist dir: dist/npm-browser
    let short_name = name.strip_prefix("@lykn/").unwrap_or(&name);
    let dist_name = format!("dist/npm-{short_name}");
    let dist = Path::new(&dist_name);

    // Clean and create
    let _ = fs::remove_dir_all(dist);
    fs::create_dir_all(dist).unwrap_or_else(|e| {
        eprintln!("error creating {}: {e}", dist.display());
        process::exit(1);
    });

    // Copy all .js files from the package directory
    let pkg_path = Path::new(pkg_dir);
    if let Ok(entries) = fs::read_dir(pkg_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "js") {
                let filename = path.file_name().unwrap();
                // Read, rewrite workspace imports for npm, write
                let content = fs::read_to_string(&path).unwrap_or_else(|e| {
                    eprintln!("error reading {}: {e}", path.display());
                    process::exit(1);
                });
                // Rewrite import map paths: from 'lang/...' → from '@lykn/lang/...'
                let content = content.replace("from 'lang/", "from '@lykn/lang/");
                let content = content.replace("from \"lang/", "from \"@lykn/lang/");
                write_file(&dist.join(filename), &content);
            }
        }
    }

    // Build dependencies from deno.json imports
    let deps = extract_npm_deps(&deno_json, &version);
    let deps_json = if deps.is_empty() {
        "{}".to_string()
    } else {
        let pairs: Vec<String> = deps
            .iter()
            .map(|(k, v)| format!("    \"{k}\": \"{v}\""))
            .collect();
        format!("{{\n{}\n  }}", pairs.join(",\n"))
    };

    let package_json = format!(
        r#"{{
  "name": "{name}",
  "version": "{version}",
  "type": "module",
  "main": "./mod.js",
  "exports": {{
    ".": "./mod.js"
  }},
  "files": ["*.js", "README.md", "LICENSE"],
  "keywords": ["lisp", "s-expression", "lykn"],
  "author": "Duncan McGreggor",
  "license": "Apache-2.0",
  "repository": {{
    "type": "git",
    "url": "https://github.com/oxur/lykn"
  }},
  "dependencies": {deps_json}
}}
"#
    );
    write_file(&dist.join("package.json"), &package_json);

    let _ = fs::copy("README.md", dist.join("README.md"));
    let _ = fs::copy("LICENSE", dist.join("LICENSE"));

    eprintln!("{name} npm package built in {dist_name}/ (v{version})");
}
