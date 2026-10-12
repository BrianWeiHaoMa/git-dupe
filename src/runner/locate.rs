//! The locate run and the workspace facts.
//!
//! One `git rev-parse`, in the environment git-dupe received and from its directory, is
//! the only discovery: `-C`, `--git-dir`, and `--work-tree` before `dupe` name the
//! repository as Git read them. Its answer is read as bytes, never as text, and
//! its failure is Git's own answer: which failure it was is never read from the message.
//! For a command that attaches, the same run also asks for the superproject, whose
//! answer marks a submodule checkout (G4).
//!
//! `rev-parse` prints its answers in turn and stops at the first it cannot give (S12).
//! A failed locate that printed nothing was made outside any repository; one that
//! printed the Git directory and the common Git directory, in a bare repository or inside
//! a Git directory. That is all that is read of a failure, and only a run the front
//! makes without a repository uses it (`Unlocated`).
//!
//! The facts a destination is compared from (`Facts`, `Holds/G18`) come from the same
//! answers, and one more: the user's directory where it may lie outside the root, asked
//! of the process when a guard needs it: located, wherever the locate printed no prefix,
//! which is outside the root or at the root itself, where reading a destination from it
//! is reading it from the root again. Where the locate failed after naming a Git
//! directory, there is no root: the common Git directory's parent stands for it, which
//! is the main worktree's root wherever that worktree's `.git` is the common Git
//! directory (`Holds/G1`), and the user's directory, inside a Git directory, is always
//! read too. The facts also hold the value of `HOME`
//! git-dupe received, which every run passes through and from which Git reads a local
//! destination's leading `~` (S13).

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use super::{Against, End, Failure, Run};

const LOCATE: [&str; 6] = [
    "rev-parse",
    "--path-format=absolute",
    "--git-dir",
    "--git-common-dir",
    "--show-toplevel",
    "--show-prefix",
];

/// Asked immediately before `--show-prefix`, so that the prefix stays the rest of the
/// answer. Outside a submodule checkout it prints nothing, not even an empty line; inside
/// one it prints the superproject's working tree, absolute.
const SUPERPROJECT: &str = "--show-superproject-working-tree";

/// What the locate run established about where git-dupe stands.
pub struct Workspace {
    git_directory: PathBuf,
    common_directory: PathBuf,
    root: PathBuf,
    prefix: Vec<u8>,
    attached: bool,
    submodule_checkout: bool,
}

/// Why there is no workspace to work in.
pub enum NotLocated {
    /// Git refused: outside any repository, or where there is no working tree. Its
    /// message and its end are the command's, but for what the front answers without a
    /// repository, made where `unlocated` says; `None` when what Git printed before it
    /// failed is neither of the two answers a failure gives.
    Refused {
        message: Vec<u8>,
        end: End,
        unlocated: Option<Unlocated>,
    },
    /// Git answered with something other than the four answers asked for.
    Unreadable,
    NotStarted(Failure),
}

/// Locates the workspace. `for_attaching` also asks whether it is a submodule checkout,
/// which only a command that attaches needs to know.
pub fn locate(for_attaching: bool) -> Result<Workspace, NotLocated> {
    let run = Run::public(words(for_attaching))
        .capture_output()
        .capture_errors()
        .start()
        .map_err(NotLocated::NotStarted)?;
    if run.end != End::Code(0) {
        return Err(NotLocated::Refused {
            unlocated: Unlocated::read(&run.stdout),
            message: run.stderr,
            end: run.end,
        });
    }
    Workspace::read(&run.stdout, for_attaching).ok_or(NotLocated::Unreadable)
}

fn words(for_attaching: bool) -> Vec<&'static str> {
    let (before_prefix, prefix) = LOCATE.split_at(LOCATE.len() - 1);
    let superproject = for_attaching.then_some(SUPERPROJECT);
    before_prefix
        .iter()
        .copied()
        .chain(superproject)
        .chain(prefix.iter().copied())
        .collect()
}

