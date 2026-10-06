---
id: tasks-b2e718
title: "check warns unlinked_step as soon as a plan is linked, so a plan awaiting review must either stay unlinked or have step tasks filed that the pickers then offer before the plan is approved"
status: idea
priority: 2
created: 2026-09-29T11:33:42Z
updated: 2026-10-06T16:52:04Z
depends: []
parent: tasks-46d207
tags: [feedback, friction, "from:mind6"]
agent: claude-code/claude-fable-5-1
---

Reported symptom: linking an unreviewed implementation plan produces unlinked_step warnings; filing its steps to satisfy the warning can make them available to pickers before the person approves the plan.

Next action: tasks-325e79 establishes the warning's actual exit behavior and compares draft linking, child creation, and parent review parks against existing pickers. Prefer reviewing the plan before filing executable children, if the existing workflow supports it; any new approval-state contract needs a separate reviewed design. Preserve this report as an idea until that investigation records a supported workflow or scoped gap.

## Notes

- 2026-10-06T16:52:03Z (main): scope: bounded investigation tasks-325e79 verifies safe draft-plan linking and child eligibility; no new review-state mechanism is assumed.
