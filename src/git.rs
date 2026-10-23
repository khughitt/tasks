use std::path::Path;
use std::process::Command;

/// A hook's repository and index must not redirect Git calls aimed at another path.
pub fn at(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .env("LC_ALL", "C")
        .env_remove("GIT_DIR")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    command
}
