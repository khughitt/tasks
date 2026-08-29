use crate::error::{Error, Result};
use crate::frontmatter::{self, Value};
use crate::model::{Note, Size, Status, Task, TaskId};

pub const NOTES_DELIMITER: &str = "## Notes";
const KEYS: [&str; 13] = [
    "id", "title", "status", "priority", "size", "owner", "created", "updated", "depends", "tags",
    "spec", "plan", "step",
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
    // Timestamps are the one schema scalar containing `:`; quote them for the strict subset parser.
    let fm = fm
        .lines()
        .map(|line| {
            if line.starts_with("created: ") || line.starts_with("updated: ") {
                let (k, v) = line.split_once(':').unwrap();
                format!("{k}: \"{}\"", v.trim_start())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
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
        owner: scalar("owner")?,
        created,
        updated,
        depends,
        tags: list("tags")?,
        spec: scalar("spec")?,
        plan: scalar("plan")?,
        step: scalar("step")?,
        body,
        notes,
    };
    validate_task(&task).map_err(|e| perr(file, e.to_string()))?;
    Ok(task)
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
    Ok((body.join("\n").trim().to_string(), notes))
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
pub fn validate_doc_path(kind: &str, dir: &str, rel: &str) -> Result<()> {
    validate_line(kind, rel)?;
    let segs: Vec<_> = rel.split('/').collect();
    let expected: Vec<_> = dir.split('/').collect();
    if !segs
        .iter()
        .all(|s| !s.is_empty() && *s != "." && *s != "..")
        || segs.len() <= expected.len()
        || segs[..expected.len()] != expected[..]
        || !rel.ends_with(".md")
    {
        return Err(Error::Validation(format!(
            "{kind} {rel:?} must be a normalized path under {dir}/"
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
        validate_doc_path("spec", "docs/specs", s)?;
    }
    if let Some(p) = &t.plan {
        validate_doc_path("plan", "docs/plans", p)?;
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
        (String::from("tags"), Value::List(t.tags.clone())),
    ]);
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
    fn rejects_bad_values() {
        assert!(parse_task(&MINIMAL.replace("tags: []", "tags: []\ncolor: red"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("priority: 2\n", ""), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("priority: 2", "priority: 7"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("status: idea", "status: soon"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("depends: []", "depends: [nope]"), "x").is_err());
        assert!(parse_task(&MINIMAL.replace("tags: []", "tags: []\nstep: only"), "x").is_err());
    }
    #[test]
    fn rejects_body_notes_paths() {
        assert!(parse_task(
            &format!("{MINIMAL}\nbody\n\n## Notes\n\nnot a bullet\n"),
            "x"
        )
        .is_err());
        assert!(parse_task(&format!("{MINIMAL}\n## Notes\n\n- bad line\n"), "x").is_err());
        assert!(validate_body("x\n## Notes\ny").is_err());
        assert!(validate_body("x\n### Notes\ny").is_ok());
        assert!(validate_note_text("a\nb").is_err());
        assert!(validate_note_text("").is_err());
        assert!(parse_task(
            &MINIMAL.replace("tags: []", "tags: []\nspec: docs/specs/../plans/x.md"),
            "x"
        )
        .is_err());
        assert!(parse_task(
            &MINIMAL.replace("tags: []", "tags: []\nspec: docs/specs/sub/x.md"),
            "x"
        )
        .is_ok());
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
}
