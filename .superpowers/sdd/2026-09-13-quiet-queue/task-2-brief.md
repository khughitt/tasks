### Task 2: A recorded-first resolver and `tasks quiet`

**Files:**
- Modify: `src/commands/parked.rs`
- Create: `src/commands/quiet.rs`
- Modify: `src/cli.rs` (a new `Quiet` variant after `Tags`), `src/commands/mod.rs` (`pub mod quiet;` and dispatch), `src/output.rs` (`Output::Quiet`, `QuietOut`, `quiet_briefs`, `warnings_of`)
- Test: `tests/cli.rs` (new tests after `a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next`, which ends around line 5935; one line in `project_and_all_projects_conflict_on_every_read_command` at line 1945)

**Interfaces:**
- Consumes: `claims::Reason::Quiet`, `claims::Needs`, `ParkInfo.needs`/`.minutes` from Task 1; `Scope::projects(&self) -> &[Project]` (`src/scope.rs:110`); `Project.root: PathBuf` (`src/repo.rs:70`); `open_read_ctx(dir, &ScopeArgs)` from `src/commands/mod.rs`; the test helpers `as_agent`, `id_of`, `error_of` already in `tests/cli.rs` (`two_roots` is *not* used: `init_forced` re-points the registry at the second root, and these tests need the worktree unregistered).
- Produces: `parked::Prefer { Registered, Recorded }`; `parked::rows_preferring(ctx: &mut ReadCtx, all: &[Task], claims: &ClaimSnapshot, now: OffsetDateTime, prefer: Prefer, reason: Option<Reason>) -> Result<Vec<ParkedRow>>`; `parked::rows(...)` unchanged in signature, equal to `rows_preferring(..., Prefer::Registered, None)`; `Command::Quiet { limit: Option<usize>, project: Option<String>, all_projects: bool }`; `quiet::run(ctx: ReadCtx, limit: Option<usize>) -> Result<Output>`; `Output::Quiet(QuietOut { tasks: Vec<ParkedRow>, warnings: Vec<String> })`; `output::quiet_briefs(rows: &[ParkedRow], painter: &Painter) -> String`.

The resolver refactor and the command land in one commit because `Prefer::Recorded` has no constructor until `quiet::run` exists, and an unconstructed variant fails the gate. The refactor is still verified on its own: step 3 runs the whole suite after it, before the command exists.

- [ ] **Step 1: Write the failing integration tests**

Add to `tests/cli.rs` after `a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next`:

