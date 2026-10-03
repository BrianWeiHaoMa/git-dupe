//! `detach` after its words: its refusals, the region's deletion, and the removal of the
//! private repository, in the order `State` "`detach`" fixes (G3, `Holds/G3`).
//!
//! 1. A region that stands beyond a symbolic link at `.git/info` refuses, with or without
//!    `--force`, before any run, because no write can delete it there.
//! 2. Without `--force`, the private runs decide, each from the root, its standard error
//!    Git's and its output captured: `--no-optional-locks status`, so that the private
//!    index is left as it was (S8); `stash list`; `config`, whose exit status alone says
//!    whether a remote is configured; and `rev-list`, fed the stash hashes byte for byte,
//!    with `--not --remotes` only where one is. Whether an answer is empty is all that is
//!    read of it. A run that fails, or does not start, is a private repository that
//!    cannot be read, and refuses; one killed by a signal ends the command with 128 plus
//!    its number. Nothing has changed.
//! 3. The hidden paths: `.gitdupe` and the private listing. Under `--force` a listing that
//!    fails other than by a signal leaves them to the keeper's `remove`, from `.gitdupe` on
//!    disk and the region; without it, it refuses as step 2 does.
//! 4. and 5. The keeper's `remove` deletes the region and asks what public Git now sees.
//! 6. The caller writes the warnings, while the private index still stands, so that a kill
//!    in the removal cannot leave the privately tracked files named nowhere.
//! 7. `Leaving::remove` removes `.git/dupe`, the one deletion git-dupe makes of its own
//!    (R8). A region that could not be deleted keeps it, and so do a question whose list
//!    did not fit and a question killed by a signal, which ends the command with 128 plus
//!    its number: attached, its region deleted, as a kill after step 4 leaves it.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::PathBuf;

use super::step;
use crate::keeper::{self, Answered, Failed, NotDeleted};
use crate::runner::locate::Workspace;
use crate::runner::{End, Run};

/// Why `detach` left the workspace attached.
pub enum NotDetached {
    /// `.git/info` is a symbolic link, at this path, and the file read through it holds
    /// the region. Nothing ran.
    RegionBeyondALink(PathBuf),
    /// Git's status of the private repository reports a change not committed.
    Uncommitted,
    /// A commit on a private branch, tag, or stash entry is on no remote-tracking branch;
    /// with no remote configured, a commit at all.
    Unpushed { remote_configured: bool },
    /// A run that decides, or the private listing, failed: the private repository cannot
    /// be read. Git's message, where it gave one, is already on standard error.
    Unreadable(Failed),
    /// A run was killed by a signal, and the command ends as it did.
    Killed(Failed),
    /// The region could not be deleted, and stands as it was.
    NotDeleted(NotDeleted),
}

/// A workspace whose region is deleted and whose private repository still stands.
pub struct Leaving {
    /// Under `--force`, why the private repository could not be listed: the hidden paths
    /// asked about were then `.gitdupe`, the paths it lists, and the region's.
    pub unlisted: Option<Failed>,
    /// The region's deletion and the exposure question: the warnings, in order, how many
    /// of them name a path as now visible, and the count of a list that did not fit (G23).
    pub removed: Answered,
    private_directory: PathBuf,
}

/// Why `.git/dupe` still stands after the region was deleted.
pub enum NotRemoved {
    /// The exposure question's list of this many paths did not fit on one command line:
    /// nothing is removed while what public Git sees is unknown.
    TooLong(usize),
    /// The exposure question's run was killed by a signal, and the command ends as it did.
    Killed(Failed),
    /// The removal failed at this path, part of the private repository perhaps gone.
    Failed(PathBuf, io::Error),
}

/// Steps 1 to 5 of `detach`, with or without `force`.
pub fn detach(workspace: &Workspace, force: bool) -> Result<Leaving, NotDetached> {
    if let Some(link) = keeper::region_beyond_a_link(workspace) {
        return Err(NotDetached::RegionBeyondALink(link));
    }
    if !force {
        decide(workspace)?;
    }
    let (hidden, unlisted) = match keeper::hidden(workspace) {
        Ok(hidden) => (Some(hidden), None),
        Err(failed @ Failed::Exited(End::Signal(_))) => return Err(NotDetached::Killed(failed)),
        Err(failed) if force => (None, Some(failed)),
        Err(failed) => return Err(NotDetached::Unreadable(failed)),
    };
    let removed = keeper::remove(workspace, hidden.as_ref().map(|hidden| &hidden.paths))
        .map_err(NotDetached::NotDeleted)?;
    Ok(Leaving {
        unlisted,
        removed,
        private_directory: workspace.private_directory(),
    })
}

