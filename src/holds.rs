//! Exclusive holds (lanes/needs design §4.4): which live claims, in the claim store of
//! every registered project, hold which exclusive needs. Read without any project's lock;
//! an acquire that would record a hold reads it under the host-wide holds lock.

use crate::claims::{Claim, ClaimStore, Liveness, ProcStat};
use crate::error::Result;
use crate::model::{Task, TaskId};
use crate::needs::Vocabulary;
use crate::registry::Registry;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use time::OffsetDateTime;

/// The live claim that holds a need: its task, the session that owns it, and the prefix
/// of the store it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub task: TaskId,
    pub session: String,
    pub prefix: String,
}

/// The live holding claims that are the caller's own (§4.4 "Blocks other sessions
/// only"), by store prefix and task. Built by `HoldSnapshot::mine` with the claim
/// ownership rule (`commands::ownership`), never by comparing session strings. The
/// default owns nothing: with no resolved identity and no proof, every hold is another
/// session's.
#[derive(Debug, Default)]
pub struct Mine {
    claims: BTreeSet<(String, TaskId)>,
}

impl Mine {
    fn owns(&self, holder: &Holder) -> bool {
        self.claims
            .contains(&(holder.prefix.clone(), holder.task.clone()))
    }
}

/// Every live hold on the host, by need name. Exclusive need names are one namespace
/// across projects (§4.1), so a hold recorded in any project counts in every project that
/// declares the same name exclusive.
#[derive(Debug, Default)]
pub struct HoldSnapshot {
    by_need: BTreeMap<String, Vec<Holder>>,
    /// The full claim behind every holder: ownership by process proof reads its pid,
    /// start time, boot and host, not only its session.
    claims: BTreeMap<(String, TaskId), Claim>,
}

fn unknown(prefix: &str, error: &crate::error::Error) -> String {
    format!("hold state unknown for {prefix} ({error})")
}

impl HoldSnapshot {
    /// Reads the store of every registered prefix (§4.4 "Which stores are read"), not
    /// only those in scope: the same set `tasks claims` reads. Stores of unregistered or
    /// renamed prefixes are never read. Unlike `ClaimSnapshot`, a store that cannot be
    /// read is a warning, not a failure, and contributes no holds.
    pub fn load(registry: &Registry, now: OffsetDateTime) -> (HoldSnapshot, Vec<String>) {
        let mut warnings = Vec::new();
        let mut stores = Vec::new();
        for prefix in registry.projects.keys() {
            match ClaimStore::path_for(prefix) {
                Ok(path) => stores.push((prefix.clone(), path)),
                Err(error) => warnings.push(unknown(prefix, &error)),
            }
        }
        let (snapshot, more) = Self::load_from_paths(
            stores,
            now,
            crate::claims::boot_id().as_deref(),
            crate::claims::proc_stat,
        );
        warnings.extend(more);
        (snapshot, warnings)
    }

    /// The path form, with liveness inputs injected, so a test needs no environment.
    pub fn load_from_paths(
        stores: impl IntoIterator<Item = (String, PathBuf)>,
        now: OffsetDateTime,
        boot_id: Option<&str>,
        stat: impl Fn(u32) -> ProcStat,
    ) -> (HoldSnapshot, Vec<String>) {
        let mut snapshot = HoldSnapshot::default();
        let mut warnings = Vec::new();
        for (prefix, path) in stores {
            let store = match ClaimStore::load_from(&path) {
                Ok(store) => store,
                Err(error) => {
                    warnings.push(unknown(&prefix, &error));
                    continue;
                }
            };
            for (key, claim) in store.iter() {
                // Held only while live (§4.4): a dead claim, a park, or a doing status
                // alone holds nothing; a pidless claim holds for its TTL.
                if claim.holds.is_empty()
                    || crate::claims::liveness_with(claim, now, boot_id, &stat) != Liveness::Live
                {
                    continue;
                }
                let task = match TaskId::parse(key) {
                    Ok(task) => task,
                    Err(error) => {
                        warnings.push(format!(
                            "hold state unknown for {prefix}: claim entry {key:?} is not a \
                             task id ({error})"
                        ));
                        continue;
                    }
                };
                for need in &claim.holds {
                    snapshot
                        .by_need
                        .entry(need.clone())
                        .or_default()
                        .push(Holder {
                            task: task.clone(),
                            session: claim.session.clone(),
                            prefix: prefix.clone(),
                        });
                }
                snapshot
                    .claims
                    .insert((prefix.clone(), task), claim.clone());
            }
        }
        (snapshot, warnings)
    }

