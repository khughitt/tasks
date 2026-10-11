//! The look of each task field, decided once (show layout spec §4). Views ask this module
//! for a field's text and role, then pad and paint the result; none of them names a
//! field's role itself, so a field reads the same in every view.

use crate::model::{Complexity, Process, Size, Status};
use crate::output::ClaimInfo;
use crate::palette::Rgb;
use crate::style::{Painter, Style, When};

/// A field's text and the role it paints in. Pad the text first, paint last.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Option<Style>,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Span {
        Span {
            text: text.into(),
            style: None,
        }
    }

    fn styled(text: impl Into<String>, style: Style) -> Span {
        Span {
            text: text.into(),
            style: Some(style),
        }
    }

    /// Left-aligned to `width` visible columns, before painting.
    pub fn left(self, width: usize) -> Span {
        Span {
            text: format!("{:<width$}", self.text),
            ..self
        }
    }

    pub fn paint(&self, painter: &Painter) -> String {
        match self.style {
            Some(style) => painter.paint(style, &self.text),
            None => self.text.clone(),
        }
    }
}

/// A task id, wherever it appears.
pub fn id(id: &str) -> Span {
    Span::styled(id, Style::Chrome)
}

/// A task id in a table row: its project's colour when it has one, else chrome.
pub fn row_id(id: &str, project: Option<Rgb>) -> Span {
    Span {
        text: id.into(),
        style: Some(project.map_or(Style::Chrome, Style::Project)),
    }
}

/// The review prefix of an id cell: marked, reserved, or absent.
pub fn review_mark(needs_review: bool, reserved: bool) -> Span {
    let text = match (reserved, needs_review) {
        (false, _) => "",
        (true, true) => "* ",
        (true, false) => "  ",
    };
    Span {
        text: text.into(),
        style: needs_review.then_some(Style::Emphasis),
    }
}

/// A title in a row: the project's color when it has one, else plain.
pub fn title(title: &str, project: Option<Rgb>) -> Span {
    Span {
        text: title.into(),
        style: project.map(Style::Project),
    }
}

/// Text shown as a heading: bold, in the project's color when it has one.
pub fn heading(text: &str, project: Option<Rgb>) -> Span {
    Span::styled(text, Style::Heading(project))
}

/// A project prefix: its color when assigned, else dim.
pub fn project(prefix: &str, color: Option<Rgb>) -> Span {
    Span::styled(prefix, color.map(Style::Project).unwrap_or(Style::Chrome))
}

pub fn status(status: Status) -> Span {
    Span::styled(status.as_str(), Style::Status(status))
}

/// A status that may be unknown: an unresolved dependency, or a park whose record is gone.
pub fn status_of(status: Option<Status>) -> Span {
    match status {
        Some(known) => self::status(known),
        None => Span::styled("?", Style::Chrome),
    }
}

pub fn priority(priority: u8) -> Span {
    Span::styled(format!("P{priority}"), Style::Priority(priority))
}

pub fn size(size: Option<Size>) -> Span {
    Span::plain(size.map(Size::as_str).unwrap_or("-"))
}

pub fn complexity(complexity: Option<Complexity>) -> Span {
    Span::plain(complexity.map(Complexity::as_str).unwrap_or("-"))
}

/// `unset` is what an unassessed task shows: `-` in a column, `unassessed` in `show`.
pub fn process(process: Option<Process>, unset: &str) -> Span {
    Span::plain(process.map(Process::as_str).unwrap_or(unset))
}

pub fn owner(owner: &str) -> Span {
    Span::styled(owner, Style::Chrome)
}

/// Tags comma-separated, without brackets; a caller that wants brackets adds them.
pub fn tags(tags: &[String]) -> Span {
    Span::styled(tags.join(", "), Style::Chrome)
}

