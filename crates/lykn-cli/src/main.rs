use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

mod bridge;
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
