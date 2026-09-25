//! The terminal theme's colors: RGB values, OKLab mixing, and where they come from
//! (`TASKS_PALETTE`, or a query to the terminal). Design:
//! docs/specs/2026-09-25-date-recency-color-design.md and
//! docs/specs/2026-09-25-priority-color-design.md.

use crate::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};

/// One 8-bit sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Exactly `#rrggbb`, either case; anything else is `None`.
    pub fn parse_hex(hex: &str) -> Option<Rgb> {
        let digits = hex.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
        Some(Rgb {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
        })
    }

    /// The color `t` of the way from `self` to `other`, interpolated in OKLab so equal
    /// steps of `t` look like equal steps of change. The ends are returned exactly, not
    /// round-tripped through floating point.
    pub fn mix(self, other: Rgb, t: f64) -> Rgb {
        if t <= 0.0 {
            return self;
        }
        if t >= 1.0 {
            return other;
        }
        let from = to_oklab(self);
        let to = to_oklab(other);
        from_oklab([
            from[0] + (to[0] - from[0]) * t,
            from[1] + (to[1] - from[1]) * t,
            from[2] + (to[2] - from[2]) * t,
        ])
    }

    /// OKLab lightness, for tests that check a scale's order.
    #[cfg(test)]
    pub fn lightness(self) -> f64 {
        to_oklab(self)[0]
    }
}

// OKLab, after Björn Ottosson, "A perceptual color space for image processing" (2020).

fn to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f64) -> u8 {
    let c = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn to_oklab(color: Rgb) -> [f64; 3] {
    let (r, g, b) = (to_linear(color.r), to_linear(color.g), to_linear(color.b));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

fn from_oklab([l, a, b]: [f64; 3]) -> Rgb {
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    Rgb {
        r: from_linear(4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_),
        g: from_linear(-1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_),
        b: from_linear(-0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_),
    }
}

/// How far the date scale's old end sits from the foreground toward the background.
const OLD_TOWARD_BACKGROUND: f64 = 0.45;

/// The theme colors the date and priority scales need. `magenta` is `None` only from a
/// `TASKS_PALETTE` that leaves it out; the terminal query always supplies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cyan: Rgb,
    pub magenta: Option<Rgb>,
}

impl Palette {
    /// Parses `TASKS_PALETTE`: `fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]`,
    /// any order.
    pub fn parse(value: &str) -> crate::error::Result<Palette> {
        let bad = |detail: String| {
            Error::Config(format!(
                "TASKS_PALETTE must be \"fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]\": {detail}"
            ))
        };
        let (mut fg, mut bg, mut cyan, mut magenta) = (None, None, None, None);
        for entry in value.split_whitespace() {
            let (key, hex) = entry
                .split_once('=')
                .ok_or_else(|| bad(format!("{entry:?} is not key=#rrggbb")))?;
            let slot = match key {
                "fg" => &mut fg,
                "bg" => &mut bg,
                "cyan" => &mut cyan,
                "magenta" => &mut magenta,
                other => return Err(bad(format!("unknown key {other:?}"))),
            };
            if slot.is_some() {
                return Err(bad(format!("{key} is given twice")));
            }
            *slot = Some(
                Rgb::parse_hex(hex).ok_or_else(|| bad(format!("{key}: {hex:?} is not #rrggbb")))?,
            );
        }
        match (fg, bg, cyan) {
            (Some(fg), Some(bg), Some(cyan)) => Ok(Palette {
                fg,
                bg,
                cyan,
                magenta,
            }),
            _ => {
                // Only the required keys can be missing.
                let missing: Vec<&str> = [("fg", fg), ("bg", bg), ("cyan", cyan)]
                    .into_iter()
                    .filter(|(_, color)| color.is_none())
                    .map(|(key, _)| key)
                    .collect();
                Err(bad(format!("missing {}", missing.join(", "))))
            }
        }
    }

    /// The faded end the date scale runs toward: the foreground dimmed toward the
    /// background.
    pub fn old(&self) -> Rgb {
        self.fg.mix(self.bg, OLD_TOWARD_BACKGROUND)
    }
}

/// How long the whole exchange may take before the terminal counts as silent.
pub const QUERY_TIMEOUT: Duration = Duration::from_millis(300);

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

/// Foreground, background and palette slots 5 and 6, each ST-terminated, then primary
/// device attributes (DA1) as a fence: terminals answer in order, so the fence arriving
/// before a color reply means the terminal does not answer color queries.
const QUERY: &[u8] = b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]4;5;?\x1b\\\x1b]4;6;?\x1b\\\x1b[c";

/// Why the terminal's colors are unavailable. `Display` is the reason the warning names.
#[derive(Debug)]
pub enum QueryError {
    /// stdout is redirected, so the terminal is not asked (spec §3.2).
    NotAsked,
    NoTerminal,
    /// Keys typed ahead were waiting; querying would consume them (see `query`).
    TypeAhead,
    TimedOut(Duration),
    Unsupported,
    Unparsable(String),
    Io(io::Error),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryError::NotAsked => f.write_str("stdout is not a terminal"),
            QueryError::NoTerminal => f.write_str("no terminal to ask"),
            QueryError::TypeAhead => f.write_str("keys were waiting in the terminal's input"),
            QueryError::TimedOut(after) => write!(f, "timed out after {} ms", after.as_millis()),
            QueryError::Unsupported => f.write_str("answered without the colors"),
            QueryError::Unparsable(reply) => write!(f, "unparsable reply {reply:?}"),
            QueryError::Io(error) => write!(f, "terminal error: {error}"),
        }
    }
}

