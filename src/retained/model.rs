use crate::error::{Error, Result};
use serde::de::{DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

pub fn invalid(detail: impl fmt::Display) -> Error {
    Error::RetainedInput(detail.to_string())
}

// serde_json's ordinary map reader overwrites duplicate keys, including nested ones.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("strict JSON")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(n.into()))
                    .ok_or_else(|| E::custom("nonfinite number"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_string<E: serde::de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut v = Vec::new();
                while let Some(Unique(x)) = a.next_element()? {
                    v.push(x)
                }
                Ok(Unique(v.into()))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut a: A,
            ) -> std::result::Result<Unique, A::Error> {
                let mut v = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if v.contains_key(&k) {
                        return Err(serde::de::Error::custom(format!("duplicate key {k}")));
                    }
                    let Unique(x) = a.next_value()?;
                    v.insert(k, x);
                }
                Ok(Unique(v.into()))
            }
        }
        d.deserialize_any(V)
    }
}
pub fn strict(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice::<Unique>(bytes)
        .map(|v| v.0)
        .map_err(invalid)
}
pub fn decode_request<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    serde_json::from_value(strict(bytes)?).map_err(invalid)
}
pub fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn text(s: &str) -> bool {
    !s.is_empty()
}
pub fn line(s: &str) -> bool {
    text(s) && !s.contains(['\0', '\r', '\n'])
}
pub fn location(s: &str) -> bool {
    line(s)
        && [("/", 1), ("$WORK_ROOT/", 11)]
            .iter()
            .any(|(p, n)| s.starts_with(p) && s.len() > *n)
}
pub fn member_path(s: &str) -> bool {
    line(s)
        && !s.contains('\\')
        && !s.starts_with('/')
        && !(s.as_bytes().get(1) == Some(&b':') && s.as_bytes()[0].is_ascii_alphabetic())
        && s.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}
