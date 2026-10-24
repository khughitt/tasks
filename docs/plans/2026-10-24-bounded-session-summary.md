# Bounded Session Summaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Recommended here: inline execution with executing-plans; the three steps are sequential, and the parser and summary share one contract.

**Goal:** Deliver opt-in bounded JSON prime without changing existing reads, then adopt it across the inventoried startup instructions only after the trial gate permits.

**Architecture:** Build prime through its existing scanner and selection pipeline, then apply a small summary transform to the completed PrimeOut. Keep all existing row types, serialize the real result to enforce the byte budget, and reuse the existing JSON renderer. Land binary delivery separately from changes to agent-loaded guidance.

**Tech Stack:** Existing Rust, clap, serde/serde_json, assert_cmd/tempfile; ops's TOML CLI inventory and existing Python validation. No new dependencies.

**Spec:** `docs/specs/2026-10-24-bounded-session-summary-design.md`, frozen at `76501bdd333e5c6b49758fdec1b6662b5c57fa6f`, accepted for planning by the user after spec round 3. That acceptance includes two implementation details specified below; the spec stays frozen.

**Status:** draft for user review. No implementation has started. Design task: tasks-e7424b; parent effort: tasks-96c00c. A plan approval authorizes the repository work, not a live shared-tool or instruction rollout.

## Global Constraints

- `tasks --json prime --summary`; `--summary` is a boolean on prime. No default change, --full option, configurable budget, new command, cache or compact task-row schema.
- Hard limit: **12,288 stdout bytes including the trailing LF**, after JSON escaping and UTF-8 encoding. This is not a universal token guarantee. Target consumer: the observed 6,000-token tool response; the 3–4 bytes/token estimate and 1,000-token framing reserve require the real consumer pilot in Task 3.
- Initial caps: parked 3, doing 5, ready 5, closeout 3, lanes 5, roadmap 5 roots with no descendants. Evict tails in order: roadmap, lanes, closeout, ready, parked, doing. Keep section prefixes, not a selection of rows that happen to fit.
- Counts/periodic/deferred/prefix/group/projects/warnings/halts stay complete. Included rows stay exact, including whole lane rows. Roadmap roots retain child_count and open_descendant_count. Do not clip text, paths or instructions.
- Summary metadata follows spec §4 exactly: byte_limit, complete, sections with total/shown/omitted/first_omitted, roadmap descendants_omitted, and full argv. The summary key is absent without the flag.
- Preserve selection/order, cross-lane holds, claim/copy precedence, sparse fields, existing errors and existing full/pretty output. Malformed excluded records still fail. No product code or new test runner is added during planning.
- New option rows land in ops first; tools/cli.toml is adopted through the vendoring tool with the parser change, never edited or copied by hand.
- Tests run only through each project's just front door. Reuse TestEnv's key-by-key isolated subprocess environment and git_fixture helpers; never inherit credentials, HOME or GIT_* handles into a new fixture.
- Task 1/2 delivery is independent of the trial gate. Task 3 guidance/consumer adoption waits until the 2027-02-16 readout or the user's specific no-effect verdict as flow-trial-1 owner. No skill mention, hook change or new link that exposes summary guidance early.

## Review Focus

1. Metadata alone exceeds the cap: preserve warnings and halts, return summary_too_large with no stdout, and keep stderr bounded. Task 2 unit/CLI assertions and Task 3 recovery exercise.
2. The running executable has been replaced, or its path is non-UTF-8: full argv must fail with typed io rather than name an unusable path or a different PATH binary. Task 2 path-helper tests, including Linux's missing path ending in ` (deleted)`.
3. A lane has huge guidance/active/held arrays: drop it whole, keep full-view decisions on retained lanes, and disclose omission. Task 2 unit/CLI assertions.
4. A local -C path contains spaces and PATH selects another tasks: full argv must retain the current executable, effective directory, original scope and repeated --without flags. Task 2 CLI assertions.
5. README text is reachable through an agent skill link: keep summary guidance in its own top-level section outside every existing skill-linked section. Task 2 manual document-boundary check; Task 3 adds the first skill link only after its gate.

## Files and responsibilities

