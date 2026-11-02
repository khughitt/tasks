//! What the user reads before approving a list (spec §4.8). This part: the body diff.

/// A line diff from `old` to `new` by longest common subsequence: every line of both
/// appears once, in order, prefixed `" "` (in both), `"-"` (only in `old`) or `"+"` (only
/// in `new`); within a change, removals come before additions. Each line ends with `\n`.
pub fn body_diff(old: &str, new: &str) -> String {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    // lengths[i][j]: the LCS length of old[i..] and new[j..].
    let mut lengths = vec![vec![0usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            lengths[i][j] = if old[i] == new[j] {
                lengths[i + 1][j + 1] + 1
            } else {
                lengths[i + 1][j].max(lengths[i][j + 1])
            };
        }
    }
    let mut out = String::new();
    let mut push = |mark: char, line: &str| {
        out.push(mark);
        out.push_str(line);
        out.push('\n');
    };
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            push(' ', old[i]);
            i += 1;
            j += 1;
        } else if j == new.len() || (i < old.len() && lengths[i + 1][j] >= lengths[i][j + 1]) {
            push('-', old[i]);
            i += 1;
        } else {
            push('+', new[j]);
            j += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::body_diff;

    #[test]
    fn every_line_appears_once_with_its_mark() {
        assert_eq!(body_diff("a\nb\nc", "a\nx\nc\nd"), " a\n-b\n+x\n c\n+d\n");
        assert_eq!(body_diff("", "é\n\n- item"), "+é\n+\n+- item\n");
        assert_eq!(body_diff("gone", ""), "-gone\n");
        assert_eq!(body_diff("same\n", "same"), " same\n");
        assert_eq!(body_diff("", ""), "");
    }

    #[test]
    fn repeated_lines_keep_the_longest_common_run() {
        assert_eq!(body_diff("x\ny\nx", "y\nx\ny"), "-x\n y\n x\n+y\n");
    }
}
