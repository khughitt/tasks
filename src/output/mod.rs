mod contract;
mod goals;
mod meta;
mod project;
mod tables;

pub use contract::*;
pub use meta::defers_width_error;
pub use project::color_of;

use crate::error::{Error, Result};
use crate::present;
use crate::style::Painter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Pretty,
}

impl Format {
    /// The output format from `--json`, `--pretty`, and `TASKS_FORMAT`. The variable is
    /// validated whenever it is set: a flag overrides a valid value, never the check on
    /// an invalid one.
    pub fn resolve(json: bool, pretty: bool, tasks_format: Option<&str>) -> Result<Format> {
        let from_env = match tasks_format {
            None | Some("json") => Format::Json,
            Some("pretty") => Format::Pretty,
            Some(other) => {
                return Err(Error::Config(format!(
                    "TASKS_FORMAT must be json or pretty, got {other:?}"
                )));
            }
        };
        Ok(match (json, pretty) {
            (true, _) => Format::Json,
            (false, true) => Format::Pretty,
            (false, false) => from_env,
        })
    }
}

pub fn render(
    out: &Output,
    format: Format,
    painter: &Painter,
    layout: &crate::table::Layout,
) -> String {
    if let Output::Doc(doc) = out {
        return doc.body.clone();
    }
    match format {
        Format::Json => serde_json::to_string(out).expect("output serializes"),
        Format::Pretty => tables::pretty(out, painter, layout),
    }
}

pub fn render_error(e: &Error) -> String {
    serde_json::json!({ "error": { "kind": e.kind(), "detail": e.to_string() } }).to_string()
}

/// stderr text for warnings in pretty mode: a `── warnings (n)` rule, then one per line.
pub fn pretty_warnings(warnings: &[String], painter: &Painter, width: Option<usize>) -> String {
    if warnings.is_empty() {
        return String::new();
    }
    let mut rendered = present::rule(
        &format!("warnings ({})", warnings.len()),
        painter,
        width.unwrap_or(crate::show_text::MEASURE),
    );
    for warning in warnings {
        rendered.push_str(warning);
        rendered.push('\n');
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_resolution_validates_the_env_var_whenever_it_is_set() {
        for (json, pretty, env, expected) in [
            (false, false, None, Format::Json),
            (false, false, Some("json"), Format::Json),
            (false, false, Some("pretty"), Format::Pretty),
            (false, true, None, Format::Pretty),
            (false, true, Some("json"), Format::Pretty),
            (false, true, Some("pretty"), Format::Pretty),
            (true, false, None, Format::Json),
            (true, false, Some("json"), Format::Json),
            (true, false, Some("pretty"), Format::Json),
        ] {
            assert_eq!(
                Format::resolve(json, pretty, env).unwrap(),
                expected,
                "--json {json}, --pretty {pretty}, TASKS_FORMAT {env:?}"
            );
        }
        for (json, pretty) in [(false, false), (false, true), (true, false)] {
            let error = Format::resolve(json, pretty, Some("xml"))
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("TASKS_FORMAT must be json or pretty"),
                "--json {json}, --pretty {pretty}: {error}"
            );
        }
    }
}