```rust
/// A second checkout of an already-registered prefix that the registry does not point
/// at: what a git worktree is to the tracker. (`init_forced` would re-point the
/// registry, which is the opposite of what these tests need.)
fn unregistered_checkout(prefix: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(path.join("tasks")).unwrap();
    std::fs::write(
        path.join("tasks/.config.toml"),
        format!("prefix = \"{prefix}\"\n"),
    )
    .unwrap();
    (dir, path)
}

#[test]
fn a_park_whose_task_file_was_deleted_in_its_own_checkout_still_warns_unavailable() {
    // Guards the resolver's shortcut: when the recorded worktree is the scanned root
    // but the task file is gone, the parked views must still say so, as they did before
    // the preference existed.
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "-p", "2"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &id, "finish", "--waiting-on", "user", "--reason", "quiet", "--minutes", "5"])
        .assert()
        .success();
    std::fs::remove_file(sci.join(format!("tasks/{id}.md"))).unwrap();

    for args in [vec!["list", "--parked"], vec!["prime"], vec!["quiet"]] {
        let v = env.json(&sci, &args);
        let rows = if args[0] == "prime" { &v["parked"] } else { &v["tasks"] };
        let row = &rows.as_array().unwrap()[0];
        assert_eq!(row["id"], id, "{args:?}: {v}");
        assert!(row["status"].is_null(), "store-only: {v}");
        assert_eq!(row["title"], "T");
        assert!(
            v["warnings"]
                .to_string()
                .contains("which is unavailable; the row shows the park entry only"),
            "{args:?}: {v}"
        );
    }
}

#[test]
fn quiet_lists_quiet_parks_across_projects_by_priority_then_park_time() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    let nowhere = tempfile::tempdir().unwrap();

    let empty = env.json(nowhere.path(), &["quiet"]);
    assert_eq!(empty, serde_json::json!({"tasks": [], "warnings": []}));
    assert_eq!(env.pretty(nowhere.path(), &["quiet"]).trim(), "");

    let later = id_of(env.json(&sci, &["add", "Later capture", "-p", "2"]));
    let review = id_of(env.json(&sci, &["add", "Sheet", "-p", "0"]));
    let first = id_of(env.json(&fam, &["add", "First capture", "-p", "2"]));
    let urgent = id_of(env.json(&fam, &["add", "Urgent sweep", "-p", "1"]));

    as_agent(&env, &sci, "agent-a")
        .args(["park", &review, "look", "--waiting-on", "user", "--reason", "review"])
        .assert()
        .success();
    as_agent(&env, &fam, "agent-a")
        .args(["park", &first, "run it", "--waiting-on", "user", "--reason", "quiet", "--minutes", "30"])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &later, "run it too", "--waiting-on", "user", "--reason", "quiet", "--minutes", "45", "--needs", "headless"])
        .assert()
        .success();
    as_agent(&env, &fam, "agent-a")
        .args(["park", &urgent, "sweep", "--reason", "quiet", "--minutes", "5"])
        .assert()
        .success();

    let v = env.json(nowhere.path(), &["quiet"]);
    let ids: Vec<&str> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [urgent.as_str(), first.as_str(), later.as_str()], "{v}");
    assert_eq!(v["warnings"], serde_json::json!([]));
    assert_eq!(v["tasks"][2]["park"]["needs"], "headless");
    assert_eq!(v["tasks"][2]["park"]["minutes"], 45);
    assert_eq!(v["tasks"][2]["park"]["worktree"], sci.to_string_lossy().as_ref());

    let one = env.json(nowhere.path(), &["quiet", "-n", "1"]);
    assert_eq!(one["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(one["tasks"][0]["id"], urgent);

    let only_sci = env.json(nowhere.path(), &["quiet", "--project", "sci"]);
    assert_eq!(only_sci["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(only_sci["tasks"][0]["id"], later);

    let explicit = env.json(&sci, &["quiet", "--all-projects"]);
    assert_eq!(explicit["tasks"].as_array().unwrap().len(), 3, "redundant, accepted");

    let text = env.pretty(nowhere.path(), &["quiet"]);
    let brief = text.lines().collect::<Vec<_>>();
    assert!(brief[0].starts_with(&urgent), "{text}");
    assert!(brief[0].contains("P1") && brief[0].contains("idle") && brief[0].contains("5 min"), "{text}");
    assert!(brief[0].contains("Urgent sweep"), "{text}");
    assert!(brief[1].trim_start().starts_with("next: sweep"), "{text}");
    assert!(
        brief[2].trim_start().starts_with("in:") && brief[2].contains(&*fam.to_string_lossy()),
        "{text}"
    );
    assert_eq!(brief[3], "", "one blank line between briefs: {text}");
    assert!(text.contains("headless") && text.contains("45 min"), "{text}");
}

#[test]
fn quiet_resolves_from_the_recorded_checkout_before_the_registered_copy() {
    let mut env = TestEnv::new();
    let main = env.init("sci");
    let (_keep, wt) = unregistered_checkout("sci");
    let id = id_of(env.json(&main, &["add", "Capture", "-p", "3"]));
    std::fs::copy(
        main.join(format!("tasks/{id}.md")),
        wt.join(format!("tasks/{id}.md")),
    )
    .unwrap();
    env.json(&main, &["done", &id, "closed on main"]);
    env.json(&wt, &["edit", &id, "-p", "1"]);
    as_agent(&env, &wt, "agent-a")
        .args(["park", &id, "capture", "--waiting-on", "user", "--reason", "quiet", "--minutes", "20"])
        .assert()
        .success();

    // the parked listing from the registered root still trusts its own (done) copy
    let parked = env.json(&main, &["list", "--parked"]);
    assert!(parked["tasks"].as_array().unwrap().is_empty(), "{parked}");

    let v = env.json(&main, &["quiet"]);
    assert_eq!(v["tasks"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["tasks"][0]["id"], id);
    assert_eq!(v["tasks"][0]["status"], "todo");
    assert_eq!(v["tasks"][0]["priority"], 1, "the worktree's priority, not main's");
    assert_eq!(v["tasks"][0]["park"]["worktree"], wt.to_string_lossy().as_ref());
    assert!(
        v["warnings"].to_string().contains("resume it from that checkout"),
        "{v}"
    );

    // the worktree gone: fall back to the registered copy, which is closed, so excluded
    std::fs::remove_dir_all(wt.join("tasks")).unwrap();
    let v = env.json(&main, &["quiet"]);
    assert!(v["tasks"].as_array().unwrap().is_empty(), "{v}");
    assert!(
        v["warnings"].to_string().contains("showing the registered copy"),
        "{v}"
    );
}

#[test]
fn quiet_keeps_scan_warnings_when_the_queue_is_empty() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fam = env.init("fam");
    std::fs::remove_file(fam.join("tasks/.config.toml")).unwrap();
    let v = env.json(&sci, &["quiet"]);
    assert_eq!(v["tasks"], serde_json::json!([]));
    assert_eq!(v["warnings"].as_array().unwrap().len(), 1, "{v}");
    assert!(v["warnings"][0].as_str().unwrap().contains("fam"), "{v}");
    let out = env
        .cmd(&sci)
        .args(["--pretty", "quiet"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
    assert!(String::from_utf8_lossy(&out.stderr).contains("unreachable"));
}

#[test]
fn quiet_includes_a_store_only_entry_last() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let real = id_of(env.json(&sci, &["add", "Real", "-p", "3"]));
    as_agent(&env, &sci, "agent-a")
        .args(["park", &real, "run", "--waiting-on", "user", "--reason", "quiet", "--minutes", "10"])
        .assert()
        .success();
    let path = env.claim_store("sci");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(
        "[parks.\"sci-0000ff\"]\nowner = \"someone\"\nsession = \"s\"\nhost = \"h\"\n\
         worktree = \"/gone\"\nat = \"2026-01-02T00:00:00Z\"\nnext_step = \"finish\"\n\
         waiting_on = \"user\"\nreason = \"quiet\"\nneeds = \"idle\"\nminutes = 15\ntitle = \"Ghost\"\n",
    );
    std::fs::write(&path, text).unwrap();
    let v = env.json(&sci, &["quiet", "--project", "sci"]);
    let rows = v["tasks"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{v}");
    assert_eq!(rows[0]["id"], real);
    assert_eq!(rows[1]["id"], "sci-0000ff", "no priority sorts last");
    assert!(rows[1]["priority"].is_null());
    assert_eq!(rows[1]["title"], "Ghost");
    assert_eq!(rows[1]["park"]["minutes"], 15);
    let text = env.pretty(&sci, &["quiet", "--project", "sci"]);
    assert!(text.contains("sci-0000ff  P-"), "{text}");
}
```

