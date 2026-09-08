use crate::error::{Error, Result};
use crate::frontmatter::{self, Value};
use crate::model::{Note, Size, Status, Task, TaskId};

pub const NOTES_DELIMITER: &str = "## Notes";
const KEYS: [&str; 16] = [
    "id", "title", "status", "priority", "size", "parallel", "owner", "created", "updated",
    "depends", "parent", "tags", "source", "spec", "plan", "step",
];

fn perr(file: &str, detail: impl Into<String>) -> Error {
    Error::Parse {
        file: file.into(),
        detail: detail.into(),
    }
}

pub fn parse_task(text: &str, file: &str) -> Result<Task> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| perr(file, "missing opening ---"))?;
    let (fm, after) = rest
        .split_once("\n---\n")
        .ok_or_else(|| perr(file, "missing closing ---"))?;
    let fm = quote_timestamps(fm);
    let pairs = frontmatter::parse(&format!("{fm}\n")).map_err(|e| match e {
        Error::Parse { detail, .. } => perr(file, detail),
        e => e,
    })?;
    for (k, _) in &pairs {
        if !KEYS.contains(&k.as_str()) {
            return Err(perr(file, format!("unknown key {k:?}")));
        }
    }
    let scalar = |k: &str| -> Result<Option<String>> {
        match pairs.iter().find(|(key, _)| key == k) {
            None => Ok(None),
            Some((_, Value::Scalar(s))) => Ok(Some(s.clone())),
            Some((_, Value::List(_) | Value::Raw(_))) => {
                Err(perr(file, format!("{k} must be a scalar")))
            }
        }
    };
    let required = |k: &str| -> Result<String> {
        scalar(k)?.ok_or_else(|| perr(file, format!("missing {k}")))
    };
    let list = |k: &str| -> Result<Vec<String>> {
        match pairs.iter().find(|(key, _)| key == k) {
            None => Err(perr(file, format!("missing {k}"))),
            Some((_, Value::List(v))) => Ok(v.clone()),
            Some((_, Value::Scalar(_) | Value::Raw(_))) => {
                Err(perr(file, format!("{k} must be a list")))
            }
        }
    };
    // Absent is false; a value that is neither boolean is an error, never a falsy default.
    let boolean = |k: &str| -> Result<bool> {
        match scalar(k)? {
            None => Ok(false),
            Some(v) if v == "true" => Ok(true),
            Some(v) if v == "false" => Ok(false),
            Some(v) => Err(perr(file, format!("{k} must be true or false, not {v:?}"))),
        }
    };
    let priority: u8 = required("priority")?
        .parse()
        .map_err(|_| perr(file, "priority must be an integer 0-4"))?;
    if priority > 4 {
        return Err(perr(file, "priority must be 0-4"));
    }
    let created = required("created")?;
    let updated = required("updated")?;
    crate::time::parse(&created).map_err(|e| perr(file, e.to_string()))?;
    crate::time::parse(&updated).map_err(|e| perr(file, e.to_string()))?;
    let depends = list("depends")?
        .iter()
        .map(|d| TaskId::parse(d))
        .collect::<Result<Vec<_>>>()
        .map_err(|e| perr(file, e.to_string()))?;
    let (body, notes) = split_body_notes(after, file)?;
    let task = Task {
        id: TaskId::parse(&required("id")?).map_err(|e| perr(file, e.to_string()))?,
        title: required("title")?,
        status: Status::parse(&required("status")?).map_err(|e| perr(file, e.to_string()))?,
        priority,
        size: scalar("size")?
            .map(|s| Size::parse(&s))
            .transpose()
            .map_err(|e| perr(file, e.to_string()))?,
        parallel: boolean("parallel")?,
        owner: scalar("owner")?,
        created,
        updated,
        depends,
        parent: scalar("parent")?
            .map(|p| TaskId::parse(&p))
            .transpose()
            .map_err(|e| perr(file, e.to_string()))?,
        tags: list("tags")?,
        source: scalar("source")?,
        spec: scalar("spec")?,
        plan: scalar("plan")?,
        step: scalar("step")?,
        body,
        notes,
    };
    validate_task(&task).map_err(|e| perr(file, e.to_string()))?;
    Ok(task)
}

