use crate::error::{Error, Result};
use crate::model::{Ref, Relation, Task};
use crate::repo::Project;
use crate::resolve::{LocatedEntry, Resolver};
use crate::{format, frontmatter};
use std::path::PathBuf;

pub enum Location<'a> {
    Path(&'a str),
    Attachment(&'a str),
    External,
}

pub enum Presence {
    Here(PathBuf),
    Main { root: PathBuf, path: PathBuf },
    Absent(PathBuf),
    Special(PathBuf),
    Unsafe { path: PathBuf, detail: String },
}

pub fn present(
    resolver: &Resolver,
    project: &Project,
    task: &Task,
    entry: &Ref,
) -> Result<Presence> {
    use crate::attachments::{self, EntryState, Ledger};

    match location(&entry.reference)? {
        Location::Path(rel) => Ok(match resolver.locate_entry(rel)? {
            LocatedEntry::Here(path) => Presence::Here(path),
            LocatedEntry::Main { root, path } => Presence::Main { root, path },
            LocatedEntry::Missing => Presence::Absent(project.doc_path(rel)),
            LocatedEntry::Special(path) => Presence::Special(path),
        }),
        Location::Attachment(name) => {
            let path = attachments::task_dir(project, &task.id).join(name);
            match attachments::check_storage(project, &task.id) {
                Ok(false) => return Ok(Presence::Absent(path)),
                Ok(true) => {}
                Err(Error::AttachmentUnsafe(detail)) => {
                    return Ok(Presence::Unsafe { path, detail });
                }
                Err(error) => return Err(error),
            }
            if attachments::ledger(task).get(name) != Some(&Ledger::Attached) {
                return Ok(Presence::Absent(path));
            }
            Ok(match attachments::entry_state(&path)? {
                EntryState::Absent => Presence::Absent(path),
                EntryState::File(_) => Presence::Here(path),
                EntryState::Unsafe(detail) => {
                    let kind = std::fs::symlink_metadata(&path)?.file_type();
                    if !kind.is_file() && !kind.is_dir() && !kind.is_symlink() {
                        Presence::Special(path)
                    } else {
                        Presence::Unsafe { path, detail }
                    }
                }
            })
        }
        Location::External => unreachable!("external references have no local presence"),
    }
}

pub fn refuse_problems(task: &Task) -> Result<()> {
    if let Some(problem) = audit(task).into_iter().next() {
        return Err(Error::Reference {
            kind: problem.kind,
            detail: problem.detail,
        });
    }
    Ok(())
}

pub fn require_targets(
    resolver: &Resolver,
    project: &Project,
    task: &Task,
    original: Option<&Task>,
) -> Result<()> {
    for entry in &task.refs {
        if entry.relation == Relation::Output
            || matches!(location(&entry.reference)?, Location::External)
            || original.is_some_and(|old| {
                old.refs.iter().any(|before| {
                    before.reference == entry.reference && before.relation == entry.relation
                })
            })
        {
            continue;
        }
        match present(resolver, project, task, entry)? {
            Presence::Here(_) => {}
            Presence::Main { root, .. } => {
                resolver.note_main_entry(&entry.handle, &entry.reference, &root)
            }
            Presence::Absent(_) => {
                return Err(Error::RefNotFound(format!(
                    "{}: reference {} {} does not exist",
                    task.id, entry.handle, entry.reference
                )));
            }
            Presence::Special(path) => crate::resolve::refuse_special(&path)?,
            Presence::Unsafe { detail, .. } => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    Ok(())
}

pub fn require_outputs(resolver: &Resolver, project: &Project, task: &Task) -> Result<()> {
    let mut missing = Vec::new();
    for entry in &task.refs {
        if entry.relation != Relation::Output
            || matches!(location(&entry.reference)?, Location::External)
        {
            continue;
        }
        match present(resolver, project, task, entry)? {
            Presence::Here(_) | Presence::Main { .. } => {}
            Presence::Absent(_) => {
                missing.push(format!("{} ({})", entry.handle, entry.reference));
            }
            Presence::Special(path) => crate::resolve::refuse_special(&path)?,
            Presence::Unsafe { detail, .. } => return Err(Error::AttachmentUnsafe(detail)),
        }
    }
    if !missing.is_empty() {
        return Err(Error::RefOutputMissing(format!(
            "{} declares outputs that do not exist: {}; produce them, or remove each with `tasks edit {} --rm-ref <handle>`",
            task.id,
            missing.join(", "),
            task.id
        )));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum Citation<'a> {
    Handle(&'a str),
    Malformed(&'a str),
}

pub fn citations(body: &str) -> Vec<Citation<'_>> {
    use pulldown_cmark::{Event, Parser, Tag};

    let code_ranges: Vec<_> = Parser::new(body)
        .into_offset_iter()
        .filter_map(|(event, range)| match event {
            Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => Some(range),
            _ => None,
        })
        .collect();
    body.match_indices("[ref:")
        .filter(|(start, _)| !code_ranges.iter().any(|range| range.contains(start)))
        .map(|(start, _)| {
            let rest = &body[start + "[ref:".len()..];
            let line = &rest[..rest.find(['\n', '\r']).unwrap_or(rest.len())];
            match line.find(']') {
                Some(end) => {
                    let handle = &line[..end];
                    if handle.len() <= 32 && format::valid_doc_scheme(handle) {
                        Citation::Handle(handle)
                    } else {
                        Citation::Malformed(handle)
                    }
                }
                None => Citation::Malformed(line),
            }
        })
        .collect()
}

pub const DUPLICATE: &str = "ref_duplicate";
pub const CITATION_DANGLING: &str = "ref_citation_dangling";
pub const CITATION_MALFORMED: &str = "ref_citation_malformed";

pub struct Problem {
    pub kind: &'static str,
    pub detail: String,
}

pub fn audit(task: &Task) -> Vec<Problem> {
    let mut problems = Vec::new();
    for (index, first) in task.refs.iter().enumerate() {
        for second in &task.refs[index + 1..] {
            if first.reference == second.reference {
                problems.push(Problem {
                    kind: DUPLICATE,
                    detail: format!(
                        "{} and {} both reference {}",
                        first.handle, second.handle, first.reference
                    ),
                });
            }
        }
    }
    for citation in citations(&task.body) {
        match citation {
            Citation::Handle(handle) => {
                if !task.refs.iter().any(|entry| entry.handle == handle) {
                    problems.push(Problem {
                        kind: CITATION_DANGLING,
                        detail: format!("[ref:{handle}] names no reference"),
                    });
                }
            }
            Citation::Malformed(text) => problems.push(Problem {
                kind: CITATION_MALFORMED,
                detail: format!("[ref:{text}] is not a reference handle"),
            }),
        }
    }
    problems
}

pub fn parse_entry(text: &str) -> Result<Ref> {
    let (handle, rest) = text
        .split_once(" (")
        .ok_or_else(|| Error::Validation("reference needs a handle and attributes".into()))?;
    let (attributes, rest) = rest
        .split_once("): ")
        .ok_or_else(|| Error::Validation("reference attributes must end with ): ".into()))?;
    let mut attributes = attributes.split(", ");
    let relation = Relation::parse(attributes.next().expect("split has a first element"))?;
    let mut required = false;
    let mut kind = None;
    for attribute in attributes {
        if attribute == "required" {
            if required || kind.is_some() {
                return Err(Error::Validation(
                    "reference attribute order must be relation, required, kind".into(),
                ));
            }
            required = true;
        } else if let Some(value) = attribute.strip_prefix("kind=") {
            if kind.is_some() {
                return Err(Error::Validation(
                    "duplicate reference kind attribute".into(),
                ));
            }
            kind = Some(value.to_string());
        } else {
            return Err(Error::Validation(format!(
                "unknown reference attribute {attribute:?}"
            )));
        }
    }
    let (reference, tail) = if rest.starts_with('"') {
        frontmatter::take_quoted(rest).map_err(Error::Validation)?
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let reference = &rest[..end];
        if reference.contains(['"', '\\']) {
            return Err(Error::Validation(
                "reference containing a quote or backslash must be quoted".into(),
            ));
        }
        (reference.to_string(), &rest[end..])
    };
    let description = if tail.is_empty() {
        None
    } else {
        Some(
            tail.strip_prefix(" — ")
                .ok_or_else(|| {
                    Error::Validation("reference description must follow \" — \"".into())
                })?
                .to_string(),
        )
    };
    let entry = Ref {
        handle: handle.into(),
        relation,
        required,
        kind,
        reference,
        description,
    };
    validate_entry(&entry)?;
    Ok(entry)
}

pub fn render_entry(entry: &Ref) -> String {
    let mut text = format!("{} ({}", entry.handle, entry.relation.as_str());
    if entry.required {
        text.push_str(", required");
    }
    if let Some(kind) = &entry.kind {
        text.push_str(&format!(", kind={kind}"));
    }
    text.push_str("): ");
    if entry
        .reference
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\\')
    {
        text.push_str(&frontmatter::quote(&entry.reference));
    } else {
        text.push_str(&entry.reference);
    }
    if let Some(description) = &entry.description {
        text.push_str(" — ");
        text.push_str(description);
    }
    text
}

pub fn validate_entry(entry: &Ref) -> Result<()> {
    if entry.handle.len() > 32 || !format::valid_doc_scheme(&entry.handle) {
        return Err(Error::Validation(format!(
            "reference handle {:?} must match [a-z][a-z0-9-]{{0,31}}",
            entry.handle
        )));
    }
    if let Some(kind) = &entry.kind
        && !format::valid_doc_scheme(kind)
    {
        return Err(Error::Validation(format!(
            "reference kind {kind:?} must match [a-z][a-z0-9-]*"
        )));
    }
    if let Some(description) = &entry.description {
        format::validate_line("reference description", description)?;
        if description.trim().is_empty() {
            return Err(Error::Validation(
                "reference description must not be empty".into(),
            ));
        }
    }
    validate_reference(&entry.reference)
}

pub fn validate_reference(reference: &str) -> Result<()> {
    location(reference).map(|_| ())
}

pub fn location(reference: &str) -> Result<Location<'_>> {
    if reference.starts_with('@') {
        return Err(Error::Validation(
            "cross-project references are not supported yet".into(),
        ));
    }
    if let Some(name) = reference.strip_prefix("files:") {
        crate::attachments::validate_name(name)
            .map_err(|error| Error::Validation(error.to_string()))?;
        return Ok(Location::Attachment(name));
    }
    if format::external_doc(reference)?.is_some() {
        return Ok(Location::External);
    }
    format::validate_line("reference", reference)?;
    if reference.ends_with('/') {
        return Err(Error::Validation(format!(
            "reference must have no trailing slash; use {}",
            reference.trim_end_matches('/')
        )));
    }
    if !format::normalized_rel(reference) || reference.contains('\0') {
        return Err(Error::Validation(format!(
            "reference {reference:?} must be a normalized repo-relative path"
        )));
    }
    Ok(Location::Path(reference))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Relation;

    #[test]
    fn citations_skip_code_spans_and_fences() {
        let body = "Use [ref:brief] here.\nQuote `[ref:shown]` or ``[ref:also]``.\nLiteral `example\n[ref:sample]` here.\n```\n[ref:fenced]\n```\n~~~md\n[ref:tilde]\n~~~\nAnd [ref:Bad Handle] and [ref:open";
        assert_eq!(
            citations(body),
            vec![
                Citation::Handle("brief"),
                Citation::Malformed("Bad Handle"),
                Citation::Malformed("open"),
            ]
        );
    }

    #[test]
    fn a_code_span_does_not_cross_a_block_boundary() {
        // The heading's backtick is literal: a heading is its own block (reviewer's case).
        assert_eq!(
            citations("# Example `\nUse `[ref:sample]` literally."),
            vec![]
        );
        // A list item boundary ends the paragraph too.
        assert_eq!(citations("- one `\n- two `[ref:sample]` x"), vec![]);
        // An indented code block is code.
        assert_eq!(
            citations("Text.\n\n    [ref:indented]\n\nAfter [ref:real]."),
            vec![Citation::Handle("real")]
        );
    }

    #[test]
    fn a_code_span_does_not_cross_a_blank_line() {
        let body = "Open `tick\n\nThen [ref:real] and `close`.";
        assert_eq!(citations(body), vec![Citation::Handle("real")]);
    }

    #[test]
    fn audit_names_duplicates_and_unresolved_citations() {
        let text = "---\nid: sci-4f2a9c\ntitle: T\nstatus: todo\npriority: 2\ncreated: 2026-08-29T14:02:11Z\nupdated: 2026-08-29T14:02:11Z\ndepends: []\ntags: []\n---\n\nSee [ref:a] and [ref:gone] and [ref:X].\n\n## References\n\n- a (context): docs/x.md\n- b (input): docs/x.md\n";
        let task = crate::format::parse_task(text, "t.md").unwrap();
        let problems = audit(&task);
        let kinds: Vec<_> = problems.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            vec![DUPLICATE, CITATION_DANGLING, CITATION_MALFORMED]
        );
        assert!(
            problems[0]
                .detail
                .contains("a and b both reference docs/x.md"),
            "{}",
            problems[0].detail
        );
    }

    #[test]
    fn an_entry_round_trips_every_field() {
        let text = "brief (input, required, kind=brief): docs/notes/b.md — admission rules, §7";
        let entry = parse_entry(text).unwrap();
        assert_eq!(entry.handle, "brief");
        assert_eq!(entry.relation, Relation::Input);
        assert!(entry.required);
        assert_eq!(entry.kind.as_deref(), Some("brief"));
        assert_eq!(entry.reference, "docs/notes/b.md");
        assert_eq!(entry.description.as_deref(), Some("admission rules, §7"));
        assert_eq!(render_entry(&entry), text);
        let bare = parse_entry("r1 (context): files:after.png").unwrap();
        assert_eq!(
            (bare.required, bare.kind.clone(), bare.description.clone()),
            (false, None, None)
        );
        assert_eq!(render_entry(&bare), "r1 (context): files:after.png");
    }

    #[test]
    fn a_reference_needing_quotes_round_trips() {
        let text = r#"rb (output, required): "docs/run books/a \"v2\" \\ — x.md" — revised here"#;
        let entry = parse_entry(text).unwrap();
        assert_eq!(entry.reference, r#"docs/run books/a "v2" \ — x.md"#);
        assert_eq!(entry.description.as_deref(), Some("revised here"));
        assert_eq!(render_entry(&entry), text);
        let ext = parse_entry(r#"t (context): "stub:a b""#).unwrap();
        assert_eq!(ext.reference, "stub:a b");
    }

    #[test]
    fn handle_boundaries_and_kind_without_required_are_supported() {
        let handle = "a".repeat(32);
        let entry =
            parse_entry(&format!("{handle} (context, kind=brief-2): results/run-12")).unwrap();
        assert_eq!(entry.handle, handle);
        assert!(!entry.required);
        assert_eq!(entry.kind.as_deref(), Some("brief-2"));
        assert!(parse_entry(&format!("a{handle} (context): a.md")).is_err());
    }

    #[test]
    fn malformed_entries_are_refused_with_the_reason() {
        for (text, needle) in [
            (" (input): a.md", "handle"),
            ("b (input, kind=Bad): a.md", "kind"),
            ("b (input, kind=): a.md", "kind"),
            ("b (input, required, required): a.md", "order"),
            ("b (input, kind=x, kind=y): a.md", "attribute"),
            ("b (input): a.md —   ", "description"),
            ("b (input): a\\b.md", "quoted"),
            ("b (input): a\"b.md", "quoted"),
            (r#"b (input): "a\nb.md""#, "escape"),
            ("b (input): /x.md", "normalized"),
            ("b (input): ./x.md", "normalized"),
            ("b (input): docs//x.md", "normalized"),
            ("b (input): stub:", "nonempty"),
            ("Brief (input): a.md", "handle"),
            ("b (uses): a.md", "relation"),
            ("b (input, pinned): a.md", "attribute"),
            ("b (input, kind=x, required): a.md", "order"),
            ("b (input): a.md trailing", " — "),
            ("b (input): a.md — ", "description"),
            ("b (input): results/run-12/", "results/run-12"),
            (
                "b (input): @lore/docs/x.md",
                "cross-project references are not supported yet",
            ),
            ("b (input): docs/../x.md", "normalized"),
            ("b (input): files:.hidden", "attachment name"),
            ("b (input): \"unterminated", "unterminated"),
        ] {
            let error = parse_entry(text).unwrap_err();
            assert!(matches!(error, Error::Validation(_)), "{text}: {error}");
            let error = error.to_string();
            assert!(error.contains(needle), "{text}: {error}");
        }
    }
}
