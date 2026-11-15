use super::facts::WakeFacts;
use super::input::{Fact, Harness, HookInput, fact};
use crate::claims::{Claim, ClaimSnapshot, Liveness};
use crate::registry::Registry;
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) type Claims = Vec<(String, Claim, Liveness)>;
pub(super) enum GuardDecision {
    Allow,
    Deny(String),
}
pub(super) fn read_claims() -> Result<Claims, String> {
    let registry = Registry::load().map_err(|_| "registered projects could not be read")?;
    let mut result = Vec::new();
    for prefix in registry.projects.keys() {
        let snapshot = ClaimSnapshot::load(std::iter::once(prefix.as_str()))
            .map_err(|_| "a registered claim store could not be read")?;
        result.extend(
            snapshot
                .iter()
                .map(|(id, (claim, live))| (id.clone(), claim.clone(), live.clone())),
        );
    }
    Ok(result)
}
fn value<'a>(container: &'a Value, key: &str) -> Option<&'a Value> {
    if let Fact::Valid(v) = fact(container, key) {
        Some(v)
    } else {
        None
    }
}
fn deny(text: impl Into<String>) -> GuardDecision {
    GuardDecision::Deny(text.into())
}
pub(super) fn decide(
    event: &HookInput,
    claims: impl FnOnce() -> Result<Claims, String>,
    facts: impl FnOnce(Harness) -> Result<WakeFacts, String>,
) -> GuardDecision {
    let h = event.harness;
    let e = &event.event;
    let f = &e["facts"];
    if h == Harness::Agy {
        if value(f, "terminationReason")
            .and_then(Value::as_str)
            .is_some_and(|s| {
                matches!(
                    s,
                    "ERROR"
                        | "USER_CANCELED"
                        | "MAX_INVOCATIONS"
                        | "MAX_FORCED_INVOCATIONS"
                        | "MAX_TOKEN_BUDGET_EXCEEDED"
                        | "TERMINAL_CUSTOM_HOOK"
                )
            })
        {
            return GuardDecision::Allow;
        }
        let count = value(f, "executionNum")
            .and_then(Value::as_f64)
            .filter(|n| {
                n.is_finite() && *n >= 0.0 && n.fract() == 0.0 && *n <= 9_007_199_254_740_991.0
            });
        match count {
            Some(n) if n > 0.0 => return GuardDecision::Allow,
            Some(_) => {}
            None => {
                return deny(
                    "claim-guard: the Stop input has no non-negative safe integer executionNum; act on your own claims before ending the turn.",
                );
            }
        }
    } else if value(f, "stopHookActive").and_then(Value::as_bool) == Some(true) {
        return GuardDecision::Allow;
    }
    let native = e["nativeEvent"].as_str();
    let supported = matches!(
        (native, e["kind"].as_str()),
        (Some("Stop"), Some("turn-end"))
    ) || (h == Harness::Codex
        && matches!(
            (native, e["kind"].as_str()),
            (Some("SubagentStop"), Some("child-stop"))
        ));
    if !supported {
        return deny(
            "claim-guard: unsupported or missing hook_event_name; repair the native hook registration.",
        );
    }
    let Some(identity) = value(e, "identity").filter(|v| v.is_object()) else {
        return deny(
            "claim-guard: the Stop input has no session_id, so held claims cannot be checked; act on your own claims before ending the turn.",
        );
    };
    let Some(session) = identity["sessionId"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 1024)
    else {
        return deny(
            "claim-guard: the Stop input has no session_id, so held claims cannot be checked; act on your own claims before ending the turn.",
        );
    };
    if identity["agentId"] != format!("{}:{session}", h.name()) {
        return deny(
            "claim-guard: event identity agentId does not match its harness/sessionId; repair the hook registration.",
        );
    }
    let worker = h == Harness::Codex && identity.get("root").is_some();
    if h == Harness::Codex && (worker || native == Some("SubagentStop")) {
        let root = identity["root"].as_str().filter(|s| {
            s.starts_with("codex:")
                && !s[6..].trim().is_empty()
                && s.len() <= 1024
                && *s != format!("codex:{session}")
        });
        if root.is_none() {
            return deny(
                "claim-guard: Codex worker event has no usable distinct root; repair the worker event before ending the turn.",
            );
        }
    }
    let rows = match claims() {
        Ok(rows) => rows,
        Err(error) => {
            return deny(format!(
                "claim-guard: the claims could not be read ({error}), so you may hold a task claim. {} If you hold none, say so and end.",
                options(h, None, None, worker)
            ));
        }
    };
    let prefixes: &[&str] = if h == Harness::Codex {
        &["codex"]
    } else {
        &["claude-code", "codex", "agy"]
    };
    let wanted = |s: &str| s == session || prefixes.iter().any(|p| s == format!("{p}:{session}"));
    let own: Vec<_> = rows
        .iter()
        .filter(|(_, c, l)| !l.is_stale() && wanted(&c.session))
        .collect();
    let root = format!("codex:{session}");
    let related: Vec<_> = rows
        .iter()
        .filter(|(_, c, l)| {
            !worker
                && h == Harness::Codex
                && !l.is_stale()
                && !wanted(&c.session)
                && c.root_session.as_deref() == Some(&root)
        })
        .collect();
    if own.is_empty() && related.is_empty() {
        return GuardDecision::Allow;
    }
    if worker {
        return deny(reason(h, &own, &[], None, None, true));
    }
    let mut wake = None;
    let mut unseen = None;
    match facts(h) {
        Err(error) => {
            unseen = Some(format!(
                "facts could not be read ({error}); repair hq's installed facts reader"
            ))
        }
        Ok(facts) => {
            wake = facts.wake;
            if wake.is_none() {
                unseen = Some("unprobed controller-wake".into());
            }
            if wake == Some(true) {
                let mut missing = Vec::new();
                if facts.kinds.is_none() {
                    missing.push("controller-wake-kinds");
                }
                if h != Harness::Agy && facts.statuses.is_none() {
                    missing.push("stop-input-in-flight-statuses");
                }
                if !missing.is_empty() {
                    wake = None;
                    unseen = Some(format!("unprobed {}", missing.join(", ")));
                } else if h == Harness::Agy {
                    let kinds = facts.kinds.as_ref().expect("validated kinds");
                    let blind: Vec<_> = ["shell", "subagent"]
                        .into_iter()
                        .filter(|s| !kinds.contains(*s))
                        .collect();
                    match value(f, "fullyIdle").and_then(Value::as_bool) {
                        None => {
                            unseen = Some("the Stop input has no boolean fullyIdle field".into())
                        }
                        Some(false) if !blind.is_empty() => {
                            unseen = Some(format!(
                                "fullyIdle cannot tell a waking child from {}",
                                blind.join(", ")
                            ))
                        }
                        Some(false) => return GuardDecision::Allow,
                        Some(true) => {}
                    }
                } else {
                    match in_flight(
                        &e["private"],
                        facts.kinds.as_ref().unwrap(),
                        facts.statuses.as_ref().unwrap(),
                    ) {
                        Ok(true) => return GuardDecision::Allow,
                        Ok(false) => {}
                        Err((why, invalid)) => {
                            unseen = Some(why);
                            if invalid {
                                wake = None;
                            }
                        }
                    }
                }
            }
        }
    }
    deny(reason(h, &own, &related, wake, unseen.as_deref(), false))
}

