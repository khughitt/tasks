use super::ReadCtx;
use crate::error::{Error, Result};
use crate::membership::Membership;
use crate::model::TaskId;
use crate::output::{Format, GoalWork, Output, TreeNode, TreeOut};
use crate::registry::Registry;

pub(super) fn populate_tree_goal_work(
    nodes: &mut [TreeNode],
    hierarchy: &crate::hierarchy::HierarchyView<'_>,
    registry: &Registry,
    membership: &mut Membership<'_>,
) -> Result<()> {
    for node in nodes {
        let parsed = TaskId::parse(&node.summary.id).expect("summary id is valid");
        let canonical = registry.canonical_id(&parsed);
        let task = hierarchy
            .get(&canonical)
            .expect("tree node came from captured hierarchy");
        node.summary.goal_work = GoalWork::of(task, node.summary.child_count, membership)?;
        populate_tree_goal_work(&mut node.children, hierarchy, registry, membership)?;
    }
    Ok(())
}

/// One forest per project in scope, concatenated in scope order. The forest builder
/// sorts every root globally, so a single run over the union would interleave projects
/// (docs/task-model.md). With an id the scope is a single project -- the one its prefix names,
/// resolved by `open_id_read_ctx`, since `--all-projects` and an id are a clap conflict --
/// so that project's scan is checked for it.
pub fn run(mut ctx: ReadCtx, id: Option<String>, all: bool, format: Format) -> Result<Output> {
    let root = id
        .as_deref()
        .map(|id| super::parse_id(&ctx.registry, &ctx.shorthand, id))
        .transpose()?;
    let now = crate::time::parse(&crate::time::now())?;
    let mut nodes = Vec::new();
    let mut captured = Vec::new();
    let mut opened = Vec::new();
    let mut projects = Vec::new();
    for (project, tasks) in ctx.scope.scan_each()? {
        let start = captured.len();
        opened.push(project.prefix.clone());
        captured.extend(tasks);
        projects.push((project.prefix.clone(), start..captured.len()));
    }
    let registry = ctx.registry.clone();
    let mut membership =
        (format == Format::Pretty).then(|| Membership::new(&registry, &opened, &captured));
    let hierarchy = match &mut membership {
        Some(membership) => {
            let records = crate::hierarchy::project_records(&captured, &opened, &registry);
            let mut indexes = crate::hierarchy::ProjectIndexes::new();
            for prefix in records.keys() {
                let index = membership
                    .hierarchy(prefix)?
                    .expect("preseeded project in Membership has an index");
                indexes.insert(prefix.clone(), index);
            }
            crate::hierarchy::HierarchyView::new(records, indexes, &registry)
        }
        None => crate::hierarchy::view_of(&captured, &opened, &registry),
    };
    // Activity sees cross-project edges, while each forest keeps its project ordering.
    let activity = crate::activity::ActivityIndex::of_scan(&captured, &registry, now);
    for (prefix, range) in projects {
        let tasks = &captured[range];
        if let Some(root) = &root {
            let canonical_root = registry.canonical_id(root);
            let canonical_prefix = registry.canonical_prefix(&prefix);
            if canonical_root.prefix != canonical_prefix || hierarchy.get(&canonical_root).is_none()
            {
                return Err(Error::TaskNotFound(root.to_string()));
            }
        }
        let deps = super::list::review_deps(&mut ctx, tasks)?;
        nodes.extend(crate::hierarchy::forest(
            tasks,
            &hierarchy,
            &deps,
            root.as_ref(),
            all,
            crate::hierarchy::Shelved::UnderShownParent,
            None,
            &registry,
            now,
        ));
    }
    if let Some(membership) = &mut membership {
        populate_tree_goal_work(&mut nodes, &hierarchy, &registry, membership)?;
        for warning in membership.take_warnings() {
            if !ctx.warnings.contains(&warning) {
                ctx.warnings.push(warning);
            }
        }
    }
    let colors = ctx.project_colors();
    let mut out = Output::Tree(TreeOut {
        nodes,
        warnings: ctx.warnings,
    });
    out.paint_projects(&colors);
    out.paint_activity(&activity);
    Ok(out)
}
