# Relay Session Process Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tasks' relay identity find the same session process relay now publishes (versioned Claude Code binaries, no daemon or pty host), read registry schema 2 only, and ship that as tasks 0.2.0.

**Architecture:** The ancestry walk in `src/relay/ancestry.rs` gains two reads, `exe` for version-shaped `comm`s and `cmdline` for the nearest Claude Code process, behind a `ProcSource` trait, and returns a `Boundary` that carries its harness. Resolution (`src/relay/resolve.rs`) and ownership proof (`claims::proves_ownership`) key on that harness instead of `comm`. The snapshot reader accepts schema 2 and refuses schema 1 as superseded. relay's `ancestry.json` corpus is vendored and every chain is run through the walk.

**Tech Stack:** Rust 2024, `serde_json` (already a dependency) for the corpus, `/proc` for `stat`, `exe` and `cmdline`. No new dependencies.

**Spec:** `docs/specs/2026-09-23-relay-session-process-design.md`, amending `docs/specs/2026-09-22-relay-ancestry-identity-design.md`. Upstream: relay `docs/specs/2026-09-23-session-process-handle.md` at relay `45f4c47`.

## Global Constraints

- Recognition: `claude-code` is `comm == "claude"`, or `comm` matching `^[0-9]+(\.[0-9]+)+$` whose `readlink /proc/<pid>/exe`, with one trailing ` (deleted)` stripped, ends in `/claude/versions/<comm>`. `codex` is `comm == "codex"`; `opencode` is `comm == "opencode"`.
- `exe` is read only for a version-shaped `comm`. `cmdline` is read only for the nearest harness process, and only when it is `claude-code`.
- Roles: `argv[1]` of `daemon` or `bg-pty-host` makes the nearest Claude Code process a hosting process. The walk never continues past the nearest harness process.
- Every failed read (`stat`, `exe`, `cmdline`), and an `exe` link that is not absolute, is unknown ancestry, never "not a harness".
- The registry reader accepts `schema: 2` only. `schema: 1` is refused with exactly `schema 1 is superseded by schema 2; run \`relay reap\`, or wait for any hook event`; anything else is `schema must be 2`.
- The vendored corpus is `tests/fixtures/relay/ancestry.json`, byte for byte from relay `45f4c47` `test/fixtures/ancestry.json`.
- The crate version becomes `0.2.0`, and the README names 0.2.0 as the minimum on a host where relay publishes schema 2 with the opt-in on.
- With relay identity off, behaviour is unchanged. Nothing outside `src/relay/`, `claims::proves_ownership`, the tests, the README, `Cargo.toml`/`Cargo.lock` and the two spec files changes.
- JSON output shapes do not change.
- Every commit builds green under `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`; run `cargo fmt` before each gate. The pre-commit hook runs `just check`.
- Run `just test-fast <filter>` while working and `just gate` before each task's final commit. Never run `cargo test` directly.
- Live tests of this worktree's binary run it by explicit path (`.worktrees/relay-schema2/target/debug/tasks`); no host launcher, symlink or config is repointed.

## Review Focus

1. **An auto-updated background session** (`exe` link ends in ` (deleted)`): must still be a `claude-code` boundary. Pinned in Task 1 (`an_updated_version_binary_is_still_a_claude_code_boundary`).
2. **A version-shaped process that is not Claude** (`/opt/tool/1.2.3`) between tasks and a real `claude`: must be walked past, not refused and not adopted. Pinned by the corpus chain `foreign-version-comm` and by `a_foreign_version_binary_is_walked_past` in Task 1.
3. **A plain shell with relay on**: must never read any `exe` or `cmdline` (a root-owned process would be unreadable and turn a plain shell into a refusal). Pinned in Task 1 (`a_plain_chain_never_reads_an_executable_or_arguments`).
4. **A `claude` process with an empty cmdline** (a zombie or kernel-cleared argv): a session, not unknown and not hosting. Pinned in Task 1 (`an_empty_cmdline_is_a_session`).
5. **A schema-1 registry left behind by an old relay**: refused with the supersession text, never read as empty. Pinned in Task 2 by a unit test and an end-to-end test.

---

### Task 1: The walk and what reads it

**Files:**
- Create: `tests/fixtures/relay/ancestry.json` (copied)
- Create: `tests/fixtures/relay/README.md`
- Modify: `src/relay/ancestry.rs` (whole file: types, walk, tests)
- Modify: `src/relay/resolve.rs` (imports, `harness_for` removed, `hint_for`, `same_session`, `no_match`, `resolve`, tests)
- Modify: `src/claims.rs:843-873` (`proves_ownership`) and its tests at `:908-1052`

