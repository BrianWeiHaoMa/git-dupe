//! Where `init` and `clone` do not attach (G4): they attach a worktree of a repository —
//! the main worktree, whose `.git` entry at the root is the common Git directory, or a
//! linked worktree that `git worktree add` made, whose Git directory lies directly under
//! the common Git directory's `worktrees` (E9), a bare repository's included — and not a
//! submodule checkout or a project that tracks `.gitdupe` publicly. A bare repository's
//! own directory never gets here: Git's locate run fails there.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::keeper::{self, Failed};
use crate::runner::locate::Workspace;

/// Why `init` does not attach this workspace, in the order they are decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A submodule checkout: Git names a superproject.
    SubmoduleCheckout,
    /// The main worktree's `.git` entry at the root is not a directory by `lstat`, or not
    /// the directory that holds this repository, as for a working tree `--work-tree`
    /// names apart from its repository. `core.worktree` `../..`, relative to `.git/dupe`,
    /// could not name the root.
    NotOwnGitDirectory,
    /// A linked worktree whose Git directory's parent is not, by device and inode,
    /// `worktrees` in the common Git directory: no worktree `git worktree add` made.
    NotUnderWorktrees,
    /// The public repository tracks `.gitdupe`.
    TracksGitdupe,
}

/// The first refusal that applies, from facts of the locate run, `stat`s of directories
/// it named, and one public `ls-files` for `.gitdupe`, which runs only when nothing else
/// refuses.
pub fn refusal(workspace: &Workspace) -> Result<Option<Refusal>, Failed> {
    if workspace.submodule_checkout() {
        return Ok(Some(Refusal::SubmoduleCheckout));
    }
    if workspace.linked() {
        if !under_worktrees(workspace) {
            return Ok(Some(Refusal::NotUnderWorktrees));
        }
    } else if !own_git_directory(workspace) {
        return Ok(Some(Refusal::NotOwnGitDirectory));
    }
    let tracked = keeper::publicly_tracked(workspace, &[keeper::GITDUPE.to_vec()])?;
    if !tracked.is_empty() {
        return Ok(Some(Refusal::TracksGitdupe));
    }
    Ok(None)
}

/// Whether `<root>/.git` is, by `lstat`, a directory, and the common Git directory
/// itself. A symbolic link to a directory is not, whatever it points to.
fn own_git_directory(workspace: &Workspace) -> bool {
    let entry = fs::symlink_metadata(workspace.root().join(".git"));
    let common = fs::metadata(workspace.common_directory());
    match (entry, common) {
        (Ok(entry), Ok(common)) => {
            entry.is_dir() && entry.dev() == common.dev() && entry.ino() == common.ino()
        }
        _ => false,
    }
}

/// Whether the Git directory's parent is the common Git directory's `worktrees`, by
/// device and inode, however either path is spelled.
fn under_worktrees(workspace: &Workspace) -> bool {
    let Some(parent) = workspace.git_directory().parent() else {
        return false;
    };
    same_directory(parent, &workspace.common_directory().join("worktrees"))
}

fn same_directory(one: &Path, other: &Path) -> bool {
    match (fs::metadata(one), fs::metadata(other)) {
        (Ok(one), Ok(other)) => {
            one.is_dir() && one.dev() == other.dev() && one.ino() == other.ino()
        }
        _ => false,
    }
}