| File | Responsibility |
|---|---|
| ops:cli.toml | Source option row; ops:tests/test_cli.py already validates inventory shape |
| src/cli.rs; src/main.rs | Parse the flag; reject resolved pretty format before project scan |
| src/commands/mod.rs; src/commands/list.rs | Preserve invocation context and run the transform after existing prime construction |
| src/prime_summary.rs (new); src/main.rs module declaration | Bounded transform and full-argv construction/validation, with focused unit tests |
| src/output.rs | Optional PrimeSummary and section-metadata types; existing row and renderer types stay |
| src/error.rs | Typed summary errors and existing io handling |
| tools/cli.toml | Adopted inventory, committed together with parser change |
| tests/cli.rs | End-to-end summary, compatibility and recovery assertions using tests/common |
| README.md | Isolated top-level Bounded session summaries section immediately before License |
| skills/tasks/SKILL.md; AGENTS.md | Deferred startup adoption, omission handling and explicit overflow recovery in this repository |
| README.md existing onboarding/adoption sections | Deferred alignment of startup snippets, separate from Task 2's isolated feature section |
| Other project startup guides | Deferred owner tasks listed in Task 3; edit each owner's authoritative source, not an unreviewed shared copy |

## Tracking, setup and review gates

Follow this repository's design-task closeout convention (also used by its task-references plan). While this plan is under review, link it as an output reference on tasks-e7424b; implementation records are created after approval, so their open work does not prevent closing the design task.

After approval, the controller files a separate implementation goal under tasks-96c00c with this plan and spec, source tasks-e7424b, P2/m/high, process direct. Add one direct child per numbered heading below, with this plan and the exact step string; complexity is given on each heading. Task 2 depends on Task 1. Also file a named post-merge reinstall task under the goal, dependent on Task 2, for its Step 7 host handoff. Task 3 depends on Task 2 and that verified reinstall, and is blocked with its explicit trial-release condition. File Task 3's other-project owner tasks at this tracking stage with the same blocked condition, before code execution; their open state must not block Tasks 1–2 or reinstall. Task 1's tasks record coordinates an ordinary ops task for the source-table edit; record that dependency and ops id, because cross-project parent links are invalid. Start/arm the actual ops task before editing there. Do not start implementation while its approval or trial gate is closed; read-only inventory and gated record filing do not activate guidance.

Then update the CLI-consumer brief with the approved contract and implementation goal id, wake tasks-4ca7bd with --no-reviewed as its design task requires, and close tasks-e7424b on this branch before integration. Before closing, transfer each retained worktree/scratch/evidence item to the open implementation goal: write and verify its receiving kept record before releasing the design task's ownership record. Use the current retained-item reference and fresh hashes; a merge/cleanup trigger grants no deletion permission. This is document delivery, not a claim that the oversized startup default has changed.

Reuse `.worktrees/tasks-e7424b` for execution if it is still the authoritative workspace; otherwise create the implementation goal's isolated worktree under .worktrees following current project instructions. Existing hydration installed a confined binary under target/setup. Before the first product-code change, execution re-enters the worktree skill and runs **just test** as the deferred full baseline; note the command and actual test count on the implementation task. Preserve the approved document revisions and verify the canonical measurement hash before reusing it. In a new ops worktree, run work-link, git worktree add/lock, **just setup**, then its **just test** full baseline before edits.

Every implementation task follows RED→GREEN where behavior changes, then just test-fast, tasks check and normal pre-commit hooks. After final branch review, handle Critical/Important findings in the required corrective rounds and record deferred minors. Inline execution is recommended; use a fresh whole-branch reviewer as executing-plans requires. Do not use this planning task's prior test results as implementation evidence. Pushes, PRs, shared binary installation and live instruction activation remain separately authorized external actions.

---

### Task 1: Land the source CLI option in ops

**Complexity:** low. **Deliverable:** a landed, validated source-table row that tasks can adopt. This is separate because ops owns and reviews its CLI inventory.

**Files:** modify ops:cli.toml; use existing ops:tests/test_cli.py validation. Resolve the registered ops root through tasks projects/root rather than a machine-specific path.

