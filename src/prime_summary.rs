use std::collections::BTreeMap;
use std::path::Path;

use crate::cli::ScopeArgs;
use crate::error::{Error, Result};
use crate::output::{PrimeOut, PrimeSection, PrimeSummary, TreeNode};

pub fn full_argv(
    executable: &Path,
    directory: &Path,
    scope: &ScopeArgs,
    closed: bool,
    without: &[String],
) -> Result<Vec<String>> {
    if !std::fs::metadata(executable)?.is_file() {
        return Err(Error::Io(
            "prime summary recovery executable must be a regular file".into(),
        ));
    }
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| Error::Io("prime summary recovery path must be valid UTF-8".into()))
    };
    let mut args = vec![
        text(executable)?,
        "--json".into(),
        "-C".into(),
        text(directory)?,
        "prime".into(),
    ];
    if let Some(project) = &scope.project {
        args.extend(["--project".into(), project.clone()]);
    }
    if let Some(group) = &scope.group {
        args.extend(["--group".into(), group.clone()]);
    }
    if scope.all_projects {
        args.push("--all-projects".into());
    }
    if closed {
        args.push("--closed".into());
    }
    for need in without {
        args.extend(["--without".into(), need.clone()]);
    }
    Ok(args)
}

fn descendants(nodes: &[TreeNode]) -> usize {
    nodes
        .iter()
        .map(|node| node.children.len() + descendants(&node.children))
        .sum()
}

pub fn bound(out: &mut PrimeOut, full: Vec<String>) -> Result<()> {
    let original: BTreeMap<&str, Vec<String>> = [
        (
            "parked",
            out.parked.iter().map(|row| row.id.clone()).collect(),
        ),
        (
            "doing",
            out.doing.iter().map(|row| row.id.clone()).collect(),
        ),
        (
            "ready",
            out.ready.iter().map(|row| row.id.clone()).collect(),
        ),
        (
            "closeout",
            out.closeout.iter().map(|row| row.id.clone()).collect(),
        ),
        (
            "lanes",
            out.lanes.iter().map(|row| row.lane.id.clone()).collect(),
        ),
        (
            "roadmap",
            out.roadmap
                .iter()
                .map(|row| row.summary.id.clone())
                .collect(),
        ),
    ]
    .into();
    let hidden_descendants = descendants(&out.roadmap);
    for root in &mut out.roadmap {
        root.children.clear();
    }
    out.parked.truncate(3);
    out.doing.truncate(5);
    out.ready.truncate(5);
    out.closeout.truncate(3);
    out.lanes.truncate(5);
    out.roadmap.truncate(5);
    out.summary = Some(PrimeSummary {
        byte_limit: 12_288,
        complete: false,
        sections: BTreeMap::new(),
        full,
    });
    refresh(out, &original, hidden_descendants);
    while serde_json::to_vec(&*out).expect("output serializes").len() + 1 > 12_288 {
        if out.roadmap.pop().is_none()
            && out.lanes.pop().is_none()
            && out.closeout.pop().is_none()
            && out.ready.pop().is_none()
            && out.parked.pop().is_none()
            && out.doing.pop().is_none()
        {
            return Err(Error::SummaryTooLarge);
        }
        refresh(out, &original, hidden_descendants);
    }
    Ok(())
}

