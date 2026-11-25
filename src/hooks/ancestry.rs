//! The harness session process at or above a pid, for the Stop guard only. `relay whoami`
//! answers for its own caller and takes no pid, so the guard cannot ask it about a claim's
//! process. Claim identity never comes from here (`crate::whoami`).

/// One process on an ancestry chain.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcEntry {
    pid: u32,
    ppid: u32,
    comm: String,
    start: u64,
}

/// A `comm` that names a harness outright. A Claude Code process can also be a versioned
/// binary; see `recognize`. No `tty` predicate applies: a headless harness is one too.
const HARNESS_COMMS: [&str; 6] = ["claude", "codex", "opencode", "agy", "pi", "pi-rpc"];

/// `argv[1]` of the Claude Code processes that host sessions without being one.
const CLAUDE_ROLES: [&str; 2] = ["daemon", "bg-pty-host"];

/// The kernel's suffix on the `exe` link of a binary replaced on disk, which a Claude Code
/// auto-update does to every running session.
const DELETED: &str = " (deleted)";

/// A chain longer than this is malformed; walking it forever is not an option.
const MAX_DEPTH: usize = 64;

/// The `comm` field can contain spaces and parentheses, so everything after the *last* `)`
/// is positional: state is field 1, ppid field 2, start time field 20 — the same rule
/// `claims::parse_proc_stat` documents for fields 3 and 22 of the whole line.
fn parse_entry(line: &str) -> Option<ProcEntry> {
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

/// The hook's parent, which relay runs beneath the stopping session.
pub(super) fn self_ppid() -> Option<u32> {
    let line = std::fs::read_to_string("/proc/self/stat").ok()?;
    parse_entry(&line).map(|entry| entry.ppid)
}

/// Where the walk reads the process tree. Injected so tests never touch the real `/proc`.
pub(super) trait ProcSource {
    fn stat(&self, pid: u32) -> Option<String>;
    /// The raw `exe` link text.
    fn exe(&self, pid: u32) -> Option<String>;
    fn cmdline(&self, pid: u32) -> Option<Vec<u8>>;
}

pub(super) struct LiveProc;

impl ProcSource for LiveProc {
    fn stat(&self, pid: u32) -> Option<String> {
        std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()
    }
    fn exe(&self, pid: u32) -> Option<String> {
        std::fs::read_link(format!("/proc/{pid}/exe"))
            .ok()?
            .into_os_string()
            .into_string()
            .ok()
    }
    fn cmdline(&self, pid: u32) -> Option<Vec<u8>> {
        std::fs::read(format!("/proc/{pid}/cmdline")).ok()
    }
}

/// Digits separated by dots, at least two groups: relay's `^[0-9]+(\.[0-9]+)+$`.
fn is_version_comm(comm: &str) -> bool {
    comm.contains('.')
        && comm
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The path to compare from a raw `exe` link. `None` is unreadable evidence: a link that is
/// not absolute names nothing that can be compared.
fn executable(link: &str) -> Option<&str> {
    let path = link.strip_suffix(DELETED).unwrap_or(link);
    path.starts_with('/').then_some(path)
}

/// `/proc/<pid>/cmdline`: arguments separated by NUL, with one trailing NUL.
fn parse_argv(bytes: &[u8]) -> Vec<String> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let body = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    body.split(|b| *b == 0)
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect()
}

enum Harness {
    ClaudeCode,
    Other,
}

/// Which harness `entry` is a process of. `None` is evidence that could not be read: an
/// unreadable executable is unknown, never "not a harness".
fn recognize(entry: &ProcEntry, source: &impl ProcSource) -> Option<Option<Harness>> {
    if entry.comm == "claude" {
        return Some(Some(Harness::ClaudeCode));
    }
    if HARNESS_COMMS.contains(&entry.comm.as_str()) {
        return Some(Some(Harness::Other));
    }
    // The shape gates the read; the path decides.
    if !is_version_comm(&entry.comm) {
        return Some(None);
    }
    let link = source.exe(entry.pid)?;
    let versioned = executable(&link)?.ends_with(&format!("/claude/versions/{}", entry.comm));
    Some(versioned.then_some(Harness::ClaudeCode))
}

/// Walk upward from `from` to the nearest harness process and return its pid and start when
/// it is a session. `None` when there is no harness, when the nearest one is a Claude Code
/// process that hosts sessions without being one, or when any level cannot be read.
pub(super) fn session_process(from: u32, source: &impl ProcSource) -> Option<(u32, u64)> {
    let mut pid = from;
    for _ in 0..MAX_DEPTH {
        if pid == 0 {
            return None;
        }
        let entry = parse_entry(&source.stat(pid)?)?;
        match recognize(&entry, source)? {
            Some(Harness::ClaudeCode) => {
                // Only the nearest harness has its arguments read, and a hosting role ends
                // the walk rather than sending it further out.
                let argv = parse_argv(&source.cmdline(entry.pid)?);
                let hosting = argv
                    .get(1)
                    .is_some_and(|word| CLAUDE_ROLES.contains(&word.as_str()));
                return (!hosting).then_some((entry.pid, entry.start));
            }
            Some(Harness::Other) => return Some((entry.pid, entry.start)),
            None => pid = entry.ppid,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// `pid (comm) state ppid …` with starttime at remainder index 19.
    fn stat(pid: u32, comm: &str, ppid: u32, start: u64) -> String {
        let filler = (0..17).map(|n| n.to_string()).collect::<Vec<_>>().join(" ");
        format!("{pid} ({comm}) S {ppid} {filler} {start} x")
    }

    /// An injected process table. `None` for `exe` or `argv` is an unreadable read.
    /// Every `exe` and `cmdline` read is recorded, so a test can assert what was never read.
    #[derive(Default)]
    struct Tree {
        stat: HashMap<u32, String>,
        exe: HashMap<u32, String>,
        argv: HashMap<u32, Vec<u8>>,
        exe_reads: RefCell<Vec<u32>>,
        cmdline_reads: RefCell<Vec<u32>>,
    }

    impl Tree {
        fn row(mut self, pid: u32, comm: &str, ppid: u32) -> Self {
            self.stat
                .insert(pid, stat(pid, comm, ppid, pid as u64 * 10));
            self
        }
        fn exe(mut self, pid: u32, link: &str) -> Self {
            self.exe.insert(pid, link.to_string());
            self
        }
        fn argv(mut self, pid: u32, args: &[&str]) -> Self {
            let mut bytes = Vec::new();
            for arg in args {
                bytes.extend_from_slice(arg.as_bytes());
                bytes.push(0);
            }
            self.argv.insert(pid, bytes);
            self
        }
        fn raw_cmdline(mut self, pid: u32, bytes: &[u8]) -> Self {
            self.argv.insert(pid, bytes.to_vec());
            self
        }
    }

    impl ProcSource for Tree {
        fn stat(&self, pid: u32) -> Option<String> {
            self.stat.get(&pid).cloned()
        }
        fn exe(&self, pid: u32) -> Option<String> {
            self.exe_reads.borrow_mut().push(pid);
            self.exe.get(&pid).cloned()
        }
        fn cmdline(&self, pid: u32) -> Option<Vec<u8>> {
            self.cmdline_reads.borrow_mut().push(pid);
            self.argv.get(&pid).cloned()
        }
    }

    fn pid_of(found: Option<(u32, u64)>) -> Option<u32> {
        found.map(|(pid, _)| pid)
    }

    const CORPUS: &str = include_str!("../../tests/fixtures/relay/ancestry.json");

    fn corpus_tree(rows: &[serde_json::Value]) -> Tree {
        let mut tree = Tree::default();
        for row in rows {
            let pid = row["pid"].as_u64().unwrap() as u32;
            let ppid = row["ppid"].as_u64().unwrap() as u32;
            tree = tree.row(pid, row["comm"].as_str().unwrap(), ppid);
            if let Some(link) = row["exe"].as_str() {
                tree = tree.exe(pid, link);
            }
            if let Some(args) = row["argv"].as_array() {
                let args: Vec<&str> = args.iter().map(|a| a.as_str().unwrap()).collect();
                tree = tree.argv(pid, &args);
            }
        }
        tree
    }

    #[test]
    fn every_relay_ancestry_chain_finds_relays_session() {
        let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
        let chains = corpus["chains"].as_array().unwrap();
        assert!(!chains.is_empty());
        for chain in chains {
            let rows = chain["rows"].as_array().unwrap();
            // Row 0 is relay's hook, which is where the guard itself sits.
            let from = rows[0]["ppid"].as_u64().unwrap() as u32;
            let found = session_process(from, &corpus_tree(rows));
            assert_eq!(
                pid_of(found).map(u64::from),
                chain["session"].as_u64(),
                "{}",
                chain["name"]
            );
        }
    }

    #[test]
    fn an_updated_version_binary_is_still_a_session() {
        let tree = Tree::default()
            .row(800, "2.1.280", 1)
            .exe(
                800,
                "/home/u/.local/share/claude/versions/2.1.280 (deleted)",
            )
            .argv(
                800,
                &[
                    "/home/u/.local/share/claude/versions/2.1.280",
                    "--session-id",
                    "x",
                ],
            );
        assert_eq!(session_process(800, &tree), Some((800, 8000)));
    }

    #[test]
    fn a_foreign_version_binary_is_walked_past() {
        let tree = Tree::default()
            .row(450, "1.2.3", 400)
            .exe(450, "/opt/tool/1.2.3")
            .row(400, "claude", 1)
            .argv(400, &["claude"]);
        assert_eq!(pid_of(session_process(450, &tree)), Some(400));
        // A binary under a *different* version directory is not this comm's binary either.
        let other = Tree::default()
            .row(450, "2.1.280", 400)
            .exe(450, "/home/u/.local/share/claude/versions/2.1.279")
            .row(400, "codex", 1);
        assert_eq!(pid_of(session_process(450, &other)), Some(400));
    }

    #[test]
    fn unreadable_evidence_finds_no_session() {
        // Each tree has a codex above the unreadable level: the walk must not go past it.
        let above = |tree: Tree| tree.row(7, "codex", 1);
        for tree in [
            above(Tree::default().row(450, "2.1.280", 7)),
            above(
                Tree::default()
                    .row(450, "2.1.280", 7)
                    .exe(450, "versions/2.1.280"),
            ),
            above(Tree::default().row(450, "claude", 7)),
            Tree::default().row(450, "zsh", 449),
        ] {
            assert_eq!(session_process(450, &tree), None);
        }
    }

    #[test]
    fn a_plain_chain_never_reads_an_executable_or_arguments() {
        let tree = Tree::default()
            .row(5, "zsh", 4)
            .row(4, "sh", 3)
            .row(3, "sudo", 2)
            .row(2, "systemd", 1)
            .row(1, "init", 0);
        assert_eq!(session_process(5, &tree), None);
        assert!(tree.exe_reads.borrow().is_empty());
        assert!(tree.cmdline_reads.borrow().is_empty());
    }

    #[test]
    fn only_the_nearest_claude_code_process_has_its_arguments_read() {
        let codex = Tree::default().row(9, "codex", 8).row(8, "claude", 1);
        assert_eq!(session_process(9, &codex), Some((9, 90)));
        assert!(codex.cmdline_reads.borrow().is_empty());
        assert!(codex.exe_reads.borrow().is_empty());

        let claude = Tree::default()
            .row(9, "claude", 8)
            .argv(9, &["claude", "-p", "x"])
            .row(8, "claude", 1);
        assert_eq!(session_process(9, &claude), Some((9, 90)));
        assert_eq!(*claude.cmdline_reads.borrow(), vec![9]);
        assert!(claude.exe_reads.borrow().is_empty());
    }

    #[test]
    fn a_hosting_role_stops_the_walk() {
        for role in CLAUDE_ROLES {
            let tree = Tree::default()
                .row(300, "claude", 200)
                .argv(300, &["/home/u/.local/bin/claude", role, "run"])
                .row(200, "claude", 1)
                .argv(200, &["claude"]);
            assert_eq!(session_process(300, &tree), None, "{role}");
        }
    }

    #[test]
    fn an_empty_cmdline_is_a_session() {
        let tree = Tree::default().row(300, "claude", 1).raw_cmdline(300, b"");
        assert_eq!(pid_of(session_process(300, &tree)), Some(300));
    }

    #[test]
    fn every_recognized_comm_is_a_session() {
        for comm in HARNESS_COMMS {
            let tree = Tree::default().row(9, comm, 1).argv(9, &[comm]);
            assert_eq!(session_process(9, &tree), Some((9, 90)), "{comm}");
        }
    }

    #[test]
    fn a_walk_terminates_on_a_cycle() {
        let tree = Tree::default().row(5, "zsh", 6).row(6, "zsh", 5);
        assert_eq!(session_process(5, &tree), None);
    }

    #[test]
    fn a_version_comm_is_digits_separated_by_dots() {
        for comm in ["2.1.280", "1.2", "10.0.0.1"] {
            assert!(is_version_comm(comm), "{comm}");
        }
        for comm in ["2", "2.", ".2", "2..1", "v2.1", "2.1-rc", "claude", ""] {
            assert!(!is_version_comm(comm), "{comm}");
        }
    }

    #[test]
    fn argv_splits_on_nul_and_drops_one_trailing_nul() {
        assert_eq!(parse_argv(b""), Vec::<String>::new());
        assert_eq!(parse_argv(b"claude\0daemon\0"), vec!["claude", "daemon"]);
        assert_eq!(parse_argv(b"claude\0daemon"), vec!["claude", "daemon"]);
        assert_eq!(parse_argv(b"a\0\0"), vec!["a", ""]);
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
}
