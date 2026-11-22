//! `init`'s words, and the lines it prints around Git's own.
//!
//! The words are read before anything else (`Composition/Front` step 2): a help request
//! anywhere before `--` is answered without locating, and a misuse is reported after the
//! locate run, so that outside a repository Git's own message ends the command.

use super::git_version;
use super::help;
use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use crate::attachment::{self, Initialized, NotInitialized};
use crate::runner::locate::Workspace;

/// What `init`'s words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked<'w> {
    /// `-h` or `--help` before `--`.
    Help,
    /// Attach, with the initial branch when one was given.
    Attach(Option<&'w [u8]>),
    /// Not `init [-b BRANCH | --initial-branch=BRANCH]`: the fault, which names nothing
    /// typed.
    Misused(&'static [u8]),
}

const AN_OPTION: &[u8] = b"'git dupe init' takes no option but -b or --initial-branch";
const NO_BRANCH: &[u8] = b"-b and --initial-branch need a branch name";
const AN_OPERAND: &[u8] =
    b"'git dupe init' takes no directory or other operand: it attaches the repository it is run in";

/// Reads `init [-b BRANCH | --initial-branch=BRANCH | --initial-branch BRANCH]`. A
/// branch option takes the next word as its value, whatever it is, as Git's `init` does,
/// and a later one replaces an earlier one; `--` ends the options, and nothing may follow.
pub fn read(words: &[Vec<u8>]) -> Asked<'_> {
    if help::requested(words) {
        return Asked::Help;
    }
    let mut branch = None;
    let mut words = words.iter();
    while let Some(word) = words.next() {
        let bytes = word.as_slice();
        if bytes == b"-b" || bytes == b"--initial-branch" {
            match words.next() {
                Some(value) => branch = Some(value.as_slice()),
                None => return Asked::Misused(NO_BRANCH),
            }
        } else if let Some(value) = bytes.strip_prefix(b"--initial-branch=") {
            branch = Some(value);
        } else if bytes == b"--" {
            if words.next().is_some() {
                return Asked::Misused(AN_OPERAND);
            }
        } else if bytes.starts_with(b"-") {
            return Asked::Misused(AN_OPTION);
        } else {
            return Asked::Misused(AN_OPERAND);
        }
    }
    Asked::Attach(branch)
}

/// The handler: names a Git older than F4's range first, then attaches the workspace, or
/// completes an attachment, and says so in one `hint:` where the workspace was attached
/// before the command. Settle follows in the sequence, whatever this returns.
pub fn run(workspace: &Workspace, branch: Option<&[u8]>) -> Outcome {
    git_version::warn_when_too_old();
    match attachment::init(workspace, branch) {
        Ok(initialized) => {
            if workspace.attached() {
                lines::write(Level::Hint, &completed(&initialized));
            }
            Outcome::Answered
        }
        Err(NotInitialized::Refused(refusal)) => outcome::refuse_to_attach(workspace, refusal),
        Err(NotInitialized::Unresolved(unresolved)) => outcome::unresolved(&unresolved),
        Err(NotInitialized::Unwritten(unwritten)) => {
            outcome::unwritten(&unwritten, b"'git dupe init' completes the attachment")
        }
        Err(NotInitialized::Failed(failed)) => outcome::failed(failed),
    }
}

