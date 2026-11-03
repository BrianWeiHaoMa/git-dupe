//! The one constructor of the pathspecs git-dupe writes.

use super::quoted::shell_word;
use crate::bytes;

/// `:(top,literal)<path>`: exactly the root-relative `path`, and everything below it,
/// whatever bytes it holds, from any directory.
pub fn top_literal(path: &[u8]) -> Vec<u8> {
    [&b":(top,literal)"[..], path].concat()
}

/// `:(top,literal,exclude)<path>`: the same path, and everything below it, kept out of
/// what the run's other pathspecs bring in (S4).
pub fn top_literal_exclude(path: &[u8]) -> Vec<u8> {
    [&b":(top,literal,exclude)"[..], path].concat()
}

/// `:(top,literal,icase)<path>`, for the one listing `clean` asks alone: the index entries
/// at or below the path in every spelling of its ASCII case (S4), as Git's `clean` reads an
/// ignore rule under `core.ignoreCase` (S18). Whatever the listing names, git-dupe goes on
/// comparing by bytes.
pub fn top_literal_icase(path: &[u8]) -> Vec<u8> {
    [&b":(top,literal,icase)"[..], path].concat()
}

/// `:(top,glob,exclude)<exact>`, for `status` alone: the non-empty `path` kept out, and
/// nothing below it. `<exact>` is the path with `\` before each byte a glob reads, and its
/// last ASCII byte other than `/`, or its last byte where it has none, as the bracket
/// expression `[\<byte>\<newline>]`, which `wildmatch` matches against that byte or a
/// newline and never as a leading directory. Git first compares the pattern's raw bytes
/// with each path, and leaves out a path spelled as the pattern and everything below it;
/// the newline makes that spelling no path a workspace holds (E4, S4).
///
/// The bracket stands on an ASCII byte because a Windows command line carries no part of
/// a character (S20). Where it would have to stand on one, the word is no name there, and
/// `top_literal_exclude`'s form stands in, which leaves out what lies below the path too
/// (`Holds/G12`); on Linux and macOS, where every byte string is a name, it never does.
pub fn top_glob_exclude_exact(path: &[u8]) -> Vec<u8> {
    let bracketed = path
        .iter()
        .rposition(|&byte| byte.is_ascii() && byte != b'/')
        .unwrap_or(path.len().saturating_sub(1));
    let (before, rest) = path.split_at(bracketed);
    let (byte, after) = rest.split_first().expect("a path is never empty");
    let mut written = b":(top,glob,exclude)".to_vec();
    for &byte in before {
        if matches!(byte, b'\\' | b'*' | b'?' | b'[') {
            written.push(b'\\');
        }
        written.push(byte);
    }
    written.extend([b'[', b'\\', *byte, b'\n', b']']);
    // What follows the bracket is `/` and bytes above ASCII, none of which a glob reads.
    written.extend_from_slice(after);
    if bytes::name(&written).is_some() {
        written
    } else {
        top_literal_exclude(path)
    }
}

