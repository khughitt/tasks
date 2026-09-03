# Upstream feedback as tasks — design

**Status:** approved (2026-09-03), not yet implemented; implements tasks-059b2f.

## 1. Problem

Projects that use `tasks` discover friction, gaps, and good surprises about the tool
itself in the middle of a session. Today those observations survive only if someone
carries them by hand into this repository; most die in chat. The recent fixes (extra spec
roots, configurable doc roots, the hierarchy design) each reached this repo through a
human relaying an agent's complaint after the fact.

## 2. Decision

Feedback is filed as a task in this repository, from wherever the reporter is, by one
command. There is no separate feedback store: a task already has an id, a status, notes,
tags, and cross-project resolution through the registry, and a per-machine store would
rebuild all of that and then die with the machine.

Consequences the design accepts:

- **Triage is the existing idea flow.** A filed entry is an `idea`. Ideas never appear in
  `ready`, and the skill already says an idea must be scoped before it is picked. Triage
  therefore means `edit --status todo` with a priority and size, `drop` with a reason, or a
  design session for anything large. No triage subcommand.
- **Recurrence is a note plus tags.** A second report of the same thing appends a note to
  the open entry and adds the reporter's `from:` and category tags if absent, instead of
  creating a duplicate. The notes are the recurrence record; the tags keep every
  reporting project and category queryable.
- **The reporting project is a tag.** `from:<prefix>` is free, since the registry already
  knows it, and `list --tag from:<prefix>` answers "what did that project hit".
- **A human commit is the disclosure gate.** This repository is public, and the summary
  and body are agent-written text crossing from a possibly private project. The command
  therefore never commits: an entry lands as an uncommitted file in the registered
  checkout of this repository, and it becomes public only when a person here reviews it,
  redacts if needed, and commits it. The skill tells reporters to describe the tool and
  not the project, but that instruction is hygiene; the commit is the control. A private
  fork registered under the target prefix is the alternative for projects that want no
  content, however scrubbed, to reach a public tree.

## 3. Command

```
tasks feedback <summary> --category friction|gap|idea|positive [-b|--body TEXT]
               [--recur ID | --new]
    File feedback about the tasks tool itself into the upstream tasks project.
```

Behaviour, in order:

1. **Locate the reporter.** The current project is found as for every other command
   (nearest `tasks/.config.toml`, or `-C`). Its prefix becomes the `from:` tag. Outside a
   project the command fails with `no_project`, as any command would; feedback with no
   provenance is not accepted.
2. **Locate the target.** The registry entry whose prefix is `tasks` is the target. If
   there is no such entry, or its root has no `tasks/.config.toml`, the command fails
   with `config` and a message saying to clone the upstream repository and run
   `tasks init` there. There is no configuration key for the target; the prefix is the
   contract. Filing from inside the target itself is allowed and yields `from:tasks`.
3. **Look for an open match** (§4) unless `--new` is given. An exact match recurs. Similar
   but inexact candidates are never merged automatically: the command fails with
   `ambiguous`, listing the candidate ids and titles, and the reporter reruns with
   `--recur ID` or `--new`. `--recur ID` names the match explicitly and skips the search;
   the id must be an open task in the target tagged `feedback`, else `validation`.
4. **Recur or create.**
   - Match: append a note reading `feedback from <prefix>: <summary>` and, if `--body` was
     given, a second note `detail from <prefix>: <text>` (single line, as all notes are;
     multi-line detail is rejected with `validation` on recurrence). Add `from:<prefix>`
     and `<category>` to the tags if absent. The task's `updated` is refreshed, so it
     rises in `list`; timestamps have second precision, so a repeat within the same
     second keeps the same value. The write is a guarded read-modify-write: the file's hash is taken on read
     and checked again immediately before the atomic replace; on a mismatch the whole
     step is retried from the read, up to eight times, then fails with
     `concurrent_modification`. Every read also re-checks that the task is still open
     feedback and, for an automatic match, still carries the matched title; a task that
     was closed, retagged, or renamed since the match fails with `validation` and is not
     touched. The remaining window between check and rename is the same one every other
     write command in the tool has; the guard closes the realistic case of two reporters
     landing seconds apart.
   - No match: add a task in the target with title `<summary>`, status `idea`, priority
     2, no size, no owner, body `<text>`, tags `feedback`, `<category>`, `from:<prefix>`.
     Creation links the file into place exclusively and regenerates the id on a
     collision, so two creators drawing the same id cannot overwrite each other; the same
     primitive backs `add`. A summary with no token of three or more characters (§4) is
     rejected with `validation`, since it could match nothing meaningfully.
5. **Report.** The output names what happened, because the reporter should know whether it
   started something or joined something.

The reporter never sets priority or size; those are triage decisions made here. The
reporter also never commits (§2); the output carries a warning naming the uncommitted
file and its checkout.

