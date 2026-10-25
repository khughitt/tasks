use super::journal::Journal;
use crate::claims::{Escalation, Park};
use crate::rename::inventory::digest;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    Absent,
    Digest(String),
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub journal: Journal,
    pub source: FileState,
    pub target: FileState,
    pub source_files: BTreeMap<String, FileState>,
    pub target_files: BTreeMap<String, FileState>,
    pub source_park: Option<Park>,
    pub source_escalation: Option<Escalation>,
    pub target_park: Option<Park>,
    pub target_escalation: Option<Escalation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    Resume,
    Refuse(String),
}

pub fn classify(s: &Snapshot) -> Recovery {
    let j = &s.journal;
    let original = FileState::Digest(j.source_digest.clone());
    let tombstone = FileState::Digest(digest(j.tombstone_text.as_bytes()));
    let target = FileState::Digest(digest(j.target_text.as_bytes()));
    if s.source != original && s.source != tombstone {
        return Recovery::Refuse(format!(
            "{} no longer matches the journal or the tombstone it would write; it changed during the move",
            j.source
        ));
    }
    if s.target != FileState::Absent && s.target != target {
        return Recovery::Refuse(format!(
            "{} exists with bytes the journal did not write",
            j.target
        ));
    }
    if s.source == tombstone && s.target == FileState::Absent {
        return Recovery::Refuse(format!(
            "{} is already a tombstone but {} is missing",
            j.source, j.target
        ));
    }
    for attachment in &j.attachments {
        let want = FileState::Digest(attachment.sha256.clone());
        for (id, files) in [(&j.source, &s.source_files), (&j.target, &s.target_files)] {
            if let Some(FileState::Digest(actual)) = files.get(&attachment.name)
                && actual != &attachment.sha256
            {
                return Recovery::Refuse(format!(
                    "attachment {} in {id} differs from its journal digest",
                    attachment.name
                ));
            }
        }
        let here = s.source_files.get(&attachment.name) == Some(&want);
        let there = s.target_files.get(&attachment.name) == Some(&want);
        if !here && !there {
            return Recovery::Refuse(format!(
                "attachment {} has no copy with its journal digest in {} or {}; restore it from a verified backup, then rerun",
                attachment.name, j.source, j.target
            ));
        }
    }
    if s.source_park.is_some() && s.source_park != j.source_park {
        return Recovery::Refuse(format!("{}'s park differs from the journal's", j.source));
    }
    if s.source_escalation.is_some() && s.source_escalation != j.source_escalation {
        return Recovery::Refuse(format!(
            "{}'s escalation differs from the journal's",
            j.source
        ));
    }
    if s.target_park.is_some() && s.target_park != j.target_park {
        return Recovery::Refuse(format!(
            "{}'s park differs from the one the journal carries",
            j.target
        ));
    }
    if s.target_escalation.is_some() && s.target_escalation != j.target_escalation {
        return Recovery::Refuse(format!(
            "{}'s escalation differs from the journal's",
            j.target
        ));
    }
    Recovery::Resume
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::WaitingOn;
    use crate::model::Complexity;
    use crate::moving::journal::JournalAttachment;

    fn sample() -> Snapshot {
        let journal = super::super::journal::tests::sample();
        Snapshot {
            source: FileState::Digest(journal.source_digest.clone()),
            journal,
            target: FileState::Absent,
            source_files: BTreeMap::new(),
            target_files: BTreeMap::new(),
            source_park: None,
            source_escalation: None,
            target_park: None,
            target_escalation: None,
        }
    }

    fn sample_with_attachment() -> Snapshot {
        let mut s = sample();
        let attached = crate::model::Note {
            at: "2026-10-01T00:00:00Z".into(),
            by: "owner".into(),
            text: crate::attachments::attached_note("shot.png", 3, None),
            provenance: None,
            occurrence: Default::default(),
        };
        for (text, id, tombstone) in [
            (&mut s.journal.source_text, "dots-00000a", false),
            (&mut s.journal.target_text, "wali-00000b", false),
            (&mut s.journal.tombstone_text, "dots-00000a", true),
        ] {
            let mut task = crate::format::parse_task(text, &format!("{id}.md")).unwrap();
            task.notes.insert(0, attached.clone());
            if tombstone {
                crate::commands::append_note(
                    &mut task,
                    "owner",
                    &crate::attachments::detached_note("shot.png", "moved to wali-00000b"),
                )
                .unwrap();
            }
            *text = crate::format::serialize_task(&task);
        }
        s.journal.source_digest = digest(s.journal.source_text.as_bytes());
        s.source = FileState::Digest(s.journal.source_digest.clone());
        s.journal.attachments = vec![JournalAttachment {
            name: "shot.png".into(),
            sha256: digest(b"png"),
        }];
        s.journal
            .validate(std::path::Path::new("dots-00000a.toml"))
            .unwrap();
        s
    }

    fn assert_refuses(snapshot: &Snapshot, name: &str) {
        match classify(snapshot) {
            Recovery::Refuse(reason) => assert!(reason.contains(name), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn untouched_source_resumes() {
        assert_eq!(classify(&sample()), Recovery::Resume);
    }

    #[test]
    fn destination_written_resumes() {
        let mut s = sample();
        s.target = FileState::Digest(digest(s.journal.target_text.as_bytes()));
        assert_eq!(classify(&s), Recovery::Resume);
    }

    #[test]
    fn tombstone_written_resumes() {
        let mut s = sample();
        s.source = FileState::Digest(digest(s.journal.tombstone_text.as_bytes()));
        s.target = FileState::Digest(digest(s.journal.target_text.as_bytes()));
        assert_eq!(classify(&s), Recovery::Resume);
    }

    #[test]
    fn hand_edited_source_refuses() {
        let mut s = sample();
        s.source = FileState::Digest(digest(b"x"));
        assert_refuses(&s, "dots-00000a");
    }

    #[test]
    fn foreign_destination_refuses() {
        let mut s = sample();
        s.target = FileState::Digest(digest(b"x"));
        assert_refuses(&s, "wali-00000b");
    }

    #[test]
    fn tombstone_without_destination_refuses() {
        let mut s = sample();
        s.source = FileState::Digest(digest(s.journal.tombstone_text.as_bytes()));
        assert_refuses(&s, "wali-00000b");
    }

    #[test]
    fn source_gone_refuses() {
        let mut s = sample();
        s.source = FileState::Absent;
        assert_refuses(&s, "dots-00000a");
    }

    #[test]
    fn attachment_with_no_matching_copy_refuses() {
        let mut s = sample_with_attachment();
        s.source = FileState::Digest(digest(s.journal.tombstone_text.as_bytes()));
        s.target = FileState::Digest(digest(s.journal.target_text.as_bytes()));
        s.source_files.insert("shot.png".into(), FileState::Absent);
        s.target_files
            .insert("shot.png".into(), FileState::Digest(digest(b"x")));
        assert_refuses(&s, "shot.png");
    }

    #[test]
    fn attachment_in_either_dir_resumes() {
        let mut s = sample_with_attachment();
        let sha = digest(b"png");
        s.source_files
            .insert("shot.png".into(), FileState::Digest(sha.clone()));
        s.target_files.insert("shot.png".into(), FileState::Absent);
        assert_eq!(classify(&s), Recovery::Resume);
        s.source_files.insert("shot.png".into(), FileState::Absent);
        s.target_files
            .insert("shot.png".into(), FileState::Digest(sha));
        assert_eq!(classify(&s), Recovery::Resume);
    }

    #[test]
    fn different_destination_park_refuses() {
        let mut s = sample();
        s.target = FileState::Digest(digest(s.journal.target_text.as_bytes()));
        let park = Park {
            owner: "owner".into(),
            session: "codex:test".into(),
            host: "host".into(),
            worktree: "/target".into(),
            at: "2026-10-09T00:00:00Z".into(),
            next_step: "continue".into(),
            waiting_on: WaitingOn::Agent,
            reason: None,
            needs: None,
            minutes: None,
            title: "task".into(),
        };
        s.journal.source_park = Some(park.clone());
        s.journal.target_park = Some(park.clone());
        s.target_park = Some(park);
        assert_eq!(classify(&s), Recovery::Resume);
        s.target_park.as_mut().unwrap().next_step = "changed".into();
        assert_refuses(&s, "wali-00000b");
    }

    #[test]
    fn different_destination_escalation_refuses() {
        let mut s = sample();
        let escalation = Escalation {
            level: Complexity::High,
            at: "2026-10-09T00:00:00Z".into(),
            session: "codex:test".into(),
        };
        s.journal.source_escalation = Some(escalation.clone());
        s.journal.target_escalation = Some(escalation.clone());
        s.target_escalation = Some(escalation);
        assert_eq!(classify(&s), Recovery::Resume);
        s.target_escalation.as_mut().unwrap().session = "codex:other".into();
        assert_refuses(&s, "wali-00000b");
    }
}