    /// Which live holding claims are the caller's own, decided once per command. `owns`
    /// is the claim ownership rule (`commands::own_holds` passes `commands::ownership`:
    /// the resolved identity, else the claim's process proof). Its error is returned, never
    /// read as "not mine".
    pub fn mine(&self, mut owns: impl FnMut(&Claim) -> Result<bool>) -> Result<Mine> {
        let mut mine = Mine::default();
        for (key, claim) in &self.claims {
            if owns(claim)? {
                mine.claims.insert(key.clone());
            }
        }
        Ok(mine)
    }

    /// The first live claim holding `need` that is neither on `task` itself (a takeover is
    /// never a hold) nor the caller's own (the holding session may take more work needing
    /// it). `Mine::default()` means the caller owns nothing, and every hold counts as
    /// another session's.
    pub fn holder(&self, need: &str, task: &TaskId, mine: &Mine) -> Option<&Holder> {
        self.by_need
            .get(need)?
            .iter()
            .find(|holder| holder.task != *task && !mine.owns(holder))
    }

    /// No live hold anywhere: a view can skip resolving the caller's identity.
    pub fn is_empty(&self) -> bool {
        self.by_need.is_empty()
    }
}

/// §4.4 "Blocks other sessions only": `task` is held back when its own project declares a
/// need exclusive and another session's live claim on another task holds a need of that
/// name. The first such need, in name order, with its holder.
pub fn held_back(
    snapshot: &HoldSnapshot,
    vocab: &Vocabulary,
    task: &Task,
    mine: &Mine,
) -> Option<(String, Holder)> {
    crate::needs::exclusive_of(vocab, &task.needs)
        .into_iter()
        .find_map(|need| {
            snapshot
                .holder(&need, &task.id, mine)
                .cloned()
                .map(|holder| (need, holder))
        })
}

/// The views' one warning per need and holder (§4.5), however many tasks wait on it.
#[derive(Debug, Default)]
pub struct HeldWarnings {
    counts: BTreeMap<(String, String, String), usize>,
}

impl HeldWarnings {
    pub fn add(&mut self, need: &str, holder: &Holder) {
        *self
            .counts
            .entry((
                need.to_string(),
                holder.task.to_string(),
                holder.session.clone(),
            ))
            .or_default() += 1;
    }

