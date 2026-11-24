//! Format-2+ version envelope dispatch, strict Entries parsing/serialization,
//! recursive duplicate-key JSON checking, and cross-entry structural validation.

use crate::error::{Error, Result};
use crate::model::{HarnessProvenance, Task, TaskId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub const SUPPORTED_RECORD_FORMAT: u32 = 2;

/// Metadata and untouched raw bytes of a record declaring a newer format version (`> 2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FutureRecord {
    pub id: TaskId,
    pub path: String,
    pub format: u32,
    pub raw: String,
}

impl FutureRecord {
    pub fn refusal(&self) -> Error {
        Error::UnsupportedRecordVersion {
            id: self.id.to_string(),
            path: self.path.clone(),
            format: self.format,
        }
    }

    pub fn warning(&self) -> String {
        format!(
            "{} ({}) uses unsupported record format {}; skipped (unsupported_record_version)",
            self.id, self.path, self.format
        )
    }
}

/// Result of inspecting a task file's physical envelope before semantic parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordEnvelope {
    Format1,
    Format2,
    Future(FutureRecord),
}

fn perr(file: &str, detail: impl Into<String>) -> Error {
    Error::Parse {
        file: file.into(),
        detail: detail.into(),
    }
}

/// Inspect the frontmatter envelope before dispatching to a format-specific parser.
///
/// Unversioned files dispatch to Format 1. `format: 2` dispatches to Format 2.
/// Any positive integer `> 2` with a well-formed envelope and valid `id` returns
/// `RecordEnvelope::Future`, keeping raw bytes untouched for read-only inspection.
/// Malformed envelopes return `Error::Parse`.
pub fn inspect_envelope(text: &str, file: &str) -> Result<RecordEnvelope> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| perr(file, "missing opening ---"))?;
    let (fm, _) = rest
        .split_once("\n---\n")
        .ok_or_else(|| perr(file, "missing closing ---"))?;

    let mut seen_keys = HashSet::new();
    let mut raw_format: Option<&str> = None;
    let mut raw_id: Option<&str> = None;

    for (index, line) in fm.lines().enumerate() {
        let n = index + 1;
        let Some((key, value)) = line.split_once(':') else {
            return Err(perr(file, format!("line {n}: expected `key: value`")));
        };
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Err(perr(file, format!("line {n}: bad key {key:?}")));
        }
        let Some(value) = value.strip_prefix(' ') else {
            return Err(perr(
                file,
                format!("line {n}: expected one space after `{key}:`"),
            ));
        };
        if value.is_empty() {
            return Err(perr(
                file,
                format!("line {n}: empty value for {key}; use \"\""),
            ));
        }
        if !seen_keys.insert(key) {
            return Err(perr(file, format!("line {n}: duplicate key {key}")));
        }
        if key == "format" {
            raw_format = Some(value);
        } else if key == "id" {
            raw_id = Some(value);
        }
    }

    let Some(format_str) = raw_format else {
        return Ok(RecordEnvelope::Format1);
    };

    if format_str.starts_with('0')
        || format_str.is_empty()
        || !format_str.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(perr(
            file,
            format!("format must be an integer >= 2, got {format_str:?}"),
        ));
    }
    let version: u32 = format_str.parse().map_err(|_| {
        perr(
            file,
            format!("format must be an integer >= 2, got {format_str:?}"),
        )
    })?;
    if version < 2 {
        return Err(perr(
            file,
            format!("format header must be >= 2 (format 1 is unversioned), got {version}"),
        ));
    }
    if version == SUPPORTED_RECORD_FORMAT {
        return Ok(RecordEnvelope::Format2);
    }

    let id_raw = raw_id.ok_or_else(|| perr(file, "future-format record is missing id"))?;
    let id_unquoted = if id_raw.starts_with('"') {
        let (s, after) = crate::frontmatter::take_quoted(id_raw).map_err(|d| perr(file, d))?;
        if !after.is_empty() {
            return Err(perr(
                file,
                format!("unexpected text after quoted id: {after:?}"),
            ));
        }
        s
    } else {
        id_raw.to_string()
    };
    let id = TaskId::parse(&id_unquoted).map_err(|e| perr(file, e.to_string()))?;

    Ok(RecordEnvelope::Future(FutureRecord {
        id,
        path: file.to_string(),
        format: version,
        raw: text.to_string(),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionKind {
    Drop,
    Suggestion,
    Question,
    Approval,
}

impl DecisionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Drop => "drop",
            Self::Suggestion => "suggestion",
            Self::Question => "question",
            Self::Approval => "approval",
        }
    }

    #[allow(dead_code)]
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "drop" => Ok(Self::Drop),
            "suggestion" => Ok(Self::Suggestion),
            "question" => Ok(Self::Question),
            "approval" => Ok(Self::Approval),
            _ => Err(Error::Validation(format!(
                "unknown decision kind {s:?}; expected one of drop, suggestion, question, approval"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionOutcome {
    Accept,
    Decline,
    Reply,
    Withdraw,
}

impl ResolutionOutcome {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Decline => "decline",
            Self::Reply => "reply",
            Self::Withdraw => "withdraw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Subject {
    Task(TaskSubject),
    Scope(ScopeSubject),
    CommitPin(CommitPinSubject),
    DigestPin(DigestPinSubject),
    Entry(EntrySubject),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSubject {
    pub task: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeSubject {
    pub scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitPinSubject {
    pub repo: String,
    pub path: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DigestPinSubject {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntrySubject {
    pub entry: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionData {
    pub kind: DecisionKind,
    pub origin: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_verdict: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionData {
    pub outcome: ResolutionOutcome,
    pub text: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen_scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryPayload {
    Decision(DecisionData),
    Resolution(ResolutionData),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub at: String,
    pub by: String,
    pub subject: Subject,
    pub payload: EntryPayload,
    pub provenance: Option<HarnessProvenance>,
}

impl Entry {
    #[allow(dead_code)]
    pub fn entry_type(&self) -> &'static str {
        match &self.payload {
            EntryPayload::Decision(_) => "decision",
            EntryPayload::Resolution(_) => "resolution",
        }
    }

    #[allow(dead_code)]
    pub fn as_decision(&self) -> Option<&DecisionData> {
        match &self.payload {
            EntryPayload::Decision(d) => Some(d),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_resolution(&self) -> Option<&ResolutionData> {
        match &self.payload {
            EntryPayload::Resolution(r) => Some(r),
            _ => None,
        }
    }
}

/// Strictly validates a lowercase RFC 4122 UUIDv4 string (`8-4-4-4-12`).
pub fn is_valid_uuidv4(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    for (i, &byte) in b.iter().enumerate() {
        if matches!(i, 8 | 13 | 18 | 23) {
            if byte != b'-' {
                return false;
            }
        } else if !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            return false;
        }
    }
    b[14] == b'4' && matches!(b[19], b'8' | b'9' | b'a' | b'b')
}

/// Normalizes ASCII case and validates a caller-supplied UUIDv4.
#[allow(dead_code)]
pub fn parse_uuidv4(s: &str) -> Result<String> {
    let lower = s.to_ascii_lowercase();
    if !is_valid_uuidv4(&lower) {
        return Err(Error::Validation(format!(
            "invalid UUIDv4 {s:?}; expected 8-4-4-4-12 hex with version 4 and RFC 4122 variant"
        )));
    }
    Ok(lower)
}

/// Generates a random lowercase RFC 4122 UUIDv4 using OS randomness.
#[allow(dead_code)]
pub fn new_uuidv4() -> Result<String> {
    let mut bytes = [0u8; 16];
    let mut filled = 0;
    while filled < bytes.len() {
        // SAFETY: `bytes` is a valid mutable buffer of length 16.
        let n = unsafe {
            libc::getrandom(
                bytes[filled..].as_mut_ptr().cast::<libc::c_void>(),
                bytes.len() - filled,
                0,
            )
        };
        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err.into());
        }
        filled += n as usize;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    ))
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_git_commit_hex(s: &str) -> bool {
    (s.len() == 40 || s.len() == 64)
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_multiline_text(field: &str, text: &str) -> std::result::Result<(), String> {
    if text.trim().is_empty() {
        return Err(format!("{field} must not be empty or blank"));
    }
    if text.contains('\0') || text.contains('\r') {
        return Err(format!("{field} must not contain NUL or CR characters"));
    }
    Ok(())
}

fn validate_single_line_text(field: &str, text: &str) -> std::result::Result<(), String> {
    if text.trim().is_empty() {
        return Err(format!("{field} must not be empty or blank"));
    }
    if text.chars().any(char::is_control) {
        return Err(format!(
            "{field} must be a single line with no control characters"
        ));
    }
    Ok(())
}

fn validate_artifact_path(path: &str) -> std::result::Result<(), String> {
    validate_single_line_text("artifact path", path)?;
    if path.starts_with('/') || !crate::format::normalized_rel(path) {
        return Err(format!(
            "artifact path {path:?} must be a normalized relative path"
        ));
    }
    Ok(())
}

fn validate_subject(subject: &Subject) -> std::result::Result<(), String> {
    match subject {
        Subject::Task(t) => {
            if !t.task {
                return Err("task subject must be {\"task\":true}".into());
            }
        }
        Subject::Scope(s) => {
            if let Some(digest) = &s.scope
                && !is_sha256_hex(digest)
            {
                return Err(format!(
                    "scope subject digest {digest:?} must be 64 lowercase hex characters"
                ));
            }
        }
        Subject::CommitPin(p) => {
            validate_single_line_text("artifact repo", &p.repo)?;
            validate_artifact_path(&p.path)?;
            if !is_git_commit_hex(&p.commit) {
                return Err(format!(
                    "artifact commit {:?} must be a 40- or 64-character lowercase hex commit id",
                    p.commit
                ));
            }
        }
        Subject::DigestPin(p) => {
            validate_artifact_path(&p.path)?;
            if !is_sha256_hex(&p.sha256) {
                return Err(format!(
                    "artifact sha256 {:?} must be 64 lowercase hex characters",
                    p.sha256
                ));
            }
        }
        Subject::Entry(e) => {
            if !is_valid_uuidv4(&e.entry) {
                return Err(format!(
                    "entry subject {:?} must be a lowercase UUIDv4",
                    e.entry
                ));
            }
        }
    }
    Ok(())
}

/// Strict RFC 8259 JSON validator rejecting duplicate object keys at every nesting depth.
pub fn validate_no_duplicate_json_keys(input: &str) -> std::result::Result<(), String> {
    let mut parser = JsonDupChecker {
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    parser.parse_value(0)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(format!(
            "unexpected trailing JSON content at byte {}",
            parser.pos
        ));
    }
    Ok(())
}

struct JsonDupChecker<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonDupChecker<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self, depth: usize) -> std::result::Result<(), String> {
        if depth > 64 {
            return Err("JSON nesting depth exceeded".into());
        }
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(depth + 1),
            Some(b'[') => self.parse_array(depth + 1),
            Some(b'"') => self.parse_string().map(|_| ()),
            Some(b't') => self.expect_literal(b"true"),
            Some(b'f') => self.expect_literal(b"false"),
            Some(b'n') => self.expect_literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(other) => Err(format!(
                "unexpected JSON byte {:?} at {}",
                other as char, self.pos
            )),
            None => Err("unexpected end of JSON".into()),
        }
    }

    fn parse_object(&mut self, depth: usize) -> std::result::Result<(), String> {
        self.pos += 1; // '{'
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(format!("expected JSON object key at byte {}", self.pos));
            }
            let key = self.parse_string()?;
            if !keys.insert(key.clone()) {
                return Err(format!("duplicate JSON key {key:?}"));
            }
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("expected ':' after key {key:?}"));
            }
            self.pos += 1;
            self.parse_value(depth)?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => return Err(format!("expected ',' or '}}' at byte {}", self.pos)),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> std::result::Result<(), String> {
        self.pos += 1; // '['
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(());
        }
        loop {
            self.parse_value(depth)?;
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(());
                }
                _ => return Err(format!("expected ',' or ']' at byte {}", self.pos)),
            }
        }
    }

    fn parse_string(&mut self) -> std::result::Result<String, String> {
        let start = self.pos;
        self.pos += 1; // '"'
        while let Some(b) = self.peek() {
            match b {
                b'"' => {
                    self.pos += 1;
                    let slice = std::str::from_utf8(&self.bytes[start..self.pos])
                        .map_err(|e| e.to_string())?;
                    return serde_json::from_str::<String>(slice)
                        .map_err(|e| format!("invalid JSON string: {e}"));
                }
                b'\\' => {
                    self.pos += 1;
                    if self.pos >= self.bytes.len() {
                        return Err("unterminated escape in JSON string".into());
                    }
                    self.pos += 1;
                }
                0x00..=0x1f => return Err("unescaped control character in JSON string".into()),
                _ => self.pos += 1,
            }
        }
        Err("unterminated JSON string".into())
    }

    fn expect_literal(&mut self, lit: &[u8]) -> std::result::Result<(), String> {
        if self.bytes[self.pos..].starts_with(lit) {
            self.pos += lit.len();
            Ok(())
        } else {
            Err(format!("invalid JSON literal at byte {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> std::result::Result<(), String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(format!("invalid JSON number at byte {start}")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(format!("invalid JSON fraction at byte {start}"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(format!("invalid JSON exponent at byte {start}"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    #[serde(rename = "type")]
    entry_type: String,
    at: String,
    by: String,
    subject: serde_json::Value,
    data: serde_json::Value,
    #[serde(default)]
    provenance: Option<HarnessProvenance>,
}

#[derive(Serialize)]
struct PhysicalEntryWire<'a, D: Serialize> {
    #[serde(rename = "type")]
    entry_type: &'static str,
    at: &'a str,
    by: &'a str,
    subject: &'a Subject,
    data: &'a D,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<&'a HarnessProvenance>,
}

/// Parse one `UUID SPACE JSON-object` line from a format-2 `## Entries` section.
pub fn parse_entry_line(line: &str) -> std::result::Result<Entry, String> {
    let Some((id, json)) = line.split_once(' ') else {
        return Err(format!(
            "malformed entry line {line:?}: expected `UUID JSON`"
        ));
    };
    if !is_valid_uuidv4(id) {
        return Err(format!("entry id {id:?} must be a lowercase UUIDv4"));
    }
    if !json.starts_with('{') {
        return Err(format!("entry {id} payload must be a JSON object"));
    }
    validate_no_duplicate_json_keys(json).map_err(|e| format!("entry {id}: {e}"))?;
    let raw: RawEntry =
        serde_json::from_str(json).map_err(|e| format!("entry {id} invalid JSON: {e}"))?;
    let subject: Subject = serde_json::from_value(raw.subject)
        .map_err(|e| format!("entry {id} invalid subject: {e}"))?;
    let payload = match raw.entry_type.as_str() {
        "decision" => {
            let data: DecisionData = serde_json::from_value(raw.data)
                .map_err(|e| format!("entry {id} invalid decision data: {e}"))?;
            EntryPayload::Decision(data)
        }
        "resolution" => {
            let data: ResolutionData = serde_json::from_value(raw.data)
                .map_err(|e| format!("entry {id} invalid resolution data: {e}"))?;
            EntryPayload::Resolution(data)
        }
        other => {
            return Err(format!(
                "entry {id} has unsupported type {other:?} for format 2"
            ));
        }
    };
    Ok(Entry {
        id: id.to_string(),
        at: raw.at,
        by: raw.by,
        subject,
        payload,
        provenance: raw.provenance,
    })
}

/// Serialize one entry into its canonical single-line `UUID SPACE JSON-object` form.
pub fn serialize_entry_line(entry: &Entry) -> String {
    let json = match &entry.payload {
        EntryPayload::Decision(data) => serde_json::to_string(&PhysicalEntryWire {
            entry_type: "decision",
            at: &entry.at,
            by: &entry.by,
            subject: &entry.subject,
            data,
            provenance: entry.provenance.as_ref(),
        })
        .expect("decision entry serializes"),
        EntryPayload::Resolution(data) => serde_json::to_string(&PhysicalEntryWire {
            entry_type: "resolution",
            at: &entry.at,
            by: &entry.by,
            subject: &entry.subject,
            data,
            provenance: entry.provenance.as_ref(),
        })
        .expect("resolution entry serializes"),
    };
    format!("{} {}", entry.id, json)
}

/// Validate all entries and `open_decisions` pointers for a supported record format.
pub fn validate_entries(format: u8, open_decisions: &[String], entries: &[Entry]) -> Result<()> {
    if format < 2 {
        if !open_decisions.is_empty() || !entries.is_empty() {
            return Err(Error::Validation(
                "format 1 records cannot carry open_decisions or Entries".into(),
            ));
        }
        return Ok(());
    }

    let mut seen_ids = HashSet::new();
    let mut decisions: BTreeMap<&str, (&DecisionData, &Subject)> = BTreeMap::new();
    for entry in entries {
        if !is_valid_uuidv4(&entry.id) {
            return Err(Error::Validation(format!(
                "entry id {:?} must be a lowercase UUIDv4",
                entry.id
            )));
        }
        if !seen_ids.insert(entry.id.as_str()) {
            return Err(Error::Validation(format!(
                "duplicate entry id {}",
                entry.id
            )));
        }
        crate::time::parse(&entry.at)?;
        validate_single_line_text("entry by", &entry.by).map_err(Error::Validation)?;
        if let Some(prov) = &entry.provenance {
            crate::provenance::validate(prov).map_err(Error::Validation)?;
        }
        validate_subject(&entry.subject).map_err(Error::Validation)?;
        if let EntryPayload::Decision(data) = &entry.payload {
            validate_single_line_text("decision origin", &data.origin)
                .map_err(Error::Validation)?;
            validate_multiline_text("decision text", &data.text).map_err(Error::Validation)?;
            if let Some(source) = &data.source {
                validate_single_line_text("decision source", source).map_err(Error::Validation)?;
            }
            if let Some(verdict) = &data.scope_verdict {
                validate_single_line_text("decision scope_verdict", verdict)
                    .map_err(Error::Validation)?;
            }
            let subject_ok = match data.kind {
                DecisionKind::Drop | DecisionKind::Suggestion => {
                    matches!(entry.subject, Subject::Scope(_))
                }
                DecisionKind::Question => matches!(
                    entry.subject,
                    Subject::Scope(_) | Subject::CommitPin(_) | Subject::DigestPin(_)
                ),
                DecisionKind::Approval => {
                    matches!(entry.subject, Subject::CommitPin(_) | Subject::DigestPin(_))
                }
            };
            if !subject_ok {
                return Err(Error::Validation(format!(
                    "entry {}: subject is incompatible with decision kind {}",
                    entry.id,
                    data.kind.as_str()
                )));
            }
            decisions.insert(entry.id.as_str(), (data, &entry.subject));
        }
    }

    let mut resolved_targets = BTreeSet::new();
    for entry in entries {
        let EntryPayload::Resolution(data) = &entry.payload else {
            continue;
        };
        let Subject::Entry(target) = &entry.subject else {
            return Err(Error::Validation(format!(
                "resolution {} must target an entry subject",
                entry.id
            )));
        };
        let Some((decision_data, _)) = decisions.get(target.entry.as_str()) else {
            return Err(Error::Validation(format!(
                "resolution {} targets unknown or non-decision entry {}",
                entry.id, target.entry
            )));
        };
        if !resolved_targets.insert(target.entry.as_str()) {
            return Err(Error::Validation(format!(
                "decision {} has duplicate or contradictory resolutions",
                target.entry
            )));
        }
        validate_multiline_text("resolution text", &data.text).map_err(Error::Validation)?;
        validate_single_line_text("resolution source", &data.source).map_err(Error::Validation)?;
        let outcome_ok = match decision_data.kind {
            DecisionKind::Drop | DecisionKind::Suggestion | DecisionKind::Approval => matches!(
                data.outcome,
                ResolutionOutcome::Accept
                    | ResolutionOutcome::Decline
                    | ResolutionOutcome::Withdraw
            ),
            DecisionKind::Question => matches!(
                data.outcome,
                ResolutionOutcome::Reply | ResolutionOutcome::Withdraw
            ),
        };
        if !outcome_ok {
            return Err(Error::Validation(format!(
                "resolution {} outcome {:?} is incompatible with {} decision {}",
                entry.id,
                data.outcome,
                decision_data.kind.as_str(),
                target.entry
            )));
        }
        let needs_seen_scope =
            decision_data.kind == DecisionKind::Drop && data.outcome == ResolutionOutcome::Accept;
        match (&data.seen_scope, needs_seen_scope) {
            (Some(token), true) => {
                if !is_sha256_hex(token) {
                    return Err(Error::Validation(format!(
                        "resolution {} seen_scope {token:?} must be 64 lowercase hex characters",
                        entry.id
                    )));
                }
            }
            (None, true) => {
                return Err(Error::Validation(format!(
                    "resolution {} accepting drop {} requires seen_scope",
                    entry.id, target.entry
                )));
            }
            (Some(_), false) => {
                return Err(Error::Validation(format!(
                    "resolution {} carries seen_scope, which is only valid when accepting a drop",
                    entry.id
                )));
            }
            (None, false) => {}
        }
    }

    let mut open_set = BTreeSet::new();
    for ptr in open_decisions {
        if !is_valid_uuidv4(ptr) {
            return Err(Error::Validation(format!(
                "open_decisions pointer {ptr:?} must be a lowercase UUIDv4"
            )));
        }
        if !open_set.insert(ptr.as_str()) {
            return Err(Error::Validation(format!(
                "duplicate open_decisions pointer {ptr}"
            )));
        }
        if !decisions.contains_key(ptr.as_str()) {
            return Err(Error::Validation(format!(
                "open_decisions pointer {ptr} does not reference a decision entry"
            )));
        }
        if resolved_targets.contains(ptr.as_str()) {
            return Err(Error::Validation(format!(
                "open_decisions pointer {ptr} references an already resolved decision"
            )));
        }
    }

    for &decision_id in decisions.keys() {
        if !resolved_targets.contains(decision_id) && !open_set.contains(decision_id) {
            return Err(Error::Validation(format!(
                "unresolved decision {decision_id} is missing from open_decisions"
            )));
        }
    }

    Ok(())
}

/// Validates and performs an explicit format-1 -> format-2 upgrade before saving.
/// Protects a format-1 body containing `## Entries` and a format-1 idea carrying
/// `awaiting_user: true` from incidental conversion.
#[allow(dead_code)]
pub fn upgrade_to_format_2(task: &mut Task, replace_awaiting_user: bool) -> Result<()> {
    if task.format >= 2 {
        return Ok(());
    }
    if task
        .body
        .lines()
        .any(|line| line == crate::format::ENTRIES_DELIMITER)
    {
        return Err(Error::Validation(format!(
            "{} body contains reserved heading `{}`; resolve or remove the heading before upgrading to format 2",
            task.id,
            crate::format::ENTRIES_DELIMITER
        )));
    }
    if task.awaiting_user && !replace_awaiting_user {
        return Err(Error::Validation(format!(
            "{} is a format-1 record with awaiting_user: true; resolve or clear the legacy wait in format 1 (or convert a confirmed matching question with --replace-awaiting-user) before upgrading to format 2",
            task.id
        )));
    }
    if replace_awaiting_user {
        task.awaiting_user = false;
    }
    task.format = 2;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const D1: &str = "6f76bdab-e417-4ae7-81ce-09fc70d6487a";
    const D2: &str = "1b4e28ba-2fa1-4d2f-9c83-2d3f9c0a1b2c";
    const R1: &str = "9a3c4d5e-6f70-4812-a345-6789abcdef01";
    const R2: &str = "8a3c4d5e-6f70-4812-a345-6789abcdef02";
    const HASH: &str = "ff13e9569c2bc118b1f6e76f89b964557b3eae8d66ff8ad3174849517704145f";
    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn envelope_dispatches_format_1_format_2_future_and_rejects_malformed() {
        let f1 = "---\nid: tst-000001\ntitle: T\nstatus: todo\npriority: 2\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\ndepends: []\ntags: []\n---\n";
        assert_eq!(
            inspect_envelope(f1, "tasks/tst-000001.md").unwrap(),
            RecordEnvelope::Format1
        );

        let f2 = format!("---\nformat: 2\n{}", f1.strip_prefix("---\n").unwrap());
        assert_eq!(
            inspect_envelope(&f2, "tasks/tst-000001.md").unwrap(),
            RecordEnvelope::Format2
        );

        let f3 = "---\nformat: 3\nid: tst-000001\nassessment: 6f76bdab-e417-4ae7-81ce-09fc70d6487a\n---\nbody\n";
        match inspect_envelope(f3, "tasks/tst-000001.md").unwrap() {
            RecordEnvelope::Future(fut) => {
                assert_eq!(fut.id.to_string(), "tst-000001");
                assert_eq!(fut.format, 3);
                assert_eq!(fut.path, "tasks/tst-000001.md");
                assert_eq!(fut.raw, f3);
            }
            other => panic!("expected Future, got {other:?}"),
        }

        for bad in [
            "---\nformat: 1\nid: tst-000001\n---\n",
            "---\nformat: 0\nid: tst-000001\n---\n",
            "---\nformat: \"2\"\nid: tst-000001\n---\n",
            "---\nformat: 3\n---\n",
            "---\nformat: 3\nid: bad-id\n---\n",
            "---\nformat: 3\nformat: 3\nid: tst-000001\n---\n",
        ] {
            assert!(inspect_envelope(bad, "t.md").is_err(), "{bad}");
        }
    }

    #[test]
    fn duplicate_json_key_checker_catches_nested_and_unicode_escaped_duplicates() {
        assert!(validate_no_duplicate_json_keys(r#"{"a":1,"b":{"c":2,"d":[3,{"e":4}]}}"#).is_ok());
        for bad in [
            r#"{"a":1,"a":2}"#,
            r#"{"a":{"x":1,"x":2}}"#,
            r#"{"a":1,"\u0061":2}"#,
            r#"{"arr":[{"k":1,"k":2}]}"#,
            r#"{"a":1} trailing"#,
        ] {
            assert!(validate_no_duplicate_json_keys(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn uuidv4_validator_and_generator() {
        assert!(is_valid_uuidv4(D1));
        assert!(!is_valid_uuidv4(&D1.to_uppercase()));
        assert!(!is_valid_uuidv4("6f76bdab-e417-1ae7-81ce-09fc70d6487a")); // v1
        assert!(!is_valid_uuidv4("6f76bdab-e417-4ae7-01ce-09fc70d6487a")); // bad variant
        assert_eq!(parse_uuidv4(&D1.to_uppercase()).unwrap(), D1);
        let generated = new_uuidv4().unwrap();
        assert!(is_valid_uuidv4(&generated), "generated {generated}");
    }

    #[test]
    fn validates_decisions_resolutions_pins_and_pointer_consistency() {
        let drop_line = format!(
            "{D1} {{\"type\":\"decision\",\"at\":\"2026-11-22T08:00:00Z\",\"by\":\"scope\",\"subject\":{{\"scope\":\"{HASH}\"}},\"data\":{{\"kind\":\"drop\",\"origin\":\"scope\",\"text\":\"Line 1\\nLine 2\"}}}}"
        );
        let drop_entry = parse_entry_line(&drop_line).unwrap();
        assert_eq!(serialize_entry_line(&drop_entry), drop_line);
        validate_entries(2, &[D1.into()], std::slice::from_ref(&drop_entry)).unwrap();

        // Missing from open_decisions fails
        assert!(validate_entries(2, &[], std::slice::from_ref(&drop_entry)).is_err());

        // Accept drop requires seen_scope
        let accept_missing_scope = parse_entry_line(&format!(
            "{R1} {{\"type\":\"resolution\",\"at\":\"2026-11-23T08:00:00Z\",\"by\":\"human\",\"subject\":{{\"entry\":\"{D1}\"}},\"data\":{{\"outcome\":\"accept\",\"text\":\"Yes\",\"source\":\"direct\"}}}}"
        )).unwrap();
        assert!(validate_entries(2, &[], &[drop_entry.clone(), accept_missing_scope]).is_err());

        let accept_ok = parse_entry_line(&format!(
            "{R1} {{\"type\":\"resolution\",\"at\":\"2026-11-23T08:00:00Z\",\"by\":\"human\",\"subject\":{{\"entry\":\"{D1}\"}},\"data\":{{\"outcome\":\"accept\",\"text\":\"Yes\",\"source\":\"direct\",\"seen_scope\":\"{HASH}\"}}}}"
        )).unwrap();
        validate_entries(2, &[], &[drop_entry.clone(), accept_ok.clone()]).unwrap();

        // Still in open_decisions after resolution fails
        assert!(
            validate_entries(2, &[D1.into()], &[drop_entry.clone(), accept_ok.clone()]).is_err()
        );

        // Duplicate resolution fails
        let decline_dup = parse_entry_line(&format!(
            "{R2} {{\"type\":\"resolution\",\"at\":\"2026-11-23T09:00:00Z\",\"by\":\"human\",\"subject\":{{\"entry\":\"{D1}\"}},\"data\":{{\"outcome\":\"decline\",\"text\":\"No\",\"source\":\"direct\"}}}}"
        )).unwrap();
        assert!(validate_entries(2, &[], &[drop_entry.clone(), accept_ok, decline_dup]).is_err());

        // Incompatible subject: approval cannot take scope
        let bad_approval = parse_entry_line(&format!(
            "{D2} {{\"type\":\"decision\",\"at\":\"2026-11-22T08:00:00Z\",\"by\":\"author\",\"subject\":{{\"scope\":\"{HASH}\"}},\"data\":{{\"kind\":\"approval\",\"origin\":\"workflow\",\"text\":\"Ok?\"}}}}"
        )).unwrap();
        assert!(validate_entries(2, &[D2.into()], &[bad_approval]).is_err());

        // Valid approval with commit pin and valid question with digest pin
        let ok_approval = parse_entry_line(&format!(
            "{D2} {{\"type\":\"decision\",\"at\":\"2026-11-22T08:00:00Z\",\"by\":\"author\",\"subject\":{{\"repo\":\"records\",\"path\":\"tasks/docs/spec.md\",\"commit\":\"{COMMIT}\"}},\"data\":{{\"kind\":\"approval\",\"origin\":\"workflow\",\"text\":\"Ok?\"}}}}"
        )).unwrap();
        validate_entries(2, &[D2.into()], &[ok_approval]).unwrap();
    }

    #[test]
    fn upgrade_protects_reserved_heading_and_awaiting_user_flag() {
        let mut t = crate::history::fixture(
            "---\nid: tst-000001\ntitle: Idea\nstatus: idea\npriority: 2\nreviewed: true\nawaiting_user: true\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\ndepends: []\ntags: []\n---\n\nBody.\n",
            "t.md",
        )
        .unwrap();
        assert_eq!(t.format, 1);
        let err = upgrade_to_format_2(&mut t, false).unwrap_err();
        assert!(err.to_string().contains("awaiting_user"), "{err}");
        assert_eq!(t.format, 1);
        assert!(t.awaiting_user);

        // Confirmed replacement clears awaiting_user and upgrades to format 2
        upgrade_to_format_2(&mut t, true).unwrap();
        assert_eq!(t.format, 2);
        assert!(!t.awaiting_user);

        // Reserved heading in format-1 body refuses upgrade even without awaiting_user
        let mut t2 = crate::history::fixture(
            "---\nid: tst-000002\ntitle: T\nstatus: todo\npriority: 2\ncreated: 2026-11-20T00:00:00Z\nupdated: 2026-11-20T00:00:00Z\ndepends: []\ntags: []\n---\n\nIntro\n\n## Entries\n\nLiteral text.\n",
            "t2.md",
        )
        .unwrap();
        let err2 = upgrade_to_format_2(&mut t2, false).unwrap_err();
        assert!(err2.to_string().contains("## Entries"), "{err2}");
        assert_eq!(t2.format, 1);
    }
}
