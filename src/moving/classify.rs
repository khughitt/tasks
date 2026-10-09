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
        let here = s.source_files.get(&attachment.name) == Some(&want);
        let there = s.target_files.get(&attachment.name) == Some(&want);
        if !here && !there {
            return Recovery::Refuse(format!(
                "attachment {} has no copy with its journal digest in {} or {}; restore it from a verified backup, then rerun",
                attachment.name, j.source, j.target
            ));
        }
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
        Snapshot {
            journal: Journal {
                source: "dots-00000a".into(),
                target: "wali-00000b".into(),
                source_root: "/source".into(),
                target_root: "/target".into(),
                source_text: "a".into(),
                source_digest: digest(b"a"),
                target_text: "b".into(),
                tombstone_text: "c".into(),
                attachments: vec![],
                source_park: None,
                source_escalation: None,
                target_park: None,
                target_escalation: None,
            },
            source: FileState::Digest(digest(b"a")),
            target: FileState::Absent,
            source_files: BTreeMap::new(),
            target_files: BTreeMap::new(),
            target_park: None,
            target_escalation: None,
        }
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
        s.target = FileState::Digest(digest(b"b"));
        assert_eq!(classify(&s), Recovery::Resume);
    }

    #[test]
    fn tombstone_written_resumes() {
        let mut s = sample();
        s.source = FileState::Digest(digest(b"c"));
        s.target = FileState::Digest(digest(b"b"));
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
        s.source = FileState::Digest(digest(b"c"));
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
        let mut s = sample();
        s.journal.attachments = vec![JournalAttachment {
            name: "shot.png".into(),
            sha256: digest(b"png"),
        }];
        s.source = FileState::Digest(digest(b"c"));
        s.target = FileState::Digest(digest(b"b"));
        s.source_files.insert("shot.png".into(), FileState::Absent);
        s.target_files
            .insert("shot.png".into(), FileState::Digest(digest(b"x")));
        assert_refuses(&s, "shot.png");
    }

    #[test]
    fn attachment_in_either_dir_resumes() {
        let mut s = sample();
        let sha = digest(b"png");
        s.journal.attachments = vec![JournalAttachment {
            name: "shot.png".into(),
            sha256: sha.clone(),
        }];
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
        s.target = FileState::Digest(digest(b"b"));
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
        s.journal.target_escalation = Some(escalation.clone());
        s.target_escalation = Some(escalation);
        assert_eq!(classify(&s), Recovery::Resume);
        s.target_escalation.as_mut().unwrap().session = "codex:other".into();
        assert_refuses(&s, "wali-00000b");
    }
}