**Interfaces:** consumes the approved boolean flag contract; produces `{ names = ["--summary"], value = "none" }` on the tasks prime command in ops main. No other command gains the flag; implied global JSON/pretty flags need no new rows.

- [ ] **Step 1 — Prepare the claimed ops task/worktree and baseline.** Follow Tracking above and ops's current guide. Read its profile before any local integration; record the baseline count.
- [ ] **Step 2 — Add the one row.** Place it beside --closed in the tasks prime options. Inspect the diff: source table only, apart from this work's task records or required generated guide updates. No new custom test is needed for this reversible declarative row; reuse the inventory validator.
- [ ] **Step 3 — Verify.** Run `just test-one tests.test_cli.CliTable.test_file_is_valid`, then `just test-fast` and `tasks check` in ops. Expect zero validation findings and passing tests; record counts. Commit with `feat(cli): declare bounded tasks prime summaries`.
- [ ] **Step 4 — Review and land locally under ops's profile.** Record the landed commit on both the ops task and tasks coordination record. Do not push or publish a table globally. Close the ops work before releasing its claim; close the coordination step only once the source row is available on ops main for adoption.

### Task 2: Deliver the bounded prime binary and isolated README section

**Complexity:** high. **Deliverable:** working opt-in CLI with complete recovery metadata and unchanged ordinary output. Keep the transform, first production caller, parser/adopted table, tests and documentation together; unused helpers cannot land separately under this binary crate's clippy gate.

**Files:** create src/prime_summary.rs; modify src/main.rs, src/cli.rs, src/commands/mod.rs, src/commands/list.rs, src/output.rs, src/error.rs, tests/cli.rs and README.md; adopt tools/cli.toml. No changes to skills, host pointers or hooks.

**Interfaces:**

- `Command::Prime` adds `summary: bool`. Main checks that flag against its already-resolved Format before commands::run; no new format argument propagates through every command.
- `prime_summary::full_argv(executable: &Path, directory: &Path, scope: &ScopeArgs, closed: bool, without: &[String]) -> Result<Vec<String>>` validates executable metadata as a regular existing file and both paths as UTF-8, and constructs spec §4's argv. Get executable from current_exe only on the summary path. Use `std::path::absolute(start_dir(cli.dir)?)?` for the recovery directory without resolving symlinks or changing process cwd; failure is typed io. Preserve option values as separate argv elements; no shell quoting or lossy conversions. An explicit .worktrees spelling in -C stays in recovery rather than becoming its storage target; the executable still uses current_exe as the spec requires.
- `list::prime(ctx: ReadCtx, closed: bool, without: Vec<String>, full: Option<Vec<String>>) -> Result<Output>` replaces its current three-argument signature; its only production caller is commands/mod.rs. None selects ordinary prime; Some requests summary after normal construction.
- `prime_summary::bound(out: &mut PrimeOut, full: Vec<String>) -> Result<()>` applies caps and byte evictions after full construction and warnings, before returning Output::Prime. It does not scan, select, paint or write stdout.
- `PrimeOut.summary: Option<PrimeSummary>` uses skip_serializing_if Option::is_none. `PrimeSummary` has byte_limit: usize, complete: bool, sections: BTreeMap<&'static str, PrimeSection>, full: Vec<String>. `PrimeSection` has total/shown/omitted: usize, optional first_omitted: String and optional descendants_omitted: usize (present only for roadmap). All six map keys are always populated.
- Add Error::SummaryRequiresJson and Error::SummaryTooLarge with spec §3/§5's exact static details and kind strings; update kind() and exhaustive with_suffix handling. Existing Error::Io covers invalid/unavailable paths.

- [ ] **Step 1 — Run the deferred full baseline, then write CLI RED tests.** Reuse TestEnv and err_kind/err_detail in tests/cli.rs. Name the new tests with `prime_summary_` so `just test-one --test cli prime_summary_` selects them. Start with empty_project and caps_and_recovery: empty arrays and zero counts, complete true for the empty case; 80 eligible records for the capped case, exact prefixes/omissions/full argv. Assertions include:

```rust
assert!(out.status.success());
assert!(out.stdout.len() <= 12_288); // includes the binary's trailing LF
assert_eq!(json["summary"]["byte_limit"], 12_288);
assert_eq!(json["summary"]["sections"]["ready"]["total"], 80);
assert_eq!(json["summary"]["sections"]["ready"]["shown"], 5);
assert_eq!(json["summary"]["sections"]["ready"]["omitted"], 75);
assert_eq!(json["ready"].as_array().unwrap().as_slice(), &full["ready"].as_array().unwrap()[..5]);
assert_eq!(json["summary"]["sections"]["ready"]["first_omitted"], full["ready"][5]["id"]);
assert!(full.get("summary").is_none());
```

Run the focused recipe and observe failure on unsupported --summary before changing production code. Use compact fixture rows so five ready rows fit. Do not add a fake-clock feature: pure fixtures use fixed values, and CLI date/defer/recurrence cases use dates safely distant from thresholds plus the existing fixture patterns.

- [ ] **Step 2 — Adopt the source table and wire one complete implementation.** After Task 1 is on ops main, run `python3 "$(tasks --pretty root ops-88a0fa)/bin/vendored" adopt cli.toml` here. Add the flag/module/errors/types and the exact interfaces above. Build full argv only for summary, preserving scope and explicit repeated --without; a valid TASKS_FORMAT=pretty is overridden by --json, otherwise summary pretty fails before scanning. Complete normal prime first, then call bound; ordinary prime gets summary None and performs no new recovery-path I/O.

Inside bound, retain full array counts and the original ids needed for first_omitted, count all non-root roadmap occurrences, clear root children, apply caps, populate metadata, and serialize the entire candidate with serde_json. While serialized length plus one LF exceeds 12_288, pop a tail in the fixed eviction order and refresh metadata. Exhausted arrays with required metadata still oversized return SummaryTooLarge. Serialization uses the existing infallible-output convention. No text clipping, warning deduplication, lane rebuilding or fallback to full stdout.

- [ ] **Step 3 — Prove byte, path and eviction edges in focused unit cycles.** Put unit tests beside bound/full_argv in src/prime_summary.rs, using one test-only minimal PrimeOut fixture with explicit fixed values. For each new case, prove its assertion fails before its fix, then passes. Run `just test-one prime_summary::tests`.

| Test | Concrete assertions |
|---|---|
| exact_byte_boundary | Pad one ASCII warning in an otherwise empty fixture to exactly 12_288 serialized bytes including LF: success; add one byte with no removable rows: summary_too_large; error detail contains neither padding nor paths |
| eviction_refreshes_metadata | Force pruning with escaped/multibyte titles; every returned row equals its original, arrays remain prefixes, shown+omitted=total, first_omitted tracks the new boundary; verify roadmap→lanes→closeout→ready→parked→doing order across successive fixtures |
| shallow_roadmap_counts | Fixed nested forest: count every non-root occurrence including children of excluded roots; no returned children, unchanged child_count/open_descendant_count; complete false even when all roots fit but descendants were removed |
| oversized_lane_is_atomic | Lane larger than budget through guidance/active/held data: omit whole lane, preserve retained lanes' already-computed picks/causes/holds, correct lane.id omission handle |
| required_metadata_overflow | Independently oversize warnings, halts, projects/full argv and malformed parked-id omission metadata; each returns the same typed bounded error after arrays empty; full fixture values are not clipped |
| recovery_paths | Existing temporary executable and cwd with spaces succeed; missing executable, a synthetic Linux path ending ` (deleted)` that does not exist, non-file executable, and Unix non-UTF-8 executable/directory return io; never strip the suffix or substitute PATH |

The synthetic deleted-path case is the regression for spec round 3 Minor 2. It exercises the same validation without racing cargo install or replacing a real executable. A later replacement after validation remains an ordinary failed fresh recovery read, not a snapshot guarantee.

- [ ] **Step 4 — Extend CLI RED→GREEN coverage for the remaining acceptance cases.** Reuse existing prime fixtures in tests/cli.rs rather than build another scanner or generic harness. Run `just test-one --test cli prime_summary_` after each group.

