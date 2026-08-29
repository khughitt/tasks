use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no tasks/.config.toml found above {0}")]
    NoProject(PathBuf),
    #[error("{0}")]
    Config(String),
    #[error("task {0} not found")]
    TaskNotFound(String),
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
    #[error("invalid transition {0} -> {1}")]
    InvalidTransition(String, String),
    #[error("dependency cycle: {0}")]
    Cycle(String),
    #[error("{0}")]
    Ambiguous(String),
    #[error("{0}")]
    DocNotFound(String),
    #[error("{0} changed on disk during edit; your edit is kept at {1}")]
    ConcurrentModification(String, String),
    #[error("{0}")]
    Editor(String),
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
            Error::Config(detail) => Error::Config(detail + suffix),
            Error::TaskNotFound(detail) => Error::TaskNotFound(detail + suffix),
            Error::UnresolvableId(detail) => Error::UnresolvableId(detail + suffix),
            Error::InvalidId(id, detail) => Error::InvalidId(id, detail + suffix),
            Error::Parse { file, detail } => Error::Parse {
                file,
                detail: detail + suffix,
            },
            Error::Validation(detail) => Error::Validation(detail + suffix),
            Error::OpenDependencies(id, detail) => Error::OpenDependencies(id, detail + suffix),
            Error::InvalidTransition(from, to) => Error::InvalidTransition(from, to + suffix),
            Error::Cycle(detail) => Error::Cycle(detail + suffix),
            Error::Ambiguous(detail) => Error::Ambiguous(detail + suffix),
            Error::DocNotFound(detail) => Error::DocNotFound(detail + suffix),
            Error::ConcurrentModification(id, path) => {
                Error::ConcurrentModification(id, path + suffix)
            }
            Error::Editor(detail) => Error::Editor(detail + suffix),
            Error::Io(detail) => Error::Io(detail + suffix),
            error @ Error::NoProject(_) => error,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Error::NoProject(_) => "no_project",
            Error::Config(_) => "config",
            Error::TaskNotFound(_) => "task_not_found",
            Error::UnresolvableId(_) => "unresolvable_id",
            Error::InvalidId(..) => "invalid_id",
            Error::Parse { .. } => "parse",
            Error::Validation(_) => "validation",
            Error::OpenDependencies(..) => "open_dependencies",
            Error::InvalidTransition(..) => "invalid_transition",
            Error::Cycle(_) => "cycle",
            Error::Ambiguous(_) => "ambiguous",
            Error::DocNotFound(_) => "doc_not_found",
            Error::ConcurrentModification(..) => "concurrent_modification",
            Error::Editor(_) => "editor",
            Error::Io(_) => "io",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
