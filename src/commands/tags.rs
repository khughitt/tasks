use super::ReadCtx;
use crate::error::Result;
use crate::model::Status;
use crate::output::{Output, TagRow, TagsOut};
use std::collections::{BTreeMap, BTreeSet};

/// Tag frequencies over open tasks, or over the given statuses: how many tasks carry
/// each tag. Visibility only: this is how a shared vocabulary would be chosen, not
/// enforced.
pub fn run(ctx: ReadCtx, statuses: Vec<String>) -> Result<Output> {
    let statuses = statuses
        .iter()
        .map(|status| Status::parse(status))
        .collect::<Result<Vec<_>>>()?;
    let all = ctx.scope.scan()?;
    let mut rows: BTreeMap<&str, TagRow> = BTreeMap::new();
    for task in all.iter().filter(|task| {
        if statuses.is_empty() {
            task.status.is_open()
        } else {
            statuses.contains(&task.status)
        }
    }) {
        // A task counts once per tag, however many times the tag is listed on it.
        let distinct: BTreeSet<&str> = task.tags.iter().map(String::as_str).collect();
        for tag in distinct {
            let row = rows.entry(tag).or_insert_with(|| TagRow {
                tag: tag.to_string(),
                count: 0,
                projects: BTreeMap::new(),
            });
            row.count += 1;
            *row.projects.entry(task.id.prefix.clone()).or_default() += 1;
        }
    }
    let mut tags: Vec<TagRow> = rows.into_values().collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
    Ok(Output::Tags(TagsOut {
        tags,
        warnings: ctx.warnings,
    }))
}
