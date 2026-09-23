/// One process on the caller's ancestry chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcEntry {
    pub pid: u32,
    pub ppid: u32,
    pub comm: String,
    pub start: u64,
}

/// The nearest harness process and the harness it belongs to. `comm` alone no longer names
/// the harness: a background Claude Code session runs under its version number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    pub entry: ProcEntry,
    pub harness: &'static str,
}

/// What the walk established about the caller's position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// The nearest harness process, and it is a session.
    Harness(Boundary),
    /// The nearest harness process is a Claude Code hosting process, named by its role.
    /// relay publishes no handle for it, and the walk does not look past it.
    Hosting(Boundary, &'static str),
    /// The whole chain was readable and held no harness.
    Outside,
    /// `file` could not be read for `pid`. Not evidence of a plain shell.
    Unknown { pid: u32, file: &'static str },
}

/// A `comm` that names its harness outright, and the agent `harness` relay publishes for
/// it. A Claude Code process can also be a versioned binary; see `harness_of`. No `tty`
/// predicate applies: a headless harness must not become a shell caller.
pub const HARNESS_COMMS: [(&str, &str); 3] = [
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
];

/// `argv[1]` of the Claude Code processes that host sessions without being one.
pub const CLAUDE_ROLES: [&str; 2] = ["daemon", "bg-pty-host"];

/// The kernel's suffix on the `exe` link of a binary replaced on disk, which a Claude Code
/// auto-update does to every running session.
const DELETED: &str = " (deleted)";

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

/// Where the walk reads the process tree. Injected so tests never touch the real `/proc`.
pub trait ProcSource {
    fn stat(&self, pid: u32) -> Option<String>;
    /// The raw `exe` link text.
    fn exe(&self, pid: u32) -> Option<String>;
    fn cmdline(&self, pid: u32) -> Option<Vec<u8>>;
}

pub struct LiveProc;

impl ProcSource for LiveProc {
    fn stat(&self, pid: u32) -> Option<String> {
        read_stat(pid)
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
pub fn is_version_comm(comm: &str) -> bool {
    comm.contains('.')
        && comm
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The path to compare from a raw `exe` link. `None` is unreadable evidence: a link that is
/// not absolute names nothing that can be compared.
pub fn executable(link: &str) -> Option<&str> {
    let path = link.strip_suffix(DELETED).unwrap_or(link);
    path.starts_with('/').then_some(path)
}

/// `/proc/<pid>/cmdline`: arguments separated by NUL, with one trailing NUL.
pub fn parse_argv(bytes: &[u8]) -> Vec<String> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let body = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    body.split(|b| *b == 0)
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect()
}

/// Which harness `entry` is a process of. `Err` names the file that could not be read: an
/// unreadable executable is unknown, never "not a harness".
fn harness_of(
    entry: &ProcEntry,
    source: &impl ProcSource,
) -> Result<Option<&'static str>, &'static str> {
    if let Some((_, harness)) = HARNESS_COMMS.iter().find(|(comm, _)| *comm == entry.comm) {
        return Ok(Some(*harness));
    }
    // The shape gates the read; the path decides.
    if !is_version_comm(&entry.comm) {
        return Ok(None);
    }
    let link = source.exe(entry.pid).ok_or("exe")?;
    let path = executable(&link).ok_or("exe")?;
    Ok(path
        .ends_with(&format!("/claude/versions/{}", entry.comm))
        .then_some("claude-code"))
}

/// The nearest harness process decides the scope. Only a Claude Code one has its arguments
/// read, and a hosting role ends the walk rather than sending it further out.
fn boundary_scope(boundary: Boundary, source: &impl ProcSource) -> Scope {
    if boundary.harness != "claude-code" {
        return Scope::Harness(boundary);
    }
    let Some(bytes) = source.cmdline(boundary.entry.pid) else {
        return Scope::Unknown {
            pid: boundary.entry.pid,
            file: "cmdline",
        };
    };
    let argv = parse_argv(&bytes);
    match CLAUDE_ROLES
        .iter()
        .find(|role| argv.get(1).map(String::as_str) == Some(**role))
    {
        Some(&role) => Scope::Hosting(boundary, role),
        None => Scope::Harness(boundary),
    }
}

/// This caller's own position, resolved against the live `/proc`.
pub fn current_scope() -> Scope {
    match self_ppid() {
        Some(ppid) => walk(ppid, &LiveProc),
        None => Scope::Unknown {
            pid: 0,
            file: "stat",
        },
    }
}

/// Walk upward from `from`, returning the nearest harness process. An unreadable level
/// short-circuits to `Unknown`: unknown ancestry is never reported as `Outside`.
pub fn walk(from: u32, source: &impl ProcSource) -> Scope {
    let mut pid = from;
    let mut seen = Vec::with_capacity(MAX_DEPTH);
    for _ in 0..MAX_DEPTH {
        if pid == 0 {
            return Scope::Outside;
        }
        if seen.contains(&pid) {
            return Scope::Unknown { pid, file: "stat" };
        }
        seen.push(pid);
        let Some(entry) = source.stat(pid).as_deref().and_then(parse_entry) else {
            return Scope::Unknown { pid, file: "stat" };
        };
        match harness_of(&entry, source) {
            Err(file) => return Scope::Unknown { pid, file },
            Ok(Some(harness)) => return boundary_scope(Boundary { entry, harness }, source),
            Ok(None) => {}
        }
        if entry.pid == 1 {
            return Scope::Outside;
        }
        pid = entry.ppid;
    }
    Scope::Unknown { pid, file: "stat" }
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

    fn describe(scope: &Scope) -> String {
        match scope {
            Scope::Harness(b) => format!("harness {} {}", b.entry.pid, b.harness),
            Scope::Hosting(b, role) => format!("hosting {} {role}", b.entry.pid),
            Scope::Outside => "outside".into(),
            Scope::Unknown { pid, file } => format!("unknown {pid} {file}"),
        }
    }

    const CORPUS: &str = include_str!("../../tests/fixtures/relay/ancestry.json");

    /// relay's `session: null` does not say which of tasks' outcomes applies, so each such
    /// chain names its expectation here (spec §5). An unlisted one fails the corpus test.
    const NULL_SESSION_EXPECTED: [(&str, &str); 3] = [
        ("daemon-nearest", "hosting 300 daemon"),
        ("pty-host-nearest", "hosting 400 bg-pty-host"),
        // relay walks for codex and never reads the claude row's arguments, so the corpus
        // carries argv: null there; tasks meets that claude first and must read them.
        ("claude-ancestor-under-codex", "unknown 20 cmdline"),
    ];

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
    fn every_relay_ancestry_chain_resolves_as_tasks_expects() {
        let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
        let chains = corpus["chains"].as_array().unwrap();
        assert!(!chains.is_empty());
        for chain in chains {
            let name = chain["name"].as_str().unwrap();
            let rows = chain["rows"].as_array().unwrap();
            let tree = corpus_tree(rows);
            // Row 0 is relay's hook, which is where tasks itself sits: walk from its parent.
            let from = rows[0]["ppid"].as_u64().unwrap() as u32;
            let scope = walk(from, &tree);
            match chain["session"].as_u64() {
                Some(pid) => assert_eq!(
                    describe(&scope),
                    format!("harness {pid} {}", chain["harness"].as_str().unwrap()),
                    "{name}"
                ),
                None => {
                    let Some((_, expected)) = NULL_SESSION_EXPECTED
                        .iter()
                        .find(|(chain, _)| *chain == name)
                    else {
                        panic!("{name}: null session with no tasks expectation (spec §5)");
                    };
                    assert_eq!(describe(&scope), *expected, "{name}");
                }
            }
        }
    }

    #[test]
    fn a_readable_claude_under_codex_is_the_claude_boundary() {
        // The corpus cannot express this: relay never reads the claude row's arguments.
        let tree = Tree::default()
            .row(30, "node", 20)
            .row(20, "claude", 10)
            .argv(20, &["claude"])
            .row(10, "zsh", 1);
        assert_eq!(describe(&walk(20, &tree)), "harness 20 claude-code");
    }

    #[test]
    fn an_updated_version_binary_is_still_a_claude_code_boundary() {
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
        assert_eq!(describe(&walk(800, &tree)), "harness 800 claude-code");
    }

    #[test]
    fn a_foreign_version_binary_is_walked_past() {
        let tree = Tree::default()
            .row(450, "1.2.3", 400)
            .exe(450, "/opt/tool/1.2.3")
            .row(400, "claude", 1)
            .argv(400, &["claude"]);
        assert_eq!(describe(&walk(450, &tree)), "harness 400 claude-code");
        // A binary under a *different* version directory is not this comm's binary either.
        let other = Tree::default()
            .row(450, "2.1.280", 400)
            .exe(450, "/home/u/.local/share/claude/versions/2.1.279")
            .row(400, "zsh", 1);
        assert_eq!(describe(&walk(450, &other)), "unknown 1 stat");
    }

    #[test]
    fn an_unreadable_or_relative_executable_is_unknown() {
        let unreadable = Tree::default().row(450, "2.1.280", 1);
        assert_eq!(describe(&walk(450, &unreadable)), "unknown 450 exe");
        let relative = Tree::default()
            .row(450, "2.1.280", 1)
            .exe(450, "versions/2.1.280");
        assert_eq!(describe(&walk(450, &relative)), "unknown 450 exe");
    }

    #[test]
    fn a_plain_chain_never_reads_an_executable_or_arguments() {
        let tree = Tree::default()
            .row(5, "zsh", 4)
            .row(4, "sh", 3)
            .row(3, "sudo", 2)
            .row(2, "systemd", 1)
            .row(1, "init", 0);
        assert_eq!(describe(&walk(5, &tree)), "outside");
        assert!(tree.exe_reads.borrow().is_empty());
        assert!(tree.cmdline_reads.borrow().is_empty());
    }

    #[test]
    fn only_the_nearest_claude_code_process_has_its_arguments_read() {
        let codex = Tree::default().row(9, "codex", 8).row(8, "claude", 1);
        assert_eq!(describe(&walk(9, &codex)), "harness 9 codex");
        assert!(codex.cmdline_reads.borrow().is_empty());

        let claude = Tree::default()
            .row(9, "claude", 8)
            .argv(9, &["claude", "-p", "x"])
            .row(8, "claude", 1);
        assert_eq!(describe(&walk(9, &claude)), "harness 9 claude-code");
        assert_eq!(*claude.cmdline_reads.borrow(), vec![9]);
    }

    #[test]
    fn a_hosting_role_stops_the_walk() {
        for role in CLAUDE_ROLES {
            let tree = Tree::default()
                .row(300, "claude", 200)
                .argv(300, &["/home/u/.local/bin/claude", role, "run"])
                .row(200, "claude", 1)
                .argv(200, &["claude"]);
            assert_eq!(describe(&walk(300, &tree)), format!("hosting 300 {role}"));
        }
    }

    #[test]
    fn an_unreadable_cmdline_is_unknown() {
        let tree = Tree::default().row(300, "claude", 1);
        assert_eq!(describe(&walk(300, &tree)), "unknown 300 cmdline");
    }

    #[test]
    fn an_empty_cmdline_is_a_session() {
        let tree = Tree::default().row(300, "claude", 1).raw_cmdline(300, b"");
        assert_eq!(describe(&walk(300, &tree)), "harness 300 claude-code");
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

    #[test]
    fn an_ancestry_walk_stops_at_the_nearest_harness() {
        let tree = Tree::default()
            .row(9, "codex", 8)
            .row(8, "claude", 2)
            .row(2, "sh", 1)
            .row(1, "init", 0);
        match walk(9, &tree) {
            Scope::Harness(b) => {
                assert_eq!(b.entry.comm, "codex");
                assert_eq!(b.harness, "codex");
                assert_eq!(b.entry.pid, 9);
                assert_eq!(b.entry.start, 90);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_ancestry_walk_reports_unknown_rather_than_outside() {
        let tree = Tree::default().row(5, "zsh", 4);
        assert_eq!(describe(&walk(5, &tree)), "unknown 4 stat");
    }

    #[test]
    fn an_ancestry_walk_matches_every_recognized_comm() {
        for (comm, harness) in HARNESS_COMMS {
            let tree = Tree::default()
                .row(9, comm, 1)
                .argv(9, &[comm])
                .row(1, "init", 0);
            assert_eq!(describe(&walk(9, &tree)), format!("harness 9 {harness}"));
        }
    }

    #[test]
    fn an_ancestry_walk_terminates_on_a_cycle() {
        let tree = Tree::default().row(5, "zsh", 6).row(6, "zsh", 5);
        assert!(matches!(
            walk(5, &tree),
            Scope::Unknown { file: "stat", .. }
        ));
    }
}
