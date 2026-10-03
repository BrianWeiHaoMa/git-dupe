//! What a command ends with, and the one place that turns it into the exit status.
//!
//! A handler returns an `Outcome` and never ends the process. The functions below are
//! how it answers, refuses, and reports a usage error; each writes and returns.

use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;

use super::lines::{self, Level};
use crate::attachment::Refusal;
use crate::keeper::Failed;
use crate::runner::locate::Workspace;
use crate::runner::{End, Failure};

/// Exit statuses are Git's.
pub enum Outcome {
    /// Requested output was printed, or an own command did what it says.
    Answered,
    /// Bare `git dupe` printed its usage, as bare `git` does.
    BareUsage,
    /// A refusal of git-dupe's own, after its `fatal:` line.
    Refused,
    /// A usage error of git-dupe's own, after its `error:` line and the usage line.
    UsageError,
    /// A Git child's end, unchanged.
    Git(End),
}

impl Outcome {
    pub fn status(&self) -> u8 {
        match self {
            Outcome::Answered => 0,
            Outcome::BareUsage => 1,
            Outcome::Refused => 128,
            Outcome::UsageError => 129,
            Outcome::Git(End::Code(code)) => *code,
            // A Git child killed by a signal ends git-dupe with 128 plus its number.
            Outcome::Git(End::Signal(signal)) => {
                128u8.saturating_add(u8::try_from(*signal).unwrap_or(u8::MAX))
            }
        }
    }
}

/// Prints requested output of git-dupe's own on standard output and ends as `printed`.
/// Output that cannot be written is a refusal naming the cause.
pub fn print(text: &[u8], printed: Outcome) -> Outcome {
    let mut out = io::stdout().lock();
    match out.write_all(text).and_then(|()| out.flush()) {
        Ok(()) => printed,
        Err(cause) => refuse(format!("cannot write to standard output: {cause}").as_bytes()),
    }
}

/// One `fatal:` line.
pub fn refuse(text: &[u8]) -> Outcome {
    lines::write(Level::Fatal, text);
    Outcome::Refused
}

/// One `error:` line, then the usage line of the command misused.
pub fn usage_error(fault: &[u8], usage_line: &[u8]) -> Outcome {
    lines::write_usage_error(fault, usage_line);
    Outcome::UsageError
}

/// The refusal in a linked worktree, which has no private repository and is never
/// attached: it names the main working tree where Git's `worktree list` does, the common
/// Git directory's parent when that directory is named `.git`.
pub fn refuse_in_linked_worktree(workspace: &Workspace) -> Outcome {
    let mut line = b"a linked worktree has no private repository; use 'git dupe init' \
                     and git-dupe in the main working tree"
        .to_vec();
    if let Some(main) = workspace.main_working_tree_of_linked() {
        line.extend_from_slice(b": ");
        line.extend_from_slice(main.as_os_str().as_bytes());
    }
    refuse(&line)
}

/// The refusals of G4 where `init` and `clone` do not attach, each with its explanation;
/// in a linked worktree, naming the main working tree where Git knows it.
pub fn refuse_to_attach(workspace: &Workspace, refusal: Refusal) -> Outcome {
    let line: &[u8] = match refusal {
        Refusal::LinkedWorktree => return refuse_in_linked_worktree(workspace),
        Refusal::SubmoduleCheckout => {
            b"this is a submodule checkout; git-dupe attaches only the main working tree \
              of an ordinary repository"
        }
        Refusal::NotOwnGitDirectory => {
            b"the .git entry at the root of this working tree is not the directory that \
              holds its repository; git-dupe attaches only such a working tree"
        }
        Refusal::TracksGitdupe => {
            b"this project tracks .gitdupe, the file that holds git-dupe's hidden paths; \
              git-dupe cannot be attached to it"
        }
    };
    refuse(line)
}

/// A list of paths that does not fit on one Git command line is a refusal naming how
/// many paths it held, because git-dupe never shortens one (G23).
pub fn list_too_long(count: usize) -> Outcome {
    let line = format!(
        "a list of {count} paths does not fit on one command line for Git, and git-dupe \
         never shortens one"
    );
    refuse(line.as_bytes())
}

/// A Git run that did not start is a refusal, and nothing ran. A run that carries a list
/// of paths names its count instead, through `list_too_long`.
pub fn not_started(failure: Failure) -> Outcome {
    match failure {
        Failure::TooLong => refuse(b"the command line for Git is too long to run"),
        Failure::Other(cause) => refuse(format!("cannot run git: {cause}").as_bytes()),
    }
}

/// A Git run whose answer the command needed and did not get: Git's own end, its message
/// already on standard error; a list that did not fit, naming its count; a run that did
/// not start; or an answer that cannot be read, which is never taken for an empty one.
pub fn failed(failed: Failed) -> Outcome {
    match failed {
        Failed::Exited(end) => Outcome::Git(end),
        Failed::TooLong(count) => list_too_long(count),
        Failed::NotStarted(failure) => not_started(failure),
        Failed::Unreadable => unreadable(),
    }
}

/// A Git answer the command needed that cannot be read as its form: a refusal, never
/// taken for an empty answer.
pub fn unreadable() -> Outcome {
    refuse(b"cannot read Git's answer to what git-dupe asked it")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_are_gits() {
        assert_eq!(Outcome::Answered.status(), 0);
        assert_eq!(Outcome::BareUsage.status(), 1);
        assert_eq!(Outcome::Refused.status(), 128);
        assert_eq!(Outcome::UsageError.status(), 129);
        assert_eq!(Outcome::Git(End::Code(16)).status(), 16);
        assert_eq!(Outcome::Git(End::Signal(15)).status(), 143);
    }
}