| Test name suffix | Setup and required assertion |
|---|---|
| format_and_errors | --json beats valid pretty env; resolved pretty summary gives summary_requires_json, exit 1, empty stdout and exact detail; invalid env, unknown group and malformed excluded record retain existing typed errors |
| scopes_and_recovery | Local worktree with spaces reached through a symlink and relative -C, --project, --group (including all unreachable) and --all-projects; full argv uses absolute current executable despite a different tasks on PATH, directory equals std::path::absolute of the supplied -C spelling (not canonicalize), preserves scope/--closed/repeated --without; execute its argv and obtain ordinary full JSON |
| claims_and_parks | More than five doing/three parked rows, claimed-only-elsewhere task, stale local copy, unverifiable claim and unresolved park; compare sampled row values to full prime, exact totals, no ownership fallback |
| lane_states_and_selection | At least seven lanes spanning all five states and contested need; compare full-view lane decisions before sampling; cutoff, unavailable needs, paused lane, future defer, due recurrence and halted project yield the same eligible prefixes as full prime |
| warnings_and_closeout | Unreachable member, stale claims and halt rows remain exact; more than three eligible closeout goals disclose omitted goals, and executing full recovery offers all of them |
| large_roadmap | At least 200 eligible leaves in a multilevel forest; at most five roots/no children, exact descendant/root omissions, valid JSON <=12_288 bytes |
| overflow_and_compatibility | Generate enough real warnings for summary_too_large: exit 1, stdout empty, static stderr detail; the same full invocation succeeds. Retain all preexisting full JSON/pretty tests; explicitly check --closed still changes no JSON counts |

Keep fixtures stable between full/summary reads: no actual claim-owner termination, registry mutation or deadline transition during comparisons. Compare semantic rows and warnings, not freshly generated process ids. Path helper unit tests cover unavailable paths without adding a test-only CLI switch.

- [ ] **Step 5 — Document only in the isolated README section.** Add `## Bounded session summaries` immediately before License, after the Reference section ends. Put all new --summary commands, caps, omission/full/closeout recovery and overflow behavior there. Leave quick start, command tour, existing skill-linked sections and all skills unchanged; add no link into this section from those sections. Manually enumerate every README target in skills/ and confirm neither its section nor any linked containing section includes the new guidance. This addresses spec round 3 Minor 1 without adding a documentation parser/test framework.
- [ ] **Step 6 — Verify, rebuild confined, review and commit delivery.** Run `just test-one parser_surface_equals_table`, `just test-fast`, `just check`, `tasks check`, and `git diff --check`; expect all green with no parser/table mismatch or unlinked implementation steps. Build `CARGO_TARGET_DIR="$PWD/target" cargo install --locked --path . --root "$PWD/target/setup"`. Drive the explicit target/setup/bin/tasks for smoke/recovery and any subsequent tracker writes in this worktree; never repoint host launchers. Verify only the isolated README section changes and no skill/hook/project-startup guidance is included. Commit adopted table with parser/feature as `feat: add bounded JSON prime summaries`. Run the required final review before integration. Record tested executable/revision and counts. The confined install verifies this worktree; it does not satisfy the post-merge shared reinstall below.
- [ ] **Step 7 — Hand off and complete the required post-merge reinstall.** Before closing Task 2's code record on its branch, name the open reinstall task from Tracking and record its exact next action. This keeps code completion before merge, as the repository requires, while the host action has an open owner. After the reviewed binary change merges locally, the reinstall task resolves `command -v tasks` and its symlink target, and inspects the Cargo install destination before proposing a command. Prepare the documented `cargo install --locked --path .` from the registered tasks main checkout, with an explicit --root only if needed to reach the verified existing install destination. Present the merged commit, install destination and concrete command, and ask before replacing the shared PATH binary, as the host-action rule requires. After approval, install, verify PATH resolves to the expected new binary, and check that its `prime --help` exposes --summary and that a bounded read in the unenrolled tasks project succeeds. Record the installed path/revision and result on both records. If approval is pending or declined, park the open reinstall task, explicitly report merged-but-not-reinstalled status and that the README describes an option absent from the installed CLI, and leave delivery incomplete on the goal. This is the repository's required reinstall, not an optional follow-up; it does not wait on Task 3's trial gate and does not activate agent guidance.