fn in_flight(
    private: &Value,
    kinds: &BTreeSet<String>,
    statuses: &BTreeSet<String>,
) -> Result<bool, (String, bool)> {
    let tasks = match fact(private, "backgroundTasks") {
        Fact::Missing => return Err(("the Stop input lists no background work".into(), false)),
        Fact::Invalid => {
            return Err((
                "backgroundTasks is unreadable; child and wake state are unknown".into(),
                true,
            ));
        }
        Fact::Valid(v) => v.as_array().filter(|xs| xs.len() <= 1024).ok_or_else(|| {
            (
                "backgroundTasks is unreadable; child and wake state are unknown".into(),
                true,
            )
        })?,
    };
    if tasks.iter().any(|t| {
        !t.is_object()
            || t["status"].as_str().is_none_or(|s| s.len() > 256)
            || t["type"].as_str().is_none_or(|s| s.len() > 256)
    }) {
        return Err((
            "the Stop input's background_tasks is not a list of tasks with a type and a status"
                .into(),
            false,
        ));
    }
    let unknown: BTreeSet<_> = tasks
        .iter()
        .filter_map(|t| t["status"].as_str())
        .filter(|s| !statuses.contains(*s))
        .collect();
    if !unknown.is_empty() {
        return Err((
            format!(
                "the Stop input's background_tasks has a status the guard does not know: {}",
                unknown.into_iter().collect::<Vec<_>>().join(", ")
            ),
            false,
        ));
    }
    let waking = tasks
        .iter()
        .any(|t| kinds.contains(t["type"].as_str().unwrap()));
    if !tasks.is_empty() && !waking {
        let types: BTreeSet<_> = tasks.iter().map(|t| t["type"].as_str().unwrap()).collect();
        return Err((
            format!(
                "the Stop input lists only work not known to wake a controller: {}",
                types.into_iter().collect::<Vec<_>>().join(", ")
            ),
            false,
        ));
    }
    if tasks.is_empty() {
        match fact(private, "sessionCrons") {
            Fact::Missing => {}
            Fact::Invalid => {
                return Err((
                    "sessionCrons is unreadable; child and wake state are unknown".into(),
                    true,
                ));
            }
            Fact::Valid(v) => {
                let crons = v
                    .as_array()
                    .filter(|xs| {
                        xs.len() <= 128
                            && xs.iter().all(|x| {
                                x["id"]
                                    .as_str()
                                    .is_some_and(|s| !s.trim().is_empty() && s.len() <= 1024)
                                    && x["prompt"].as_str().is_some_and(|s| s.len() <= 8192)
                            })
                    })
                    .ok_or_else(|| {
                        (
                            "sessionCrons is unreadable; child and wake state are unknown".into(),
                            true,
                        )
                    })?;
                if !crons.is_empty() {
                    return Err(("the Stop input lists a scheduled wake-up, which the guard does not count as a child".into(),false));
                }
            }
        }
    }
    Ok(waking)
}
fn detached(h: Harness) -> &'static str {
    if h == Harness::Agy {
        "an `IsDaemon` command or a job detached with `&` or `nohup` never does"
    } else {
        "a job detached with `&` or `nohup` never does"
    }
}
fn not_running(h: Harness) -> &'static str {
    if h == Harness::Agy {
        "An `IsDaemon` command or a job detached with `&` or `nohup` is never a running child."
    } else {
        "A job detached with `&` or `nohup` is never a running child."
    }
}
fn options(h: Harness, wake: Option<bool>, unseen: Option<&str>, worker: bool) -> String {
    let mut context = String::new();
    if !worker && wake == Some(true) {
        context = if let Some(unseen) = unseen {
            format!(
                "The guard cannot see whether a child is running ({unseen}). If one the harness tracks is running and your last message named it, reply with one line naming it again and end; the stop that follows is allowed, and the child's completion starts your next turn; {}. ",
                detached(h)
            )
        } else {
            format!(
                "The Stop input lists no running child, though a child the harness tracks may still be running outside this snapshot, or one that just finished may have a queued completion that starts your next turn. If such a child is running or you have not received its result, and your last message named it, reply with one line naming it again and end; the stop that follows is allowed. Without such a child or completion, nothing will start your next turn; {}. ",
                detached(h)
            )
        };
    }
    if wake.is_none() && !worker {
        context.push_str(&format!(
            "Wake inputs are unprobed or unavailable{}. ",
            unseen.map(|s| format!(": {s}")).unwrap_or_default()
        ));
    }
    let ending = if worker {
        format!(
            "If you are a subagent, report and stop only when the assignment is complete or blocked on your controller or a person. If work or a check is still running, continue or wait, then report. {} ",
            not_running(h)
        )
    } else if wake != Some(true) {
        let bounded = match h {
            Harness::Claude => {
                "the harness's bounded wait (a foreground check with a timeout that covers it)"
            }
            Harness::Codex => "the harness's bounded wait (`wait_agent` with a long timeout)",
            Harness::Agy => {
                "the harness's bounded wait (a foreground `run_command` with a timeout that covers it)"
            }
        };
        format!(
            "(4) A child is running: Do not end the turn. Wait on it with {bounded}, act on its result, and if the wait times out, check the child before waiting again. {} ",
            not_running(h)
        )
    } else {
        String::new()
    };
    format!(
        "{context}If the user told you this turn to stop, wait or only answer, use (2) with `--waiting-on user`, not (1). Otherwise do one of these for your own claims before the turn ends: (1) Continue: the work is unblocked and yours. (2) Park: `tasks park <id> \"<next step>\"`, an action and who takes it, never a state; add `--waiting-on user --reason review|decision|approval` only when a person must act. (3) Done: `tasks done <id> \"<what landed>\"`. {ending}Decisions and Processes in your instructions give the rules."
    )
}
fn reason(
    h: Harness,
    own: &[&(String, Claim, Liveness)],
    related: &[&(String, Claim, Liveness)],
    wake: Option<bool>,
    unseen: Option<&str>,
    worker: bool,
) -> String {
    let names = |rows: &[&(String, Claim, Liveness)]| {
        rows.iter()
            .map(|(id, c, _)| format!("`{id}` in `{}`", c.worktree))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut parts = Vec::new();
    if !own.is_empty() {
        parts.push(format!(
            "You are ending a turn while this session holds a live task claim: {}. {}",
            names(own),
            options(h, wake, unseen, worker)
        ));
    }
    if !related.is_empty() {
        parts.push(format!("Related worker claims remain live: {}. Inspect the worker's result and claim. Only after your own completion/wait result for that specific worker proves its assignment stopped with no pending tool, check or background process, recover there with `tasks start <id> --force --reason \"<worker-specific completion evidence>\"`, then review or close under your own claim. Timeout, silence, root state and a shared PID are not that evidence. If work is still running or uncertain, reconcile it and keep waiting. Do not resume the worker on that task without a new ownership handoff.",names(related)));
        if let Some(unseen) = unseen {
            parts.push(format!("Wake inputs unavailable: {unseen}; repair the facts reader. No wake exception was granted."));
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn event(h: Harness) -> HookInput {
        HookInput {
            harness: h,
            event: json!({"kind":"turn-end","nativeEvent":"Stop","identity":{"status":"valid","value":{"agentId":format!("{}:actor",h.name()),"sessionId":"actor"}},"facts":{"executionNum":{"status":"valid","value":0}}}),
        }
    }
    #[test]
    fn retry_header_precedes_unrelated_validation() {
        for h in [Harness::Claude, Harness::Codex, Harness::Agy] {
            let mut e = event(h);
            e.event["identity"] = Value::Null;
            e.event["kind"] = Value::Null;
            if h == Harness::Agy {
                e.event["facts"]["executionNum"] = json!({"status":"valid","value":1});
            } else {
                e.event["facts"]["stopHookActive"] = json!({"status":"valid","value":true});
            }
            assert!(matches!(
                decide(&e, || panic!("claims"), |_| panic!("facts")),
                GuardDecision::Allow
            ));
        }
    }
    #[test]
    fn complete_read_precedes_child_and_no_claim_skips_facts() {
        let e = event(Harness::Claude);
        assert!(matches!(
            decide(&e, || Ok(vec![]), |_| panic!("facts")),
            GuardDecision::Allow
        ));
        assert!(matches!(
            decide(&e, || Err("failed".into()), |_| panic!("facts")),
            GuardDecision::Deny(_)
        ));
    }
    #[test]
    fn entire_background_snapshot_and_cron_uncertainty() {
        let kinds = BTreeSet::from(["shell".into(), "subagent".into()]);
        let statuses = BTreeSet::from(["running".into(), "pending".into()]);
        for tasks in [
            json!([{"type":"shell","status":"running"}]),
            json!([{"type":"subagent","status":"pending"}]),
        ] {
            assert_eq!(
                in_flight(
                    &json!({"backgroundTasks":{"status":"valid","value":tasks}}),
                    &kinds,
                    &statuses
                ),
                Ok(true)
            );
        }
        for tasks in [
            json!([{"type":"shell","status":"running"},{"type":"shell","status":"new"}]),
            json!([{"type":"other","status":"running"}]),
            json!([{"type":"shell","status":"running"},{}]),
        ] {
            assert!(
                in_flight(
                    &json!({"backgroundTasks":{"status":"valid","value":tasks}}),
                    &kinds,
                    &statuses
                )
                .is_err()
            );
        }
        assert!(in_flight(&json!({"backgroundTasks":{"status":"valid","value":[]},"sessionCrons":{"status":"invalid"}}),&kinds,&statuses).unwrap_err().1);
        assert_eq!(
            in_flight(
                &json!({"backgroundTasks":{"status":"valid","value":[]}}),
                &kinds,
                &statuses
            ),
            Ok(false)
        );
    }
    fn row(session: &str, live: Liveness) -> (String, Claim, Liveness) {
        let claim=toml::from_str(&format!("owner='tester'\nsession='{session}'\nhost='fixture'\nworktree='/fixture'\nstarted='2026-11-15T00:00:00Z'\nseen='2026-11-15T00:00:00Z'\n")).unwrap();
        ("sci-000001".into(), claim, live)
    }
    fn wake(value: Option<bool>, kinds: Option<&[&str]>, statuses: Option<&[&str]>) -> WakeFacts {
        WakeFacts {
            wake: value,
            kinds: kinds.map(|xs| xs.iter().map(|s| (*s).into()).collect()),
            statuses: statuses.map(|xs| xs.iter().map(|s| (*s).into()).collect()),
        }
    }
    #[test]
    fn liveness_session_spelling_and_related_recovery() {
        for h in [Harness::Claude, Harness::Codex, Harness::Agy] {
            let e = event(h);
            for session in [
                "actor",
                "codex:actor",
                "claude-code:actor",
                "agy:actor",
                "foreign",
            ] {
                for live in [
                    Liveness::Live,
                    Liveness::Stale("dead".into()),
                    Liveness::Unverifiable("remote".into()),
                ] {
                    let protected = !live.is_stale()
                        && (session == "actor"
                            || session == "codex:actor"
                            || (h != Harness::Codex && session != "foreign"));
                    let result = decide(
                        &e,
                        || Ok(vec![row(session, live)]),
                        |_| Ok(wake(Some(false), None, None)),
                    );
                    assert_eq!(
                        matches!(result, GuardDecision::Deny(_)),
                        protected,
                        "{h:?} {session}"
                    );
                }
            }
        }
        let e = event(Harness::Codex);
        let mut related = row("codex:worker", Liveness::Live);
        related.1.root_session = Some("codex:actor".into());
        let result = decide(
            &e,
            || Ok(vec![related]),
            |_| Ok(wake(Some(false), None, None)),
        );
        let GuardDecision::Deny(text) = result else {
            panic!("lost worker claim")
        };
        assert!(text.contains("worker-specific completion evidence"));
    }
    #[test]
    fn wake_matrix_requires_probed_sets_and_agy_blind_kinds() {
        for h in [Harness::Claude, Harness::Codex, Harness::Agy] {
            let mut e = event(h);
            e.event["private"] = json!({"backgroundTasks":{"status":"valid","value":[{"type":"shell","status":"running"}]}});
            e.event["facts"]["fullyIdle"] = json!({"status":"valid","value":false});
            for capability in [None, Some(false), Some(true)] {
                for kinds in [None, Some(&["shell"][..]), Some(&["shell", "subagent"][..])] {
                    for statuses in [None, Some(&["running"][..])] {
                        let allowed = capability == Some(true)
                            && kinds.is_some()
                            && (if h == Harness::Agy {
                                kinds.unwrap().len() == 2
                            } else {
                                statuses.is_some()
                            });
                        let result = decide(
                            &e,
                            || Ok(vec![row("actor", Liveness::Live)]),
                            |_| Ok(wake(capability, kinds, statuses)),
                        );
                        assert_eq!(
                            matches!(result, GuardDecision::Allow),
                            allowed,
                            "{h:?} {capability:?} {kinds:?} {statuses:?}"
                        );
                    }
                }
            }
            e.event["facts"]["fullyIdle"] = json!({"status":"invalid"});
            if h == Harness::Agy {
                assert!(matches!(
                    decide(
                        &e,
                        || Ok(vec![row("actor", Liveness::Live)]),
                        |_| Ok(wake(Some(true), Some(&["shell", "subagent"]), None))
                    ),
                    GuardDecision::Deny(_)
                ));
            }
        }
    }
}
