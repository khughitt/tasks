use crate::error::{Error, Result};
use crate::model::Status;
use crate::output::Format;
use crate::palette::{Palette, Rgb};

/// Whether pretty headings use kitty text sizing: `auto` when the terminal is kitty,
/// `always`, or `never`. Set by `TASKS_TEXT_SIZING` or config `[output] text_sizing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSizing {
    Auto,
    Always,
    Never,
}

impl TextSizing {
    pub fn parse(value: &str) -> Result<TextSizing> {
        match value {
            "auto" => Ok(TextSizing::Auto),
            "always" => Ok(TextSizing::Always),
            "never" => Ok(TextSizing::Never),
            other => Err(Error::Validation(format!(
                "text sizing must be auto, always, or never, got {other:?}"
            ))),
        }
    }
}

/// Scaling is on for pretty output with colour not turned off, and then `always`, or
/// `auto` on a kitty terminal (`TERM=xterm-kitty`) attached to stdout outside tmux.
/// Never on for JSON.
pub fn text_sizing_on(
    mode: TextSizing,
    format: Format,
    color: ColorMode,
    stdout_is_terminal: bool,
    painter_enabled: bool,
    term: Option<&str>,
    tmux: bool,
) -> bool {
    format == Format::Pretty
        && color != ColorMode::Never
        && match mode {
            TextSizing::Always => true,
            TextSizing::Never => false,
            TextSizing::Auto => {
                stdout_is_terminal && painter_enabled && term == Some("xterm-kitty") && !tmux
            }
        }
}

/// The style table for headings: each role names a scale, painted bold in its hue
/// (a lane in its project colour, a top-level goal in its goal hue). Later roles
/// extend it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    LaneHeading,
    GoalHeading,
}

/// One OSC 66 scale: `s=2` with fraction `n/d`, each chunk `payload_cells` wide and
/// occupying `width * 2` cells.
#[derive(Debug, Clone, Copy)]
struct Scale {
    numerator: usize,
    denominator: usize,
    width: usize,
    payload_cells: usize,
}

impl Scale {
    /// Cells one chunk occupies (`s * w`).
    fn occupied(self) -> usize {
        2 * self.width
    }
}

impl Role {
    fn scale(self) -> Scale {
        match self {
            // 1.5x: 4 cells of text drawn in 6.
            Role::LaneHeading => Scale {
                numerator: 3,
                denominator: 4,
                width: 3,
                payload_cells: 4,
            },
            // 1.25x: 8 cells of text drawn in 10.
            Role::GoalHeading => Scale {
                numerator: 5,
                denominator: 8,
                width: 5,
                payload_cells: 8,
            },
        }
    }
}