And in `project_and_all_projects_conflict_on_every_read_command`, change the loop list to `["list", "ready", "next", "prime", "tree", "tags", "quiet"]`.

- [ ] **Step 2: Run the new tests to verify they fail**

Run: `cargo test --test cli quiet`
Expected: the `quiet_*` tests and `a_park_whose_task_file_was_deleted_in_its_own_checkout_still_warns_unavailable` fail with exit 2, `quiet` is not a subcommand (the deleted-file test fails on its `quiet` iteration; its `list --parked` and `prime` iterations pass today, which is the point).

- [ ] **Step 3: Restructure the resolver around a preference**

Replace the body of `src/commands/parked.rs` from `pub fn rows` through the end of `fn resolve_elsewhere` (keep the `candidates` function below untouched) with:

```rust
/// Which copy of a parked task a view trusts when the registered checkout and the
/// park's recorded worktree both hold it. The parked listing and `prime` read the
/// checkout they were run in (`Registered`); the quiet queue reads where the work was
/// actually set down (`Recorded`), since a task reopened and parked in a worktree may
/// still read `done`, or carry a stale priority, in the registered copy
/// (quiet-queue design §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefer {
    Registered,
    Recorded,
}

pub fn rows(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
) -> Result<Vec<ParkedRow>> {
    rows_preferring(ctx, all, claims, now, Prefer::Registered, None)
}

pub fn rows_preferring(
    ctx: &mut ReadCtx,
    all: &[Task],
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    prefer: Prefer,
    reason: Option<Reason>,
) -> Result<Vec<ParkedRow>> {
    let mut entries: Vec<(&String, &Park)> = claims
        .parks()
        .filter(|(_, park)| reason.is_none_or(|reason| park.reason == Some(reason)))
        .collect();
    entries.sort_by(|a, b| b.1.at.cmp(&a.1.at).then_with(|| a.0.cmp(b.0)));
    let mut rows = Vec::new();
    for (key, park) in entries {
        let id = match TaskId::parse(key) {
            Ok(id) => ctx.registry.canonical_id(&id),
            Err(error) => {
                ctx.warnings.push(format!(
                    "park entry {key:?} is not a task id ({error}); shown as is"
                ));
                rows.push(ParkedRow::unresolved(key, park));
                continue;
            }
        };
        let registered = all
            .iter()
            .find(|task| task.id == id)
            .map(|task| resolved_row(task, all, claims, &ctx.registry, now));
        // When the recorded worktree is one of the scanned roots *and* the scan found
        // the task, the registered copy is the recorded copy: opening the root again
        // would only add a misleading "resume it from that checkout". A scanned root
        // that lacks the file still goes through `recorded_or_fallback`, so the
        // "unavailable" warning the parked views have always given survives.
        let recorded_is_scanned = ctx
            .scope
            .projects()
            .iter()
            .any(|project| project.root == Path::new(&park.worktree));
        let row = match (prefer, registered) {
            (_, Some(row)) if recorded_is_scanned => Some(row),
            (Prefer::Registered, Some(row)) => Some(row),
            (Prefer::Registered, None) => {
                recorded_or_fallback(ctx, &id, park, claims, now, None)
            }
            (Prefer::Recorded, registered) => {
                recorded_or_fallback(ctx, &id, park, claims, now, registered)
            }
        };
        rows.push(row.unwrap_or_else(|| ParkedRow::unresolved(key, park)));
    }
    Ok(rows)
}

fn resolved_row(
    task: &Task,
    all: &[Task],
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> ParkedRow {
    ParkedRow::resolved(
        TaskSummary::of(task, all, Some(claims), registry, now),
        Phase::of(task),
    )
}

/// The row from the park's recorded worktree, with the "resume it from that checkout"
/// warning; else `fallback` (the registered copy, when the caller preferred the recorded
/// one and has it) with the "unavailable" warning; else `None`, which the caller renders
/// store-only. Never errors: an unreadable worktree is a warning, not a failed listing.
fn recorded_or_fallback(
    ctx: &mut ReadCtx,
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    now: OffsetDateTime,
    fallback: Option<ParkedRow>,
) -> Option<ParkedRow> {
    match resolve_recorded(id, park, claims, &ctx.registry, now) {
        Ok(Some(row)) => {
            ctx.warnings.push(format!(
                "{id} is parked in {}; resume it from that checkout",
                park.worktree
            ));
            Some(row)
        }
        Ok(None) => {
            ctx.warnings
                .push(unavailable(id, park, None, fallback.is_some()));
            fallback
        }
        Err(error) => {
            ctx.warnings
                .push(unavailable(id, park, Some(&error), fallback.is_some()));
            fallback
        }
    }
}

/// The two pre-existing "unavailable" messages byte for byte, plus the fallback form
/// the quiet queue adds.
fn unavailable(
    id: &TaskId,
    park: &Park,
    error: Option<&crate::error::Error>,
    fell_back: bool,
) -> String {
    let cause = match error {
        Some(error) => format!(" ({error})"),
        None => String::new(),
    };
    let shown = if fell_back {
        "showing the registered copy"
    } else {
        "the row shows the park entry only"
    };
    format!(
        "{id} is parked in {}, which is unavailable{cause}; {shown}",
        park.worktree
    )
}

fn resolve_recorded(
    id: &TaskId,
    park: &Park,
    claims: &ClaimSnapshot,
    registry: &Registry,
    now: OffsetDateTime,
) -> Result<Option<ParkedRow>> {
    let root = Path::new(&park.worktree);
    if !crate::scope::has_config(root)? {
        return Ok(None);
    }
    let project = Project::open(root)?;
    if project.prefix != id.prefix {
        return Ok(None);
    }
    let scan = project.scan()?;
    let Some(task) = scan.iter().find(|task| task.id == *id) else {
        return Ok(None);
    };
    Ok(Some(resolved_row(task, &scan, claims, registry, now)))
}
```

