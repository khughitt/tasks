use serde_json::Value;
use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

pub(crate) const TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const STDOUT_CAP: usize = 64 * 1024;

const UNVERIFIED_REASONS: &[&str] = &[
    "no-thread",
    "not-under-codex",
    "no-rollout",
    "ambiguous-rollout",
    "no-turn-context",
    "scan-limit",
    "unbound-call",
    "earlier-turn",
    "turn-boundary",
    "unstampable",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Harness {
    Codex,
    ClaudeCode,
    Agy,
    Opencode,
}

impl Harness {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "codex" => Some(Self::Codex),
            "claude-code" => Some(Self::ClaudeCode),
            "agy" => Some(Self::Agy),
            "opencode" => Some(Self::Opencode),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Agy => "agy",
            Self::Opencode => "opencode",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Run {
    Exited { success: bool, stdout: Vec<u8> },
    Timeout,
    Missing,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Verified {
        agent: String,
        model: String,
    },
    Unverified {
        reason: &'static str,
        nearest: Option<Harness>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outcome {
    Verified {
        agent: String,
        model: String,
    },
    Environment,
    Bare {
        agent: &'static str,
        warning: String,
    },
}

pub(crate) fn run_relay(timeout: Duration, cap: usize) -> Run {
    let deadline = Instant::now() + timeout;
    let mut child = match Command::new("relay")
        .args(["model", "--harness", "codex", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Run::Missing,
        Err(_) => return Run::Failed,
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Run::Failed;
    };

    let limit = (cap as u64).saturating_add(1);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let res = stdout.take(limit).read_to_end(&mut buf).map(|_| buf);
        let _ = tx.send(res);
    });

    let remaining = deadline.saturating_duration_since(Instant::now());
    let stdout = match rx.recv_timeout(remaining) {
        Ok(Ok(buf)) if buf.len() > cap => {
            let _ = child.kill();
            let _ = child.wait();
            return Run::Failed;
        }
        Ok(Ok(buf)) => buf,
        Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
            let _ = child.kill();
            let _ = child.wait();
            return Run::Failed;
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            let _ = child.kill();
            let _ = child.wait();
            drop(rx);
            return Run::Timeout;
        }
    };

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Run::Exited {
                    success: status.success(),
                    stdout,
                };
            }
            Ok(None) => {
                let now = Instant::now();
                if now >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Run::Timeout;
                }
                let sleep_for = (deadline - now).min(Duration::from_millis(10));
                std::thread::sleep(sleep_for);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Run::Failed;
            }
        }
    }
}

fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model
            .bytes()
            .all(|b| matches!(b, 0x21..=0x3F | 0x41..=0x7E))
}

fn valid_effort(effort: &str) -> bool {
    !effort.is_empty()
        && effort
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(crate) fn parse_answer(stdout: &[u8]) -> Option<Answer> {
    let Value::Object(obj) = serde_json::from_slice::<Value>(stdout).ok()? else {
        return None;
    };
    let verified = obj.get("verified")?.as_bool()?;
    if obj.get("harness")?.as_str()? != "codex" {
        return None;
    }
    let source = obj.get("source")?.as_object()?;
    let nearest = match source.get("nearest")? {
        Value::Null => None,
        Value::String(name) => Some(Harness::parse(name)?),
        _ => return None,
    };

    if verified {
        if nearest != Some(Harness::Codex) {
            return None;
        }
        if let Some(reason) = obj.get("reason")
            && !reason.is_null()
        {
            return None;
        }
        let model = obj.get("model")?.as_str()?;
        if !valid_model(model) || crate::format::validate_line("model", model).is_err() {
            return None;
        }
        let effort = match obj.get("effort")? {
            Value::Null => None,
            Value::String(effort) => {
                if !valid_effort(effort) {
                    return None;
                }
                Some(effort.as_str())
            }
            _ => return None,
        };
        let agent = obj.get("agent")?.as_str()?;
        let expected = match effort {
            None => format!("codex/{model}"),
            Some(effort) => format!("codex/{model}@{effort}"),
        };
        if agent != expected || crate::format::validate_line("agent", agent).is_err() {
            return None;
        }
        Some(Answer::Verified {
            agent: agent.to_string(),
            model: model.to_string(),
        })
    } else {
        let reason_raw = obj.get("reason")?.as_str()?;
        let &reason = UNVERIFIED_REASONS.iter().find(|&&r| r == reason_raw)?;
        if reason == "not-under-codex" {
            if nearest == Some(Harness::Codex) {
                return None;
            }
        } else if nearest != Some(Harness::Codex) {
            return None;
        }
        Some(Answer::Unverified { reason, nearest })
    }
}

pub(crate) fn classify(run: Run, tasks_agent: Option<&OsStr>) -> Outcome {
    let stdout = match run {
        Run::Timeout => {
            return Outcome::Bare {
                agent: "codex",
                warning: "model resolver timeout".into(),
            };
        }
        Run::Missing => {
            return Outcome::Bare {
                agent: "codex",
                warning: "model resolver missing".into(),
            };
        }
        Run::Failed | Run::Exited { success: false, .. } => {
            return Outcome::Bare {
                agent: "codex",
                warning: "model resolver failed".into(),
            };
        }
        Run::Exited {
            success: true,
            stdout,
        } => stdout,
    };

    match parse_answer(&stdout) {
        Some(Answer::Verified { agent, model }) => Outcome::Verified { agent, model },
        Some(Answer::Unverified {
            reason: "not-under-codex",
            nearest: None,
        }) => Outcome::Bare {
            agent: "codex",
            warning: "model unverified: no-harness".into(),
        },
        Some(Answer::Unverified {
            reason: "not-under-codex",
            nearest: Some(harness),
        }) => {
            let matches_env = tasks_agent
                .and_then(OsStr::to_str)
                .and_then(|value| value.split('/').next())
                .is_some_and(|prefix| prefix == harness.as_str());
            if matches_env {
                Outcome::Environment
            } else {
                Outcome::Bare {
                    agent: harness.as_str(),
                    warning: "model unverified: other-harness".into(),
                }
            }
        }
        Some(Answer::Unverified { reason, .. }) => Outcome::Bare {
            agent: "codex",
            warning: format!("model unverified: {reason}"),
        },
        None => Outcome::Bare {
            agent: "codex",
            warning: "model resolver failed".into(),
        },
    }
}

pub(crate) fn resolve() -> &'static Outcome {
    static OUTCOME: OnceLock<Outcome> = OnceLock::new();
    OUTCOME.get_or_init(|| {
        let run = run_relay(TIMEOUT, STDOUT_CAP);
        let tasks_agent = std::env::var_os("TASKS_AGENT");
        classify(run, tasks_agent.as_deref())
    })
}