impl Workspace {
    /// Reads the four answers: the Git directory, the common Git directory, and the root,
    /// absolute and one line each, then the prefix, relative and ending in `/`, or empty
    /// at the root.
    /// No working tree or Git directory of the project has a newline in its path, but a
    /// directory below the root may: the prefix is the rest of the answer.
    /// With `superproject` asked, a submodule checkout's answer has one more line before
    /// the prefix, the superproject's absolute path; a prefix never begins with `/`, so a
    /// line that does is that answer.
    /// An answer of any other shape is not the one asked for. A newline in one of the
    /// three paths, which git-dupe does not support, usually gives one; where it does
    /// not, the paths read here are not the real ones.
    fn read(answer: &[u8], superproject: bool) -> Option<Workspace> {
        let mut lines = answer.strip_suffix(b"\n")?.splitn(4, |&b| b == b'\n');
        let (Some(git_directory), Some(common_directory), Some(root), Some(mut prefix)) =
            (lines.next(), lines.next(), lines.next(), lines.next())
        else {
            return None;
        };
        let absolute = |path: &[u8]| path.starts_with(b"/");
        let submodule_checkout = superproject && absolute(prefix);
        if submodule_checkout {
            let (_, after) = prefix.split_at(prefix.iter().position(|&b| b == b'\n')?);
            prefix = &after[1..];
        }
        let relative = prefix.is_empty() || (!absolute(prefix) && prefix.ends_with(b"/"));
        if !(absolute(git_directory) && absolute(common_directory) && absolute(root) && relative) {
            return None;
        }
        let mut workspace = Workspace {
            git_directory: PathBuf::from(OsStr::from_bytes(git_directory)),
            common_directory: PathBuf::from(OsStr::from_bytes(common_directory)),
            root: PathBuf::from(OsStr::from_bytes(root)),
            prefix: prefix.to_vec(),
            attached: false,
            submodule_checkout,
        };
        workspace.attached = workspace.attached_now();
        Some(workspace)
    }

    /// Whether the workspace was attached when the command began.
    pub fn attached(&self) -> bool {
        self.attached
    }

    /// Whether the workspace is attached as things stand now, asked again after a
    /// handler that can attach it: this worktree's private Git directory is a directory
    /// by `lstat`, whatever the other worktrees hold (G4). A symbolic link there is not
    /// one, wherever it leads.
    pub fn attached_now(&self) -> bool {
        fs::symlink_metadata(self.private_directory()).is_ok_and(|found| found.is_dir())
    }

    /// The private Git directory's path where something other than a directory stands
    /// there by `lstat` (`obstructed`).
    pub fn obstruction(&self) -> Option<PathBuf> {
        let private = self.private_directory();
        obstructed(&private).then_some(private)
    }

    /// The root: the working tree's top directory, absolute.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The user's directory relative to the root, as Git printed it: ending in `/`, or
    /// empty at the root. Operands are read relative to it.
    pub fn prefix(&self) -> &[u8] {
        &self.prefix
    }

    /// The common Git directory, absolute: the public repository's `.git`, or a bare
    /// repository's own directory, which holds `info/exclude` for every worktree.
    pub fn common_directory(&self) -> &Path {
        &self.common_directory
    }

    /// Where this worktree's private repository lives: `dupe` in its Git directory,
    /// `.git/dupe` in the main worktree and `.git/worktrees/<name>/dupe` in a linked one
    /// (F2). Every private run and every write of the private repository's is made here,
    /// and where it is no directory the worktree is unattached, so that a run made there
    /// before an `init` names a path nothing has created yet (`Holds/G24`).
    pub fn private_directory(&self) -> PathBuf {
        private_directory(&self.git_directory)
    }

    /// The Git directory, absolute, as the locate run printed it: the common Git
    /// directory in the main worktree, `<common Git directory>/worktrees/<name>` in a
    /// linked one (S17).
    pub fn git_directory(&self) -> &Path {
        &self.git_directory
    }