Compare `unavailable`'s two `fell_back == false` outputs against the two `format!` strings in `git show HEAD:src/commands/parked.rs`; they must be identical, because `a_task_parked_only_in_another_checkout_is_listed_from_there_and_never_next` asserts on "which is unavailable" and the new deleted-file test asserts the full phrase.

Run the whole suite now, before the command exists, to prove the refactor changed nothing observable: `cargo test`. Expected: every pre-existing test passes; only the six new tests from step 1 fail. (`Prefer::Recorded` is unconstructed at this point, so `just check` would fail on `dead_code`; that is why this step does not commit.)

- [ ] **Step 4: Add the CLI variant and the dispatch**

In `src/cli.rs`, after the `Tags` variant:

```rust
    /// Work parked waiting for an idle host, as resume briefs: every reachable project
    /// by default, priority first, then oldest park first.
    Quiet {
        /// At most this many briefs; -n 1 is the top of the queue.
        #[arg(short = 'n', long)]
        limit: Option<usize>,
        /// One registered project instead of all of them; needs no local project.
        #[arg(
            long,
            conflicts_with = "all_projects",
            add = ArgValueCandidates::new(crate::complete::prefixes)
        )]
        project: Option<String>,
        /// The default; accepted so the flag reads like the other read commands.
        #[arg(long)]
        all_projects: bool,
    },
```

