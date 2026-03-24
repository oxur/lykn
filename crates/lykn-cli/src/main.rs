mod reader;
mod formatter;

use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        process::exit(0);
    }

    match args[1].as_str() {
        "fmt" => cmd_fmt(&args[2..]),
        "check" => cmd_check(&args[2..]),
        "--help" | "-h" | "help" => print_help(),
        "--version" | "-V" => println!("lykn {}", env!("CARGO_PKG_VERSION")),
        other => {
            eprintln!("Unknown command: {}", other);
            eprintln!("Run 'lykn --help' for usage.");
            process::exit(1);
        }
    }
}

fn cmd_fmt(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: lykn fmt <file.lykn>");
        process::exit(1);
    }

    for path in args {
        let source = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path, e);
            process::exit(1);
        });

        let exprs = reader::read(&source);
        let formatted = formatter::format_exprs(&exprs, 0);

        if args.iter().any(|a| a == "--write" || a == "-w") {
            fs::write(path, &formatted).unwrap_or_else(|e| {
                eprintln!("Error writing {}: {}", path, e);
                process::exit(1);
            });
            eprintln!("Formatted {}", path);
        } else {
            print!("{}", formatted);
        }
    }
}

fn cmd_check(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: lykn check <file.lykn>");
        process::exit(1);
    }

    for path in args {
        let source = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path, e);
            process::exit(1);
        });

        let exprs = reader::read(&source);
        if exprs.is_empty() && !source.trim().is_empty() {
            eprintln!("{}: parse warning — no expressions found", path);
        } else {
            eprintln!("{}: ok ({} top-level expressions)", path, exprs.len());
        }
    }
}

fn print_help() {
    println!(
        "lykn — s-expression syntax for JavaScript

Usage:
  lykn fmt <file.lykn>           Format a .lykn file (stdout)
  lykn fmt -w <file.lykn>        Format in place
  lykn check <file.lykn>         Syntax check
  lykn --version                 Show version
  lykn --help                    Show this help"
    );
}
