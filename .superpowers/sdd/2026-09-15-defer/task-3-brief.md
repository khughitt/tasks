### Task 3: The flags — `--defer`, `--no-defer`, and the write-time refusals

Step record: **tasks-795fed**.

**Files:**
- Modify: `src/defer.rs` (append `Defer::resolve` and `can_carry`, with their tests)
- Modify: `src/cli.rs:66-68` (`FieldArgs`, after `every`), `:96-111` (`EditArgs`: `status` conflict, `no_defer`)
- Modify: `src/complete.rs:77` (add `defer_dates`)
- Modify: `src/commands/mod.rs:484` (`apply_fields`, after `every`)
- Modify: `src/commands/edit.rs:51-80` (`has_flags`), `:98` (`no_defer` clearing)
- Modify: `src/hierarchy.rs:32-38` (`validate_parent`), `:143-170` (add `validate_defer` beside `validate_periodic`)
- Modify: `src/repo.rs:344-348` (`write_task` calls `validate_defer`)
- Test: `tests/cli.rs`

**Interfaces:**
- Consumes: `crate::defer::Defer` (Task 1), `crate::periodic::Interval::parse`.
- Produces: `Defer::resolve(&str, today: Date) -> Result<Defer>`, `defer::can_carry(Status) -> bool`, `FieldArgs.defer: Option<String>`, `EditArgs.no_defer: bool`, `hierarchy::validate_defer(project, registry, task) -> Result<()>`, `complete::defer_dates()`.

- [x] **Step 0: `tasks start tasks-795fed`**

- [x] **Step 1: Write the failing end-to-end tests**

Append to `tests/cli.rs` (before the `two_roots` helper is fine; helpers are file-global):

```rust
#[test]
fn defer_stores_an_absolute_date_from_either_form_and_clears() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let fixed = id_of(env.json(&sci, &["add", "Fixed", "--defer", "2099-01-02"]));
    let v = env.json(&sci, &["show", &fixed]);
    assert_eq!(v["task"]["defer"], "2099-01-02");
    let text = std::fs::read_to_string(sci.join(format!("tasks/{fixed}.md"))).unwrap();
    assert!(text.contains("\ndefer: 2099-01-02\n"), "{text}");

    let relative = id_of(env.json(&sci, &["add", "Relative", "--status", "idea", "--defer", "60d"]));
    let stored = env.json(&sci, &["show", &relative])["task"]["defer"].clone();
    let today = time::OffsetDateTime::now_utc().date();
    let expected = today + time::Duration::days(60);
    assert_eq!(
        stored,
        format!("{:04}-{:02}-{:02}", expected.year(), expected.month() as u8, expected.day())
    );

    env.json(&sci, &["edit", &fixed, "--defer", "8w"]);
    assert_ne!(env.json(&sci, &["show", &fixed])["task"]["defer"], "2099-01-02");
    env.json(&sci, &["edit", &fixed, "--no-defer"]);
    assert!(env.json(&sci, &["show", &fixed])["task"]["defer"].is_null());
    env.json(&sci, &["edit", &fixed, "--defer", "2099-01-02", "-p", "1"]);
    let v = env.json(&sci, &["show", &fixed]);
    assert_eq!(v["task"]["defer"], "2099-01-02");
    assert_eq!(v["task"]["priority"], 1);
}

#[test]
fn defer_refuses_bad_values_and_incompatible_records() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let today = time::OffsetDateTime::now_utc().date();
    let today = format!("{:04}-{:02}-{:02}", today.year(), today.month() as u8, today.day());
    for bad in [today.as_str(), "2020-01-01", "0d", "soon", "2099-13-01"] {
        let error = error_of(&env, &sci, &["add", "Bad", "--defer", bad]);
        assert_eq!(error["error"]["kind"], "validation", "{bad}: {error}");
    }
    let error = error_of(&env, &sci, &["add", "Bad", "--defer", &today]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("must be after today"));

    let sweep = id_of(env.json(&sci, &["add", "Sweep", "--every", "30d"]));
    let error = error_of(&env, &sci, &["edit", &sweep, "--defer", "30d"]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("defer and every cannot both be set"));
    let later = id_of(env.json(&sci, &["add", "Later", "--defer", "30d"]));
    let error = error_of(&env, &sci, &["edit", &later, "--every", "30d"]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("defer and every cannot both be set"));
    let error = error_of(&env, &sci, &["add", "Both", "--every", "30d", "--defer", "30d"]);
    assert_eq!(error["error"]["kind"], "validation");

    let goal = id_of(env.json(&sci, &["add", "Goal"]));
    env.json(&sci, &["add", "Kid", "--parent", &goal]);
    let error = error_of(&env, &sci, &["edit", &goal, "--defer", "30d"]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("cannot be deferred"), "{error}");
    let error = error_of(&env, &sci, &["add", "Orphan", "--parent", &later]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("is deferred and cannot have children"), "{error}");
    let loose = id_of(env.json(&sci, &["add", "Loose"]));
    let error = error_of(&env, &sci, &["edit", &loose, "--parent", &later]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("is deferred and cannot have children"), "{error}");

    let busy = id_of(env.json(&sci, &["add", "Busy"]));
    env.json(&sci, &["start", &busy]);
    let error = error_of(&env, &sci, &["edit", &busy, "--defer", "30d"]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("is doing"), "{error}");
    let closed = id_of(env.json(&sci, &["add", "Closed"]));
    env.json(&sci, &["done", &closed, "x"]);
    let error = error_of(&env, &sci, &["edit", &closed, "--defer", "30d"]);
    assert!(error["error"]["detail"].as_str().unwrap().contains("is done"), "{error}");
}

#[test]
fn defer_flag_conflicts_are_usage_errors() {
    let mut env = TestEnv::new();
    let sci = env.init("sci");
    let id = id_of(env.json(&sci, &["add", "T", "--status", "idea"]));
    for args in [
        vec!["edit", id.as_str(), "--defer", "30d", "--no-defer"],
        vec!["edit", id.as_str(), "--status", "todo", "--defer", "30d"],
    ] {
        env.cmd(&sci).args(&args).assert().code(2);
    }
    assert!(env.json(&sci, &["show", &id])["task"]["defer"].is_null());
}
```

