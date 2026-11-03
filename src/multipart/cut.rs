use crate::error::{Error, Result};
use std::ops::Range;

pub fn cut(whole: &[u8], max_part: u64) -> Result<Vec<Range<usize>>> {
    let text =
        std::str::from_utf8(whole).map_err(|error| Error::MultipartNotText(error.to_string()))?;
    if whole.is_empty() {
        return Ok(std::iter::once(0..0).collect());
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while start < whole.len() {
        let mut end = start + max_part.min((whole.len() - start) as u64) as usize;
        if let Some(newline) = whole[start..end].iter().rposition(|byte| *byte == b'\n') {
            end = start + newline + 1;
        } else {
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }
        }
        if end == start {
            let length = text[start..]
                .chars()
                .next()
                .expect("nonempty remainder")
                .len_utf8();
            return Err(Error::MultipartUncuttable(format!(
                "multipart cap {max_part} bytes cannot hold character at byte offset {start} with length {length} bytes"
            )));
        }
        parts.push(start..end);
        start = end;
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_prefers_newlines() {
        assert_eq!(
            cut(b"123456789\n123456789\n123456789\n", 25).unwrap(),
            [0..20, 20..30]
        );
    }
    #[test]
    fn cut_never_splits_a_utf8_character() {
        let text = "é".repeat(9);
        let parts = cut(text.as_bytes(), 5).unwrap();
        assert_eq!(
            parts
                .iter()
                .flat_map(|r| &text.as_bytes()[r.clone()])
                .copied()
                .collect::<Vec<_>>(),
            text.as_bytes()
        );
        assert!(
            parts
                .iter()
                .all(|r| std::str::from_utf8(&text.as_bytes()[r.clone()]).is_ok())
        );
    }
    #[test]
    fn cut_handles_empty_and_unterminated_input() {
        let empty = cut(b"", 5).unwrap();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0], 0..0);
        assert_eq!(cut(b"abcde", 3).unwrap(), [0..3, 3..5]);
    }
    #[test]
    fn cut_parts_never_exceed_the_cap() {
        for cap in 4..20 {
            let text = "aé日🦀\nxyz\n".repeat(20);
            let parts = cut(text.as_bytes(), cap).unwrap();
            assert!(
                parts
                    .iter()
                    .all(|range| !range.is_empty() && range.len() <= cap as usize)
            );
            assert_eq!(parts.first().unwrap().start, 0);
            assert_eq!(parts.last().unwrap().end, text.len());
            assert!(parts.windows(2).all(|pair| pair[0].end == pair[1].start));
        }
    }
    #[test]
    fn cut_refuses_when_no_character_fits() {
        let error = cut("é".as_bytes(), 1).unwrap_err();
        assert_eq!(error.kind(), "multipart_uncuttable");
        assert!(error.to_string().contains("1"));
        assert!(error.to_string().contains("0"));
        assert!(error.to_string().contains("2"));
        assert_eq!(cut(b"aaa", 1).unwrap(), [0..1, 1..2, 2..3]);
        assert_eq!(cut(b"a", 0).unwrap_err().kind(), "multipart_uncuttable");
        assert_eq!(cut(&[255], 1).unwrap_err().kind(), "multipart_not_text");
    }
}
