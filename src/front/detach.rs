//! `detach`'s words, its handler, and the lines it prints.
//!
//! The words are read before anything else (`Composition/Front` step 2): a help request
//! anywhere before `--` is answered without locating, and a misuse is reported after the
//! locate run and the unattached refusal. `detach` is no command G6 covers: the sequence
//! gives it neither the starting region of step (6) nor the settle of step (8), whatever
//! it returns (R3).

use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::lines::{self, Level};
use super::outcome::{self, Outcome};
use crate::attachment::{self, NotDetached, NotRemoved};
use crate::keeper::NotDeleted;
use crate::runner::locate::Workspace;

/// What `detach`'s words ask for.
#[derive(Debug, PartialEq, Eq)]
pub enum Asked {
    /// `-h` or `--help` before `--`.
    Help,
    /// Detach, with `--force` or without.
    Detach { force: bool },
    /// Not `detach [--force]`: the fault, which names nothing typed.
    Misused(&'static [u8]),
}

const AN_OPTION: &[u8] = b"'git dupe detach' takes no option but --force";
const AN_OPERAND: &[u8] =
    b"'git dupe detach' takes no operand: it detaches the repository it is run in";

/// Reads `detach [--force]`: `--force` as written, given once or more, and nothing else;
/// `--` ends the options, and nothing may follow.
pub fn read(words: &[OsString]) -> Asked {
    let end_of_options = words
        .iter()
        .position(|word| word.as_bytes() == b"--")
        .unwrap_or(words.len());
    if words[..end_of_options]
        .iter()
        .any(|word| matches!(word.as_bytes(), b"-h" | b"--help"))
    {
        return Asked::Help;
    }
    let (options, after) = words.split_at(end_of_options);
    if after.len() > 1 {
        return Asked::Misused(AN_OPERAND);
    }
    let mut force = false;
    for word in options {
        match word.as_bytes() {
            b"--force" => force = true,
            option if option.starts_with(b"-") => return Asked::Misused(AN_OPTION),
            _ => return Asked::Misused(AN_OPERAND),
        }
    }
    Asked::Detach { force }
}

/// The handler: deletes the region, writes the warnings while the private repository
/// still stands, then removes it, and ends with one `hint:` counting the paths the
/// warnings named as now visible, where they named any.
pub fn run(workspace: &Workspace, force: bool) -> Outcome {
    let leaving = match attachment::detach(workspace, force) {
        Ok(leaving) => leaving,
        Err(not_detached) => return refuse(not_detached),
    };
    if let Some(failed) = &leaving.unlisted {
        lines::write(
            Level::Warning,
            &[
                b"cannot list the private repository (",
                &failed.cause()[..],
                b"); the paths asked about are .gitdupe, the paths it lists, and those of \
                  the managed region",
            ]
            .concat(),
        );
    }
    for warning in &leaving.removed.warnings {
        lines::write(Level::Warning, warning);
    }
    let visible = leaving.removed.visible;
    match leaving.remove() {
        Ok(()) => {
            if visible > 0 {
                lines::write(Level::Hint, &now_visible(visible));
            }
            Outcome::Answered
        }
        Err(NotRemoved::TooLong(count)) => outcome::list_too_long(count),
        Err(NotRemoved::Killed(failed)) => outcome::failed(failed),
        Err(NotRemoved::Failed(path, cause)) => outcome::refuse(
            &[
                b"cannot remove ",
                path.as_os_str().as_bytes(),
                b": ",
                cause.to_string().as_bytes(),
                b"; the managed region is deleted; run 'git dupe detach --force' to finish",
            ]
            .concat(),
        ),
    }
}

/// The closing `hint:`: how many formerly hidden paths the warnings above named as
/// visible, which a public `git add -A` would now stage. It counts only what was named: a
/// path the question could not answer for may be visible too.
fn now_visible(count: usize) -> Vec<u8> {
    let (paths, are, them) = match count {
        1 => ("path", "is", "it"),
        _ => ("paths", "are", "them"),
    };
    format!(
        "{count} formerly hidden {paths} named above {are} now visible to public Git; \
         check 'git status' before 'git add -A' stages {them}"
    )
    .into_bytes()
}

fn refuse(not_detached: NotDetached) -> Outcome {
    let line = match not_detached {
        NotDetached::RegionBeyondALink(link) => beyond_a_link(&link),
        NotDetached::Uncommitted => b"the private repository has changes not committed, \
            which detaching would lose; commit them, or run 'git dupe detach --force' to \
            remove them with it"
            .to_vec(),
        NotDetached::Unpushed {
            remote_configured: true,
        } => b"a private commit is on no remote-tracking branch, and detaching would lose \
            it; push it, or run 'git dupe detach --force' to remove it with the private \
            repository"
            .to_vec(),
        NotDetached::Unpushed {
            remote_configured: false,
        } => b"no remote is configured to keep the private commits, and detaching would \
            lose them; push them to a private remote, or run 'git dupe detach --force' to \
            remove them with the private repository"
            .to_vec(),
        NotDetached::Unreadable(failed) => [
            b"the private repository cannot be read (",
            &failed.cause()[..],
            b"); run 'git dupe init' to complete what an interrupted init left, or \
              'git dupe detach --force' to remove it",
        ]
        .concat(),
        NotDetached::Killed(failed) => return outcome::failed(failed),
        NotDetached::NotDeleted(NotDeleted::BeyondALink(link)) => beyond_a_link(&link),
        NotDetached::NotDeleted(NotDeleted::Failed { file, cause }) => [
            b"cannot delete the managed region of ",
            file.as_os_str().as_bytes(),
            b": ",
            cause.to_string().as_bytes(),
            b"; nothing is detached",
        ]
        .concat(),
    };
    outcome::refuse(&line)
}

/// The refusal while the region stands beyond a link at `.git/info` (G3).
fn beyond_a_link(link: &Path) -> Vec<u8> {
    [
        link.as_os_str().as_bytes(),
        b" is a symbolic link, and the managed region in the file read through it cannot \
          be deleted there; make .git/info a directory again, then detach",
    ]
    .concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn detach_reads_force_as_written_and_nothing_else() {
        let detach = |force| Asked::Detach { force };
        assert_eq!(read(&words(&[])), detach(false));
        assert_eq!(read(&words(&["--"])), detach(false));
        assert_eq!(read(&words(&["--force"])), detach(true));
        assert_eq!(read(&words(&["--force", "--"])), detach(true));
        assert_eq!(read(&words(&["--force", "--force"])), detach(true));
    }

    #[test]
    fn any_other_word_is_a_misuse() {
        for (misuse, fault) in [
            (&["x"][..], AN_OPERAND),
            (&[""], AN_OPERAND),
            (&["--force", "x"], AN_OPERAND),
            (&["--", "x"], AN_OPERAND),
            (&["--", "--force"], AN_OPERAND),
            (&["--", "-h"], AN_OPERAND),
            (&["-f"], AN_OPTION),
            (&["--forc"], AN_OPTION),
            (&["--force=yes"], AN_OPTION),
            (&["--no-force"], AN_OPTION),
            (&["-"], AN_OPTION),
            (&["x", "--force"], AN_OPERAND),
        ] {
            assert_eq!(read(&words(misuse)), Asked::Misused(fault), "{misuse:?}");
        }
    }

    #[test]
    fn a_help_request_anywhere_before_the_end_of_options_decides() {
        for request in [
            &["-h"][..],
            &["--help"],
            &["--force", "--help"],
            &["x", "-h"],
            &["-f", "-h", "--", "y"],
        ] {
            assert_eq!(read(&words(request)), Asked::Help, "{request:?}");
        }
    }
}
