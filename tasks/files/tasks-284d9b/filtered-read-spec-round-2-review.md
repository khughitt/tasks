I recommend **revise**. One Important finding and three minors, all checked against the code at 998e6a4. The spec is the base of the frozen commit ce08cdc. This was a read-only review: I edited nothing and wrote no task note. Next, you decide on the Important finding. Then the Codex session that parked tasks-284d9b revises the spec, and the revision goes to review as a new frozen commit.

Everything else I checked holds. The resolver flow, the shorthand behaviour, the punctuation trimming, the error kinds from `open_registered` (`unresolvable_id` from an id, `config` from a prefix), the commands that take filters, and the choice to keep `--source` literal all match the code.

**Important: rule 2 keeps local scope for a foreign filter, so the read can only fail**
- **Finding:** a parent must be in the same project as its child. `validate_parent` (`src/hierarchy.rs`) rejects anything else ("parent … must be in this project").
- **Consequence:** inside project A, `list --under ops-2fe949` and `--parent ops-2fe949` can never match a task in A. Rule 2 therefore turns them into a guaranteed `task_not_found`, while `tree ops-2fe949` and `show ops-2fe949` from the same directory route to `ops`.
- **The spec's reason doesn't hold:** it says "the requested local or explicit scope is authoritative." Local scope is only implied by where you're standing; you didn't ask for it. Explicit flags are the real request, and rule 1 already covers them.
- **Recommendation:**
  - Without explicit scope, a full-prefix filter routes exactly as `open_id_ctx_unwarned` does today. A prefix matching the local project keeps the checkout (including `-C`, worktrees and linked records). Any other prefix goes through the registry.
  - Rule 5's conflict check then applies inside a project too (for example `--parent` local with `--under` foreign).
  - The new helper becomes: agree on one filter id, then call `open_id_read_ctx`. That gives filters and `tree <id>` one policy, which is what the task's "one shared helper … behave alike" asks for. It also drops the special case that only routes outside a project.
- **Rejected alternative:** keep rule 2, but make the `task_not_found` detail name the project and suggest `--project ops`. That's cheaper, but the read still fails in a case where the answer is unambiguous.
- **Cost if I'm wrong:** a mistyped prefix inside a project reads another project instead of failing. The rows still carry their prefixes, and the risk is the same one `tree <id>` already takes.

**Minor**
1. **Your original `--source` command still fails with a bare `no_project`.** Keeping `--source` literal is sound: it records provenance, and routing to the source's project would miss tasks filed elsewhere. But the error is the only thing you'll see when you run it again. When filters are present, the `no_project` detail could name `--all-projects` and `--project <prefix>`. That changes message text only, not the JSON shape.
2. **Completion is ambiguous.** One function, `complete::scoped`, completes `--parent`, `--under`, `next --under` and positional `tree <id>`. The spec changes filter completion but puts positional `tree` completion out of scope. The plan has to either split the function or accept that `tree` changes too. If the Important finding is adopted, the existing typed-prefix routing in `complete.rs` (`local_or_foreign`) already does the right thing for both.
3. **Rule 5's argument for wide scopes doesn't stand up.** It says an explicit `--group` or `--all-projects` scope "may contain both projects". But since parents share their child's project, a `--parent` in one project combined with an `--under` in another is empty in every scope. Keeping ordinary intersection there is still acceptable. The argument should just stop implying the combination can ever match.

For the record, I didn't check the current state of tasks-e147b9. The spec already says to recheck it before implementation.

For whoever writes the round note, I believe this is round 1, since the task's only note is `started` and the earlier "Builder check" wasn't recorded as a review round:

`review: spec round 1 — verdict: revise; findings: Important 1, Minor 3; reviewer: claude-code/claude-opus-5-5`

Nothing was pushed. I left no processes running and made no changes to the worktree or any branch. The spec stays in .worktrees/tasks-284d9b at ce08cdc, unmerged until the planned work completes.
