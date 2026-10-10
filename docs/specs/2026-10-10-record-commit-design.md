# Committing task records as tasks writes them

Status: draft (awaiting review)
Task: tasks-6939f6

## 1. Goal

Agents create and change task records through the CLI and rarely commit them. Every
project therefore collects uncommitted files under `tasks/`. On 2026-10-10 the ops main
checkout held 4 modified and 7 untracked records. The person sorts and commits them by
hand, and the next agent starts from a dirty tree whose changes it cannot attribute.
Records are vetted before anyone acts on them (scoping, `process`, spec and plan
reviews), so committing a record write needs no human gate.

The backlog has two sources, and both must be covered:

- **Own-session lifecycle writes** in the checkout the session stands in: `start`,
  `note`, `park` (ops-b46491, ops-0a6ba1 and ops-a0318e that day).
- **Writes routed by id prefix into another project's registered checkout.** All 7
  untracked ops records were `feedback` filed from sci, hq and ops sessions, and one
  modified record held a feedback recurrence note from vdocs. These land in a repository
  the writing session does not own.

Success:

- In a project that opts in, every record write the CLI makes leaves that write
  committed, or prints a typed warning naming why it is not.
- A dirty `tasks/` is an exception with a stated cause, and `tasks commit` clears it in
  one command.
- A person's staged or unstaged work outside the written records is never committed,
  unstaged, or modified.

Non-goals: pushing; committing anything the command did not write itself (except
`tasks commit`, §3.5); project administration commands (`init`, `rename`, `retire`,
`unretire`, `unregister`), which keep their own procedures and preconditions.

## 2. Approaches considered

- **A. The CLI commits after each write (chosen).** Every harness, every agent, and a
  person at a terminal get the same behaviour, with nothing to remember.
- **B. A batch `tasks commit` run from instructions or a session-end hook.** This is
  today's model plus a tool. It still depends on each harness wiring a hook and on agents
  following an instruction, which is the failure being fixed. B survives only as the
  recovery command in §3.5.
- **C. Commit only at lifecycle boundaries** (`add`, `start`, `park`, `done`). Notes
  and edits, a large share of the backlog, would stay dirty. Rejected.

## 3. Design

### 3.1 Opt-in

`tasks/.config.toml` gains:

```toml
[git]
commit = true
```

When the table or key is missing, commits are off and behaviour is unchanged. Any other
value or key in `[git]` is a config error, typed like the other tables. Opt-in per project
means work and external checkouts never begin committing unasked. Rejected alternative:
on by default in every git checkout.

For a linked project (`[checkout] path`), the key lives in the overlay's config and the
commit is made in the overlay repository, where the records live. That matches the
existing rule that the matching commit of `<prefix>/tasks/` is made in the overlay.

### 3.2 What a command commits

A write command collects the paths it wrote, grouped by project root:

- task records it created or rewrote, which can be more than one: `start --force
  --reason` past a halt also writes the reason onto the halt's record;
- attachment files it added or removed under `tasks/files/<id>/` (`attach`, `detach`);
- both records of a `move` (destination record, source tombstone), each in its own
  project root.

After the command's writes succeed, and before it prints its result, the CLI makes one
commit per root whose config opts in. The commit holds exactly those paths. Commands
covered: every command that writes a record, namely `add`, `edit`, `note`, `dep`, the
status commands (`start`, `park`, `done`, `drop`, `block`, `unblock`, `shelve`,
`unshelve`), `attach`, `detach`, `feedback` and `move`. A read command never commits.
Claim and park state outside git is not committed; only files under the project root are.

The message is conventional and names the verb and ids:
`chore(tasks): <verb> <id>[ <id>…]`, for example `chore(tasks): note tasks-6939f6`. For
`add` and `feedback` it is followed by the title, truncated to keep the subject at most 72
characters. The author and committer are the repository's own git configuration. The CLI
never adds a trailer.

### 3.3 Commit procedure and preconditions

For each root, with `LC_ALL=C` like the existing git calls:

1. **Skip with a warning** (the write stands, the record stays dirty) when:
   - the root is not in a git repository, or git is missing (`commit_unavailable`);
   - a merge, rebase, cherry-pick, revert or bisect is in progress, or HEAD is detached
     (`commit_deferred`, naming the state);
   - the index already holds staged changes to any path outside the command's own paths
     (`commit_deferred: staged changes`, naming up to five of those paths).
2. Otherwise `git add -A -- <paths>` (which covers new, changed and deleted files), then
   `git commit -m <message> -- <paths>`. Hooks run. The CLI never passes `--no-verify`.
3. If `index.lock` is held, retry with backoff for up to 2 s, then warn
   (`commit_failed: index locked`).
4. If the commit fails (a hook refused, or any other nonzero exit), unstage the paths
   step 2 staged (`git reset -q -- <paths>`), so the index is as it was. Then warn with
   `commit_failed`, the hook's or git's last stderr lines, and the paths.