/// Timestamps are the one schema scalar containing `:`; quote them for the strict subset
/// parser. Shared with the rename rewrite, which must round-trip a file it did not write.
pub fn quote_timestamps(fm: &str) -> String {
    fm.lines()
        .map(|line| {
            if line.starts_with("created: ") || line.starts_with("updated: ") {
                let (k, v) = line.split_once(':').expect("prefix matched");
                format!("{k}: \"{}\"", v.trim_start())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_body_notes(after: &str, file: &str) -> Result<(String, Vec<Note>)> {
    let mut body = Vec::new();
    let mut notes = Vec::new();
    let mut in_notes = false;
    for line in after.lines() {
        if line == NOTES_DELIMITER {
            if in_notes {
                return Err(perr(file, "second ## Notes heading"));
            }
            in_notes = true;
            continue;
        }
        if !in_notes {
            body.push(line);
        } else if !line.trim().is_empty() {
            notes.push(
                parse_note_line(line)
                    .ok_or_else(|| perr(file, format!("malformed note line {line:?}")))?,
            );
        }
    }
    let body = body.join("\n");
    let body = body
        .strip_prefix('\n')
        .unwrap_or(&body)
        .trim_end()
        .to_string();
    Ok((body, notes))
}

fn parse_note_line(line: &str) -> Option<Note> {
    let rest = line.strip_prefix("- ")?;
    let (at, rest) = rest.split_once(" (")?;
    let (by, text) = rest.split_once("): ")?;
    crate::time::parse(at).ok()?;
    if by.is_empty() || text.is_empty() {
        return None;
    }
    Some(Note {
        at: at.into(),
        by: by.into(),
        text: text.into(),
    })
}

pub fn validate_body(body: &str) -> Result<()> {
    if body.lines().any(|l| l == NOTES_DELIMITER) {
        Err(Error::Validation(
            "body contains reserved ## Notes delimiter".into(),
        ))
    } else {
        Ok(())
    }
}
pub fn validate_note_text(text: &str) -> Result<()> {
    validate_line("note text", text)
}
pub fn validate_line(field: &str, s: &str) -> Result<()> {
    if s.is_empty() {
        return Err(Error::Validation(format!("{field} must not be empty")));
    }
    if s.contains(['\n', '\r']) {
        return Err(Error::Validation(format!("{field} must be a single line")));
    }
    Ok(())
}
pub fn validate_owner(o: &str) -> Result<()> {
    validate_line("owner", o)?;
    if !o
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._/@+-".contains(c))
    {
        return Err(Error::Validation(format!(
            "owner {o:?} has invalid characters"
        )));
    }
    Ok(())
}
/// A doc link must be a normalized repo-relative `.md` path (no empty, `.`, or `..`
/// segments). Which roots it may live under is the project's decision (`Project::validate_docs`).
pub fn validate_doc_shape(kind: &str, rel: &str) -> Result<()> {
    validate_line(kind, rel)?;
    let normalized = rel
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    if !normalized || !rel.ends_with(".md") {
        return Err(Error::Validation(format!(
            "{kind} {rel:?} must be a normalized repo-relative .md path"
        )));
    }
    Ok(())
}

pub fn validate_doc_path(kind: &str, dirs: &[String], rel: &str) -> Result<()> {
    validate_doc_shape(kind, rel)?;
    let under_root = dirs.iter().any(|dir| {
        rel.strip_prefix(dir.as_str())
            .is_some_and(|rest| rest.starts_with('/'))
    });
    if !under_root {
        return Err(Error::Validation(format!(
            "{kind} {rel:?} must be under {}/",
            dirs.join("/ or ")
        )));
    }
    Ok(())
}

pub fn validate_task(t: &Task) -> Result<()> {
    TaskId::parse(&t.id.to_string())?;
    for dependency in &t.depends {
        TaskId::parse(&dependency.to_string())?;
    }
    validate_line("title", &t.title)?;
    if t.priority > 4 {
        return Err(Error::Validation("priority must be 0-4".into()));
    }
    crate::time::parse(&t.created)?;
    crate::time::parse(&t.updated)?;
    if t.step.is_some() && t.plan.is_none() {
        return Err(Error::Validation("step requires plan".into()));
    }
    if let Some(s) = &t.spec {
        validate_doc_shape("spec", s)?;
    }
    if let Some(p) = &t.plan {
        validate_doc_shape("plan", p)?;
    }
    if let Some(st) = &t.step {
        validate_line("step", st)?;
    }
    if let Some(o) = &t.owner {
        validate_owner(o)?;
    }
    for tag in &t.tags {
        validate_line("tag", tag)?;
    }
    if let Some(source) = &t.source {
        validate_line("source", source)?;
    }
    if t.depends.contains(&t.id) {
        return Err(Error::Validation("task cannot depend on itself".into()));
    }
    validate_body(&t.body)?;
    for n in &t.notes {
        crate::time::parse(&n.at)?;
        validate_owner(&n.by)?;
        validate_note_text(&n.text)?;
    }
    Ok(())
}

pub fn serialize_task(t: &Task) -> String {
    let s = |v: &str| Value::Scalar(v.to_string());
    let mut pairs = vec![
        ("id".into(), s(&t.id.to_string())),
        ("title".into(), s(&t.title)),
        ("status".into(), s(t.status.as_str())),
        ("priority".into(), Value::Raw(t.priority.to_string())),
    ];
    if let Some(z) = t.size {
        pairs.push(("size".into(), s(z.as_str())));
    }
    // Raw, not Scalar: needs_quotes quotes the literal `true`, which would write
    // `parallel: "true"` — readable back, but out of step with every other scalar.
    if t.parallel {
        pairs.push(("parallel".into(), Value::Raw("true".into())));
    }
    if let Some(o) = &t.owner {
        pairs.push(("owner".into(), s(o)));
    }
    pairs.extend([
        (String::from("created"), Value::Raw(t.created.clone())),
        (String::from("updated"), Value::Raw(t.updated.clone())),
        (
            String::from("depends"),
            Value::List(t.depends.iter().map(ToString::to_string).collect()),
        ),
    ]);
    if let Some(parent) = &t.parent {
        pairs.push(("parent".into(), s(&parent.to_string())));
    }
    pairs.push((String::from("tags"), Value::List(t.tags.clone())));
    if let Some(v) = &t.source {
        pairs.push(("source".into(), s(v)));
    }
    if let Some(v) = &t.spec {
        pairs.push(("spec".into(), s(v)));
    }
    if let Some(v) = &t.plan {
        pairs.push(("plan".into(), s(v)));
    }
    if let Some(v) = &t.step {
        pairs.push(("step".into(), s(v)));
    }
    let mut out = String::from("---\n");
    out.push_str(&frontmatter::serialize(&pairs));
    out.push_str("---\n");
    let body = t.body.trim_end();
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
        out.push('\n');
    }
    if !t.notes.is_empty() {
        out.push('\n');
        out.push_str(NOTES_DELIMITER);
        out.push_str("\n\n");
        for n in &t.notes {
            out.push_str(&format!("- {} ({}): {}\n", n.at, n.by, n.text));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    const MINIMAL: &str = "---\nid: sci-000001\ntitle: Tiny\nstatus: idea\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n";
    const FULL: &str = "---\nid: sci-4f2a9c\ntitle: Bank the holdings ledger\nstatus: todo\npriority: 2\nsize: m\nowner: keith\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: [sci-91be03, fam-0c3d7e]\ntags: [world-index, cut-12]\nspec: docs/specs/2026-08-24-holdings-design.md\nplan: docs/plans/2026-08-24-holdings.md\nstep: \"Task 3: emit the ledger row\"\n---\n\nFree-form body.\n\nSecond paragraph.\n\n## Notes\n\n- 2026-08-29T15:10:44Z (keith): started; the spec's §4 assumption no longer holds.\n- 2026-08-29T16:41:02Z (slice-12): split the emitter into sci-a7d1e2.\n";
    #[test]
    fn full_roundtrip() {
        let t = parse_task(FULL, "x").unwrap();
        assert_eq!(t.title, "Bank the holdings ledger");
        assert_eq!(t.size, Some(Size::M));
        assert_eq!(t.depends.len(), 2);
        assert_eq!(t.step.as_deref(), Some("Task 3: emit the ledger row"));
        assert_eq!(t.body, "Free-form body.\n\nSecond paragraph.");
        assert_eq!(t.notes.len(), 2);
        assert_eq!(serialize_task(&t), FULL);
    }
    #[test]
    fn minimal_roundtrip() {
        let t = parse_task(MINIMAL, "x").unwrap();
        assert_eq!(t.body, "");
        assert!(t.notes.is_empty());
        assert_eq!(serialize_task(&t), MINIMAL);
    }
    #[test]
    fn parent_roundtrips_after_depends() {
        let with_parent = MINIMAL.replace("depends: []", "depends: []\nparent: sci-000002");
        let t = parse_task(&with_parent, "x").unwrap();
        assert_eq!(t.parent.as_ref().unwrap().to_string(), "sci-000002");
        assert_eq!(serialize_task(&t), with_parent);
        let own = MINIMAL.replace("depends: []", "depends: []\nparent: sci-000001");
        assert!(
            parse_task(&own, "x").is_ok(),
            "self-parent is a hierarchy rule, not a format rule"
        );
    }
    #[test]
    fn rejects_bad_values() {
        assert!(parse_task(&MINIMAL.replace("tags: []", "tags: []\ncolor: red"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("priority: 2\n", ""), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("priority: 2", "priority: 7"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("status: idea", "status: soon"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("depends: []", "depends: [nope]"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("tags: []", "tags: []\nstep: only"), "x").is_err());
    }

    #[test]
    fn parallel_round_trips_and_is_omitted_when_false() {
        let mut t = parse_task(MINIMAL, "x").unwrap();
        assert!(!t.parallel, "absent key reads as false");
        assert!(
            !serialize_task(&t).contains("parallel"),
            "false is never written"
        );

        t.parallel = true;
        let text = serialize_task(&t);
        assert!(
            text.contains("\nparallel: true\n"),
            "emitted unquoted: {text}"
        );
        assert!(parse_task(&text, "x").unwrap().parallel);
    }

    #[test]
    fn parallel_accepts_both_spellings_and_rejects_anything_else() {
        // frontmatter::parse discards quoting, so the quoted forms are indistinguishable
        // from the bare words by the time parse_task sees them.
        for (value, expected) in [
            ("true", true),
            ("\"true\"", true),
            ("false", false),
            ("\"false\"", false),
        ] {
            let text = MINIMAL.replace("depends: []", &format!("parallel: {value}\ndepends: []"));
            assert_eq!(
                parse_task(&text, "x").unwrap().parallel,
                expected,
                "parallel: {value}"
            );
        }
        for bad in ["maybe", "1", "True", "yes"] {
            let text = MINIMAL.replace("depends: []", &format!("parallel: {bad}\ndepends: []"));
            let err = parse_task(&text, "x").unwrap_err().to_string();
            assert!(
                err.contains("parallel must be true or false"),
                "parallel: {bad} gave {err}"
            );
        }
    }

    #[test]
    fn parallel_false_in_a_file_is_dropped_on_the_next_write() {
        let text = MINIMAL.replace("depends: []", "parallel: false\ndepends: []");
        let t = parse_task(&text, "x").unwrap();
        assert!(!t.parallel);
        assert!(!serialize_task(&t).contains("parallel"));
    }

    #[test]
    fn rejects_non_utc_timestamp() {
        assert!(
            parse_task(
                &MINIMAL.replace(
                    "created: 2026-08-29T14:02:11Z",
                    "created: 2026-08-29T14:02:11+02:00"
                ),
                "x"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_note_owner() {
        assert!(
            parse_task(
                &format!("{MINIMAL}\n## Notes\n\n- 2026-08-29T15:10:44Z (bad owner): text\n"),
                "x"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_carriage_return_in_note_text() {
        assert!(validate_note_text("a\rb").is_err());
    }

    #[test]
    fn rejects_duplicate_notes_delimiter() {
        assert!(
            parse_task(
                &format!("{MINIMAL}\n## Notes\n\n- 2026-08-29T15:10:44Z (k): a\n\n## Notes\n"),
                "x"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_non_normalized_doc_paths() {
        for path in ["/docs/specs/x.md", "docs/specs/./x.md", "docs/specs//x.md"] {
            assert!(
                parse_task(
                    &MINIMAL.replace("tags: []", &format!("tags: []\nspec: {path}")),
                    "x"
                )
                .is_err(),
                "accepted {path}"
            );
        }
    }

    #[test]
    fn rejects_body_notes_paths() {
        assert!(
            parse_task(
                &format!("{MINIMAL}\nbody\n\n## Notes\n\nnot a bullet\n"),
                "x"
            )
            .is_err()
        );
        assert!(parse_task(&format!("{MINIMAL}\n## Notes\n\n- bad line\n"), "x").is_err());
        assert!(validate_body("x\n## Notes\ny").is_err());
        assert!(validate_body("x\n### Notes\ny").is_ok());
        assert!(validate_note_text("a\nb").is_err());
        assert!(validate_note_text("").is_err());
        assert!(
            parse_task(
                &MINIMAL.replace("tags: []", "tags: []\nspec: docs/specs/../plans/x.md"),
                "x"
            )
            .is_err()
        );
        assert!(
            parse_task(
                &MINIMAL.replace("tags: []", "tags: []\nspec: docs/specs/sub/x.md"),
                "x"
            )
            .is_ok()
        );
    }

    #[test]
    fn rejects_invalid_task_and_dependency_ids() {
        let mut task = parse_task(MINIMAL, "x").unwrap();
        task.id.hex = "bad".into();
        assert!(validate_task(&task).is_err());

        let mut task = parse_task(MINIMAL, "x").unwrap();
        task.depends.push(TaskId {
            prefix: "b".into(),
            hex: "000001".into(),
        });
        assert!(validate_task(&task).is_err());
    }

    #[test]
    fn source_round_trips_bare_and_sits_after_tags() {
        let text = MINIMAL.replace("tags: []", "tags: []\nsource: keep-note-42");
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.source.as_deref(), Some("keep-note-42"));
        let out = serialize_task(&t);
        assert!(
            out.contains("tags: []\nsource: keep-note-42\n---\n"),
            "{out}"
        );
        assert_eq!(parse_task(&out, "x").unwrap(), t);
    }

    #[test]
    fn source_with_a_colon_is_quoted_on_write_and_unquoted_on_read() {
        let text = FULL.replace(
            "tags: [world-index, cut-12]",
            "tags: [world-index, cut-12]\nsource: \"mail:<42@example.org>\"",
        );
        let t = parse_task(&text, "x").unwrap();
        assert_eq!(t.source.as_deref(), Some("mail:<42@example.org>"));
        let out = serialize_task(&t);
        // written after tags, before spec, quoted because of the colon
        assert!(
            out.contains(
                "tags: [world-index, cut-12]\nsource: \"mail:<42@example.org>\"\nspec: docs/specs/"
            ),
            "{out}"
        );
        assert_eq!(parse_task(&out, "x").unwrap(), t);
    }

    #[test]
    fn source_is_omitted_when_absent() {
        let t = parse_task(MINIMAL, "x").unwrap();
        assert_eq!(t.source, None);
        assert!(!serialize_task(&t).contains("source:"));
    }

    #[test]
    fn rejects_empty_or_multiline_source() {
        let err =
            parse_task(&MINIMAL.replace("tags: []", "tags: []\nsource: \"\""), "x").unwrap_err();
        assert!(
            err.to_string().contains("source must not be empty"),
            "{err}"
        );

        let mut t = parse_task(MINIMAL, "x").unwrap();
        t.source = Some("a\nb".into());
        let err = validate_task(&t).unwrap_err();
        assert!(
            err.to_string().contains("source must be a single line"),
            "{err}"
        );
        t.source = Some(String::new());
        let err = validate_task(&t).unwrap_err();
        assert!(
            err.to_string().contains("source must not be empty"),
            "{err}"
        );
    }
}