    /// In a linked worktree, its name: the last component of the Git directory, as bytes
    /// (E9, S17). The main worktree has none. Neither the root's name nor any other Git
    /// run decides it, so that a worktree moved with `git worktree move` keeps it.
    pub fn worktree_name(&self) -> Option<&OsStr> {
        self.linked()
            .then(|| self.git_directory().file_name())
            .flatten()
    }

    /// Whether the locate run, made for a command that attaches, found a superproject:
    /// this working tree is a submodule checkout. A locate for any other command does
    /// not ask, and this is false there.
    pub fn submodule_checkout(&self) -> bool {
        self.submodule_checkout
    }

    /// The facts a destination is compared from: the root, the common Git directory, the
    /// user's directory where the locate printed no prefix, and `HOME` (`Facts`). Below the
    /// root that directory is none; a directory that cannot be had is none, because Git
    /// could not read a destination from it either.
    pub fn facts(&self) -> Facts {
        Facts {
            root: self.root.clone(),
            common_directory: self.common_directory.clone(),
            outside: self
                .prefix
                .is_empty()
                .then(env::current_dir)
                .and_then(Result::ok),
            home: env::var_os("HOME"),
        }
    }

    /// Whether this is a linked worktree: its Git directory is not the common one.
    pub fn linked(&self) -> bool {
        self.git_directory != self.common_directory
    }
}

#[cfg(test)]
impl Workspace {
    /// The main working tree at `root`, absolute, as the locate run names it from the
    /// root: for unit checks of what a part does on disk there.
    pub fn at_root(root: &Path) -> Workspace {
        let root = root.as_os_str().as_bytes();
        let answer = [root, b"/.git\n", root, b"/.git\n", root, b"\n\n"].concat();
        Workspace::read(&answer, false).expect("an absolute root without a newline")
    }

    /// The linked worktree `name` of the common Git directory `common`, at `root`, all
    /// absolute, as the locate run names it from that root.
    pub fn linked_at(common: &Path, name: &[u8], root: &Path) -> Workspace {
        let common = common.as_os_str().as_bytes();
        let root = root.as_os_str().as_bytes();
        let answer = [
            common,
            b"/worktrees/",
            name,
            b"\n",
            common,
            b"\n",
            root,
            b"\n\n",
        ]
        .concat();
        Workspace::read(&answer, false).expect("absolute paths without a newline")
    }
}

/// `dupe` in a Git directory: the private repository of the worktree it belongs to (F2).
/// Every part that names a private Git directory names it here.
pub fn private_directory(git_directory: &Path) -> PathBuf {
    git_directory.join("dupe")
}

/// Whether something other than a directory stands at a private Git directory's path by
/// `lstat`: a symbolic link, wherever it leads, or a file. Such a worktree is not
/// attached, and no Git run is made against that path, because Git would follow it to
/// whatever repository it names, the public one or another worktree's private one (G5,
/// G28): `init` and `clone` make no repository there, no alias is looked up there, and a
/// help request runs nothing.
pub fn obstructed(private_directory: &Path) -> bool {
    fs::symlink_metadata(private_directory).is_ok_and(|found| !found.is_dir())
}

/// Whether something stands at the root-relative `path` by `lstat`: a file, a directory,
/// or a symbolic link, a dangling one included; an `lstat` that fails for any cause finds
/// nothing (G9, G27, `Holds/G3`, `Holds/G13`).
pub fn present(root: &Path, path: &[u8]) -> bool {
    fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_ok()
}

/// Whether `lstat` finds a directory at the root-relative `path`: a symbolic link to one
/// is none (`Holds/G13`, `Holds/G16`, `Holds/G2`).
pub fn directory_at(root: &Path, path: &[u8]) -> bool {
    fs::symlink_metadata(root.join(OsStr::from_bytes(path))).is_ok_and(|found| found.is_dir())
}

