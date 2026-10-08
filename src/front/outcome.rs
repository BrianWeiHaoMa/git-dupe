//! What a command ends with, and the one place that turns it into the exit status.
//!
//! A handler returns an `Outcome` and never ends the process. The functions below are
//! how it answers, refuses, and reports a usage error; each writes and returns.

use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::lines::{self, Level};
use crate::attachment::{Refusal, Unresolved};
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

/// The refusal of a command that requires an attached workspace, in a worktree that is
/// not attached, whatever the other worktrees hold (G4).
pub fn refuse_unattached() -> Outcome {
    refuse(b"no private repository is attached to this worktree; run 'git dupe init' here first")
}

/// The refusals of G4 where `init` and `clone` do not attach, each with its explanation.
pub fn refuse_to_attach(workspace: &Workspace, refusal: Refusal) -> Outcome {
    let line: Vec<u8> = match refusal {
        Refusal::SubmoduleCheckout => b"this is a submodule checkout; git-dupe attaches a \
              worktree of a project, never a submodule checkout"
            .to_vec(),
        Refusal::NotOwnGitDirectory => b"the .git entry at the root of this working tree \
              is not the directory that holds its repository; git-dupe attaches only such \
              a main worktree"
            .to_vec(),
        Refusal::NotUnderWorktrees => [
            &b"the Git directory of this linked worktree, "[..],
            workspace.git_directory().as_os_str().as_bytes(),
            b", is not directly in ",
            workspace
                .common_directory()
                .join("worktrees")
                .as_os_str()
                .as_bytes(),
            b", where 'git worktree add' makes one; git-dupe attaches only such a linked \
              worktree",
        ]
        .concat(),
        Refusal::Obstructed(path) => return obstructed(&path),
        Refusal::TracksGitdupe => b"this project tracks .gitdupe, the file that holds \
              git-dupe's hidden paths; git-dupe cannot be attached to it"
            .to_vec(),
    };
    refuse(&line)
}

/// Something that is not a directory standing where a private repository goes: a refusal
/// naming it, before anything was run or written through it or in its place.
pub fn obstructed(path: &Path) -> Outcome {
    let line = [
        path.as_os_str().as_bytes(),
        b" is not a directory, and a private repository goes there; git-dupe runs and \
          writes nothing through it: move it away first",
    ]
    .concat();
    refuse(&line)
}

/// A path the private repository's working tree is recorded from that has no canonical
/// form: a refusal naming it and the cause, before anything was written.
pub fn unresolved(unresolved: &Unresolved) -> Outcome {
    let line = [
        &b"cannot resolve "[..],
        unresolved.path.as_os_str().as_bytes(),
        b" to record the private repository's working tree: ",
        unresolved.cause.to_string().as_bytes(),
    ]
    .concat();
    refuse(&line)
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
