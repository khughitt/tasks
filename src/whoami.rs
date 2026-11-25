//! This process's native session, as `relay whoami --json` reports it.
use crate::config::HostConfig;
use crate::error::{Error, Result};
use crate::relay_cli::{self, Run};
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::Duration;

/// A refusal stops every claim write, so a loaded host gets far longer than one answer needs.
const TIMEOUT: Duration = Duration::from_secs(10);
const STDOUT_CAP: usize = 256 * 1024;

const SET_EXPLICIT: &str =
    "set TASKS_SESSION and TASKS_SESSION_PID to name this session explicitly";

/// The advice for a caller in a child PID namespace. A pid it can name is namespace-local,
/// and a claim carrying one stays held after the session ends, so it must not be offered.
const SET_SESSION_ONLY: &str = "set TASKS_SESSION to this session's exact id, without \
     TASKS_SESSION_PID (a pid from this namespace keeps the claim held after the session \
     ends), or run with the host PID view";

/// A matched agent: its id becomes the claim session, its process handle the claim's proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
    pub(crate) session: String,
    pub(crate) root_session: Option<String>,
    pub(crate) process: Process,
}

/// The Linux process handle of the nearest harness process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Process {
    pub(crate) host: String,
    pub(crate) boot_id: String,
    pub(crate) pid: u32,
    pub(crate) start: u64,
}

/// What proof-only continuation needs: the nearest harness *session* process, with the
/// native caller that may contradict a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Caller {
    pub(crate) harness: String,
    pub(crate) process: Process,
    /// The native session id relay selected for this harness.
    pub(crate) hint: Option<String>,
    /// Codex only: the caller's own thread id. The root id never stands in for it.
    pub(crate) thread: Option<String>,
}

/// relay's answer for one caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Resolved(Resolved, Caller),
    Outside,
    /// No single session could be named. `caller` survives when the nearest harness
    /// process is a session whose native values relay could read.
    Refused {
        detail: String,
        caller: Option<Caller>,
    },
}