/// `text` packed into chunks of exactly `cells` display cells: a wide glyph that would
/// straddle a boundary opens the next chunk, and every short chunk is space-padded.
fn chunks(text: &str, cells: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthChar;
    let mut out = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for c in text.chars() {
        let width = c.width().unwrap_or(0);
        if used + width > cells {
            current.push_str(&" ".repeat(cells - used));
            out.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push(c);
        used += width;
    }
    if !current.is_empty() {
        current.push_str(&" ".repeat(cells - used));
        out.push(current);
    }
    out
}

/// Cells `text` occupies once scaled for `role`; tests measure headings with it.
#[cfg(test)]
pub fn scaled_width(text: &str, role: Role) -> usize {
    let scale = role.scale();
    chunks(text, scale.payload_cells).len() * scale.occupied()
}

/// `title` clipped with `…` so that, scaled for `role`, it fits in `available` cells.
pub fn clip_for_heading(title: &str, role: Role, available: usize) -> String {
    let scale = role.scale();
    let cap = available / scale.occupied() * scale.payload_cells;
    crate::ellipsis::clip(title, cap, crate::ellipsis::Measure::DisplayCells)
}

/// The hue steps a top-level goal's colour turns through, in degrees, well separated.
const GOAL_HUE_STEPS: [f64; 8] = [0.0, 40.0, -40.0, 80.0, -80.0, 120.0, -120.0, 160.0];

/// Below this OKLCH chroma a project colour is too close to grey to turn.
const MIN_HUE_CHROMA: f64 = 0.04;

/// A project colour that can carry a hue, or `None` when it is too close to grey.
pub fn heading_color(project: Option<Rgb>) -> Option<Rgb> {
    project.filter(|color| color.chroma() >= MIN_HUE_CHROMA)
}

/// A top-level goal's own hue: the project colour turned by a step fixed by the goal's
/// id (the first 8 hex of its SHA-256, mod 8), so it keeps its colour across runs and
/// views. `None` for a near-grey project.
pub fn goal_hue(project: Rgb, goal_id: &str) -> Option<Rgb> {
    use sha2::Digest;
    heading_color(Some(project))?;
    let digest = sha2::Sha256::digest(goal_id.as_bytes());
    let first = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    Some(project.rotate_hue(GOAL_HUE_STEPS[first as usize % GOAL_HUE_STEPS.len()]))
}

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

/// Combined task and 1-hop neighborhood activity quantized to `0..=255` (`0` = cold, `255` = hot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activity(pub u8);

impl Activity {
    pub const COLD: Activity = Activity(0);
    pub const HOT: Activity = Activity(255);

    pub fn from_unit(a: f64) -> Activity {
        Activity((a.clamp(0.0, 1.0) * 255.0).round() as u8)
    }

    pub fn unit(self) -> f64 {
        f64::from(self.0) / f64::from(Self::HOT.0)
    }
}

/// A role, never a color. Call sites name what a span means; this module decides how that
/// looks, so the same meaning renders identically in every view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Title {
        color: Rgb,
        activity: Activity,
    },
    Project(Rgb),
    /// A task id: its fixed project shade toward white, independent of title activity.
    Id(Rgb),
    /// A goal id: bold, in its fixed project shade when assigned, else bold alone.
    GoalId(Option<Rgb>),
    /// A tree guide segment painted at its column depth.
    TreeGuide {
        color: Option<Rgb>,
        depth: usize,
    },
    Status(Status),
    Chrome,
    Emphasis,
    Warning,
    Error,
    Urgent,
    Ok,
    /// A date, painted by its distance from today (docs/configuration.md).
    Date(When),
    /// A priority, painted on the magenta scale when the painter has one
    /// (docs/configuration.md), else bold at P0 and P1.
    Priority(u8),
    /// A section or title, bold and optionally in its project color.
    Heading(Option<Rgb>),
}

/// Which day a date cell shows. `Today` is the due column's `now`, which names no date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Today,
    On(time::Date),
}

/// How far an id's shade sits from its project colour toward white, in OKLab.
const ID_TOWARD_WHITE: f64 = 0.3;
const WHITE: Rgb = Rgb {
    r: 255,
    g: 255,
    b: 255,
};

