//! The caller's PID view: the PID namespace its `/proc` numbers pids in (claim PID-view
//! design §1). A pid means something only together with the view it was read through.

use std::sync::OnceLock;

/// nsfs inode of the initial PID namespace, a fixed kernel constant (`PROC_PID_INIT_INO`
/// in include/linux/proc_ns.h). A kernel that changed it would make every view read as
/// non-initial, which only withholds views (spec §2), never misjudges a claim.
pub const PROC_PID_INIT_INO: u64 = 0xEFFF_FFFC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PidView {
    pub dev: u64,
    pub ino: u64,
}

impl PidView {
    pub fn is_initial(&self) -> bool {
        self.ino == PROC_PID_INIT_INO
    }
}

impl std::fmt::Display for PidView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.dev, self.ino)
    }
}

/// `status` is `/proc/self/status`; `ns` is the `(st_dev, st_ino)` of `/proc/self/ns/pid`,
/// following the link. `NSpid` lists the caller's pid from the procfs's namespace down to
/// its own, so exactly one entry means the procfs numbers in the caller's namespace. Any
/// other count, an entry that is not a positive pid, or any failed read, is unknown
/// (spec §1).
pub fn from_parts(status: Option<&str>, ns: Option<(u64, u64)>) -> Option<PidView> {
    let nspid = status?
        .lines()
        .find_map(|line| line.strip_prefix("NSpid:"))?;
    let mut entries = nspid.split_whitespace();
    let pid: u32 = entries.next()?.parse().ok()?;
    if pid == 0 || entries.next().is_some() {
        return None;
    }
    let (dev, ino) = ns?;
    Some(PidView { dev, ino })
}

/// The caller's view, read once per process: a process does not change PID namespace.
pub fn current() -> Option<PidView> {
    static VIEW: OnceLock<Option<PidView>> = OnceLock::new();
    *VIEW.get_or_init(|| {
        use std::os::unix::fs::MetadataExt;
        let status = std::fs::read_to_string("/proc/self/status").ok();
        let ns = std::fs::metadata("/proc/self/ns/pid")
            .ok()
            .map(|meta| (meta.dev(), meta.ino()));
        from_parts(status.as_deref(), ns)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = "Name:\tx\nNSpid:\t1584044\nNSpgid:\t1\n";

    #[test]
    fn one_nspid_entry_names_the_callers_namespace() {
        assert_eq!(
            from_parts(Some(ONE), Some((5, 4026531836))),
            Some(PidView {
                dev: 5,
                ino: 4026531836
            })
        );
    }

    #[test]
    fn an_ancestor_procfs_is_unknown() {
        let two = "NSpid:\t1678460\t2\n";
        assert_eq!(from_parts(Some(two), Some((5, 4026533721))), None);
    }

    #[test]
    fn a_missing_or_unreadable_input_is_unknown() {
        assert_eq!(from_parts(None, Some((5, 1))), None, "status unreadable");
        assert_eq!(
            from_parts(Some("Name:\tx\n"), Some((5, 1))),
            None,
            "no NSpid line"
        );
        assert_eq!(
            from_parts(Some("NSpid:\t\n"), Some((5, 1))),
            None,
            "empty NSpid"
        );
        assert_eq!(
            from_parts(Some("NSpid:\tgarbage\n"), Some((5, 1))),
            None,
            "not a pid"
        );
        assert_eq!(
            from_parts(Some("NSpid:\t0\n"), Some((5, 1))),
            None,
            "pid 0 is no process"
        );
        assert_eq!(
            from_parts(Some("NSpid:\t-4\n"), Some((5, 1))),
            None,
            "negative"
        );
        assert_eq!(
            from_parts(Some(ONE), None),
            None,
            "namespace link unreadable"
        );
    }

    #[test]
    fn only_the_init_inode_is_initial() {
        assert!(
            PidView {
                dev: 5,
                ino: PROC_PID_INIT_INO
            }
            .is_initial()
        );
        assert!(
            !PidView {
                dev: 5,
                ino: 4026533720
            }
            .is_initial()
        );
        assert_eq!(PROC_PID_INIT_INO, 4026531836);
    }

    #[test]
    fn a_view_formats_as_dev_colon_ino() {
        assert_eq!(PidView { dev: 5, ino: 7 }.to_string(), "5:7");
    }
}