**Interfaces:**
- Consumes: nothing new.
- Produces, in `crate::relay::ancestry`:
  - `pub struct Boundary { pub entry: ProcEntry, pub harness: &'static str }` (`Debug, Clone, PartialEq, Eq`)
  - `pub enum Scope { Harness(Boundary), Hosting(Boundary, &'static str), Outside, Unknown { pid: u32, file: &'static str } }`
  - `pub trait ProcSource { fn stat(&self, pid: u32) -> Option<String>; fn exe(&self, pid: u32) -> Option<String>; fn cmdline(&self, pid: u32) -> Option<Vec<u8>>; }`
  - `pub struct LiveProc;` implementing `ProcSource` against `/proc`
  - `pub fn walk(from: u32, source: &impl ProcSource) -> Scope`
  - `pub fn is_version_comm(comm: &str) -> bool`, `pub fn executable(link: &str) -> Option<&str>`, `pub fn parse_argv(bytes: &[u8]) -> Vec<String>`
  - `pub const HARNESS_COMMS` (unchanged values), `pub const CLAUDE_ROLES: [&str; 2] = ["daemon", "bg-pty-host"]`
- Produces, in `crate::relay::resolve`: `pub fn hint_for(harness: &str, get) -> Result<Option<String>>` and `pub fn same_session(claim_session: &str, harness: &str, session_id: &str) -> bool`. Both now take the **harness** (`"claude-code"`), not the comm.

- [ ] **Step 1: Vendor the corpus**

```bash
RELAY=$(tasks root relay-cb616b | jq -r .root)
mkdir -p tests/fixtures/relay
git -C "$RELAY" show 45f4c47:test/fixtures/ancestry.json > tests/fixtures/relay/ancestry.json
git -C "$RELAY" show 45f4c47:test/fixtures/ancestry.json | cmp - tests/fixtures/relay/ancestry.json
```

Expected: `cmp` prints nothing.

Write `tests/fixtures/relay/README.md`:

```markdown
# Vendored relay fixtures

`ancestry.json` is relay's cross-language contract for the session-process rule, copied
byte for byte from relay `test/fixtures/ancestry.json` at revision `45f4c47`. Its own
`schema` field is the fixture format version, not the registry schema.

`src/relay/ancestry.rs` runs every chain through tasks' walk. Refresh it only by copying
the file from a newer relay revision and updating the revision above in the same commit.
A new chain whose `session` is null fails that test until its tasks-side expectation is
added to `NULL_SESSION_EXPECTED` there.
```

- [ ] **Step 2: Write the failing ancestry tests**

Replace the whole `#[cfg(test)] mod tests` of `src/relay/ancestry.rs` with:

```rust
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
            self.stat.insert(pid, stat(pid, comm, ppid, pid as u64 * 10));
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
                    let Some((_, expected)) =
                        NULL_SESSION_EXPECTED.iter().find(|(chain, _)| *chain == name)
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
            .exe(800, "/home/u/.local/share/claude/versions/2.1.280 (deleted)")
            .argv(800, &["/home/u/.local/share/claude/versions/2.1.280", "--session-id", "x"]);
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
        assert!(matches!(walk(5, &tree), Scope::Unknown { file: "stat", .. }));
    }
}
```

Note the second case of `a_foreign_version_binary_is_walked_past`: the walk passes 450, reaches `zsh` at 400, then pid 1, which the tree does not hold, so `unknown 1 stat`. That is the assertion: it proves 450 was walked past rather than adopted.

- [ ] **Step 3: Run the ancestry tests to verify they fail**

Run: `just test-fast ancestry`
Expected: compile errors (`Boundary`, `ProcSource`, `Scope::Hosting` and friends do not exist).

- [ ] **Step 4: Implement the walk**

In `src/relay/ancestry.rs`, keep `ProcEntry`, `MAX_DEPTH`, `parse_entry`, `read_stat`, `read_self_stat`, `proc_available` and `self_ppid` as they are. Replace `Scope`, the `HARNESS_COMMS` doc comment, `current_scope` and `walk` with:

```rust
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
fn harness_of(entry: &ProcEntry, source: &impl ProcSource) -> Result<Option<&'static str>, &'static str> {
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
        Some(role) => Scope::Hosting(boundary, *role),
        None => Scope::Harness(boundary),
    }
}

/// This caller's own position, resolved against the live `/proc`.
pub fn current_scope() -> Scope {
    match self_ppid() {
        Some(ppid) => walk(ppid, &LiveProc),
        None => Scope::Unknown { pid: 0, file: "stat" },
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
```

- [ ] **Step 5: Run the ancestry tests**

Run: `just test-fast ancestry`
Expected: the crate still fails to compile, now in `resolve.rs` and `claims.rs`. The ancestry module itself has no errors. Continue.

- [ ] **Step 6: Write the failing resolution and proof tests**

In `src/relay/resolve.rs` tests, replace the `ancestor` helper and add `version`:

