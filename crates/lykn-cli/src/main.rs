use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

mod compile;
mod doctest;

use lykn_cli::config;
use lykn_cli::dist;

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
        /// Check formatting without writing (exit 1 if unformatted)
        #[arg(short, long)]
        check: bool,
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
        /// Directory for compiled JS test output (reserved for future use)
        #[arg(long, hide = true)]
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
    /// Build browser bundle, npm package, or dist staging
    Build {
        /// Build the browser bundle (dist/lykn-browser.js)
        #[arg(long)]
        browser: bool,
        /// Build the npm package (dist/npm/) [deprecated: use --dist]
        #[arg(long)]
        npm: bool,
        /// Stage all workspace packages into dist/ for publishing
        #[arg(long)]
        dist: bool,
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
        /// Skip the build step (assume dist/ is already staged)
        #[arg(long)]
        no_build: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fmt { files, write, check } => cmd_fmt(&files, write, check),
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
        } => cmd_test(
            &patterns,
            docs.as_deref(),
            out_dir.as_deref(),
            compile_only,
            &deno_args,
        ),
        Commands::Lint { paths } => cmd_lint(&paths),
        Commands::New { name, path } => cmd_new(&name, path.as_deref()),
        Commands::Build { browser, npm, dist } => cmd_build(browser, npm, dist),
        Commands::Publish {
            jsr,
            npm,
            dry_run,
            no_build,
        } => cmd_publish(jsr, npm, dry_run, no_build),
    }
}

