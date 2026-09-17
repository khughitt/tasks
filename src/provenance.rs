use crate::model::HarnessProvenance;
use std::ffi::OsString;

pub fn validate(provenance: &HarnessProvenance) -> Result<(), String> {
    let prefix = match provenance.harness_session_source.as_str() {
        "CLAUDE_CODE_SESSION_ID" => "claude-code:",
        "CODEX_SESSION_ID" | "CODEX_THREAD_ID" => "codex:",
        _ => return Err("harness_session_source must name a supported native variable".into()),
    };
    match provenance.harness_session.strip_prefix(prefix) {
        Some(id) if !id.is_empty() && !id.chars().any(char::is_control) => Ok(()),
        _ => Err(
            "harness_session must match its source and contain a nonempty, control-free native ID"
                .into(),
        ),
    }
}

pub fn resolve_from(
    get: impl Fn(&str) -> Option<OsString>,
) -> Result<Option<HarnessProvenance>, String> {
    let var = |name: &str| -> Result<Option<String>, String> {
        let Some(value) = get(name) else {
            return Ok(None);
        };
        let value = value
            .into_string()
            .map_err(|_| format!("unknown harness provenance: {name} is not Unicode"))?;
        if value.chars().any(char::is_control) {
            return Err(format!(
                "unknown harness provenance: {name} contains control characters"
            ));
        }
        Ok((!value.is_empty()).then_some(value))
    };
    let claude = var("CLAUDE_CODE_SESSION_ID")?;
    let session = var("CODEX_SESSION_ID")?;
    let thread = var("CODEX_THREAD_ID")?;
    let explicit = var("TASKS_SESSION")?;
    if let (Some(session), Some(thread)) = (&session, &thread)
        && session != thread
    {
        return Err(
            "unknown harness provenance: CODEX_SESSION_ID conflicts with CODEX_THREAD_ID".into(),
        );
    }
    let codex = session
        .map(|id| (id, "CODEX_SESSION_ID"))
        .or_else(|| thread.map(|id| (id, "CODEX_THREAD_ID")));
    let (key, source) = match (claude, codex) {
        (Some(_), Some((_, source))) => {
            return Err(format!(
                "unknown harness provenance: CLAUDE_CODE_SESSION_ID conflicts with {source}"
            ));
        }
        (Some(id), None) => (format!("claude-code:{id}"), "CLAUDE_CODE_SESSION_ID"),
        (None, Some((id, source))) => (format!("codex:{id}"), source),
        (None, None) => return Ok(None),
    };
    if let Some(explicit) = explicit
        && (explicit.starts_with("claude-code:") || explicit.starts_with("codex:"))
        && explicit != key
    {
        return Err(format!(
            "unknown harness provenance: TASKS_SESSION conflicts with {source}"
        ));
    }
    Ok(Some(HarnessProvenance {
        harness_session: key,
        harness_session_source: source.into(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    type Vars<'a> = &'a [(&'a str, &'a str)];

    #[test]
    fn native_sources_resolve_without_relabeling_claim_overrides() {
        let cases: &[(Vars<'_>, &str, &str)] = &[
            (
                &[("CLAUDE_CODE_SESSION_ID", "parent")],
                "claude-code:parent",
                "CLAUDE_CODE_SESSION_ID",
            ),
            (
                &[("CODEX_SESSION_ID", "opaque : id")],
                "codex:opaque : id",
                "CODEX_SESSION_ID",
            ),
            (&[("CODEX_THREAD_ID", "a")], "codex:a", "CODEX_THREAD_ID"),
            (
                &[("CODEX_SESSION_ID", "a"), ("CODEX_THREAD_ID", "a")],
                "codex:a",
                "CODEX_SESSION_ID",
            ),
            (
                &[("CODEX_SESSION_ID", ""), ("CODEX_THREAD_ID", "a")],
                "codex:a",
                "CODEX_THREAD_ID",
            ),
            (
                &[("CODEX_SESSION_ID", "a"), ("TASKS_SESSION", "codex:a")],
                "codex:a",
                "CODEX_SESSION_ID",
            ),
            (
                &[
                    ("CLAUDE_CODE_SESSION_ID", "parent"),
                    ("TASKS_SESSION", "child-claim"),
                ],
                "claude-code:parent",
                "CLAUDE_CODE_SESSION_ID",
            ),
            (
                &[
                    ("CLAUDE_CODE_SESSION_ID", "a"),
                    ("TASKS_SESSION", "claude-code:a"),
                ],
                "claude-code:a",
                "CLAUDE_CODE_SESSION_ID",
            ),
        ];
        for (vars, key, source) in cases {
            let got = resolve_from(|name| {
                assert_ne!(name, "TASKS_SESSION_PID");
                vars.iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| OsString::from(v))
            })
            .unwrap()
            .unwrap();
            assert_eq!(got.harness_session, *key);
            assert_eq!(got.harness_session_source, *source);
        }
    }

    #[test]
    fn missing_native_data_never_uses_an_override_as_provenance() {
        for override_value in [
            None,
            Some(""),
            Some("codex:a"),
            Some("claude-code:a"),
            Some("claim"),
        ] {
            assert_eq!(
                resolve_from(|name| match name {
                    "TASKS_SESSION" => override_value.map(OsString::from),
                    _ => None,
                })
                .unwrap(),
                None
            );
        }
    }

    #[test]
    fn conflicts_and_invalid_inputs_report_names_not_values() {
        let cases: &[(Vars<'_>, &[&str])] = &[
            (
                &[
                    ("CODEX_SESSION_ID", "private-a"),
                    ("CODEX_THREAD_ID", "private-b"),
                ],
                &["CODEX_SESSION_ID", "CODEX_THREAD_ID"],
            ),
            (
                &[
                    ("CLAUDE_CODE_SESSION_ID", "private-a"),
                    ("CODEX_THREAD_ID", "private-a"),
                ],
                &["CLAUDE_CODE_SESSION_ID", "CODEX_THREAD_ID"],
            ),
            (
                &[
                    ("CODEX_SESSION_ID", "private-a"),
                    ("TASKS_SESSION", "codex:private-b"),
                ],
                &["CODEX_SESSION_ID", "TASKS_SESSION"],
            ),
            (
                &[
                    ("CODEX_SESSION_ID", "private-a"),
                    ("TASKS_SESSION", "claude-code:private-a"),
                ],
                &["CODEX_SESSION_ID", "TASKS_SESSION"],
            ),
            (
                &[
                    ("CODEX_SESSION_ID", "private-a"),
                    ("CODEX_THREAD_ID", "private-b"),
                    ("TASKS_SESSION", "codex:private-a"),
                ],
                &["CODEX_SESSION_ID", "CODEX_THREAD_ID"],
            ),
        ];
        for (vars, names) in cases {
            let error = resolve_from(|name| {
                vars.iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| OsString::from(v))
            })
            .unwrap_err();
            for name in *names {
                assert!(error.contains(name), "{error}");
            }
            assert!(!error.contains("private"), "{error}");
        }
        for name in [
            "CLAUDE_CODE_SESSION_ID",
            "CODEX_SESSION_ID",
            "CODEX_THREAD_ID",
            "TASKS_SESSION",
        ] {
            for value in [
                "private\nvalue",
                "private\rvalue",
                "private\tvalue",
                "private\u{7f}value",
            ] {
                let error = resolve_from(|key| (key == name).then(|| value.into())).unwrap_err();
                assert!(
                    error.contains(name) && !error.contains("private"),
                    "{error}"
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_input_is_unknown_not_lossily_converted() {
        use std::os::unix::ffi::OsStringExt;
        let error =
            resolve_from(|key| (key == "CODEX_SESSION_ID").then(|| OsString::from_vec(vec![255])))
                .unwrap_err();
        assert!(error.contains("CODEX_SESSION_ID"));
    }
}
