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
    let _ = io::stderr().write_all(bytes);
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

    #[test]
    fn a_typed_word_is_named_as_typed_unless_a_newline_needs_gits_quoting() {
        assert_eq!(typed(b"caf\xe9 *\r\x1b"), b"'caf\xe9 *\r\x1b'");
        assert_eq!(
            typed(b"--a\nb\t\"\\\x1b\xe9"),
            b"\"--a\\nb\\t\\\"\\\\\\033\xe9\""
        );
    }
}
