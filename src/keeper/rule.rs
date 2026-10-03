//! The region rule of a path, and its reading back (F3, S5).
//!
//! A rule is `/`, then the path with `\` before each `\`, `*`, `?`, `[`, `#`, `!`, and
//! space, and each carriage return written as the bracket expression `[<CR>]`. Anchored,
//! literal, and without a trailing slash, it matches exactly that path whether a file, a
//! directory, or nothing stands there; an escaped trailing space is kept, and a leading
//! `#` or `!` is neither a comment nor a negation.

use crate::guards::operand::{self, Cleaned};

/// The rule of a root-relative path.
pub fn of(path: &[u8]) -> Vec<u8> {
    let mut rule = Vec::with_capacity(path.len() + 1);
    rule.push(b'/');
    for &byte in path {
        match byte {
            b'\\' | b'*' | b'?' | b'[' | b'#' | b'!' | b' ' => {
                rule.extend_from_slice(&[b'\\', byte])
            }
            b'\r' => rule.extend_from_slice(b"[\r]"),
            _ => rule.push(byte),
        }
    }
    rule
}

/// The path a rule was written for: the exact inverse of `of` over the paths a region
/// holds. Any other line is no rule of the region's and reads as nothing.
pub fn path_of(rule: &[u8]) -> Option<Vec<u8>> {
    let mut rest = rule.strip_prefix(b"/")?;
    let mut path = Vec::with_capacity(rest.len());
    while let Some((&byte, after)) = rest.split_first() {
        rest = match byte {
            b'\\' => {
                let (&escaped, after) = after.split_first()?;
                path.push(escaped);
                after
            }
            b'[' => {
                let after = after.strip_prefix(b"\r]")?;
                path.push(b'\r');
                after
            }
            _ => {
                path.push(byte);
                after
            }
        };
    }
    let clean = operand::clean(&path) == Cleaned::Inside(path.clone());
    (clean && of(&path) == rule).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ten byte-sensitive paths, each with its rule.
    const ESCAPED: [(&[u8], &[u8]); 10] = [
        (b"has space", b"/has\\ space"),
        (b"trail ", b"/trail\\ "),
        (b"#hash", b"/\\#hash"),
        (b"!bang", b"/\\!bang"),
        (b"star*", b"/star\\*"),
        (b"q?x", b"/q\\?x"),
        (b"br[1]", b"/br\\[1]"),
        (b"back\\slash", b"/back\\\\slash"),
        (b"cr\rx", b"/cr[\r]x"),
        (b"caf\xe9", b"/caf\xe9"),
    ];

    #[test]
    fn each_byte_git_would_read_as_a_pattern_is_escaped() {
        for (path, rule) in ESCAPED {
            assert_eq!(of(path), rule, "{}", path.escape_ascii());
        }
        assert_eq!(of(b"notes"), b"/notes");
        assert_eq!(of(b".gitdupe"), b"/.gitdupe");
        assert_eq!(of(b"docs/plan.md"), b"/docs/plan.md");
    }

    #[test]
    fn reading_back_is_the_exact_inverse() {
        let plain: [&[u8]; 4] = [b"notes", b".gitdupe", b"a/b", b"x]y"];
        for path in ESCAPED.iter().map(|(path, _)| *path).chain(plain) {
            assert_eq!(
                path_of(&of(path)).as_deref(),
                Some(path),
                "{}",
                path.escape_ascii()
            );
        }
    }

    #[test]
    fn a_line_no_path_has_as_its_rule_reads_as_nothing() {
        for line in [
            &b""[..],
            b"/",
            b"notes",
            b"/notes/",
            b"/a//b",
            b"/./a",
            b"/a/../b",
            b"/has space",
            b"/star*",
            b"/#hash",
            b"/br[1]",
            b"/cr\\\rx",
            b"/trailing\\",
            b"/\\n",
            b"!/notes",
            b"# BEGIN git-dupe",
        ] {
            assert_eq!(path_of(line), None, "{}", line.escape_ascii());
        }
    }
}