- [x] **Step 2: Run them to see them fail**

Run: `just test-fast defer_`
Expected: three FAIL (`unexpected argument '--defer'`).

- [x] **Step 3: The grammar and the status rule, in `src/defer.rs`**

Extend the imports to `use crate::model::Status;` and `use time::{Date, Month};` (already there). Inside `impl Defer`, after `parse`:

```rust
    /// The `--defer` value (spec §3.1): an absolute date, or the periodic interval grammar
    /// counted from `today`. Either way the result must be after `today`; a deferral to
    /// today or the past is a typo, and it would do nothing but read as due.
    pub fn resolve(value: &str, today: Date) -> Result<Defer> {
        let date = if value.ends_with('d') || value.ends_with('w') {
            let interval = crate::periodic::Interval::parse(value)?;
            today
                .checked_add(time::Duration::days(interval.days()))
                .ok_or_else(|| bad(value, "not a representable date"))?
        } else {
            Defer::parse(value)?.0
        };
        if date <= today {
            return Err(bad(
                value,
                &format!("must be after today ({})", Defer(today)),
            ));
        }
        Ok(Defer(date))
    }
```

After the `Serialize` impl:

```rust
/// The statuses a deferral may sit on (spec §3.2): the ones the pickers read. Every path
/// into any other status clears the field, so this is the writers' rule, not parsing's.
pub fn can_carry(status: Status) -> bool {
    matches!(status, Status::Idea | Status::Todo | Status::Blocked)
}
```

In the tests module, add a helper and three tests:

```rust
    fn day(s: &str) -> Date {
        Defer::parse(s).unwrap().0
    }

    #[test]
    fn resolve_accepts_a_future_date_or_an_interval_from_today() {
        let today = day("2026-09-15");
        assert_eq!(Defer::resolve("2026-11-10", today).unwrap().to_string(), "2026-11-10");
        assert_eq!(Defer::resolve("60d", today).unwrap().to_string(), "2026-11-14");
        assert_eq!(Defer::resolve("8w", today).unwrap().to_string(), "2026-11-10");
        assert_eq!(Defer::resolve("1d", today).unwrap().to_string(), "2026-09-16");
    }

    #[test]
    fn resolve_refuses_today_the_past_and_bad_intervals() {
        let today = day("2026-09-15");
        for bad in ["2026-09-15", "2026-09-14", "2020-01-01", "0d", "0w", "30m", "d"] {
            assert!(Defer::resolve(bad, today).is_err(), "{bad} should be rejected");
        }
        let error = Defer::resolve("2026-09-15", today).unwrap_err().to_string();
        assert!(error.contains("must be after today (2026-09-15)"), "{error}");
    }

    #[test]
    fn only_picker_statuses_carry_a_deferral() {
        assert!(can_carry(Status::Idea));
        assert!(can_carry(Status::Todo));
        assert!(can_carry(Status::Blocked));
        for status in [Status::Doing, Status::Shelved, Status::Done, Status::Dropped] {
            assert!(!can_carry(status), "{status:?}");
        }
    }
```

