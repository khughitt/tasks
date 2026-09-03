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
- **Recurrence is a note.** A second report of the same thing appends a note to the open
  entry instead of creating a duplicate. The list of notes is the recurrence record.
- **The reporting project is a tag.** `from:<prefix>` is free, since the registry already
  knows it, and `list --tag from:<prefix>` answers "what did that project hit".
- **This repository is public.** Entries filed from private projects are public. The
  skill instructs agents to describe the tool, never the project, and to include no paths,
  names, or secrets; nothing enforces it. A private fork registered under the target
  prefix is the escape hatch.

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
3. **Look for an open match** (§4) unless `--new` is given. `--recur ID` names the match
   explicitly and skips the search; the id must be an open task in the target tagged
   `feedback`, else `validation`.
4. **Recur or create.**
   - Match: append a note to it reading `feedback from <prefix>: <summary>` and, if
     `--body` was given, a second note `detail from <prefix>: <text>` (single line, as all
     notes are; multi-line detail is rejected with `validation` on recurrence). The task's
     `updated` moves, so it rises in `list`.
   - No match: add a task in the target with title `<summary>`, status `idea`, priority
     2, no size, no owner, body `<text>`, tags `feedback`, `<category>`, `from:<prefix>`.
5. **Report.** The output names what happened, because the reporter should know whether it
   started something or joined something.

The reporter never sets priority or size; those are triage decisions made here. The
reporter also never commits: the entry lands as an uncommitted file in the target's
working tree, and the output carries a warning saying so and where. Committing it is the
job of whoever next works in this repository, which `prime` makes hard to miss because
the idea count is visible and `git status` shows the file.

## 4. Matching

Candidates are the target's open tasks tagged `feedback`. Titles are normalized to
lowercase ASCII-alphanumeric tokens of three or more characters. Two titles match when the
Jaccard similarity of their token sets is at least 0.6. The best match wins; ties go to the
older task. The threshold is deliberately strict: a false merge hides a report, a false
split costs one `--recur` later. The reporter can always override in either direction.

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
  report.

A second, short section for sessions in this repository: ideas tagged `feedback` are the
triage queue; scope, drop, or promote them like any other idea, and mention the outcome in
a note so the reporting project's next reader sees it through `show`.

## 7. Original design updates

Land with the implementation: §5 gains the `feedback` command and §5.1 its output shape;
§8 gains the skill sections above.

## 8. Testing

End to end (`tests/cli.rs`), with two projects in one registry, one registered as `tasks`:

- filing from the other project creates an `idea` in the target with the three tags, the
  body, and a warning naming the uncommitted file; the JSON says `created` and the path
  exists;
- a second filing with a similar summary says `recurred`, adds no file, and appends the
  note with the reporting prefix; `--body` on recurrence appends the detail note;
- `--new` forces a second file for a similar summary; `--recur <id>` targets a specific
  entry and rejects an id that is not open feedback;
- a dissimilar summary creates a new entry (threshold check both sides of 0.6);
- no `tasks` prefix in the registry, or a registered root without a config, fails with
  `config` and writes nothing; running outside any project fails with `no_project`;
- filing from inside the target yields `from:tasks`.

Unit: token normalization and the Jaccard threshold, including short-word dropping.

## 9. Out of scope

Configurable targets; private or per-machine stores; syncing or uploading; automatic
triage; a feedback view separate from `list --tag feedback`; editing or closing feedback
from the reporting side.