/// Runs the query over `tty` (a raw-mode terminal in production, a fake in tests).
///
/// Every reply is read through the fence before any is parsed: a malformed reply must not
/// leave the replies after it unread, or they reach the shell as input once raw mode ends.
/// A timeout and an I/O error are the exits that can leave bytes behind: the deadline
/// bounds the timeout, and an I/O error ends the exchange with the rest of the reply
/// unread.
pub fn exchange<T: Read + Write>(tty: &mut T, timeout: Duration) -> Result<Palette, QueryError> {
    tty.write_all(QUERY).map_err(QueryError::Io)?;
    tty.flush().map_err(QueryError::Io)?;
    let bytes = read_through_fence(tty).map_err(|error| match error.kind() {
        io::ErrorKind::TimedOut => QueryError::TimedOut(timeout),
        _ => QueryError::Io(error),
    })?;
    parse_replies(&bytes)
}

/// One byte per read, so nothing past the fence's final `c` is consumed: whatever the
/// user types after it stays theirs.
fn read_byte(reader: &mut impl Read) -> io::Result<u8> {
    let mut byte = [0u8];
    reader.read_exact(&mut byte)?;
    Ok(byte[0])
}

/// Where a scan for the fence's reply stands. DA1's reply is `ESC [ ?`, then digits and
/// `;`, then `c`; any other byte resets the scan and an `ESC` restarts it. So a key typed
/// while the query runs (an arrow is `ESC [ A`) cannot pass for the fence, and neither can
/// the `c` in a color reply's hex.
#[derive(Clone, Copy)]
enum Fence {
    Idle,
    Esc,
    Csi,
    Params,
}

/// Reads through the fence's reply and returns what came before it: the color replies,
/// plus any keys typed meanwhile.
fn read_through_fence(reader: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut fence = Fence::Idle;
    let mut fence_start = 0;
    loop {
        let byte = read_byte(reader)?;
        bytes.push(byte);
        fence = match (fence, byte) {
            (_, ESC) => {
                fence_start = bytes.len() - 1;
                Fence::Esc
            }
            (Fence::Esc, b'[') => Fence::Csi,
            (Fence::Csi, b'?') => Fence::Params,
            (Fence::Params, b'0'..=b'9' | b';') => Fence::Params,
            (Fence::Params, b'c') => {
                bytes.truncate(fence_start);
                return Ok(bytes);
            }
            _ => Fence::Idle,
        };
    }
}