```rust
    use crate::relay::ancestry::{Boundary, HARNESS_COMMS, ProcEntry};

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
        let (_, harness) = HARNESS_COMMS.iter().find(|(c, _)| *c == comm).unwrap();
        Scope::Harness(boundary(comm, *harness, pid, start))
    }

    /// A background Claude Code session: a versioned binary, not `claude`.
    fn version(pid: u32, start: u64) -> Scope {
        Scope::Harness(boundary("2.1.280", "claude-code", pid, start))
    }
```

Change `a_match_refuses_unknown_ancestry_before_opening_the_registry` to pass `Scope::Unknown { pid: 7, file: "stat" }` and assert `error.contains("/proc/7/stat")`. In `a_session_compares_equal_across_its_known_representations_only`, replace every `"claude"` second argument with `"claude-code"`. Then add:

```rust
    #[test]
    fn a_version_boundary_matches_a_claude_code_row_and_checks_its_hint() {
        let agents = || vec![agent("claude-code", "bg1", "linux", 800, 900, BOOT)];
        let resolved = go(version(800, 900), agents(), &env(&[]))
            .unwrap()
            .unwrap();
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
```

In `src/claims.rs` tests, replace `nearest` and the `use` line:

```rust
    use crate::relay::ancestry::{Boundary, HARNESS_COMMS, ProcEntry, Scope};

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

    fn nearest(comm: &str, pid: u32, start: u64) -> Scope {
        let (_, harness) = HARNESS_COMMS.iter().find(|(c, _)| *c == comm).unwrap();
        Scope::Harness(boundary(comm, *harness, pid, start))
    }
```

Change `Scope::Unknown(4)` in `a_proof_needs_an_established_harness_boundary` to `Scope::Unknown { pid: 4, file: "stat" }`, and add:

```rust
    #[test]
    fn a_proof_is_never_established_by_a_hosting_process() {
        let claim = claim_of("claude-code:c1", Some(300), Some(3000));
        assert!(!proves_ownership(
            &claim,
            &Scope::Hosting(boundary("claude", "claude-code", 300, 3000), "daemon"),
            "testhost",
            Some(PROOF_BOOT),
            &no_env()
        ));
    }

    #[test]
    fn a_proof_at_a_version_boundary_is_its_own_not_the_outer_sessions() {
        // The outer claude(500) holds the claim; a nested 2.1.280(600) runs under it, with
        // the outer session's id inherited. Its boundary is 600, so it proves nothing.
        let outer = claim_of("claude-code:c1", Some(500), Some(5000));
        let inherited = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "c1".to_string());
        let nested = Scope::Harness(boundary("2.1.280", "claude-code", 600, 6000));
        assert!(!proves_ownership(
            &outer,
            &nested,
            "testhost",
            Some(PROOF_BOOT),
            &inherited
        ));
        // A claim that the version process itself holds is proved, and its hint is the
        // Claude Code variable.
        let own = claim_of("claude-code:bg1", Some(600), Some(6000));
        let hint = |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "bg1".to_string());
        assert!(proves_ownership(&own, &nested, "testhost", Some(PROOF_BOOT), &hint));
        let contradicted =
            |key: &str| (key == "CLAUDE_CODE_SESSION_ID").then(|| "other".to_string());
        assert!(!proves_ownership(
            &own,
            &nested,
            "testhost",
            Some(PROOF_BOOT),
            &contradicted
        ));
    }
```

- [ ] **Step 7: Implement resolution and proof by harness**

In `src/relay/resolve.rs`:

Replace the import line with `use crate::relay::ancestry::{Boundary, Scope};` and delete `fn harness_for`.

Replace `hint_for` and `same_session`:

```rust
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
```

In `no_match`, replace the `nearest: &ProcEntry, harness: &str` parameters with `boundary: &Boundary`, bind `let nearest = &boundary.entry; let harness = boundary.harness;` at the top, and change the harness-disagreement message to:

```rust
            return format!(
                "the agent on pid {} is {:?}, but the nearest harness process ({:?}) is {:?}",
                nearest.pid, agent.harness, nearest.comm, harness
            );
```

Leave the handle-less message alone in this task; Task 2 rewrites it.

In `resolve`, replace the opening down to `let boot_id = …` with:

```rust
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
    let hint = hint_for(harness, get)?;
```

and pass `&boundary` in place of `&nearest, harness` in the `no_match` call. The remaining uses of `nearest.pid` and `nearest.start` read the same fields through the reference.

In `src/claims.rs` `proves_ownership`:

```rust
    let crate::relay::ancestry::Scope::Harness(boundary) = scope else {
        return false;
    };
    let nearest = &boundary.entry;
```

and change the final match to:

```rust
    match crate::relay::resolve::hint_for(boundary.harness, get) {
        Ok(Some(hint)) => {
            crate::relay::resolve::same_session(&claim.session, boundary.harness, &hint)
        }
        Ok(None) => true,
        Err(_) => false,
    }
```

- [ ] **Step 8: Run the tests**

