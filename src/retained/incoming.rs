//! Incoming `next_owner` retained custody queries across same-project and all-projects scopes.

use super::model::{Identity, RetentionRecord, Source};
use crate::error::Result;
use crate::model::TaskId;
use serde::{Deserialize, Deserializer, Serialize};

fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncomingScope {
    Project,
    AllProjects,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingCustody {
    pub scope: IncomingScope,
    #[serde(deserialize_with = "required_option")]
    pub complete: Option<bool>,
    pub items: Vec<IncomingItem>,
    pub diagnostics: Vec<IncomingDiagnostic>,
}

impl IncomingCustody {
    pub fn none() -> Self {
        Self {
            scope: IncomingScope::None,
            complete: None,
            items: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingItem {
    pub holder: String,
    pub identity: Identity,
    pub record: RetentionRecord,
    pub source: Source,
    pub observed_at: String,
    #[serde(deserialize_with = "required_option")]
    pub checkpoint: Option<Source>,
    pub coverage: String,
}

impl IncomingItem {
    pub(crate) fn sort_key(&self) -> (&str, &Identity) {
        (&self.holder, &self.identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncomingDiagnostic {
    pub code: String,
    pub project: String,
    #[serde(deserialize_with = "required_option")]
    pub holder: Option<String>,
    #[serde(deserialize_with = "required_option")]
    pub host: Option<String>,
    #[serde(deserialize_with = "required_option")]
    pub identity: Option<Identity>,
    #[serde(deserialize_with = "required_option")]
    pub source: Option<Source>,
    pub detail: String,
    pub action: String,
}

type SourceSortKey<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str, &'a str, u64);

fn source_sort_key(source: &Source) -> SourceSortKey<'_> {
    (
        &source.basis.project,
        &source.basis.layout,
        &source.basis.records_subdir,
        &source.basis.commit,
        &source.basis.task_path,
        &source.basis.retained_ref,
        source.ordinal,
    )
}

impl IncomingDiagnostic {
    #[allow(clippy::type_complexity)]
    pub(crate) fn sort_key(
        &self,
    ) -> (
        &str,
        Option<&str>,
        &str,
        Option<&str>,
        Option<&Identity>,
        Option<SourceSortKey<'_>>,
        &str,
        &str,
    ) {
        (
            &self.project,
            self.holder.as_deref(),
            &self.code,
            self.host.as_deref(),
            self.identity.as_ref(),
            self.source.as_ref().map(source_sort_key),
            &self.detail,
            &self.action,
        )
    }
}

pub(crate) struct PreparedIncoming {
    pub custody: IncomingCustody,
}

pub(crate) fn collect(
    _ctx: &crate::commands::Ctx,
    _focal_id: &TaskId,
    _focal_pinned: &super::basis::PinnedTask,
    _host: Option<&str>,
    all_projects: bool,
) -> Result<(PreparedIncoming, Vec<String>)> {
    let scope = if all_projects {
        IncomingScope::AllProjects
    } else {
        IncomingScope::Project
    };
    Ok((
        PreparedIncoming {
            custody: IncomingCustody {
                scope,
                complete: Some(true),
                items: Vec::new(),
                diagnostics: Vec::new(),
            },
        },
        Vec::new(),
    ))
}

pub(crate) fn recheck_freshness(
    ctx: &crate::commands::Ctx,
    focal_id: &TaskId,
    focal_pinned: &super::basis::PinnedTask,
    mut prepared: PreparedIncoming,
) -> Result<IncomingCustody> {
    super::freshness::current(&ctx.project, focal_id, focal_pinned, false)?;
    prepared
        .custody
        .items
        .sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    prepared
        .custody
        .diagnostics
        .sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    prepared.custody.diagnostics.dedup();
    prepared.custody.complete = Some(prepared.custody.diagnostics.is_empty());
    Ok(prepared.custody)
}