    pub fn into_warnings(self) -> Vec<String> {
        self.counts
            .into_iter()
            .map(|((need, id, session), count)| {
                format!("{count} task(s) wait for {need}, held by {id} ({session})")
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::ProcStat;

    fn at(s: &str) -> OffsetDateTime {
        crate::time::parse(s).unwrap()
    }

    fn alive(_: u32) -> ProcStat {
        ProcStat::Found {
            state: 'S',
            starttime: 7,
        }
    }

    fn gone(_: u32) -> ProcStat {
        ProcStat::NotFound
    }

    /// An unresolved identity: no hold is the caller's.
    fn nobody() -> Mine {
        Mine::default()
    }

    /// The holds a caller owns by session equality alone, as an explicit
    /// `TASKS_SESSION` caller does under `commands::ownership`.
    fn session(snapshot: &HoldSnapshot, name: &str) -> Mine {
        snapshot.mine(|claim| Ok(claim.session == name)).unwrap()
    }

    /// One claim entry seen at 10:00. With a pid, its liveness is the process's; without
    /// one, the four-hour TTL's.
    fn entry(id: &str, session: &str, pid: Option<u32>, holds: &[&str]) -> String {
        let pid = pid
            .map(|pid| format!("pid = {pid}\npid_start = 7\nboot_id = \"boot\"\n"))
            .unwrap_or_default();
        let holds = if holds.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = holds.iter().map(|need| format!("{need:?}")).collect();
            format!("holds = [{}]\n", list.join(", "))
        };
        format!(
            "[claims.\"{id}\"]\nowner = \"o\"\nsession = \"{session}\"\n{pid}host = \"h\"\n\
             worktree = \"/w\"\nstarted = \"2026-10-03T10:00:00Z\"\n\
             seen = \"2026-10-03T10:00:00Z\"\n{holds}"
        )
    }

    fn store(dir: &std::path::Path, prefix: &str, text: &str) -> (String, PathBuf) {
        let path = dir.join(format!("{prefix}.toml"));
        std::fs::write(&path, text).unwrap();
        (prefix.to_string(), path)
    }

    fn id(text: &str) -> TaskId {
        TaskId::parse(text).unwrap()
    }

    #[test]
    fn a_live_claim_holds_and_a_dead_one_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "a", Some(42), &["quiet"]),
        )];
        let now = at("2026-10-03T11:00:00Z");
        let (live, warnings) =
            HoldSnapshot::load_from_paths(stores.clone(), now, Some("boot"), alive);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            live.holder("quiet", &id("sci-000002"), &nobody()),
            Some(&Holder {
                task: id("sci-000001"),
                session: "a".into(),
                prefix: "sci".into(),
            })
        );
        let (dead, _) = HoldSnapshot::load_from_paths(stores, now, Some("boot"), gone);
        assert!(dead.holder("quiet", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn a_claim_without_a_pid_holds_until_its_ttl() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "codex:x", None, &["quiet"]),
        )];
        let (within, _) = HoldSnapshot::load_from_paths(
            stores.clone(),
            at("2026-10-03T13:59:00Z"),
            Some("boot"),
            alive,
        );
        assert!(
            within
                .holder("quiet", &id("sci-000002"), &nobody())
                .is_some()
        );
        let (past, _) =
            HoldSnapshot::load_from_paths(stores, at("2026-10-03T15:00:00Z"), Some("boot"), alive);
        assert!(past.holder("quiet", &id("sci-000002"), &nobody()).is_none());
    }

    #[test]
    fn an_entry_written_before_holds_existed_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "a", Some(42), &[]),
        )];
        let (snapshot, warnings) =
            HoldSnapshot::load_from_paths(stores, at("2026-10-03T11:00:00Z"), Some("boot"), alive);
        assert!(warnings.is_empty());
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &nobody())
                .is_none()
        );
    }

    #[test]
    fn an_unreadable_store_warns_and_the_others_still_count() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![
            store(dir.path(), "fam", "claims = [not toml"),
            store(
                dir.path(),
                "sci",
                &entry("sci-000001", "a", Some(42), &["quiet"]),
            ),
            // A store that was never written is empty, not unknown.
            ("ops".to_string(), dir.path().join("ops.toml")),
        ];
        let (snapshot, warnings) =
            HoldSnapshot::load_from_paths(stores, at("2026-10-03T11:00:00Z"), Some("boot"), alive);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].starts_with("hold state unknown for fam ("),
            "{warnings:?}"
        );
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &nobody())
                .is_some()
        );
    }

    #[test]
    fn a_claim_key_that_is_not_a_task_id_warns_and_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("junk", "a", Some(42), &["quiet"]),
        )];
        let (snapshot, warnings) =
            HoldSnapshot::load_from_paths(stores, at("2026-10-03T11:00:00Z"), Some("boot"), alive);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].starts_with("hold state unknown for sci:"),
            "{warnings:?}"
        );
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &nobody())
                .is_none()
        );
    }

    #[test]
    fn the_target_task_and_the_callers_session_never_hold_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "a", Some(42), &["quiet"]),
        )];
        let (snapshot, _) =
            HoldSnapshot::load_from_paths(stores, at("2026-10-03T11:00:00Z"), Some("boot"), alive);
        // A claim on the target itself: a takeover, never a hold.
        assert!(
            snapshot
                .holder("quiet", &id("sci-000001"), &nobody())
                .is_none()
        );
        // The holding session may take more work that needs what it holds.
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &session(&snapshot, "a"))
                .is_none()
        );
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &session(&snapshot, "b"))
                .is_some()
        );
        // Unresolved identity: every hold is someone else's.
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &nobody())
                .is_some()
        );
        // A need nobody holds.
        assert!(
            snapshot
                .holder("gpu", &id("sci-000002"), &nobody())
                .is_none()
        );
    }

    #[test]
    fn whose_hold_it_is_follows_the_ownership_rule_not_the_session_string() {
        let dir = tempfile::tempdir().unwrap();
        let stores = vec![store(
            dir.path(),
            "sci",
            &entry("sci-000001", "c1", Some(42), &["quiet"]),
        )];
        let (snapshot, _) =
            HoldSnapshot::load_from_paths(stores, at("2026-10-03T11:00:00Z"), Some("boot"), alive);
        // A rule that owns the claim by its process (as relay proof does) makes the hold
        // the caller's own, though the caller's session string is not `c1`.
        let proved = snapshot.mine(|claim| Ok(claim.pid == Some(42))).unwrap();
        assert!(
            snapshot
                .holder("quiet", &id("sci-000002"), &proved)
                .is_none()
        );
        // The rule's error is the caller's error, never a silent "not mine".
        let error = snapshot
            .mine(|_| Err(crate::error::Error::Io("relay config unreadable".into())))
            .unwrap_err();
        assert!(
            error.to_string().contains("relay config unreadable"),
            "{error}"
        );
    }

    #[test]
    fn held_warnings_aggregate_per_need_and_holder() {
        let sci = Holder {
            task: id("sci-000001"),
            session: "a".into(),
            prefix: "sci".into(),
        };
        let fam = Holder {
            task: id("fam-000002"),
            session: "b".into(),
            prefix: "fam".into(),
        };
        let mut warnings = HeldWarnings::default();
        warnings.add("quiet", &sci);
        warnings.add("quiet", &sci);
        warnings.add("gpu", &fam);
        assert_eq!(
            warnings.into_warnings(),
            [
                "1 task(s) wait for gpu, held by fam-000002 (b)",
                "2 task(s) wait for quiet, held by sci-000001 (a)",
            ]
        );
    }
}