impl Leaving {
    /// Step 7: removes `.git/dupe` recursively, once the warnings are written.
    pub fn remove(self) -> Result<(), NotRemoved> {
        if let Some(count) = self.removed.refused {
            return Err(NotRemoved::TooLong(count));
        }
        if let Some(killed) = self.removed.killed {
            return Err(NotRemoved::Killed(killed));
        }
        fs::remove_dir_all(&self.private_directory)
            .map_err(|cause| NotRemoved::Failed(self.private_directory, cause))
    }
}

/// Step 2: whether the private repository holds work that exists nowhere else.
fn decide(workspace: &Workspace) -> Result<(), NotDetached> {
    let private = workspace.private_directory();
    let root = workspace.root();
    let run = |words: &[&str]| {
        Run::private(&private, root, words.iter().copied())
            .from(root)
            .capture_output()
    };
    let no_optional_locks = [OsString::from("--no-optional-locks")];

    let status =
        run(&["status", "--porcelain", "-z", "--untracked-files=no"]).prefixed(&no_optional_locks);
    if !step(status, &[0]).map_err(unreadable)?.stdout.is_empty() {
        return Err(NotDetached::Uncommitted);
    }
    let stashed = step(run(&["stash", "list", "--format=%H"]), &[0]).map_err(unreadable)?;
    let remotes = run(&["config", "-z", "--get-regexp", r"^remote\..*\.url$"]);
    let remote_configured = step(remotes, &[0, 1]).map_err(unreadable)?.end == End::Code(0);
    let mut words = vec!["rev-list", "-n", "1", "--branches", "--tags", "--stdin"];
    // With no remote configured every commit counts, whatever stale remote-tracking refs
    // remain (`Holds/G3`).
    if remote_configured {
        words.extend(["--not", "--remotes"]);
    }
    let found = step(run(&words).feed(stashed.stdout), &[0]).map_err(unreadable)?;
    if !found.stdout.is_empty() {
        return Err(NotDetached::Unpushed { remote_configured });
    }
    Ok(())
}

/// A deciding run that failed: killed by a signal, or a private repository that cannot be
/// read.
fn unreadable(failed: Failed) -> NotDetached {
    match failed {
        Failed::Exited(End::Signal(_)) => NotDetached::Killed(failed),
        failed => NotDetached::Unreadable(failed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn leaving(private_directory: PathBuf, refused: Option<usize>) -> Leaving {
        Leaving {
            unlisted: None,
            removed: Answered {
                warnings: Vec::new(),
                visible: 0,
                refused,
                killed: None,
            },
            private_directory,
        }
    }

    #[test]
    fn nothing_is_removed_while_what_public_git_sees_is_unknown() {
        let scratch =
            Scratch(std::env::temp_dir().join(format!("git-dupe-detach-{}", std::process::id())));
        let private = scratch.0.join("dupe");
        fs::create_dir_all(private.join("objects")).unwrap();
        assert!(matches!(
            leaving(private.clone(), Some(3)).remove(),
            Err(NotRemoved::TooLong(3))
        ));
        assert!(private.join("objects").is_dir());
        let mut killed = leaving(private.clone(), None);
        killed.removed.killed = Some(Failed::Exited(End::Signal(15)));
        assert!(matches!(
            killed.remove(),
            Err(NotRemoved::Killed(Failed::Exited(End::Signal(15))))
        ));
        assert!(private.join("objects").is_dir());

        assert!(leaving(private.clone(), None).remove().is_ok());
        assert!(!private.exists());
        // A removal that fails names the path it was asked to remove.
        match leaving(private.clone(), None).remove() {
            Err(NotRemoved::Failed(path, _)) => assert_eq!(path, private),
            _ => panic!("removing nothing succeeded"),
        }
        assert!(scratch.0.is_dir());
    }
}
