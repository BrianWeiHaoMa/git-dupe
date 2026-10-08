//! The worktrees of a project as Git itself names them: a worktree's root, its Git
//! directory, and the common Git directory, read from one public `rev-parse` at its root,
//! so that a scenario of a linked worktree — of an ordinary repository or a bare one, and
//! after `git worktree move` — finds its private repository and its region where Git puts
//! them, never by its root's name (F2, F3, S17). The facts are a snapshot: read them again
//! after a move.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::attached::{Private, Region, region_in};
use super::output::records;
use super::scenario::Scenario;
use super::tree::Tree;

/// One worktree, as Git answered for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    /// The top of its working tree, absolute.
    pub root: PathBuf,
    /// Its Git directory, absolute: the common Git directory in the main worktree,
    /// `<common Git directory>/worktrees/<name>` in a linked one.
    pub git_directory: PathBuf,
    /// The common Git directory, absolute: `.git`, or a bare repository's own directory.
    pub common_directory: PathBuf,
}

impl Worktree {
    /// The facts of the worktree at `root`, from one `git rev-parse` run there, which
    /// must succeed.
    pub fn read(s: &Scenario, root: &Path) -> Worktree {
        let answer = s
            .git([
                "rev-parse",
                "--path-format=absolute",
                "--git-dir",
                "--git-common-dir",
                "--show-toplevel",
            ])
            .from(root)
            .succeeds();
        let lines = records(&answer.stdout, b'\n');
        let [git_directory, common_directory, top] = lines[..] else {
            panic!("{}: {answer:?}", root.display());
        };
        let path = |bytes: &[u8]| PathBuf::from(OsStr::from_bytes(bytes));
        Worktree {
            root: path(top),
            git_directory: path(git_directory),
            common_directory: path(common_directory),
        }
    }

    /// A linked worktree's name, the last component of its Git directory; none in the
    /// main worktree, whose Git directory is the common one.
    pub fn name(&self) -> Option<&OsStr> {
        (self.git_directory != self.common_directory)
            .then(|| self.git_directory.file_name())
            .flatten()
    }

    /// Where its private repository is: `dupe` in its Git directory.
    pub fn private_directory(&self) -> PathBuf {
        self.git_directory.join("dupe")
    }

    /// Git against its private repository, from its root.
    pub fn private<'s>(&self, s: &'s Scenario) -> Private<'s> {
        s.private_at(&self.root, &self.private_directory())
    }

    /// Its region of the common Git directory's `info/exclude`, as `region_in` reads it.
    pub fn region(&self) -> Option<Region> {
        region_in(&self.common_directory, self.name())
    }

    /// Its region as the bytes of the exclude file, its markers included, by the bounds
    /// `region_in` reads: for comparing a region byte for byte across another worktree's
    /// commands. It must have one.
    pub fn region_bytes(&self) -> Vec<u8> {
        let exclude = self.common_directory.join("info/exclude");
        let region = self
            .region()
            .unwrap_or_else(|| panic!("no region of {self:?} in {}", exclude.display()));
        let bytes =
            fs::read(&exclude).unwrap_or_else(|cause| panic!("{}: {cause}", exclude.display()));
        bytes[region.before.len()..bytes.len() - region.after.len()].to_vec()
    }

    /// The public repository as a `git dupe` command in this worktree must leave it:
    /// everything in the common Git directory but this worktree's private repository and
    /// `info/exclude`, the two places git-dupe writes there of its own accord once `info`
    /// exists (G5). The other worktrees' private repositories and Git directories are
    /// read with it, and so must stand as they were too.
    pub fn public_git(&self) -> Tree {
        let written = [
            self.private_directory(),
            self.common_directory.join("info/exclude"),
        ];
        Tree::of(&self.common_directory).without(&written.each_ref().map(PathBuf::as_path))
    }
}

/// `git dupe <words>` from the worktree's root, which must exit 0 and leave the public
/// repository, the other worktrees' private repositories among it, as `public_git` reads
/// it. A transfer that writes another worktree's private repository on purpose is run
/// without it.
pub fn worktree_dupe(s: &Scenario, wt: &Worktree, words: &[&str]) -> super::output::Output {
    let before = wt.public_git();
    let output = s
        .git(["dupe"].iter().chain(words))
        .from(&wt.root)
        .succeeds();
    let changed = before.changed_in(&wt.public_git());
    assert!(changed.is_empty(), "{output:?}: changed {changed:?}");
    output
}

/// `git dupe clone <remote>` in the unattached worktree, which must succeed without a
/// warning and print what this release's own `git init` prints for the private
/// repository's path: a reference repository made there by that `init` and removed
/// before the clone.
pub fn worktree_clone(s: &Scenario, wt: &Worktree, remote: &Path) -> super::output::Output {
    assert!(!wt.private_directory().exists());
    let init = s
        .git(["init", "--initial-branch=main"])
        .from(&wt.root)
        .variable("GIT_DIR", wt.private_directory())
        .variable("GIT_WORK_TREE", &wt.root)
        .succeeds();
    fs::remove_dir_all(wt.private_directory()).unwrap();
    let output = worktree_dupe(s, wt, &["clone", remote.to_str().unwrap()]);
    assert_eq!(output.stdout, init.stdout);
    assert!(output.lines("warning").is_empty(), "{output:?}");
    output
}

/// Public `git status` in the worktree lists nothing: no change, and no untracked file.
pub fn worktree_public_clean(s: &Scenario, wt: &Worktree) {
    let output = s
        .git(["status", "--porcelain", "--untracked-files=all"])
        .from(&wt.root)
        .succeeds();
    assert!(output.stdout.is_empty(), "{output:?}");
}

/// The commit a worktree's private HEAD names, read with Git itself.
pub fn worktree_private_head(s: &Scenario, wt: &Worktree) -> Vec<u8> {
    wt.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout
}