**Where the inbox is.** The registry maps the `tasks` prefix to one checkout, and the
entry lands there. Worktrees of the same repository do not see it until it is committed.
This repository's own protocol runs `tasks prime` in the registered checkout before any
worktree is created, and `prime` gains a warning listing uncommitted files under
`tasks/`, project-relative, including the config file and excluding only transient
temp files. The list comes from `git status --porcelain`. It is skipped in exactly two
cases, stated here so neither is a silent fallback: git reports the root is not inside a
repository (git's own discovery, which also covers an unreadable HEAD), or there is no
git executable. Any other git failure is an error. That warning is useful on its own,
since `done` in the same commit as the code is a rule that is easy to break, and it is
what makes an unfiled report hard to miss.

## 4. Matching

Candidates are the target's open tasks tagged `feedback`. Titles are normalized to
lowercase ASCII-alphanumeric tokens of three or more characters, in order.

- **Exact:** identical normalized token sequences. This recurs automatically.
- **Similar:** Jaccard similarity of the token sets at least 0.6 but not exact. These are
  advisory only and produce the `ambiguous` error in §3, in descending similarity, ties
  to the older task.

Fuzzy matching is never allowed to merge on its own because a merge hides a report and the
reporting side has no way to unmerge later: "check rejects missing spec" and "check
rejects missing plan" share three of five tokens, score exactly 0.6, and are different
bugs. A false split, by contrast, costs
the maintainer one `dep` or `drop` at triage. The reporter can always override in either
direction with `--recur` or `--new`.

## 5. Output

```
feedback -> { id, action: "created"|"recurred", path: string, warnings }
```

`path` is the absolute path of the task file in the target. `--pretty` prints
`<action> <id>` and the warnings on stderr, like other write commands.

## 6. Skill

`skills/tasks/SKILL.md` gains a "Feedback" section:

- **When:** the tool got in the way (`friction`), the tool could not do something needed
  (`gap`), an improvement occurred to you (`idea`), or something worked notably well
  (`positive`). File it at the moment, in one command, and carry on; do not hold it for
  the end of the session.
- **What:** one line about the tool. The body may add the command, the error kind, and
  what was expected. Describe the tool, not the project: no repository names, file paths,
  people, or content from the project. The upstream repository is public.
- **Then:** nothing. Do not commit in the upstream repository; do not triage your own
  report. Keep the returned id in a note on the task you were working on if the outcome
  matters to it; `tasks show <id>` from any registered project shows the entry and its
  triage notes later (§6.1).

A second, short section for sessions in this repository: uncommitted files under `tasks/`
tagged `feedback` are unreviewed reports; read each, redact anything that describes a
project rather than the tool, and only then commit. Ideas tagged `feedback` are the triage
queue; scope, drop, or promote them like any other idea, and record the outcome in a note
so a reporter checking back sees it.

### 6.1 `show` resolves foreign ids

`tasks show <id>` currently loads only local tasks. It gains registry resolution: an id
with another project's prefix is read from that project through the resolver, read-only,
with `spec_path`/`plan_path` made absolute against that project's root. Unreachable
prefixes fail with `unresolvable_id`, as `dep` does. This is what lets a reporter follow
up on a filed entry, and it is useful for cross-project dependencies regardless.

## 7. Original design updates

Land with the implementation: §5 gains the `feedback` command, the foreign-id behaviour
of `show`, and the `prime` uncommitted-files warning; §5.1 gains the `feedback` output
shape; §8 gains the skill sections above.

## 8. Testing

End to end (`tests/cli.rs`), with two projects in one registry, one registered as `tasks`:

- filing from the other project creates an `idea` in the target with the three tags, the
  body, and a warning naming the uncommitted file; the JSON says `created` and the path
  exists;
- a second filing with the same summary from a third project says `recurred`, adds no
  file, appends the note with that project's prefix, and adds its `from:` tag and a
  differing category tag; `--body` on recurrence appends the detail note;
- a similar but inexact summary fails with `ambiguous` naming the candidate and writes
  nothing; `--new` then creates a second file and `--recur <id>` recurs; `--recur` rejects
  an id that is not open feedback;
- a dissimilar summary creates a new entry (threshold check both sides of 0.6);
- no `tasks` prefix in the registry, or a registered root without a config, fails with
  `config` and writes nothing; running outside any project fails with `no_project`;
- filing from inside the target yields `from:tasks`;
- `show <foreign id>` from the reporting project returns the entry with its notes, and an
  unregistered prefix fails with `unresolvable_id`;
- `prime` warns about an uncommitted task file in a git checkout and stays silent in a
  plain directory.

Unit: token normalization and the Jaccard threshold, including short-word dropping and
the exact-versus-similar distinction; the guarded write retries on a changed hash and
gives up with `concurrent_modification`.

## 9. Out of scope

Configurable targets; private or per-machine stores; syncing or uploading; automatic
triage; a feedback view separate from `list --tag feedback`; editing, unmerging, or
closing feedback from the reporting side; file locking beyond the hash guard in §3.