impl Answer {
    pub(crate) fn caller(&self) -> Option<&Caller> {
        match self {
            Answer::Resolved(_, caller) => Some(caller),
            Answer::Refused { caller, .. } => caller.as_ref(),
            Answer::Outside => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wire {
    schema: u64,
    status: String,
    boundary: Option<WireBoundary>,
    /// Nullable, but always present beside a boundary.
    #[serde(default, deserialize_with = "present")]
    hint: Option<Option<String>>,
    native: WireNative,
    agent: Option<WireAgent>,
    /// Nullable, but always present when resolved.
    #[serde(default, deserialize_with = "present")]
    root_session: Option<Option<String>>,
    code: Option<String>,
    message: Option<String>,
}

/// Tell an absent field (`None`) from a null one (`Some(None)`).
fn present<'de, D: serde::Deserializer<'de>>(
    field: D,
) -> std::result::Result<Option<Option<String>>, D::Error> {
    Option::deserialize(field).map(Some)
}

#[derive(Deserialize)]
struct WireBoundary {
    harness: String,
    role: String,
    process: WireProcess,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WireProcess {
    platform: String,
    host: String,
    boot_id: Option<String>,
    pid: u32,
    start: String,
}

#[derive(Deserialize)]
struct WireAgent {
    id: String,
    harness: String,
    process: WireProcess,
}

#[derive(Deserialize)]
struct WireNative {
    #[serde(rename = "CODEX_THREAD_ID")]
    codex_thread: WireEntry,
}

#[derive(Deserialize)]
struct WireEntry {
    state: String,
    value: Option<String>,
}

fn refuse(detail: &str) -> Error {
    Error::Config(format!("relay identity: {detail}"))
}

/// Parse one schema-1 result line from a run that exited with `success`. `Err` is an answer
/// outside the contract.
fn parse(success: bool, stdout: &[u8]) -> std::result::Result<Answer, String> {
    let wire: Wire = serde_json::from_slice(stdout).map_err(|error| error.to_string())?;
    if wire.schema != 1 {
        return Err(format!("schema {} is not supported", wire.schema));
    }
    if success == (wire.status == "failed") {
        return Err(format!("status {:?} contradicts the exit", wire.status));
    }
    let refused = |caller| -> std::result::Result<Answer, String> {
        let code = wire.code.as_deref().ok_or("no code")?;
        let message = wire.message.as_deref().ok_or("no message")?;
        let advice = if code == "HIDDEN_ANCESTRY" {
            SET_SESSION_ONLY
        } else {
            SET_EXPLICIT
        };
        Ok(Answer::Refused {
            detail: format!("{message} ({code}); {advice}"),
            caller,
        })
    };
    match wire.status.as_str() {
        "outside" => return Ok(Answer::Outside),
        "failed" => return refused(None),
        "resolved" | "unresolved" => {}
        other => return Err(format!("unknown status {other:?}")),
    }
    let boundary = wire.boundary.as_ref().ok_or("no boundary")?;
    let process = &boundary.process;
    if process.platform != "linux" {
        return Err(format!(
            "a {:?} handle cannot prove a claim; tasks reads Linux handles only",
            process.platform
        ));
    }
    let caller = Caller {
        harness: boundary.harness.clone(),
        process: Process {
            host: process.host.clone(),
            boot_id: process.boot_id.clone().ok_or("no bootId")?,
            pid: process.pid,
            start: process
                .start
                .parse()
                .map_err(|_| format!("start {:?} is not a tick count", process.start))?,
        },
        hint: wire.hint.clone().ok_or("no hint")?,
        thread: (boundary.harness == "codex" && wire.native.codex_thread.state == "present")
            .then(|| wire.native.codex_thread.value.clone())
            .flatten(),
    };
    if wire.status == "unresolved" {
        // A hosting process is not a session, a hidden one is not this session's process,
        // and relay gives no hint to contradict a claim with when a native value is invalid.
        let usable = boundary.role == "session"
            && !matches!(
                wire.code.as_deref(),
                Some("HIDDEN_ANCESTRY" | "INVALID_NATIVE_VALUE")
            );
        return refused(usable.then_some(caller));
    }
    let agent = wire.agent.as_ref().ok_or("no agent")?;
    if agent.process != *process || agent.harness != boundary.harness || boundary.role != "session"
    {
        return Err("the agent is not the boundary's session".into());
    }
    Ok(Answer::Resolved(
        Resolved {
            session: agent.id.clone(),
            root_session: wire.root_session.clone().ok_or("no rootSession")?,
            process: caller.process.clone(),
        },
        caller,
    ))
}

/// relay's answer from one finished run. An identity that cannot be read refuses: with
/// relay on, no caller is ever placed on a lower level by a missing or broken relay.
fn classify(run: Run) -> Answer {
    let detail = match run {
        Run::Exited { success, stdout } => match parse(success, &stdout) {
            Ok(answer) => return answer,
            Err(error) => format!(
                "`relay whoami --json` gave no usable answer ({error}); it needs relay 0.4.0 or \
                 newer on PATH"
            ),
        },
        Run::Missing => "relay is not on PATH".into(),
        Run::Timeout => format!("`relay whoami` exceeded {} s", TIMEOUT.as_secs()),
        Run::Failed => "`relay whoami` could not be run".into(),
    };
    Answer::Refused {
        detail: format!("{detail}; {SET_EXPLICIT}"),
        caller: None,
    }
}

/// The answer for this process, asked once: its ancestry and environment do not change.
pub(crate) fn current() -> &'static Answer {
    static ANSWER: OnceLock<Answer> = OnceLock::new();
    ANSWER.get_or_init(|| classify(relay_cli::run(&["whoami", "--json"], TIMEOUT, STDOUT_CAP)))
}

/// The relay level of the identity ladder. `Ok(None)` means relay is off or the caller is
/// under no harness; either way the native levels apply unchanged.
pub(crate) fn level(host: &HostConfig) -> Result<Option<Resolved>> {
    if !host.relay_identity {
        return Ok(None);
    }
    stage_platform(std::fs::metadata("/proc/self/stat").is_ok())?;
    match current() {
        Answer::Outside => Ok(None),
        Answer::Resolved(resolved, _) => Ok(Some(resolved.clone())),
        Answer::Refused { detail, .. } => Err(refuse(detail)),
    }
}

/// relay answers on hosts without `/proc`; claims do not, because their liveness is judged
/// from it. The refusal belongs to the relay level, so it never takes away the explicit pair.
fn stage_platform(proc_available: bool) -> Result<()> {
    if proc_available {
        return Ok(());
    }
    Err(Error::Config(
        "relay identity is enabled but this host does not expose /proc, so a claim's process \
         cannot be judged. Disable [identity].relay, or set TASKS_SESSION and \
         TASKS_SESSION_PID to name this session explicitly."
            .into(),
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::{Value, json};

    const CORPUS: &str = include_str!("../tests/fixtures/relay/whoami.json");

    /// relay's conformance cases: each has a name, the exact `expect` line, and `tasks`'
    /// own outcome where the result alone does not settle it.
    pub(crate) fn corpus() -> Vec<Value> {
        serde_json::from_str::<Value>(CORPUS)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    /// The case's line as relay prints it. A failed case holds only its code, and relay's
    /// CLI wraps that in the failed result.
    pub(crate) fn answer(case: &Value) -> std::result::Result<Answer, String> {
        let line = match case["expect"]["failed"].as_str() {
            None => case["expect"].clone(),
            Some(code) => json!({
                "schema": 1, "status": "failed", "code": code, "message": "failed",
                "native": case["expect"].get("native").cloned()
                    .unwrap_or_else(|| json!({"CODEX_THREAD_ID": {"state": "absent"}})),
            }),
        };
        parse(line["status"] != "failed", line.to_string().as_bytes())
    }

    fn case(name: &str) -> Value {
        corpus()
            .into_iter()
            .find(|case| case["name"] == name)
            .unwrap_or_else(|| panic!("no case {name:?}"))
    }

    fn detail(name: &str) -> String {
        match answer(&case(name)).unwrap() {
            Answer::Refused { detail, .. } => detail,
            other => panic!("{name}: {other:?}"),
        }
    }

    #[test]
    fn every_relay_case_maps_to_one_tasks_outcome() {
        let cases = corpus();
        assert!(cases.len() >= 104, "{}", cases.len());
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let expect = &case["expect"];
            let darwin = expect["boundary"]["process"]["platform"] == "darwin";
            let answer = answer(&case);
            let code = expect["failed"].as_str().or(expect["code"].as_str());
            match expect["status"].as_str().unwrap_or("failed") {
                "outside" => assert_eq!(answer, Ok(Answer::Outside), "{name}"),
                _ if darwin => assert!(answer.unwrap_err().contains("Linux"), "{name}"),
                "resolved" => {
                    let Ok(Answer::Resolved(resolved, caller)) = answer else {
                        panic!("{name}: {answer:?}")
                    };
                    assert_eq!(resolved.session, expect["agent"]["id"], "{name}");
                    if let Some(session) = case["tasks"]["claimSession"].as_str() {
                        assert_eq!(resolved.session, session, "{name}");
                    }
                    assert_eq!(
                        json!(resolved.root_session),
                        expect["rootSession"],
                        "{name}"
                    );
                    let process = &expect["agent"]["process"];
                    assert_eq!(resolved.process.pid, process["pid"], "{name}");
                    assert_eq!(
                        resolved.process.start.to_string(),
                        process["start"],
                        "{name}"
                    );
                    assert_eq!(resolved.process.boot_id, process["bootId"], "{name}");
                    assert_eq!(resolved.process.host, process["host"], "{name}");
                    assert_eq!(caller.process, resolved.process, "{name}");
                }
                _ => {
                    let Ok(Answer::Refused { detail, .. }) = answer else {
                        panic!("{name}: {answer:?}")
                    };
                    assert!(detail.contains("TASKS_SESSION"), "{name}: {detail}");
                    assert!(detail.contains(code.unwrap()), "{name}: {detail}");
                }
            }
        }
    }

    #[test]
    fn a_hidden_session_is_not_offered_a_namespace_local_pid() {
        let hidden = detail("hidden namespace");
        assert!(hidden.contains("without TASKS_SESSION_PID"), "{hidden}");
        assert!(!hidden.contains("set TASKS_SESSION and"), "{hidden}");
    }

    #[test]
    fn evidence_is_withheld_from_every_unprovable_boundary() {
        for name in [
            "outside to init",
            "unknown ancestry with native variables",
            "daemon hosting process",
            "bg-pty-host hosting process",
            "hidden namespace",
            "invalid CODEX_SESSION_ID newline",
            "invalid CODEX_THREAD_ID non-Unicode",
        ] {
            assert_eq!(answer(&case(name)).unwrap().caller(), None, "{name}");
        }
        // A lost registry is the case proof exists for.
        let lost = answer(&case("corrupt registry with harness")).unwrap();
        assert!(lost.caller().is_some());
    }

    #[test]
    fn an_answer_outside_the_contract_is_refused() {
        let good = &case("a match adopts the agent id and its proof")["expect"];
        let changed = |pointer: &str, value: Value| {
            let mut line = good.clone();
            *line.pointer_mut(pointer).unwrap() = value;
            parse(true, line.to_string().as_bytes()).unwrap_err()
        };
        changed("/schema", json!(2));
        changed("/status", json!("maybe"));
        changed("/boundary", Value::Null);
        changed("/agent", Value::Null);
        changed("/agent/process/pid", json!(43));
        changed("/boundary/process/start", json!("soon"));
        changed("/boundary/process/bootId", Value::Null);
        changed("/boundary/role", json!("daemon"));
        changed("/agent/harness", json!("agy"));
        for key in ["hint", "rootSession"] {
            let mut line = good.clone();
            line.as_object_mut().unwrap().remove(key);
            assert!(parse(true, line.to_string().as_bytes()).is_err(), "{key}");
        }
        // Only `failed` exits 1.
        assert!(parse(false, good.to_string().as_bytes()).is_err());
        assert!(parse(false, b"").is_err());
        assert!(parse(false, b"{\"error\":{\"kind\":\"usage\"}}").is_err());
    }

    #[test]
    fn a_relay_that_cannot_answer_refuses_rather_than_falling_through() {
        for (run, expected) in [
            (Run::Missing, "not on PATH"),
            (Run::Timeout, "exceeded"),
            (Run::Failed, "could not be run"),
            (
                Run::Exited {
                    success: false,
                    stdout: Vec::new(),
                },
                "0.4.0",
            ),
        ] {
            let Answer::Refused { detail, caller } = classify(run) else {
                panic!("{expected}")
            };
            assert!(detail.contains(expected), "{detail}");
            assert!(detail.contains("TASKS_SESSION"), "{detail}");
            assert_eq!(caller, None);
        }
    }

    #[test]
    fn a_host_without_proc_refuses_with_the_explicit_remedy() {
        assert!(stage_platform(true).is_ok());
        let error = stage_platform(false).unwrap_err().to_string();
        assert!(error.contains("/proc"), "{error}");
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn relay_off_never_asks() {
        assert_eq!(level(&HostConfig::default()).unwrap(), None);
    }
}
