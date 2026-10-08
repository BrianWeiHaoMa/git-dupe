//! A name as Git quotes a path that needs it, so that a line naming it stays one line
//! whatever it holds (F8): between double quotes, `"` and `\` escaped, and each control
//! byte written as Git's C-style escape. Git's lines name such a path the same way.
//! git-dupe's lines use it for a name holding a newline, which E4 keeps out of the
//! workspace's paths but not out of a word the user types or a file Git reads rules from.
//!
//! A path a line offers as an operand of a command to run is written by `shell_word`,
//! so that the command, typed as the line shows it, gives that path as one word (G25).

/// `name` between double quotes, escaped as Git escapes a path it quotes.
pub fn quoted(name: &[u8]) -> Vec<u8> {
    let mut quoted = vec![b'"'];
    for &byte in name {
        let escape: &[u8] = match byte {
            b'"' => b"\\\"",
            b'\\' => b"\\\\",
            0x07 => b"\\a",
            0x08 => b"\\b",
            b'\t' => b"\\t",
            b'\n' => b"\\n",
            0x0b => b"\\v",
            0x0c => b"\\f",
            b'\r' => b"\\r",
            0x00..=0x1f | 0x7f => {
                quoted.extend_from_slice(format!("\\{byte:03o}").as_bytes());
                continue;
            }
            _ => {
                quoted.push(byte);
                continue;
            }
        };
        quoted.extend_from_slice(escape);
    }
    quoted.push(b'"');
    quoted
}

/// `word` as a POSIX shell reads it back as one word: as it stands when it holds only
/// bytes no shell treats specially, else between double quotes, with `\`, `"`, `$`,
/// and `` ` `` escaped. A byte above ASCII is no shell's concern, and a leading `-` is
/// the command's, which writes `--` before the word.
pub fn shell_word(word: &[u8]) -> Vec<u8> {
    let plain = |byte: &u8| {
        byte.is_ascii_alphanumeric() || b"._/@%+=:,-".contains(byte) || !byte.is_ascii()
    };
    if !word.is_empty() && word.iter().all(plain) {
        return word.to_vec();
    }
    let mut quoted = vec![b'"'];
    for &byte in word {
        if matches!(byte, b'\\' | b'"' | b'$' | b'`') {
            quoted.push(b'\\');
        }
        quoted.push(byte);
    }
    quoted.push(b'"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_is_quoted_only_where_a_shell_would_read_it_otherwise() {
        for plain in [&b"notes"[..], b"-x", b"a/b.c", b"caf\xe9", b"a=b,c:d@e%f+g"] {
            assert_eq!(shell_word(plain), plain);
        }
        assert_eq!(shell_word(b"two words"), b"\"two words\"");
        assert_eq!(shell_word(b"it's"), b"\"it's\"");
        assert_eq!(shell_word(b"a\"b\\c$d`e"), b"\"a\\\"b\\\\c\\$d\\`e\"");
        assert_eq!(shell_word(b"~home"), b"\"~home\"");
        assert_eq!(shell_word(b"#x;y&z|w"), b"\"#x;y&z|w\"");
        assert_eq!(shell_word(b"!bang"), b"\"!bang\"");
    }

    #[test]
    fn every_control_byte_is_escaped_and_every_other_byte_kept() {
        assert_eq!(
            quoted(b"--a\nb\t\"\\\x1b\xe9 *"),
            b"\"--a\\nb\\t\\\"\\\\\\033\xe9 *\""
        );
    }
}
