use crate::error::{Error, Result};
use crate::relay::ancestry::{Boundary, Scope};
use crate::relay::snapshot::Snapshot;

/// A matched agent: its id becomes the claim session, its handle the claim's proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub session: String,
    pub pid: u32,
    pub pid_start: u64,
    pub boot_id: String,
    pub host: String,
}

const SET_EXPLICIT: &str =
    "set TASKS_SESSION and TASKS_SESSION_PID to name this session explicitly";

fn refuse(detail: String) -> Error {
    Error::Config(format!("relay identity: {detail}; {SET_EXPLICIT}"))
}

/// The session hint belonging to `harness`, and only to it. A nested harness inherits its
/// parent's environment, so a variable from another harness says nothing about this one.
pub fn hint_for(harness: &str, get: &impl Fn(&str) -> Option<String>) -> Result<Option<String>> {
    match harness {
        "claude-code" => Ok(get("CLAUDE_CODE_SESSION_ID")),
        "codex" => match (get("CODEX_SESSION_ID"), get("CODEX_THREAD_ID")) {
            (Some(session), Some(thread)) if session != thread => Err(refuse(format!(
                "CODEX_SESSION_ID {session:?} conflicts with CODEX_THREAD_ID {thread:?}"
            ))),
            (Some(value), _) | (None, Some(value)) => Ok(Some(value)),
            (None, None) => Ok(None),
        },
        _ => Ok(None),
    }
}

/// Whether `claim_session` names the session `session_id` of `harness`. The same session is
/// written more than one way across the levels: natively a Claude claim stores the raw id,
/// its tagged form is `claude:<id>`, and a relay agent id is `claude-code:<id>`. Exactly
/// those known forms compare equal; any other difference is a real mismatch, never a
/// change of notation.
pub fn same_session(claim_session: &str, harness: &str, session_id: &str) -> bool {
    claim_session == session_id
        || claim_session == format!("{harness}:{session_id}")
        || (harness == "claude-code" && claim_session == format!("claude:{session_id}"))
}

/// Why nothing qualified. Spec §5 requires the harness and platform refusals to name what
/// was wrong, which the bare "no match" line cannot do, so a row that *is* this process is
/// diagnosed against each predicate in turn. A row with no handle cannot be tied to this
/// process, but when the session hint names it, the agent is present and only its handle
/// is missing, which the operator must be able to tell apart from an absent agent.
fn no_match(
    agents: &[crate::relay::snapshot::Agent],
    same_process: &[&crate::relay::snapshot::Agent],
    boundary: &Boundary,
    boot_id: &str,
    hint: Option<&str>,
) -> String {
    let nearest = &boundary.entry;
    let harness = boundary.harness;
    for agent in same_process {
        let process = agent
            .process
            .as_ref()
            .expect("filtered on a present handle");
        if agent.harness != harness {
            return format!(
                "the agent on pid {} is {:?}, but the nearest harness process ({:?}) is {:?}",
                nearest.pid, agent.harness, nearest.comm, harness
            );
        }
        if process.platform != "linux" {
            return format!(
                "agent {} has a {:?} handle, which is not an identity candidate",
                agent.id, process.platform
            );
        }
        if process.boot_id.as_deref() != Some(boot_id) {
            return format!("agent {} was recorded on an earlier boot", agent.id);
        }
    }
    if let Some(hint) = hint
        && let Some(agent) = agents.iter().find(|agent| {
            agent.process.is_none() && agent.harness == harness && agent.session_id == hint
        })
    {
        return format!(
            "agent {} is this {} session but relay recorded no process handle for it; relay \
             records one only for a harness with a controlling terminal",
            agent.id, nearest.comm
        );
    }
    format!(
        "no relay agent matches the nearest {} ancestor, pid {}",
        nearest.comm, nearest.pid
    )
}

