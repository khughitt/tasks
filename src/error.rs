use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("prime --summary requires JSON; use tasks --json prime --summary")]
    SummaryRequiresJson,
    #[error(
        "prime summary exceeds 12288 bytes with required metadata; rerun the same JSON prime without --summary and capture stdout to a file"
    )]
    SummaryTooLarge,
    #[error("no tasks/.config.toml found above {0}")]
    NoProject(PathBuf),
    #[error("no tasks/.config.toml found above {dir}; {next}")]
    NoProjectNext { dir: PathBuf, next: String },
    #[error("{0}")]
    Config(String),
    #[error("task {0} not found")]
    TaskNotFound(String),
    /// `task_not_found` with what to do next, after the fact rather than inside it.
    #[error("task {id} not found; {next}")]
    TaskNotFoundNext { id: String, next: String },
    #[error("cannot resolve {0}")]
    UnresolvableId(String),
    #[error("invalid id {0:?}: {1}")]
    InvalidId(String, String),
    #[error("{file}: {detail}")]
    Parse { file: String, detail: String },
    #[error("{0}")]
    Validation(String),
    #[error("{0} has open dependencies: {1}")]
    OpenDependencies(String, String),
    #[error("{0} has open descendants: {1}")]
    OpenDescendants(String, String),
    #[error("invalid transition {0} -> {1}")]
    InvalidTransition(String, String),
    #[error("dependency cycle: {0}")]
    Cycle(String),
    #[error("{0}")]
    Ambiguous(String),
    #[error("{0}")]
    DocNotFound(String),
    #[error("{0}")]
    SpecialFile(String),
    #[error("{detail}")]
    Reference { kind: &'static str, detail: String },
    #[error("{0}")]
    RefNotFound(String),
    #[error("{0}")]
    RefOutputMissing(String),
    #[error("{0}")]
    RefCitesAttachment(String),
    #[error("{0}")]
    RefNotText(String),
    #[error("{0} changed on disk; {1}")]
    ConcurrentModification(String, String),
    #[error("{0} is claimed by {1}")]
    Claimed(String, String),
    #[error("{0}")]
    StaleCopy(String),
    #[error("{0}")]
    Moved(String),
    #[error("{0}")]
    MovePending(String),
    #[error("{0}")]
    RetiredProject(String),
    #[error("{0}")]
    OpenTasks(String),
    #[error("{0}")]
    UnreadableCheckout(String),
    #[error("{0}")]
    NotRegisteredCheckout(String),
    #[error("{0}")]
    CheckoutConflict(String),
    #[error("{0}")]
    RecordsMoved(String),
    #[error("{0}")]
    RecordsUnpaired(String),
    #[error("{0}")]
    RecordsInTree(String),
    #[error("{0}")]
    Halted(String),
    #[error("{0}")]
    NestedLane(String),
    #[error("{0}")]
    NeedHeld(String),
    #[error("{0}")]
    UnknownNeed(String),
    #[error("{0}")]
    UnknownGroup(String),
    #[error("{0}")]
    Editor(String),
    #[error("{0}")]
    InvalidAttachmentName(String),
    #[error("{0}")]
    AttachmentExists(String),
    #[error("{0}")]
    AttachmentMissing(String),
    #[error("{0}")]
    AttachmentUnsafe(String),
    #[error("{0}")]
    AttachmentTooLarge(String),
    #[error("{0}")]
    ClipboardUnavailable(String),
    #[error("{0}")]
    ClipboardNoImage(String),
    #[error("io: {0}")]
    Io(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Error {
        Error::Io(e.to_string())
    }
}

impl Error {
    /// Appends `suffix` to the human-readable detail; the kind is unchanged.
    pub fn with_suffix(self, suffix: &str) -> Error {
        match self {
            Error::NoProjectNext { dir, next } => Error::NoProjectNext {
                dir,
                next: next + suffix,
            },
            Error::Config(detail) => Error::Config(detail + suffix),
            Error::TaskNotFound(detail) => Error::TaskNotFound(detail + suffix),
            Error::TaskNotFoundNext { id, next } => Error::TaskNotFoundNext {
                id,
                next: next + suffix,
            },
            Error::UnresolvableId(detail) => Error::UnresolvableId(detail + suffix),
            Error::InvalidId(id, detail) => Error::InvalidId(id, detail + suffix),
            Error::Parse { file, detail } => Error::Parse {
                file,
                detail: detail + suffix,
            },
            Error::Validation(detail) => Error::Validation(detail + suffix),
            Error::OpenDependencies(id, detail) => Error::OpenDependencies(id, detail + suffix),
            Error::OpenDescendants(id, detail) => Error::OpenDescendants(id, detail + suffix),
            Error::InvalidTransition(from, to) => Error::InvalidTransition(from, to + suffix),
            Error::Cycle(detail) => Error::Cycle(detail + suffix),
            Error::Ambiguous(detail) => Error::Ambiguous(detail + suffix),
            Error::DocNotFound(detail) => Error::DocNotFound(detail + suffix),
            Error::SpecialFile(detail) => Error::SpecialFile(detail + suffix),
            Error::Reference { kind, detail } => Error::Reference {
                kind,
                detail: detail + suffix,
            },
            Error::RefNotFound(detail) => Error::RefNotFound(detail + suffix),
            Error::RefOutputMissing(detail) => Error::RefOutputMissing(detail + suffix),
            Error::RefCitesAttachment(detail) => Error::RefCitesAttachment(detail + suffix),
            Error::RefNotText(detail) => Error::RefNotText(detail + suffix),
            Error::ConcurrentModification(id, path) => {
                Error::ConcurrentModification(id, path + suffix)
            }
            Error::Claimed(id, detail) => Error::Claimed(id, detail + suffix),
            Error::StaleCopy(detail) => Error::StaleCopy(detail + suffix),
            Error::Moved(detail) => Error::Moved(detail + suffix),
            Error::MovePending(detail) => Error::MovePending(detail + suffix),
            Error::RetiredProject(detail) => Error::RetiredProject(detail + suffix),
            Error::OpenTasks(detail) => Error::OpenTasks(detail + suffix),
            Error::UnreadableCheckout(detail) => Error::UnreadableCheckout(detail + suffix),
            Error::NotRegisteredCheckout(detail) => Error::NotRegisteredCheckout(detail + suffix),
            Error::CheckoutConflict(detail) => Error::CheckoutConflict(detail + suffix),
            Error::RecordsMoved(detail) => Error::RecordsMoved(detail + suffix),
            Error::RecordsUnpaired(detail) => Error::RecordsUnpaired(detail + suffix),
            Error::RecordsInTree(detail) => Error::RecordsInTree(detail + suffix),
            Error::Halted(detail) => Error::Halted(detail + suffix),
            Error::NestedLane(detail) => Error::NestedLane(detail + suffix),
            Error::NeedHeld(detail) => Error::NeedHeld(detail + suffix),
            Error::UnknownNeed(detail) => Error::UnknownNeed(detail + suffix),
            Error::UnknownGroup(detail) => Error::UnknownGroup(detail + suffix),
            Error::Editor(detail) => Error::Editor(detail + suffix),
            Error::InvalidAttachmentName(detail) => Error::InvalidAttachmentName(detail + suffix),
            Error::AttachmentExists(detail) => Error::AttachmentExists(detail + suffix),
            Error::AttachmentMissing(detail) => Error::AttachmentMissing(detail + suffix),
            Error::AttachmentUnsafe(detail) => Error::AttachmentUnsafe(detail + suffix),
            Error::AttachmentTooLarge(detail) => Error::AttachmentTooLarge(detail + suffix),
            Error::ClipboardUnavailable(detail) => Error::ClipboardUnavailable(detail + suffix),
            Error::ClipboardNoImage(detail) => Error::ClipboardNoImage(detail + suffix),
            Error::Io(detail) => Error::Io(detail + suffix),
            error @ (Error::NoProject(_) | Error::SummaryRequiresJson | Error::SummaryTooLarge) => {
                error
            }
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Error::SummaryRequiresJson => "summary_requires_json",
            Error::SummaryTooLarge => "summary_too_large",
            Error::NoProject(_) | Error::NoProjectNext { .. } => "no_project",
            Error::Config(_) => "config",
            Error::TaskNotFound(_) | Error::TaskNotFoundNext { .. } => "task_not_found",
            Error::UnresolvableId(_) => "unresolvable_id",
            Error::InvalidId(..) => "invalid_id",
            Error::Parse { .. } => "parse",
            Error::Validation(_) => "validation",
            Error::OpenDependencies(..) => "open_dependencies",
            Error::OpenDescendants(..) => "open_descendants",
            Error::InvalidTransition(..) => "invalid_transition",
            Error::Cycle(_) => "cycle",
            Error::Ambiguous(_) => "ambiguous",
            Error::DocNotFound(_) => "doc_not_found",
            Error::SpecialFile(_) => "special_file",
            Error::Reference { kind, .. } => kind,
            Error::RefNotFound(_) => "ref_not_found",
            Error::RefOutputMissing(_) => "ref_output_missing",
            Error::RefCitesAttachment(_) => "ref_cites_attachment",
            Error::RefNotText(_) => "ref_not_text",
            Error::ConcurrentModification(..) => "concurrent_modification",
            Error::Claimed(..) => "claimed",
            Error::StaleCopy(_) => "stale_copy",
            Error::Moved(_) => "moved",
            Error::MovePending(_) => "move_pending",
            Error::RetiredProject(_) => "retired_project",
            Error::OpenTasks(_) => "open_tasks",
            Error::UnreadableCheckout(_) => "unreadable_checkout",
            Error::NotRegisteredCheckout(_) => "not_registered_checkout",
            Error::CheckoutConflict(_) => "checkout_conflict",
            Error::RecordsMoved(_) => "records_moved",
            Error::RecordsUnpaired(_) => "records_unpaired",
            Error::RecordsInTree(_) => "records_in_tree",
            Error::Halted(_) => "halted",
            Error::NestedLane(_) => "nested_lane",
            Error::NeedHeld(_) => "need_held",
            Error::UnknownNeed(_) => "unknown_need",
            Error::UnknownGroup(_) => "unknown_group",
            Error::Editor(_) => "editor",
            Error::InvalidAttachmentName(_) => "invalid_attachment_name",
            Error::AttachmentExists(_) => "attachment_exists",
            Error::AttachmentMissing(_) => "attachment_missing",
            Error::AttachmentUnsafe(_) => "attachment_unsafe",
            Error::AttachmentTooLarge(_) => "attachment_too_large",
            Error::ClipboardUnavailable(_) => "clipboard_unavailable",
            Error::ClipboardNoImage(_) => "clipboard_no_image",
            Error::Io(_) => "io",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