fn task_id(s: &str) -> bool {
    crate::model::TaskId::parse(s).is_ok()
}
fn owner(s: &str) -> bool {
    s.strip_prefix("task:").is_some_and(task_id)
        || s.strip_prefix("session:")
            .is_some_and(|v| !v.is_empty() && !v.chars().any(char::is_whitespace))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FieldString {
    #[default]
    Absent,
    Null,
    Text(String),
}
impl FieldString {
    pub fn absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
    pub fn value(&self) -> Option<&str> {
        if let Self::Text(s) = self {
            Some(s)
        } else {
            None
        }
    }
}
impl Serialize for FieldString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Self::Text(v) => s.serialize_str(v),
            _ => s.serialize_none(),
        }
    }
}
impl<'de> Deserialize<'de> for FieldString {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Ok(match Option::<String>::deserialize(d)? {
            Some(s) => Self::Text(s),
            None => Self::Null,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub host: String,
    pub kind: String,
    pub path: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u8>,
    pub host: String,
    pub kind: String,
    pub path: String,
    pub state: String,
    pub verification: String,
    pub purpose: String,
    pub next_owner: String,
    pub cleanup_trigger: String,
    #[serde(default, skip_serializing_if = "FieldString::absent")]
    pub sha256: FieldString,
    #[serde(default, skip_serializing_if = "FieldString::absent")]
    pub manifest: FieldString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_action: Option<String>,
}
impl RetentionRecord {
    pub fn identity(&self) -> Identity {
        Identity {
            host: self.host.clone(),
            kind: self.kind.clone(),
            path: self.path.clone(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.version.is_some_and(|v| v != 1) {
            return Err(invalid("unsupported_version"));
        }
        if !text(&self.host)
            || !text(&self.purpose)
            || !["worktree", "scratch", "evidence"].contains(&self.kind.as_str())
        {
            return Err(invalid("record fields"));
        }
        let path = if self.kind == "worktree" {
            line(&self.path)
                && ((self.path.starts_with(".worktrees/") && self.path.len() > 11)
                    || (self.path.starts_with('/') && self.path.len() > 1))
        } else {
            location(&self.path)
        };
        if !path {
            return Err(invalid("record path"));
        }
        if self.state == "released" {
            if self.verification != "verified"
                || self.next_owner != "none"
                || self.cleanup_trigger != "none"
                || !self.sha256.absent()
                || !self.manifest.absent()
                || self.handoff.is_some()
                || self.verification_action.is_some()
            {
                return Err(invalid("released fields"));
            }
            return Ok(());
        }
        if self.state != "kept" || !line(&self.cleanup_trigger) || self.cleanup_trigger == "none" {
            return Err(invalid("kept fields"));
        }
        if self.next_owner == "user" {
            if !self.handoff.as_deref().is_some_and(text) {
                return Err(invalid("missing user handoff"));
            }
        } else if !owner(&self.next_owner) || self.handoff.is_some() {
            return Err(invalid("owner"));
        }
        if self.kind == "worktree" && (!self.sha256.absent() || !self.manifest.absent()) {
            return Err(invalid("worktree digest"));
        }
        match self.verification.as_str() {
            "verified" => {
                if self.verification_action.is_some() {
                    return Err(invalid("resolved action"));
                }
                if self.kind != "worktree" && !self.sha256.value().is_some_and(digest) {
                    return Err(invalid("missing digest"));
                }
                if self.kind == "scratch" && !self.manifest.value().is_some_and(location) {
                    return Err(invalid("missing manifest"));
                }
                if !self.manifest.absent() && !self.manifest.value().is_some_and(location) {
                    return Err(invalid("manifest"));
                }
            }
            "unknown" => {
                if !self.verification_action.as_deref().is_some_and(text) {
                    return Err(invalid("unknown action"));
                }
                if self.kind != "worktree" && self.sha256 != FieldString::Null {
                    return Err(invalid("unknown current digest"));
                }
                if !self.manifest.absent()
                    && self.manifest != FieldString::Null
                    && !self.manifest.value().is_some_and(location)
                {
                    return Err(invalid("unknown manifest"));
                }
            }
            _ => return Err(invalid("verification")),
        }
        Ok(())
    }
}
pub fn decode_record(raw: &str) -> Result<RetentionRecord> {
    let v = strict(raw.as_bytes())?;
    if v.get("version").is_some_and(|v| v.as_u64() != Some(1)) {
        return Err(invalid("unsupported_version"));
    }
    for key in ["handoff", "verification_action"] {
        if v.get(key).is_some_and(|v| !v.is_string()) {
            return Err(invalid(format!("{key} must be text")));
        }
    }
    let r: RetentionRecord = serde_json::from_value(v).map_err(invalid)?;
    r.validate()?;
    Ok(r)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg(test)]
pub struct Manifest {
    pub version: u8,
    pub entries: Vec<ManifestEntry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[cfg(test)]
pub enum ManifestEntry {
    File { path: String, sha256: String },
    Directory { path: String },
    Symlink { path: String, target: String },
}
#[cfg(test)]
impl ManifestEntry {
    pub fn path(&self) -> &str {
        match self {
            Self::File { path, .. } | Self::Directory { path } | Self::Symlink { path, .. } => path,
        }
    }
}
#[cfg(test)]
pub fn decode_manifest(raw: &str) -> Result<Manifest> {
    let m: Manifest = decode_request(raw.as_bytes())?;
    if m.version != 1
        || m.entries.iter().any(|e| {
            !member_path(e.path())
                || match e {
                    ManifestEntry::File { sha256, .. } => !digest(sha256),
                    ManifestEntry::Symlink { target, .. } => !text(target),
                    _ => false,
                }
        })
    {
        return Err(invalid("manifest structure"));
    }
    Ok(m)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    pub project: String,
    pub layout: String,
    pub records_subdir: String,
    pub commit: String,
    pub task_path: String,
    pub retained_ref: String,
}
impl Basis {
    pub fn validate(&self) -> Result<()> {
        if !crate::model::is_valid_prefix(&self.project)
            || !["in-tree", "detached", "linked"].contains(&self.layout.as_str())
            || !(self.records_subdir == "." || member_path(&self.records_subdir))
            || !member_path(&self.task_path)
            || self.commit.len() != 40
            || !self
                .commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || !self.retained_ref.starts_with("refs/heads/")
            || !line(&self.retained_ref)
        {
            return Err(invalid("basis"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub basis: Basis,
    pub ordinal: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub task: String,
    pub basis: Basis,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationKey {
    pub coordinator: String,
    pub operation_id: String,
}
impl OperationKey {
    pub fn validate(&self) -> Result<()> {
        if !task_id(&self.coordinator)
            || self.operation_id.len() != 32
            || !self
                .operation_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Err(invalid("operation key"))
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArtifactLocator {
    Git {
        basis: Basis,
        path: String,
        sha256: String,
    },
    Attachment {
        task: String,
        name: String,
        basis: Basis,
        sha256: String,
    },
    Directory {
        host: String,
        path: String,
        manifest: Box<ArtifactLocator>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    pub kind: String,
    pub path: String,
    pub purpose: String,
    pub cleanup_trigger: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preservation {
    pub identity: Identity,
    pub artifact: ArtifactLocator,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub bad: Source,
    pub declarations: Vec<Declaration>,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointRequest {
    pub version: u8,
    pub operation_id: String,
    pub host: String,
    pub inputs: Vec<Input>,
    #[serde(default)]
    pub declarations: Vec<Declaration>,
    #[serde(default)]
    pub preserved_previous: Vec<Preservation>,
    #[serde(default)]
    pub resolutions: Vec<Resolution>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransferRequest {
    pub version: u8,
    pub operation_id: String,
    pub host: String,
    pub inputs: Vec<Input>,
    pub items: Vec<Identity>,
    pub purpose: String,
    pub cleanup_trigger: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileRequest {
    pub version: u8,
    pub operation: OperationKey,
    pub reconciliation_id: String,
    pub inputs: Vec<Input>,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub sender: Identity,
    pub receiver: Identity,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub task: String,
    pub phase: String,
    pub basis: Basis,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionRelation {
    pub bad: Source,
    pub successors: Vec<u64>,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    pub code: String,
    pub scope: String,
    pub host: Option<String>,
    pub identity: Option<Identity>,
    pub source: Option<Source>,
    pub detail: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<Source>,
}

fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Empty {}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemsOutput {
    pub items: Vec<u64>,
    pub artifacts: Vec<ArtifactLocator>,
    pub resolutions: Vec<ResolutionRelation>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverOutput {
    pub items: Vec<u64>,
    pub artifacts: Vec<ArtifactLocator>,
    pub mapping: Vec<Mapping>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SenderOutput {
    pub items: Vec<u64>,
    pub receiver_basis: Basis,
    pub mapping: Vec<Mapping>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletedOutput {
    pub outcome: String,
    pub publications: Vec<Publication>,
    #[serde(deserialize_with = "required_option")]
    pub checkpoint: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncompleteOutput {
    pub attempt: u64,
    pub previous_phase: String,
    #[serde(deserialize_with = "required_option")]
    pub checkpoint: Option<u64>,
    pub action: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconcileIntentOutput {
    pub directive: ArtifactLocator,
    pub reconciliation_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadyOutput {
    pub directive: ArtifactLocator,
    pub reconciliation_id: String,
    pub items: Vec<u64>,
    pub unresolved: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciledOutput {
    pub directive: ArtifactLocator,
    pub reconciliation_id: String,
    pub ready_bases: Vec<Publication>,
    pub unresolved: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PhaseOutputs {
    Intent(Empty),
    Items(ItemsOutput),
    Receiver(ReceiverOutput),
    Sender(SenderOutput),
    Completed(CompletedOutput),
    Incomplete(IncompleteOutput),
    ReconcileIntent(ReconcileIntentOutput),
    Ready(ReadyOutput),
    Reconciled(ReconciledOutput),
}
impl PhaseOutputs {
    pub fn phase(&self) -> &str {
        match self {
            Self::Intent(_) => "intent",
            Self::Items(_) => "items_published",
            Self::Receiver(_) => "receiver_published",
            Self::Sender(_) => "sender_released",
            Self::Completed(_) => "completed",
            Self::Incomplete(_) => "incomplete",
            Self::ReconcileIntent(_) => "reconcile_intent",
            Self::Ready(_) => "reconcile_ready",
            Self::Reconciled(_) => "reconciled",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationNote {
    pub version: u8,
    pub key: OperationKey,
    pub phase: String,
    pub request: ArtifactLocator,
    pub input_bases: Vec<Input>,
    pub host: String,
    pub at: String,
    #[serde(deserialize_with = "required_option")]
    pub observer: Option<String>,
    pub outputs: PhaseOutputs,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub kept_verified: u64,
    pub kept_unknown: u64,
    pub released: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointScope {
    pub mode: String,
    pub host: String,
    pub other_hosts: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointNote {
    pub version: u8,
    pub operation: OperationKey,
    pub observed_at: String,
    pub host: String,
    #[serde(deserialize_with = "required_option")]
    pub observer: Option<String>,
    #[serde(deserialize_with = "required_option")]
    pub basis: Option<Basis>,
    pub scope: CheckpointScope,
    #[serde(deserialize_with = "required_option")]
    pub counts: Option<Counts>,
    pub changed: u64,
    pub actions: Vec<String>,
    pub complete: bool,
}
pub fn decode_operation(raw: &str) -> Result<OperationNote> {
    let n: OperationNote = decode_request(raw.as_bytes())?;
    n.key.validate()?;
    if n.version != 1 || n.outputs.phase() != n.phase || !line(&n.host) || n.input_bases.is_empty()
    {
        return Err(invalid("operation shape/phase"));
    }
    crate::time::parse(&n.at)?;
    for i in &n.input_bases {
        i.basis.validate()?;
        if !task_id(&i.task) {
            return Err(invalid("input task"));
        }
    }
    Ok(n)
}
pub fn decode_checkpoint(raw: &str) -> Result<CheckpointNote> {
    let n: CheckpointNote = decode_request(raw.as_bytes())?;
    n.operation.validate()?;
    crate::time::parse(&n.observed_at)?;
    if n.version != 1 {
        return Err(invalid("unsupported_version"));
    }
    if n.host != n.scope.host
        || n.scope.mode != "all_known_kept_on_host"
        || n.scope.other_hosts != "outside_observation"
        || (n.complete && (n.basis.is_none() || n.counts.is_none()))
    {
        return Err(invalid("checkpoint shape"));
    }
    if let Some(b) = &n.basis {
        b.validate()?
    }
    Ok(n)
}
pub fn unique_identities(items: &[Identity]) -> Result<()> {
    if items.iter().collect::<BTreeSet<_>>().len() != items.len() {
        return Err(invalid("duplicate identities"));
    }
    Ok(())
}
fn inputs_valid(inputs: &[Input]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for i in inputs {
        i.basis.validate()?;
        if !task_id(&i.task) || !seen.insert(&i.task) {
            return Err(invalid("input identity"));
        }
    }
    Ok(())
}
impl Declaration {
    pub fn record(&self, host: &str, owner: &str) -> Result<RetentionRecord> {
        let r = RetentionRecord {
            version: Some(1),
            host: host.into(),
            kind: self.kind.clone(),
            path: self.path.clone(),
            state: "kept".into(),
            verification: "unknown".into(),
            purpose: self.purpose.clone(),
            next_owner: format!("task:{owner}"),
            cleanup_trigger: self.cleanup_trigger.clone(),
            sha256: if self.kind == "worktree" {
                FieldString::Absent
            } else {
                FieldString::Null
            },
            manifest: if self.kind == "scratch" {
                FieldString::Null
            } else {
                FieldString::Absent
            },
            handoff: None,
            verification_action: Some("Inspect the declared retained item".into()),
        };
        r.validate()?;
        Ok(r)
    }
}
impl ArtifactLocator {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Git {
                basis,
                path,
                sha256,
            } => {
                basis.validate()?;
                if !member_path(path) || !digest(sha256) {
                    return Err(invalid("artifact locator"));
                }
            }
            Self::Attachment {
                task,
                name,
                basis,
                sha256,
            } => {
                basis.validate()?;
                if !task_id(task) || !digest(sha256) {
                    return Err(invalid("attachment locator"));
                }
                crate::attachments::validate_name(name)?;
            }
            Self::Directory {
                host,
                path,
                manifest,
            } => {
                if !line(host)
                    || !location(path)
                    || matches!(manifest.as_ref(), Self::Directory { .. })
                {
                    return Err(invalid("preserved directory"));
                }
                manifest.validate()?;
            }
        }
        Ok(())
    }
}
impl CheckpointRequest {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.inputs.len() != 1 || !line(&self.host) {
            return Err(invalid("checkpoint request"));
        }
        inputs_valid(&self.inputs)?;
        OperationKey {
            coordinator: self.inputs[0].task.clone(),
            operation_id: self.operation_id.clone(),
        }
        .validate()?;
        let mut ids = Vec::new();
        for d in &self.declarations {
            ids.push(d.record(&self.host, &self.inputs[0].task)?.identity());
        }
        unique_identities(&ids)?;
        for p in &self.preserved_previous {
            p.artifact.validate()?;
        }
        for r in &self.resolutions {
            r.bad.basis.validate()?;
            if r.bad.ordinal == 0 || !line(&r.reason) {
                return Err(invalid("resolution"));
            }
            for d in &r.declarations {
                d.record(&self.host, &self.inputs[0].task)?;
            }
        }
        Ok(())
    }
}
impl TransferRequest {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1
            || self.inputs.len() != 2
            || self.items.is_empty()
            || !line(&self.host)
            || !line(&self.purpose)
            || !line(&self.cleanup_trigger)
            || self.cleanup_trigger == "none"
        {
            return Err(invalid("transfer request"));
        }
        inputs_valid(&self.inputs)?;
        OperationKey {
            coordinator: self.inputs[0].task.clone(),
            operation_id: self.operation_id.clone(),
        }
        .validate()?;
        unique_identities(&self.items)?;
        for i in &self.items {
            if i.host != self.host {
                return Err(invalid("remote transfer unsupported"));
            }
            Declaration {
                kind: i.kind.clone(),
                path: i.path.clone(),
                purpose: self.purpose.clone(),
                cleanup_trigger: self.cleanup_trigger.clone(),
            }
            .record(&self.host, &self.inputs[1].task)?;
        }
        Ok(())
    }
}
impl ReconcileRequest {
    pub fn validate(&self) -> Result<()> {
        self.operation.validate()?;
        OperationKey {
            coordinator: self.operation.coordinator.clone(),
            operation_id: self.reconciliation_id.clone(),
        }
        .validate()?;
        inputs_valid(&self.inputs)?;
        if self.version != 1
            || self.inputs.is_empty()
            || self.inputs.len() > 2
            || self.inputs[0].task != self.operation.coordinator
            || !line(&self.reason)
        {
            return Err(invalid("reconciliation request"));
        }
        Ok(())
    }
}