/// The truecolor shade used for task IDs of a project with an identity color.
pub(crate) fn id_color(color: Rgb) -> Rgb {
    color.mix(WHITE, ID_TOWARD_WHITE)
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

const COLD_TITLE_LIGHTNESS: f64 = 0.60;
const TITLE_LIGHTNESS_SPAN: f64 = 0.30;
const MIN_TITLE_CONTRAST: f64 = 3.5;

/// Shared OKLab lightness range, directed toward readable ink on the run's background.
#[derive(Debug, Clone, Copy)]
pub struct TitleScale {
    bg: Rgb,
    ink_lightness: f64,
}

impl TitleScale {
    pub fn new(palette: &Palette) -> TitleScale {
        let black = Rgb { r: 0, g: 0, b: 0 };
        TitleScale {
            bg: palette.bg,
            ink_lightness: if WHITE.contrast_ratio(palette.bg) >= black.contrast_ratio(palette.bg) {
                1.0
            } else {
                0.0
            },
        }
    }

    /// Retain a quantized color that passes contrast while approaching from the target.
    fn readable(&self, base: Rgb, lightness: f64) -> (f64, Rgb) {
        let color = base.with_lightness(lightness);
        if color.contrast_ratio(self.bg) >= MIN_TITLE_CONTRAST {
            return (lightness, color);
        }
        let (mut failed, mut passed) = (lightness, self.ink_lightness);
        let mut color = base.with_lightness(passed);
        for _ in 0..20 {
            let mid = 0.5 * (failed + passed);
            let candidate = base.with_lightness(mid);
            if candidate.contrast_ratio(self.bg) >= MIN_TITLE_CONTRAST {
                passed = mid;
                color = candidate;
            } else {
                failed = mid;
            }
        }
        (passed, color)
    }

    pub fn color(&self, base: Rgb, activity: Activity) -> Rgb {
        let (cold, _) = self.readable(base, COLD_TITLE_LIGHTNESS);
        let hot = if self.ink_lightness == 1.0 {
            (cold + TITLE_LIGHTNESS_SPAN).min(1.0)
        } else {
            (cold - TITLE_LIGHTNESS_SPAN).max(0.0)
        };
        self.readable(base, cold + (hot - cold) * activity.unit()).1
    }
}

const MIN_TREE_CONTRAST: f64 = 3.0;
const TREE_DEPTH_CAP: usize = 6;
const TREE_START_MUTE: f64 = 0.45;
const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

/// Depth-progressive OKLab connector scale with quantized 3:1 contrast floor against `bg`.
#[derive(Debug, Clone, Copy)]
pub struct TreeScale {
    bg: Rgb,
    target: Rgb,
    bw_ink: Rgb,
}

impl TreeScale {
    pub fn new(palette: &Palette) -> Self {
        let bw_ink = if WHITE.contrast_ratio(palette.bg) >= BLACK.contrast_ratio(palette.bg) {
            WHITE
        } else {
            BLACK
        };
        let target = if palette.fg.contrast_ratio(palette.bg) >= MIN_TREE_CONTRAST {
            palette.fg
        } else {
            bw_ink
        };
        TreeScale {
            bg: palette.bg,
            target,
            bw_ink,
        }
    }

    fn repair(&self, sampled: Rgb) -> Rgb {
        if sampled.contrast_ratio(self.bg) >= MIN_TREE_CONTRAST {
            return sampled;
        }
        let (mut failed, mut passed) = (0.0_f64, 1.0_f64);
        let mut result = self.bw_ink;
        debug_assert!(result.contrast_ratio(self.bg) >= MIN_TREE_CONTRAST);
        for _ in 0..20 {
            let mid = 0.5 * (failed + passed);
            let candidate = sampled.mix(self.bw_ink, mid);
            if candidate.contrast_ratio(self.bg) >= MIN_TREE_CONTRAST {
                passed = mid;
                result = candidate;
            } else {
                failed = mid;
            }
        }
        result
    }

    pub fn color(&self, project: Option<Rgb>, depth: usize) -> Rgb {
        let clamped = depth.clamp(1, TREE_DEPTH_CAP);
        let step = (clamped - 1) as f64 / (TREE_DEPTH_CAP - 1) as f64;
        let start = match project {
            Some(color) => id_color(color).mix(self.bg, TREE_START_MUTE),
            None => self.target.mix(self.bg, TREE_START_MUTE),
        };
        self.repair(start.mix(self.target, step))
    }
}

/// Paints for one output stream. `main` builds one per stream, because stdout and stderr
/// are redirected independently and only `Auto` can differ between them.
pub struct Painter {
    enabled: bool,
    recency: Option<Recency>,
    priority: Option<PriorityScale>,
    title: Option<TitleScale>,
    tree: Option<TreeScale>,
    sizing: bool,
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
            title: None,
            tree: None,
            sizing: false,
        }
    }

    /// Headings scale with kitty text sizing (OSC 66) only when `on`; `main` decides it
    /// with [`text_sizing_on`].
    pub fn with_text_sizing(self, on: bool) -> Painter {
        Painter { sizing: on, ..self }
    }

    pub fn sizing(&self) -> bool {
        self.sizing
    }

    /// `text` scaled for `role` as OSC 66 chunks when sizing is on, else unchanged.
    /// Style it (bold, colour) around the result: the terminal scales painted text.
    pub fn scaled(&self, text: &str, role: Role) -> String {
        if !self.sizing {
            return text.to_string();
        }
        let scale = role.scale();
        chunks(text, scale.payload_cells)
            .iter()
            .map(|payload| {
                format!(
                    "\x1b]66;s=2:n={}:d={}:w={}:v=2;{payload}\x07",
                    scale.numerator, scale.denominator, scale.width
                )
            })
            .collect()
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

    /// Titles modulate by activity only with a `TitleScale`; without one they use their
    /// unmodulated project color.
    pub fn with_title_scale(self, scale: TitleScale) -> Painter {
        Painter {
            title: Some(scale),
            ..self
        }
    }

    pub fn with_tree_scale(self, scale: TreeScale) -> Painter {
        Painter {
            tree: Some(scale),
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
            Style::Title { color, activity } => {
                let shade = match &self.title {
                    Some(scale) => scale.color(color, activity),
                    None => color,
                };
                format!("38;2;{};{};{}", shade.r, shade.g, shade.b)
            }
            Style::Project(color) => format!("38;2;{};{};{}", color.r, color.g, color.b),
            Style::Id(color) => {
                let shade = id_color(color);
                format!("38;2;{};{};{}", shade.r, shade.g, shade.b)
            }
            Style::GoalId(Some(color)) => {
                let shade = id_color(color);
                format!("1;38;2;{};{};{}", shade.r, shade.g, shade.b)
            }
            Style::GoalId(None) => "1".into(),
            Style::TreeGuide { color, depth } => match &self.tree {
                Some(scale) => {
                    let shade = scale.color(color, depth);
                    format!("38;2;{};{};{}", shade.r, shade.g, shade.b)
                }
                None => "2".into(),
            },
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
            Style::Heading(None) => "1".into(),
            Style::Heading(Some(color)) => format!("1;38;2;{};{};{}", color.r, color.g, color.b),
            Style::Status(Status::Idea) => "34".into(),
            Style::Status(Status::Todo) => return text.into(),
            Style::Status(Status::Doing) => "33".into(),
            Style::Status(Status::Blocked) => "31".into(),
            Style::Status(Status::Shelved) => "2;34".into(),
            Style::Status(Status::Done) => "2;32".into(),
            Style::Status(Status::Dropped | Status::Moved) => "2;31".into(),
            Style::Chrome => "2".into(),
            Style::Emphasis => "1".into(),
            Style::Warning => "33".into(),
            Style::Error => "31".into(),
            Style::Urgent => "1;31".into(),
            Style::Ok => "32".into(),
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
        assert_eq!(redirected.paint(Style::Heading(None), "heading"), "heading");

        let terminal = Painter::new(ColorMode::Auto, Format::Pretty, true);
        assert_eq!(
            terminal.paint(Style::Heading(None), "heading"),
            "\x1b[1mheading\x1b[0m"
        );

        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        for (style, code) in [
            (Style::Status(Status::Idea), "34"),
            (Style::Status(Status::Doing), "33"),
            (Style::Status(Status::Blocked), "31"),
            (Style::Status(Status::Shelved), "2;34"),
            (Style::Status(Status::Done), "2;32"),
            (Style::Status(Status::Dropped), "2;31"),
            (Style::Status(Status::Moved), "2;31"),
            (Style::Chrome, "2"),
            (Style::Emphasis, "1"),
            (Style::Warning, "33"),
            (Style::Error, "31"),
            (Style::Urgent, "1;31"),
            (Style::Ok, "32"),
            (Style::Heading(None), "1"),
        ] {
            let painted = colored.paint(style, "x");
            assert_eq!(painted, format!("\x1b[{code}mx\x1b[0m"));
            assert!(painted.ends_with("\x1b[0m"));
        }
        assert_eq!(
            colored.paint(Style::Heading(Some(Rgb { r: 1, g: 2, b: 3 })), "x"),
            "\x1b[1;38;2;1;2;3mx\x1b[0m"
        );
        assert_eq!(colored.paint(Style::Status(Status::Todo), "todo"), "todo");
    }

    #[test]
    fn the_id_role_paints_its_project_colour_lightened_toward_white() {
        let colored = Painter::new(ColorMode::Always, Format::Pretty, false);
        let hex = |value| Rgb::parse_hex(value).unwrap();
        // each colour mixed 30% of the way to white in OKLab, computed outside tasks
        for (base, shade) in [("#123456", "#546c87"), ("#abcdef", "#c4dcf4")] {
            let (base, shade) = (hex(base), hex(shade));
            assert_eq!(
                colored.paint(Style::Id(base), "x"),
                format!("\x1b[38;2;{};{};{}mx\x1b[0m", shade.r, shade.g, shade.b)
            );
            assert!(
                shade.lightness() > base.lightness(),
                "{base:?} -> {shade:?}"
            );
        }
        // white has nowhere lighter to go
        assert_eq!(
            colored.paint(Style::Id(hex("#ffffff")), "x"),
            "\x1b[38;2;255;255;255mx\x1b[0m"
        );
        let plain = Painter::new(ColorMode::Never, Format::Pretty, true);
        assert_eq!(plain.paint(Style::Id(hex("#123456")), "x"), "x");
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
        // collapses the scale (docs/task-model.md).
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
    #[test]
    fn activity_title_scale_covers_palette_edges() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/activity-title-mapping.json"
        ))
        .unwrap();
        let black = Rgb::parse_hex("#000000").unwrap();
        let white = Rgb::parse_hex("#ffffff").unwrap();
        for bg in fixture["backgrounds"].as_array().unwrap() {
            let bg = Rgb::parse_hex(bg.as_str().unwrap()).unwrap();
            let palette = Palette {
                bg,
                ..Palette::BUILTIN
            };
            let scale = TitleScale::new(&palette);
            let direction = if white.contrast_ratio(bg) >= black.contrast_ratio(bg) {
                1.0
            } else {
                -1.0
            };
            for hex in fixture["colors"].as_array().unwrap() {
                let base = Rgb::parse_hex(hex.as_str().unwrap()).unwrap();
                let cold = scale.color(base, Activity::COLD);
                let hot = scale.color(base, Activity::HOT);
                assert!(
                    (hot.lightness() - cold.lightness()) * direction >= 0.20,
                    "{hex} on {bg:?}"
                );
                let mut previous = cold.lightness();
                for a in 0..=255 {
                    let color = scale.color(base, Activity(a));
                    assert!(
                        color.contrast_ratio(bg) >= 3.5,
                        "{hex} at {a} on {bg:?}: {color:?}"
                    );
                    let lightness = color.lightness();
                    assert!(
                        (lightness - previous) * direction >= -0.002,
                        "{hex} at {a} on {bg:?}"
                    );
                    previous = lightness;
                }
                if bg == Palette::BUILTIN.bg {
                    assert!((cold.lightness() - 0.60).abs() <= 0.003, "{hex}");
                    assert!((hot.lightness() - 0.90).abs() <= 0.003, "{hex}");
                }
            }
        }
    }

    #[test]
    fn activity_title_scale_matches_captured_goals() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/activity-title-mapping.json"
        ))
        .unwrap();
        let scale = TitleScale::new(&Palette::BUILTIN);
        let rows = fixture["goals"].as_array().unwrap();
        assert_eq!(rows.len(), 80);
        let mut projects: std::collections::BTreeMap<&str, Vec<f64>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let base = Rgb::parse_hex(row["base"].as_str().unwrap()).unwrap();
            let actual = scale.color(
                base,
                Activity(row["activity"].as_u64().unwrap().try_into().unwrap()),
            );
            let expected = Rgb::parse_hex(row["expected"].as_str().unwrap()).unwrap();
            for (got, want) in [actual.r, actual.g, actual.b]
                .into_iter()
                .zip([expected.r, expected.g, expected.b])
            {
                assert!(got.abs_diff(want) <= 1, "{row}: {actual:?}");
            }
            projects
                .entry(row["project"].as_str().unwrap())
                .or_default()
                .push(actual.lightness());
        }
        assert_eq!(projects.len(), 19);
        for (project, expected_span) in [
            ("ops", 0.2993),
            ("sci", 0.2003),
            ("material", 0.0482),
            ("mind6", 0.2844),
        ] {
            let values = &projects[project];
            let low = values.iter().copied().fold(f64::INFINITY, f64::min);
            let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                (high - low - expected_span).abs() <= 0.006,
                "{project}: {}",
                high - low
            );
        }
    }

    #[test]
    fn painter_modulates_title_only_when_title_scale_is_present() {
        let base = Rgb::parse_hex("#abcdef").unwrap();
        let plain_painter = Painter::new(ColorMode::Always, Format::Pretty, true);
        assert_eq!(
            plain_painter.paint(
                Style::Title {
                    color: base,
                    activity: Activity::COLD
                },
                "Title"
            ),
            "\x1b[38;2;171;205;239mTitle\x1b[0m"
        );

        let scaled_painter = plain_painter.with_title_scale(TitleScale::new(&Palette::BUILTIN));
        assert_eq!(
            scaled_painter.paint(
                Style::Title {
                    color: base,
                    activity: Activity::HOT
                },
                "Title"
            ),
            "\x1b[38;2;197;226;255mTitle\x1b[0m"
        );
        let cold_painted = scaled_painter.paint(
            Style::Title {
                color: base,
                activity: Activity::COLD,
            },
            "Title",
        );
        assert_ne!(cold_painted, "\x1b[38;2;171;205;239mTitle\x1b[0m");
    }

    #[test]
    fn tree_guide_contrast_and_unassigned_depth() {
        let dark = Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        let light = Palette::parse("fg=#1e1e1e bg=#ffffff cyan=#005f87").unwrap();
        let arbitrary = Palette::parse("fg=#dbe7f0 bg=#223344 cyan=#00d7ff").unwrap();
        let low_fg = Palette::parse("fg=#283848 bg=#223344 cyan=#00d7ff").unwrap();
        let project = Rgb::parse_hex("#4e9af1").unwrap();

        for pal in [&dark, &light, &arbitrary, &low_fg] {
            let scale = TreeScale::new(pal);
            for proj in [None, Some(project)] {
                for depth in 1..=8 {
                    let c = scale.color(proj, depth);
                    assert!(
                        c.contrast_ratio(pal.bg) >= 3.0,
                        "contrast {} < 3.0 on bg {:?} proj {:?} depth {depth}",
                        c.contrast_ratio(pal.bg),
                        pal.bg,
                        proj
                    );
                }
            }
        }

        let dark_scale = TreeScale::new(&dark);
        assert_ne!(dark_scale.color(None, 1), dark_scale.color(None, 6));
        let light_scale = TreeScale::new(&light);
        assert_ne!(light_scale.color(None, 1), light_scale.color(None, 6));
    }

    #[test]
    fn tree_guide_column_depth_is_stable() {
        let dark = Palette::parse("fg=#e5e3d7 bg=#13140d cyan=#00d7ff").unwrap();
        let scale = TreeScale::new(&dark);
        let proj = Some(Rgb::parse_hex("#e07a5f").unwrap());

        assert_eq!(scale.color(proj, 0), scale.color(proj, 1));
        assert_ne!(scale.color(proj, 1), scale.color(proj, 2));
        assert_ne!(scale.color(proj, 2), scale.color(proj, 6));
        // Saturation at depth 6
        assert_eq!(scale.color(proj, 6), scale.color(proj, 7));
        assert_eq!(scale.color(proj, 6), scale.color(proj, 12));
        assert_eq!(scale.color(None, 6), scale.color(None, 10));
    }

    #[test]
    fn tree_guide_disabled_and_no_palette() {
        let rgb = Rgb::parse_hex("#123456").unwrap();
        let no_palette = Painter::new(ColorMode::Always, Format::Pretty, false);
        assert_eq!(
            no_palette.paint(
                Style::TreeGuide {
                    color: Some(rgb),
                    depth: 1
                },
                "├─ "
            ),
            "\x1b[2m├─ \x1b[0m"
        );
        assert_eq!(
            no_palette.paint(Style::GoalId(None), "aa-000001"),
            "\x1b[1maa-000001\x1b[0m"
        );
        assert_eq!(
            no_palette.paint(Style::GoalId(Some(rgb)), "aa-000001"),
            "\x1b[1;38;2;84;108;135maa-000001\x1b[0m"
        );

        let disabled = Painter::new(ColorMode::Never, Format::Pretty, false)
            .with_tree_scale(TreeScale::new(&Palette::BUILTIN));
        assert_eq!(
            disabled.paint(
                Style::TreeGuide {
                    color: Some(rgb),
                    depth: 2
                },
                "│  "
            ),
            "│  "
        );
        assert_eq!(
            disabled.paint(Style::GoalId(Some(rgb)), "aa-000001"),
            "aa-000001"
        );
        assert_eq!(
            disabled.paint(Style::GoalId(None), "aa-000001"),
            "aa-000001"
        );
    }

    #[test]
    fn tree_guide_saturated_hues_near_fg_floor() {
        let light_floor = Palette::parse("fg=#949494 bg=#ffffff cyan=#005f87").unwrap();
        let dark_floor = Palette::parse("fg=#5a5a5a bg=#000000 cyan=#00d7ff").unwrap();
        assert!(light_floor.fg.contrast_ratio(light_floor.bg) >= 3.0);
        assert!(dark_floor.fg.contrast_ratio(dark_floor.bg) >= 3.0);

        let hues = [
            None,
            Some(Rgb::parse_hex("#00ff00").unwrap()),
            Some(Rgb::parse_hex("#00ffff").unwrap()),
            Some(Rgb::parse_hex("#ffff00").unwrap()),
        ];

        for pal in [&light_floor, &dark_floor] {
            let scale = TreeScale::new(pal);
            for hue in hues {
                for depth in [1, 2, 3, 4, 5, 6, 9] {
                    let emitted = scale.color(hue, depth);
                    assert!(
                        emitted.contrast_ratio(pal.bg) >= 3.0,
                        "hue={hue:?} depth={depth} bg={:?} emitted={emitted:?} ratio={}",
                        pal.bg,
                        emitted.contrast_ratio(pal.bg)
                    );
                }
            }
        }
    }

    fn sizing(
        mode: TextSizing,
        color: ColorMode,
        terminal: bool,
        term: Option<&str>,
        tmux: bool,
    ) -> bool {
        let painter = Painter::new(color, Format::Pretty, terminal);
        text_sizing_on(
            mode,
            Format::Pretty,
            color,
            terminal,
            painter.enabled(),
            term,
            tmux,
        )
    }

    #[test]
    fn text_sizing_rejects_unknown_value() {
        assert_eq!(TextSizing::parse("auto").unwrap(), TextSizing::Auto);
        assert_eq!(TextSizing::parse("always").unwrap(), TextSizing::Always);
        assert_eq!(TextSizing::parse("never").unwrap(), TextSizing::Never);
        assert!(TextSizing::parse("big").is_err());
    }

    #[test]
    fn auto_mode_off_when_not_terminal() {
        let kitty = Some("xterm-kitty");
        assert!(sizing(
            TextSizing::Auto,
            ColorMode::Auto,
            true,
            kitty,
            false
        ));
        assert!(!sizing(
            TextSizing::Auto,
            ColorMode::Auto,
            false,
            kitty,
            false
        ));
        assert!(!sizing(
            TextSizing::Auto,
            ColorMode::Always,
            false,
            kitty,
            false
        ));
        assert!(!sizing(
            TextSizing::Auto,
            ColorMode::Auto,
            true,
            Some("xterm-256color"),
            false
        ));
        assert!(!sizing(
            TextSizing::Auto,
            ColorMode::Auto,
            true,
            None,
            false
        ));
        assert!(!sizing(
            TextSizing::Never,
            ColorMode::Auto,
            true,
            kitty,
            false
        ));
    }

    #[test]
    fn auto_mode_off_under_tmux() {
        assert!(!sizing(
            TextSizing::Auto,
            ColorMode::Auto,
            true,
            Some("xterm-kitty"),
            true
        ));
    }

    #[test]
    fn always_mode_emits_when_piped() {
        assert!(sizing(
            TextSizing::Always,
            ColorMode::Auto,
            false,
            None,
            true
        ));
        let painter = Painter::new(ColorMode::Auto, Format::Pretty, false).with_text_sizing(true);
        assert!(painter.sizing());
        assert!(
            painter
                .scaled("Tag", Role::GoalHeading)
                .starts_with("\x1b]66;")
        );
        assert!(!text_sizing_on(
            TextSizing::Always,
            Format::Json,
            ColorMode::Always,
            true,
            false,
            None,
            false
        ));
    }

    #[test]
    fn never_with_color_never() {
        assert!(!sizing(
            TextSizing::Always,
            ColorMode::Never,
            true,
            Some("xterm-kitty"),
            false
        ));
        let off = Painter::new(ColorMode::Never, Format::Pretty, true);
        assert!(!off.sizing());
        assert_eq!(
            off.scaled("Tag semantics", Role::LaneHeading),
            "Tag semantics"
        );
    }

    #[test]
    fn lane_scaled_chunks_are_four_characters() {
        let painter = Painter::new(ColorMode::Always, Format::Pretty, true).with_text_sizing(true);
        let chunk = |payload: &str| format!("\x1b]66;s=2:n=3:d=4:w=3:v=2;{payload}\x07");
        assert_eq!(
            painter.scaled("Tag semantics", Role::LaneHeading),
            [chunk("Tag "), chunk("sema"), chunk("ntic"), chunk("s   ")].concat()
        );
        assert_eq!(scaled_width("Tag semantics", Role::LaneHeading), 24);
    }

    #[test]
    fn goal_scaled_chunks_are_eight_characters() {
        let painter = Painter::new(ColorMode::Always, Format::Pretty, true).with_text_sizing(true);
        let chunk = |payload: &str| format!("\x1b]66;s=2:n=5:d=8:w=5:v=2;{payload}\x07");
        assert_eq!(
            painter.scaled("Tag semantics", Role::GoalHeading),
            [chunk("Tag sema"), chunk("ntics   ")].concat()
        );
        assert_eq!(scaled_width("Tag semantics", Role::GoalHeading), 20);
    }

    #[test]
    fn scaled_chunks_count_wide_cells() {
        let painter = Painter::new(ColorMode::Always, Format::Pretty, true).with_text_sizing(true);
        let chunk = |payload: &str| format!("\x1b]66;s=2:n=5:d=8:w=5:v=2;{payload}\x07");
        // 漢字abc = 7 cells; the next 漢 would straddle cell 8, so it opens the next chunk.
        assert_eq!(
            painter.scaled("漢字abc漢&x", Role::GoalHeading),
            [chunk("漢字abc "), chunk("漢&x    ")].concat()
        );
        assert_eq!(scaled_width("漢字abc漢&x", Role::GoalHeading), 20);
        // A wide glyph that ends exactly on the boundary stays in its chunk.
        assert_eq!(
            painter.scaled("漢字ab漢", Role::GoalHeading),
            chunk("漢字ab漢")
        );
        assert_eq!(scaled_width("", Role::GoalHeading), 0);
    }

    #[test]
    fn clip_for_heading_never_exceeds_width() {
        let long = "x".repeat(200);
        let lane = clip_for_heading(&long, Role::LaneHeading, 80);
        assert_eq!(crate::ellipsis::display_cells(&lane), 52);
        assert!(lane.ends_with('…'));
        assert!(scaled_width(&lane, Role::LaneHeading) <= 80);
        let goal = clip_for_heading(&long, Role::GoalHeading, 80);
        assert_eq!(crate::ellipsis::display_cells(&goal), 64);
        assert!(scaled_width(&goal, Role::GoalHeading) <= 80);
        assert_eq!(clip_for_heading("Short", Role::GoalHeading, 80), "Short");
    }

    #[test]
    fn goal_hue_is_stable_per_id() {
        let project = Rgb::parse_hex("#4e9af1").unwrap();
        let first = goal_hue(project, "tasks-cea445").unwrap();
        assert_eq!(goal_hue(project, "tasks-cea445"), Some(first));
        let hues: std::collections::BTreeSet<(u8, u8, u8)> = (0..40)
            .map(|n| goal_hue(project, &format!("tasks-{n:06x}")).unwrap())
            .map(|c| (c.r, c.g, c.b))
            .collect();
        assert!(hues.len() >= 4, "{hues:?}");
    }

    #[test]
    fn goal_hue_none_for_grey_project() {
        let grey = Rgb::parse_hex("#808080").unwrap();
        assert_eq!(goal_hue(grey, "tasks-cea445"), None);
    }
}
