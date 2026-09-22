pub mod ancestry;
pub mod resolve;
pub mod snapshot;

use crate::config::HostConfig;
use crate::error::{Error, Result};

/// Whether the relay level is configured on. Read by callers that treat a relay-level
/// failure differently from a native one.
pub fn enabled() -> Result<bool> {
    Ok(HostConfig::load()?.relay_identity)
}

/// Stage 3 of spec §7, asking what the host offers rather than what the target triple
/// says: a Linux build without a mounted `/proc` cannot establish scope either. The
/// refusal belongs to the relay level, not to startup, so a session with `TASKS_SESSION`
/// set never reaches it and it can never take away the recovery path.
pub fn stage_platform(enabled: bool, proc_available: bool) -> Result<()> {
    if enabled && !proc_available {
        return Err(Error::Config(
            "relay identity is enabled but this host does not expose /proc, so ancestry \
             cannot be established. Disable [identity].relay, or set TASKS_SESSION and \
             TASKS_SESSION_PID to name this session explicitly."
                .into(),
        ));
    }
    Ok(())
}

/// Stages 2, 3 and 4 of spec §7, in order. `Ok(None)` means relay is off or the caller is
/// out of scope; either way the native ladder applies unchanged.
pub fn level() -> Result<Option<resolve::Resolved>> {
    // Stage 2: configuration. Nothing below runs when relay is not enabled.
    if !enabled()? {
        return Ok(None);
    }
    // Stage 3: platform support.
    stage_platform(true, ancestry::proc_available())?;
    // Stage 4: ancestry, and only then the registry. `resolve` decides `Outside` and
    // `Unknown` before calling the loader, so an unknown-ancestry caller is refused as
    // such rather than by whatever the registry read would have said.
    resolve::resolve(
        ancestry::current_scope(),
        snapshot::load,
        &crate::claims::hostname(),
        crate::claims::boot_id().as_deref(),
        &|key| {
            std::env::var_os(key)
                .and_then(|value| value.into_string().ok())
                .filter(|value| !value.is_empty())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stage_platform_refusal_needs_both_relay_on_and_no_proc() {
        // Relay off: the stage is never reached, whatever the host offers.
        assert!(stage_platform(false, false).is_ok());
        assert!(stage_platform(false, true).is_ok());
        // Relay on: a host with a readable process tree passes; one without is refused.
        assert!(stage_platform(true, true).is_ok());
        let error = stage_platform(true, false).unwrap_err().to_string();
        assert!(error.contains("/proc"), "{error}");
        assert!(error.contains("TASKS_SESSION"), "{error}");
    }
}
