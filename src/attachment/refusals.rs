//! Where `init` and `clone` do not attach (G4): they attach a worktree of a repository —
//! the main worktree, whose `.git` entry at the root is the common Git directory, or a
//! linked worktree that `git worktree add` made, whose Git directory lies directly under
//! the common Git directory's `worktrees` (E9), a bare repository's included — and not a
//! submodule checkout, a worktree where something other than a directory stands at its
//! private repository's path, or a project that tracks `.gitdupe` publicly. A bare
//! repository's own directory never gets here: Git's locate run fails there.

use std::fs;
use std::path::{Path, PathBuf};

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
    /// A linked worktree whose Git directory's parent is not, by canonical path,
    /// `worktrees` in the common Git directory: no worktree `git worktree add` made.
    NotUnderWorktrees,
    /// Something other than a directory stands at the private Git directory's path by
    /// `lstat`, this path: Git's `init` and `config` would follow it to whatever it leads
    /// to, the public repository or another worktree's private one (G5, G28), and git-dupe
    /// deletes nothing to make room (R8).
    Obstructed(PathBuf),
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
    if let Some(private) = workspace.obstruction() {
        return Ok(Some(Refusal::Obstructed(private)));
    }
    let tracked = keeper::publicly_tracked(workspace, &[keeper::GITDUPE.to_vec()])?;
    if !tracked.is_empty() {
        return Ok(Some(Refusal::TracksGitdupe));
    }
    Ok(None)
}

/// Whether `<root>/.git` is, by `lstat`, a directory, and the common Git directory
/// itself. A symbolic link to a directory is not, whatever it points to: the `lstat` comes
/// first because its canonical path would answer equal.
fn own_git_directory(workspace: &Workspace) -> bool {
    let entry = workspace.root().join(".git");
    fs::symlink_metadata(&entry).is_ok_and(|found| found.is_dir())
        && same_directory(&entry, workspace.common_directory())
}

/// Whether the Git directory's parent is the common Git directory's `worktrees`, by
/// canonical path, however either path is spelled.
fn under_worktrees(workspace: &Workspace) -> bool {
    let Some(parent) = workspace.git_directory().parent() else {
        return false;
    };
    same_directory(parent, &workspace.common_directory().join("worktrees"))
}

/// Whether the two paths have one canonical path; one that has none is not the same
/// (`Holds/G1`, S24: a device and inode are no question every platform can ask).
fn same_directory(one: &Path, other: &Path) -> bool {
    match (fs::canonicalize(one), fs::canonicalize(other)) {
        (Ok(one), Ok(other)) => one == other,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::symlink;

    /// A directory of the check's own, with a repository's `.git` in `repo`, removed after.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let path = std::env::temp_dir()
                .join(format!("git-dupe-refusals-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("repo/.git/worktrees/one")).unwrap();
            Scratch(path)
        }

        fn common(&self) -> PathBuf {
            self.0.join("repo/.git")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_git_directory_is_the_common_one_by_canonical_path_and_never_through_a_link() {
        let scratch = Scratch::new("own");
        let root = scratch.0.join("repo");
        assert!(own_git_directory(&Workspace::at_root(&root)));
        // Another spelling of the root, and a link in an ancestor of it.
        assert!(own_git_directory(&Workspace::at_root(&root.join("."))));
        symlink(&root, scratch.0.join("to-repo")).unwrap();
        let through = Workspace::at_root(&scratch.0.join("to-repo"));
        assert!(own_git_directory(&through));
        // A `.git` that is a link to the common directory is not a directory by `lstat`,
        // though its canonical path is that directory's.
        let linked_root = scratch.0.join("linked");
        fs::create_dir(&linked_root).unwrap();
        symlink(scratch.common(), linked_root.join(".git")).unwrap();
        assert!(!own_git_directory(&Workspace::at_root(&linked_root)));
        // No `.git` has no canonical form.
        let bare_root = scratch.0.join("empty");
        fs::create_dir(&bare_root).unwrap();
        assert!(!own_git_directory(&Workspace::at_root(&bare_root)));
    }

    #[test]
    fn a_linked_worktree_lies_in_the_common_directorys_worktrees_by_canonical_path() {
        let scratch = Scratch::new("under");
        let root = scratch.0.join("one");
        let linked = Workspace::linked_at(&scratch.common(), b"one", &root);
        assert!(under_worktrees(&linked));
        // The common Git directory reached through a link.
        symlink(scratch.common(), scratch.0.join("to-git")).unwrap();
        let through = Workspace::linked_at(&scratch.0.join("to-git"), b"one", &root);
        assert!(under_worktrees(&through));
        // A directory named `worktrees` of another repository is not it.
        let other = scratch.0.join("other/.git/worktrees/one");
        fs::create_dir_all(&other).unwrap();
        let answered = same_directory(
            other.parent().expect("a parent"),
            &scratch.common().join("worktrees"),
        );
        assert!(!answered);
    }
}
