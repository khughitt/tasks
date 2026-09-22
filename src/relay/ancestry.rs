/// One process on the caller's ancestry chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcEntry {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub start: u64,
}

/// What the walk established about the caller's position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// The nearest recognized harness ancestor.
    Harness(ProcEntry),
    /// The whole chain was readable and held no harness.
    Outside,
    /// The chain could not be read at this pid. Not evidence of a plain shell.
    Unknown(u32),
}

/// `comm` as the kernel reports it, and the agent `harness` relay publishes for it. The
/// `tty` predicate relay's own adapters apply belongs to qualified process resolution, not
/// to this test: a headless harness must not become a shell caller.
pub const HARNESS_COMMS: [(&str, &str); 3] = [
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
];

/// A chain longer than this is malformed; walking it forever is not an option.
const MAX_DEPTH: usize = 64;

/// The `comm` field can contain spaces and parentheses, so everything after the *last* `)`
/// is positional: state is field 1, ppid field 2, start time field 20 — the same rule
/// `claims::parse_proc_stat` documents for fields 3 and 22 of the whole line.
pub fn parse_entry(line: &str) -> Option<ProcEntry> {
    let pid = line.split_whitespace().next()?.parse().ok()?;
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    let comm = line.get(open + 1..close)?.to_string();
    let rest: Vec<&str> = line.get(close + 1..)?.split_whitespace().collect();
    Some(ProcEntry {
        pid,
        ppid: rest.get(1)?.parse().ok()?,
        comm,
        start: rest.get(19)?.parse().ok()?,
    })
}

pub fn read_stat(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()
}

fn read_self_stat() -> Option<String> {
    std::fs::read_to_string("/proc/self/stat").ok()
}

/// Whether this host exposes the process tree at all. Stage 3 of spec §7 asks this, not
/// what the target triple says: a Linux build without a mounted `/proc` cannot establish
/// scope either.
pub fn proc_available() -> bool {
    read_self_stat().is_some()
}

pub fn self_ppid() -> Option<u32> {
    parse_entry(&read_self_stat()?).map(|entry| entry.ppid)
}

/// This caller's own position, resolved against the live `/proc`.
pub fn current_scope() -> Scope {
    match self_ppid() {
        Some(ppid) => walk(ppid, read_stat),
        None => Scope::Unknown(0),
    }
}

/// Walk upward from `from`, returning the nearest harness ancestor. An unreadable level
/// short-circuits to `Unknown`: unknown ancestry is never reported as `Outside`.
pub fn walk(from: u32, read: impl Fn(u32) -> Option<String>) -> Scope {
    let mut pid = from;
    let mut seen = Vec::with_capacity(MAX_DEPTH);
    for _ in 0..MAX_DEPTH {
        if pid == 0 {
            return Scope::Outside;
        }
        if seen.contains(&pid) {
            return Scope::Unknown(pid);
        }
        seen.push(pid);
        let Some(line) = read(pid) else {
            return Scope::Unknown(pid);
        };
        let Some(entry) = parse_entry(&line) else {
            return Scope::Unknown(pid);
        };
        if HARNESS_COMMS.iter().any(|(comm, _)| *comm == entry.comm) {
            return Scope::Harness(entry);
        }
        if entry.pid == 1 {
            return Scope::Outside;
        }
        pid = entry.ppid;
    }
    Scope::Unknown(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// `pid (comm) state ppid …` with starttime at remainder index 19.
    fn stat(pid: u32, comm: &str, ppid: u32, start: u64) -> String {
        let filler = (0..17).map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
        format!("{pid} ({comm}) S {ppid} {filler} {start} x")
    }

    fn tree(rows: &[(u32, &str, u32, u64)]) -> impl Fn(u32) -> Option<String> + use<> {
        let map: HashMap<u32, String> = rows
            .iter()
            .map(|(pid, comm, ppid, start)| (*pid, stat(*pid, comm, *ppid, *start)))
            .collect();
        move |pid| map.get(&pid).cloned()
    }

    #[test]
    fn an_ancestry_entry_parses_a_comm_containing_spaces_and_parentheses() {
        let entry =
            parse_entry("7 (weird ) name) S 3 a b c d e f g h i j k l m n o p q 4242 x").unwrap();
        assert_eq!(entry.pid, 7);
        assert_eq!(entry.comm, "weird ) name");
        assert_eq!(entry.ppid, 3);
        assert_eq!(entry.start, 4242);
    }

    #[test]
    fn an_ancestry_walk_stops_at_the_nearest_harness() {
        // tasks(10) -> codex(9) -> claude(8) -> sh(2) -> init(1)
        let read = tree(&[
            (9, "codex", 8, 900),
            (8, "claude", 2, 800),
            (2, "sh", 1, 200),
            (1, "init", 0, 1),
        ]);
        match walk(9, read) {
            Scope::Harness(entry) => {
                assert_eq!(entry.comm, "codex");
                assert_eq!(entry.pid, 9);
                assert_eq!(entry.start, 900);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_ancestry_walk_of_a_plain_shell_is_outside() {
        let read = tree(&[
            (5, "zsh", 2, 500),
            (2, "systemd", 1, 200),
            (1, "init", 0, 1),
        ]);
        assert!(matches!(walk(5, read), Scope::Outside));
    }

    #[test]
    fn an_ancestry_walk_reports_unknown_rather_than_outside() {
        // 5 is readable, its parent 4 is not: unknown ancestry is never Outside.
        let read = tree(&[(5, "zsh", 4, 500)]);
        assert!(matches!(walk(5, read), Scope::Unknown(4)));
    }

    #[test]
    fn an_ancestry_walk_matches_every_recognized_comm() {
        for (comm, harness) in HARNESS_COMMS {
            let read = tree(&[(9, comm, 1, 900), (1, "init", 0, 1)]);
            match walk(9, read) {
                Scope::Harness(entry) => assert_eq!(entry.comm, comm),
                other => panic!("{comm} -> {other:?}"),
            }
            assert!(!harness.is_empty());
        }
    }

    #[test]
    fn an_ancestry_walk_terminates_on_a_cycle() {
        let read = tree(&[(5, "zsh", 6, 500), (6, "zsh", 5, 600)]);
        assert!(matches!(walk(5, read), Scope::Unknown(_)));
    }
}
