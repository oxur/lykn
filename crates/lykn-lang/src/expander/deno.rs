//! Deno subprocess management for macro compilation and evaluation.
//!
//! The expander delegates JavaScript execution to a long-lived Deno child
//! process. Communication uses a JSON-line protocol over stdin/stdout:
//! the Rust side writes one JSON object per line and reads one JSON response
//! per line.

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Child, Command, Stdio};

use crate::ast::sexpr::SExpr;
use crate::error::LyknError;
use crate::reader::source_loc::SourceLoc;

use super::env;

/// A managed Deno subprocess that compiles and evaluates lykn macros.
///
/// The subprocess is spawned once and reused for all macro operations during
/// a single expansion run. It is killed when this struct is dropped.
pub struct DenoSubprocess {
    child: Child,
    stdin: BufWriter<std::process::ChildStdin>,
    stdout: BufReader<std::process::ChildStdout>,
}

impl std::fmt::Debug for DenoSubprocess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DenoSubprocess")
            .field("pid", &self.child.id())
            .finish()
    }
}

impl DenoSubprocess {
    /// Spawn a new Deno evaluator subprocess.
    ///
    /// The subprocess runs `deno eval` with the macro API and evaluator script
    /// injected. Returns an error if Deno is not installed or cannot be started.
    pub fn spawn() -> Result<Self, LyknError> {
        let script = format!("{}\n{}", env::MACRO_ENV_JS, env::DENO_EVALUATOR_JS);

        let mut child = Command::new("deno")
            .arg("eval")
            .arg("--ext=js")
            .arg(&script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    LyknError::Read {
                        message: "user-defined macros require Deno \u{2014} install from \
                                  https://deno.land"
                            .to_string(),
                        location: SourceLoc::default(),
                    }
                } else {
                    LyknError::Io(e)
                }
            })?;

        let stdin = BufWriter::new(child.stdin.take().expect("stdin was configured as piped"));
        let stdout = BufReader::new(child.stdout.take().expect("stdout was configured as piped"));

        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    /// Send a JSON request and read the JSON response.
    ///
    /// The protocol expects each response to contain an `"ok"` boolean field.
    /// On success (`ok: true`), the `"result"` field is returned.
    /// On failure (`ok: false`), the `"error"` message is wrapped in a
    /// [`LyknError`].
    fn request(&mut self, req: serde_json::Value) -> Result<serde_json::Value, LyknError> {
        let line = serde_json::to_string(&req).map_err(|e| LyknError::Read {
            message: format!("JSON serialization error: {e}"),
            location: SourceLoc::default(),
        })?;

        writeln!(self.stdin, "{line}").map_err(LyknError::Io)?;
        self.stdin.flush().map_err(LyknError::Io)?;

        let mut response_line = String::new();
        self.stdout
            .read_line(&mut response_line)
            .map_err(LyknError::Io)?;

        if response_line.is_empty() {
            return Err(LyknError::Read {
                message: "Deno subprocess closed unexpectedly (empty response)".to_string(),
                location: SourceLoc::default(),
            });
        }

        let response: serde_json::Value =
            serde_json::from_str(response_line.trim()).map_err(|e| LyknError::Read {
                message: format!(
                    "invalid response from Deno subprocess: {e}\nraw: {response_line}"
                ),
                location: SourceLoc::default(),
            })?;

        if response.get("ok").and_then(|v| v.as_bool()) == Some(true) {
            Ok(response["result"].clone())
        } else {
            let error_msg = response["error"].as_str().unwrap_or("unknown error");
            Err(LyknError::Read {
                message: format!("macro expansion error: {error_msg}"),
                location: SourceLoc::default(),
            })
        }
    }

    /// Compile a macro definition (in lykn source form) to a JavaScript
    /// function body string.
    ///
    /// The source is sent to Deno where the JS-side `compileMacroBody` function
    /// parses and compiles it. The resulting JS string is returned so that it
    /// can be stored in [`CompiledMacro`](super::CompiledMacro) for later
    /// evaluation.
    pub fn compile_macro(&mut self, macro_source: &str) -> Result<String, LyknError> {
        let req = serde_json::json!({
            "action": "compile",
            "source": macro_source,
        });
        let result = self.request(req)?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| LyknError::Read {
                message: "macro compilation returned non-string result".to_string(),
                location: SourceLoc::default(),
            })
    }

    /// Evaluate a previously compiled macro with the given arguments.
    ///
    /// Each argument is serialized to the JSON wire format, sent to Deno,
    /// and the result is deserialized back into an [`SExpr`].
    pub fn eval_macro(&mut self, js_body: &str, args: &[SExpr]) -> Result<SExpr, LyknError> {
        let args_json: Vec<serde_json::Value> =
            args.iter().map(env::sexpr_to_protocol_json).collect();
        let req = serde_json::json!({
            "action": "eval",
            "jsBody": js_body,
            "args": args_json,
        });
        let result = self.request(req)?;
        env::protocol_json_to_sexpr(&result)
    }

    /// Verify that the subprocess is alive and responsive.
    pub fn ping(&mut self) -> Result<(), LyknError> {
        let req = serde_json::json!({ "action": "ping" });
        self.request(req)?;
        Ok(())
    }
}

impl Drop for DenoSubprocess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: returns true if `deno` is available on PATH.
    fn deno_available() -> bool {
        Command::new("deno")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }

    #[test]
    fn test_spawn_fails_gracefully_without_deno() {
        // This test verifies the error path; it will only exercise the
        // NotFound branch if deno is not installed. If deno IS installed,
        // spawning will succeed and we just verify that.
        let result = DenoSubprocess::spawn();
        if deno_available() {
            assert!(result.is_ok());
        }
        // If deno is not available, spawn returns an error — either way
        // we don't panic.
    }

    #[test]
    fn test_ping_when_deno_available() {
        if !deno_available() {
            eprintln!("skipping test_ping_when_deno_available: deno not found");
            return;
        }
        let mut deno = DenoSubprocess::spawn().expect("deno should spawn");
        deno.ping().expect("ping should succeed");
    }

    #[test]
    fn test_debug_impl() {
        if !deno_available() {
            eprintln!("skipping test_debug_impl: deno not found");
            return;
        }
        let deno = DenoSubprocess::spawn().expect("deno should spawn");
        let debug = format!("{deno:?}");
        assert!(debug.contains("DenoSubprocess"));
        assert!(debug.contains("pid"));
    }
}
