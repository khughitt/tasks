# Task References Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every task an explicit, ordered list of the documents and artifacts it cites (`## References` entries cited in prose as `[ref:<handle>]`), report one inventory of everything a task points at in `show`, and enforce it in `check`, `done`, `detach` and `move`.

**Architecture:**
- **Pure record layer.** `src/refs.rs` holds the `Ref` entry: its grammar (parse and render), its reference forms, the audit of duplicates and citations, and the presence and target checks. `src/format.rs` stores the entries as a managed body section between the body and `## Notes`.
- **Resolver.** `src/resolve.rs` gains `locate_entry` (regular files or directories, worktree first; special files refused), a special-file guard on every local document read, and `read_external`.
- **Citations.** The citation scanner uses `pulldown-cmark` to find what CommonMark parses as code, rather than a hand-written Markdown approximation.
- **Show.** `src/inventory.rs` builds `show`'s inventory rows from the governing links, the entries, the source and the attachment ledger, with the merge and status rules.
- **Gates.** Each lives where its command already decides: `transition()` (completion), `detach`, move preflight, and `check`.

**Tech Stack:** Rust (clap, serde, assert_cmd integration tests).

**Spec:** `docs/specs/2026-10-19-task-references-design.md` (approved 2026-10-19 after review round 2; amended by this plan's review round 1). Read it with this plan; section numbers below refer to it.

## Global Constraints

- **Every commit passes the pre-commit hook.** `just check` runs `cargo clippy --all-targets -- -D warnings`, and this is a binary crate, so an unused function, variant or field fails it.
  - A helper lands in the same task as its first production caller.
  - Never add `#[allow(dead_code)]` or another lint allowance to get past this.
  - Each task's commit step runs `just check` first.
- **Entry line** (spec §3.2): `- <handle> (<relation>[, required][, kind=<kind>]): <reference>[ — <description>]`. The separator is space, U+2014 em dash, space.
- **Field rules** (spec §3.2):
  - handle: `[a-z][a-z0-9-]{0,31}`;
  - relation: `context`, `input` or `output`;
  - kind: `[a-z][a-z0-9-]*`;
  - attribute order is fixed (relation, `required`, `key=value`), and an unknown attribute is an error.
- **Reference spelling** (spec §3.2):
  - *bare*: no whitespace, no `"`, no `\`;
  - *quoted* otherwise: `"…"` with `\"` and `\\` as the only escapes, reusing `frontmatter::take_quoted` and `quote`. The writer quotes exactly when bare is not allowed.
- **Reference forms** (spec §3.3):
  - local normalized repo-relative path: a regular file or a directory after following symlinks, with no trailing slash; the hint names the spelling without it;
  - `files:<name>` (an attachment);
  - `<scheme>:<rest>` (external, per `format::external_doc`).
  - A leading `@` is refused with exactly `cross-project references are not supported yet`. `files` is a reserved scheme.
  - Any other filesystem object (FIFO, socket, device) is a special file, and nothing ever opens one (spec §3.3):
    - writes of `context` and `input` entries, `doc`, and the completion gate refuse it as `special_file`;
    - `check` reports it for every relation and for spec and plan paths;
    - `show` reports status `special` with a warning.
    - `Resolver::read` refuses a special file at a local spec or plan path before opening it.
- **Section** (spec §3.1): heading exactly `## References`, after the body and before `## Notes`, written only when the task has entries. `validate_body` refuses the heading.
- **Section parse errors:** a second heading, the heading after `## Notes`, a malformed line, a duplicate handle.
- **Citation** (spec §4): `[ref:` up to the next `]` on the same line, in the body only.
  - Never inside anything CommonMark parses as code: an inline code span, or a fenced or indented code block.
  - The code ranges come from `pulldown_cmark::Parser::new(body).into_offset_iter()`, using `pulldown-cmark = { version = "0.13", default-features = false }`, which is in the local cargo cache.
  - A malformed handle is `ref_citation_malformed`; an unknown one is `ref_citation_dangling`.
- **Duplicate references** (spec §3.2): equal decoded references are `ref_duplicate`, naming both handles.
- **Audit placement.** `parse_task` calls `validate_task`, so the duplicate-reference and citation audit never runs there: a read must never fail on prose. It runs in `check` (open tasks) and on `add` and `edit` writes that change the body or the entries.
- **Error kinds** (JSON `error.kind`): `ref_not_found`, `ref_output_missing`, `ref_cites_attachment`, `ref_not_text`, `special_file`, `ref_duplicate`, `ref_citation_dangling`, `ref_citation_malformed`.
- **Check finding kinds** (spec §8):
  - `ref_missing` (error), `ref_in_main_checkout` (warning);
  - `special_file` (error): any relation, outputs included, and the spec and plan paths;
  - `ref_citation_dangling`, `ref_citation_malformed` and `ref_duplicate` (errors);
  - open tasks only.
- **Inventory row** (spec §7.2):
  - `origin`: `spec`, `plan`, `ref`, `source`, `attachment`;
  - `location`: `path`, `attachment`, `external`, `opaque`;
  - `status`: `present`, `missing`, `pending`, `special`, `available`, `unavailable`, `unchecked`.
  - Attachment rows come from the ledger, in name order (spec §5.1).
- **JSON omission** (spec §7): empty lists and absent options are omitted, never `[]` or `null`. `task.refs` is on full task objects only.
- **CLI surface** (spec §6):
  - `add --ref` and `edit --ref` (repeatable);
  - `edit --rm-ref` (repeatable) and `edit --no-refs`;
  - `list --ref` and `search --ref` (they share `ListArgs`);
  - `doc <id> [<handle>]`.
  
  The rows land in ops's `cli.toml` first, and this repo adopts them with `vendored adopt` in the parser commit (AGENTS.md Rules). Never edit `tools/cli.toml` by hand.
- **Skills activate on merge.** `~/.claude/skills/{tasks,scope,curate}` are symlinks into this repository's main checkout. Skill changes (Task 9) merge only after every host runs the new binary (see Rollout).
- **Tests:** run them only through `just test-one <args>` and `just test-fast`, never `cargo test` directly (AGENTS.md). After CLI changes, `cargo install --path .` before using `tasks` on this repo's own records.
- **Bounded tests.** A test that touches a FIFO sets `assert_cmd`'s `.timeout(Duration::from_secs(10))`. A test of a flag path that must not open the editor sets `EDITOR` to a script that writes a marker file and exits 1.

## Review Focus

1. A body that shows citation syntax inside backticks (including a code span broken across lines) or a fenced block, as an agent quoting the form would, must save. Test in Task 3 (unit) and Task 5 (integration).
2. A reference with spaces, a `"`, a `\` and a literal ` — ` round-trips byte for byte from `--ref` to the file to JSON. Test in Task 2 (unit) and Task 5 (integration).
3. Editing an unrelated field on a task whose existing `input` reference has since disappeared must not be refused: only new or changed entries are checked for targets. Test in Task 5.
4. A task declares a spec as an `output` entry, writes it, then links it with `edit --spec`. The inventory shows one row (origin `spec`, the entry's handle), and `[ref:<handle>]` still resolves. Test in Task 6.
5. `tasks done` on a recurring task already completed this occurrence, whose output file was deleted since, still releases its claim. Test in Task 7.

## Tracking

- **Close the design task.** tasks-3a445e ends when this plan is reviewed. Its closeout runs on the design branch `task-references`, before that branch merges:
  1. File the implementation goal: `tasks add "Implement task references" --plan 2026-10-19-task-references --spec 2026-10-19-task-references-design -p 2 --size l --complexity high --process planned --source tasks-3a445e`.
  2. Under that goal, add one child per `### Task N:` heading below, with `--parent <goal>`, `--plan 2026-10-19-task-references --step "Task N: <title>"`, `--process direct`, the complexity on each task's **Complexity:** line, and `--depends` on the previous step.
  3. `tasks done tasks-3a445e "<spec and plan reviewed; goal id>"`.
- **Run the implementation** in `.worktrees/task-references` on `task-references`, which already holds the spec and plan. Run the worktree skill's baseline tests (`just test-fast`) before the first code change. The Rollout below is the goal's last step.

## Concurrent work

- **tasks-a73dce and tasks-82bd08** (branch-only documents) touch `Resolver::locate`. `locate_entry` (Task 4) must follow whatever `locate` does after they land. Whichever lands second makes the two agree.
- **Rebase onto main** before Task 2 and before the final review. Expect conflicts near `KEYS`, `parse_task`, `ShowFields` and `transition`.

---

### Task 1: The CLI rows in ops

**Complexity:** low

The vendored `tools/cli.toml` arrives only from ops main, and `parser_surface_equals_table` fails until it matches the parser. These rows land in ops before Task 5 adopts them.

**Files:**
- Modify (in an ops worktree, then ops main): `cli.toml`

**Interfaces:**
- Produces: ops main's `cli.toml` with:
  - on `add`: `{ names = ["--ref"], value = "string", repeatable = true }`;
  - on `edit`: the same row, plus `{ names = ["--rm-ref"], value = "string", repeatable = true }` and `{ names = ["--no-refs"], value = "none" }`;
  - on `list` and `search`: `{ names = ["--ref"], value = "string" }`;
  - on `doc`: `args = [{ name = "id", value = "ref", required = true }, { name = "handle", value = "string", required = false }]` and `summary = "Print a linked plan or spec body, or a referenced document"`.

- [ ] **Step 1: Create the ops worktree**

In the ops main checkout (`tasks --pretty root ops-88a0fa` prints it):

```bash
work-link --ensure .worktrees
git worktree add -b feat/cli-task-refs .worktrees/cli-task-refs main
git worktree lock --reason "on WORK_ROOT storage (host: $(uname -n))" .worktrees/cli-task-refs
cd .worktrees/cli-task-refs && just setup
```

- [ ] **Step 2: Add the rows**

Edit `cli.toml` as the Interfaces block says. In `edit`, place `--rm-ref` and `--no-refs` after `--no-needs`, and `--ref` after `--depends`. In `add`, place `--ref` after `--depends`. In `list` and `search`, place `--ref` after `--plan`.

Run: `git diff --stat`
Expected: `cli.toml` only.

- [ ] **Step 3: Run ops's gate**

Run: `just test-fast`
Expected: PASS.

- [ ] **Step 4: Commit, merge locally, remove the worktree**

```bash
git add cli.toml
git commit -m "feat(cli): task reference options on tasks add, edit, list, search and doc"
cd "$(tasks --pretty root ops-88a0fa)"
git merge --ff-only feat/cli-task-refs
git -C .worktrees/cli-task-refs status --ignored --porcelain   # copy out anything still needed
git worktree unlock .worktrees/cli-task-refs
git worktree remove .worktrees/cli-task-refs
git branch -d feat/cli-task-refs
```

Expected: the merge fast-forwards. If it does not, rebase onto ops main, rerun Step 3, and merge again. ops is a personal-profile repository, so a local merge needs no further approval.

---

### Task 2: Entries and the stored section

**Complexity:** mid

**Files:**
- Create: `src/refs.rs` (declared in `src/main.rs` beside `mod resolve;`)
- Modify: `src/model.rs` (`Relation`, `Ref`, `Task::refs`, the `Task { … }` literal)
- Modify: `src/format.rs` (`REFS_DELIMITER`, `split_body_notes` → `split_body`, `parse_task`, `serialize_task`, `validate_body`, `validate_task`, a `normalized_rel` helper extracted from `validate_doc_shape`)
- Modify: `src/frontmatter.rs` (`take_quoted` and `quote` become `pub(crate)`)
- Modify: every other `Task { … }` literal. `grep -rln "notes: vec!\[\]" src` lists them: `src/halt.rs`, `src/complexity.rs`, `src/hierarchy.rs`, `src/repo.rs`, `src/defer.rs`, `src/lanes.rs`, `src/periodic.rs`, `src/query.rs`, `src/similarity.rs`, `src/commands/add.rs`.
- Test: unit tests in `src/refs.rs` and `src/format.rs`

**Interfaces:**
- Produces, in `src/model.rs`:
  - `pub enum Relation { Context, Input, Output }`, deriving `Debug, Clone, Copy, PartialEq, Eq, Serialize` with `#[serde(rename_all = "lowercase")]`, plus `Relation::parse(&str) -> Result<Relation>` and `Relation::as_str(self) -> &'static str`.
  - `pub struct Ref`, deriving `Debug, Clone, PartialEq, Eq, Serialize`, with fields `handle: String`, `relation: Relation`, `required: bool`, `kind: Option<String>`, `reference: String` (decoded, never quoted) and `description: Option<String>`. `kind` and `description` carry `skip_serializing_if = "Option::is_none"`.
  - `Task.refs: Vec<Ref>`, after `body` and before `notes`, with `skip_serializing_if = "Vec::is_empty"`.
- Produces, in `src/refs.rs`:
  - `pub fn parse_entry(text: &str) -> Result<Ref>`: the entry without its leading `- `, with `Error::Validation` on any grammar or form error.
  - `pub fn render_entry(entry: &Ref) -> String`: without the leading `- `.
  - `pub fn validate_entry(entry: &Ref) -> Result<()>`: field rules plus `validate_reference`.
  - `pub fn validate_reference(reference: &str) -> Result<()>`: the form rules of Global Constraints, without filesystem access. Task 4 reimplements it over `location`.
- Produces, in `src/format.rs`: `pub const REFS_DELIMITER: &str = "## References";` and `pub fn normalized_rel(rel: &str) -> bool`. The latter is the segment rule `validate_doc_shape` applies today; `validate_doc_shape` calls it.

- [ ] **Step 1: Write the failing grammar tests in `src/refs.rs`**

```rust
#[test]
fn an_entry_round_trips_every_field() {
    let text = "brief (input, required, kind=brief): docs/notes/b.md — admission rules, §7";
    let entry = parse_entry(text).unwrap();
    assert_eq!(entry.handle, "brief");
    assert_eq!(entry.relation, Relation::Input);
    assert!(entry.required);
    assert_eq!(entry.kind.as_deref(), Some("brief"));
    assert_eq!(entry.reference, "docs/notes/b.md");
    assert_eq!(entry.description.as_deref(), Some("admission rules, §7"));
    assert_eq!(render_entry(&entry), text);
    let bare = parse_entry("r1 (context): files:after.png").unwrap();
    assert_eq!((bare.required, bare.kind.clone(), bare.description.clone()), (false, None, None));
    assert_eq!(render_entry(&bare), "r1 (context): files:after.png");
}

#[test]
fn a_reference_needing_quotes_round_trips() {
    let text = r#"rb (output, required): "docs/run books/a \"v2\" \\ — x.md" — revised here"#;
    let entry = parse_entry(text).unwrap();
    assert_eq!(entry.reference, r#"docs/run books/a "v2" \ — x.md"#);
    assert_eq!(entry.description.as_deref(), Some("revised here"));
    assert_eq!(render_entry(&entry), text);
    let ext = parse_entry(r#"t (context): "stub:a b""#).unwrap();
    assert_eq!(ext.reference, "stub:a b");
}

#[test]
fn malformed_entries_are_refused_with_the_reason() {
    for (text, needle) in [
        ("Brief (input): a.md", "handle"),
        ("b (uses): a.md", "relation"),
        ("b (input, pinned): a.md", "attribute"),
        ("b (input, kind=x, required): a.md", "order"),
        ("b (input): a.md trailing", " — "),
        ("b (input): a.md — ", "description"),
        ("b (input): results/run-12/", "results/run-12"),
        ("b (input): @lore/docs/x.md", "cross-project references are not supported yet"),
        ("b (input): docs/../x.md", "normalized"),
        ("b (input): files:.hidden", "attachment name"),
        ("b (input): \"unterminated", "unterminated"),
    ] {
        let error = parse_entry(text).unwrap_err().to_string();
        assert!(error.contains(needle), "{text}: {error}");
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one refs::`
Expected: FAIL (compile errors: `parse_entry`, `Ref`, `Relation` undefined).

- [ ] **Step 3: Implement `Relation`, `Ref`, `parse_entry`, `render_entry`, `validate_entry` and `validate_reference`**

Parsing:
1. Split the handle off at the first ` (`, then the attributes at the first `): `.
2. The reference is quoted when it starts with `"`; read it with `frontmatter::take_quoted`. Otherwise it runs to the first whitespace.
3. What remains must be empty or ` — ` followed by a non-empty description.

`render_entry` quotes with `frontmatter::quote` exactly when the reference contains whitespace, `"` or `\`, or starts with `"`.

`validate_reference`, in this order:
1. A leading `@` is the refusal from Global Constraints.
2. `files:<name>` needs `attachments::validate_name(name)`.
3. `format::external_doc` returning `Some` is accepted.
4. Otherwise the reference must pass `format::normalized_rel`, and a trailing `/` is refused with `use <path without the slash>`.

- [ ] **Step 4: Run them to verify they pass**

Run: `just test-one refs::`
Expected: PASS.

- [ ] **Step 5: Write the failing storage tests in `src/format.rs`**

```rust
const WITH_REFS: &str = "---\nid: sci-4f2a9c\ntitle: T\nstatus: todo\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n\nSee [ref:brief].\n\n## References\n\n- brief (input, required): docs/notes/b.md — the brief\n- out (output): \"docs/a b.md\"\n\n## Notes\n\n- 2026-08-29T15:10:44Z (keith): started\n";

#[test]
fn the_references_section_round_trips_between_body_and_notes() {
    let task = parse_task(WITH_REFS, "t.md").unwrap();
    assert_eq!(task.body, "See [ref:brief].");
    assert_eq!(task.refs.len(), 2);
    assert_eq!(task.refs[1].reference, "docs/a b.md");
    assert_eq!(task.notes.len(), 1);
    assert_eq!(serialize_task(&task), WITH_REFS);
}

#[test]
fn no_section_is_written_without_entries() {
    let task = parse_task(FULL, "t.md").unwrap();
    assert_eq!(serialize_task(&task), FULL);
    assert!(!serialize_task(&task).contains(REFS_DELIMITER));
}

#[test]
fn a_misplaced_or_repeated_section_is_a_parse_error() {
    let notes_at = WITH_REFS.find("## Notes").unwrap();
    let cases = [
        (WITH_REFS.replacen("## Notes", "## References\n\n## Notes", 1), "second ## References heading"),
        (format!("{}\n## References\n", WITH_REFS), "## References after ## Notes"),
        (format!("{}not an entry\n\n{}", &WITH_REFS[..notes_at], &WITH_REFS[notes_at..]), "malformed reference line"),
        (WITH_REFS.replacen("- out (output)", "- brief (output)", 1), "duplicate reference handle brief"),
    ];
    for (text, needle) in cases {
        let error = parse_task(&text, "t.md").unwrap_err().to_string();
        assert!(error.contains(needle), "{needle}: {error}");
    }
}

#[test]
fn the_body_refuses_the_references_heading() {
    let error = validate_body("x\n## References\n").unwrap_err().to_string();
    assert!(error.contains("## References"), "{error}");
}

#[test]
fn citation_text_in_the_body_never_fails_a_read() {
    let text = WITH_REFS.replacen("See [ref:brief].", "See [ref:gone] and [ref:Bad].", 1);
    assert!(parse_task(&text, "t.md").is_ok());
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `just test-one format::`
Expected: the new tests FAIL. Existing tests still pass once `refs: vec![]` is added to every literal.

- [ ] **Step 7: Implement the section**

- **Parsing.** `split_body_notes` becomes `split_body(after, file) -> Result<(String, Vec<Ref>, Vec<Note>)>`:
  - a line equal to `REFS_DELIMITER` before `## Notes` switches to entries;
  - blank lines are skipped;
  - each `- ` line goes through `refs::parse_entry`, with errors wrapped by `perr`;
  - anything else is `malformed reference line {line:?}`.
- **Validation.** `validate_task` adds `refs::validate_entry` for each entry and the duplicate-handle error, and nothing else (Audit placement).
- **Serializing.** `serialize_task` writes `\n## References\n\n` and one `- <render_entry>` line per entry, after the body and before the notes, only when `refs` is non-empty.
- **Body.** `validate_body` refuses `## References` as it refuses `## Notes`.

- [ ] **Step 8: Run the fast suite and the lint gate**

Run: `just test-fast`, then `just check`.
Expected: both PASS.

- [ ] **Step 9: Commit**

```bash
git add src
git commit -m "feat(refs): reference entries stored in a managed body section"
```

---

### Task 3: The audit in `check`

**Complexity:** mid

**Files:**
- Modify: `Cargo.toml`, `Cargo.lock` (`pulldown-cmark = { version = "0.13", default-features = false }`)
- Modify: `src/refs.rs` (citations, `Problem`, `audit`)
- Modify: `src/config.rs` (the `files` scheme reservation)
- Modify: `src/commands/check.rs` (audit findings)
- Move: `worktree_with_excluded_docs`, `check_findings`, `findings_of` and `write_doc` from `tests/cli.rs` to `tests/common/mod.rs` as `pub fn`s, unchanged.
- Create: `tests/refs.rs`
- Record: tasks-3a445e's body (Step 7)

**Interfaces:**
- Produces, in `src/refs.rs`:
  - `pub enum Citation<'a> { Handle(&'a str), Malformed(&'a str) }`, deriving `Debug, PartialEq, Eq`, and `pub fn citations(body: &str) -> Vec<Citation<'_>>`.
  - `pub struct Problem { pub kind: &'static str, pub detail: String }`, with the constants `DUPLICATE = "ref_duplicate"`, `CITATION_DANGLING = "ref_citation_dangling"` and `CITATION_MALFORMED = "ref_citation_malformed"`.
  - `pub fn audit(task: &Task) -> Vec<Problem>`.
- Consumed later: Task 5 adds `refuse_problems` and `Error::Reference` alongside their first callers (the writers).

- [ ] **Step 1: Write the failing unit tests in `src/refs.rs`**

```rust
#[test]
fn citations_skip_code_spans_and_fences() {
    let body = "Use [ref:brief] here.\nQuote `[ref:shown]` or ``[ref:also]``.\nLiteral `example\n[ref:sample]` here.\n```\n[ref:fenced]\n```\n~~~md\n[ref:tilde]\n~~~\nAnd [ref:Bad Handle] and [ref:open";
    assert_eq!(citations(body), vec![
        Citation::Handle("brief"),
        Citation::Malformed("Bad Handle"),
        Citation::Malformed("open"),
    ]);
}

#[test]
fn a_code_span_does_not_cross_a_block_boundary() {
    // The heading's backtick is literal: a heading is its own block (reviewer's case).
    assert_eq!(citations("# Example `\nUse `[ref:sample]` literally."), vec![]);
    // A list item boundary ends the paragraph too.
    assert_eq!(citations("- one `\n- two `[ref:sample]` x"), vec![]);
    // An indented code block is code.
    assert_eq!(citations("Text.\n\n    [ref:indented]\n\nAfter [ref:real]."), vec![Citation::Handle("real")]);
}