Run: `just test-fast relay` then `just test-fast proof`
Expected: all pass, including `every_relay_ancestry_chain_resolves_as_tasks_expects`.

Then `just test-fast` for the whole non-ignored suite. The end-to-end relay tests in `tests/cli.rs` run `codex` and `claude` shims; the `claude` shim is `sh -c`, so `argv[1]` is `-c` and it stays a session. Expected: all pass.

- [ ] **Step 9: Gate and commit**

Run: `just gate`
Expected: pass.

```bash
git add tests/fixtures/relay src/relay/ancestry.rs src/relay/resolve.rs src/claims.rs
git commit -m "feat(relay): recognize the session's own process as relay does"
```

---

### Task 2: Registry schema 2

**Files:**
- Modify: `src/relay/snapshot.rs` (`parse`, tests)
- Modify: `src/relay/resolve.rs` (`no_match` handle-less text and its test)
- Modify: `tests/common/mod.rs:299` (`WRITE_REGISTRY`)
- Modify: `tests/cli.rs` (three inline empty registries, one new test)

**Interfaces:**
- Consumes: Task 1's `Boundary` in `no_match`.
- Produces: `snapshot::parse` accepting schema 2 only. `common::WRITE_REGISTRY` writes schema 2, which Task 3 relies on.

- [ ] **Step 1: Write the failing snapshot tests**

In `src/relay/snapshot.rs` tests, move every test registry to schema 2:

```bash
sed -i 's/"schema":1,/"schema":2,/g' src/relay/snapshot.rs
```

That turns the refusal case `("schema 2", r#"{"schema":2,…` into a duplicate; edit that one line so the case reads `("schema 3", r#"{"schema":3,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#.into()),`. Confirm with `grep -n '"schema":' src/relay/snapshot.rs` that the only non-2 value left is that `3`, plus the `snapshot_json` helper now writing `2`.

Add:

```rust
    #[test]
    fn a_schema_one_registry_is_superseded_not_read() {
        let text = r#"{"schema":1,"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{}}"#;
        let error = parse(text).unwrap_err().to_string();
        assert!(
            error.contains("schema 1 is superseded by schema 2"),
            "{error}"
        );
        assert!(error.contains("relay reap"), "{error}");
        assert!(error.contains("wait for any hook event"), "{error}");
    }

    #[test]
    fn a_registry_of_an_unknown_schema_is_invalid() {
        for schema in ["0", "3", "\"2\"", "null"] {
            let text = format!(
                r#"{{"schema":{schema},"generation":"11111111-2222-4333-8444-555555555555","revision":1,"agents":{{}}}}"#
            );
            let error = parse(&text).unwrap_err().to_string();
            assert!(error.contains("schema must be 2"), "{schema}: {error}");
        }
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `just test-fast snapshot`
Expected: FAIL. Every schema-2 fixture is refused with `schema must be 1`.

- [ ] **Step 3: Implement**

In `parse`, replace the schema check with:

```rust
    // Schema 2 changed what `process` names (relay spec 2026-09-23). A schema-1 file can
    // hold rows that borrowed another session's handle, so it is refused, never read.
    match raw.get("schema").and_then(serde_json::Value::as_u64) {
        Some(2) => {}
        Some(1) => {
            return Err(invalid(
                "schema 1 is superseded by schema 2; run `relay reap`, or wait for any hook event",
            ));
        }
        _ => return Err(invalid("schema must be 2")),
    }
```

Run: `just test-fast snapshot`
Expected: PASS.

- [ ] **Step 4: Rewrite the handle-less refusal**

In `resolve.rs`'s `no_match`, replace the handle-less `format!` with:

```rust
        return format!(
            "agent {} is this {} session but relay recorded no process handle for it: relay's \
             hook found no {} session process among its ancestors, or found only a daemon or \
             pty host",
            agent.id, harness, harness
        );
