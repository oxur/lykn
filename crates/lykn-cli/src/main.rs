use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::{self, Command};

mod compile;

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
        /// Test file patterns (default: test/)
        #[arg(default_value = "test/")]
        patterns: Vec<String>,
    },
    /// Lint compiled JS via Deno
    Lint {
        /// Paths to lint (default: packages/)
        #[arg(default_value = "packages/")]
        paths: Vec<String>,
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
        Commands::Test { patterns } => cmd_test(&patterns),
        Commands::Lint { paths } => cmd_lint(&paths),
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

fn cmd_test(patterns: &[String]) {
    let config = find_config();
    let mut deno_args = vec!["test", "--config", &config, "--no-check", "-A"];
    let refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
    deno_args.extend(refs);
    exec_deno(&deno_args);
}

fn cmd_lint(paths: &[String]) {
    let config = find_config();
    let mut deno_args = vec!["lint", "--config", &config];
    let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    deno_args.extend(refs);
    exec_deno(&deno_args);
}