/// The command a line offers to be typed, from the root, to run Git's `words` on the
/// root-relative `path` alone (G25): `git <words> -- <path>`, the path written as a shell
/// reads it back as one word. Where Git would read a leading `:` as magic, or `*`, `?`,
/// `[`, or `\` as a pattern that names other paths too, the path is `:(literal)<path>`, and
/// `--no-literal-pathspecs` goes before the words, because a developer's own
/// `GIT_LITERAL_PATHSPECS` governs what they type (G19) and would read that magic as part of
/// the path; a glob setting yields to `literal`. A plain path needs neither, and a literal
/// setting reads it as it stands.
pub fn offered_command(words: &[u8], path: &[u8]) -> Vec<u8> {
    let read_otherwise = path.starts_with(b":")
        || path
            .iter()
            .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b'\\'));
    let (global, pathspec): (&[u8], Vec<u8>) = if read_otherwise {
        (
            b"--no-literal-pathspecs ",
            [&b":(literal)"[..], path].concat(),
        )
    } else {
        (b"", path.to_vec())
    };
    [b"git ", global, words, b" -- ", &shell_word(&pathspec)].concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::names_every_byte_string;

    #[test]
    fn an_offered_pathspec_is_literal_only_where_git_would_read_it_otherwise() {
        let offered = |path: &[u8]| offered_command(b"rm --cached", path);
        for plain in [&b"notes"[..], b"-x", b"a:b", b"caf\xe9"] {
            assert_eq!(
                offered(plain),
                [&b"git rm --cached -- "[..], plain].concat()
            );
        }
        assert_eq!(
            offered(b"two $words/a b"),
            b"git rm --cached -- \"two \\$words/a b\""
        );
        for read_otherwise in [
            &b":(exclude)keep"[..],
            b":x",
            b"star*.txt",
            b"a?b",
            b"[x]",
            b"a\\b",
        ] {
            assert_eq!(
                offered(read_otherwise),
                [
                    &b"git --no-literal-pathspecs rm --cached -- "[..],
                    &shell_word(&[&b":(literal)"[..], read_otherwise].concat()),
                ]
                .concat()
            );
        }
        assert_eq!(
            offered_command(b"dupe restore", b"star*.txt"),
            b"git --no-literal-pathspecs dupe restore -- \":(literal)star*.txt\""
        );
    }

    #[test]
    fn a_path_is_written_after_the_magic_byte_for_byte() {
        assert_eq!(top_literal(b"notes"), b":(top,literal)notes");
        assert_eq!(
            top_literal(b"a b/*[x]\xe9\r"),
            b":(top,literal)a b/*[x]\xe9\r"
        );
        assert_eq!(top_literal(b":x"), b":(top,literal):x");
        assert_eq!(
            top_literal_icase(b"a b/*\xe9"),
            b":(top,literal,icase)a b/*\xe9"
        );
        assert_eq!(
            top_literal_exclude(b"notes/a b\xe9"),
            b":(top,literal,exclude)notes/a b\xe9"
        );
    }

    #[test]
    fn an_exact_exclusion_escapes_what_a_glob_reads_and_brackets_the_last_byte() {
        assert_eq!(
            top_glob_exclude_exact(b"notes/sub"),
            b":(top,glob,exclude)notes/su[\\b\n]"
        );
        assert_eq!(top_glob_exclude_exact(b"h"), b":(top,glob,exclude)[\\h\n]");
        assert_eq!(
            top_glob_exclude_exact(b"a\\b*c?d[e]f\xc3\xa9 :!^-"),
            b":(top,glob,exclude)a\\\\b\\*c\\?d\\[e]f\xc3\xa9 :!^[\\-\n]"
        );
        // A last byte a bracket expression would otherwise read: escaped inside it.
        for (last, bracketed) in [
            (b'\\', &b"[\\\\\n]"[..]),
            (b']', b"[\\]\n]"),
            (b'[', b"[\\[\n]"),
            (b'!', b"[\\!\n]"),
            (b'^', b"[\\^\n]"),
            (b'*', b"[\\*\n]"),
            (b'-', b"[\\-\n]"),
        ] {
            let written = top_glob_exclude_exact(&[b'x', last]);
            assert_eq!(written, [&b":(top,glob,exclude)x"[..], bracketed].concat());
        }
    }

    #[test]
    fn the_bracket_stands_on_the_last_ascii_byte_and_what_follows_it_is_written_as_it_is() {
        assert_eq!(
            top_glob_exclude_exact(b"notes/caf\xc3\xa9"),
            b":(top,glob,exclude)notes/ca[\\f\n]\xc3\xa9"
        );
        // A `/` is no byte to bracket: it would match no separator.
        assert_eq!(
            top_glob_exclude_exact(b"notes/\xc3\xa9"),
            b":(top,glob,exclude)note[\\s\n]/\xc3\xa9"
        );
        assert_eq!(
            top_glob_exclude_exact(b"a*\xc3\xa9/\xc3\xa9"),
            b":(top,glob,exclude)a[\\*\n]\xc3\xa9/\xc3\xa9"
        );
        assert_eq!(
            top_glob_exclude_exact(b"\xc3\xa9/x\xc3\xa9"),
            b":(top,glob,exclude)\xc3\xa9/[\\x\n]\xc3\xa9"
        );
    }

    #[test]
    fn a_path_with_no_ascii_byte_is_bracketed_only_where_part_of_a_character_is_a_name() {
        for (path, split) in [
            (&b"\xc3\xa9"[..], &b"\xc3[\\\xa9\n]"[..]),
            (b"\xc3\xa9/\xc3\xa9", b"\xc3\xa9/\xc3[\\\xa9\n]"),
        ] {
            let expected = if names_every_byte_string() {
                [&b":(top,glob,exclude)"[..], split].concat()
            } else {
                top_literal_exclude(path)
            };
            assert_eq!(top_glob_exclude_exact(path), expected);
        }
    }
}
