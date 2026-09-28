//! An image from the Wayland clipboard through `wl-paste`.

use crate::error::{Error, Result};
use std::io::Read;
use std::process::{Command, Stdio};

/// Preferred MIME types, first match wins, with the extension each is stored under.
const PREFERRED: [(&str, &str); 4] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
];

pub struct Image {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

pub fn choose(offered: &[String]) -> Option<(&'static str, &'static str)> {
    PREFERRED
        .iter()
        .find(|(mime, _)| offered.iter().any(|type_| type_ == mime))
        .copied()
}

/// `clipboard-<yyyymmddThhmmssZ>.<ext>` from an RFC 3339 UTC stamp such as `time::now()`.
pub fn default_name(now: &str, extension: &str) -> String {
    let compact: String = now.chars().filter(|c| *c != '-' && *c != ':').collect();
    format!("clipboard-{compact}.{extension}")
}

fn unavailable(args: &[&str], stderr: &[u8]) -> Error {
    Error::ClipboardUnavailable(format!(
        "wl-paste {}: {}",
        args.join(" "),
        String::from_utf8_lossy(stderr).trim()
    ))
}

/// The offered types. A listing is a few lines, so it is read whole.
fn list_types() -> Result<Vec<u8>> {
    let args = ["--list-types"];
    let output = Command::new("wl-paste")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| Error::ClipboardUnavailable(format!("wl-paste: {error}")))?;
    if !output.status.success() {
        return Err(unavailable(&args, &output.stderr));
    }
    Ok(output.stdout)
}

/// The image's bytes, streamed through `attachments::read_capped`: at most `max + 1`
/// bytes are read, and a larger image kills and reaps `wl-paste` rather than draining it.
fn paste(mime: &str, max: u64) -> Result<Vec<u8>> {
    let args = ["--no-newline", "--type", mime];
    let mut child = Command::new("wl-paste")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::ClipboardUnavailable(format!("wl-paste: {error}")))?;
    // stderr drains on its own thread, so a chatty child cannot block on a full pipe
    // while this thread waits on stdout.
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let drain = std::thread::spawn(move || {
        let mut text = Vec::new();
        stderr.read_to_end(&mut text).map(|_| text)
    });
    let stdout = child.stdout.take().expect("stdout is piped");
    let bytes = match crate::attachments::read_capped(stdout, max, "the clipboard image") {
        Ok(bytes) => bytes,
        Err(error) => {
            // Not yet waited on, so the child is running or a zombie and kill succeeds.
            child.kill()?;
            child.wait()?;
            return Err(error);
        }
    };
    let status = child.wait()?;
    let stderr = drain.join().expect("the stderr reader does not panic")?;
    if !status.success() {
        return Err(unavailable(&args, &stderr));
    }
    Ok(bytes)
}

/// The preferred image on the clipboard, refused as `attachment_too_large` past `max`.
pub fn read(max: u64) -> Result<Image> {
    let listed = list_types()?;
    let offered: Vec<String> = String::from_utf8_lossy(&listed)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    let (mime, extension) = choose(&offered).ok_or_else(|| {
        Error::ClipboardNoImage(if offered.is_empty() {
            "the clipboard offers nothing".into()
        } else {
            format!("the clipboard offers no image: {}", offered.join(", "))
        })
    })?;
    Ok(Image {
        bytes: paste(mime, max)?,
        extension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_wins_over_jpeg_and_text_is_never_chosen() {
        let offered = |types: &[&str]| types.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        assert_eq!(
            choose(&offered(&["text/plain", "image/jpeg", "image/png"])),
            Some(("image/png", "png"))
        );
        assert_eq!(
            choose(&offered(&["image/jpeg"])),
            Some(("image/jpeg", "jpg"))
        );
        assert_eq!(choose(&offered(&["text/plain", "image/bmp"])), None);
    }

    #[test]
    fn default_names_compact_the_stamp() {
        assert_eq!(
            default_name("2026-09-28T10:39:21Z", "png"),
            "clipboard-20260928T103921Z.png"
        );
    }
}
