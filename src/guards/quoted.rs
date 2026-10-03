//! A name as Git quotes a path that needs it, so that a line naming it stays one line
//! whatever it holds (F8): between double quotes, `"` and `\` escaped, and each control
//! byte written as Git's C-style escape. Git's lines name such a path the same way.
//! git-dupe's lines use it for a name holding a newline, which E4 keeps out of the
//! workspace's paths but not out of a word the user types or a file Git reads rules from.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_control_byte_is_escaped_and_every_other_byte_kept() {
        assert_eq!(
            quoted(b"--a\nb\t\"\\\x1b\xe9 *"),
            b"\"--a\\nb\\t\\\"\\\\\\033\xe9 *\""
        );
    }
}
