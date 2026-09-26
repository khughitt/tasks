use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::Format;
use crate::palette::{Palette, Rgb};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Status(Status),
    Chrome,
    Emphasis,
    Error,
    Ok,
    Warning,
    /// A date, painted by its distance from today (docs/specs/2026-09-25-date-recency-color-design.md).
    Date(When),
    /// A priority, painted on the magenta scale when the painter has one
    /// (docs/specs/2026-09-25-priority-color-design.md), else bold at P0 and P1.
    Priority(u8),
}

/// Which day a date cell shows. `Today` is the due column's `now`, which names no date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Today,
    On(time::Date),
}

/// Days beyond which every date looks the same.
const HORIZON_DAYS: f64 = 730.0;

/// How far along the scale a date `age_days` from today sits: 0 is today, 1 is the
/// horizon or beyond. Logarithmic, so the first weeks spread across half the range.
pub fn fade(age_days: u64) -> f64 {
    ((1.0 + age_days as f64).ln() / (1.0 + HORIZON_DAYS).ln()).min(1.0)
}

/// Today and the two ends of the date scale, fixed for one run.
#[derive(Debug, Clone, Copy)]
pub struct Recency {
    today: time::Date,
    recent: Rgb,
    old: Rgb,
}

impl Recency {
    pub fn new(today: time::Date, palette: &Palette) -> Recency {
        Recency {
            today,
            recent: palette.cyan,
            old: palette.old(),
        }
    }

    /// The color for `when`, by its absolute distance from today.
    pub fn color(&self, when: When) -> Rgb {
        let day = match when {
            When::Today => self.today,
            When::On(day) => day,
        };
        let age = (self.today - day).whole_days().unsigned_abs();
        self.recent.mix(self.old, fade(age))
    }
}

/// How far each priority, P0 to P4, sits from magenta toward the scale's end, in even
/// steps. P0 and P1 share a color; bold sets P0 apart.
const PRIORITY_STEPS: [f64; 5] = [0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0];

/// How far the priority scale's end sits from the foreground toward the background.
/// Dimmer than the date scale's old end: magenta and that end are close in lightness,
/// which left P1 to P3 hard to tell apart.
const PRIORITY_TOWARD_BACKGROUND: f64 = 0.75;

/// The five priority colors, fixed for one run.
#[derive(Debug, Clone, Copy)]
pub struct PriorityScale {
    steps: [Rgb; 5],
}

impl PriorityScale {
    pub fn new(magenta: Rgb, palette: &Palette) -> PriorityScale {
        let end = palette.fg.mix(palette.bg, PRIORITY_TOWARD_BACKGROUND);
        PriorityScale {
            steps: PRIORITY_STEPS.map(|t| magenta.mix(end, t)),
        }
    }

    /// `priority` is validated 0-4 wherever a record is read.
    pub fn color(&self, priority: u8) -> Rgb {
        self.steps[usize::from(priority)]
    }
}

