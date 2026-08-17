//! stdout / stderr contracts.
//!
//! JSON goes to stdout. Errors go to stderr (JSON when `--json`). Never mix
//! progress text into stdout.

use std::io::Write;
use std::process::ExitCode;

use adscli_api::ApiError;
use serde::Serialize;
use tabwriter::TabWriter;

pub fn write_json<T: Serialize>(v: &T) -> Result<(), ApiError> {
    let s = serde_json::to_string_pretty(v)
        .map_err(|e| ApiError::transport(format!("marshal JSON: {e}")))?;
    println!("{s}");
    Ok(())
}

pub fn write_table(header: &[&str], rows: &[Vec<String>]) -> Result<(), ApiError> {
    let stdout = std::io::stdout();
    let mut tw = TabWriter::new(stdout.lock()).minwidth(0).padding(3);
    let _ = writeln!(tw, "{}", header.join("\t"));
    let sep: Vec<String> = header.iter().map(|h| "-".repeat(h.len().max(3))).collect();
    let _ = writeln!(tw, "{}", sep.join("\t"));
    for r in rows {
        let _ = writeln!(tw, "{}", r.join("\t"));
    }
    tw.flush()
        .map_err(|e| ApiError::transport(format!("write table: {e}")))
}

pub fn write_quiet(values: &[String]) {
    for v in values {
        println!("{v}");
    }
}

pub fn emit_error(json: bool, err: &ApiError) -> ExitCode {
    if json {
        let _ = writeln!(std::io::stderr(), "{}", err.to_json());
    } else {
        let _ = writeln!(std::io::stderr(), "Error: {err}");
    }
    ExitCode::from(err.kind.exit_code())
}