In `src/commands/mod.rs`: add `pub mod quiet;` to the module list (alphabetical, after `projects`), and after the `Command::Tags` dispatch:

```rust
        Command::Quiet {
            limit,
            project,
            all_projects: _,
        } => {
            // Quiet-queue design §5: the bedtime question is cross-project, so the scope
            // is every project unless one is named.
            let scope = ScopeArgs {
                all_projects: project.is_none(),
                project,
            };
            quiet::run(open_read_ctx(dir, &scope)?, limit)
        }
```

- [ ] **Step 5: Write `src/commands/quiet.rs`**

```rust
//! `tasks quiet`: the queue of work parked waiting for an idle host
//! (docs/specs/2026-09-13-quiet-queue-design.md §5).
//!
//! Every park with reason `quiet` in scope, resolved from its recorded checkout first,
//! closed tasks dropped after resolution, ordered priority first then oldest park first.
//! Warnings from the scan and from resolution are kept whether or not a row survives:
//! an empty queue with no warnings is the only "nothing to run tonight".

use super::ReadCtx;
use super::parked::{Prefer, rows_preferring};
use crate::claims::Reason;
use crate::error::Result;
use crate::output::{Output, ParkedRow, QuietOut};

pub fn run(mut ctx: ReadCtx, limit: Option<usize>) -> Result<Output> {
    let (all, claims) = ctx.scan_with_claims()?;
    let now = crate::time::parse(&crate::time::now())?;
    let rows = rows_preferring(
        &mut ctx,
        &all,
        &claims,
        now,
        Prefer::Recorded,
        Some(Reason::Quiet),
    )?;
    let mut tasks: Vec<ParkedRow> = rows
        .into_iter()
        .filter(|row| {
            row.park
                .as_ref()
                .is_some_and(|park| park.reason == Some(Reason::Quiet))
        })
        .filter(|row| row.status.is_none_or(|status| status.is_open()))
        .collect();
    tasks.sort_by(|a, b| {
        let priority = |row: &ParkedRow| row.priority.unwrap_or(u8::MAX);
        let at = |row: &ParkedRow| {
            row.park
                .as_ref()
                .map(|park| park.at.clone())
                .unwrap_or_default()
        };
        priority(a)
            .cmp(&priority(b))
            .then_with(|| at(a).cmp(&at(b)))
            .then_with(|| a.id.cmp(&b.id))
    });
    if let Some(limit) = limit {
        tasks.truncate(limit);
    }
    Ok(Output::Quiet(QuietOut {
        tasks,
        warnings: ctx.warnings,
    }))
}
```

