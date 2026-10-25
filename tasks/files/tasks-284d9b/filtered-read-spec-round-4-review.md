I recommend accept. The revision f03f0b4 resolves all four round-2 findings, and I found nothing Critical or Important. I left two wording nits that don't need another round. Approving the spec is still your call under the process policy, so the implementation plan starts once you say so. This was a read-only review: I edited nothing and wrote no task note. Rounds 1 to 3 are already on the task, so this is round 4, and I've given its note line at the end.

Round-2 findings, checked against the diff from ce08cdc
- Important (routing): resolved. Rule 2 now routes a full-prefix filter the same way a positional ID does: a matching prefix keeps the local checkout or worktree, and any other prefix goes to its registered project. The opener checks that --parent and --under name the same project, then hands one of them to open_id_read_ctx. That puts filters and tree <id> on a single resolver. The rule-5 conflict check now applies inside a project too, and the tests cover it.
- Minor 1 (no_project guidance): resolved. The error keeps its kind and gains --all-projects / --project <prefix> in its detail text, which is the only text that changes. With a full-ID filter, open_id_ctx_unwarned already handles NoProject itself, so the guidance only ever appears on calls without one, which is what it's meant for.
- Minor 2 (completion): resolved. complete::scoped stays shared and uses local_or_foreign. I read that helper at 998e6a4: a typed prefix matching the local project keeps the worktree, and outside a project an empty fragment offers nothing, which matches the spec.
- Minor 3 (wide scopes): resolved. The spec now says the cross-project intersection is always empty, and a test asserts that.

I also checked the round-2 attachment: it holds my full review text, and the round notes on the task are accurate.

Remaining nits (no new round needed)
1. Rule 4 is worded against itself. It opens with "Without explicit scope" but then makes a claim about explicit scope ("including with explicit scope"). Both statements are true of today's Shorthand, but the second belongs on its own line or under rule 1.
2. Rule 3 says "may". It says the opener "may give this same guidance on an unfiltered call". But the implementation section then decorates every NoProject it returns, which settles the question, so "may" should become "does".

One thing for the plan, not the spec: parsing filters before choosing a destination needs the registry, because comparing retired prefixes means canonicalizing them. That can make the registry load before the local project is located. The only effect I can find is which config error wins when both the local config and the registry are broken; the error kind stays the same. The plan should still state the order so the "malformed local config keeps its typed error" test pins it.

For the note writer:

review: spec round 4 — verdict: accept; findings: Minor 2; reviewer: claude-code/claude-opus-5-5

Keith: approved for planning.
