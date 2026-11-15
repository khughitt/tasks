//! Read-only session policy on relay's projected subscriber envelope.
mod claim_guard;
mod facts;
mod halt_notice;
mod input;

use serde_json::json;
use std::io::{Read, Write};
const OWNER: &str =
    "(owner: tasks; a wrong or unclear block is feedback, see Feedback in your instructions)";
const OVERFLOW: &str = "claim-guard: the claim guidance exceeds the reply limit. Run `tasks claims --all-projects` and act on your own claims before ending the turn.";

fn read(input: &mut dyn Read) -> Result<input::HookInput, String> {
    let mut bytes = Vec::new();
    input
        .take((input::MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "input could not be read")?;
    input::parse(&bytes)
}
fn bounded(text: &str) -> bool {
    text.encode_utf16().count() <= 10_000
}

pub(crate) fn claim_guard(
    input: &mut dyn Read,
    output: &mut dyn Write,
    diagnostics: &mut dyn Write,
) -> i32 {
    let decision = match read(input) {
        Ok(event) => claim_guard::decide(&event, claim_guard::read_claims, facts::read),
        Err(error) => claim_guard::GuardDecision::Deny(format!(
            "claim-guard: {error}; if you hold a task claim, act on it before ending the turn."
        )),
    };
    let reply = match decision {
        claim_guard::GuardDecision::Allow => json!({"decision":"allow"}),
        claim_guard::GuardDecision::Deny(reason) => {
            let reason = format!("{reason}\n{OWNER}");
            let reason = if bounded(&reason) {
                reason
            } else {
                format!("{OVERFLOW}\n{OWNER}")
            };
            json!({"decision":"deny","reason":reason})
        }
    };
    if serde_json::to_writer(output, &reply).is_err() {
        let _ = writeln!(diagnostics, "claim-guard: reply could not be written");
        return 1;
    }
    0
}

pub(crate) fn halt_notice(
    input: &mut dyn Read,
    output: &mut dyn Write,
    diagnostics: &mut dyn Write,
) -> i32 {
    let result = read(input)
        .and_then(|event| halt_notice::notice(&event))
        .and_then(|context| {
            if context.as_deref().is_some_and(|text| !bounded(text)) {
                return Err("context exceeds the reply limit".into());
            }
            let reply = match context {
                Some(text) => json!({"context":text}),
                None => json!({}),
            };
            serde_json::to_writer(output, &reply).map_err(|_| "reply could not be written".into())
        });
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(diagnostics, "halt-notice: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reply_counts_utf16_units() {
        assert!(bounded(&"\u{1f600}".repeat(5000)));
        assert!(!bounded(&"\u{1f600}".repeat(5001)));
    }
    #[test]
    fn malformed_guard_input_is_a_deny() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(claim_guard(&mut &b"not JSON"[..], &mut out, &mut err), 0);
        let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(doc["decision"], "deny");
        assert!(err.is_empty());
    }
    #[test]
    fn unreadable_stdin_is_explicit_without_quoting_its_error() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("fixture secret"))
            }
        }
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(claim_guard(&mut Broken, &mut out, &mut err), 0);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&out).unwrap()["decision"],
            "deny"
        );
        assert!(!String::from_utf8(out).unwrap().contains("secret"));
        let mut out = Vec::new();
        assert_eq!(halt_notice(&mut Broken, &mut out, &mut err), 1);
        assert!(out.is_empty());
        assert!(!String::from_utf8(err).unwrap().contains("secret"));
    }
    #[test]
    fn write_failure_is_explicit() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut err = Vec::new();
        assert_eq!(claim_guard(&mut &b"{}"[..], &mut Broken, &mut err), 1);
        assert!(!err.is_empty());
    }
}
