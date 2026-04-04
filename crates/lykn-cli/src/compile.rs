//! Compilation pipeline — reads lykn source and emits kernel JSON or JavaScript.
//!
//! The pipeline is: read -> expand -> classify -> analyze -> emit -> (bridge to JS).

use std::path::Path;

use lykn_lang::analysis;
use lykn_lang::classifier;
use lykn_lang::diagnostics::Severity;
use lykn_lang::emitter;
use lykn_lang::expander;
use lykn_lang::reader;

use super::bridge;

/// Compile a `.lykn` source file through the full pipeline.
///
/// Returns the compiled output as a string: either kernel JSON (when
/// `kernel_json_only` is `true`) or JavaScript (by bridging through Deno).
pub fn compile_file(
    path: &Path,
    strip_assertions: bool,
    kernel_json_only: bool,
) -> Result<String, String> {
    // 1. Read source
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("error reading {}: {e}", path.display()))?;

    // 2. Parse S-expressions
    let forms = reader::read(&source).map_err(|e| format!("{e}"))?;

    // 3. Expand macros
    let forms = expander::expand(forms, Some(path)).map_err(|e| format!("{e}"))?;

    // 4. Classify into surface forms
    let classified = classifier::classify(&forms).map_err(|diags| {
        diags
            .iter()
            .map(|d| format!("{d}"))
            .collect::<Vec<_>>()
            .join("\n")
    })?;

    // 5. Run static analysis (builds its own type registry internally)
    let analysis_result = analysis::analyze(&classified);

    if analysis_result.has_errors {
        let msgs: Vec<String> = analysis_result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| format!("{d}"))
            .collect();
        return Err(msgs.join("\n"));
    }

    // Print warnings to stderr
    for diag in &analysis_result.diagnostics {
        if diag.severity == Severity::Warning {
            eprintln!("{diag}");
        }
    }

    // 6. Emit kernel forms using the registry from analysis
    let kernel = emitter::emit(
        &classified,
        &analysis_result.type_registry,
        strip_assertions,
    );

    // 7. Output
    if kernel_json_only {
        Ok(emitter::json::emit_module_json(&kernel))
    } else {
        let kernel_json = emitter::json::emit_module_json(&kernel);
        bridge::kernel_json_to_js(&kernel_json, path)
    }
}
