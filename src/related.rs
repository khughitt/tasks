//! Operations and helpers for related tasks and cross-record task citations.

use crate::model::TaskId;
use crate::registry::Registry;

/// A task-id source reference (§5.4): exactly one task id whose prefix this host's registry knows.
///
/// The id keeps its stored spelling; callers canonicalize for lookup.
pub fn source_task_id(registry: &Registry, source: &str) -> Option<TaskId> {
    TaskId::parse(source)
        .ok()
        .filter(|id| registry.knows_prefix(&id.prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn source_task_id_recognizes_registered_prefixes_only() {
        let mut r = Registry::default();
        r.register("aa", Path::new("/tmp/aa")).unwrap();
        r.aliases.insert("old".into(), "aa".into());
        r.groups.insert("grp".into(), vec!["aa".into()]);

        // Live prefix
        assert_eq!(
            source_task_id(&r, "aa-123456"),
            Some(TaskId::parse("aa-123456").unwrap())
        );
        // Retired prefix keeps stored spelling
        assert_eq!(
            source_task_id(&r, "old-123456"),
            Some(TaskId::parse("old-123456").unwrap())
        );
        // Group name is rejected
        assert_eq!(source_task_id(&r, "grp-123456"), None);
        // Unregistered prefix is rejected
        assert_eq!(source_task_id(&r, "zz-123456"), None);
        // Surrounding text or invalid grammar is rejected
        assert_eq!(source_task_id(&r, "from aa-123456"), None);
        assert_eq!(source_task_id(&r, " aa-123456"), None);
        assert_eq!(source_task_id(&r, "aa-123456 "), None);
        assert_eq!(source_task_id(&r, "aa-123456/x"), None);
    }
}
