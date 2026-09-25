//! `tasks claims`: every claim in every registered prefix's claim store, with its
//! liveness. It opens no checkout, so a claim on a task that exists only in a worktree is
//! listed and an unreachable checkout changes nothing. A store that cannot be read fails
//! the command: there is no partial answer (ai docs/specs/2026-09-24-turn-boundary-gate-design.md §3.1).

use crate::claims::ClaimSnapshot;
use crate::error::Result;
use crate::output::{ClaimInfo, ClaimRow, ClaimsOut, Output};
use crate::registry::Registry;

pub fn run() -> Result<Output> {
    let registry = Registry::load()?;
    let mut claims = Vec::new();
    // One prefix at a time so each row carries the prefix of the store that holds it.
    for prefix in registry.projects.keys() {
        let snapshot = ClaimSnapshot::load(std::iter::once(prefix.as_str()))?;
        for (id, (claim, live)) in snapshot.iter() {
            claims.push(ClaimRow {
                id: id.clone(),
                prefix: prefix.clone(),
                claim: ClaimInfo::of(claim, live),
            });
        }
    }
    Ok(Output::Claims(ClaimsOut { claims }))
}
