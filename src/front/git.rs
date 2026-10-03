//! `git dupe git WORDS`: Git with `WORDS` as typed, guarded by nothing (G20).
//!
//! Only the first word of `WORDS` is read (`Composition/Front`): `-h` or `--help` there is
//! the help request, and any other dashed first word is a Git global option after `dupe`,
//! the usage error F7 names, both answered without locating. Every later word is Git's to
//! read: `git dupe git log -h` is Git's usage of `log`, run in an attached workspace. No
//! `WORDS` at all is a misuse of the command's form, reported after locating.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

/// What `git dupe git`'s words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked<'w> {
    /// `-h` or `--help` as the first word.
    Help,
    /// `WORDS`, run unread.
    Run(&'w [OsString]),
    /// A dashed first word other than a help request.
    GlobalOption,
    /// No word.
    Empty,
}

/// The `error:` line of a dashed first word (F7).
pub const GLOBAL_OPTION: &[u8] = b"'git dupe git' takes a Git command or alias first; \
    Git's own options go before 'dupe': git <options> dupe git <command>";

/// The `error:` line of no words.
pub const EMPTY: &[u8] = b"'git dupe git' needs a Git command or alias to run";

/// Reads the words after `git`, the first of them alone.
pub fn read(words: &[OsString]) -> Asked<'_> {
    match words.first().map(|word| word.as_bytes()) {
        None => Asked::Empty,
        Some(b"-h" | b"--help") => Asked::Help,
        Some(first) if first.starts_with(b"-") => Asked::GlobalOption,
        Some(_) => Asked::Run(words),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_the_first_word_is_read() {
        assert_eq!(read(&[]), Asked::Empty);
        for help in [
            &["-h"][..],
            &["--help"],
            &["-h", "log"],
            &["--help", "--all"],
        ] {
            assert_eq!(read(&owned(help)), Asked::Help, "{help:?}");
        }
        for option in [
            &["-c", "a=b", "status"][..],
            &["--", "status"],
            &["-C", "x", "log"],
            &["--no-pager", "log"],
            &["-hx"],
            &["--help=x"],
            &["-"],
        ] {
            assert_eq!(read(&owned(option)), Asked::GlobalOption, "{option:?}");
        }
        for run in [
            &["status", "--porc"][..],
            &["log", "-h"],
            &["stash", "-u"],
            &["add", "-i"],
            &["log", "--", "-c"],
            &[""],
        ] {
            let words = owned(run);
            assert_eq!(read(&words), Asked::Run(&words), "{run:?}");
        }
    }
}