Why the staged-changes precondition, rather than `git commit --only` with a temporary
index: hooks that regenerate and `git add` a derived file (ops's guide regeneration)
misbehave under a pathspec commit. The derived file reaches the commit while the
pre-regeneration copy stays staged in the real index (ops-89d93b). With an index that is
otherwise clean, a plain commit lets such a hook stage what it needs, and the result is
correct. A person's staged work is never swept in, because the CLI declines to commit
next to it.

Warnings use the command's existing JSON `warnings` array and its pretty stderr form.
Each one names the root, the paths, and the remedy: `tasks commit` after the cause is
cleared, or the record-commit routes in the skill for a content gate. The exit status
stays 0, because the write succeeded. A nonzero exit would invite a retry that duplicates
a note. This is not a silent fallback: every skipped commit is reported in the output, and
`prime` keeps listing uncommitted task files (tasks-80fec3).

### 3.4 Worktrees and branches

The commit lands on whatever branch the checkout holds:

- In a task's worktree, record commits ride the task branch and merge with it.
- `add` and `start` in the main checkout commit the record, which automatically
  satisfies the rule that a record is committed before `git worktree add` (tasks-5a46b9).
- `tasks done` commits the record by itself, on the branch. The protocol's "in the same
  commit as the code" becomes "on the task's branch before it merges". No code consumes
  same-commit pairing (checked 2026-10-10: tasks, obs). It is a convention in the tasks
  and scope skills only.

`stale_copy` and claim moves are unchanged. A write refused there writes nothing and
commits nothing.

### 3.5 `tasks commit`

`tasks commit [--project <prefix>]` commits every uncommitted file under the project's
`tasks/` in one commit, `chore(tasks): commit <n> records`, through the same procedure and
preconditions. It ignores the transient `.tmp` and `.edit.md` files, as
`uncommitted_task_files` does. It is the recovery after a `commit_deferred` or
`commit_failed` warning and the one-time sweep of an existing backlog. Unlike §3.2, it
commits files other sessions wrote. That is acceptable under §3.6, because every record
under `tasks/` is committable. It works whether or not `[git] commit` is set, since it is
an explicit request.

If `cli.toml` in ops governs subcommands, the new command lands there first, under the
ops rule for new CLI vocabulary.

### 3.6 Cross-project writes and feedback

A write routed to another project's root commits in that root under that project's
config, with the same preconditions. The writing session does not own that checkout, and
§3.3 is what makes this safe: only the written record enters the commit, and the commit
is skipped whenever anything else is staged there or an operation is in progress.

`feedback` is committed like any other write. Today the protocol uses "uncommitted" as
the owner's unreviewed queue ("read each, redact anything that describes a project
rather than the tool, then commit"). That marker moves to the queue the skill already
names: an `idea` tagged `feedback` with no triage note. Redaction becomes an `edit` of the
committed record, and git history keeps the original body. Writers are already told to
describe the tool and never the project, so redaction is a backstop, and these
repositories are the person's own. Rejected alternative: leave feedback uncommitted.
Feedback was 7 of the 11 dirty files in ops that day, so excluding it would leave most of
the backlog in place.

### 3.7 Concurrency and sync

Two sessions writing in one repository serialize on git's `index.lock` (§3.3 step 3). A
synced `.git` across hosts carries the same risks it does for manual commits today. This
design neither adds to them nor addresses them.

## 4. Instructions and rollout

Changes that land with the code:

- tasks skill: session protocol step 6 ("on the task's branch before it merges"); the
  "commit the task record before creating one with `git worktree add`" step, now automatic
  under `[git] commit`; the feedback sections (§3.6); and a line on the commit warnings
  and `tasks commit`.
- README: the `[git]` table, the warnings, and `tasks commit`.

Filed as pieces in other projects after this lands:

- lore: the global instructions' feedback paragraph ("lands as an uncommitted idea").
- ops: turn on `[git] commit` in the personal projects, run `tasks commit` once in each to
  clear its backlog, and confirm that ops's pre-commit guide regeneration commits
  correctly under §3.3. The interaction is ops-89d93b.

## 5. Testing

Integration tests against temporary git repositories (the pattern in
`src/locations.rs` tests), each with a scratch registry:

- config off: a write commits nothing;
- `add` of a new record, a `note`, and a `done` each produce one commit holding exactly
  the record, with the expected message;
- `attach` and `detach` commit the file changes with the record;
- `move` commits in both roots;
- another file staged: the commit is skipped with `commit_deferred`, and the index and
  the staged file are unchanged;
- unstaged edits to other files: the commit happens, and those edits stay unstaged;
- merge in progress, and detached HEAD: skipped with `commit_deferred`;
- a refusing pre-commit hook: `commit_failed` with the hook's output, the record written,
  and the index restored;
- a hook that stages a derived file: the commit includes it, and nothing stays staged
  afterwards;
- a held `index.lock`: retried, then `commit_failed`;
- a routed write into a second registered project commits there;
- `feedback` commits in the owner's root;
- a linked project commits in the overlay repository;
- a root outside git: `commit_unavailable`;
- `tasks commit` sweeps several dirty records into one commit and skips transient files.
