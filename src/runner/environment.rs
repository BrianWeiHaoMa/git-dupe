//! The environments of the two kinds of run (G10). Every run inherits the environment
//! git-dupe received, changed here by the fixed rules below and by nothing else, before
//! the one run that sets a variable of its own sets it over them (`Run::variable`). The
//! changes are made through `Command`'s own environment and a value is read through the
//! caller's lookup, because only those fold a variable's name where the platform does: on
//! Windows a received `Git_Dir` is `GIT_DIR` (S20).
//!
//! - A **public run** keeps the environment as received, except that a relative
//!   `GIT_DIR` or `GIT_WORK_TREE` is made absolute against the user's directory, so that
//!   a run from the root names the same repository; that `GIT_INDEX_FILE` is removed
//!   from `ls-files`, which asks which paths are publicly tracked, so that a hook's
//!   temporary index is never read as the public index (S9), except from the listing
//!   that reads the index the user's own command reads: `clean` asks it before its
//!   `clean` runs, so that both read one index (`Holds/G16`).
//! - A **private run** has `GIT_DIR` and `GIT_WORK_TREE` set to the private Git directory
//!   and the root, absolute, and every other variable that locates a repository, index,
//!   or object store removed. Everything else passes through, `GIT_CONFIG_PARAMETERS`,
//!   `GIT_PAGER`, and `GIT_NAMESPACE` included, which is how Git's global options before
//!   `dupe` reach it (S3). Where the locate failed after naming a Git directory, a
//!   private run has `GIT_DIR` set to `dupe` in the Git directory it named, which is the
//!   common one but inside a linked worktree's Git directory, and `GIT_WORK_TREE`
//!   removed, because there is no root to name (`Composition/Runner`).
//! - Either kind, when its pathspecs or paths are git-dupe's own, has the four pathspec
//!   variables removed: under `GIT_LITERAL_PATHSPECS` a `:(top,literal)` pathspec is a
//!   file name, and `check-ignore` refuses every path (S5).
//! - Either kind of run of git-dupe's own `config` has `GIT_CONFIG` removed: Git reads
//!   that variable only in `git config`, as if `--file` named it. A run whose command
//!   word is the user's has no own command word, and neither rule that reads one applies
//!   to it: the user's `git dupe config` reads `GIT_CONFIG` as `git config` does.
//!
//! No variable is set or removed but those named here and that one run's; `LC_ALL` and
//! `LANG` are never set (R5).

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// Which repository a run is made against.
#[derive(Clone, Copy)]
pub enum Against<'p> {
    /// The public repository, as the received environment and directory name it.
    Public,
    /// The private repository: its Git directory and the root, both absolute.
    Private {
        git_directory: &'p Path,
        root: &'p Path,
    },
    /// A private Git directory, absolute, with no working tree: where the locate failed
    /// after naming a Git directory.
    PrivateWithoutWorkTree { git_directory: &'p Path },
}

impl Against<'_> {
    /// The private Git directory a run names, where a directory stands there: a private
    /// repository, with state to read. None for a public run, or where nothing, or no
    /// directory, stands at that path, as in a worktree no `init` has attached.
    pub fn private_repository(&self) -> Option<&Path> {
        match self {
            Against::Private { git_directory, .. }
            | Against::PrivateWithoutWorkTree { git_directory } => {
                Some(*git_directory).filter(|directory| directory.is_dir())
            }
            Against::Public => None,
        }
    }
}

/// The variables that locate a repository, an index, or an object store, other than
/// `GIT_DIR` and `GIT_WORK_TREE`, which a private run sets (G10). Git reads
/// `GIT_REFERENCE_BACKEND`, where references are kept, from 2.54.0 (S15).
const LOCATING: [&str; 5] = [
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_REFERENCE_BACKEND",
];

/// The variables Git sets for `--literal-pathspecs` and its siblings (S3).
const PATHSPEC: [&str; 4] = [
    "GIT_LITERAL_PATHSPECS",
    "GIT_GLOB_PATHSPECS",
    "GIT_NOGLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
];

/// What a run changes of the environment it inherits: these names removed, then these
/// set.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub removed: Vec<&'static str>,
    pub set: Vec<(&'static str, OsString)>,
}

/// The changes of one run, by the rules above, for a run `against` a repository whose
/// command word is git-dupe's own `own_command`, or the user's when `None`, whose paths are
/// git-dupe's own when `own_paths`, and which, when public, reads the index the user's own
/// command reads when `users_index`. `received` answers what a variable holds. The user's
/// directory is asked for only when a relative location must be made absolute.
pub fn of(
    against: &Against,
    own_command: Option<&[u8]>,
    own_paths: bool,
    users_index: bool,
    received: impl Fn(&str) -> Option<OsString>,
    user_directory: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<Changes> {
    let mut changes = Changes::default();
    if own_paths {
        changes.removed.extend(PATHSPEC);
    }
    if own_command == Some(b"config") {
        changes.removed.push("GIT_CONFIG");
    }
    match against {
        Against::Public => {
            if own_command == Some(b"ls-files") && !users_index {
                changes.removed.push("GIT_INDEX_FILE");
            }
            let mut user_directory = Some(user_directory);
            let mut known = None;
            for name in ["GIT_DIR", "GIT_WORK_TREE"] {
                let Some(value) = received(name) else {
                    continue;
                };
                if value.is_empty() || Path::new(&value).is_absolute() {
                    continue;
                }
                if known.is_none() {
                    let asked = user_directory.take().expect("asked once");
                    known = Some(asked()?);
                }
                let directory: &Path = known.as_deref().expect("known");
                changes
                    .set
                    .push((name, directory.join(&value).into_os_string()));
            }
        }
        Against::Private {
            git_directory,
            root,
        } => {
            changes.removed.extend(LOCATING);
            changes
                .set
                .push(("GIT_DIR", git_directory.as_os_str().to_owned()));
            changes
                .set
                .push(("GIT_WORK_TREE", root.as_os_str().to_owned()));
        }
        Against::PrivateWithoutWorkTree { git_directory } => {
            changes.removed.extend(LOCATING);
            changes.removed.push("GIT_WORK_TREE");
            changes
                .set
                .push(("GIT_DIR", git_directory.as_os_str().to_owned()));
        }
    }
    Ok(changes)
}

#[cfg(test)]
mod tests;