/// `Ok(None)` means the caller is out of scope and the native ladder applies unchanged.
/// `load` is invoked only once scope is established as a harness boundary.
pub fn resolve(
    scope: Scope,
    load: impl FnOnce() -> Result<Snapshot>,
    host: &str,
    boot_id: Option<&str>,
    get: &impl Fn(&str) -> Option<String>,
) -> Result<Option<Resolved>> {
    let boundary: Boundary = match scope {
        Scope::Outside => return Ok(None),
        Scope::Unknown { pid, file } => {
            return Err(refuse(format!(
                "cannot read /proc/{pid}/{file}, so this caller's ancestry is unknown"
            )));
        }
        Scope::Hosting(boundary, role) => {
            return Err(refuse(format!(
                "the nearest Claude Code process, pid {}, is its {role} process, not a \
                 session, and relay publishes no handle for it",
                boundary.entry.pid
            )));
        }
        Scope::Harness(boundary) => boundary,
    };
    let nearest = &boundary.entry;
    let harness = boundary.harness;
    let boot_id = boot_id.ok_or_else(|| refuse("the host boot id is unreadable".into()))?;
    // The hint is read before the registry so a self-contradicting pair is reported as
    // itself rather than as a match failure.
    let hint = hint_for(harness, get)?;
    // The loader's own errors name the cause and the path, but they are registry errors,
    // not identity errors: they must still carry the §5 recovery instruction, because a
    // caller whose registry is missing needs to be told how to name itself.
    let snapshot = load().map_err(|error| refuse(error.to_string()))?;

    // Rows whose handle names this very process. Narrowing here first is what lets a
    // refusal say *why*: a row that is this process but fails one predicate is worth
    // naming, an unrelated row is not.
    let same_process: Vec<&crate::relay::snapshot::Agent> = snapshot
        .agents
        .iter()
        .filter(|agent| {
            agent.process.as_ref().is_some_and(|process| {
                process.host == host && process.pid == nearest.pid && process.start == nearest.start
            })
        })
        .collect();

    // Every predicate of spec §4.3 applies *before* cardinality: a row that cannot qualify
    // — wrong harness, wrong platform, an earlier boot — is not a rival candidate, and
    // counting it would turn one good match into a false ambiguity.
    let qualifying: Vec<&crate::relay::snapshot::Agent> = same_process
        .iter()
        .copied()
        .filter(|agent| {
            agent.harness == harness
                && agent.process.as_ref().is_some_and(|process| {
                    process.platform == "linux" && process.boot_id.as_deref() == Some(boot_id)
                })
        })
        .collect();

    let agent = match qualifying.as_slice() {
        [one] => *one,
        [] => {
            return Err(refuse(no_match(
                &snapshot.agents,
                &same_process,
                &boundary,
                boot_id,
                hint.as_deref(),
            )));
        }
        many => {
            let ids: Vec<&str> = many.iter().map(|agent| agent.id.as_str()).collect();
            return Err(refuse(format!(
                "several relay agents match pid {}: {}",
                nearest.pid,
                ids.join(", ")
            )));
        }
    };

    if let Some(hint) = hint
        && hint != agent.session_id
    {
        // A qualifying agent exists, so the disagreement is the environment's, not the
        // registry's.
        return Err(refuse(format!(
            "the environment names session {hint:?} but the matched agent is {}",
            agent.id
        )));
    }

    let process = agent
        .process
        .as_ref()
        .expect("filtered on a present handle");
    Ok(Some(Resolved {
        session: agent.id.clone(),
        pid: process.pid,
        pid_start: process.start,
        boot_id: boot_id.to_string(),
        host: host.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::ancestry::{Boundary, HARNESS_COMMS, ProcEntry};
    use crate::relay::snapshot::{Agent, Handle};

    const BOOT: &str = "0f9d5a1e-1c2b-4d3e-8f4a-5b6c7d8e9f01";
    const OTHER_BOOT: &str = "22222222-2222-4222-8222-222222222222";

    fn boundary(comm: &str, harness: &'static str, pid: u32, start: u64) -> Boundary {
        Boundary {
            entry: ProcEntry {
                pid,
                ppid: 1,
                comm: comm.into(),
                start,
            },
            harness,
        }
    }

    /// A boundary whose comm names its harness outright.
    fn ancestor(comm: &str, pid: u32, start: u64) -> Scope {
        let &(_, harness) = HARNESS_COMMS.iter().find(|(c, _)| *c == comm).unwrap();
        Scope::Harness(boundary(comm, harness, pid, start))
    }

    /// A background Claude Code session: a versioned binary, not `claude`.
    fn version(pid: u32, start: u64) -> Scope {
        Scope::Harness(boundary("2.1.280", "claude-code", pid, start))
    }

    fn agent(
        harness: &str,
        session: &str,
        platform: &str,
        pid: u32,
        start: u64,
        boot: &str,
    ) -> Agent {
        Agent {
            id: format!("{harness}:{session}"),
            harness: harness.into(),
            session_id: session.into(),
            scope: if harness == "opencode" {
                "process"
            } else {
                "session"
            }
            .into(),
            process: Some(Handle {
                platform: platform.into(),
                host: "testhost".into(),
                boot_id: (platform == "linux").then(|| boot.to_string()),
                pid,
                start,
            }),
        }
    }

    fn snap(agents: Vec<Agent>) -> Snapshot {
        Snapshot {
            generation: "11111111-2222-4333-8444-555555555555".into(),
            revision: 1,
            agents,
        }
    }

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| {
            owned
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_empty())
        }
    }

    fn go(
        scope: Scope,
        agents: Vec<Agent>,
        e: &impl Fn(&str) -> Option<String>,
    ) -> Result<Option<Resolved>> {
        resolve(scope, || Ok(snap(agents)), "testhost", Some(BOOT), e)
    }

    #[test]
    fn a_match_adopts_the_agent_id_and_its_proof() {
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![agent("codex", "s1", "linux", 42, 900, BOOT)],
            &env(&[]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s1");
        assert_eq!(resolved.pid, 42);
        assert_eq!(resolved.pid_start, 900);
        assert_eq!(resolved.boot_id, BOOT);
        assert_eq!(resolved.host, "testhost");
    }

    #[test]
    fn a_match_is_not_attempted_out_of_scope() {
        let out = resolve(
            Scope::Outside,
            || panic!("the registry must not be opened out of scope"),
            "testhost",
            Some(BOOT),
            &env(&[]),
        )
        .unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn a_match_refuses_unknown_ancestry_before_opening_the_registry() {
        let error = resolve(
            Scope::Unknown {
                pid: 7,
                file: "stat",
            },
            || panic!("the registry must not be opened for unknown ancestry"),
            "testhost",
            Some(BOOT),
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("/proc/7/stat"), "{error}");
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn a_match_requires_the_harness_to_agree_with_the_ancestor_comm() {
        // Satisfies host, pid, start and boot for a codex ancestor, but is a claude-code
        // row. Without the harness comparison this would be adopted and key the claim to
        // the wrong session.
        let error = go(
            ancestor("codex", 42, 900),
            vec![agent("claude-code", "c1", "linux", 42, 900, BOOT)],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("codex"), "{error}");
    }

    #[test]
    fn a_match_refuses_a_darwin_handle() {
        let error = go(
            ancestor("codex", 42, 900),
            vec![agent("codex", "s1", "darwin", 42, 900, BOOT)],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("darwin"), "{error}");
    }

    #[test]
    fn a_match_requires_every_proof_component() {
        let agents = || vec![agent("codex", "s1", "linux", 42, 900, BOOT)];
        assert!(go(ancestor("codex", 43, 900), agents(), &env(&[])).is_err()); // pid
        assert!(go(ancestor("codex", 42, 901), agents(), &env(&[])).is_err()); // start
        assert!(
            resolve(
                ancestor("codex", 42, 900),
                || Ok(snap(agents())),
                "elsewhere",
                Some(BOOT),
                &env(&[])
            )
            .is_err()
        ); // host
        assert!(
            resolve(
                ancestor("codex", 42, 900),
                || Ok(snap(agents())),
                "testhost",
                Some(OTHER_BOOT),
                &env(&[])
            )
            .is_err()
        ); // boot
    }

    #[test]
    fn a_match_on_an_empty_registry_is_the_explicit_identity_error() {
        // The real Codex case: SessionStart runs at the first turn, not at launch.
        let error = go(ancestor("codex", 42, 900), vec![], &env(&[]))
            .unwrap_err()
            .to_string();
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn a_match_is_ambiguous_only_among_fully_qualifying_candidates() {
        // Two rows share the handle and both qualify: ambiguous.
        let error = go(
            ancestor("codex", 42, 900),
            vec![
                agent("codex", "s1", "linux", 42, 900, BOOT),
                agent("codex", "s2", "linux", 42, 900, BOOT),
            ],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("codex:s1") && error.contains("codex:s2"),
            "{error}"
        );

        // A stale row from an earlier boot shares host, pid and start but cannot qualify.
        // Exactly one candidate satisfies the spec, so this must resolve, not refuse.
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![
                agent("codex", "old", "linux", 42, 900, OTHER_BOOT),
                agent("codex", "s1", "linux", 42, 900, BOOT),
            ],
            &env(&[]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s1");

        // Likewise a wrong-harness row and a darwin row alongside one good row.
        let resolved = go(
            ancestor("codex", 42, 900),
            vec![
                agent("claude-code", "c1", "linux", 42, 900, BOOT),
                agent("codex", "d1", "darwin", 42, 900, BOOT),
                agent("codex", "s1", "linux", 42, 900, BOOT),
            ],
            &env(&[]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(resolved.session, "codex:s1");
    }

    #[test]
    fn a_match_names_a_handle_less_agent_for_this_session() {
        // relay publishes process: null for a harness without a controlling terminal. The
        // agent is present and the hint identifies it; only its handle is missing, and the
        // refusal must say so rather than claim no agent matches.
        let mut headless = agent("claude-code", "c1", "linux", 42, 900, BOOT);
        headless.process = None;
        let error = go(
            ancestor("claude", 42, 900),
            vec![headless],
            &env(&[("CLAUDE_CODE_SESSION_ID", "c1")]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("claude-code:c1"), "{error}");
        assert!(error.contains("no process handle"), "{error}");
        assert!(error.contains("controlling terminal"), "{error}");
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }

    #[test]
    fn a_handle_less_agent_for_another_session_is_not_named() {
        let mut headless = agent("claude-code", "other", "linux", 42, 900, BOOT);
        headless.process = None;
        let error = go(
            ancestor("claude", 42, 900),
            vec![headless],
            &env(&[("CLAUDE_CODE_SESSION_ID", "c1")]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("no relay agent matches"), "{error}");
    }

    #[test]
    fn a_hint_from_another_harness_is_ignored() {
        // A codex session nested under a claude one inherits CLAUDE_CODE_SESSION_ID. Its
        // own proof and its own variable are valid and must be accepted.
        let e = env(&[
            ("CLAUDE_CODE_SESSION_ID", "outer"),
            ("CODEX_SESSION_ID", "s1"),
        ]);
        assert_eq!(
            go(
                ancestor("codex", 42, 900),
                vec![agent("codex", "s1", "linux", 42, 900, BOOT)],
                &e
            )
            .unwrap()
            .unwrap()
            .session,
            "codex:s1"
        );
    }

    #[test]
    fn a_hint_for_the_nearest_harness_must_agree() {
        let agents = || vec![agent("claude-code", "c1", "linux", 42, 900, BOOT)];
        let wrong = env(&[("CLAUDE_CODE_SESSION_ID", "different")]);
        assert!(go(ancestor("claude", 42, 900), agents(), &wrong).is_err());
        let right = env(&[("CLAUDE_CODE_SESSION_ID", "c1")]);
        assert!(
            go(ancestor("claude", 42, 900), agents(), &right)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn a_hint_pair_for_codex_must_agree_with_itself() {
        let agents = || vec![agent("codex", "s1", "linux", 42, 900, BOOT)];
        let split = env(&[("CODEX_SESSION_ID", "s1"), ("CODEX_THREAD_ID", "other")]);
        assert!(go(ancestor("codex", 42, 900), agents(), &split).is_err());
        let only_thread = env(&[("CODEX_THREAD_ID", "s1")]);
        assert!(
            go(ancestor("codex", 42, 900), agents(), &only_thread)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn a_hint_that_is_empty_is_unset_and_opencode_compares_none() {
        let claude = vec![agent("claude-code", "c1", "linux", 42, 900, BOOT)];
        let empty = env(&[("CLAUDE_CODE_SESSION_ID", "")]);
        assert!(
            go(ancestor("claude", 42, 900), claude, &empty)
                .unwrap()
                .is_some()
        );

        let opencode = vec![agent("opencode", "o1", "linux", 42, 900, BOOT)];
        let noisy = env(&[("CLAUDE_CODE_SESSION_ID", "x"), ("CODEX_SESSION_ID", "y")]);
        assert!(
            go(ancestor("opencode", 42, 900), opencode, &noisy)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn a_session_compares_equal_across_its_known_representations_only() {
        for stored in ["c1", "claude:c1", "claude-code:c1"] {
            assert!(same_session(stored, "claude-code", "c1"), "{stored}");
        }
        for stored in ["s1", "codex:s1"] {
            assert!(same_session(stored, "codex", "s1"), "{stored}");
        }
        assert!(!same_session("claude-code:other", "claude-code", "c1"));
        assert!(!same_session("sid:4242", "claude-code", "c1"));
        assert!(!same_session("codex:c1", "claude-code", "c1"));
    }

    #[test]
    fn a_version_boundary_matches_a_claude_code_row_and_checks_its_hint() {
        let agents = || vec![agent("claude-code", "bg1", "linux", 800, 900, BOOT)];
        let resolved = go(version(800, 900), agents(), &env(&[])).unwrap().unwrap();
        assert_eq!(resolved.session, "claude-code:bg1");
        assert_eq!(resolved.pid, 800);
        let wrong = env(&[("CLAUDE_CODE_SESSION_ID", "outer")]);
        assert!(go(version(800, 900), agents(), &wrong).is_err());
    }

    #[test]
    fn a_version_boundary_refuses_a_codex_row_on_its_process() {
        let error = go(
            version(800, 900),
            vec![agent("codex", "s1", "linux", 800, 900, BOOT)],
            &env(&[]),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("claude-code"), "{error}");
    }

    #[test]
    fn a_hosting_process_is_refused_before_the_registry_is_opened() {
        for role in ["daemon", "bg-pty-host"] {
            let error = resolve(
                Scope::Hosting(boundary("claude", "claude-code", 300, 3000), role),
                || panic!("the registry must not be opened for a hosting process"),
                "testhost",
                Some(BOOT),
                &env(&[]),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains("300") && error.contains(role), "{error}");
            assert!(error.contains("not a session"), "{error}");
            assert!(error.contains("TASKS_SESSION"), "{error}");
        }
    }

    #[test]
    fn an_unknown_executable_or_cmdline_is_named() {
        for file in ["exe", "cmdline"] {
            let error = resolve(
                Scope::Unknown { pid: 450, file },
                || panic!("the registry must not be opened for unknown ancestry"),
                "testhost",
                Some(BOOT),
                &env(&[]),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains(&format!("/proc/450/{file}")), "{error}");
        }
    }
}