`quiet` passes `Some(Reason::Quiet)` so unrelated review, decision, and other parks are filtered before recorded-checkout resolution; their warnings never leak into the quiet output. `list --parked` and `prime` pass `None` and retain warnings for every parked entry.

- [ ] **Step 6: Add the output type and the pretty briefs**

In `src/output.rs`, after `ParkedOut`:

```rust
#[derive(Serialize)]
pub struct QuietOut {
    pub tasks: Vec<ParkedRow>,
    pub warnings: Vec<String>,
}
```

Add `Quiet(QuietOut),` to `enum Output` after `Parked(ParkedOut),`. In `pretty()` add `Output::Quiet(o) => quiet_briefs(&o.tasks, painter),` after the `Parked` arm. In `warnings_of` add `Output::Quiet(o) => o.warnings.clone(),`. After `parked_table` add:

```rust
/// One brief per quiet park (quiet-queue design §5): the row, the next step, and the
/// checkout to open an agent in. No launch command: that belongs to the harness.
pub fn quiet_briefs(rows: &[ParkedRow], painter: &Painter) -> String {
    let mut rendered = String::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(park) = &row.park else {
            continue;
        };
        if index > 0 {
            rendered.push('\n');
        }
        let id = painter.paint(Style::Chrome, &row.id);
        let priority = match row.priority {
            Some(priority) => format!("P{priority}"),
            None => "P-".into(),
        };
        let needs = park.needs.map(crate::claims::Needs::as_str).unwrap_or("-");
        let minutes = match park.minutes {
            Some(minutes) => format!("{minutes} min"),
            None => "-".into(),
        };
        rendered.push_str(&format!(
            "{id}  {priority}  {needs:<8}  {minutes:>8}  parked {}  {}\n",
            crate::time::day(&park.at),
            row.title
        ));
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("        next: {}", park.next_step),
        ));
        rendered.push('\n');
        rendered.push_str(&painter.paint(
            Style::Chrome,
            &format!("        in:   {}", park.worktree),
        ));
        rendered.push('\n');
    }
    rendered
}
```

- [ ] **Step 7: Run the tests and the gate**

Run: `cargo test --test cli quiet`
Expected: ok; the six new tests and the conflict test pass. If `quiet_lists_quiet_parks_across_projects_by_priority_then_park_time` fails on the `worktree` assertion, `park.worktree` is `ctx.project.root.display()` and `TestEnv::init` canonicalizes the temp path, so the two should match; if not, compare after `canonicalize()` on the test side.

Run: `cargo test`
Expected: ok, whole suite.

Run: `just check`
Expected: passes; `Prefer::Recorded` now has its constructor in `quiet::run`.

- [ ] **Step 8: Commit**

```bash
tasks done tasks-0f780e "parked resolver takes a registered-or-recorded preference; tasks quiet lists quiet parks across projects as resume briefs, recorded checkout first, warnings kept"
git add src/commands/parked.rs src/commands/quiet.rs src/cli.rs src/commands/mod.rs src/output.rs tests/cli.rs tasks/
git commit -m "feat(cli): tasks quiet lists work parked for an idle host as resume briefs"
```

---