```

In the test `a_match_names_a_handle_less_agent_for_this_session`, replace the comment's first sentence with "relay publishes process: null when its hook finds no session process of the harness." and replace `assert!(error.contains("controlling terminal"), "{error}");` with `assert!(error.contains("daemon or pty host"), "{error}");`. Check nothing else still says it: `grep -rn "controlling terminal" src` prints nothing.

- [ ] **Step 5: Move the end-to-end registries to schema 2 and add the supersession test**

```bash
sed -i 's/{"schema":1,"generation"/{"schema":2,"generation"/' tests/common/mod.rs
sed -i 's/{{\\"schema\\":1,/{{\\"schema\\":2,/g' tests/cli.rs
grep -n 'schema\\":1\|"schema":1' tests/common/mod.rs tests/cli.rs
```

Expected: the final `grep` prints nothing.

Append to `tests/cli.rs`, after `an_acceptance_world_readable_registry_is_refused`:

```rust
#[test]
fn an_acceptance_schema_one_registry_is_superseded() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);

    // A correct row for this very process, but in a schema-1 file: refused, not matched.
    let script = format!(
        "{}\nwrite_registry\n\
         sed -i 's/\"schema\":2,/\"schema\":1,/' \"$RELAY_STATE_DIR/agents.json\"\n\
         \"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "codex", "s1")
    );
    let out = common::harness_shim(&dir, env.home.path(), "codex", &script);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("superseded"), "{text}");
    assert!(text.contains("relay reap"), "{text}");
    assert!(text.contains("TASKS_SESSION"), "{text}");
    assert!(!env.claim_store("sci").exists() || !std::fs::read_to_string(env.claim_store("sci")).unwrap().contains("codex:s1"));
}
```

- [ ] **Step 6: Run and commit**

Run: `just test-fast acceptance` then `just gate`
Expected: pass.

```bash
git add src/relay/snapshot.rs src/relay/resolve.rs tests/common/mod.rs tests/cli.rs
git commit -m "feat(relay): read registry schema 2 and refuse a superseded schema-1 file"
```

---

### Task 3: End-to-end acceptance under a versioned Claude Code binary

**Files:**
- Modify: `tests/common/mod.rs` (split `harness_shim`, add `claude_version_binary`, `harness_shim_at`, `shim_with_argument`)
- Modify: `tests/cli.rs` (five tests, after `an_acceptance_schema_one_registry_is_superseded`)

**Interfaces:**
- Consumes: Task 1's walk (through the built binary) and Task 2's schema-2 `WRITE_REGISTRY`.
- Produces, in `tests/common`:
  - `pub fn claude_version_binary(home: &Path, version: &str) -> PathBuf`
  - `pub fn harness_shim_at(program: &Path, dir: &Path, home: &Path, script: &str) -> Output`
  - `pub fn shim_with_argument(program: &Path, word: &str, dir: &Path, home: &Path, script: &str) -> Output`
  - `harness_shim` keeps its signature and behaviour.

- [ ] **Step 1: Add the shim helpers**

In `tests/common/mod.rs`, split `harness_shim` so the environment is written once. Replace the function with:

```rust
pub fn harness_shim(dir: &Path, home: &Path, comm: &str, script: &str) -> std::process::Output {
    let shim = home.join(comm);
    if !shim.exists() {
        std::os::unix::fs::symlink("/bin/sh", &shim).unwrap();
    }
    harness_shim_at(&shim, dir, home, script)
}

/// `harness_shim` for a shell already at `program`, whose basename becomes the `comm`.
pub fn harness_shim_at(
    program: &Path,
    dir: &Path,
    home: &Path,
    script: &str,
) -> std::process::Output {
    shim_command(program, dir, home)
        .arg("-c")
        .arg(format!("{script}\nexit $?\n"))
        .output()
        .unwrap()
}

/// Run `script` under `program` with `word` as its `argv[1]`. `sh -c` cannot put a chosen
/// word there, so the script is a *file* named `word`, run by that relative name from its
/// own directory; it `cd`s to `dir` first. The file is read by the shell, never
/// `execve`d, so writing it cannot cause the `ETXTBSY` race `harness_shim` documents.
pub fn shim_with_argument(
    program: &Path,
    word: &str,
    dir: &Path,
    home: &Path,
    script: &str,
) -> std::process::Output {
    let scripts = home.join("shim-scripts");
    std::fs::create_dir_all(&scripts).unwrap();
    std::fs::write(
        scripts.join(word),
        format!("cd '{}' || exit 90\n{script}\nexit $?\n", dir.display()),
    )
    .unwrap();
    shim_command(program, &scripts, home)
        .arg(word)
        .output()
        .unwrap()
}

/// A copy of `sh` at `<home>/.local/share/claude/versions/<version>`: its `comm` is the
/// version and its `exe` ends in the versions path, as a background Claude Code session's
/// does. Unlike `harness_shim`, this has to be a copy, since a symlink's `exe` resolves to
/// `/bin/sh`'s target. The copy is made by a `cp` child process, so the writable descriptor
/// lives only in that child and no sibling test thread can inherit it across a fork.
pub fn claude_version_binary(home: &Path, version: &str) -> std::path::PathBuf {
    let dir = home.join(".local/share/claude/versions");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(version);
    if !path.exists() {
        let status = std::process::Command::new("cp")
            .arg("/bin/sh")
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success(), "cp /bin/sh {}", path.display());
    }
    path
}