fn refresh(out: &mut PrimeOut, original: &BTreeMap<&'static str, Vec<String>>, descendants: usize) {
    let sizes = [
        ("parked", out.parked.len()),
        ("doing", out.doing.len()),
        ("ready", out.ready.len()),
        ("closeout", out.closeout.len()),
        ("lanes", out.lanes.len()),
        ("roadmap", out.roadmap.len()),
    ];
    let summary = out.summary.as_mut().expect("summary was initialized");
    summary.complete = descendants == 0;
    for (name, shown) in sizes {
        let ids = &original[name];
        summary.complete &= shown == ids.len();
        summary.sections.insert(
            name,
            PrimeSection {
                total: ids.len(),
                shown,
                omitted: ids.len() - shown,
                first_omitted: ids.get(shown).cloned(),
                descendants_omitted: (name == "roadmap").then_some(descendants),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{Counts, DeferredSummary, PeriodicSummary, TaskSummary};

    fn empty() -> PrimeOut {
        PrimeOut {
            summary: None,
            prefix: Some("demo".into()),
            group: None,
            projects: vec!["demo".into()],
            counts: Counts::default(),
            periodic: PeriodicSummary::default(),
            deferred: DeferredSummary::default(),
            closed: false,
            ready: vec![],
            parked: vec![],
            doing: vec![],
            lanes: vec![],
            roadmap: vec![],
            closeout: vec![],
            halts: vec![],
            warnings: vec![],
        }
    }

    fn row(id: &str) -> TaskSummary {
        let task = crate::format::parse_task(&format!(
            "---\nid: demo-{id}\ntitle: Work\nstatus: todo\npriority: 2\ncreated: 2026-10-01T00:00:00Z\nupdated: 2026-10-01T00:00:00Z\ndepends: []\ntags: []\n---\n"
        ), "fixture").unwrap();
        let scan = [task];
        let deps = crate::review::Deps::of_scan(&scan, |_| Ok(None)).unwrap();
        TaskSummary::of(
            &scan[0],
            &scan,
            &deps,
            None,
            &crate::registry::Registry::default(),
            crate::time::parse("2026-10-24T00:00:00Z").unwrap(),
        )
    }

    fn size(out: &PrimeOut) -> usize {
        serde_json::to_vec(out).unwrap().len() + 1
    }

    #[test]
    fn exact_byte_boundary() {
        let mut at = empty();
        at.warnings.push(String::new());
        bound(&mut at, vec![]).unwrap();
        let padding = 12_288 - size(&at);
        at.warnings[0] = "x".repeat(padding);
        bound(&mut at, vec![]).unwrap();
        assert_eq!(size(&at), 12_288);
        at.warnings[0].push('x');
        let error = bound(&mut at, vec![]).expect_err("one byte beyond the budget must fail");
        assert_eq!(error.kind(), "summary_too_large");
        assert!(error.to_string().len() < 200);
        assert!(!error.to_string().contains("xxxx"));
    }

    #[test]
    fn eviction_refreshes_metadata() {
        let mut out = empty();
        let a = row("000001");
        let mut b = row("000002");
        b.title = "\"\n海".repeat(3_000);
        out.ready = vec![a.clone(), b];
        out.roadmap = vec![TreeNode {
            summary: a.clone(),
            children: vec![],
        }];
        bound(&mut out, vec![]).unwrap();
        assert!(size(&out) <= 12_288);
        assert!(out.roadmap.is_empty());
        assert_eq!(out.ready.len(), 1);
        assert_eq!(
            serde_json::to_value(&out.ready[0]).unwrap(),
            serde_json::to_value(a).unwrap()
        );
        let meta = &out.summary.as_ref().unwrap().sections["ready"];
        assert_eq!((meta.total, meta.shown, meta.omitted), (2, 1, 1));
        assert_eq!(meta.first_omitted.as_deref(), Some("demo-000002"));
    }

    #[test]
    fn shallow_roadmap_counts() {
        let mut out = empty();
        let mut root = row("000001");
        root.child_count = 1;
        root.open_descendant_count = 2;
        out.roadmap = vec![TreeNode {
            summary: root,
            children: vec![TreeNode {
                summary: row("000002"),
                children: vec![TreeNode {
                    summary: row("000003"),
                    children: vec![],
                }],
            }],
        }];
        bound(&mut out, vec![]).unwrap();
        assert!(out.roadmap[0].children.is_empty());
        assert_eq!(out.roadmap[0].summary.child_count, 1);
        assert_eq!(out.roadmap[0].summary.open_descendant_count, 2);
        let meta = out.summary.unwrap();
        assert_eq!(meta.sections["roadmap"].descendants_omitted, Some(2));
        assert!(!meta.complete);
    }

    #[test]
    fn recovery_paths() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("tasks");
        std::fs::write(&executable, "fixture").unwrap();
        let scope = ScopeArgs {
            project: Some("demo".into()),
            all_projects: false,
            group: None,
        };
        let args = full_argv(
            &executable,
            dir.path(),
            &scope,
            true,
            &["a b".into(), "quiet".into()],
        )
        .unwrap();
        assert_eq!(
            &args[4..],
            [
                "prime",
                "--project",
                "demo",
                "--closed",
                "--without",
                "a b",
                "--without",
                "quiet"
            ]
        );
        for missing in [
            dir.path().join("absent"),
            dir.path().join("tasks (deleted)"),
            dir.path().to_path_buf(),
        ] {
            let error = full_argv(&missing, dir.path(), &scope, false, &[])
                .expect_err("unusable executable must fail");
            assert_eq!(error.kind(), "io");
        }
        use std::os::unix::ffi::OsStringExt;
        let non_utf8 = dir
            .path()
            .join(std::ffi::OsString::from_vec(vec![b't', 255]));
        std::fs::write(&non_utf8, "fixture").unwrap();
        assert_eq!(
            full_argv(&non_utf8, dir.path(), &scope, false, &[])
                .unwrap_err()
                .kind(),
            "io"
        );
        assert_eq!(
            full_argv(&executable, &non_utf8, &scope, false, &[])
                .unwrap_err()
                .kind(),
            "io"
        );
    }

    fn lane(id: &str) -> crate::lanes::LaneRow {
        crate::lanes::LaneRow {
            lane: row(id),
            guidance: Some("Guidance".into()),
            state: crate::lanes::LaneState::Ready,
            pick: Some(row("000099")),
            steps: 2,
            active: vec![row("000098")],
            held: vec![],
            causes: crate::lanes::Causes::default(),
        }
    }

    #[test]
    fn oversized_lane_is_atomic() {
        let mut out = empty();
        let small = lane("000001");
        let expected = serde_json::to_value(&small).unwrap();
        let mut big = lane("000002");
        big.guidance = Some("x".repeat(20_000));
        big.held.push(crate::lanes::HeldStep {
            id: crate::model::TaskId::parse("demo-000003").unwrap(),
            need: "quiet".into(),
            holder: crate::model::TaskId::parse("demo-000004").unwrap(),
            by: crate::lanes::HeldBy::Claim,
        });
        out.lanes = vec![small, big];
        bound(&mut out, vec![]).unwrap();
        assert_eq!(out.lanes.len(), 1);
        assert_eq!(serde_json::to_value(&out.lanes[0]).unwrap(), expected);
        let meta = &out.summary.unwrap().sections["lanes"];
        assert_eq!((meta.total, meta.shown, meta.omitted), (2, 1, 1));
        assert_eq!(meta.first_omitted.as_deref(), Some("demo-000002"));
    }

    #[test]
    fn required_metadata_overflow() {
        for section in ["warnings", "halts", "projects", "full", "parked-id"] {
            let mut out = empty();
            let large = "x".repeat(13_000);
            let mut full = vec![];
            match section {
                "warnings" => out.warnings.push(large),
                "halts" => out.halts.push(crate::output::HaltRow {
                    id: "demo-000001".into(),
                    title: large,
                    owner: None,
                    priority: 0,
                    present_locally: true,
                }),
                "projects" => out.projects = (0..1500).map(|n| format!("p{n:07}")).collect(),
                "full" => full.push(large),
                "parked-id" => {
                    let mut park = crate::output::ParkedRow::resolved(
                        row("000001"),
                        crate::model::Phase::Implementing,
                    );
                    park.id = large;
                    out.parked.push(park);
                }
                _ => unreachable!(),
            }
            let warnings = out.warnings.clone();
            let halts = serde_json::to_value(&out.halts).unwrap();
            let error = bound(&mut out, full).unwrap_err();
            assert_eq!(error.kind(), "summary_too_large", "{section}");
            assert!(error.to_string().len() < 200);
            assert_eq!(out.warnings, warnings);
            assert_eq!(serde_json::to_value(&out.halts).unwrap(), halts);
        }
    }

    #[test]
    fn eviction_preserves_section_priority_and_row_values() {
        for warning_bytes in [0, 500, 2000, 5000, 8000, 11000] {
            let mut out = empty();
            let mut large = row("000001");
            large.title = "x".repeat(2000);
            out.ready.push(large.clone());
            out.doing.push(large.clone());
            out.closeout.push(large.clone());
            out.parked.push(crate::output::ParkedRow::resolved(
                large.clone(),
                crate::model::Phase::Implementing,
            ));
            out.roadmap.push(TreeNode {
                summary: large.clone(),
                children: vec![],
            });
            let mut effort = lane("000002");
            effort.lane = large;
            out.lanes.push(effort);
            out.warnings.push("w".repeat(warning_bytes));
            let before = serde_json::to_value(&out).unwrap();
            bound(&mut out, vec![]).unwrap();
            assert!(size(&out) <= 12_288);
            let after = serde_json::to_value(&out).unwrap();
            let mut lengths = Vec::new();
            for name in ["roadmap", "lanes", "closeout", "ready", "parked", "doing"] {
                let rows = after[name].as_array().unwrap();
                lengths.push(rows.len());
                if !rows.is_empty() {
                    assert_eq!(rows[0], before[name][0]);
                }
                let meta = &after["summary"]["sections"][name];
                assert_eq!(meta["total"], 1);
                assert_eq!(meta["shown"], rows.len());
                assert_eq!(meta["omitted"], 1 - rows.len());
            }
            assert!(
                lengths.windows(2).all(|pair| pair[0] <= pair[1]),
                "{lengths:?}"
            );
        }
    }
}