fn cmd_fmt(files: &[PathBuf], write: bool, check: bool) {
    if files.is_empty() {
        eprintln!("Usage: lykn fmt <file.lykn>");
        process::exit(1);
    }

    let mut unformatted = 0;

    for path in files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {e}", path.display());
                process::exit(1);
            }
        };

        let exprs = match lykn_cli::reader::read(&source) {
            Ok(exprs) => exprs,
            Err(e) => {
                eprintln!("{}: {e}", path.display());
                process::exit(1);
            }
        };
        let formatted = lykn_cli::formatter::format_exprs(&exprs, 0);

        if check {
            if source != formatted {
                eprintln!("{}: not formatted", path.display());
                unformatted += 1;
            }
        } else if write {
            if let Err(e) = std::fs::write(path, &formatted) {
                eprintln!("error writing {}: {e}", path.display());
                process::exit(1);
            }
            eprintln!("{}: formatted", path.display());
        } else {
            print!("{formatted}");
        }
    }

    if check && unformatted > 0 {
        eprintln!("{unformatted} file(s) not formatted");
        process::exit(1);
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

        match lykn_cli::reader::read(&source) {
            Ok(exprs) => {
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
            Err(e) => {
                eprintln!("{}: error: {e}", path.display());
                process::exit(1);
            }
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
fn find_config_in(start: &Path, filenames: &[&str]) -> Option<PathBuf> {
    let fnames = filenames.to_vec();
    lykn_cli::util::walk_up_find(start, |dir| fnames.iter().any(|f| dir.join(f).exists()))
        .and_then(|dir| filenames.iter().map(|f| dir.join(f)).find(|p| p.exists()))
}

fn find_config() -> String {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_config_in(&cwd, &["project.json", "deno.json"])
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project.json".to_string())
}

/// Execute a deno command, returning its exit code.
fn run_deno(args: &[&str]) -> i32 {
    let status = Command::new("deno")
        .args(args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run deno: {e}");
            eprintln!("is deno installed? try: brew install deno");
            process::exit(1);
        });
    status.code().unwrap_or(1)
}

/// Execute a deno command, exiting with its status code.
fn exec_deno(args: &[&str]) {
    process::exit(run_deno(args));
}

fn cmd_run(file: &std::path::Path, args: &[String]) {
    let config = find_config();

    if file.extension().is_some_and(lykn_cli::util::is_lykn_ext) {
        // Compile .lykn/.lyk to temp .js, then run
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
    _out_dir: Option<&Path>,
    compile_only: bool,
    extra_deno_args: &[String],
) {
    let config = find_config();

    // Handle --docs mode: extract and test Markdown code blocks
    if let Some(docs_path) = docs {
        // If there are also .lykn patterns, compile them first
        let lykn_files = discover_lykn_test_files(patterns);
        if !lykn_files.is_empty() {
            let compiled = compile_lykn_test_files(&lykn_files, None);
            if compile_only {
                eprintln!("Compiled {} .lykn test file(s).", compiled.len());
            } else {
                let test_paths: Vec<String> = compiled
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect();
                run_deno_test(&config, &test_paths, extra_deno_args);
            }
            clean_compiled_test_files(&compiled);
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
        // Compile .lykn files next to sources, run, then clean up
        let compiled = compile_lykn_test_files(&lykn_files, None);
        eprintln!("Compiled {} .lykn test file(s).", compiled.len());

        if compile_only {
            return;
        }

        // Run tests from the original directories
        let mut paths: Vec<String> = Vec::new();
        for pattern in patterns {
            let p = Path::new(pattern);
            if p.is_dir() {
                paths.push(pattern.clone());
            }
        }
        if paths.is_empty() {
            paths = compiled
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
        }

        let path_refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        let mut deno_args = vec!["test", "--config", &config, "--no-check", "-A"];
        deno_args.extend(path_refs);
        let extra_refs: Vec<&str> = extra_deno_args.iter().map(|s| s.as_str()).collect();
        deno_args.extend(extra_refs);
        let exit_code = run_deno(&deno_args);

        clean_compiled_test_files(&compiled);
        process::exit(exit_code);
    }
}

fn clean_compiled_test_files(compiled: &[PathBuf]) {
    for path in compiled {
        let _ = fs::remove_file(path);
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
            results.extend(lykn_cli::util::collect_files_recursive(path, |p| {
                is_lykn_test_file(p)
            }));
        }
    }
    results.sort();
    results
}

/// Check whether a path matches lykn test file naming conventions.
///
/// Matches: `*_test.lykn`, `*.test.lykn`, `*_test.lyk`, `*.test.lyk`,
/// and any `.lykn`/`.lyk` file inside a `__tests__` directory.
fn is_lykn_test_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with("_test.lykn")
        || name.ends_with(".test.lykn")
        || name.ends_with("_test.lyk")
        || name.ends_with(".test.lyk")
    {
        return true;
    }
    if lykn_cli::util::has_lykn_ext(path)
        && let Some(parent) = path.parent()
    {
        return parent.components().any(|c| c.as_os_str() == "__tests__");
    }
    false
}

/// Compile a list of `.lykn` test files to JavaScript.
///
/// Returns the list of compiled `.js` file paths.
fn compute_compiled_path(lykn_path: &Path, out_dir: Option<&Path>) -> PathBuf {
    if let Some(od) = out_dir {
        let relative = lykn_path.strip_prefix(".").unwrap_or(lykn_path);
        od.join(relative).with_extension("js")
    } else {
        lykn_path.with_extension("js")
    }
}

fn compile_lykn_test_files(files: &[PathBuf], out_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut compiled = Vec::new();

    for lykn_path in files {
        let js_path = compute_compiled_path(lykn_path, out_dir);
        if let Some(parent) = js_path.parent()
            && let Some(od) = out_dir
            && js_path.starts_with(od)
            && let Err(e) = fs::create_dir_all(parent)
        {
            eprintln!("error creating directory {}: {e}", parent.display());
            process::exit(1);
        }

        // Use the JS compiler via Deno (supports surface-macros, testing DSL, etc.)
        let config = find_config();
        let lykn_str = lykn_path.to_string_lossy();
        let js_str = js_path.to_string_lossy();
        let script = format!(
            "import {{ lykn }} from 'lang/mod.js';\n\
             const source = Deno.readTextFileSync({:?});\n\
             const js = lykn(source);\n\
             Deno.writeTextFileSync({:?}, js);\n",
            lykn_str, js_str,
        );
        let status = Command::new("deno")
            .args(["eval", "--config", &config, &script])
            .status()
            .unwrap_or_else(|e| {
                eprintln!("failed to run deno: {e}");
                process::exit(1);
            });
        if status.success() {
            eprintln!("  {} -> {}", lykn_path.display(), js_path.display());
            compiled.push(js_path);
        } else {
            eprintln!("error compiling {}", lykn_path.display());
            process::exit(status.code().unwrap_or(1));
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

fn resolve_publish_targets(jsr: bool, npm: bool) -> (bool, bool) {
    (jsr || !npm, npm)
}

fn cmd_publish(jsr: bool, npm: bool, dry_run: bool, no_build: bool) {
    let (do_jsr, do_npm) = resolve_publish_targets(jsr, npm);

    // Build dist/ unless --no-build was passed
    if !no_build && (do_jsr || do_npm) {
        match dist::build_dist(Path::new(".")) {
            Ok(packages) => {
                for pkg in &packages {
                    eprintln!("{} staged in dist/{}/", pkg.name, pkg.short_name);
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
    }

    if do_jsr {
        let config = "dist/project.json".to_string();
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
        // Find all subdirectories of dist/ that contain package.json
        let npm_dirs: Vec<_> = fs::read_dir("dist")
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_type().is_ok_and(|t| t.is_dir()) && e.path().join("package.json").exists()
            })
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect();
        for dir in &npm_dirs {
            if dry_run {
                eprintln!("npm dry run -- checking {dir}...");
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

fn check_project_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("project name cannot be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("project name must be kebab-case (lowercase letters, digits, hyphens)");
    }
    if name.starts_with('-') || name.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("project name must start with a letter");
    }
    Ok(())
}

fn validate_project_name(name: &str) {
    if let Err(msg) = check_project_name(name) {
        eprintln!("error: {msg}");
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
    "exports": "./mod.js",
    "lykn": {{
        "kind": "runtime"
    }}
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
        r#"(import-macros "jsr:@lykn/testing" (test is-equal))

(test "{name}: placeholder test"
  (is-equal (+ 1 1) 2))
"#
    )
}

fn readme_template(name: &str) -> String {
    format!(
        r#"# {name}

A [lykn](https://github.com/lykn-lang/lykn) project.

## Quick Start

```sh
lykn run packages/{name}/mod.lykn
lykn test
```

## License

Apache-2.0
"#
    )
}

const LICENSE_TEMPLATE: &str = "\
                                 Apache License\n\
                           Version 2.0, January 2004\n\
                        http://www.apache.org/licenses/\n\
\n\
   Licensed under the Apache License, Version 2.0 (the \"License\");\n\
   you may not use this file except in compliance with the License.\n\
   You may obtain a copy of the License at\n\
\n\
       http://www.apache.org/licenses/LICENSE-2.0\n\
\n\
   Unless required by applicable law or agreed to in writing, software\n\
   distributed under the License is distributed on an \"AS IS\" BASIS,\n\
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.\n\
   See the License for the specific language governing permissions and\n\
   limitations under the License.\n";

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
        &project_dir.join("test").join("mod_test.lykn"),
        &test_template(name),
    );
    write_file(&project_dir.join("README.md"), &readme_template(name));
    write_file(&project_dir.join("LICENSE"), LICENSE_TEMPLATE);
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

fn cmd_build(browser: bool, npm: bool, dist_flag: bool) {
    if !browser && !npm && !dist_flag {
        eprintln!("Usage: lykn build --browser | --npm | --dist");
        process::exit(1);
    }
    if browser {
        build_browser_bundle();
    }
    if npm {
        eprintln!("warning: --npm is deprecated; use --dist instead");
    }
    if npm || dist_flag {
        match dist::build_dist(Path::new(".")) {
            Ok(packages) => {
                for pkg in &packages {
                    eprintln!("{} staged in dist/{}/", pkg.name, pkg.short_name);
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- check_project_name --

    #[test]
    fn valid_project_names() {
        assert!(check_project_name("my-app").is_ok());
        assert!(check_project_name("hello").is_ok());
        assert!(check_project_name("app2").is_ok());
        assert!(check_project_name("a").is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        assert!(check_project_name("").is_err());
    }

    #[test]
    fn uppercase_rejected() {
        assert!(check_project_name("MyApp").is_err());
    }

    #[test]
    fn leading_hyphen_rejected() {
        assert!(check_project_name("-app").is_err());
    }

    #[test]
    fn leading_digit_rejected() {
        assert!(check_project_name("3app").is_err());
    }

    #[test]
    fn special_chars_rejected() {
        assert!(check_project_name("my_app").is_err());
        assert!(check_project_name("my.app").is_err());
        assert!(check_project_name("my app").is_err());
    }

    // -- resolve_publish_targets --

    #[test]
    fn publish_defaults_to_jsr() {
        assert_eq!(resolve_publish_targets(false, false), (true, false));
    }

    #[test]
    fn publish_jsr_only() {
        assert_eq!(resolve_publish_targets(true, false), (true, false));
    }

    #[test]
    fn publish_npm_only() {
        assert_eq!(resolve_publish_targets(false, true), (false, true));
    }

    #[test]
    fn publish_both() {
        assert_eq!(resolve_publish_targets(true, true), (true, true));
    }

    // -- compute_compiled_path --

    #[test]
    fn compiled_path_no_out_dir() {
        let result = compute_compiled_path(Path::new("test/foo_test.lykn"), None);
        assert_eq!(result, PathBuf::from("test/foo_test.js"));
    }

    #[test]
    fn compiled_path_with_out_dir() {
        let result =
            compute_compiled_path(Path::new("test/foo_test.lykn"), Some(Path::new("/tmp/out")));
        assert_eq!(result, PathBuf::from("/tmp/out/test/foo_test.js"));
    }

    #[test]
    fn compiled_path_strips_dot_prefix() {
        let result =
            compute_compiled_path(Path::new("./test/bar.lykn"), Some(Path::new("/tmp/out")));
        assert_eq!(result, PathBuf::from("/tmp/out/test/bar.js"));
    }

    #[test]
    fn compiled_path_lyk_extension() {
        let result = compute_compiled_path(Path::new("mod.lyk"), None);
        assert_eq!(result, PathBuf::from("mod.js"));
    }

    // -- find_config_in --

    #[test]
    fn find_config_in_returns_none_for_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(find_config_in(tmp.path(), &["project.json", "deno.json"]).is_none());
    }

    #[test]
    fn find_config_in_finds_project_json() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("project.json"), "{}").unwrap();
        let result = find_config_in(tmp.path(), &["project.json", "deno.json"]);
        assert_eq!(result, Some(tmp.path().join("project.json")));
    }

    #[test]
    fn find_config_in_prefers_first_filename() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("project.json"), "{}").unwrap();
        std::fs::write(tmp.path().join("deno.json"), "{}").unwrap();
        let result = find_config_in(tmp.path(), &["project.json", "deno.json"]);
        assert_eq!(result, Some(tmp.path().join("project.json")));
    }

    #[test]
    fn find_config_in_walks_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let child = tmp.path().join("packages/app");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(tmp.path().join("project.json"), "{}").unwrap();
        let result = find_config_in(&child, &["project.json"]);
        assert_eq!(result, Some(tmp.path().join("project.json")));
    }

    // -- is_lykn_test_file --

    #[test]
    fn test_file_patterns() {
        assert!(is_lykn_test_file(Path::new("foo_test.lykn")));
        assert!(is_lykn_test_file(Path::new("foo.test.lykn")));
        assert!(is_lykn_test_file(Path::new("foo_test.lyk")));
        assert!(is_lykn_test_file(Path::new("foo.test.lyk")));
        assert!(!is_lykn_test_file(Path::new("foo.lykn")));
        assert!(!is_lykn_test_file(Path::new("foo.js")));
    }

    #[test]
    fn test_file_in_tests_dir() {
        assert!(is_lykn_test_file(Path::new("__tests__/anything.lykn")));
        assert!(is_lykn_test_file(Path::new("__tests__/anything.lyk")));
        assert!(!is_lykn_test_file(Path::new("__tests__/anything.js")));
    }

    // -- template functions --

    #[test]
    fn project_json_template_includes_name() {
        let tmpl = project_json_template("my-app");
        assert!(tmpl.contains("./packages/my-app"));
        assert!(tmpl.contains("\"my-app/\""));
    }

    #[test]
    fn deno_json_template_includes_name() {
        let tmpl = deno_json_template("my-app");
        assert!(tmpl.contains("@my-app/my-app"));
        assert!(tmpl.contains("0.1.0"));
    }

    #[test]
    fn mod_lykn_template_includes_name() {
        let tmpl = mod_lykn_template("my-app");
        assert!(tmpl.contains("my-app"));
        assert!(tmpl.contains("Hello from"));
    }
}