/// Paints for one output stream. `main` builds one per stream, because stdout and stderr
/// are redirected independently and only `Auto` can differ between them.
pub struct Painter {
    enabled: bool,
    recency: Option<Recency>,
    priority: Option<PriorityScale>,
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
            recency: None,
            priority: None,
        }
    }

    /// Dates paint only with a recency; without one `Style::Date` is the identity.
    pub fn with_recency(self, recency: Recency) -> Painter {
        Painter {
            recency: Some(recency),
            ..self
        }
    }

    /// Priorities paint on the scale only with one; without it P0 and P1 are bold.
    pub fn with_priority_scale(self, scale: PriorityScale) -> Painter {
        Painter {
            priority: Some(scale),
            ..self
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Wraps `text` without changing its visible width, so callers pad first and paint
    /// last. `text` must not contain a newline: the reset has to land before any break.
    pub fn paint(&self, style: Style, text: &str) -> String {
        debug_assert!(
            !text.contains('\n'),
            "paint spans a newline; the reset would land after the break: {text:?}"
        );
        let code: String = match style {
            Style::Date(when) => match &self.recency {
                Some(recency) => {
                    let color = recency.color(when);
                    format!("38;2;{};{};{}", color.r, color.g, color.b)
                }
                None => return text.into(),
            },
            Style::Priority(priority) => match &self.priority {
                Some(scale) => {
                    let color = scale.color(priority);
                    let bold = if priority == 0 { "1;" } else { "" };
                    format!("{bold}38;2;{};{};{}", color.r, color.g, color.b)
                }
                None if priority <= 1 => "1".into(),
                None => return text.into(),
            },
            Style::Status(Status::Idea) => "34".into(),
            Style::Status(Status::Todo) => return text.into(),
            Style::Status(Status::Doing) => "33".into(),
            Style::Status(Status::Blocked) => "31".into(),
            Style::Status(Status::Shelved) => "2;34".into(),
            Style::Status(Status::Done) => "2;32".into(),
            Style::Status(Status::Dropped) => "2;31".into(),
            Style::Chrome => "2".into(),
            Style::Emphasis => "1".into(),
            Style::Error => "31".into(),
            Style::Ok => "32".into(),
            Style::Warning => "33".into(),
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
            (Style::Status(Status::Shelved), "2;34"),
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

    use crate::palette::{Palette, Rgb};
    use ::time::{Date, Month};

    fn day(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).unwrap()
    }

    fn palette() -> Palette {
        Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap()
    }

    #[test]
    fn fade_is_logarithmic_and_clipped_at_two_years() {
        for (age, expected) in [
            (0, 0.0),
            (1, 0.105),
            (7, 0.315),
            (28, 0.511),
            (91, 0.686),
            (365, 0.895),
            (730, 1.0),
            (5000, 1.0),
        ] {
            assert!(
                (fade(age) - expected).abs() < 0.001,
                "age {age}: {}",
                fade(age)
            );
        }
    }

    #[test]
    fn recency_runs_from_cyan_to_a_dimmed_foreground_by_absolute_distance() {
        let today = day(2026, Month::September, 25);
        let recency = Recency::new(today, &palette());
        let cyan = Rgb {
            r: 0,
            g: 0xd7,
            b: 0xff,
        };
        let old = Rgb {
            r: 125,
            g: 125,
            b: 115,
        };
        assert_eq!(recency.color(When::Today), cyan);
        assert_eq!(recency.color(When::On(today)), cyan);
        assert_eq!(recency.color(When::On(day(2023, Month::January, 1))), old);
        let past = recency.color(When::On(day(2026, Month::September, 15)));
        let future = recency.color(When::On(day(2026, Month::October, 5)));
        assert_eq!(past, future, "ten days either side is the same distance");
        assert_ne!(past, cyan);
        assert_ne!(past, old);
    }

    fn magenta() -> Rgb {
        Rgb {
            r: 0xd7,
            g: 0x5f,
            b: 0xd7,
        }
    }

    #[test]
    fn the_priority_scale_samples_the_mix_at_fixed_steps() {
        let palette = palette();
        let scale = PriorityScale::new(magenta(), &palette);
        // Its own end, dimmer than the date scale's, so the steps sit apart.
        let end = palette.fg.mix(palette.bg, 0.75);
        assert_eq!(scale.color(0), magenta());
        assert_eq!(scale.color(1), magenta());
        assert_eq!(scale.color(2), magenta().mix(end, 1.0 / 3.0));
        assert_eq!(scale.color(3), magenta().mix(end, 2.0 / 3.0));
        assert_eq!(scale.color(4), end);
        // The reference values the end-to-end tests pin.
        assert_eq!(
            scale.color(2),
            Rgb {
                r: 163,
                g: 88,
                b: 159
            }
        );
        assert_eq!(
            scale.color(3),
            Rgb {
                r: 112,
                g: 78,
                b: 106
            }
        );
    }

    #[test]
    fn with_this_fixture_the_steps_fall_in_lightness() {
        // A fixture property, not a guarantee: a theme whose magenta equals the old end
        // collapses the scale (spec §2.3).
        let scale = PriorityScale::new(magenta(), &palette());
        let lightness: Vec<f64> = (1..=4).map(|p| scale.color(p).lightness()).collect();
        assert!(
            lightness.windows(2).all(|pair| pair[0] > pair[1]),
            "{lightness:?}"
        );
    }

    #[test]
    fn the_priority_role_paints_the_scale_or_falls_back_to_bold() {
        let bare = Painter::new(ColorMode::Always, Format::Pretty, false);
        assert_eq!(bare.paint(Style::Priority(0), "P0"), "\x1b[1mP0\x1b[0m");
        assert_eq!(bare.paint(Style::Priority(1), "P1"), "\x1b[1mP1\x1b[0m");
        for p in 2..=4 {
            assert_eq!(bare.paint(Style::Priority(p), "Px"), "Px");
        }

        let scaled = Painter::new(ColorMode::Always, Format::Pretty, false)
            .with_priority_scale(PriorityScale::new(magenta(), &palette()));
        assert_eq!(
            scaled.paint(Style::Priority(0), "P0"),
            "\x1b[1;38;2;215;95;215mP0\x1b[0m"
        );
        assert_eq!(
            scaled.paint(Style::Priority(1), "P1"),
            "\x1b[38;2;215;95;215mP1\x1b[0m"
        );
        assert_eq!(
            scaled.paint(Style::Priority(4), "P4"),
            "\x1b[38;2;64;64;56mP4\x1b[0m"
        );

        let off = Painter::new(ColorMode::Never, Format::Pretty, true)
            .with_priority_scale(PriorityScale::new(magenta(), &palette()));
        assert_eq!(off.paint(Style::Priority(0), "P0"), "P0");
    }

    #[test]
    fn the_date_role_paints_truecolor_only_with_a_recency() {
        let today = day(2026, Month::September, 25);
        let bare = Painter::new(ColorMode::Always, Format::Pretty, false);
        assert_eq!(
            bare.paint(Style::Date(When::Today), "2026-09-25"),
            "2026-09-25"
        );
        let dated = bare.with_recency(Recency::new(today, &palette()));
        assert_eq!(
            dated.paint(Style::Date(When::Today), "2026-09-25"),
            "\x1b[38;2;0;215;255m2026-09-25\x1b[0m"
        );
        let off = Painter::new(ColorMode::Never, Format::Pretty, true)
            .with_recency(Recency::new(today, &palette()));
        assert_eq!(
            off.paint(Style::Date(When::Today), "2026-09-25"),
            "2026-09-25"
        );
        assert!(dated.enabled());
        assert!(!off.enabled());
    }
}