/// The four color replies among the bytes before the fence. Fewer than four means
/// the terminal skipped a query it does not support.
fn parse_replies(bytes: &[u8]) -> Result<Palette, QueryError> {
    let bodies = osc_bodies(bytes)?;
    match bodies.as_slice() {
        [fg, bg, magenta, cyan] => Ok(Palette {
            fg: reply_color(fg, b"10;")?,
            bg: reply_color(bg, b"11;")?,
            cyan: reply_color(cyan, b"4;6;")?,
            magenta: Some(reply_color(magenta, b"4;5;")?),
        }),
        fewer if fewer.len() < 4 => Err(QueryError::Unsupported),
        _ => Err(QueryError::Unparsable(format!(
            "{} replies to four queries",
            bodies.len()
        ))),
    }
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The bodies of the `ESC ] <body> (BEL | ESC \)` replies in `bytes`, in order. Bytes
/// outside a reply are keys typed while the query ran: they are skipped, not parsed.
fn osc_bodies(bytes: &[u8]) -> Result<Vec<&[u8]>, QueryError> {
    let mut bodies = Vec::new();
    let mut rest = bytes;
    while let Some(at) = rest.windows(2).position(|pair| pair == [ESC, b']']) {
        let body = &rest[at + 2..];
        let end = body
            .iter()
            .position(|&byte| byte == BEL || byte == ESC)
            .ok_or_else(|| QueryError::Unparsable(lossy(body)))?;
        let terminator = match (body[end], body.get(end + 1)) {
            (BEL, _) => 1,
            (ESC, Some(b'\\')) => 2,
            // An unterminated reply runs into the next escape.
            _ => return Err(QueryError::Unparsable(lossy(&body[..end]))),
        };
        bodies.push(&body[..end]);
        rest = &body[end + terminator..];
    }
    Ok(bodies)
}

fn reply_color(body: &[u8], prefix: &[u8]) -> Result<Rgb, QueryError> {
    let unparsable = || QueryError::Unparsable(lossy(body));
    let color = body.strip_prefix(prefix).ok_or_else(unparsable)?;
    let color = xterm_color::Color::parse(color).map_err(|_| unparsable())?;
    Ok(Rgb {
        r: (color.red >> 8) as u8,
        g: (color.green >> 8) as u8,
        b: (color.blue >> 8) as u8,
    })
}

impl Palette {
    /// Asks the controlling terminal (never stdout) for its colors, in raw mode, bounded
    /// by `timeout`. The raw-mode guard restores the terminal when it drops, on every path.
    pub fn query(timeout: Duration) -> Result<Palette, QueryError> {
        use std::os::fd::AsRawFd as _;
        let mut terminal = terminal_trx::terminal().map_err(|error| match error.kind() {
            // No controlling terminal: /dev/tty does not exist, or it reports no session
            // for this process (ENXIO). Anything else is a real error worth reporting.
            io::ErrorKind::NotFound => QueryError::NoTerminal,
            _ if error.raw_os_error() == Some(libc::ENXIO) => QueryError::NoTerminal,
            _ => QueryError::Io(error),
        })?;
        let mut lock = terminal.lock();
        let raw = lock.enable_raw_mode().map_err(QueryError::Io)?;
        // Raw mode makes type-ahead readable, and the reply reader would take it for
        // stray bytes and drop it: a user typing the next command while this one ran
        // would lose those keys. Nothing can push them back, so when any are waiting
        // the query is not sent, and they stay queued for the shell when raw mode ends.
        if pending_input(raw.as_raw_fd()).map_err(QueryError::Io)? > 0 {
            return Err(QueryError::TypeAhead);
        }
        // Ctrl-C must not end the exchange: raw mode leaves ISIG set, so a SIGINT would
        // kill the process before any guard drops and leave echo and canonical mode off.
        // Clearing ISIG turns the byte into ordinary input the reply reader skips.
        // Declared after `raw` so it drops first and `raw`'s saved termios, ISIG
        // included, has the final word.
        let _isig = IsigOff::clear(raw.as_raw_fd()).map_err(QueryError::Io)?;
        let mut timed = Timed {
            inner: raw,
            deadline: Instant::now() + timeout,
        };
        exchange(&mut timed, timeout)
    }
}

/// ISIG cleared on `fd`, restored on drop. See `Palette::query` for why. `saved` is `None`
/// when ISIG was already off, so a drop then leaves the terminal as it was found.
struct IsigOff {
    fd: std::os::fd::RawFd,
    saved: Option<libc::termios>,
}

impl IsigOff {
    fn clear(fd: std::os::fd::RawFd) -> io::Result<IsigOff> {
        // SAFETY: `fd` is a valid descriptor for the duration of the calls.
        let mut current: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: `current` is one valid termios pointer for the call.
        if unsafe { libc::tcgetattr(fd, &mut current) } < 0 {
            return Err(io::Error::last_os_error());
        }
        if current.c_lflag & libc::ISIG == 0 {
            return Ok(IsigOff { fd, saved: None });
        }
        let saved = current;
        current.c_lflag &= !libc::ISIG;
        // SAFETY: `current` is one valid termios pointer for the call.
        if unsafe { libc::tcsetattr(fd, libc::TCSADRAIN, &current) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(IsigOff {
            fd,
            saved: Some(saved),
        })
    }
}

impl Drop for IsigOff {
    fn drop(&mut self) {
        let Some(saved) = &self.saved else {
            return;
        };
        // Best effort: the exchange is over and the raw-mode guard restores the saved
        // termios right after, so a failure here has nowhere to report and nothing
        // left to protect.
        // SAFETY: `saved` is one valid termios pointer for the call.
        unsafe { libc::tcsetattr(self.fd, libc::TCSADRAIN, saved) };
    }
}

/// How many bytes are waiting to be read on `fd`.
fn pending_input(fd: std::os::fd::RawFd) -> io::Result<usize> {
    let mut waiting: libc::c_int = 0;
    // SAFETY: FIONREAD writes one c_int through the pointer, which is valid for the call.
    if unsafe { libc::ioctl(fd, libc::FIONREAD, &mut waiting) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(waiting as usize)
}

/// A terminal whose reads fail with `TimedOut` once the deadline passes, so a silent
/// terminal cannot hang the run.
struct Timed<T> {
    inner: T,
    deadline: Instant,
}

impl<T: Read + std::os::fd::AsRawFd> Read for Timed<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        // `poll` with a zero timeout still reports ready input, so a terminal streaming
        // garbage would outlast the deadline unless it is checked here.
        if left.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "terminal did not answer",
            ));
        }
        let millis = left.as_millis().min(i32::MAX as u128) as libc::c_int;
        let mut fd = libc::pollfd {
            fd: self.inner.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `fd` is one valid pollfd for the duration of the call, and the count
        // passed is 1.
        let ready = unsafe { libc::poll(&mut fd, 1, millis) };
        match ready {
            0 => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "terminal did not answer",
            )),
            n if n < 0 => Err(io::Error::last_os_error()),
            _ => self.inner.read(buf),
        }
    }
}

