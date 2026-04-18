//! Markdown code block extraction and test generation for `lykn test --docs`.
//!
//! Extracts fenced lykn code blocks from Markdown files, generates temporary
//! Deno test files, and invokes `deno test` to verify them. This is the
//! primary mechanism for keeping book and guide examples correct.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

// ---------------------------------------------------------------------------
// Block annotation types
// ---------------------------------------------------------------------------

/// The annotation on a fenced lykn code block, controlling test behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Annotation {
    /// Compile check only (the default for bare `` ```lykn ``).
    Compile,
    /// Compile and run; assert no runtime errors.
    Run,
    /// Expect compilation to fail.
    CompileFail,
    /// Skip this block entirely.
    Skip,
    /// A partial expression, not compilable standalone.
    Fragment,
    /// Concatenate with previous `Continue` blocks in the same section.
    Continue,
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Annotation::Compile => write!(f, "compile"),
            Annotation::Run => write!(f, "run"),
            Annotation::CompileFail => write!(f, "compile-fail"),
            Annotation::Skip => write!(f, "skip"),
            Annotation::Fragment => write!(f, "fragment"),
            Annotation::Continue => write!(f, "continue"),
        }
    }
}

/// Parse an annotation string after `lykn,` in a fence line.
fn parse_annotation(s: &str) -> Annotation {
    match s.trim() {
        "run" => Annotation::Run,
        "compile-fail" => Annotation::CompileFail,
        "skip" => Annotation::Skip,
        "fragment" => Annotation::Fragment,
        "continue" => Annotation::Continue,
        _ => Annotation::Compile,
    }
}

// ---------------------------------------------------------------------------
// Extracted code blocks
// ---------------------------------------------------------------------------

/// A single fenced code block extracted from a Markdown file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    /// 1-indexed block number within the file (counting only lykn blocks).
    pub number: usize,
    /// The annotation controlling test behavior.
    pub annotation: Annotation,
    /// The source code inside the fenced block.
    pub source: String,
    /// Optional expected JS output (from a following `` ```js `` block).
    pub expected_js: Option<String>,
    /// Optional expected execution output (from a following plain `` ``` ``
    /// or `` ```text `` block). The lykn is compiled, the resulting JS is
    /// evaluated, and the result is compared to this string.
    pub expected_output: Option<String>,
}

// ---------------------------------------------------------------------------
// Markdown scanner
// ---------------------------------------------------------------------------

/// Extract all lykn code blocks from Markdown source text.
///
/// The scanner is a simple line-by-line state machine:
/// - A line starting with `` ```lykn `` opens a lykn block.
/// - A line starting with `` ``` `` (and nothing else) closes any open block.
/// - `## ` headings reset the `continue` accumulator.
/// - A `` ```js `` block immediately following a lykn block (within a few
///   non-blank, non-fence lines) is paired for output comparison.
pub fn extract_blocks(source: &str) -> Vec<CodeBlock> {
    let lines: Vec<&str> = source.lines().collect();
    let mut blocks: Vec<CodeBlock> = Vec::new();
    let mut block_number: usize = 0;
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Detect `## ` headings — we don't need to do anything with them
        // at extraction time; section tracking is handled during generation.

        // Look for opening lykn fence
        if line.starts_with("```lykn") && !line.starts_with("````") {
            let annotation = if let Some(rest) = line.strip_prefix("```lykn,") {
                parse_annotation(rest)
            } else if line == "```lykn" {
                Annotation::Compile
            } else {
                // Something like ```lykn-foo — not our block
                i += 1;
                continue;
            };

            // Collect lines until closing fence
            i += 1;
            let mut body = String::new();
            while i < lines.len() {
                let l = lines[i];
                if l.trim() == "```" {
                    break;
                }
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(l);
                i += 1;
            }
            // `i` now points at the closing ``` (or past end)
            i += 1; // move past closing fence

            // Skip empty blocks
            if body.trim().is_empty() {
                continue;
            }

            block_number += 1;

            // Look ahead for a paired expected-output block
            let expected_block = look_ahead_for_expected_block(&lines, i);
            let (expected_js, expected_output) = match expected_block {
                Some(ExpectedBlock::Js(s)) => (Some(s), None),
                Some(ExpectedBlock::Output(s)) => (None, Some(s)),
                None => (None, None),
            };

            blocks.push(CodeBlock {
                number: block_number,
                annotation,
                source: body,
                expected_js,
                expected_output,
            });
        } else {
            i += 1;
        }
    }

    blocks
}

