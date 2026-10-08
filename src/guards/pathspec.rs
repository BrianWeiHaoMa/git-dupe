//! The one constructor of the pathspecs git-dupe writes.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;

/// `:(top,literal)<path>`: exactly the root-relative `path`, and everything below it,
/// whatever bytes it holds, from any directory.
pub fn top_literal(path: &[u8]) -> OsString {
    OsString::from_vec([&b":(top,literal)"[..], path].concat())
}

/// `:(top,literal,exclude)<path>`: the same path, and everything below it, kept out of
/// what the run's other pathspecs bring in (S4).
pub fn top_literal_exclude(path: &[u8]) -> OsString {
    OsString::from_vec([&b":(top,literal,exclude)"[..], path].concat())
}

/// `:(top,glob,exclude)<exact>`, for `status` alone: the non-empty `path` kept out, and
/// nothing below it. `<exact>` is the path with `\` before each byte a glob reads, and its
/// last byte as the bracket expression `[\<byte>\<newline>]`, which `wildmatch` matches
/// against that byte or a newline and never as a leading directory. Git first compares
/// the pattern's raw bytes with each path, and leaves out a path spelled as the pattern
/// and everything below it; the newline makes that spelling no path a workspace holds
/// (E4, S4).
pub fn top_glob_exclude_exact(path: &[u8]) -> OsString {
    let (last, before) = path.split_last().expect("a path is never empty");
    let mut written = b":(top,glob,exclude)".to_vec();
    for &byte in before {
        if matches!(byte, b'\\' | b'*' | b'?' | b'[') {
            written.push(b'\\');
        }
        written.push(byte);
    }
    written.extend([b'[', b'\\', *last, b'\n', b']']);
    OsString::from_vec(written)
}

/// The root-relative `path` as a line offers it to be typed, from the root, as a pathspec
/// of a command Git runs: as it stands, or `:(literal)<path>` where Git would read a
/// leading `:` as magic, or `*`, `?`, `[`, or `\` as a pattern that names other paths too,
/// so that the command acts on that path alone (G25).
pub fn offered(path: &[u8]) -> Vec<u8> {
    let read_otherwise = path.starts_with(b":")
        || path
            .iter()
            .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b'\\'));
    if read_otherwise {
        [&b":(literal)"[..], path].concat()
    } else {
        path.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    #[test]
    fn an_offered_pathspec_is_literal_only_where_git_would_read_it_otherwise() {
        for plain in [&b"notes"[..], b"two $words/a b", b"-x", b"a:b", b"caf\xe9"] {
            assert_eq!(offered(plain), plain);
        }
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
                [&b":(literal)"[..], read_otherwise].concat()
            );
        }
    }

    #[test]
    fn a_path_is_written_after_the_magic_byte_for_byte() {
        assert_eq!(top_literal(b"notes").as_bytes(), b":(top,literal)notes");
        assert_eq!(
            top_literal(b"a b/*[x]\xe9\r").as_bytes(),
            b":(top,literal)a b/*[x]\xe9\r"
        );
        assert_eq!(top_literal(b":x").as_bytes(), b":(top,literal):x");
        assert_eq!(
            top_literal_exclude(b"notes/a b\xe9").as_bytes(),
            b":(top,literal,exclude)notes/a b\xe9"
        );
    }

    #[test]
    fn an_exact_exclusion_escapes_what_a_glob_reads_and_brackets_the_last_byte() {
        assert_eq!(
            top_glob_exclude_exact(b"notes/sub").as_bytes(),
            b":(top,glob,exclude)notes/su[\\b\n]"
        );
        assert_eq!(
            top_glob_exclude_exact(b"h").as_bytes(),
            b":(top,glob,exclude)[\\h\n]"
        );
        assert_eq!(
            top_glob_exclude_exact(b"a\\b*c?d[e]f\xe9 :!^-").as_bytes(),
            b":(top,glob,exclude)a\\\\b\\*c\\?d\\[e]f\xe9 :!^[\\-\n]"
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
            assert_eq!(
                written.as_bytes(),
                [&b":(top,glob,exclude)x"[..], bracketed].concat()
            );
        }
    }
}
