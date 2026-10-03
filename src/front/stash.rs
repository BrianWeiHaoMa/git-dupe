//! `stash`'s words as Git reads them, for its guard (G17, `Holds/G17`).
//!
//! The form decides what is read: a first word `push` or `save` is that form; no word,
//! or a dashed first word, is the bare form, read as `push`; any other first word is
//! another subcommand, whose words are not read and run unchanged. In the three forms the
//! words are read left to right up to an end of options, skipping the word Git takes as
//! the separate value of `-m`, of `--message`, or of `--pathspec-from-file`, so that a
//! message or a file named `-u` is never read as an option and an option after one is.
//! A help request changes nothing here, because Git may read `-h` as a value.

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use crate::guards::stash::untracked;

/// The refusal of an untracked form: `git dupe add` first, then `git dupe stash`.
pub const REFUSAL: &[u8] = b"-u, -a, and their long forms would stash, and remove from disk, \
    every file the private repository does not track, the project's own included; \
    run 'git dupe add' on what to keep first, then 'git dupe stash'";

/// Whether `git dupe stash` with `words`, those after `stash`, is refused (G17).
pub fn refused(words: &[OsString]) -> bool {
    options(words).into_iter().any(untracked)
}

/// The words Git reads as options of the push, save, or bare form, in order, or none for
/// another subcommand: every word up to an end of options — `--` or `--end-of-options`,
/// where Git's option parser stops alike — skipping the separate value of an option that
/// takes one, even when that value is one of those two.
fn options(words: &[OsString]) -> Vec<&[u8]> {
    let words: Vec<&[u8]> = words.iter().map(|word| word.as_bytes()).collect();
    let form = match words.first() {
        None => &[][..],
        Some(&(b"push" | b"save")) => &words[1..],
        Some(first) if first.starts_with(b"-") => &words[..],
        Some(_) => &[][..],
    };
    let mut read = Vec::new();
    let mut words = form.iter();
    while let Some(&word) = words.next() {
        if word == b"--" || word == b"--end-of-options" {
            break;
        }
        read.push(word);
        if takes_the_next_word(word) {
            words.next();
        }
    }
    read
}

/// Whether Git takes the next word as this option's value: `-m` alone or ending a bundle;
/// `--message` or a prefix of it of at least three bytes; `--pathspec-from-file` or a
/// prefix of it at least as long as `--pathspec-fr`, a shorter one being ambiguous with
/// `--pathspec-file-nul`, which Git refuses.
fn takes_the_next_word(option: &[u8]) -> bool {
    if let Some(long) = option.strip_prefix(b"--") {
        let message = !long.is_empty() && b"message".starts_with(long);
        let pathspec_file =
            long.len() >= b"pathspec-fr".len() && b"pathspec-from-file".starts_with(long);
        return message || pathspec_file;
    }
    match option.strip_prefix(b"-") {
        Some(letters) => letters
            .iter()
            .position(|&letter| letter == b'm')
            .is_some_and(|at| at + 1 == letters.len()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refuses(words: &[&str]) -> bool {
        refused(&words.iter().map(OsString::from).collect::<Vec<_>>())
    }

    /// The forms of `stash` that take an untracked file, under every listed release.
    #[test]
    fn the_untracked_forms_are_refused() {
        for words in [
            &["-u"][..],
            &["-a"],
            &["push", "-a"],
            &["push", "-u"],
            &["save", "--include-untracked"],
            &["-ku"],
            &["--inc"],
            &["--al"],
            &["-um", "msg"],
            &["save", "msg", "-u"],
            &["--mess", "msg", "-u"],
            &["push", "-m", "msg", "--all"],
            &["push", "--pathspec-from-file", "x", "-u"],
            &["push", "--pathspec-from-file=x", "-u"],
            &["push", "notes", "-u"],
            &["-u", "-h"],
            &["-h", "-u"],
            &["push", "-m", "--", "-u"],
            &["push", "--pathspec-f", "-u"],
            &["push", "-m", "--end-of-options", "-u"],
            &["--message", "--end-of-options", "-a"],
            &["push", "--end-of-option", "-u"],
            &["push", "--end-of-options=x", "-u"],
        ] {
            assert!(refuses(words), "{words:?}");
        }
    }

    /// The forms that take none: a value, a negation, the end of options, another
    /// subcommand.
    #[test]
    fn every_other_form_runs() {
        for words in [
            &[][..],
            &["push", "-m", "u"],
            &["-m", "-u"],
            &["-qm", "-u"],
            &["--mes", "-u"],
            &["--message", "--all"],
            &["--message=-u"],
            &["-mu"],
            &["--", "-u"],
            &["push", "--", "-a"],
            &["push", "--no-include-untracked"],
            &["--no-all"],
            &["list"],
            &["show", "-u"],
            &["pop"],
            &["apply", "-a"],
            &["--pathspec-from-file", "-u"],
            &["--pathspec-fr", "-u"],
            &["-h"],
            &["-"],
            &["push", "-k"],
            &["save", "--end-of-options", "-u"],
            &["push", "--end-of-options", "--all"],
            &["--end-of-options", "-a"],
            &["-k", "--end-of-options", "-u"],
            &["push", "--end-of-options", "--", "-u"],
        ] {
            assert!(!refuses(words), "{words:?}");
        }
    }
}
