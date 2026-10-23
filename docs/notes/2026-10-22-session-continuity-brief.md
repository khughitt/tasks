# Claim ownership after a resumed conversation changes identity

Scoping handoff, 2026-10-22. Idea: tasks-d8d97e. Lane: tasks-2fa8c6.
Not an approved design.

## Problem

A resumed conversation should have a clear, safe route to its earlier claims.
The report says Claude Code supplied a new CLAUDE_CODE_SESSION_ID while the old
process remained alive; park refused until an explicit start --force takeover.

## Current behaviour and evidence

Checked the main checkout at 2ce5663. In src/relay/resolve.rs, same_session
normalizes known spellings of one ID, not different native IDs.
src/claims.rs::proves_ownership requires matching host, boot and nearest process,
then rejects a contradictory native hint even when that process matches.
src/commands/park.rs refuses a foreign live or unverifiable claim. This is
consistent with the reported refusal; it does not prove the harness supplied a
legitimate continuation. The report has no exact harness version, session source,
or before/after capture.

The existing delegated-identity brief and completed work concern Codex thread/root
separation and PID views. Open lane work covers diagnostics and review minors;
tasks-00593c covers relay acceptance gaps. None establishes Claude resume lineage.

## Constraints

Preserve the scoped-hint noncontradiction rule in
docs/specs/2026-09-22-relay-ancestry-identity-design.md and distinct worker ownership.
Shared process, root, or conversation content alone cannot authorize takeover.
Do not change live claims or host configuration during research. The incident is
a report, not an established upstream defect; check upstream reports, branches
and release notes before reproducing or patching any suspected external defect.

## Alternatives

1. Retain explicit, recorded takeover and make recovery guidance concrete.
2. Recognize continuation only from authoritative lineage evidence, if the harness
   or relay provides it and its trust boundary can be established.
3. Treat a matching process as sufficient despite a changed native ID.

Lean toward option 1 until evidence supports option 2. Reject option 3 because
the existing checks deliberately prevent one logical session owning another's work.

## Unanswered questions

- What exact resume operation, harness version and identity inputs produced the
  change? The research owner needs sanitized incident evidence or a bounded capture.
- Is verifiable old-to-new lineage available, and who owns its delivery? Research
  must distinguish tasks policy from harness or relay behavior.
- Is documented explicit recovery sufficient, or does evidence justify a reviewed
  ownership change? Research returns a recommendation, not implementation.

## Proposed decomposition

tasks-ea0f60 (P2/s/mid/direct) under tasks-2fa8c6 establishes the continuity evidence and
safe recovery contract; tasks-d8d97e waits on it. No design task is needed until
that finding establishes a change worth designing.
