//! Completion candidates for the `CompleteEnv` protocol.
//!
//! Nothing here is reachable from a command path, and nothing here returns `Result`.
//! Completion runs on every TAB with no error channel — the bash stub discards
//! `COMPREPLY` when the completer exits non-zero, and a write to stderr corrupts the
//! prompt — so every failure yields an empty candidate list instead. This is the
//! deliberate exception to the repo's fail-early rule recorded in
//! `docs/specs/2026-09-05-shell-completions-design.md`.

use clap_complete::CompletionCandidate;

use crate::model::{Size, Status};
use crate::registry::Registry;

fn plain(
    values: impl IntoIterator<Item = impl Into<std::ffi::OsString>>,
) -> Vec<CompletionCandidate> {
    values.into_iter().map(CompletionCandidate::new).collect()
}

/// Every status. `edit --status`, `list --status`, `tags --status`.
pub fn statuses() -> Vec<CompletionCandidate> {
    plain(Status::ALL.iter().map(|status| status.as_str()))
}

/// The two `add` accepts; it rejects the rest with a validation error.
pub fn add_statuses() -> Vec<CompletionCandidate> {
    plain([Status::Idea.as_str(), Status::Todo.as_str()])
}

pub fn sizes() -> Vec<CompletionCandidate> {
    plain(Size::ALL.iter().map(|size| size.as_str()))
}

/// The keys `query::SortKey::parse` accepts.
pub fn sorts() -> Vec<CompletionCandidate> {
    plain(["priority", "updated", "created"])
}

/// The modes `style::ColorMode::resolve` accepts.
pub fn colors() -> Vec<CompletionCandidate> {
    plain(["auto", "always", "never"])
}

pub fn categories() -> Vec<CompletionCandidate> {
    plain(crate::commands::feedback::CATEGORIES)
}

/// Registered prefixes, in registry order. An unreadable registry offers nothing.
pub fn prefixes() -> Vec<CompletionCandidate> {
    let Ok(registry) = Registry::load() else {
        return Vec::new();
    };
    plain(registry.projects.keys().cloned().collect::<Vec<_>>())
}
