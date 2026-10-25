# Filtered-read routing verification

Product commits: 634cf8d (routing), d8c147a (completion).
Final reviewed revision: bde3079; base: 998e6a4.
Reviewer: codex/gpt-6-astra; independent read-only whole-branch acceptance,
0 Critical, 0 Important, 0 Minor. Full review attached separately.

| Check | Result |
| --- | --- |
| Full pre-change baseline, just test | 1685 passed, 0 failed, 0 ignored; 14 binaries |
| Routing RED | 6 CLI and 2 linked cases failed on missing routing/guidance; fixture setup corrected before accepting RED |
| Routing GREEN | 6 CLI and 2 linked cases passed |
| Existing suffix/project-scope/positional-ID controls | 2 + 4 + 1 passed |
| Broken Git marker control | 1 passed; error retained |
| Hostile Git environment wrapper | 1 passed, including both new Git-backed fixtures |
| Completion RED | Foreign filter and updated tree assertions failed with empty candidates; local/explicit boundary control passed |
| Completion GREEN | Both new cases and all 16 completion_ cases passed |
| Routing fast suite | 1689 passed, 0 failed, 4 ignored |
| Final fast suite, just test-fast | 1691 passed, 0 failed, 4 ignored; 14 binaries |
| Commit checks | Hygiene, formatting, Clippy and tasks check passed; corrected four cloned-ref assertions before successful routing commit |
| Confined release install | Succeeded after each product task; host launcher never repointed |
| Installed outside-project ID read | Matched registered main, 0 descendants; branch-only children correctly excluded |
| Original source-only command outside project | Exit 1, no_project, both scope suggestions present |
| Live all-projects source recovery | Exit 0, 0 matches, 0 warnings |
| Final installed outside typed-prefix completion | Offered tasks-284d9b |
| Final task consistency and whitespace checks | Passed |

The four fast-suite skips are exhaustive rename snapshot enumeration and the
external-doc check/show/strict production-deadline cases. All four passed in the
full pre-change baseline. No affected behavior depends on these ignored cases;
hooks retain the normal full pre-push gate. No push was requested.

No JSON shape, stored field, option, dependency, mutation routing or Git-marker
policy changed. Sources remain literal and need explicit scope outside projects.
Related tasks-e147b9 was rechecked before execution and remained an open
investigation with no reproduced finding; it owns its excluded incident.

Routine raw test/build logs and extracted briefs are regenerable output. The
test timing wrapper records runs centrally; this summary, task notes, committed
tests and attached full reviews preserve the required evidence for closeout.
