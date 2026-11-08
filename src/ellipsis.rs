//! Clip unpainted text with an ellipsis, measured in the caller's explicit units.
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Copy)]
pub enum Measure {
    Characters,
    DisplayCells,
}

pub fn clip(text: &str, limit: usize, measure: Measure) -> String {
    if limit == 0 {
        return String::new();
    }
    let width = |c: char| match measure {
        Measure::Characters => 1,
        Measure::DisplayCells => c.width().unwrap_or(0),
    };
    if text.chars().map(width).sum::<usize>() <= limit {
        return text.to_owned();
    }
    let mut used = 0;
    let mut end = 0;
    for (at, c) in text.char_indices() {
        let next = width(c);
        if used + next > limit - 1 {
            break;
        }
        used += next;
        end = at + c.len_utf8();
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_budget_drops_even_zero_width_text() {
        assert_eq!(clip("\u{301}", 0, Measure::DisplayCells), "");
    }
    #[test]
    fn clipping_preserves_fitting_text_and_reserves_the_ellipsis() {
        for (text, limit, measure, expected) in [
            ("abcdef", 4, Measure::Characters, "abc…"),
            ("abc", 3, Measure::Characters, "abc"),
            ("界界", 3, Measure::DisplayCells, "界…"),
            ("界界", 2, Measure::DisplayCells, "…"),
            ("界界", 2, Measure::Characters, "界界"),
            ("e\u{301}xxxx", 2, Measure::DisplayCells, "e\u{301}…"),
            ("abc", 1, Measure::DisplayCells, "…"),
            ("abc", 0, Measure::DisplayCells, ""),
        ] {
            assert_eq!(clip(text, limit, measure), expected);
        }
    }
    #[test]
    fn display_cells_match_the_table_on_emoji_and_variation_selectors() {
        for text in ["👩‍💻ab", "❤\u{fe0f}ab", "👋🏿abc", "界e\u{301}xy"] {
            for limit in 0..8 {
                let clipped = clip(text, limit, Measure::DisplayCells);
                assert!(
                    crate::table::visible_width(&clipped) <= limit,
                    "{clipped:?} at {limit}"
                );
                if crate::table::visible_width(text) <= limit {
                    assert_eq!(clipped, text);
                } else if limit > 0 {
                    assert!(clipped.ends_with('…'));
                }
            }
        }
    }
}