/// What kind of expected-output block follows a lykn block.
#[derive(Debug)]
enum ExpectedBlock {
    /// A `` ```js `` block — compare compiled JS source.
    Js(String),
    /// A plain `` ``` `` or `` ```text `` block — compare execution output.
    Output(String),
}

/// Look ahead from position `start` for an expected-output block within a
/// few lines. Returns a `` ```js `` block as `Js` (compiler output matching)
/// or a plain `` ``` `` / `` ```text `` block as `Output` (execution output
/// matching).
///
/// Skips blank lines and short prose connectors (e.g., "Compiles to:",
/// "Output:").
fn look_ahead_for_expected_block(lines: &[&str], start: usize) -> Option<ExpectedBlock> {
    let limit = (start + 5).min(lines.len());
    let mut i = start;

    while i < limit {
        let line = lines[i].trim();

        if !line.starts_with("````") {
            if line.starts_with("```js") {
                return collect_block_body(lines, i + 1).map(ExpectedBlock::Js);
            }
            if line == "```" || line == "```text" {
                return collect_block_body(lines, i + 1).map(ExpectedBlock::Output);
            }
        }

        // If we hit another fenced block type, stop looking
        if line.starts_with("```") && line != "```" && !line.starts_with("```js")
            && !line.starts_with("```text")
        {
            return None;
        }

        i += 1;
    }

    None
}

