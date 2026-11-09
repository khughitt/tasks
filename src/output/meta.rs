use std::path::Path;

use crate::cli::Command;

use super::contract::Output;

/// Unified per-output metadata: whether pretty mode needs terminal theme colors, whether a
/// palette without magenta costs priority colors, and the warnings the output carries.
pub struct OutputMeta {
    pub needs_theme: bool,
    pub shows_priority: bool,
    pub warnings: Vec<String>,
}

impl OutputMeta {
    fn plain(warnings: &[String]) -> Self {
        Self {
            needs_theme: false,
            shows_priority: false,
            warnings: warnings.to_vec(),
        }
    }

    fn themed(warnings: &[String]) -> Self {
        Self {
            needs_theme: true,
            shows_priority: false,
            warnings: warnings.to_vec(),
        }
    }

    fn priority(warnings: &[String]) -> Self {
        Self {
            needs_theme: true,
            shows_priority: true,
            warnings: warnings.to_vec(),
        }
    }
}

impl Output {
    pub fn meta(&self) -> OutputMeta {
        match self {
            Output::RetainedState(o) => OutputMeta::plain(&o.warnings),
            Output::RetainedMutation(o) => OutputMeta::plain(&o.warnings),
            Output::AttachmentEligibility(_) => OutputMeta::plain(&[]),
            Output::Init(o) => OutputMeta::plain(&o.warnings),
            Output::Retire(o) => OutputMeta::plain(&o.warnings),
            Output::Rename(o) => OutputMeta::plain(&o.warnings),
            Output::Move(o) => OutputMeta::plain(&o.warnings),
            Output::MigrateDraft(o) => OutputMeta::plain(&o.warnings),
            Output::Migrate(o) => OutputMeta::plain(&o.warnings),
            Output::Id(o) => OutputMeta::plain(&o.warnings),
            Output::Commit(o) => OutputMeta::plain(&o.warnings),
            Output::Add(o) => OutputMeta::plain(&o.warnings),
            Output::Root(o) => OutputMeta::plain(&o.warnings),
            Output::Where(o) => OutputMeta::plain(&o.warnings),
            Output::Resolve(o) => OutputMeta::plain(&o.warnings),
            Output::IdentityRefresh(o) => OutputMeta::plain(&o.warnings),
            Output::Group(o) => OutputMeta::plain(&o.warnings),
            Output::Groups(o) => OutputMeta::plain(&o.warnings),
            Output::Projects(o) => OutputMeta::themed(&o.warnings),
            Output::Show(o) => OutputMeta::priority(&o.warnings),
            Output::Doc(o) => OutputMeta::plain(&o.warnings),
            Output::Raw(o) => OutputMeta::plain(&o.warnings),
            Output::Export(o) => OutputMeta::plain(&o.warnings),
            Output::Next(o) => OutputMeta::priority(&o.warnings),
            Output::List(o) => OutputMeta::priority(&o.warnings),
            Output::Parked(o) => OutputMeta::themed(&o.warnings),
            Output::Quiet(o) => OutputMeta::priority(&o.warnings),
            Output::Goals(o) => OutputMeta::priority(&o.warnings),
            Output::Claims(_) => OutputMeta::plain(&[]),
            Output::Prime(o) => OutputMeta::priority(&o.warnings),
            Output::Lanes(o) => OutputMeta::priority(&o.warnings),
            Output::Blockers(o) => OutputMeta::priority(&o.warnings),
            Output::Graph(o) => OutputMeta::plain(&o.warnings),
            Output::Check(o) => OutputMeta {
                needs_theme: false,
                shows_priority: false,
                warnings: o
                    .warnings
                    .iter()
                    .map(|finding| {
                        format!("{} [{}] {}", finding.file, finding.kind, finding.detail)
                    })
                    .collect(),
            },
            Output::Tree(o) => OutputMeta::priority(&o.warnings),
            Output::Tags(o) => OutputMeta::plain(&o.warnings),
            Output::Docs(o) => OutputMeta::plain(&o.warnings),
            Output::Feedback(o) => OutputMeta::plain(&o.warnings),
            Output::ArchivePreview(_) => OutputMeta::plain(&[]),
            Output::ArchiveApply(o) => OutputMeta::plain(&o.warnings),
            Output::Attach(o) => OutputMeta::plain(&o.warnings),
            Output::Detach(o) => OutputMeta::plain(&o.warnings),
        }
    }
}

/// Whether `command` can run and emit its warnings before a terminal-width error is raised.
/// Read commands and `export --output -` (which writes raw bytes) defer width errors; writers
/// fail before running so a presentation error cannot follow a successful mutation.
pub fn defers_width_error(command: &Command) -> bool {
    match command {
        Command::Export {
            output: Some(path), ..
        } => path == Path::new("-"),
        Command::Projects { .. }
        | Command::Groups
        | Command::Root { .. }
        | Command::Where { .. }
        | Command::Resolve { .. }
        | Command::Show { .. }
        | Command::Doc { .. }
        | Command::Docs { .. }
        | Command::List { .. }
        | Command::Search { .. }
        | Command::Ready { .. }
        | Command::Next { .. }
        | Command::Sample { .. }
        | Command::Blockers(_)
        | Command::Graph { .. }
        | Command::Attachments { .. }
        | Command::Check { .. }
        | Command::Prime { .. }
        | Command::Lanes { .. }
        | Command::Tree { .. }
        | Command::Tags { .. }
        | Command::Quiet { .. }
        | Command::Goals { .. }
        | Command::Claims { .. } => true,
        Command::Retained { .. }
        | Command::Init { .. }
        | Command::Identity { .. }
        | Command::Rename { .. }
        | Command::Move { .. }
        | Command::Migrate { .. }
        | Command::Unregister { .. }
        | Command::Retire { .. }
        | Command::Unretire
        | Command::Group { .. }
        | Command::Add { .. }
        | Command::Export { output: None, .. }
        | Command::Edit { .. }
        | Command::Note { .. }
        | Command::Attach { .. }
        | Command::ArchiveNotes { .. }
        | Command::Detach { .. }
        | Command::Start { .. }
        | Command::Park { .. }
        | Command::Done { .. }
        | Command::Drop { .. }
        | Command::Release { .. }
        | Command::Block { .. }
        | Command::Unblock { .. }
        | Command::Shelve { .. }
        | Command::Unshelve { .. }
        | Command::Dep { .. }
        | Command::Commit { .. }
        | Command::Feedback { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{DateColumn, IdOut, ListOut, NextOut, ParkedOut};

    #[test]
    fn theme_gates_cover_dates_and_priorities() {
        let list = Output::List(ListOut {
            tasks: vec![],
            halts: vec![],
            warnings: vec![],
            date: DateColumn::Updated,
        });
        assert!(list.meta().needs_theme);
        assert!(list.meta().shows_priority);
        let show = Output::Next(Box::new(NextOut {
            next: None,
            halts: vec![],
            warnings: vec![],
        }));
        assert!(show.meta().needs_theme);
        assert!(show.meta().shows_priority);
        let parked = Output::Parked(ParkedOut {
            tasks: vec![],
            warnings: vec![],
        });
        assert!(parked.meta().needs_theme);
        assert!(!parked.meta().shows_priority);
        let id = Output::Id(IdOut {
            id: "xx-000001".into(),
            warnings: vec![],
        });
        assert!(!id.meta().needs_theme);
        assert!(!id.meta().shows_priority);
    }
}