impl<T: Write> Write for Timed<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn hex(value: &str) -> Rgb {
        Rgb::parse_hex(value).unwrap()
    }

    #[test]
    fn hex_parses_exactly_rrggbb() {
        assert_eq!(
            hex("#e5e3d7"),
            Rgb {
                r: 0xe5,
                g: 0xe3,
                b: 0xd7
            }
        );
        assert_eq!(
            hex("#00D7FF"),
            Rgb {
                r: 0,
                g: 0xd7,
                b: 0xff
            }
        );
        for bad in [
            "e5e3d7", "#e5e3d", "#e5e3d7f", "#g5e3d7", "#", "", "#+5e3d7",
        ] {
            assert_eq!(Rgb::parse_hex(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn mix_returns_the_exact_ends_and_darkens_toward_the_background() {
        let fg = hex("#e5e3d7");
        let bg = hex("#13140d");
        assert_eq!(fg.mix(bg, 0.0), fg);
        assert_eq!(fg.mix(bg, 1.0), bg);
        assert_eq!(fg.mix(bg, -0.5), fg);
        assert_eq!(fg.mix(bg, 1.5), bg);
        // The spec's old end for this theme, from a reference OKLab implementation.
        assert_eq!(
            fg.mix(bg, 0.45),
            Rgb {
                r: 125,
                g: 125,
                b: 115
            }
        );
    }

    #[test]
    fn palette_parses_three_or_four_keys_in_any_order() {
        let palette = Palette::parse("cyan=#00d7ff fg=#e5e3d7  bg=#13140d").unwrap();
        assert_eq!(
            palette,
            Palette {
                fg: hex("#e5e3d7"),
                bg: hex("#13140d"),
                cyan: hex("#00d7ff"),
                magenta: None,
            }
        );
        let palette = Palette::parse("magenta=#d75fd7 cyan=#00d7ff fg=#e5e3d7 bg=#13140d").unwrap();
        assert_eq!(palette.magenta, Some(hex("#d75fd7")));
    }

    #[test]
    fn the_old_end_is_the_foreground_dimmed_toward_the_background() {
        let palette = Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        assert_eq!(
            palette.old(),
            Rgb {
                r: 125,
                g: 125,
                b: 115
            }
        );
    }

    #[test]
    fn palette_rejects_missing_unknown_duplicate_and_malformed_entries() {
        for (value, needle) in [
            ("fg=#e5e3d7 bg=#13140d", "missing cyan"),
            ("", "missing fg, bg, cyan"),
            (
                "fg=#e5e3d7 bg=#13140d cyan=#00d7ff red=#ff0000",
                "unknown key \"red\"",
            ),
            (
                "fg=#e5e3d7 fg=#e5e3d7 bg=#13140d cyan=#00d7ff",
                "fg is given twice",
            ),
            (
                "fg=e5e3d7 bg=#13140d cyan=#00d7ff",
                "fg: \"e5e3d7\" is not #rrggbb",
            ),
            ("fg bg=#13140d cyan=#00d7ff", "\"fg\" is not key=#rrggbb"),
            ("fg=#e5e3d7 bg=#13140d magenta=#d75fd7", "missing cyan"),
            (
                "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7 magenta=#d75fd7",
                "magenta is given twice",
            ),
            (
                "fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=d75fd7",
                "magenta: \"d75fd7\" is not #rrggbb",
            ),
        ] {
            let error = Palette::parse(value).unwrap_err();
            assert_eq!(error.kind(), "config", "{value:?}");
            let text = error.to_string();
            assert!(text.contains("TASKS_PALETTE"), "{value:?}: {text}");
            assert!(text.contains(needle), "{value:?}: {text}");
        }
        let text = Palette::parse("").unwrap_err().to_string();
        assert!(
            text.contains("\"fg=#rrggbb bg=#rrggbb cyan=#rrggbb [magenta=#rrggbb]\""),
            "{text}"
        );
    }

    use std::io::{self, Read, Write};
    use std::time::Duration;

    /// A terminal that answers with `replies`, `chunk` bytes per read, then times out.
    struct Fake {
        replies: Vec<u8>,
        at: usize,
        chunk: usize,
        sent: Vec<u8>,
    }

    impl Fake {
        fn new(replies: &[u8], chunk: usize) -> Fake {
            Fake {
                replies: replies.to_vec(),
                at: 0,
                chunk,
                sent: Vec::new(),
            }
        }
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.at == self.replies.len() {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "no reply"));
            }
            let n = self.chunk.min(buf.len()).min(self.replies.len() - self.at);
            buf[..n].copy_from_slice(&self.replies[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.sent.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    const DA1: &[u8] = b"\x1b[?62;22c";
    const T: Duration = Duration::from_millis(300);

    /// Replies in query order, with slot 5 fixed at `#d75fd7`.
    fn replies(fg: &str, bg: &str, cyan: &str, end: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (prefix, color) in [
            ("10;", fg),
            ("11;", bg),
            ("4;5;", "rgb:d7d7/5f5f/d7d7"),
            ("4;6;", cyan),
        ] {
            bytes.extend_from_slice(format!("\x1b]{prefix}{color}{end}").as_bytes());
        }
        bytes.extend_from_slice(DA1);
        bytes
    }

    fn expected() -> Palette {
        Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff magenta=#d75fd7").unwrap()
    }

    #[test]
    fn exchange_sends_the_four_queries_and_the_fence() {
        let mut tty = Fake::new(
            &replies(
                "rgb:e5e5/e3e3/d7d7",
                "rgb:1313/1414/0d0d",
                "rgb:0000/d7d7/ffff",
                "\x1b\\",
            ),
            64,
        );
        assert_eq!(exchange(&mut tty, T).unwrap(), expected());
        assert_eq!(
            tty.sent,
            b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]4;5;?\x1b\\\x1b]4;6;?\x1b\\\x1b[c".to_vec()
        );
        assert_eq!(tty.at, tty.replies.len(), "the fence reply is consumed too");
    }

    #[test]
    fn exchange_accepts_bel_two_digit_components_and_one_byte_reads() {
        let bytes = replies("rgb:e5/e3/d7", "rgb:13/14/0d", "rgb:00/d7/ff", "\x07");
        assert_eq!(exchange(&mut Fake::new(&bytes, 1), T).unwrap(), expected());
    }

    #[test]
    fn a_fence_before_the_colors_means_unsupported() {
        let err = exchange(&mut Fake::new(DA1, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unsupported), "{err:?}");
        let mut partial = b"\x1b]10;rgb:e5/e3/d7\x07\x1b]11;rgb:13/14/0d\x07".to_vec();
        partial.extend_from_slice(DA1);
        let err = exchange(&mut Fake::new(&partial, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unsupported), "{err:?}");
        assert_eq!(err.to_string(), "answered without the colors");
    }

    #[test]
    fn three_replies_are_no_longer_enough() {
        // A terminal that answers fg, bg and slot 6 but skips slot 5.
        let mut bytes = Vec::new();
        for (prefix, color) in [
            ("10;", "rgb:e5/e3/d7"),
            ("11;", "rgb:13/14/0d"),
            ("4;6;", "rgb:00/d7/ff"),
        ] {
            bytes.extend_from_slice(format!("\x1b]{prefix}{color}\x07").as_bytes());
        }
        bytes.extend_from_slice(DA1);
        let err = exchange(&mut Fake::new(&bytes, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unsupported), "{err:?}");
    }

    #[test]
    fn a_garbled_reply_is_unparsable_and_silence_is_a_timeout() {
        let bytes = replies("rgb:zz/e3/d7", "rgb:13/14/0d", "rgb:00/d7/ff", "\x07");
        let err = exchange(&mut Fake::new(&bytes, 64), T).unwrap_err();
        assert!(matches!(err, QueryError::Unparsable(_)), "{err:?}");
        assert!(err.to_string().starts_with("unparsable reply"), "{err}");

        let err = exchange(&mut Fake::new(b"", 64), T).unwrap_err();
        assert!(matches!(err, QueryError::TimedOut(_)), "{err:?}");
        assert_eq!(err.to_string(), "timed out after 300 ms");
    }

    #[test]
    fn a_malformed_reply_still_reads_through_the_fence() {
        // One byte per read: the fence must be consumed even though the first reply
        // fails to parse, or its bytes reach the shell after raw mode ends. The `c` in
        // the cyan reply's hex must not be taken for the fence's end.
        let bytes = replies("rgb:zz/e3/d7", "rgb:13/14/0d", "rgb:cc/cc/cc", "\x07");
        let mut tty = Fake::new(&bytes, 1);
        let err = exchange(&mut tty, T).unwrap_err();
        assert!(matches!(err, QueryError::Unparsable(_)), "{err:?}");
        assert_eq!(
            tty.at,
            tty.replies.len(),
            "every reply and the fence were read"
        );

        let mut unterminated = b"\x1b]10;rgb:e5/e3/d7".to_vec();
        unterminated.extend_from_slice(DA1);
        let mut tty = Fake::new(&unterminated, 1);
        let err = exchange(&mut tty, T).unwrap_err();
        assert!(matches!(err, QueryError::Unparsable(_)), "{err:?}");
        assert_eq!(tty.at, tty.replies.len(), "the fence was read");
    }

    #[test]
    fn typed_keys_neither_end_the_read_nor_outlast_the_fence() {
        // An Up arrow typed as the query goes out, then valid replies whose cyan hex
        // holds `c`, then the fence, then more typing that belongs to the shell.
        let mut bytes = b"\x1b[A".to_vec();
        bytes.extend(replies(
            "rgb:e5/e3/d7",
            "rgb:13/14/0d",
            "rgb:cc/cc/cc",
            "\x1b\\",
        ));
        let through_fence = bytes.len();
        bytes.extend_from_slice(b"ls\r");
        let mut tty = Fake::new(&bytes, 1);
        let palette = exchange(&mut tty, T).unwrap();
        assert_eq!(
            palette.cyan,
            Rgb {
                r: 0xcc,
                g: 0xcc,
                b: 0xcc
            }
        );
        assert_eq!(
            tty.at, through_fence,
            "reading stops exactly after the real fence"
        );
    }

    #[test]
    fn an_expired_deadline_stops_reading_even_with_input_waiting() {
        use std::os::unix::net::UnixStream;
        use std::time::Instant;
        let (mut terminal, ours) = UnixStream::pair().unwrap();
        terminal.write_all(b"\x1b]10;").unwrap();
        let mut timed = Timed {
            inner: ours,
            deadline: Instant::now(),
        };
        let mut buf = [0u8; 8];
        let err = timed.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        let mut open = Timed {
            inner: timed.inner,
            deadline: Instant::now() + Duration::from_secs(1),
        };
        assert_eq!(
            open.read(&mut buf).unwrap(),
            5,
            "the input was there all along"
        );
    }

    #[test]
    fn pending_input_counts_bytes_waiting_to_be_read() {
        use std::os::fd::AsRawFd;
        use std::os::unix::net::UnixStream;
        let (mut terminal, ours) = UnixStream::pair().unwrap();
        assert_eq!(pending_input(ours.as_raw_fd()).unwrap(), 0);
        terminal.write_all(b"ls\r").unwrap();
        assert_eq!(pending_input(ours.as_raw_fd()).unwrap(), 3);
    }

    fn lflag(fd: libc::c_int) -> libc::tcflag_t {
        // SAFETY: `termios` is zeroed first and one valid pointer for the call.
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: `termios` is one valid pointer for the call.
        assert_eq!(unsafe { libc::tcgetattr(fd, &mut termios) }, 0);
        termios.c_lflag
    }

    #[test]
    fn isig_off_clears_isig_and_restores_it_on_drop() {
        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;
        // SAFETY: four valid pointers for the call; the attributes are left default.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        assert_ne!(lflag(slave) & libc::ISIG, 0, "a pty starts with ISIG on");
        let guard = IsigOff::clear(slave).unwrap();
        assert_eq!(lflag(slave) & libc::ISIG, 0, "ISIG off for the exchange");
        drop(guard);
        assert_ne!(lflag(slave) & libc::ISIG, 0, "ISIG back on after the drop");
        // SAFETY: both descriptors are ours to close.
        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }

    #[test]
    fn isig_off_leaves_an_already_clear_isig_alone() {
        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;
        // SAFETY: four valid pointers for the call; the attributes are left default.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: `termios` is one valid pointer for the call.
        assert_eq!(unsafe { libc::tcgetattr(slave, &mut termios) }, 0);
        termios.c_lflag &= !libc::ISIG;
        // SAFETY: `termios` is one valid pointer for the call.
        assert_eq!(
            unsafe { libc::tcsetattr(slave, libc::TCSADRAIN, &termios) },
            0
        );
        let guard = IsigOff::clear(slave).unwrap();
        assert!(guard.saved.is_none(), "nothing to save when ISIG is off");
        drop(guard);
        assert_eq!(lflag(slave) & libc::ISIG, 0, "the drop is a no-op");
        // SAFETY: both descriptors are ours to close.
        unsafe {
            libc::close(master);
            libc::close(slave);
        }
    }

    #[test]
    fn the_reasons_read_as_the_spec_words_them() {
        assert_eq!(QueryError::NotAsked.to_string(), "stdout is not a terminal");
        assert_eq!(QueryError::NoTerminal.to_string(), "no terminal to ask");
        assert_eq!(
            QueryError::TypeAhead.to_string(),
            "keys were waiting in the terminal's input"
        );
    }
}