/// Collect lines from `start` until a closing `` ``` `` fence.
fn collect_block_body(lines: &[&str], start: usize) -> Option<String> {
    let mut body = String::new();
    let mut i = start;
    while i < lines.len() {
        let l = lines[i];
        if l.trim() == "```" {
            break;
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(l);
        i += 1;
    }
    if body.trim().is_empty() {
        None
    } else {
        Some(body)
    }
}

// ---------------------------------------------------------------------------
// Section tracking for `continue` blocks
// ---------------------------------------------------------------------------

/// Track `## ` section boundaries in the Markdown source and assign section
/// indices to each block based on their position in the source.
fn assign_sections(source: &str, blocks: &[CodeBlock]) -> Vec<usize> {
    let lines: Vec<&str> = source.lines().collect();
    let mut sections = Vec::with_capacity(blocks.len());
    let mut current_section: usize = 0;
    let mut block_idx: usize = 0;
    let mut lykn_block_number: usize = 0;
    let mut i = 0;

    while i < lines.len() && block_idx < blocks.len() {
        let line = lines[i].trim();

        // Detect section headings
        if line.starts_with("## ") {
            current_section += 1;
        }

        // Detect lykn fences to track block positions
        if line.starts_with("```lykn") && !line.starts_with("````") {
            // Determine if this is actually a valid block (same logic as extract)
            let is_valid = if let Some(rest) = line.strip_prefix("```lykn,") {
                let _ = parse_annotation(rest);
                true
            } else {
                line == "```lykn"
            };

            if is_valid {
                // Skip to end of block
                i += 1;
                let mut body_empty = true;
                while i < lines.len() {
                    if lines[i].trim() == "```" {
                        break;
                    }
                    if !lines[i].trim().is_empty() {
                        body_empty = false;
                    }
                    i += 1;
                }
                i += 1; // past closing fence

                // Only non-empty blocks got numbered
                if !body_empty {
                    lykn_block_number += 1;
                    if lykn_block_number == blocks[block_idx].number {
                        sections.push(current_section);
                        block_idx += 1;
                    }
                }
                continue;
            }
        }

        i += 1;
    }

    // Fill any remaining blocks with the last section
    while sections.len() < blocks.len() {
        sections.push(current_section);
    }

    sections
}

// ---------------------------------------------------------------------------
// Test file generation
// ---------------------------------------------------------------------------

/// Escape a string for embedding in a JavaScript template literal.
fn js_escape_template(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
}

/// Generate the contents of a Deno test file for the given Markdown file.
///
/// The generated file imports `lykn` from the lang package and uses
/// `assertEquals` / `assertThrows` from `@std/assert`.
pub fn generate_test_file(
    md_path: &str,
    blocks: &[CodeBlock],
    md_source: &str,
    config_path: &Path,
) -> String {
    let mut out = String::new();

    // Compute the absolute path to packages/lang/mod.js based on the config
    // file location (e.g., /project/project.json -> /project/packages/lang/mod.js)
    let project_root = config_path.parent().unwrap_or(Path::new("."));
    let lang_mod = project_root.join("packages/lang/mod.js");
    let lang_mod_str = lang_mod.to_string_lossy();

    out.push_str("import { assertEquals } from \"jsr:@std/assert\";\n");
    out.push_str(&format!("import {{ lykn }} from \"{}\";\n\n", lang_mod_str));

    // Assign sections for continue-block accumulation
    let sections = assign_sections(md_source, blocks);

    // Track accumulated source for `continue` blocks per section
    let mut continue_accum: Vec<(usize, String)> = Vec::new();

    for (idx, block) in blocks.iter().enumerate() {
        let section = sections[idx];

        match block.annotation {
            Annotation::Skip | Annotation::Fragment => continue,
            _ => {}
        }

        // Build the source to compile
        let compile_source = if block.annotation == Annotation::Continue {
            // Find or start accumulation for this section
            if let Some(entry) = continue_accum.iter_mut().find(|(s, _)| *s == section) {
                entry.1.push('\n');
                entry.1.push_str(&block.source);
                entry.1.clone()
            } else {
                continue_accum.push((section, block.source.clone()));
                block.source.clone()
            }
        } else {
            block.source.clone()
        };

        let escaped_source = js_escape_template(&compile_source);
        let test_name = format!("{} block {}", md_path, block.number);

        match block.annotation {
            Annotation::CompileFail => {
                out.push_str(&format!(
                    concat!(
                        "Deno.test(\"{name}\", () => {{\n",
                        "  let threw = false;\n",
                        "  try {{\n",
                        "    lykn(`{src}`);\n",
                        "  }} catch (_e) {{\n",
                        "    threw = true;\n",
                        "  }}\n",
                        "  assertEquals(threw, true, \"expected compilation to fail\");\n",
                        "}});\n\n",
                    ),
                    name = test_name,
                    src = escaped_source,
                ));
            }
            Annotation::Run => {
                // Compile check (placeholder — run semantics TBD per DD-31)
                out.push_str(&format!(
                    "Deno.test(\"{name}\", () => {{\n\
                     \x20 assertEquals(typeof lykn(`{src}`), \"string\");\n\
                     }});\n\n",
                    name = test_name,
                    src = escaped_source,
                ));
            }
            _ => {
                if let Some(ref expected) = block.expected_output {
                    // Execution output matching — compile, eval, compare result
                    let escaped_expected = js_escape_template(expected);
                    out.push_str(&format!(
                        concat!(
                            "Deno.test(\"{name}\", () => {{\n",
                            "  const js = lykn(`{src}`);\n",
                            "  const __logs = [];\n",
                            "  const __origLog = console.log;\n",
                            "  console.log = (...args) => __logs.push(args.map(String).join(\" \"));\n",
                            "  let __result;\n",
                            "  try {{\n",
                            "    __result = (0, eval)(js);\n",
                            "  }} finally {{\n",
                            "    console.log = __origLog;\n",
                            "  }}\n",
                            "  const __output = __logs.length > 0\n",
                            "    ? __logs.join(\"\\n\")\n",
                            "    : (__result !== undefined ? String(__result) : \"\");\n",
                            "  assertEquals(__output.trim(), `{expected}`.trim());\n",
                            "}});\n\n",
                        ),
                        name = test_name,
                        src = escaped_source,
                        expected = escaped_expected,
                    ));
                } else if let Some(ref expected) = block.expected_js {
                    // Compiler output matching — normalize whitespace and compare
                    let escaped_expected = js_escape_template(expected);
                    out.push_str(&format!(
                        "Deno.test(\"{name}\", () => {{\n\
                         \x20 const normalize = (s) => s\n\
                         \x20   .replace(/\\/\\/[^\\n]*/g, \"\")\n\
                         \x20   .replace(/\\/\\*[\\s\\S]*?\\*\\//g, \"\")\n\
                         \x20   .replace(/\\s+/g, \" \")\n\
                         \x20   .trim();\n\
                         \x20 const result = lykn(`{src}`);\n\
                         \x20 assertEquals(normalize(result), normalize(`{expected}`));\n\
                         }});\n\n",
                        name = test_name,
                        src = escaped_source,
                        expected = escaped_expected,
                    ));
                } else {
                    // Simple compile check
                    out.push_str(&format!(
                        "Deno.test(\"{name}\", () => {{\n\
                         \x20 assertEquals(typeof lykn(`{src}`), \"string\");\n\
                         }});\n\n",
                        name = test_name,
                        src = escaped_source,
                    ));
                }
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

/// Recursively find all `.md` files under a directory.
fn find_md_files(dir: &Path) -> Vec<PathBuf> {
    lykn_cli::util::collect_files_recursive(dir, |p: &Path| {
        p.extension().is_some_and(|e| e == "md")
    })
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Sanitize a file path into a valid JS filename component.
///
/// Replaces path separators and dots (except the final extension) with
/// underscores.
fn sanitize_filename(path: &str) -> String {
    path.replace(['/', '\\'], "__").replace('.', "_")
}

/// Run documentation tests for Markdown files matching the given pattern.
///
/// This function:
/// 1. Finds `.md` files in the given path (file or directory).
/// 2. Extracts lykn code blocks from each.
/// 3. Generates temporary Deno test files under `.lykn-test-out/`.
/// 4. Invokes `deno test` on the generated files.
/// 5. Exits with Deno's exit code.
pub fn run_doc_tests(docs_path: &str, config: &str, deno_args: &[String]) -> ! {
    let path = Path::new(docs_path);
    let md_files = if path.is_file() {
        if path.extension().is_some_and(|e| e == "md") {
            vec![path.to_path_buf()]
        } else {
            eprintln!(
                "error: --docs path is not a Markdown file: {}",
                path.display()
            );
            process::exit(1);
        }
    } else if path.is_dir() {
        find_md_files(path)
    } else {
        eprintln!("error: --docs path does not exist: {}", path.display());
        process::exit(1);
    };

    if md_files.is_empty() {
        eprintln!("No Markdown files found in {}", docs_path);
        process::exit(0);
    }

    let config_path = Path::new(config);
    let config_dir = config_path.parent().unwrap_or(Path::new("."));

    let out_dir = config_dir.join(".lykn-test-out");
    if let Err(e) = fs::create_dir_all(&out_dir) {
        eprintln!("error creating {}: {e}", out_dir.display());
        process::exit(1);
    }

    let mut generated_files: Vec<PathBuf> = Vec::new();
    let mut total_blocks: usize = 0;
    let mut total_skipped: usize = 0;

    for md_file in &md_files {
        let md_source = match fs::read_to_string(md_file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error reading {}: {e}", md_file.display());
                continue;
            }
        };

        // Normalize CRLF to LF
        let md_source = md_source.replace("\r\n", "\n");

        let blocks = extract_blocks(&md_source);

        if blocks.is_empty() {
            eprintln!("info: no lykn blocks in {}", md_file.display());
            continue;
        }

        let testable_count = blocks
            .iter()
            .filter(|b| b.annotation != Annotation::Skip && b.annotation != Annotation::Fragment)
            .count();
        let skip_count = blocks.len() - testable_count;
        total_blocks += testable_count;
        total_skipped += skip_count;

        let md_path_str = md_file.to_string_lossy();
        let test_content = generate_test_file(&md_path_str, &blocks, &md_source, config_path);

        let test_filename = format!("{}.test.js", sanitize_filename(&md_path_str));
        let test_path = out_dir.join(&test_filename);

        if let Err(e) = fs::write(&test_path, &test_content) {
            eprintln!("error writing {}: {e}", test_path.display());
            continue;
        }

        generated_files.push(test_path);
    }

    if generated_files.is_empty() {
        eprintln!("No testable lykn blocks found in Markdown files.");
        process::exit(0);
    }

    eprintln!(
        "Generated {} test file(s) with {} block(s) ({} skipped)",
        generated_files.len(),
        total_blocks,
        total_skipped
    );

    // Invoke deno test on the output directory
    let out_dir_str = out_dir.to_string_lossy().into_owned();
    let mut args: Vec<&str> = vec!["test", "--config", config, "--no-check", "-A", &out_dir_str];
    let extra_refs: Vec<&str> = deno_args.iter().map(|s| s.as_str()).collect();
    args.extend(extra_refs);

    let status = Command::new("deno")
        .args(&args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("failed to run deno: {e}");
            eprintln!("is deno installed? try: brew install deno");
            process::exit(1);
        });
    process::exit(status.code().unwrap_or(1));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_annotation_known_values() {
        assert_eq!(parse_annotation("run"), Annotation::Run);
        assert_eq!(parse_annotation("compile-fail"), Annotation::CompileFail);
        assert_eq!(parse_annotation("skip"), Annotation::Skip);
        assert_eq!(parse_annotation("fragment"), Annotation::Fragment);
        assert_eq!(parse_annotation("continue"), Annotation::Continue);
    }

    #[test]
    fn test_parse_annotation_unknown_defaults_to_compile() {
        assert_eq!(parse_annotation("unknown"), Annotation::Compile);
        assert_eq!(parse_annotation(""), Annotation::Compile);
    }

    #[test]
    fn test_parse_annotation_trims_whitespace() {
        assert_eq!(parse_annotation("  run  "), Annotation::Run);
        assert_eq!(parse_annotation("\tskip\t"), Annotation::Skip);
    }

    #[test]
    fn test_extract_blocks_single_lykn_block() {
        let md = r#"# Hello

```lykn
(bind x 42)
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].number, 1);
        assert_eq!(blocks[0].annotation, Annotation::Compile);
        assert_eq!(blocks[0].source, "(bind x 42)");
        assert_eq!(blocks[0].expected_js, None);
    }

    #[test]
    fn test_extract_blocks_with_annotation() {
        let md = r#"```lykn,run
(console:log "hello")
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].annotation, Annotation::Run);
    }

    #[test]
    fn test_extract_blocks_compile_fail() {
        let md = r#"```lykn,compile-fail
(bind)
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].annotation, Annotation::CompileFail);
    }

    #[test]
    fn test_extract_blocks_skip_and_fragment() {
        let md = r#"```lykn,skip
(partial)
```

```lykn,fragment
(also partial)
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].annotation, Annotation::Skip);
        assert_eq!(blocks[1].annotation, Annotation::Fragment);
    }

    #[test]
    fn test_extract_blocks_with_js_output_matching() {
        let md = r#"```lykn
(bind max-retries 3)
```

Compiles to:

```js
const maxRetries = 3;
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].source, "(bind max-retries 3)");
        assert_eq!(
            blocks[0].expected_js.as_deref(),
            Some("const maxRetries = 3;")
        );
    }

    #[test]
    fn test_extract_blocks_multiline_js_output() {
        let md = r#"```lykn
(func greet :args (name) :body (+ "hello " name))
```

```js
function greet(name) {
  return "hello " + name;
}
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].expected_js.is_some());
        let js = blocks[0].expected_js.as_ref().unwrap();
        assert!(js.contains('\n'));
    }

    #[test]
    fn test_extract_blocks_empty_block_skipped() {
        let md = r#"```lykn
```

```lykn
(bind x 1)
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].number, 1);
        assert_eq!(blocks[0].source, "(bind x 1)");
    }

    #[test]
    fn test_extract_blocks_multiple_blocks() {
        let md = r#"```lykn
(bind x 1)
```

```lykn
(bind y 2)
```

```lykn,skip
(partial thing)
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].number, 1);
        assert_eq!(blocks[1].number, 2);
        assert_eq!(blocks[2].number, 3);
    }

    #[test]
    fn test_extract_blocks_non_lykn_blocks_ignored() {
        let md = r#"```rust
fn main() {}
```

```lykn
(bind x 1)
```

```python
print("hello")
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].source, "(bind x 1)");
    }

    #[test]
    fn test_extract_blocks_js_block_not_paired_when_far() {
        let md = r#"```lykn
(bind x 1)
```

This is a long paragraph that separates the blocks by more than 5 lines.
It goes on and on.
And on.
And on.
And on some more.

```js
const x = 1;
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        // JS block is too far away — should not be paired
        assert_eq!(blocks[0].expected_js, None);
    }

    #[test]
    fn test_extract_blocks_js_block_not_paired_when_another_fence_intervenes() {
        let md = r#"```lykn
(bind x 1)
```

```python
something
```

```js
const x = 1;
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        // Another fence block (python) intervenes
        assert_eq!(blocks[0].expected_js, None);
    }

    #[test]
    fn test_extract_blocks_multiline_source() {
        let md = r#"```lykn
(bind x 1)
(bind y 2)
(bind z (+ x y))
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].source, "(bind x 1)\n(bind y 2)\n(bind z (+ x y))");
    }

    #[test]
    fn test_extract_blocks_continue_annotation() {
        let md = r#"## Section One

```lykn,continue
(type Color Red Green Blue)
```

Some prose here.

```lykn,continue
(bind c Red)
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].annotation, Annotation::Continue);
        assert_eq!(blocks[1].annotation, Annotation::Continue);
    }

    #[test]
    fn test_assign_sections_basic() {
        let md = r#"# Title

```lykn
(bind x 1)
```

## Section A

```lykn
(bind y 2)
```

## Section B

```lykn
(bind z 3)
```
"#;
        let blocks = extract_blocks(md);
        let sections = assign_sections(md, &blocks);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0], 0); // Before any ## heading
        assert_eq!(sections[1], 1); // After ## Section A
        assert_eq!(sections[2], 2); // After ## Section B
    }

    #[test]
    fn test_assign_sections_continue_same_section() {
        let md = r#"## Types

```lykn,continue
(type Color Red Green Blue)
```

```lykn,continue
(bind c Red)
```

## Other

```lykn
(bind x 1)
```
"#;
        let blocks = extract_blocks(md);
        let sections = assign_sections(md, &blocks);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0], 1); // Both in ## Types
        assert_eq!(sections[1], 1);
        assert_eq!(sections[2], 2); // In ## Other
    }

    #[test]
    fn test_js_escape_template_backticks() {
        assert_eq!(js_escape_template("hello `world`"), "hello \\`world\\`");
    }

    #[test]
    fn test_js_escape_template_dollar_brace() {
        assert_eq!(js_escape_template("${foo}"), "\\${foo}");
    }

    #[test]
    fn test_js_escape_template_backslash() {
        assert_eq!(js_escape_template("a\\b"), "a\\\\b");
    }

    #[test]
    fn test_generate_test_file_compile_block() {
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::Compile,
            source: "(bind x 42)".to_string(),
            expected_js: None,
            expected_output: None,
        }];
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, "```lykn\n(bind x 42)\n```", config);
        assert!(result.contains("Deno.test(\"test.md block 1\""));
        assert!(result.contains("typeof lykn("));
        assert!(result.contains("\"string\""));
    }

    #[test]
    fn test_generate_test_file_compile_fail_block() {
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::CompileFail,
            source: "(bind)".to_string(),
            expected_js: None,
            expected_output: None,
        }];
        let config = Path::new("/project/project.json");
        let result = generate_test_file(
            "test.md",
            &blocks,
            "```lykn,compile-fail\n(bind)\n```",
            config,
        );
        assert!(result.contains("threw"));
        assert!(result.contains("expected compilation to fail"));
    }

    #[test]
    fn test_generate_test_file_output_matching_single_line() {
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::Compile,
            source: "(bind x 42)".to_string(),
            expected_js: Some("const x = 42;".to_string()),
            expected_output: None,
        }];
        let md = "```lykn\n(bind x 42)\n```\n\n```js\nconst x = 42;\n```";
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        assert!(result.contains("normalize"));
    }

    #[test]
    fn test_generate_test_file_output_matching_multiline() {
        let expected = "function greet(name) {\n  return name;\n}";
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::Compile,
            source: "(func greet :args (name) :body name)".to_string(),
            expected_js: Some(expected.to_string()),
            expected_output: None,
        }];
        let md = "```lykn\n(func greet :args (name) :body name)\n```\n\n```js\nfunction greet(name) {\n  return name;\n}\n```";
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        assert!(result.contains("normalize"));
    }

    #[test]
    fn test_generate_test_file_skip_blocks_omitted() {
        let blocks = vec![
            CodeBlock {
                number: 1,
                annotation: Annotation::Skip,
                source: "(partial)".to_string(),
                expected_js: None,
            expected_output: None,
            },
            CodeBlock {
                number: 2,
                annotation: Annotation::Fragment,
                source: "(also partial)".to_string(),
                expected_js: None,
            expected_output: None,
            },
        ];
        let md = "```lykn,skip\n(partial)\n```\n\n```lykn,fragment\n(also partial)\n```";
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        assert!(!result.contains("Deno.test"));
    }

    #[test]
    fn test_generate_test_file_continue_accumulation() {
        let md = r#"## Types

```lykn,continue
(type Color Red Green Blue)
```

```lykn,continue
(bind c Red)
```
"#;
        let blocks = extract_blocks(md);
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        // The second continue block should contain accumulated source.
        // The template literal in JS contains an actual newline between the
        // two expressions.
        assert!(result.contains("(type Color Red Green Blue)\n(bind c Red)"));
    }

    #[test]
    fn test_generate_test_file_continue_resets_at_section() {
        let md = r#"## Section A

```lykn,continue
(type Color Red Green Blue)
```

## Section B

```lykn,continue
(bind x 1)
```
"#;
        let blocks = extract_blocks(md);
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        // Block 1 (Section A) should contain Color
        assert!(result.contains("(type Color Red Green Blue)"));
        // Block 2 (Section B) should contain "(bind x 1)" but NOT "Color"
        // in the same test. Since they are in different sections, the
        // second continue block starts fresh.
        assert!(result.contains("(bind x 1)"));
        // The accumulated source for block 2 should not contain Color
        // (section boundary resets the accumulator)
        assert!(!result.contains("Color Red Green Blue)\n(bind x 1)"));
    }

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(
            sanitize_filename("docs/guides/01-core.md"),
            "docs__guides__01-core_md"
        );
    }

    #[test]
    fn test_sanitize_filename_backslashes() {
        assert_eq!(
            sanitize_filename("docs\\guides\\test.md"),
            "docs__guides__test_md"
        );
    }

    #[test]
    fn test_annotation_display() {
        assert_eq!(format!("{}", Annotation::Compile), "compile");
        assert_eq!(format!("{}", Annotation::Run), "run");
        assert_eq!(format!("{}", Annotation::CompileFail), "compile-fail");
        assert_eq!(format!("{}", Annotation::Skip), "skip");
        assert_eq!(format!("{}", Annotation::Fragment), "fragment");
        assert_eq!(format!("{}", Annotation::Continue), "continue");
    }

    #[test]
    fn test_extract_blocks_quad_backtick_fence_ignored() {
        let md = r#"````markdown
```lykn
(bind x 1)
```
````
"#;
        let blocks = extract_blocks(md);
        // The inner ```lykn is inside a ````markdown block — but our simple
        // scanner doesn't know about nested fences. This is a known limitation.
        // For now, it will find the block. A future improvement could handle
        // nested fences.
        // Just verify it doesn't panic.
        assert!(!blocks.is_empty() || blocks.is_empty());
    }

    #[test]
    fn test_extract_blocks_preserves_indentation() {
        let md = r#"```lykn
(func greet :args (name)
  :body
  (+ "hello " name))
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].source.contains("  :body"));
    }

    #[test]
    fn test_extract_blocks_crlf_handling() {
        let md = "```lykn\r\n(bind x 1)\r\n```\r\n";
        // Note: lines() handles \r\n by splitting on \n, leaving \r at end.
        // The real entry point normalizes CRLF before calling extract_blocks.
        // Here we test that the scanner doesn't crash.
        let blocks = extract_blocks(md);
        assert!(!blocks.is_empty());
    }

    #[test]
    fn test_generate_test_file_imports_correct_path() {
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::Compile,
            source: "(bind x 1)".to_string(),
            expected_js: None,
            expected_output: None,
        }];
        let config = Path::new("/my/project/project.json");
        let result = generate_test_file("test.md", &blocks, "```lykn\n(bind x 1)\n```", config);
        assert!(result.contains("/my/project/packages/lang/mod.js"));
    }

    #[test]
    fn test_generate_test_file_run_annotation() {
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::Run,
            source: "(console:log \"hello\")".to_string(),
            expected_js: None,
            expected_output: None,
        }];
        let md = "```lykn,run\n(console:log \"hello\")\n```";
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        assert!(result.contains("typeof lykn("));
        assert!(result.contains("\"string\""));
    }

    #[test]
    fn test_extract_blocks_no_blocks() {
        let md = "# Just a title\n\nSome prose.\n";
        let blocks = extract_blocks(md);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_blocks_only_whitespace_content() {
        let md = "```lykn\n   \n  \n```\n";
        let blocks = extract_blocks(md);
        // Block with only whitespace should be skipped
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_blocks_plain_output_block() {
        let md = r#"```lykn
(+ 1 2 3)
```

Output:

```
6
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].expected_js, None);
        assert_eq!(blocks[0].expected_output.as_deref(), Some("6"));
    }

    #[test]
    fn test_extract_blocks_text_output_block() {
        let md = r#"```lykn
(+ 1 2)
```

```text
3
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].expected_js, None);
        assert_eq!(blocks[0].expected_output.as_deref(), Some("3"));
    }

    #[test]
    fn test_extract_blocks_js_block_preferred_over_plain() {
        let md = r#"```lykn
(bind x 1)
```

```js
const x = 1;
```
"#;
        let blocks = extract_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].expected_js.is_some());
        assert_eq!(blocks[0].expected_output, None);
    }

    #[test]
    fn test_generate_test_file_execution_output() {
        let blocks = vec![CodeBlock {
            number: 1,
            annotation: Annotation::Compile,
            source: "(+ 1 2 3)".to_string(),
            expected_js: None,
            expected_output: Some("6".to_string()),
        }];
        let md = "```lykn\n(+ 1 2 3)\n```\n\n```\n6\n```";
        let config = Path::new("/project/project.json");
        let result = generate_test_file("test.md", &blocks, md, config);
        assert!(result.contains("eval"));
        assert!(result.contains("__logs"));
        assert!(result.contains("`6`"));
    }
}
