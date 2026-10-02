//! Bare six-hex ids (`2578c3` for `tasks-2578c3`): input shorthand for the project the
//! invocation stands in, located from the cwd or `-C`. The prefix is fixed for the whole
//! invocation, so routing a command to another project never changes what a suffix
//! means, and a missing task is never searched for elsewhere. Stored ids stay canonical.
use std::cell::OnceCell;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::model::TaskId;
use crate::registry::Registry;
use crate::repo::Project;

pub struct Shorthand {
    start: PathBuf,
    local: OnceCell<String>,
}

impl Shorthand {
    /// Locates the local project only when a bare suffix needs it, so commands that never
    /// see one behave exactly as before outside a project or with `--project`.
    pub fn new(start: PathBuf) -> Self {
        Self {
            start,
            local: OnceCell::new(),
        }
    }

    /// The local project is already known.
    pub fn located(start: PathBuf, prefix: String) -> Self {
        Self {
            start,
            local: OnceCell::from(prefix),
        }
    }

    /// A user-typed id: a bare suffix takes the local prefix; anything else is a full id.
    /// Trailing periods are sentence punctuation either way. A checkout found here for the
    /// first time is held to the same stale-prefix rule as one opened as the local project.
    pub fn parse(&self, registry: &Registry, input: &str) -> Result<TaskId> {
        let trimmed = input.trim_end_matches('.');
        if !is_bare_suffix(trimmed) {
            return TaskId::parse_input(input);
        }
        TaskId::parse(&format!("{}-{trimmed}", self.prefix(registry, input)?))
    }

    fn prefix(&self, registry: &Registry, input: &str) -> Result<&str> {
        if let Some(prefix) = self.local.get() {
            return Ok(prefix);
        }
        let project = match Project::locate(&self.start) {
            Ok(project) => project,
            Err(Error::NoProject(_)) => {
                return Err(Error::InvalidId(
                    input.into(),
                    "a bare suffix names a task in the current project, and there is none \
                     here; give the full id (<prefix>-<hex6>)"
                        .into(),
                ));
            }
            Err(error) => return Err(error),
        };
        crate::commands::reject_stale_local(registry, &project)?;
        Ok(self.local.get_or_init(|| project.prefix))
    }
}

/// Exactly six lowercase hex digits: the only shorthand. Shorter, longer, and uppercase
/// forms are left to `TaskId::parse` to reject.
fn is_bare_suffix(s: &str) -> bool {
    s.len() == 6
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_six_lowercase_hex_digits_are_shorthand() {
        for good in ["2578c3", "000000", "abcdef"] {
            assert!(is_bare_suffix(good), "{good}");
        }
        for bad in ["2578c", "2578c3a", "2578C3", "2578g3", "sci-2578c3", ""] {
            assert!(!is_bare_suffix(bad), "{bad}");
        }
    }

    #[test]
    fn a_located_prefix_expands_without_touching_the_filesystem() {
        let shorthand = Shorthand::located(PathBuf::from("/nonexistent"), "sci".into());
        assert_eq!(
            shorthand.parse(&Registry::default(), "2578c3..").unwrap(),
            TaskId::parse("sci-2578c3").unwrap()
        );
        assert_eq!(
            shorthand.parse(&Registry::default(), "fam-2578c3").unwrap(),
            TaskId::parse("fam-2578c3").unwrap()
        );
    }

    #[test]
    fn outside_every_project_a_suffix_asks_for_the_full_id() {
        let dir = tempfile::tempdir().unwrap();
        let shorthand = Shorthand::new(dir.path().to_path_buf());
        match shorthand.parse(&Registry::default(), "2578c3") {
            Err(Error::InvalidId(input, detail)) => {
                assert_eq!(input, "2578c3");
                assert!(detail.contains("full id"), "{detail}");
            }
            other => panic!("{other:?}"),
        }
        assert!(shorthand.parse(&Registry::default(), "sci-2578c3").is_ok());
    }
}
