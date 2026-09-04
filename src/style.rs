use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::Format;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn parse(value: &str) -> Result<ColorMode> {
        match value {
            "auto" => Ok(ColorMode::Auto),
            "always" => Ok(ColorMode::Always),
            "never" => Ok(ColorMode::Never),
            other => Err(Error::Validation(format!(
                "color mode must be auto, always, or never, got {other:?}"
            ))),
        }
    }

    /// Precedence: explicit flag, then a non-empty `NO_COLOR`, then `TASKS_COLOR`, then
    /// off. `configured` is parsed whenever it is present, even when something outranks
    /// it, so a typo is reported instead of silently ignored.
    pub fn resolve(
        flag: Option<&str>,
        configured: Option<&str>,
        no_color: bool,
    ) -> Result<ColorMode> {
        let flag = flag.map(ColorMode::parse).transpose()?;
        let configured = configured
            .map(|value| {
                ColorMode::parse(value).map_err(|_| {
                    Error::Config(format!(
                        "TASKS_COLOR must be auto, always, or never, got {value:?}"
                    ))
                })
            })
            .transpose()?;
        Ok(match (flag, no_color, configured) {
            (Some(mode), _, _) => mode,
            (None, true, _) => ColorMode::Never,
            (None, false, Some(mode)) => mode,
            (None, false, None) => ColorMode::Never,
        })
    }
}

/// A role, never a color. Call sites name what a span means; this module decides how that
/// looks, so the same meaning renders identically in every view.
#[derive(Debug, Clone, Copy)]
pub enum Style {
    // `expect`, not `allow`: these three are unused until tables are painted, and the
    // expectation fails once they are, so the attribute cannot outlive its reason.
    #[cfg_attr(not(test), expect(dead_code))]
    Status(Status),
    #[cfg_attr(not(test), expect(dead_code))]
    Chrome,
    #[cfg_attr(not(test), expect(dead_code))]
    Emphasis,
    Error,
    Ok,
    Warning,
}

/// Paints for one output stream. `main` builds one per stream, because stdout and stderr
/// are redirected independently and only `Auto` can differ between them.
pub struct Painter {
    enabled: bool,
}

impl Painter {
    pub fn new(mode: ColorMode, format: Format, stream_is_terminal: bool) -> Painter {
        Painter {
            enabled: format == Format::Pretty
                && match mode {
                    ColorMode::Auto => stream_is_terminal,
                    ColorMode::Always => true,
                    ColorMode::Never => false,
                },
        }
    }

    /// Wraps `text` without changing its visible width, so callers pad first and paint
    /// last. `text` must not contain a newline: the reset has to land before any break.
    pub fn paint(&self, style: Style, text: &str) -> String {
        debug_assert!(
            !text.contains('\n'),
            "paint spans a newline; the reset would land after the break: {text:?}"
        );
        let code = match style {
            Style::Status(Status::Idea) => "34",
            Style::Status(Status::Todo) => return text.into(),
            Style::Status(Status::Doing) => "33",
            Style::Status(Status::Blocked) => "31",
            Style::Status(Status::Done) => "2;32",
            Style::Status(Status::Dropped) => "2;31",
            Style::Chrome => "2",
            Style::Emphasis => "1",
            Style::Error => "31",
            Style::Ok => "32",
            Style::Warning => "33",
        };
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_precedence_and_validates_config_even_when_overridden() {
        assert_eq!(
            ColorMode::resolve(None, None, false).unwrap(),
            ColorMode::Never
        );
        assert_eq!(
            ColorMode::resolve(None, Some("auto"), false).unwrap(),
            ColorMode::Auto
        );
        assert_eq!(
            ColorMode::resolve(None, Some("always"), true).unwrap(),
            ColorMode::Never
        );
        assert_eq!(
            ColorMode::resolve(Some("always"), Some("never"), true).unwrap(),
            ColorMode::Always
        );
        assert_eq!(
            ColorMode::resolve(Some("chartreuse"), None, false)
                .unwrap_err()
                .kind(),
            "validation"
        );
        assert_eq!(
            ColorMode::resolve(Some("never"), Some("chartreuse"), false)
                .unwrap_err()
                .kind(),
            "config"
        );
    }

    #[test]
    fn painter_obeys_format_mode_stream_and_roles() {
        let plain = Painter::new(ColorMode::Always, Format::Json, true);
        assert_eq!(plain.paint(Style::Error, "error"), "error");

        let redirected = Painter::new(ColorMode::Auto, Format::Pretty, false);
        assert_eq!(redirected.paint(Style::Warning, "warning:"), "warning:");

        let terminal = Painter::new(ColorMode::Auto, Format::Pretty, true);
        assert_eq!(
            terminal.paint(Style::Warning, "warning:"),
            "\x1b[33mwarning:\x1b[0m"
        );

        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        for (style, code) in [
            (Style::Status(Status::Idea), "34"),
            (Style::Status(Status::Doing), "33"),
            (Style::Status(Status::Blocked), "31"),
            (Style::Status(Status::Done), "2;32"),
            (Style::Status(Status::Dropped), "2;31"),
            (Style::Chrome, "2"),
            (Style::Emphasis, "1"),
            (Style::Error, "31"),
            (Style::Ok, "32"),
            (Style::Warning, "33"),
        ] {
            let painted = colored.paint(style, "x");
            assert_eq!(painted, format!("\x1b[{code}mx\x1b[0m"));
            assert!(painted.ends_with("\x1b[0m"));
        }
        assert_eq!(colored.paint(Style::Status(Status::Todo), "todo"), "todo");
    }
}