#[test]
fn a_code_span_does_not_cross_a_blank_line() {
    let body = "Open `tick\n\nThen [ref:real] and `close`.";
    assert_eq!(citations(body), vec![Citation::Handle("real")]);
}

#[test]
fn audit_names_duplicates_and_unresolved_citations() {
    let text = "---\nid: sci-4f2a9c\ntitle: T\nstatus: todo\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n\nSee [ref:a] and [ref:gone] and [ref:X].\n\n## References\n\n- a (context): docs/x.md\n- b (input): docs/x.md\n";
    let task = crate::format::parse_task(text, "t.md").unwrap();
    let problems = audit(&task);
    let kinds: Vec<_> = problems.iter().map(|p| p.kind).collect();
    assert_eq!(kinds, vec![DUPLICATE, CITATION_DANGLING, CITATION_MALFORMED]);
    assert!(problems[0].detail.contains("a and b both reference docs/x.md"), "{}", problems[0].detail);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one refs::`
Expected: FAIL (`citations`, `audit` undefined).

- [ ] **Step 3: Implement `citations` and `audit`**

**Citations.**
1. **Code ranges.** Collect the byte ranges of every `Event::Code` and of every code block (the range of `Event::Start(Tag::CodeBlock(_))`, which spans the whole block, fenced or indented) from `pulldown_cmark::Parser::new(body).into_offset_iter()`.
2. **Citations.** Find each `[ref:` in the body whose start offset lies in no code range. It ends at the next `]` on the same line. It is `Handle` when its text matches the handle grammar, and `Malformed` otherwise, including an unterminated one, whose text runs to the end of the line.

**Audit.** Duplicates come first, as pairs in entry order, with detail `<first> and <second> both reference <reference>`. Then citations in body order: an unknown handle with detail `[ref:<h>] names no reference`, and a malformed one with detail `[ref:<text>] is not a reference handle`.

- [ ] **Step 4: Run them to verify they pass**

Run: `just test-one refs::`
Expected: PASS.

- [ ] **Step 5: Write the failing integration tests in `tests/refs.rs`**

```rust
#[test]
fn check_reports_reference_audit_findings_on_open_tasks_only() {
    let mut env = TestEnv::new();
    let dir = env.init("rf");
    write_doc(&dir, "docs/x.md", "x");
    let id = id_of(env.json(&dir, &["add", "A"]));
    // Hand-write the body and section: the CLI writers arrive in Task 5.
    let rel = format!("tasks/{id}.md");
    let text = env.read(&dir, &rel).replacen("---\n\n", "---\n\nSee [ref:gone] and [ref:Bad].\n\n## References\n\n- a (context): docs/x.md\n- b (input): docs/x.md\n\n", 1);
    std::fs::write(dir.join(&rel), text).unwrap();
    let (code, v) = check_findings(&env, &dir);
    assert_eq!(code, 1, "{v}");
    for kind in ["ref_duplicate", "ref_citation_dangling", "ref_citation_malformed"] {
        assert_eq!(findings_of(&v, "errors", kind).len(), 1, "{kind}: {v}");
    }
    env.json(&dir, &["start", &id]);
    env.json(&dir, &["done", &id, "x"]);
    let (code, v) = check_findings(&env, &dir);
    assert_eq!(code, 0, "{v}");
}

```

Adjust the `replacen` anchor to the record shape `add` writes if it differs (`env.read` the record first).

In `src/config.rs` unit tests (the host configuration loads lazily, so an integration command would not necessarily read it):

```rust
#[test]
fn the_files_scheme_is_reserved() {
    let error = HostConfig::parse("[docs.schemes]\nfiles = 'cat'\n", "c.toml").unwrap_err().to_string();
    assert!(error.contains("files is reserved for task attachments"), "{error}");
}
```

- [ ] **Step 6: Implement the check findings and the scheme reservation**

- **Check.** In `check::run`, for each open task, push `finding(Some(task), file, problem.kind, problem.detail)` into `errors` for every `refs::audit` problem.
- **Scheme reservation.** In `HostConfig::parse`, refuse the key `files` with `{path}: [docs.schemes] key files is reserved for task attachments`.

Run: `just test-one --test refs` and then `just test-fast`.
Expected: PASS.

- [ ] **Step 7: Fix this repository's own record**

tasks-3a445e's body contains an unescaped `[ref:<handle>]` (and `[ref:` text). Rewrite the body with `tasks edit tasks-3a445e --body -` so every citation-form example sits in backticks. Then confirm with the branch binary:

Run: `cargo run -q -- check`
Expected: no `ref_citation_*` finding.

- [ ] **Step 8: Lint gate and commit**

Run: `just check`
Expected: PASS.

```bash
git add Cargo.toml Cargo.lock src tests
git commit -m "feat(refs): check audits duplicate references and citations"
```

---

### Task 4: Locating entries, and `check` for their targets

**Complexity:** mid

**Files:**
- Modify: `src/refs.rs` (`Location`, `location`, `present`; `validate_reference` now calls `location`)
- Modify: `src/resolve.rs` (`LocatedEntry`, `locate_entry`, `refuse_special`, the guard in `read`)
- Modify: `src/error.rs` (`Error::SpecialFile(String)` → `special_file`)
- Modify: `src/commands/check.rs` (the open-task block beside `doc_missing`, and the spec and plan loop)
- Test: unit tests in `src/resolve.rs`; integration in `tests/refs.rs`

**Interfaces:**
- Produces, in `src/refs.rs`:
  - `pub enum Location<'a> { Path(&'a str), Attachment(&'a str), External }` and `pub fn location(reference: &str) -> Result<Location<'_>>`, with the same rules and messages as Task 2's `validate_reference`, which becomes `location(reference).map(|_| ())`.
  - `pub enum Presence { Here(PathBuf), Main { root: PathBuf, path: PathBuf }, Absent(PathBuf), Special(PathBuf), Unsafe { path: PathBuf, detail: String } }`. Every variant carries the path actually examined. `Absent` carries the path that would hold the target: the worktree path, or the attachment's storage path.
  - `pub fn present(resolver: &Resolver, project: &Project, task: &Task, entry: &Ref) -> Result<Presence>`. Callers map each variant explicitly (the table in Step 3, and the callers in Tasks 5-7).
    - A local path maps `locate_entry` one to one (`Here`, `Main`, `Special`, and `Missing` → `Absent(project.root.join(rel))`).
    - An attachment keeps the existing safety rules: `attachments::check_storage` first, and `entry_state` (which never follows a symlink) on the storage path.
      - Unsafe storage gives `Unsafe` with its detail. Catch `Error::AttachmentUnsafe` from `check_storage` for this; nothing beneath it is examined.
      - A name the ledger does not hold as `Attached`, or `EntryState::Absent`, gives `Absent(path)`.
      - `File` gives `Here(path)`.
      - `Unsafe` whose object is a special file (the `symlink_metadata` file type is neither file, directory nor symlink) gives `Special(path)`. Any other `Unsafe` (a symlink or a directory) gives `Unsafe { path, detail }`.
    - External entries: never called (callers branch on `location` first).
- Produces, in `src/resolve.rs`:
  - `pub enum LocatedEntry { Here(PathBuf), Main { root: PathBuf, path: PathBuf }, Special(PathBuf), Missing }`. It records no file-or-directory kind, because no caller would read it (Clippy's `dead_code`); `doc` tests `is_dir()` on the located path. `Special` keeps the path where the special file actually is, here or in main.
  - `Resolver::locate_entry(&self, rel: &str) -> Result<LocatedEntry>`.
    - Look at `project.root.join(rel)` with `fs::metadata`, which follows symlinks. Only `NotFound` means nothing exists; other errors propagate.
    - A regular file or a directory is found. Any other type is `Special(path)` with that path. It is not an error, so each caller decides, and the path is never opened.
    - Only when nothing exists here is the main checkout consulted, by the same rule.
  - `pub(crate) fn refuse_special(path: &Path) -> Result<()>`: `fs::metadata` (following symlinks). `NotFound`, a regular file and a directory are `Ok`; anything else is `Error::SpecialFile("<path> is not a regular file or directory")`. Used by the guard in `read`, and by callers that turn `Special(path)` into a refusal.
  - **The guard in `Resolver::read`.** Before `read_to_string` on a local document, call `refuse_special` on the path it is about to open. This covers `doc --plan`, `doc --spec`, `step_exists`, `edit --step` and `check`'s plan reads, none of which may hang on a FIFO.

- [ ] **Step 1: Write the failing tests**

In `src/resolve.rs` unit tests, using a temp project root:

```rust
#[test]
fn locate_entry_finds_files_and_directories_here_first() {
    // root/docs/a.md (file), root/results/run-12/ (dir), root/link -> docs/a.md (symlink)
    assert!(matches!(resolver.locate_entry("docs/a.md").unwrap(), LocatedEntry::Here(p) if p.is_file()));
    assert!(matches!(resolver.locate_entry("results/run-12").unwrap(), LocatedEntry::Here(p) if p.is_dir()));
    assert!(matches!(resolver.locate_entry("link").unwrap(), LocatedEntry::Here(_)));
    assert!(matches!(resolver.locate_entry("nope").unwrap(), LocatedEntry::Missing));
    assert!(matches!(resolver.locate("results/run-12").unwrap(), Located::Missing)); // locate stays file-only
}

#[test]
fn locate_entry_reports_a_fifo_and_a_symlink_to_one_as_special() {
    // std::process::Command::new("mkfifo").arg(root.join("pipe")); symlink root/to-pipe -> pipe
    for rel in ["pipe", "to-pipe"] {
        assert!(matches!(resolver.locate_entry(rel).unwrap(), LocatedEntry::Special(p) if p == root.join(rel)));
    }
}
```

In `tests/refs.rs`, using the helpers moved in Task 3, hand-writing the section as Task 3 did:

```rust
#[test]
fn check_reports_missing_main_checkout_and_special_targets_on_open_tasks() {
    // worktree_with_excluded_docs: entry i (input): docs/specs/x-design.md exists only in main
    //   → from side: warning ref_in_main_checkout whose detail names main.
    // remove it from main → error ref_missing naming "i".
    // entry o (output): docs/later.md, absent → no finding.
    // entry f (context): files:gone.png never attached → error ref_missing.
    // attach shot.png, entry s (input): files:shot.png, then delete the stored bytes → ref_missing.
    // mkfifo docs/pipe, entry p (context): docs/pipe → error special_file
    //   (run check with .timeout(Duration::from_secs(10))).
    // close the task → none of these findings.
}
```

Write it in full.

In `tests/refs.rs`:

```rust
#[test]
fn a_fifo_at_a_linked_plan_path_is_refused_not_opened() {
    // add T with plan docs/plans/x.md, step "Task 1: A" (file with that heading);
    // replace docs/plans/x.md with a FIFO (remove, then mkfifo).
    // With .timeout(Duration::from_secs(10)) on each command:
    // `doc <id> --plan` → kind special_file;
    // `check` → error finding special_file naming the plan path (no doc_missing for it).
}
```

- [ ] **Step 2: Run to verify failure**

Run: `just test-one resolve::` and `just test-one --test refs check_reports_missing`
Expected: FAIL.

- [ ] **Step 3: Implement `location`, `present`, `locate_entry`, `SpecialFile` and the check findings**

In `check`, inside `if task.status.is_open()`, map `present` for each non-external entry. For `context` and `input` entries:
- `Main { root, path }`: warning `ref_in_main_checkout`, detail `reference <h> <rel> is not in this checkout; read from <path> in the main checkout at <root>`;
- `Absent(path)`: error `ref_missing`, detail `reference <h> <reference> does not exist (looked for <path>)`;
- `Special(path)`: error `special_file`, detail `reference <h>: <path> is not a regular file or directory`;
- `Unsafe { path, detail }`: error `ref_missing`, detail `reference <h>: <path> is not a usable attachment (<detail>)`, alongside the audit's existing `attachment_unsafe`.

These details read every field `present` returns, so Task 4 passes Clippy's `dead_code` with no later caller.

For an `output` entry, only `Special` is reported (spec §8's exception).

In the spec and plan loop, a `Located::Missing` link for which `locate_entry` gives `Special(path)` is the error `special_file` naming that path (here or main), instead of `doc_missing`.

- [ ] **Step 4: Run the tests and the lint gate**

Run: `just test-one resolve::`, `just test-one --test refs`, `just test-fast`, then `just check`.
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src tests
git commit -m "feat(refs): locate entries worktree first; check reports missing and special targets"
```

---

### Task 5: Writing entries from the CLI, `list --ref` and `doc <handle>`

**Complexity:** high

**Files:**
- Adopt: `tools/cli.toml` (`python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml`)
- Modify: `src/cli.rs`:
  - `FieldArgs.refs`: `--ref`, `value_name = "ENTRY"`, repeatable;
  - `EditArgs.rm_refs` and `EditArgs.no_refs`;
  - `ListArgs.reference`: `--ref`;
  - `Command::Doc`: an optional positional `handle`, with the `doc_kind` group over `plan`, `spec` and `handle`, still required and single. The doc comment matches the new summary row.
- Modify: `src/refs.rs` (`refuse_problems`, `require_targets`)
- Modify: `src/resolve.rs` (`read_external` split out of `read`; `note_main_entry`)
- Modify: `src/error.rs` (`Error::Reference { kind, detail }`, `Error::RefNotFound(String)` → `ref_not_found`, `Error::RefNotText(String)` → `ref_not_text`)
- Modify: `src/commands/mod.rs` (`apply_fields`)
- Modify: `src/commands/edit.rs` (`has_flags`, the clear-and-remove block, `editor`)
- Modify: `src/commands/list.rs` (both `plan` filter sites)
- Modify: `src/commands/doc.rs`, `src/main.rs` (dispatch of `Doc`)
- Test: `tests/refs.rs`

**Interfaces:**
- Consumes: `refs::parse_entry`, `audit`, `location` and `present`; `Resolver::locate_entry` (Tasks 2-4).
- Produces:
  - `pub fn refuse_problems(task: &Task) -> Result<()>`: the first `audit` problem as `Error::Reference { kind: problem.kind, detail: problem.detail }`. `Error::kind()` returns `kind`.
  - `pub fn require_targets(resolver: &Resolver, project: &Project, task: &Task, original: Option<&Task>) -> Result<()>`. It checks every non-external `context` or `input` entry whose `(reference, relation)` pair is not in `original`, mapping `present` as follows:
    - `Here`: ok;
    - `Main`: ok, with `note_main_entry`;
    - `Absent`: `Error::RefNotFound("<id>: reference <handle> <reference> does not exist")`;
    - `Special(path)`: `refuse_special(&path)`'s `SpecialFile` error;
    - `Unsafe { detail, .. }`: `Error::AttachmentUnsafe(detail)`, the existing kind.
  - `Resolver::note_main_entry(&self, handle: &str, rel: &str, root: &Path)`: queues `reference <handle> <rel> is not in this checkout; read from the main checkout at <root>` for `take_warnings`.
  - `Resolver::read_external(&self, value: &str) -> Result<DocRead>`: the external branch of `read`, moved out unchanged, which `read` now calls.
  - `doc::run(ctx, id, target: DocTarget, fetch)`, with `pub enum DocTarget { Plan, Spec, Ref(String) }` replacing the `DocKind` argument.

- [ ] **Step 1: Write the failing integration tests in `tests/refs.rs`**

```rust
#[test]
fn add_and_edit_write_entries_that_show_returns() {
    // init "rf"; write docs/notes/b.md and "docs/run books/a.md"
    let id = id_of(env.json(&dir, &["add", "T", "--body", "See [ref:brief].",
        "--ref", "brief (input, required, kind=brief): docs/notes/b.md — the brief",
        "--ref", r#"rb (output): "docs/run books/a \"v2\" \\ — x.md""#]));
    let refs = &env.json(&dir, &["show", &id])["task"]["refs"];
    assert_eq!(refs[0], serde_json::json!({"handle":"brief","relation":"input","required":true,"kind":"brief","reference":"docs/notes/b.md","description":"the brief"}));
    assert_eq!(refs[1], serde_json::json!({"handle":"rb","relation":"output","required":false,"reference":"docs/run books/a \"v2\" \\ — x.md"}));
    assert!(env.read(&dir, &format!("tasks/{id}.md")).contains("## References\n\n- brief (input, required, kind=brief): docs/notes/b.md — the brief\n"));
    // `edit --ref` alone never opens the editor (EDITOR stub writes a marker and exits 1)
    let marker = env.home.path().join("editor-ran");
    let stub = write_editor_stub(&env, &marker); // `#!/bin/sh\ntouch <marker>\nexit 1`
    let out = env.cmd(&dir).env("EDITOR", &stub)
        .args(["edit", &id, "--ref", "brief (context): docs/notes/b.md", "--ref", "c (context): https://example.org/x"])
        .timeout(std::time::Duration::from_secs(10)).output().unwrap();
    assert!(out.status.success() && !marker.exists());
    let refs = env.json(&dir, &["show", &id])["task"]["refs"].clone();
    let handles: Vec<_> = refs.as_array().unwrap().iter().map(|r| r["handle"].as_str().unwrap().to_string()).collect();
    assert_eq!(handles, ["brief", "rb", "c"]);
    assert_eq!(refs[0]["relation"], "context");
    // --rm-ref and --no-refs alone never open the editor either (same stub, same assertions)
}

#[test]
fn writes_refuse_broken_entries_and_citations() {
    assert_eq!(env.fail(&dir, &["add", "T", "--ref", "a (input): docs/none.md"]), "ref_not_found");
    assert_eq!(env.fail(&dir, &["add", "T", "--body", "[ref:gone]"]), "ref_citation_dangling");
    assert_eq!(env.fail(&dir, &["add", "T", "--ref", "a (context): docs/b.md", "--ref", "b (input): docs/b.md"]), "ref_duplicate");
    assert_eq!(env.fail(&dir, &["add", "T", "--ref", "a (context): docs/b.md", "--ref", "a (input): docs/b.md"]), "validation");
    // outputs need not exist; a files: output need not be attached
    env.json(&dir, &["add", "T", "--ref", "o (output): docs/later.md", "--ref", "s (output): files:after.png"]);
    // code spans (one line and across lines) and fences are not citations (Review Focus 1)
    env.json(&dir, &["add", "T", "--body", "Write `[ref:x]` to cite.\nLiteral `example\n[ref:sample]` here.\n```\n[ref:y]\n```"]);
    // --rm-ref of a cited handle is refused unless the same call replaces the body
    let id = id_of(env.json(&dir, &["add", "T", "--body", "See [ref:a].", "--ref", "a (context): docs/b.md"]));
    assert_eq!(env.fail(&dir, &["edit", &id, "--rm-ref", "a"]), "ref_citation_dangling");
    env.json(&dir, &["edit", &id, "--rm-ref", "a", "--body", "Nothing cited."]);
    assert_eq!(env.fail(&dir, &["edit", &id, "--rm-ref", "a"]), "validation");
}

#[test]
fn an_attachment_input_needs_its_bytes_not_just_its_ledger_entry() {
    // attach shot.png to T; delete tasks/files/<id>/shot.png;
    assert_eq!(env.fail(&dir, &["edit", &id, "--ref", "s (input): files:shot.png"]), "ref_not_found");
}

#[test]
fn an_unrelated_edit_ignores_an_input_that_has_since_vanished() {
    // add with a (input): docs/b.md; delete docs/b.md; `edit <id> -p 1` succeeds (Review Focus 3)
}

#[test]
fn editor_saves_validate_added_entries() {
    // EDITOR script appends "## References\n\n- x (input): docs/none.md\n" before ## Notes
    // (or at the end when no notes) → exit 1, kind ref_not_found, edit kept;
    // appending "- x (input): docs/b.md" instead lands.
}

#[test]
fn list_and_search_filter_by_reference_byte_for_byte() {
    // two tasks citing docs/b.md and docs/c.md; `list --ref docs/b.md` returns one id;
    // `search T --ref docs/b.md` the same; `list --ref docs/b` returns none.
}

#[test]
fn doc_reads_an_entry_by_handle() {
    // text file → body printed raw; directory → ref_not_text; bytes [0xff,0xfe] → ref_not_text;
    // pending output → doc_not_found; unknown handle → doc_not_found;
    // attachment entry (after `tasks attach`) → its text;
    // external "stub:x" via DocFixture → fixture body; with --no-fetch → doc_not_found.
}

#[test]
fn a_fifo_is_refused_without_being_opened() {
    // mkfifo docs/pipe. With .timeout(Duration::from_secs(10)) on each command:
    // `add T --ref "p (context): docs/pipe"` → special_file;
    // hand-write entry p (context): docs/pipe into a record, then `doc <id> p` → special_file.
}
```

Write the elided bodies in full, with the assertions named in each comment. Put `write_editor_stub` in `tests/common/mod.rs` if no equivalent exists there (`grep -n EDITOR tests/common/mod.rs tests/cli.rs`).

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test refs`
Expected: FAIL (unknown options).

- [ ] **Step 3: Adopt the CLI table and add the clap options**

Run: `python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml`, then add the options listed under **Files**.

Run: `just test-one parser_surface_equals_table`
Expected: PASS.

- [ ] **Step 4: Implement the writers**

- **`has_flags`** (`edit::run`) gains all three: `!fields.refs.is_empty()`, `args.no_refs` and `!args.rm_refs.is_empty()`.
- **`edit::run`.** Following the tag pattern, `--no-refs` clears and then `--rm-ref` removes (`<id> has no reference <h>` when absent) before `apply_fields`.
- **`apply_fields`**, after the body is set:
  1. Parse each `fields.refs` with `refs::parse_entry`; a handle repeated within the call is `Validation("--ref names <h> twice")`.
  2. Replace an existing handle in place, or append.
  3. Call `refs::require_targets(&resolver, &ctx.project, task, original)`, then move `resolver.take_warnings()` into `ctx.warnings`.
  4. When `original` is `None`, or the body or entries differ from `original`, call `refs::refuse_problems(task)`.
- **`editor`.** After the needs check, call `require_targets(&resolver, &ctx.project, &edited, Some(&original))`. Then call `refuse_problems(&edited)` when the body or entries changed. Both errors go through `.map_err(keep)`.

- [ ] **Step 5: Implement `list --ref` and `doc <handle>`**

- **`list --ref`.** At both `plan` filter sites, add `reference.as_deref().is_none_or(|r| task.refs.iter().any(|e| e.reference == r))`. On the parked-row site, an unresolved park has no record and does not match.
- **`doc`.** Find the entry by handle, else `DocNotFound("<id> has no reference <h>")`.
  - An external reference goes through `read_external(..)?.required()`.
  - Otherwise map `refs::present`:
    - `Here` or `Main` gives the path to read;
    - `Absent` is `DocNotFound("<id> reference <h> <reference> does not exist yet")`;
    - `Special(path)` is the `SpecialFile` error, without opening it;
    - `Unsafe` is `AttachmentUnsafe`.
  - A directory is `RefNotText("<path> is a directory")`.
  - The bytes must be UTF-8, else `RefNotText("<path> is not UTF-8 text")`.
  - Print as `doc` prints today.

- [ ] **Step 6: Run the tests and the lint gate**

Run: `just test-one --test refs`, `just test-fast`, then `just check`.
Expected: all PASS.

- [ ] **Step 7: Commit, then reinstall**

```bash
git add tools/cli.toml src tests
git commit -m "feat(refs): --ref, --rm-ref and --no-refs; list --ref; doc <handle>"
cargo install --path .
```

---

### Task 6: The inventory in `show`

**Complexity:** high

**Files:**
- Create: `src/inventory.rs` (declared in `src/main.rs`)
- Modify: `src/output.rs` (`InventoryRow`, `Origin`, `RowLocation`, `RowStatus`, `ShowFields.inventory`)
- Modify: `src/commands/show.rs` (`describe`: the `step_found` fix in Step 3, then the inventory after `step_found` and `files`)
- Modify: `src/show_text.rs` (a `references` section between `related` and `files`)
- Modify: every `ShowFields { … }` literal (`grep -rn "ShowFields {" src`)
- Test: `tests/refs.rs`

**Interfaces:**
- Consumes: `Resolver::locate`, `locate_entry`, `read` and `abs`; `refs::location` and `refs::present`; `attachments::ledger`, `task_dir` and `entry_state`.
- Produces, in `src/output.rs`:
  - `pub struct InventoryRow { origin: Origin, required: bool, reference: String, location: RowLocation, status: RowStatus, handle: Option<String>, relation: Option<Relation>, kind: Option<String>, description: Option<String>, path: Option<String>, step: Option<String>, step_found: Option<bool>, cause: Option<String> }`. Every `Option` is skipped when `None`, and the enums serialize lowercase.
- Produces, in `src/inventory.rs`: `pub fn build(resolver: &Resolver, project: &Project, task: &Task, step_found: Option<bool>, fetch: Fetch, warnings: &mut Vec<String>) -> Result<Vec<InventoryRow>>`.

- [ ] **Step 1: Write the failing integration tests in `tests/refs.rs`**

```rust
#[test]
fn the_inventory_lists_everything_in_order_with_origins() {
    // spec docs/specs/x-design.md, plan docs/plans/x.md with step "Task 1: A",
    // entries: brief (input, required): docs/notes/b.md; ctx (context): https://e.org/x;
    // out (output, required): docs/out.md (absent); c (context): files:cited.png;
    // source "user:2026-10-19 chat"; attachments: cited.png and shot.png (both attached)
    let rows = env.json(&dir, &["show", &id])["inventory"].as_array().unwrap().clone();
    let summary: Vec<(String, String, bool)> = rows.iter().map(|r| (
        r["origin"].as_str().unwrap().into(), r["status"].as_str().unwrap().into(), r["required"].as_bool().unwrap())).collect();
    assert_eq!(summary, [
        ("spec".into(), "present".into(), true),
        ("plan".into(), "present".into(), true),
        ("ref".into(), "present".into(), true),      // brief
        ("ref".into(), "unchecked".into(), false),   // ctx
        ("ref".into(), "pending".into(), true),      // out
        ("ref".into(), "present".into(), false),     // c → files:cited.png
        ("source".into(), "unchecked".into(), false),
        ("attachment".into(), "present".into(), false), // shot.png only
    ]);
    assert_eq!(rows[1]["step"], "Task 1: A");
    assert_eq!(rows[1]["step_found"], true);
    assert_eq!(rows[6]["location"], "opaque");
    assert!(rows[0].get("handle").is_none());
    assert!(rows[2]["path"].as_str().unwrap().ends_with("docs/notes/b.md"));
}

#[test]
fn attachment_rows_come_from_the_ledger() {
    // attach a.png and b.png; delete b.png's stored bytes; drop an unrecorded file
    // stray.png into tasks/files/<id>/.
    // inventory attachment rows: a.png present, b.png missing; no row for stray.png;
    // show's warnings still carry the existing audit lines for b.png and stray.png.
}

#[test]
fn a_missing_stepped_plan_on_an_open_task_is_a_missing_row() {
    // open task with plan docs/plans/x.md and step "Task 1: A"; delete the plan file.
    // show succeeds; plan row status "missing", no step_found key in the row or at top level.
}

#[test]
fn an_entry_equal_to_the_spec_merges_once_targets_agree() {
    // Review Focus 4: add with s (output, kind=spec): docs/specs/y-design.md and body "[ref:s]";
    // inventory: one ref row, status pending. Write the file; edit --spec docs/specs/y-design.md.
    // inventory: first row origin spec, handle "s", relation "output", kind "spec", required true;
    // no other row has reference docs/specs/y-design.md; `edit --body "[ref:s] again"` succeeds.
}

#[test]
fn merge_keeps_separate_rows_when_targets_differ() {
    // worktree_with_excluded_docs: spec docs/specs/x-design.md exists only in main.
    // In side, create a directory at docs/specs/x-design.md and hand-write entry
    // d (context): docs/specs/x-design.md. show in side: a spec row (path under main) and a
    // separate ref row (path under side, status present), plus a warning containing "d"
    // and both paths.
}

#[test]
fn external_plan_rows_report_the_fetch_outcome() {
    // DocFixture with body "### Task 1: A"; plan stub:p, step "Task 1: A":
    // plan row status "available", step_found true. fixture.disconnect(): "unavailable" with a cause.
    // show --no-fetch: "unchecked" and no step_found.
    // external spec stub:s: always "unchecked".
}

#[test]
fn show_stays_readable_with_special_files_and_reports_them() {
    // With .timeout(Duration::from_secs(10)) on show:
    // (c) attach a.png and b.png, then replace both stored files with FIFOs; entry s (input): files:a.png.
    //     → show succeeds; the ref row for s and the attachment row for b.png both have status
    //     "special", with paths under tasks/files/<id>/; a symlinked attachment c.png instead
    //     gives status "missing" plus the existing attachment_unsafe warning line.
    // (d) worktree_with_excluded_docs: replace main's docs/specs/x-design.md with a FIFO; in side,
    //     the spec row has status "special" and a path under main (not side); add entry
    //     d (context): docs/specs/x-design.md → one merged row, handle "d", status "special",
    //     path under main.
    // (a) plan docs/plans/x.md with step, replaced by a FIFO, plus entry p (context): docs/plans/x.md
    //     → show succeeds; the plan row (merged, handle "p") has status "special", no step_found
    //     in the row or at top level; a warning names the path.
    // (b) entry o (output): docs/out-pipe where docs/out-pipe is a FIFO → its row has status "special"
    //     and a warning names it.
}

#[test]
fn pretty_show_has_a_references_section() {
    let out = env.pretty(&dir, &["show", &id]);
    assert!(out.contains("── references"));
    assert!(out.contains("read first: spec, plan, brief, out"));
    assert!(out.contains("- brief (input, required, kind=brief): docs/notes/b.md — the brief"));
    assert!(out.contains("- out (output, required): docs/out.md [pending]"));
}
```

Write the elided bodies in full.

- [ ] **Step 2: Run them to verify they fail**

Run: `just test-one --test refs`
Expected: the new tests FAIL (no `inventory` key; the missing-plan test fails with an `io` error).

- [ ] **Step 3: Keep `show` readable when a stepped local plan is missing or not a regular file**

In `describe`, the final `(Some(plan), Some(step))` arm calls `step_exists` only when `resolver.locate(plan)?` is `Here` or `Main`, which are regular files. When it is `Missing`:
- If something exists at the path `read` would open (`project.root.join(plan)`), or at the main checkout's copy when nothing is here, push the warning `plan <path> is not a regular file; its step is not checked`.
- Either way, `step_found` is `None`.

Never call `read` on that path. Any other error still propagates. `check` reports these cases (`doc_missing`, or `special_file` from Task 4).

- [ ] **Step 4: Implement `inventory::build`**

Rows come in spec §5.1 order.

- **Governing rows.**
  - A local spec or plan: `locate` `Here` or `Main` gives `present`, with that path. When `locate` is `Missing`, `locate_entry` decides:
    - `Special(path)` gives `special`, with that actual path (here or main) and a warning;
    - anything else gives `missing`, with `path = resolver.abs(..)`.
    
    The row keeps the path it examined, and the merge compares that path, never `abs()`'s fallback.
  - An external spec is `unchecked`.
  - An external plan with a step: when `fetch` is allowed, `resolver.read(DocKind::Plan, ..)`, which is cached since `describe` already read it, maps `Available` to `available` and `Unavailable` to `unavailable` with `cause = cause.detail()`. Under `FetchDisabled` it is `unchecked`.
  - An external plan without a step is `unchecked`.
  - The plan row carries `step` and the `step_found` passed in.
- **Merge** (spec §5.2). An entry merges into the spec row (the plan row only when the spec does not match) when its reference equals the link and the targets agree:
  - for a local path, both are found at the same path, both are special at the same actual path (the merged row is `special`, with that path), or both are missing;
  - for an external reference, the strings are equal.
  
  A merged row takes the entry's handle, relation, kind and description, and keeps `required: true`. When the targets disagree, the entry keeps its own row, and a warning goes to `warnings`: `reference <h> <reference> resolves to <entry path|missing>, but the <spec|plan> link resolves to <path|missing>`.
- **Entry rows.**
  - A local path or attachment maps `refs::present`:
    - `Here` or `Main` is `present`, with that path;
    - `Absent(path)` is `pending` for an output and `missing` otherwise, with that would-be path;
    - `Special(path)` is `special` for any relation, with that path and a warning;
    - `Unsafe { path, .. }` is `missing`, with that path, and the attachment audit's existing `attachment_unsafe` warning explains it.
  - Nothing here opens a target or propagates a special-file or unsafe-attachment error, so `show` never fails on one.
  - An external entry is `unchecked`, with no path.
- **Source row.** `location: opaque`, `status: unchecked`.
- **Attachment rows.** Iterate `attachments::ledger(task)` (a `BTreeMap`, so name order) for names whose latest entry is `Attached` and that no entry names. Each row has origin `attachment` and reference `files:<name>`, and its status and path come from the same attachment branch of `refs::present` as an entry's:
  - `Here` is `present`;
  - `Absent` is `missing`;
  - `Special` is `special`, with a warning;
  - `Unsafe` is `missing`.

  The existing `audit_task` warnings stay as they are.

Wire it into `describe` and every `ShowFields` literal.

- [ ] **Step 5: Implement the pretty section**

The `references` rule, then `read first: <labels>` when any row is required, then one line per row:
- `- <label> (<facets>): <reference>[ [<status>]][ — <description>]`.
- `<label>` is the handle, else the origin word (`spec`, `plan`, `source`), else the attachment name.
- `<facets>` are the relation (or the origin word for an unmerged governing row, `attachment` for an attachment row), then `required`, then `kind=<k>`.
- `[<status>]` is omitted for `present`.
- The plan row appends ` (step: <step>)` before the description.
- References are rendered with `refs::render_entry`'s quoting rule.

- [ ] **Step 6: Run the tests and the lint gate**

Run: `just test-one --test refs`, `just test-fast`, then `just check`.
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src tests
git commit -m "feat(show): one inventory of spec, plan, references, source and attachments"
```

---

### Task 7: The gates: completion, detach, move

**Complexity:** mid

**Files:**
- Modify: `src/refs.rs` (`require_outputs`)
- Modify: `src/commands/mod.rs` (`transition`)
- Modify: `src/commands/attach.rs` (`detach`)
- Modify: `src/moving/preflight.rs` (`validate_destination_docs`)
- Modify: `src/error.rs` (`RefOutputMissing` → `ref_output_missing`, `RefCitesAttachment` → `ref_cites_attachment`)
- Test: `tests/refs.rs`

**Interfaces:**
- Produces: `pub fn require_outputs(resolver: &Resolver, project: &Project, task: &Task) -> Result<()>`. For each `output` entry that is a local path or attachment, it maps `present`:
  - `Here` or `Main` passes;
  - `Absent` entries are collected into `Error::RefOutputMissing("<id> declares outputs that do not exist: <h> (<reference>), …; produce them, or remove each with `tasks edit <id> --rm-ref <handle>`")`;
  - `Special(path)` is the `SpecialFile` error;
  - `Unsafe` is `AttachmentUnsafe`.

- [ ] **Step 1: Write the failing integration tests**

```rust
#[test]
fn completion_refuses_missing_outputs_on_every_path() {
    // task with o (output): docs/out.md, started.
    assert_eq!(env.fail(&dir, &["done", &id, "x"]), "ref_output_missing");
    assert_eq!(env.fail(&dir, &["edit", &id, "--status", "done"]), "ref_output_missing");
    assert_eq!(env.fail(&dir, &["edit", &id, "--status", "done", "--force"]), "ref_output_missing");
    // the flagless editor changing `status: doing` to `status: done` → exit 1,
    // kind ref_output_missing, edit kept.
    // the same call removing it lands:
    env.json(&dir, &["edit", &id, "--status", "done", "--rm-ref", "o"]);
    // a produced output passes: second task, write docs/out.md, done succeeds.
    // a files: output passes after `tasks attach <id> <file> --name after.png`.
    // a FIFO output (mkfifo, .timeout 10s) → special_file.
    // drop never checks outputs.
}

#[test]
fn an_already_done_recurring_task_releases_its_claim_with_an_output_gone() {
    // Review Focus 5. Copy the setup of the tests/cli.rs test that reaches
    // "was already completed; releasing its claim" (grep for that text): an --every task
    // whose occurrence is done while its claim remains. Give it o (output): docs/out.md,
    // write the file, complete the occurrence, then delete docs/out.md. The second
    // `tasks done` succeeds with that warning, and its error is never ref_output_missing.
}

#[test]
fn detach_refuses_an_attachment_an_entry_names() {
    // attach shot.png; edit --ref "s (context): files:shot.png";
    assert_eq!(env.fail(&dir, &["detach", &id, "shot.png", "stale"]), "ref_cites_attachment");
}

#[test]
fn move_preflight_checks_input_paths_at_the_destination() {
    // two projects; task with a (input): docs/a.md existing only in the source → move refused,
    // message contains "--rm-ref"; an output or external entry does not block the move.
}
```

Write the elided bodies in full.

- [ ] **Step 2: Run to verify they fail**

Run: `just test-one --test refs`
Expected: the new tests FAIL.

- [ ] **Step 3: Implement the gates**

- **Completion.** In `transition`, compute `completing` (`to == Done && task.status != Done`) before the dependency checks. When `completing`, call `refs::require_outputs(&Resolver::new(&ctx.project, &ctx.registry), &ctx.project, task)?` before any stamp, whatever `force` is.
- **Detach.** In `detach`, after name validation, refuse with `RefCitesAttachment("<id> reference <h> names <name>; remove it first with `tasks edit <id> --rm-ref <h>`")` when an entry's reference is `files:<name>`.
- **Move.** In `validate_destination_docs`, for each `context` or `input` entry with a local path, call `locate_entry` on the destination resolver. `Missing` is `Validation("<source>: reference <h> <reference> does not exist in <prefix>; run `tasks edit <source> --rm-ref <h>` or create it there")`.

- [ ] **Step 4: Run the tests and the lint gate**

Run: `just test-one --test refs`, `just test-fast`, then `just check`.
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src tests
git commit -m "feat(refs): completion, detach and move honour references"
```

---

### Task 8: README and spec status

**Complexity:** low

**Files:**
- Modify: `README.md`, `docs/specs/2026-10-19-task-references-design.md` (Status line)

- [ ] **Step 1: Update the README**

Document the reference options next to `--spec` and `--plan`: the entry grammar, `[ref:<handle>]` citations, `doc <id> <handle>`, `list --ref`, the inventory in `show`, and the completion gate.

- [ ] **Step 2: Mark the spec implemented**

`Status: implemented (plan docs/plans/2026-10-19-task-references.md); skills activate after the host rollout`.

- [ ] **Step 3: Gate and commit**

Run: `just check`
Expected: PASS.

```bash
git add README.md docs/specs/2026-10-19-task-references-design.md
git commit -m "docs: task references in the README; spec implemented"
```

---

### Task 9: Skills (the activation commit)

**Complexity:** low

This is the branch's last commit and merges separately (Rollout steps 3-4). The installed skills are symlinks into main, so merging this commit is what tells agents to write references.

**Files:**
- Modify: `skills/tasks/SKILL.md`, `skills/scope/SKILL.md`, `skills/curate/SKILL.md`

- [ ] **Step 1: Update the skills**

- **`skills/tasks/SKILL.md`** (spec §10):
  - filing with `--ref` and citing with `[ref:<handle>]` (written in backticks when quoting the syntax), saying why each matters;
  - `required` in reading order;
  - declaring outputs;
  - reading the inventory's required rows from `tasks show` on pickup;
  - `doc <id> <handle>`;
  - `list --ref` before moving a document;
  - the kind vocabulary of spec §3.4;
  - the completion gate and its `--rm-ref` remedy.
- **`skills/scope/SKILL.md`**: a brief the pass writes becomes a reference on the tasks it scopes.
- **`skills/curate/SKILL.md`**: promoting a prose path to an entry, with the prose rewritten to cite it, is a bounded edit.

- [ ] **Step 2: Gate and commit**

Run: `just check`
Expected: PASS.

```bash
git add skills
git commit -m "docs(skills): file, cite and read task references"
```

---

## Rollout (after the branch is reviewed; operator steps, not implementer tasks)

1. **Merge the code without the skills.** Merge `task-references` into main up to Task 8's commit: `git merge --no-ff <Task 8 sha>` on main (personal profile, local merge). Then run `cargo install --path .` from main.
2. **Upgrade every other host** that runs `tasks` from that main commit before any record gains a `## References` section. Older binaries read the section as body text, but an older `edit --body` would drop it (spec §12). Confirm on each host with a `tasks show` of a task carrying a reference whose JSON has `inventory`.
3. **Sweep for stray citation text.** Run `grep -ln '\[ref:' "$root"/tasks/*.md` for each registered project root (`tasks projects`). Rewrite any open record whose citation-form text is not in backticks, then run `tasks check` there.
4. **Activate the skills.** Merge the rest of `task-references` (Task 9's commit) into main. The symlinked skills change at that moment. Run `just install-skills` only to confirm the links, since they already point into main.
5. **File follow-ups**, each with `concerns: tasks-3a445e extension — <one line>`, for the skills other projects own: flow (flows), quick-add (ops), lore's instruction corpus (lore).
