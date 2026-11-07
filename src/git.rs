use std::path::Path;
use std::process::Command;

/// A hook's repository and index must not redirect Git calls aimed at another path.
pub fn at(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn at_disables_git_optional_locks_and_clears_inherited_git_dirs() {
        let cmd = at(Path::new("."));
        let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();
        assert!(envs.contains(&(OsStr::new("GIT_OPTIONAL_LOCKS"), Some(OsStr::new("0")))));
        assert!(envs.contains(&(OsStr::new("LC_ALL"), Some(OsStr::new("C")))));
        for cleared in [
            "GIT_DIR",
            "GIT_COMMON_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
        ] {
            assert!(envs.contains(&(OsStr::new(cleared), None)));
        }
    }
}
