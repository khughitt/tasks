use super::snapshot::{FileState, Snapshot};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    Fresh,
    ResumeFiles,
    ResumeRegistry,
    ResumeCleanup,
    Complete,
    Refuse(String),
}

/// Derive recovery from one observation; a verdict does not authorize writes.
pub fn classify(snap: &Snapshot) -> Recovery {
    if let Some(reason) = refusal(snap) {
        return Recovery::Refuse(reason);
    }
    let invocation = &snap.invocation;
    if invocation.source == invocation.target {
        return Recovery::Refuse(format!(
            "source and target prefixes are both {:?}",
            invocation.source
        ));
    }
    let registry = &snap.registry;
    let registry_old = registry.old_key.as_ref() == Some(&invocation.root)
        && registry.new_key.is_none()
        && registry.alias.is_none();
    let registry_new = registry.new_key.as_ref() == Some(&invocation.root)
        && registry.old_key.is_none()
        && registry.alias.as_ref() == Some(&invocation.target);

    match &snap.inventory {
        None => {
            let prefix = snap.config.as_ref().map(|config| &config.prefix);
            if prefix == Some(&invocation.source) && registry_old {
                if snap.named.target != 0 {
                    return Recovery::Refuse(format!(
                        "{} destination task files with prefix {:?} already exist",
                        snap.named.target, invocation.target
                    ));
                }
                Recovery::Fresh
            } else if snap.named.source == 0 && prefix == Some(&invocation.target) && registry_new {
                Recovery::Complete
            } else {
                Recovery::Refuse(format!(
                    "no inventory and no settled state: config {prefix:?}, named {:?}, registry {registry:?}",
                    snap.named
                ))
            }
        }
        Some(inventory) => {
            let config = snap.config.as_ref().expect("R6 checked the config");
            let files_done = snap.entries.iter().all(|entry| {
                matches!(entry.source, FileState::Absent)
                    && matches!(entry.dest, FileState::Present(_))
            });
            if config.digest == inventory.config_from && registry_old {
                Recovery::ResumeFiles
            } else if files_done && config.digest == inventory.config_to && registry_old {
                Recovery::ResumeRegistry
            } else if files_done && config.digest == inventory.config_to && registry_new {
                Recovery::ResumeCleanup
            } else {
                Recovery::Refuse(format!(
                    "no resumable stage: files_done={files_done}, config digest {:?}, registry {registry:?}",
                    config.digest
                ))
            }
        }
    }
}