fn shim_command(program: &Path, dir: &Path, home: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command
        .current_dir(dir)
        .env("HOME", home)
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_STATE_HOME")
        .env_remove("TASKS_FORMAT")
        .env_remove("TASKS_OWNER")
        .env_remove("TASKS_SESSION")
        .env_remove("TASKS_SESSION_PID")
        .env_remove("TASKS_MODEL")
        .env_remove("TASKS_AGENT")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_PID")
        .env_remove("CODEX_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env("USER", "tester")
        .env("TASKS_BIN", assert_cmd::cargo::cargo_bin("tasks"));
    command
}
```

Keep the existing doc comment above `harness_shim`. Run `just test-fast acceptance`; expected: pass (the refactor changes no behaviour).

- [ ] **Step 2: Write the acceptance tests**

Append to `tests/cli.rs`:

```rust
#[test]
fn an_acceptance_background_session_claims_as_its_versioned_process() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");

    let script = format!(
        "{}\nwrite_registry\necho \"SHIM=$$\"\n\"$TASKS_BIN\" start {id}\n",
        shim_env(&state, "claude-code", "bg1")
    );
    let out = common::harness_shim_at(&bin, &dir, env.home.path(), &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let shim = stdout
        .lines()
        .find_map(|line| line.strip_prefix("SHIM="))
        .unwrap();
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("session = \"claude-code:bg1\""), "{store}");
    assert!(store.contains(&format!("pid = {shim}\n")), "{store}");
}

#[test]
fn an_acceptance_nested_version_session_cannot_continue_the_outer_claim() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");

    // The outer claude shim claims the task under its own row, then runs a nested 2.1.280
    // twice: once with no hint, once with the outer session's id inherited. Each nested
    // `start`, `park` and `done` must fail. `exit $?` in the inner scripts keeps `tasks` a
    // child of the nested process, for the reason `harness_shim` documents.
    let nested = |prefix: &str| {
        format!(
            "{prefix}\"{bin}\" -c '\"$TASKS_BIN\" start {id} && echo NESTED_STARTED; \
             \"$TASKS_BIN\" park {id} next && echo NESTED_PARKED; \
             \"$TASKS_BIN\" done {id} landed && echo NESTED_CLOSED; exit 0'\n",
            bin = bin.display()
        )
    };
    let script = format!(
        "{}\nwrite_registry\n\"$TASKS_BIN\" start {id} || exit 70\n{}{}",
        shim_env(&state, "claude-code", "c1"),
        nested(""),
        nested("CLAUDE_CODE_SESSION_ID=c1 "),
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for marker in ["NESTED_STARTED", "NESTED_PARKED", "NESTED_CLOSED"] {
        assert!(!stdout.contains(marker), "{marker}: {stdout}");
    }
    assert_eq!(env.json(&dir, &["show", &id])["task"]["status"], "doing");
    let store = std::fs::read_to_string(env.claim_store("sci")).unwrap();
    assert!(store.contains("session = \"claude-code:c1\""), "{store}");
}

#[test]
fn an_acceptance_version_session_under_claude_never_claims_as_the_outer_session() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    let state = relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");

    // The review chain: tasks -> 2.1.280 -> claude, with a correct row for the outer claude
    // and no hint. The nearest harness is the version process, which has no row.
    let script = format!(
        "{}\nwrite_registry\n\"{}\" -c '\"$TASKS_BIN\" start {id}; exit $?'\n",
        shim_env(&state, "claude-code", "outer"),
        bin.display()
    );
    let out = common::harness_shim(&dir, env.home.path(), "claude", &script);
    assert_ne!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stderr);
    assert!(text.contains("2.1.280"), "{text}");
    assert!(text.contains("TASKS_SESSION"), "{text}");
    let store = env.claim_store("sci");
    assert!(
        !store.exists() || !std::fs::read_to_string(&store).unwrap().contains("claude-code:outer"),
        "the nested session must never claim as the outer one"
    );
}

