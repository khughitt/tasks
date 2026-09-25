//! The terminal theme's colors: RGB values, OKLab mixing, and where they come from
//! (`TASKS_PALETTE`, or a query to the terminal). Design:
//! docs/specs/2026-09-25-date-recency-color-design.md.

use crate::error::Error;

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

/// The three theme colors the date scale needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cyan: Rgb,
}

impl Palette {
    /// Parses `TASKS_PALETTE`: `fg=#rrggbb bg=#rrggbb cyan=#rrggbb`, any order.
    pub fn parse(value: &str) -> crate::error::Result<Palette> {
        let bad = |detail: String| {
            Error::Config(format!(
                "TASKS_PALETTE must be \"fg=#rrggbb bg=#rrggbb cyan=#rrggbb\": {detail}"
            ))
        };
        let (mut fg, mut bg, mut cyan) = (None, None, None);
        for entry in value.split_whitespace() {
            let (key, hex) = entry
                .split_once('=')
                .ok_or_else(|| bad(format!("{entry:?} is not key=#rrggbb")))?;
            let slot = match key {
                "fg" => &mut fg,
                "bg" => &mut bg,
                "cyan" => &mut cyan,
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
            (Some(fg), Some(bg), Some(cyan)) => Ok(Palette { fg, bg, cyan }),
            _ => {
                let missing: Vec<&str> = [("fg", fg), ("bg", bg), ("cyan", cyan)]
                    .into_iter()
                    .filter(|(_, color)| color.is_none())
                    .map(|(key, _)| key)
                    .collect();
                Err(bad(format!("missing {}", missing.join(", "))))
            }
        }
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
    fn palette_parses_three_keys_in_any_order() {
        let palette = Palette::parse("cyan=#00d7ff fg=#e5e3d7  bg=#13140d").unwrap();
        assert_eq!(
            palette,
            Palette {
                fg: hex("#e5e3d7"),
                bg: hex("#13140d"),
                cyan: hex("#00d7ff"),
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
        ] {
            let error = Palette::parse(value).unwrap_err();
            assert_eq!(error.kind(), "config", "{value:?}");
            let text = error.to_string();
            assert!(text.contains("TASKS_PALETTE"), "{value:?}: {text}");
            assert!(text.contains(needle), "{value:?}: {text}");
        }
    }
}
