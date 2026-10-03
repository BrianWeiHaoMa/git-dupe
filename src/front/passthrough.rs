//! Passthrough: a command git-dupe neither adds nor changes, run as Git itself (G19).
//!
//! What the code below cannot show:
//!
//! - The run is its words after `-c help.autocorrect=0` and nothing else of git-dupe's,
//!   from the user's directory with every stream inherited, so that output, color,
//!   pager, editor, hooks, prompts, and the exit status are Git's own (G19, F9). No word
//!   of it is read here. Its paths are the user's, so the pathspec variables of a global
//!   option before `dupe` reach it. A command an alias chain reached also carries the
//!   chain's prefix, which the runner puts before every run once the chain is dispatched.
//! - `Passed` is every command that runs so: a word F6 does not name that step (4) passes
//!   through, its words as received and no prefix (`alias`); `stash` once its guard has
//!   passed (G17); `push`, `pull`, `fetch`, and `remote` once nothing they would reach
//!   names a public place (G18, `transfer`); and `git dupe git`'s words, guarded by
//!   nothing (G20). Settle is not their business: each returns into the sequence, which
//!   settles. Of git-dupe's own lines only the guard's may follow the run, before
//!   settle's warnings: the hint of a failed `push` whose configuration chooses no
//!   remote (`transfer`).
//! - Where no workspace is attached, a help request after a word passed through, after
//!   `stash`, or after one of the four is the only thing that runs, unread, after the
//!   same guard (`Holds/G24`). The four's guard compares there too, from the facts of
//!   where the command stands and against the repository the run would be made against,
//!   because that run can reach a private repository from inside its Git directory;
//!   outside any repository there are no facts and no public place, and the words run at
//!   once (`Holds/G18`).

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;

use super::outcome::{self, Outcome};
use super::stash;
use super::transfer;
use crate::runner::locate::Facts;
use crate::runner::{Against, Run};

/// A command Git runs as itself once its guard has passed, with the words its run is
/// made of.
#[derive(Clone, Copy)]
pub enum Passed<'w> {
    /// A word F6 does not name that passes through, then the words after it, as received.
    Word(&'w [OsString]),
    /// `stash`, then the words after it, as received.
    Stash(&'w [OsString]),
    /// `push`, `pull`, `fetch`, or `remote`, then the words after it, as received.
    Transfer(&'w [OsString]),
    /// `WORDS` of `git dupe git WORDS`.
    Git(&'w [OsString]),
}

impl Passed<'_> {
    /// Its guard, then its run against the repository `against` names. `facts` gives
    /// where the command stands, for a guard that compares destinations, and nothing
    /// outside any repository; it is asked only by such a guard, which also asks the
    /// repository `against` names for its remotes where that is a private one.
    pub fn run(self, against: Against, facts: impl FnOnce() -> Option<Facts>) -> Outcome {
        match self {
            Passed::Stash(words) if stash::refused(&words[1..]) => outcome::refuse(stash::REFUSAL),
            Passed::Transfer(words) => {
                match facts().map(|facts| transfer::guard(words, &facts, against)) {
                    Some(Err(ended)) => ended,
                    Some(Ok(then)) => {
                        let ended = run(against, words);
                        transfer::after(then, &ended);
                        ended
                    }
                    None => run(against, words),
                }
            }
            Passed::Word(words) | Passed::Stash(words) | Passed::Git(words) => run(against, words),
        }
    }

    /// Whether its words ask for help where no workspace is attached: `-h` or `--help` as
    /// a whole word before any `--` after a word passed through or after `stash`; and
    /// anywhere among the words of one of the four guarded by `transfer`, because Git may
    /// take a `--` there as an option's value, so that `push -o -- -h` asks Git for its
    /// usage (`Holds/G18`). Never after `git`, whose own form reads only its first word
    /// (`Composition/Front`).
    pub fn asks_for_help(self) -> bool {
        let help = |word: &OsString| matches!(word.as_bytes(), b"-h" | b"--help");
        match self {
            Passed::Word(words) | Passed::Stash(words) => words[1..]
                .iter()
                .take_while(|word| word.as_bytes() != b"--")
                .any(help),
            Passed::Transfer(words) => words[1..].iter().any(help),
            Passed::Git(_) => false,
        }
    }
}

/// The run of `words` as Git itself.
fn run(against: Against, words: &[OsString]) -> Outcome {
    match Run::against(against, words).users_command().start() {
        Ok(finished) => Outcome::Git(finished.end),
        Err(failure) => outcome::not_started(failure),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn a_help_request_is_a_whole_word_before_the_end_of_options() {
        for asking in [
            &["log", "-h"][..],
            &["log", "--help"],
            &["log", "--oneline", "-h", "--", "x"],
            &["nosuch", "-h"],
        ] {
            let asking = words(asking);
            assert!(Passed::Word(&asking).asks_for_help(), "{asking:?}");
            assert!(Passed::Stash(&asking).asks_for_help(), "{asking:?}");
            assert!(Passed::Transfer(&asking).asks_for_help(), "{asking:?}");
        }
        for not_asking in [
            &["log"][..],
            &["log", "-hx"],
            &["log", "--he"],
            &["-h"],
            &["log", "--help=x"],
        ] {
            let not_asking = words(not_asking);
            assert!(!Passed::Word(&not_asking).asks_for_help(), "{not_asking:?}");
            assert!(
                !Passed::Stash(&not_asking).asks_for_help(),
                "{not_asking:?}"
            );
            assert!(
                !Passed::Transfer(&not_asking).asks_for_help(),
                "{not_asking:?}"
            );
        }
        // After a `--`, a transfer command still asks: Git may take the `--` as a value.
        for after_the_end in [&["log", "--", "-h"][..], &["push", "-o", "--", "--help"]] {
            let after_the_end = words(after_the_end);
            assert!(!Passed::Word(&after_the_end).asks_for_help());
            assert!(!Passed::Stash(&after_the_end).asks_for_help());
            assert!(Passed::Transfer(&after_the_end).asks_for_help());
        }
        let git = words(&["log", "-h"]);
        assert!(!Passed::Git(&git).asks_for_help());
    }
}