#[test]
fn an_acceptance_hosting_process_refuses_before_reading_the_registry() {
    let mut env = TestEnv::new();
    let dir = env.init("sci");
    let id = id_of(env.json(&dir, &["add", "Thing", "-p", "2"]));
    // Relay on, and deliberately no registry at all: a refusal that names the role, not a
    // missing file, shows the registry was never opened.
    relay_on(&env);
    let bin = common::claude_version_binary(env.home.path(), "2.1.280");
    let claude = env.home.path().join("claude");
    std::os::unix::fs::symlink("/bin/sh", &claude).unwrap();

    for (program, role) in [(&bin, "bg-pty-host"), (&claude, "daemon")] {
        let script = format!("\"$TASKS_BIN\" start {id}\n");
        let out = common::shim_with_argument(program, role, &dir, env.home.path(), &script);
        assert_ne!(out.status.code(), Some(0), "{role}");
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(text.contains(role) && text.contains("not a session"), "{role}: {text}");
        assert!(!text.contains("does not exist"), "{role}: {text}");
    }
}
```

`shim_with_argument` runs with `HOME` set, so `relay_on`'s config is found. The daemon case uses a `claude` symlink, whose `comm` is `claude`; `argv[1]` is `daemon` because the script file is named that.

- [ ] **Step 3: Run the tests**

Run: `just test-fast an_acceptance`
Expected: all pass. If `an_acceptance_background_session_claims_as_its_versioned_process` refuses with `unknown … exe`, check that `/proc/<pid>/exe` of the copy is readable and ends in `/claude/versions/2.1.280`; it must, as the copy is a regular file owned by the test user.

To confirm the tests exercise the new walk, temporarily make `harness_of` return `Ok(None)` for version comms (`git stash` afterwards): `an_acceptance_background_session_claims_as_its_versioned_process` and `an_acceptance_nested_version_session_cannot_continue_the_outer_claim` must fail. Restore the code.

- [ ] **Step 4: Gate and commit**

Run: `just gate`

```bash
git add tests/common/mod.rs tests/cli.rs
git commit -m "test(relay): background, nested and hosting Claude Code sessions end to end"
```

---

### Task 4: README, version 0.2.0 and spec notes

**Files:**
- Modify: `README.md:162-203` ("Relay identity (opt-in)")
- Modify: `Cargo.toml:3`, `Cargo.lock` (the `tasks` package entry)
- Modify: `docs/specs/2026-09-22-relay-ancestry-identity-design.md` (§4.2, §4.3 amendment notes)
- Modify: `docs/specs/2026-09-23-relay-session-process-design.md` (status line)

**Interfaces:**
- Consumes: Tasks 1-3 landed.
- Produces: `tasks --version` prints `tasks 0.2.0`.

- [ ] **Step 1: Bump the version**

In `Cargo.toml`, `version = "0.1.0"` becomes `version = "0.2.0"`. Run `cargo build` once so `Cargo.lock`'s `tasks` entry follows, then `git diff Cargo.lock` shows only that version line.

- [ ] **Step 2: Rewrite the README section**

In "Relay identity (opt-in)", replace these two sentences:

> Relay records a process handle only for a harness with a controlling terminal, so a headless session (`claude -p`, `codex exec`) is published without one and must name itself with `TASKS_SESSION`.

with:

> Relay records the handle of the process that is the session, so headless, nested and background sessions (`claude -p`, `claude --bg`, `codex exec`) each claim as themselves.

Replace the sentence beginning "With it on, `tasks` walks its own process ancestry to the nearest harness process" with:

> With it on, `tasks` walks its own process ancestry to the nearest harness process — `claude`, `codex` or `opencode`, or a versioned Claude Code binary under `claude/versions/`, which is how a background session runs; Claude Code's own daemon and pty host are not sessions and are refused — and looks that process up in relay's agent registry, matching on host, boot id, pid and process start time, and requiring the registry's harness to agree with the process it found.

Insert before the paragraph beginning "Enabling relay never rewrites a claim already held":

> **Upgrading.** tasks 0.2.0 reads relay's registry schema 2 and no other; against an older registry it refuses and asks you to run `relay reap`. On a host with this switch on, every `tasks` that can run there must be 0.2.0 or later before relay publishes schema 2: check `tasks --version` for each `tasks` on `PATH` in each harness's environment. An older `tasks` refuses fresh claims against the new registry, but it can still let a session nested inside another continue the outer session's claims, which no registry format can prevent.

- [ ] **Step 3: Amend the parent spec and mark this one**

In `docs/specs/2026-09-22-relay-ancestry-identity-design.md`, append to the end of §4.2 (after "…to find an outer harness that does match."):

```markdown
*Amended by `2026-09-23-relay-session-process-design.md`:* a Claude Code process is also
a version-named binary under `claude/versions/`, and a nearest Claude Code process that
is its daemon or pty host ends the walk as a refusal.
```

and to the end of §4.3's first paragraph (after "…treats a snapshot that fails validation as unavailable (§5)…" paragraph):

```markdown
*Amended by `2026-09-23-relay-session-process-design.md`:* the reader validates schema 2
and refuses schema 1 as superseded.
```

In `docs/specs/2026-09-23-relay-session-process-design.md`, change `Status: draft, for review.` to `Status: approved 2026-09-23; implemented on feat/relay-schema2, live check pending (Task 5).`

- [ ] **Step 4: Gate, commit, reinstall**

Run: `just gate`

```bash
git add README.md Cargo.toml Cargo.lock docs/specs/2026-09-22-relay-ancestry-identity-design.md docs/specs/2026-09-23-relay-session-process-design.md
git commit -m "docs(readme): relay identity under registry schema 2; release 0.2.0"
```

Do not `cargo install` from the worktree here; installation happens from the main checkout after merge (Task 5, Step 6). Run `./target/debug/tasks --version` and expect `tasks 0.2.0`.

---

### Task 5: Live check

**Files:**
- Modify: `docs/specs/2026-09-23-relay-session-process-design.md` (status line, only on success)

**Interfaces:**
- Consumes: the built worktree binary at `.worktrees/relay-schema2/target/debug/tasks`, and relay at `45f4c47` or later (its checkout: `tasks root relay-cb616b`).

The spec requires: with the opt-in on against a private relay registry, an outer interactive Claude Code session and a nested `claude -p` each acquire their own claim. Nothing on the host is repointed: relay's hooks are attached to scratch sessions with `claude --settings <scratch file>`, and tasks runs by explicit path with a scratch `XDG_CONFIG_HOME`.

- [ ] **Step 1: Prepare the scratch area**

```bash
S=$(mktemp -d -p "${TMPDIR:-/tmp}" tasks-live-2dd094.XXXX)
BIN=$PWD/target/debug/tasks          # run from the worktree root
# Scratch config *and* state: the project registry and the claim store must not touch the
# host's. Every tasks invocation in this task goes through $T.
T="env XDG_CONFIG_HOME=$S/config XDG_STATE_HOME=$S/state $BIN"
RELAY=$(tasks root relay-cb616b | jq -r .root)
mkdir -p "$S/config/tasks" "$S/proj" && chmod 700 "$S"
printf '[identity]\nrelay = true\n' > "$S/config/tasks/config.toml"
(cd "$S/proj" && git init -q && $T init --prefix live >/dev/null)
A=$(cd "$S/proj" && $T add "Outer" -p 2 | jq -r .task.id)
B=$(cd "$S/proj" && $T add "Nested" -p 2 | jq -r .task.id)
echo "$S $A $B"
```

(If `add`'s JSON shape differs, read the id from its output by hand.)

- [ ] **Step 2: Write the scratch hook settings**

Read relay's `docs/runbooks/hook-install-rollback.md` and `node "$RELAY/bin/relay.js" --help` for the current hook command form (`relay hook --harness claude-code <Event>` with `--outer-ms` and `--subscribers`). Prefer generating the entries with relay's installer in dry-run mode against a scratch settings target, if it offers one. Otherwise write `$S/settings.json` by hand with one entry each for `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `Stop` and `SessionEnd`:

```json
{ "hooks": { "SessionStart": [ { "hooks": [ { "type": "command",
  "command": "RELAY_STATE_DIR=<S>/relay node <RELAY>/bin/relay.js hook --harness claude-code SessionStart",
  "timeout": 600 } ] } ] } }
```

with `<S>` and `<RELAY>` substituted and the other events alike, plus whatever flags the runbook requires. An empty subscriber file (`{}` or the runbook's empty form, mode 0600 in a 0700 directory under `$S`) is enough.

- [ ] **Step 3: Run the outer and nested sessions**

Run the outer session interactively in a detached tmux session that this step owns and kills:

```bash
trap 'tmux kill-session -t live-2dd094 2>/dev/null' EXIT INT TERM
tmux new-session -d -s live-2dd094 -c "$S/proj" \
  "claude --settings '$S/settings.json' --allowedTools Bash"
sleep 5
tmux send-keys -t live-2dd094 "Run exactly these two commands with Bash and show their full output: (1) $T start $A  (2) claude -p --settings $S/settings.json --allowedTools Bash 'Run $T start $B with Bash and print its full output'" Enter
```

Wait for the outer session to finish (poll `tmux capture-pane -p -t live-2dd094` until both outputs appear; allow a few minutes), then capture the pane to `$S/outer.txt`.

- [ ] **Step 4: Verify**

```bash
RELAY_STATE_DIR="$S/relay" node "$RELAY/bin/relay.js" list
cat "$S/state/tasks/claims/live.toml"
(cd "$S/proj" && $T show "$A" | jq '.task.status')
(cd "$S/proj" && $T show "$B" | jq '.task.status')
```

Pass when:
- `relay list` shows two `claude-code` agents with different session ids and different `process.pid`s: the outer interactive `claude` and the nested `claude` process;
- both tasks are `doing`;
- the claim store holds two claims whose `session`s are those two agent ids and whose `pid`s are those two pids.

Record the result with `tasks note tasks-2dd094 "live: outer claude-code:<id> pid <n>, nested claude-code:<id> pid <m>; both claimed"`.

If relay's hooks cannot be attached to a scratch session without changing host configuration, or the session cannot be driven, stop here: `tasks park tasks-2dd094 "live check: <what failed>; rerun Task 5 from Step 1" --reason environment`, leaving the plan's other tasks landed.

- [ ] **Step 5: Clean up**

```bash
tmux kill-session -t live-2dd094 2>/dev/null; trap - EXIT INT TERM
rm -rf "$S"
host-load --section session
```

Expected: nothing left running from this step.

- [ ] **Step 6: Mark verified and close**

Change the spec's status line to `Status: approved 2026-09-23; implemented and verified live on Linux, 2026-09-23.` and commit it with `tasks done tasks-2dd094 "<what landed>"` in the same commit, after the branch is merged per the finishing skill. Then from the main checkout: `cargo install --path .` and `tasks --version` prints `tasks 0.2.0`. Relay's `relay-c19c0a` is unblocked by this task closing.