fn triggered_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|v| !v.is_empty())
}

pub(crate) fn triggered() -> bool {
    triggered_value(std::env::var_os("CODEX_THREAD_ID").as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    const VERIFIED_WITH_EFFORT: &[u8] = br#"{"verified":true,"harness":"codex","agent":"codex/mock-model@low","model":"mock-model","effort":"low","reason":null,"source":{"nearest":"codex"}}"#;
    const VERIFIED_WITHOUT_EFFORT: &[u8] = br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null,"source":{"nearest":"codex"}}"#;

    #[test]
    fn triggered_checks_nonempty_codex_thread_id() {
        assert!(!triggered_value(None));
        assert!(!triggered_value(Some(OsStr::new(""))));
        assert!(triggered_value(Some(OsStr::new("019a0000"))));
        assert!(triggered_value(Some(OsStr::from_bytes(b"\xff"))));
    }

    #[test]
    fn parse_answer_verified_with_and_without_effort() {
        assert_eq!(
            parse_answer(VERIFIED_WITH_EFFORT),
            Some(Answer::Verified {
                agent: "codex/mock-model@low".into(),
                model: "mock-model".into(),
            })
        );
        assert_eq!(
            parse_answer(VERIFIED_WITHOUT_EFFORT),
            Some(Answer::Verified {
                agent: "codex/mock-model".into(),
                model: "mock-model".into(),
            })
        );
    }

    #[test]
    fn parse_answer_rejects_invalid_grammar_and_inconsistent_fields() {
        let invalid_cases: &[(&str, &[u8])] = &[
            (
                "multiline effort",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model@lo\nw","model":"mock-model","effort":"lo\nw","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "uppercase effort",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model@Low","model":"mock-model","effort":"Low","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "space in effort",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model@lo w","model":"mock-model","effort":"lo w","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "empty effort",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model@","model":"mock-model","effort":"","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "space in model",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock model@low","model":"mock model","effort":"low","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "at-sign in model",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock@model@low","model":"mock@model","effort":"low","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "empty model",
                br#"{"verified":true,"harness":"codex","agent":"codex/","model":"","effort":null,"reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "del byte in model",
                b"{\"verified\":true,\"harness\":\"codex\",\"agent\":\"codex/mock\x7f\",\"model\":\"mock\x7f\",\"effort\":null,\"reason\":null,\"source\":{\"nearest\":\"codex\"}}",
            ),
            (
                "effort present but missing from agent",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":"low","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "agent carries effort while effort is null",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model@low","model":"mock-model","effort":null,"reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "effort key missing on verified",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "wrong harness",
                br#"{"verified":true,"harness":"agy","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null,"source":{"nearest":"codex"}}"#,
            ),
            (
                "unknown unverified reason",
                br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"sideways","source":{"nearest":"codex"}}"#,
            ),
            ("non-json", b"not json"),
            ("json array", b"[]"),
            (
                "source missing",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null}"#,
            ),
            (
                "nearest key missing",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null,"source":{"thread":"t","rollout":"r"}}"#,
            ),
            (
                "nearest outside closed set",
                br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"pi"}}"#,
            ),
            (
                "verified with non-codex nearest",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null,"source":{"nearest":"claude-code"}}"#,
            ),
            (
                "verified with null nearest",
                br#"{"verified":true,"harness":"codex","agent":"codex/mock-model","model":"mock-model","effort":null,"reason":null,"source":{"nearest":null}}"#,
            ),
            (
                "not-under-codex with codex nearest",
                br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"codex"}}"#,
            ),
            (
                "earlier-turn with null nearest",
                br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"earlier-turn","source":{"nearest":null}}"#,
            ),
            (
                "earlier-turn with non-codex nearest",
                br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"earlier-turn","source":{"nearest":"claude-code"}}"#,
            ),
            (
                "no-thread without nearest",
                br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"no-thread","source":{}}"#,
            ),
        ];

        for (label, input) in invalid_cases {
            assert_eq!(parse_answer(input), None, "case should be invalid: {label}");
        }
    }

    #[test]
    fn parse_answer_accepts_documented_unverified_reasons() {
        for reason in [
            "no-thread",
            "no-rollout",
            "ambiguous-rollout",
            "no-turn-context",
            "scan-limit",
            "unbound-call",
            "earlier-turn",
            "turn-boundary",
            "unstampable",
        ] {
            let body = format!(
                r#"{{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"{reason}","source":{{"nearest":"codex"}}}}"#
            );
            assert_eq!(
                parse_answer(body.as_bytes()),
                Some(Answer::Unverified {
                    reason,
                    nearest: Some(Harness::Codex),
                }),
                "documented reason {reason}"
            );
        }

        for (raw, expected) in [
            ("null", None),
            ("\"claude-code\"", Some(Harness::ClaudeCode)),
            ("\"agy\"", Some(Harness::Agy)),
            ("\"opencode\"", Some(Harness::Opencode)),
        ] {
            let body = format!(
                r#"{{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{{"nearest":{raw}}}}}"#
            );
            assert_eq!(
                parse_answer(body.as_bytes()),
                Some(Answer::Unverified {
                    reason: "not-under-codex",
                    nearest: expected,
                }),
                "not-under-codex with nearest {raw}"
            );
        }
    }

    #[test]
    fn classify_covers_verified_other_harness_unverified_and_failures() {
        let ok = |stdout: &[u8]| Run::Exited {
            success: true,
            stdout: stdout.to_vec(),
        };

        assert_eq!(
            classify(ok(VERIFIED_WITH_EFFORT), Some(OsStr::new("poison-agent"))),
            Outcome::Verified {
                agent: "codex/mock-model@low".into(),
                model: "mock-model".into(),
            }
        );

        let other_claude = br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"claude-code"}}"#;
        assert_eq!(
            classify(
                ok(other_claude),
                Some(OsStr::new("claude-code/claude-opus-5-5"))
            ),
            Outcome::Environment
        );
        assert_eq!(
            classify(ok(other_claude), Some(OsStr::new("claude-code"))),
            Outcome::Environment
        );

        for mismatched in [
            None,
            Some(OsStr::new("")),
            Some(OsStr::new("codex")),
            Some(OsStr::new("claude-code-x/m")),
            Some(OsStr::from_bytes(b"\xff\xfe")),
        ] {
            assert_eq!(
                classify(ok(other_claude), mismatched),
                Outcome::Bare {
                    agent: "claude-code",
                    warning: "model unverified: other-harness".into(),
                },
                "mismatched TASKS_AGENT {mismatched:?}"
            );
        }

        let other_agy = br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":"agy"}}"#;
        assert_eq!(
            classify(ok(other_agy), Some(OsStr::new("codex"))),
            Outcome::Bare {
                agent: "agy",
                warning: "model unverified: other-harness".into(),
            }
        );

        let no_harness = br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"not-under-codex","source":{"nearest":null}}"#;
        assert_eq!(
            classify(ok(no_harness), Some(OsStr::new("codex/gpt-6"))),
            Outcome::Bare {
                agent: "codex",
                warning: "model unverified: no-harness".into(),
            }
        );

        let unbound = br#"{"verified":false,"harness":"codex","agent":null,"model":null,"effort":null,"reason":"unbound-call","source":{"nearest":"codex"}}"#;
        assert_eq!(
            classify(ok(unbound), None),
            Outcome::Bare {
                agent: "codex",
                warning: "model unverified: unbound-call".into(),
            }
        );

        assert_eq!(
            classify(Run::Timeout, None),
            Outcome::Bare {
                agent: "codex",
                warning: "model resolver timeout".into(),
            }
        );
        assert_eq!(
            classify(Run::Missing, None),
            Outcome::Bare {
                agent: "codex",
                warning: "model resolver missing".into(),
            }
        );
        assert_eq!(
            classify(Run::Failed, None),
            Outcome::Bare {
                agent: "codex",
                warning: "model resolver failed".into(),
            }
        );
        assert_eq!(
            classify(
                Run::Exited {
                    success: false,
                    stdout: VERIFIED_WITH_EFFORT.to_vec(),
                },
                None
            ),
            Outcome::Bare {
                agent: "codex",
                warning: "model resolver failed".into(),
            }
        );
        assert_eq!(
            classify(ok(b"not json"), None),
            Outcome::Bare {
                agent: "codex",
                warning: "model resolver failed".into(),
            }
        );
    }
}
