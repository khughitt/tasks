# Make task priority convey useful distinctions

## Problem

Most tasks receive the default P2, leaving priority with little influence on
selection. tasks-5ed8a4 asks whether a wider or remapped scale would help, alongside
the same question about thought importance in mind6-66dd9e. Establish useful ranking
behaviour before changing stored numbers or migrating existing records.

## Current behaviour and evidence

Code inspected at 7a7c85c on 2026-10-20. src/cli.rs defines priority as urgency
from 0 to 4; src/format.rs rejects values above 4. Thus P0..P4 already has five
levels: relabelling it P1..P5 alone adds no distinctions. The task's 2026-10-19
note reports 644 of 892 open tasks across 11 projects at P2 (72%); that historical
sample was not remeasured. A local list on 2026-10-20, before this pass's writes,
returned 122 idea/todo/doing/blocked records: P0 0, P1 3, P2 85, P3 33, P4 1
(70% at P2). These include goals; they are not a ready-work-only sample.

The canonical source was read with mindful --json show using the full typed
thought:0082fda09c0ec41dcf9f2ae116727ea5 reference. It asks for more dynamic range
and links the question to importance in search, walks and gardening. The task's
existing source and full body remain intact.

mind6-66dd9e already waits on research mind6-8a54c8, which compares derived signals,
explicit levels and relative ranking against blind human judgments. Its five-field
body explicitly names tasks-5ed8a4 as a consumer of the finding. Reuse that research;
no duplicate signal-comparison study is needed. Its task record was read through
the CLI; its external code and brief were not inspected under this pass's fixed
checkout boundary. No local matching brief or attached draft spec was found.

## Constraints

Task urgency and thought importance are related but not interchangeable. A good
thought ranker does not establish a correct task execution order. Preserve explicit
priorities and the JSON contract until there is a reviewed migration decision;
re-scoping never silently changes existing priorities. Do not derive urgency from
agent edits or repeat captures without evidence of independent human interest.
No shared needs are declared here. If a later task-ranking pilot needs human
judgments, propose a declared need for that resource before using --need.

## Alternatives

1. Keep P0..P4 and improve assignment through relative comparisons during scope or
   curate. Current lean: await the existing pilot, then test whether its finding
   transfers to task ordering before choosing a policy or schema change.
2. Derive a ranking signal with an explicit override. The existing research probes
   this for thoughts; task-specific dependency, urgency and ownership constraints
   still need separate acceptance criteria.
3. Widen the scale, or remap it to P1..P5. Remapping alone preserves the default
   spike; more levels need evidence that users can reliably make more distinctions.

## Unanswered questions

- Which signals agree with blind human importance judgments, and does a simple
   combination beat the current ranker? Existing research mind6-8a54c8 answers it.
- Should task priority remain urgency, represent importance, or express preferred
   execution order? The user owns that distinction; frame it with the pilot's finding.
- Does the result transfer to tasks, and would relative comparisons improve ready
   work ordering? Revisit after the result; no implementation or extra design task
   is filed before that evidence arrives.

## Proposed decomposition

tasks-298107 is an ordinary child of Work discovery and resumption (tasks-46d207).
It holds tasks-5ed8a4 as a briefed idea. That idea now depends on the existing
mind6-8a54c8 (P2/s/mid/direct), without changing the research's sample, judging
protocol or implementation exclusions. On completion, its finding must be delivered
with tasks note tasks-5ed8a4 --no-reviewed before the result's branch merges;
the dependency also wakes the idea when the research closes. Review that outcome
and the task-specific distinction above before scoping a remedy. No new research
or priority migration is authorized by this handoff.