- [x] **Step 4: Implement the flags**

`src/cli.rs`, `FieldArgs`, after the `every` field:

```rust
    /// Hide this task from ready, next, quiet, and sample until a date: `YYYY-MM-DD`, or
    /// `<n>d`/`<n>w` from today. Cleared by any status change; see `--no-defer`.
    #[arg(long, add = ArgValueCandidates::new(crate::complete::defer_dates))]
    pub defer: Option<String>,
```

`EditArgs`: on `status`, add the conflict — a new deferral and a status change cannot travel in one write (§2.2):

```rust
    #[arg(long, conflicts_with = "defer", add = ArgValueCandidates::new(crate::complete::statuses))]
    pub status: Option<String>,
```

and after `no_every`:

```rust
    /// Clear the deferral.
    #[arg(long, conflicts_with = "defer")]
    pub no_defer: bool,
```

`src/complete.rs`, after `intervals`:

```rust
/// Convenience candidates for `--defer`; any §3.1 value is valid.
pub fn defer_dates() -> Vec<CompletionCandidate> {
    plain(["30d", "60d", "90d"])
}
```

`src/commands/mod.rs`, `apply_fields`, after the `every` block:

```rust
    if let Some(value) = &fields.defer {
        // spec §3.2: the writers enforce the status rule, since a transition must be able
        // to clear a deferral it finds on the way through.
        if !crate::defer::can_carry(task.status) {
            return Err(Error::Validation(format!(
                "{} is {}; a deferral can only sit on an idea, todo, or blocked task",
                task.id,
                task.status.as_str()
            )));
        }
        let today = crate::time::parse(&crate::time::now())?.date();
        task.defer = Some(crate::defer::Defer::resolve(value, today)?);
    }
```

`src/commands/edit.rs`: add `|| fields.defer.is_some() || args.no_defer` to `has_flags`, and after the `no_every` block:

```rust
    if args.no_defer {
        task.defer = None;
    }
```

- [x] **Step 5: Implement the goal refusals**

`src/hierarchy.rs`, after `validate_periodic`:

```rust
/// spec §3.3: a deferral on a goal hides nothing from `ready`, which already excludes any
/// task with children. Refused at the write, like a cadence.
pub fn validate_defer(project: &Project, registry: &Registry, task: &Task) -> Result<()> {
    if task.defer.is_none() {
        return Ok(());
    }
    let all = project.scan()?;
    let kids = children(&all, &task.id, registry);
    if !kids.is_empty() {
        return Err(Error::Validation(format!(
            "{} has children ({}) and cannot be deferred; a task with children is a goal, \
             and a goal is never ready",
            task.id,
            kids.iter()
                .map(|kid| kid.id.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(())
}
```

In `validate_parent`, replace the single `every` check with one read serving both:

```rust
    // spec §4.7 of the periodic design and §3.3 of the defer design: neither a recurrence
    // nor a deferred task is a goal, so nothing may hang beneath one.
    let parent_record = project.read_task(&parent)?;
    if parent_record.every.is_some() {
        return Err(Error::Validation(format!(
            "{parent} is a recurrence and cannot have children; clear its cadence with \
             `tasks edit {parent} --no-every` first"
        )));
    }
    if parent_record.defer.is_some() {
        return Err(Error::Validation(format!(
            "{parent} is deferred and cannot have children; clear the date with \
             `tasks edit {parent} --no-defer` first"
        )));
    }
```

`src/repo.rs`, `write_task`: add `crate::hierarchy::validate_defer(self, registry, task)?;` after the `validate_periodic` line.

- [x] **Step 6: Run the tests**

Run: `just test-fast defer`
Expected: the three end-to-end tests and the three new unit tests PASS. Then `just test-fast` — the full inner loop passes (the `every`-on-goal tests still pass with the shared read).

- [x] **Step 7: Close the step record and commit**

```bash
tasks done tasks-795fed "--defer (date or <n>d/<n>w, after today) and --no-defer; refused on every, on goals, on doing/shelved/closed; parenting under a deferred task refused"
git add src/defer.rs src/cli.rs src/complete.rs src/commands/mod.rs src/commands/edit.rs src/hierarchy.rs src/repo.rs tests/cli.rs tasks/
git commit -m "feat(defer): --defer and --no-defer with the write-time refusals"
```

---


## Controller supplement (spec coverage)

Additional spec §8 coverage: parenting a loose task beneath a deferred task must be refused through an editor save as well as flags. Exercise the existing shared writer guard; inspect editor implementation to choose the fixture.
