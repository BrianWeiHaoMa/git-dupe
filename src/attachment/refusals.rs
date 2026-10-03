//! Where `init` does not attach (G4): it attaches only the main working tree of an
//! ordinary repository whose `.git` entry at the root is the repository's Git directory,
//! and not a project that tracks `.gitdupe` publicly. A bare repository never gets here:
//! Git's locate run fails there.

use std::fs;
use std::os::unix::fs::MetadataExt;

use crate::keeper::{self, Failed};
use crate::runner::locate::Workspace;

/// Why `init` does not attach this workspace, in the order they are decided.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A linked worktree: its Git directory is not the common one.
    LinkedWorktree,
    /// A submodule checkout: Git names a superproject.
    SubmoduleCheckout,
    /// The `.git` entry at the root is not a directory by `lstat`, or not the directory
    /// that holds this repository, as for a working tree `--work-tree` names apart from
    /// its repository. `core.worktree` `../..`, relative to `.git/dupe`, could not name
    /// the root.
    NotOwnGitDirectory,
    /// The public repository tracks `.gitdupe`.
    TracksGitdupe,
}

/// The first refusal that applies, from facts of the locate run, one `lstat`, and one
/// public `ls-files` for `.gitdupe`, which runs only when nothing else refuses.
pub fn refusal(workspace: &Workspace) -> Result<Option<Refusal>, Failed> {
    if workspace.linked() {
        return Ok(Some(Refusal::LinkedWorktree));
    }
    if workspace.submodule_checkout() {
        return Ok(Some(Refusal::SubmoduleCheckout));
    }
    if !own_git_directory(workspace) {
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
