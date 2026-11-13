//! Debug-build-only observations and deterministic test barriers; never bypass validation.
use crate::error::Result;

pub(crate) fn observe(event: &str, detail: &str) -> Result<()> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("TASKS_RETAINED_OBSERVER") {
        use std::{
            io::{Read, Write},
            os::unix::net::UnixStream,
            sync::OnceLock,
            time::{Duration, Instant},
        };
        static START: OnceLock<Instant> = OnceLock::new();
        let elapsed = START.get_or_init(Instant::now).elapsed().as_nanos();
        let mut stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;
        let bytes = serde_json::to_vec(
            &serde_json::json!({"event":event,"detail":detail,"elapsed_ns":elapsed}),
        )
        .map_err(super::model::invalid)?;
        if bytes.len() > 4096 {
            return Err(super::model::invalid("test observation exceeds bound"));
        }
        stream.write_all(&bytes)?;
        stream.write_all(b"\n")?;
        let mut ack = [0];
        stream.read_exact(&mut ack)?;
        if ack != *b"+" {
            return Err(super::model::invalid("test observer refused continuation"));
        }
    }
    let _ = (event, detail);
    Ok(())
}