fn refusal(snap: &Snapshot) -> Option<String> {
    if let Some(inventory) = &snap.inventory {
        if inventory.source != snap.invocation.source
            || inventory.target != snap.invocation.target
            || inventory.root != snap.invocation.root
        {
            return Some(format!(
                "R1: inventory {} -> {} at {} disagrees with invocation {} -> {} at {}",
                inventory.source,
                inventory.target,
                inventory.root.display(),
                snap.invocation.source,
                snap.invocation.target,
                snap.invocation.root.display()
            ));
        }
        let baseline: BTreeMap<_, _> = inventory
            .entries
            .iter()
            .map(|entry| (&entry.hex, entry))
            .collect();
        let observed: BTreeMap<_, _> = snap
            .entries
            .iter()
            .map(|entry| (&entry.hex, entry))
            .collect();
        if baseline.len() != inventory.entries.len()
            || observed.len() != snap.entries.len()
            || !baseline.keys().eq(observed.keys())
        {
            return Some(
                "entry observations do not correspond one-to-one with the inventory baseline"
                    .into(),
            );
        }
        for entry in &snap.entries {
            if let FileState::Present(digest) = &entry.source
                && digest != &baseline[&entry.hex].from
            {
                return Some(format!(
                    "R2: source {}-{} was edited since the inventory",
                    inventory.source, entry.hex
                ));
            }
        }
        for entry in &snap.entries {
            if let FileState::Present(digest) = &entry.dest
                && digest != &baseline[&entry.hex].to
            {
                return Some(format!(
                    "R3: destination {} conflicts with the inventory for source {}",
                    inventory
                        .root
                        .join("tasks")
                        .join(format!("{}-{}.md", inventory.target, entry.hex))
                        .display(),
                    inventory
                        .root
                        .join("tasks")
                        .join(format!("{}-{}.md", inventory.source, entry.hex))
                        .display()
                ));
            }
        }
        for entry in &snap.entries {
            if matches!(entry.source, FileState::Absent) && matches!(entry.dest, FileState::Absent)
            {
                return Some(format!(
                    "R4: task {} is missing from both source and destination",
                    entry.hex
                ));
            }
        }
        if !snap.strays.is_empty() {
            return Some(format!(
                "R5: task files outside the inventory: {:?}",
                snap.strays
            ));
        }
        if !snap.config.as_ref().is_some_and(|config| {
            config.digest == inventory.config_from || config.digest == inventory.config_to
        }) {
            return Some(
                "R6: config is absent or its digest matches neither inventory config digest".into(),
            );
        }
    }
    if let (Some(old), Some(new)) = (&snap.registry.old_key, &snap.registry.new_key)
        && old != new
    {
        return Some(format!(
            "R7: old and new registry keys name different roots: {} and {}",
            old.display(),
            new.display()
        ));
    }
    if let Some(new) = &snap.registry.new_key
        && new != &snap.invocation.root
    {
        return Some(format!(
            "R8: new registry key names foreign root {} instead of {}",
            new.display(),
            snap.invocation.root.display()
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rename::inventory::{Inventory, InventoryEntry};
    use crate::rename::snapshot::{
        ConfigState, EntryState, FileState, Invocation, Named, RegistryState, Snapshot,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Verdict {
        Fresh,
        ResumeFiles,
        ResumeRegistry,
        ResumeCleanup,
        Complete,
        Refuse,
    }

    fn verdict(recovery: Recovery) -> Verdict {
        match recovery {
            Recovery::Fresh => Verdict::Fresh,
            Recovery::ResumeFiles => Verdict::ResumeFiles,
            Recovery::ResumeRegistry => Verdict::ResumeRegistry,
            Recovery::ResumeCleanup => Verdict::ResumeCleanup,
            Recovery::Complete => Verdict::Complete,
            Recovery::Refuse(reason) => {
                assert!(!reason.trim().is_empty());
                Verdict::Refuse
            }
        }
    }

    fn baseline(count: usize) -> Inventory {
        Inventory {
            source: "old".into(),
            target: "new".into(),
            root: "/project".into(),
            config_from: "config before".into(),
            config_to: "config after".into(),
            entries: (0..count)
                .map(|index| InventoryEntry {
                    hex: format!("{index:06x}"),
                    from: format!("before {index}"),
                    to: format!("after {index}"),
                })
                .collect(),
            parks_store: None,
            store_to: None,
        }
    }

    fn inventory_only() -> Snapshot {
        let inventory = baseline(2);
        Snapshot {
            invocation: Invocation {
                source: inventory.source.clone(),
                target: inventory.target.clone(),
                root: inventory.root.clone(),
            },
            registry: RegistryState {
                old_key: Some(inventory.root.clone()),
                new_key: None,
                alias: None,
            },
            config: Some(ConfigState {
                prefix: inventory.source.clone(),
                digest: inventory.config_from.clone(),
            }),
            entries: inventory
                .entries
                .iter()
                .map(|entry| EntryState {
                    hex: entry.hex.clone(),
                    source: FileState::Present(entry.from.clone()),
                    dest: FileState::Absent,
                })
                .collect(),
            inventory: Some(inventory),
            named: Named {
                source: 2,
                target: 0,
            },
            strays: Vec::new(),
        }
    }

    fn move_entry(snap: &mut Snapshot, index: usize) {
        snap.entries[index].source = FileState::Absent;
        snap.entries[index].dest =
            FileState::Present(snap.inventory.as_ref().unwrap().entries[index].to.clone());
    }

    #[test]
    fn every_phase_boundary_resumes_rather_than_refusing() {
        let mut snap = inventory_only();
        assert_eq!(classify(&snap), Recovery::ResumeFiles, "just after P2");
        move_entry(&mut snap, 0);
        assert_eq!(classify(&snap), Recovery::ResumeFiles, "partial file pass");
        move_entry(&mut snap, 1);
        assert_eq!(
            classify(&snap),
            Recovery::ResumeFiles,
            "files done before P4"
        );
        snap.config = Some(ConfigState {
            prefix: "new".into(),
            digest: "config after".into(),
        });
        assert_eq!(classify(&snap), Recovery::ResumeRegistry, "after P4");
        snap.registry = RegistryState {
            old_key: None,
            new_key: Some("/project".into()),
            alias: Some("new".into()),
        };
        assert_eq!(classify(&snap), Recovery::ResumeCleanup, "after P5");
        snap.inventory = None;
        snap.entries.clear();
        snap.named = Named {
            source: 0,
            target: 2,
        };
        assert_eq!(classify(&snap), Recovery::Complete, "after P6");
        let mut empty = inventory_only();
        empty.entries.clear();
        empty.inventory.as_mut().unwrap().entries.clear();
        empty.named = Named::default();
        assert_eq!(
            classify(&empty),
            Recovery::ResumeFiles,
            "empty project after P2"
        );
        empty.inventory = None;
        assert_eq!(classify(&empty), Recovery::Fresh, "untouched empty project");
        let mut fresh = inventory_only();
        fresh.inventory = None;
        fresh.entries.clear();
        assert_eq!(classify(&fresh), Recovery::Fresh, "untouched project");
    }

    #[test]
    fn each_refusal_fires_on_its_own_before_the_table() {
        for rule in 1..=8 {
            let mut snap = inventory_only();
            match rule {
                1 => snap.inventory.as_mut().unwrap().source = "other".into(),
                2 => snap.entries[0].source = FileState::Present("edited".into()),
                3 => snap.entries[0].dest = FileState::Present("conflict".into()),
                4 => snap.entries[0].source = FileState::Absent,
                5 => snap.strays.push("tasks/old-ffffff.md".into()),
                6 => snap.config.as_mut().unwrap().digest = "edited".into(),
                7 => {
                    snap.registry.old_key = Some("/foreign".into());
                    snap.registry.new_key = Some("/project".into());
                }
                8 => {
                    snap.registry.old_key = None;
                    snap.registry.new_key = Some("/foreign".into());
                }
                _ => unreachable!(),
            }
            let Recovery::Refuse(reason) = classify(&snap) else {
                panic!("R{rule} did not refuse: {snap:?}");
            };
            assert!(reason.starts_with(&format!("R{rule}:")), "{reason}");
            if rule >= 7 {
                snap.inventory = None;
                let Recovery::Refuse(reason) = classify(&snap) else {
                    panic!("R{rule} needs no inventory");
                };
                assert!(reason.starts_with(&format!("R{rule}:")), "{reason}");
            }
        }
    }

    #[test]
    fn entry_identity_is_matched_by_hex_and_bad_observation_shapes_refuse() {
        let mut snap = inventory_only();
        snap.entries.reverse();
        assert_eq!(classify(&snap), Recovery::ResumeFiles);
        let mut missing = snap.clone();
        missing.entries.pop();
        assert_eq!(verdict(classify(&missing)), Verdict::Refuse);
        let mut duplicate = snap.clone();
        duplicate.entries[1] = duplicate.entries[0].clone();
        assert_eq!(verdict(classify(&duplicate)), Verdict::Refuse);
        let mut duplicate_baseline = snap;
        let inv = duplicate_baseline.inventory.as_mut().unwrap();
        inv.entries[1] = inv.entries[0].clone();
        assert_eq!(verdict(classify(&duplicate_baseline)), Verdict::Refuse);
    }

    // Independent transcription of §5.3: accumulate excluded states, then look up
    // the exact table row. This does not call any production recovery predicates.
    fn expected(snap: &Snapshot) -> Verdict {
        let invocation = &snap.invocation;
        let registry = &snap.registry;
        let two_roots = match (&registry.old_key, &registry.new_key) {
            (Some(old), Some(new)) => old != new,
            _ => false,
        };
        let foreign_target = match &registry.new_key {
            Some(root) => root != &invocation.root,
            None => false,
        };
        let registry_column = match (&registry.old_key, &registry.new_key, &registry.alias) {
            (Some(root), None, None) if root == &invocation.root => 1,
            (None, Some(root), Some(target))
                if root == &invocation.root && target == &invocation.target =>
            {
                2
            }
            _ => 0,
        };
        let mut excluded = two_roots || foreign_target;
        let mut done = true;
        let (has_inventory, config_column) = match &snap.inventory {
            Some(inv) => {
                excluded |= (&inv.source, &inv.target, &inv.root)
                    != (&invocation.source, &invocation.target, &invocation.root);
                excluded |= !snap.strays.is_empty();
                let observed: BTreeSet<_> = snap.entries.iter().map(|entry| &entry.hex).collect();
                let recorded: BTreeSet<_> = inv.entries.iter().map(|entry| &entry.hex).collect();
                excluded |= observed != recorded
                    || observed.len() != snap.entries.len()
                    || recorded.len() != inv.entries.len();
                for entry in &snap.entries {
                    let Some(record) = inv.entries.iter().find(|record| record.hex == entry.hex)
                    else {
                        excluded = true;
                        continue;
                    };
                    let source = match &entry.source {
                        FileState::Absent => 0,
                        FileState::Present(digest) if digest == &record.from => 1,
                        FileState::Present(_) => 2,
                    };
                    let dest = match &entry.dest {
                        FileState::Absent => 0,
                        FileState::Present(digest) if digest == &record.to => 1,
                        FileState::Present(_) => 2,
                    };
                    excluded |= !matches!((source, dest), (1, 0) | (1, 1) | (0, 1));
                    done &= (source, dest) == (0, 1);
                }
                let column = match &snap.config {
                    Some(config) if config.digest == inv.config_from => 1,
                    Some(config) if config.digest == inv.config_to => 2,
                    _ => 0,
                };
                excluded |= column == 0;
                (true, column)
            }
            None => (
                false,
                match &snap.config {
                    Some(config) if config.prefix == invocation.source => 1,
                    Some(config) if config.prefix == invocation.target => 2,
                    _ => 0,
                },
            ),
        };
        if excluded {
            return Verdict::Refuse;
        }
        match (has_inventory, config_column, registry_column) {
            (false, 1, 1) if snap.named.target == 0 => Verdict::Fresh,
            (true, 1, 1) => Verdict::ResumeFiles,
            (true, 2, 1) if done => Verdict::ResumeRegistry,
            (true, 2, 2) if done => Verdict::ResumeCleanup,
            (false, 2, 2) if snap.named.source == 0 => Verdict::Complete,
            _ => Verdict::Refuse,
        }
    }

    fn enumerate_snapshots(mut visit: impl FnMut(&Snapshot)) {
        let roots = [
            None,
            Some(PathBuf::from("/project")),
            Some(PathBuf::from("/foreign")),
        ];
        let aliases = [None, Some("new".to_string()), Some("other".to_string())];
        let configs: Vec<_> = std::iter::once(None)
            .chain(["old", "new", "other"].into_iter().flat_map(|prefix| {
                ["config before", "config after", "other"]
                    .into_iter()
                    .map(move |digest| {
                        Some(ConfigState {
                            prefix: prefix.into(),
                            digest: digest.into(),
                        })
                    })
            }))
            .collect();
        for count in 0..=2 {
            for encoded in 0..9usize.pow(count as u32) {
                let inventory = baseline(count);
                let mut snap = inventory_only();
                let mut digits = encoded;
                snap.entries = inventory
                    .entries
                    .iter()
                    .map(|record| {
                        let pair = digits % 9;
                        digits /= 9;
                        let source = match pair % 3 {
                            0 => FileState::Absent,
                            1 => FileState::Present(record.from.clone()),
                            _ => FileState::Present("edited".into()),
                        };
                        let dest = match pair / 3 {
                            0 => FileState::Absent,
                            1 => FileState::Present(record.to.clone()),
                            _ => FileState::Present("conflict".into()),
                        };
                        EntryState {
                            hex: record.hex.clone(),
                            source,
                            dest,
                        }
                    })
                    .collect();
                for inventory_case in 0..8 {
                    snap.inventory = match inventory_case {
                        0 => None,
                        _ => {
                            let mut inv = inventory.clone();
                            match inventory_case {
                                2 => inv.source = "other".into(),
                                3 => inv.target = "other".into(),
                                4 => inv.root = "/foreign".into(),
                                5 => {
                                    inv.entries.pop();
                                }
                                6 => inv.entries.push(InventoryEntry {
                                    hex: "ffffff".into(),
                                    from: "missing before".into(),
                                    to: "missing after".into(),
                                }),
                                7 => {
                                    if let Some(entry) = inv.entries.first_mut() {
                                        entry.hex = "ffffff".into();
                                    }
                                }
                                _ => {}
                            }
                            Some(inv)
                        }
                    };
                    for old in &roots {
                        for new in &roots {
                            for alias in &aliases {
                                snap.registry = RegistryState {
                                    old_key: old.clone(),
                                    new_key: new.clone(),
                                    alias: alias.clone(),
                                };
                                for config in &configs {
                                    snap.config = config.clone();
                                    for source in 0..=1 {
                                        for target in 0..=1 {
                                            snap.named = Named { source, target };
                                            for stray in [false, true] {
                                                snap.strays.clear();
                                                if stray {
                                                    snap.strays.push("tasks/old-ffffff.md".into());
                                                }
                                                visit(&snap);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_enumerated_snapshot_gets_the_verdict_the_spec_names() {
        let started = std::time::Instant::now();
        let mut counts = BTreeMap::new();
        enumerate_snapshots(|snap| {
            let got = verdict(classify(snap));
            let want = expected(snap);
            assert_eq!(got, want, "snapshot {snap:?}");
            *counts.entry(got).or_insert(0usize) += 1;
        });
        for verdict in [
            Verdict::Fresh,
            Verdict::ResumeFiles,
            Verdict::ResumeRegistry,
            Verdict::ResumeCleanup,
            Verdict::Complete,
            Verdict::Refuse,
        ] {
            assert!(
                counts.get(&verdict).copied().unwrap_or(0) > 0,
                "{verdict:?} unreached: {counts:?}"
            );
        }
        assert_eq!(counts.values().sum::<usize>(), 1_572_480);
        eprintln!("1,572,480 snapshots in {:?}: {counts:?}", started.elapsed());
    }
}