/// A validated timestamp or `YYYY-MM-DD` day, shown as the day and painted by its age.
pub fn date(timestamp: &str) -> Span {
    let day = crate::time::day(timestamp);
    let date =
        crate::time::calendar_day(day).expect("records and park entries carry validated dates");
    Span::styled(day, Style::Date(When::On(date)))
}

/// The due column's `now`, which names no date.
pub fn today() -> Span {
    Span::styled("now", Style::Date(When::Today))
}

/// `<owner> [<session>]`, with ` stale` inside the brackets when the claim is not live.
pub fn claim(claim: &ClaimInfo) -> Span {
    let stale = if claim.live { "" } else { " stale" };
    Span::styled(
        format!("{} [{}{stale}]", claim.owner, claim.session),
        Style::Chrome,
    )
}

/// A status count, dimmed at zero whatever its status: a red `0` under `blocked` reads
/// as an alarm.
pub fn count(status: Status, value: usize) -> Span {
    let style = if value == 0 {
        Style::Chrome
    } else {
        Style::Status(status)
    };
    Span::styled(value.to_string(), style)
}

/// `── name ` in the heading role, then a dim rule out to `width` (show layout §3.2).
pub fn rule(name: &str, painter: &Painter, width: usize) -> String {
    let head = format!("── {name} ");
    let fill = width.saturating_sub(head.chars().count());
    let mut line = heading(&head, None).paint(painter);
    if fill > 0 {
        line.push_str(&painter.paint(Style::Chrome, &"─".repeat(fill)));
    }
    line.push('\n');
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_carry_their_roles() {
        assert_eq!(id("xx-000001").style, Some(Style::Chrome));
        assert_eq!(owner("me").style, Some(Style::Chrome));
        assert_eq!(
            status(Status::Doing).style,
            Some(Style::Status(Status::Doing))
        );
        assert_eq!(status_of(None), Span::styled("?", Style::Chrome));
        assert_eq!(priority(1), Span::styled("P1", Style::Priority(1)));
        assert_eq!(size(None), Span::plain("-"));
        assert_eq!(complexity(Some(Complexity::Low)), Span::plain("low"));
        assert_eq!(process(None, "unassessed"), Span::plain("unassessed"));
        assert_eq!(process(Some(Process::Direct), "-"), Span::plain("direct"));
        assert_eq!(
            tags(&["a".into(), "b".into()]),
            Span::styled("a, b", Style::Chrome)
        );
        assert_eq!(today(), Span::styled("now", Style::Date(When::Today)));
        assert_eq!(title("T", None), Span::plain("T"));
        assert_eq!(project("xx", None).style, Some(Style::Chrome));
        assert_eq!(count(Status::Blocked, 0).style, Some(Style::Chrome));
        assert_eq!(
            count(Status::Blocked, 2).style,
            Some(Style::Status(Status::Blocked))
        );
    }

    #[test]
    fn a_date_shows_the_day_and_paints_by_it() {
        let span = date("2026-09-25T15:47:16Z");
        assert_eq!(span.text, "2026-09-25");
        let day = ::time::Date::from_calendar_date(2026, ::time::Month::September, 25).unwrap();
        assert_eq!(span.style, Some(Style::Date(When::On(day))));
        assert_eq!(date("2026-09-25").text, "2026-09-25");
    }

    #[test]
    fn left_pads_before_painting() {
        assert_eq!(id("ab").left(4).text, "ab  ");
    }

    /// Spec §4.1: the views ask this module, so a field cannot look different in two
    /// of them. Test code is exempt.
    #[test]
    fn field_roles_are_built_only_here() {
        for (file, text) in [
            ("output.rs", include_str!("output.rs")),
            ("show_text.rs", include_str!("show_text.rs")),
        ] {
            let code = text.split("#[cfg(test)]").next().unwrap();
            for role in [
                "Style::Status(",
                "Style::Priority(",
                "Style::Date(",
                "Style::Project(",
            ] {
                assert!(
                    !code.contains(role),
                    "{file} builds {role} itself; ask present instead"
                );
            }
        }
    }
}
