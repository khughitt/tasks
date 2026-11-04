# Retention contract fixtures

Source: lore commit 252684ccbd105949bcbecce54740d447f3cf98c4.

- record.schema.json: skills/references/retention-v1-record.schema.json, SHA-256 24195790dc0f30563d24d6f43f40ad7cedc83ba32452df3f306e846380788c20
- manifest.schema.json: skills/references/retention-v1-manifest.schema.json, SHA-256 97b6879c7fcaa26c0698f64ddeac26da5f63869e41d8a390360c54d408ce1d0c
- cases.json: tools/fixtures/retention-contract/cases.json, SHA-256 3dbeed078b4ba0db0ef757ada408491676cec4dfb0b916bd00d5a17b1b49c3b0

The structural verdict describes v1, so a missing version is structurally invalid
for v1 even though the history reader explicitly accepts historical unversioned
records. Semantic expectations remain separate obligations. JSON readers must
reject duplicate keys before structural validation; JSON Schema cannot recover
keys already overwritten by a JSON parser. Full byte and history verification
remain required; these fixtures establish no filesystem verdict.
