//! Staged checks: validate a project's records as the index Git will publish them.
//!
//! `snapshot` reads the index and HEAD trees, `content` materializes the staged tree for
//! the full audit, `note_route` qualifies an ordinary appended note against a validated
//! base, and `validated` keeps the host-local record of trees a staged check passed.

pub(crate) mod content;
pub(crate) mod guard;
pub(crate) mod note_route;
pub(crate) mod scratch;
pub(crate) mod snapshot;
pub(crate) mod validated;
