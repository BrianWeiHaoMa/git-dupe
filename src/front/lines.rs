//! The one writer of git-dupe's own lines: `level: text` on standard error, uncolored,
//! with no program-name prefix. The text is bytes, because the paths and words a line
//! names are.

use std::io::{self, Write};

use crate::guards::quoted::quoted;

pub enum Level {
    /// A refusal.
    Fatal,
    /// A usage error.
    Error,
    /// Something worth knowing after a command that ran; it never changes the status.
    Warning,
    /// What git-dupe did, or what to do next; it never changes the status.
    Hint,
}

impl Level {
    fn name(&self) -> &'static [u8] {
        match self {
            Level::Fatal => b"fatal",
            Level::Error => b"error",
            Level::Warning => b"warning",
            Level::Hint => b"hint",
        }
    }
}

pub fn write(level: Level, text: &[u8]) {
    to_standard_error(&line(level, text));
}

/// A usage error: its `error:` line, then the usage line of the command misused.
pub fn write_usage_error(fault: &[u8], usage_line: &[u8]) {
    to_standard_error(&[&line(Level::Error, fault), usage_line].concat());
}

/// Git's own message, captured from a run whose failure ends the command: the bytes as
/// Git wrote them, with nothing of git-dupe's beside them.
pub fn relay(message: &[u8]) {
    to_standard_error(message);
}

/// A word the user typed, as a line names it: between single quotes, as typed; or, when
/// it holds a newline, which no one line can hold, quoted as Git quotes a path that needs
/// it (`guards::quoted`).
pub fn typed(word: &[u8]) -> Vec<u8> {
    if !word.contains(&b'\n') {
        return [b"'", word, b"'"].concat();
    }
    quoted(word)
}

fn to_standard_error(bytes: &[u8]) {
    // One write, so the lines arrive whole. A failed write is dropped: standard error is
    // where a failure would be reported.
    if let Some(refused) = written(&mut io::stderr(), bytes) {
        let _ = io::stderr().write_all(&lossy(refused));
    }
}

/// Writes `bytes`, and returns what is left of them when `out` refuses them as `InvalidData`:
/// a Windows console takes UTF-8 alone, and writes what comes before the first byte that is
/// not (S20), so that this is what the line owes. Any other failure ends the writing, and
/// only this one is tried again, so that a closed stream neither loops nor doubles a line.
fn written<'b>(out: &mut impl Write, mut bytes: &'b [u8]) -> Option<&'b [u8]> {
    while !bytes.is_empty() {
        match out.write(bytes) {
            Ok(0) => return None,
            Ok(count) => bytes = &bytes[count..],
            Err(cause) if cause.kind() == io::ErrorKind::Interrupted => {}
            Err(cause) if cause.kind() == io::ErrorKind::InvalidData => return Some(bytes),
            Err(_) => return None,
        }
    }
    None
}

/// `bytes` with each byte that is not UTF-8 replaced.
fn lossy(bytes: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(bytes).into_owned().into_bytes()
}

fn line(level: Level, text: &[u8]) -> Vec<u8> {
    [level.name(), b": ", text, b"\n"].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_is_its_level_then_the_text_as_bytes() {
        assert_eq!(
            line(Level::Fatal, b"no \xff\xfe here"),
            b"fatal: no \xff\xfe here\n"
        );
        assert_eq!(line(Level::Error, b"misused"), b"error: misused\n");
        assert_eq!(
            line(Level::Warning, b"notes\r is \xe9"),
            b"warning: notes\r is \xe9\n"
        );
        assert_eq!(line(Level::Hint, b"done"), b"hint: done\n");
    }

    /// A console as S20 reads it: it takes what comes before a byte that is not UTF-8, and
    /// refuses that byte.
    #[derive(Default)]
    struct Console(Vec<u8>);

    impl Write for Console {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            match std::str::from_utf8(bytes) {
                Ok(_) => self.0.extend_from_slice(bytes),
                Err(fault) if fault.valid_up_to() == 0 => {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                Err(fault) => {
                    self.0.extend_from_slice(&bytes[..fault.valid_up_to()]);
                    return Ok(fault.valid_up_to());
                }
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_console_that_refuses_a_byte_gets_the_rest_of_the_line_lossily_and_none_of_it_twice() {
        let mut console = Console::default();
        let refused = written(&mut console, b"warning: caf\xe9 is\n");
        assert_eq!(refused, Some(&b"\xe9 is\n"[..]));
        console
            .write_all(&lossy(refused.expect("refused")))
            .unwrap();
        assert_eq!(console.0, "warning: caf\u{fffd} is\n".as_bytes());
    }

    #[test]
    fn valid_bytes_are_written_whole_and_any_other_failure_is_dropped() {
        let mut console = Console::default();
        assert_eq!(written(&mut console, "caf\u{e9}\n".as_bytes()), None);
        assert_eq!(console.0, "caf\u{e9}\n".as_bytes());

        struct Closed(usize);
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                self.0 += 1;
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut closed = Closed(0);
        assert_eq!(written(&mut closed, b"caf\xe9\n"), None);
        assert_eq!(closed.0, 1);
    }

    #[test]
    fn each_byte_that_is_not_utf8_is_replaced() {
        assert_eq!(lossy(b"caf\xe9"), "caf\u{fffd}".as_bytes());
    }

    #[test]
    fn a_typed_word_is_named_as_typed_unless_a_newline_needs_gits_quoting() {
        assert_eq!(typed(b"caf\xe9 *\r\x1b"), b"'caf\xe9 *\r\x1b'");
        assert_eq!(
            typed(b"--a\nb\t\"\\\x1b\xe9"),
            b"\"--a\\nb\\t\\\"\\\\\\033\xe9\""
        );
    }
}
