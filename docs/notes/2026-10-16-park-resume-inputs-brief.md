# Brief: declaring the files a parked task needs to resume

Scope pass, 2026-10-16. Idea: tasks-2b5461 (from material).

## Problem

A task parked for an idle host can depend on files prepared before the park: a frozen
input, a built binary, a capture recipe's scripts. When those files are gone at resume,
the quiet queue still offers the work. The person gives up the machine, and the agent
learns about the loss only partway through. In the reported case the input sat on tmpfs,
was named only in the next-step text, and was lost in a reboot between park and resume.
The person reading `tasks quiet` before bed wants to know that the work can still run.

## Current behaviour and evidence

- Park records `next_step`, `waiting_on`, `reason`, and for `--reason quiet` the recipe
  `needs` and `minutes`, plus `host` and `worktree` (`src/commands/park.rs`; quiet queue
  design `docs/specs/2026-09-13-quiet-queue-design.md` §4). Nothing names a file, and no
  reader checks one.
- The park store is per prefix and host-local (`src/claims.rs`). The record and its notes
  sync. So a path is meaningful only beside the host the park already records.
- The quiet-park protocol (`skills/tasks/SKILL.md`, step 5) asks for phases, minutes and
  earlier refusals in the next step, but says nothing about where inputs live.
- `attach` (`tasks/files/<id>/`) already carries small files with a record across hosts.
  It does not suit large or host-bound inputs.

## Constraints

- JSON is the contract. A park field is additive under `ParkInfo` (as `reason`, `needs`
  and `minutes` were), while a record field changes the frontmatter and every row.
- The idea's own rule: keep actions declarative. A stored cleanup command would run on
  whichever host closes the task, so tasks reports and never deletes.
- Checks stay cheap on hot readers (`quiet`, `next`, `prime`): `stat` only, with hashing
  only on request.
- The quiet queue design defers idle detection and launching to its §8. This brief does
  not touch them.

## Alternatives

1. **Guidance only.** The quiet-park protocol says to keep resume inputs on persistent
   storage and to list their paths in the next step, and to check them before parking.
   This costs nothing, but nothing detects a loss.
2. **Park-scoped inputs.** `park --input <path>` (repeatable) stores absolute paths in the
   park entry beside `host`. `ParkInfo.inputs` is additive. `park` warns when an input is
   missing or sits on a volatile mount (tmpfs, `/run`, `/tmp`, found from the filesystem
   type). On the recorded host, `quiet`, `next` and `prime` flag an entry whose input is
   missing. A re-park replaces the list and `start` clears it, as with the recipe. This
   solves the reported loss and leaves the record format alone.
3. **Record-wide artifacts, as proposed.** A frontmatter list of `{path, role
   (resume-input | evidence | scratch), sha256?, host}`. Park and the queue readers check
   resume inputs, and `done`/`drop` list scratch artifacts still present with the command
   to remove them. This is the broadest option and also covers evidence and cleanup. It
   changes the record contract and puts host-local paths into synced records.

Lean: 2, with 1's guidance line beside it. The `done`/`drop` scratch listing in 3 is a
separate want, and it can follow as an extension if it is still wanted after 2.

## Unanswered questions

1. Park-scoped (2) or record-wide (3)? Is scratch cleanup at `done`/`drop` wanted now? —
   the user. Taken as the lean on rerun, 2026-10-17: park-scoped, with scratch cleanup
   left out. The design task's spec review is where the user can override it.
2. How is a volatile mount decided: the filesystem type from `statfs`, a fixed path list,
   or both? Is a warning enough, or should `park` refuse? — the design task.
3. Should an input missing on another host be reported as unknown rather than missing,
   since the store is host-local? — the design task.

## Proposed decomposition

Rerun 2026-10-17: the first pass filed no follow-up, so the idea had nothing to wait on
and came back as stranded. Filed now:

- tasks-037538 (goal, todo): declare and check a parked task's resume inputs. tasks-2b5461
  is its child.
- tasks-57425c (planned, child of the goal): "Design park-scoped resume inputs from the
  brief". It answers Q2 and Q3, adds the guidance line, and amends quiet queue §4.
  tasks-2b5461 depends on it and is woken by its finding note.

Record-wide artifacts and the `done`/`drop` scratch listing (option 3) are not filed. File
them as an extension of tasks-037538 if they are still wanted once 2 lands.