### Task 3: Adopt bounded reads in agent guidance after the trial gate

**Complexity:** mid. **Deliverable:** aligned startup instructions across this repository and the inventoried active project guides, with owner-task and consumer-pilot evidence, released only when permitted. This task may remain blocked after Task 2 lands; binary delivery and reinstall must not wait on it. Updating the skill alone is not adoption completion.

**Files:** modify skills/tasks/SKILL.md, this repository's AGENTS.md, and README.md's existing startup snippets after the gate. Other guides change only through their owner tasks below. Pilot results are task notes/individual attachments, not executable examples added to a reference document. No new hook or generic consumer adapter is planned: the inspected ops SessionStart hook checks halts but does not call prime.

**Interfaces:** consumes Task 2's installed argv/JSON/error contract and a recorded trial release. Produces startup instructions using `tasks --json prime --summary`, explicit full-read and closeout recovery, and visible failure recovery. The CLI default remains full. Preserve owners' other task-selection and claim rules. Cross-project parent links are invalid: Task 3 records each owner task as a dependency and records its approved revision, affected source and release evidence.

**Instruction inventory, checked 2026-10-24:** registered checkouts were read through tasks projects. The eleven top-level guides in the review are fam, forge, forge2, material, mm, prism, relay, sun, tasks, tui and wali. The registry-wide read adds active mind6 and nrp guides. File every other-project owner task below at plan-approved tracking time, with `source` pointing to this implementation effort and a body naming this plan's frozen revision, target source, bounded command/recovery contract and gate. Run tasks block on each: `Await flow-trial-1 readout (2027-02-16) or the user's no-effect verdict for this precise startup-guidance change`. Filing and inventory may happen before readout; source edits and activation may not. Read each owner's profile and canonical generation route before implementation; a work/external profile's integration remains its owner's decision.

| Owner | Startup surface and observed wording | Tracking/action |
|---|---|---|
| tasks | AGENTS.md: Start with tasks prime; skills/tasks/SKILL.md session protocol; README.md Quick start / Tell the project's agents and Adopting in an existing project startup snippets | Task 3 owns these gated edits here |
| fam | AGENTS.md: Run tasks prime at the start of a work session | Separate fam owner task, same gate |
| forge | AGENTS.md: Run tasks prime at the start of a work session | Separate forge owner task, same gate |
| forge2 | AGENTS.md: Run tasks prime at the start of a work session | Separate forge2 owner task, same gate |
| material | AGENTS.md: tasks prime, then tasks start before changing anything | Separate material owner task, same gate |
| mind6 | AGENTS.md: Run tasks prime at the start of a work session | Separate mind6 owner task, same gate |
| mm | AGENTS.md: tasks prime, then tasks start before changing anything | Separate mm owner task, same gate |
| nrp | AGENTS.md: Run tasks prime at session start | Separate nrp owner task, same gate and owner-profile integration rules |
| prism | AGENTS.md: Run tasks prime at the start of a work session | Separate prism owner task, same gate |
| relay | AGENTS.md: Follow the tasks skill: tasks prime | Separate relay owner task, same gate |
| sun | AGENTS.md: tasks prime, then tasks start before changing anything | Separate sun owner task, same gate |
| tui | AGENTS.md: Start with tasks prime | Separate tui owner task, same gate |
| wali | AGENTS.md: Run tasks prime at the start of a work session | Separate wali owner task, same gate |
| lore | instructions/fragments/tests.md and its assembled instructions/AGENTS.md: prime/prime's session-start message names an open halt | Descriptive halt wording, not a required full startup read: leave unchanged; no lore owner task is needed for this wording |
| mind3 | Retired project's AGENTS.md still names tasks prime | Excluded: retired to mind6; do not file rollout work in a retired project |

No root AGENTS.md was present for ns in this read. Other reachable registered root guides did not match a prime instruction; this is a bounded inventory, not a claim about every unregistered checkout or nested harness rule. A live re-inventory below is required before adoption. Literal full-read examples describing recovery or historical behavior are not startup prescriptions and need not be replaced.

- [ ] **Step 1 — Re-inventory instruction surfaces and verify release before source edits.** Re-read all registered project guides, harness aliases and declared generators, the tasks skill, README startup prescriptions and lore's halt wording. Attach the actual match inventory and each owner task id to Task 3; add newly found active surfaces to their own gated owner tasks, and record a reason for every excluded match. Confirm every nonlocal task carries the same blocked gate condition. Read flow-trial-1's actual readout or the user's no-effect verdict naming these precise instruction changes. A date passing alone is not a readout. Without that evidence, keep all source-edit steps blocked; do not modify any startup guide or add a summary link. A no-effect verdict covering only the skill does not release other owners' guides. If lore's wording later requires an actual startup-prescription change, file a lore owner task under the same gate; do not hand-edit assembled instructions or fixtures.
- [ ] **Step 2 — Run a bounded pilot before adoption.** Through the actual harness tool with max_output_tokens 6000, run the explicit confined binary on representative data and inspect whether framing plus stdout is truncated. Exercise one summary_too_large case end to end in an isolated fixture: retain original argv/context, rerun full through the same executable with stdout captured to a persistent caller-owned file, emit a visible failure/path notice, then read warnings/halts and needed sections in bounded pieces. If full capture fails, keep an explicit nonzero failure. Do not retry other summary errors. Record stdout bytes, observed truncation and any actual token accounting supplied; otherwise label the token count unknown. Stop adoption on a failed pilot. No live desktop or shared launcher changes are needed.
- [ ] **Step 3 — Update this repository's startup prescriptions and coordinate owner edits.** In the skill and AGENTS.md replace the required startup read with explicit JSON summary; align the README onboarding/adoption startup snippets, and link the isolated feature section only now. State that displayed arrays are samples, require full doing/parks before parallel scheduling or concluding no resume exists, and recover omitted closeout invitations exclusively from full prime. Preserve cutoff selection through ready/next, warnings/halts, start's claim authority, and visible summary_too_large file recovery from spec §8. State that full argv is an argument array, not a shell command; preserve cwd/scope/environment. Each owner task makes the same bounded-startup change in its own authoritative guide or generator, under its own review/integration rules, with recovery guidance linked or included. Do not edit generated guides in place or treat the skill change as covering unchanged project instructions.
- [ ] **Step 4 — Verify and review guidance coverage.** Manually check successful and overflow examples against pilot output; `just test-fast`, `just check`, `tasks check` and `git diff --check` pass here, and each owner runs its declared documentation checks. Commit local changes as `docs: adopt bounded prime in agent startup instructions`, recording release and pilot evidence. Obtain required independent reviews. Compare the live instruction inventory with owner-task dispositions: every active startup prescription is changed and verified or explicitly remains open with its owner and blocker; describe partial adoption as partial. Task 3 cannot close with an unhandled project guide or an open owner dependency. This does not silently waive Critical/Important corrective rounds.
- [ ] **Step 5 — Request approval for actual shared activation.** Before merging skill changes into a checkout whose installed skills link there, or activating project instructions on a live shared surface, present the concrete rollout and get authorization at that moment. Respect each owner's profile and ask before any newly discovered shared hook/binary repoint; Task 2 Step 7 owns the initial binary reinstall. Verify one host/profile end to end for a shared configuration path; a distinct harness/configuration path needs its own check. Close only with release/pilot/activation evidence and settled owner dependencies, or park on the actual gate. Leave no background process, host pointer or discarded evidence behind.

## Plan self-review and handoff

Before freezing, check every spec §7 acceptance row against Task 2's unit/CLI table, and spec §8's adoption requirements against Task 3. Verify all signatures and metadata names agree; no helper lands unused; keep format resolution in main and scope/copy logic in its existing engine. Confirm both round 3 minors are assigned and that this plan changes no default or trial-visible skill now.

After user plan approval, the controller creates the implementation goal/steps as above and closes this design task with both approved frozen revisions and the implementation goal id. The recommended execution method is inline with a fresh final reviewer; the written plan remains the gate. Default migration and any new automated startup consumer are separate future scope, not implicit tasks here.