/// Where a destination is compared from (`Holds/G18`), all absolute: the root a relative
/// destination is read from, the common Git directory that holds `worktrees/`, and the
/// user's directory where a relative destination is also read from when it lies outside
/// the root (`outside`; it may be the root itself, which reads the same). And `home`, the
/// value of `HOME` as received, which may be anything: Git reads a destination's leading
/// `~` from it, and none when it is unset (S13).
pub struct Facts {
    root: PathBuf,
    common_directory: PathBuf,
    outside: Option<PathBuf>,
    home: Option<OsString>,
}

impl Facts {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn common_directory(&self) -> &Path {
        &self.common_directory
    }

    pub fn outside(&self) -> Option<&Path> {
        self.outside.as_deref()
    }

    pub fn home(&self) -> Option<&OsStr> {
        self.home.as_deref()
    }
}

/// Where a run is made that needs no repository, after a locate that failed
/// (`Composition/Runner`).
pub enum Unlocated {
    /// The locate printed nothing: outside any repository. Such a run gets the environment
    /// received.
    Outside,
    /// The locate printed the Git directory and the common Git directory: a bare
    /// repository, or a directory inside a Git directory. Such a run is private, its Git
    /// directory `dupe` in that Git directory, with no working tree, so that it never
    /// selects the repository found there, whose own aliases could write (G5). Inside a
    /// linked worktree's Git directory that is the linked worktree's own private
    /// repository, or a path nothing has created, and never the main worktree's
    /// (`Holds/G24`). The common Git directory is kept as printed, for the facts.
    InGitDirectory {
        git_directory: PathBuf,
        common_directory: PathBuf,
    },
}

impl Unlocated {
    /// What a failed locate printed before failing: nothing, or two absolute lines, the
    /// Git directory and the common Git directory. Any other answer is neither.
    fn read(printed: &[u8]) -> Option<Unlocated> {
        if printed.is_empty() {
            return Some(Unlocated::Outside);
        }
        let mut lines = printed.strip_suffix(b"\n")?.split(|&b| b == b'\n');
        let (Some(git_directory), Some(common_directory), None) =
            (lines.next(), lines.next(), lines.next())
        else {
            return None;
        };
        if !(git_directory.starts_with(b"/") && common_directory.starts_with(b"/")) {
            return None;
        }
        Some(Unlocated::InGitDirectory {
            git_directory: private_directory(Path::new(OsStr::from_bytes(git_directory))),
            common_directory: PathBuf::from(OsStr::from_bytes(common_directory)),
        })
    }

    /// Inside a Git directory, its private Git directory's path where something other
    /// than a directory stands there (`obstructed`): no run is made against it.
    pub fn obstruction(&self) -> Option<&Path> {
        match self {
            Unlocated::Outside => None,
            Unlocated::InGitDirectory { git_directory, .. } => {
                obstructed(git_directory).then_some(git_directory.as_path())
            }
        }
    }

    /// The repository such a run is made against.
    pub fn against(&self) -> Against<'_> {
        match self {
            Unlocated::Outside => Against::Public,
            Unlocated::InGitDirectory { git_directory, .. } => {
                Against::PrivateWithoutWorkTree { git_directory }
            }
        }
    }

    /// The facts a destination is compared from, inside a Git directory: its common Git
    /// directory, that directory's parent as the root, the user's directory always, and
    /// `HOME`. Outside any repository there is no public place, and none.
    pub fn facts(&self) -> Option<Facts> {
        let Unlocated::InGitDirectory {
            common_directory, ..
        } = self
        else {
            return None;
        };
        Some(Facts {
            root: common_directory
                .parent()
                .unwrap_or(common_directory)
                .to_path_buf(),
            common_directory: common_directory.clone(),
            outside: env::current_dir().ok(),
            home: env::var_os("HOME"),
        })
    }
}

#[cfg(test)]
mod tests;