/// The hint of an `init` in a workspace attached before it: which steps it completed.
fn completed(initialized: &Initialized) -> Vec<u8> {
    let done: Vec<&[u8]> = [
        (initialized.repository, &b"its private repository"[..]),
        (initialized.settings, b"its settings"),
        (
            initialized.attributes,
            b"its info/attributes line for .gitdupe",
        ),
    ]
    .into_iter()
    .filter_map(|(completed, what)| completed.then_some(what))
    .collect();
    let said: Vec<u8> = match done.as_slice() {
        [] => b"nothing was missing".to_vec(),
        [one] => [b"completed ", *one].concat(),
        [first, second] => [b"completed ", *first, b" and ", *second].concat(),
        [first, second, third] => {
            [b"completed ", *first, b", ", *second, b", and ", *third].concat()
        }
        _ => unreachable!("three steps at most"),
    };
    [&b"this workspace was already attached; "[..], &said].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> Vec<Vec<u8>> {
        words.iter().map(|word| word.as_bytes().to_vec()).collect()
    }

    fn branch(name: &str) -> Asked<'_> {
        Asked::Attach(Some(name.as_bytes()))
    }

    #[test]
    fn init_reads_one_branch_in_each_of_its_spellings() {
        assert_eq!(read(&words(&[])), Asked::Attach(None));
        assert_eq!(read(&words(&["--"])), Asked::Attach(None));
        assert_eq!(read(&words(&["-b", "other"])), branch("other"));
        assert_eq!(read(&words(&["--initial-branch=other"])), branch("other"));
        assert_eq!(
            read(&words(&["--initial-branch", "other"])),
            branch("other")
        );
        assert_eq!(
            read(&words(&["-b", "feature/x", "--"])),
            branch("feature/x")
        );
        assert_eq!(read(&words(&["--initial-branch="])), branch(""));
        // The next word is the value, whatever it is; the last branch given stands.
        assert_eq!(read(&words(&["-b", "--"])), branch("--"));
        assert_eq!(read(&words(&["-b", "x", "-b", "y"])), branch("y"));
        let not_utf8 = [b"-b".to_vec(), b"\xff".to_vec()];
        assert_eq!(read(&not_utf8), Asked::Attach(Some(&b"\xff"[..])));
    }

    #[test]
    fn any_other_word_is_a_misuse() {
        for (misuse, fault) in [
            (&["x"][..], AN_OPERAND),
            (&[""], AN_OPERAND),
            (&["-b", "x", "y"], AN_OPERAND),
            (&["--", "x"], AN_OPERAND),
            (&["-b", "--", "x"], AN_OPERAND),
            (&["--bogus"], AN_OPTION),
            (&["-q"], AN_OPTION),
            (&["-"], AN_OPTION),
            (&["-bother"], AN_OPTION),
            (&["--initial"], AN_OPTION),
            (&["--bare"], AN_OPTION),
            (&["--", "-h"], AN_OPERAND),
            (&["-b"], NO_BRANCH),
            (&["--initial-branch"], NO_BRANCH),
            (&["x", "-b"], AN_OPERAND),
        ] {
            assert_eq!(read(&words(misuse)), Asked::Misused(fault), "{misuse:?}");
        }
    }

    #[test]
    fn a_help_request_anywhere_before_the_end_of_options_decides() {
        for request in [
            &["-h"][..],
            &["--help"],
            &["-b", "x", "-h"],
            &["-b", "-h"],
            &["--bogus", "--help"],
            &["x", "-h", "--", "y"],
        ] {
            assert_eq!(read(&words(request)), Asked::Help, "{request:?}");
        }
    }

    #[test]
    fn the_hint_says_what_was_completed() {
        let hint = |repository, attributes, settings| {
            completed(&Initialized {
                repository,
                settings,
                attributes,
            })
        };
        let mut all = Vec::new();
        for repository in [false, true] {
            for attributes in [false, true] {
                for settings in [false, true] {
                    all.push(hint(repository, attributes, settings));
                }
            }
        }
        for (index, text) in all.iter().enumerate() {
            assert!(text.starts_with(b"this workspace was already attached; "));
            assert!(all[..index].iter().all(|earlier| earlier != text));
        }
        assert_eq!(
            hint(false, false, false),
            b"this workspace was already attached; nothing was missing"
        );
        assert_eq!(
            hint(false, true, true),
            b"this workspace was already attached; completed its settings and its \
              info/attributes line for .gitdupe"
        );
        assert_eq!(
            hint(true, true, true),
            b"this workspace was already attached; completed its private repository, its \
              settings, and its info/attributes line for .gitdupe"
        );
    }
}
