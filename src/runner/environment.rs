//! The environments of the two kinds of run (G10). Every run's environment is built here
//! from the one git-dupe received, by the fixed rules below and by nothing else:
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
//! No variable is set or removed but those named here; `LC_ALL` and `LANG` are never set
//! (R5).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::ffi::OsStrExt;
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

/// The environment of one run: `received` changed by the rules above for a run
/// `against` a repository whose command word is git-dupe's own `own_command`, or the
/// user's when `None`, whose paths are git-dupe's own when `own_paths`, and which, when
/// public, reads the index the user's own command reads when `users_index`. The user's
/// directory is asked for only when a relative location must be made absolute.
pub fn of(
    against: &Against,
    own_command: Option<&OsStr>,
    own_paths: bool,
    users_index: bool,
    received: impl IntoIterator<Item = (OsString, OsString)>,
    user_directory: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<BTreeMap<OsString, OsString>> {
    let mut environment: BTreeMap<OsString, OsString> = received.into_iter().collect();
    let mut remove = |names: &[&str]| {
        for name in names {
            environment.remove(OsStr::new(name));
        }
    };
    if own_paths {
        remove(&PATHSPEC);
    }
    if own_command == Some(OsStr::new("config")) {
        remove(&["GIT_CONFIG"]);
    }
    match against {
        Against::Public => {
            if own_command == Some(OsStr::new("ls-files")) && !users_index {
                remove(&["GIT_INDEX_FILE"]);
            }
            let mut user_directory = Some(user_directory);
            let mut known = None;
            for name in ["GIT_DIR", "GIT_WORK_TREE"] {
                let Some(value) = environment.get_mut(OsStr::new(name)) else {
                    continue;
                };
                if value.is_empty() || value.as_bytes().starts_with(b"/") {
                    continue;
                }
                if known.is_none() {
                    let asked = user_directory.take().expect("asked once");
                    known = Some(asked()?);
                }
                let directory: &Path = known.as_deref().expect("known");
                *value = directory.join(&*value).into_os_string();
            }
        }
        Against::Private {
            git_directory,
            root,
        } => {
            remove(&LOCATING);
            environment.insert("GIT_DIR".into(), git_directory.as_os_str().to_owned());
            environment.insert("GIT_WORK_TREE".into(), root.as_os_str().to_owned());
        }
        Against::PrivateWithoutWorkTree { git_directory } => {
            remove(&LOCATING);
            remove(&["GIT_WORK_TREE"]);
            environment.insert("GIT_DIR".into(), git_directory.as_os_str().to_owned());
        }
    }
    Ok(environment)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A caller's environment as a hook of the public repository sees it, run with
    /// `--literal-pathspecs` and `-c` before `dupe`.
    fn received() -> Vec<(OsString, OsString)> {
        [
            ("HOME", "/home/u"),
            ("PATH", "/bin"),
            ("GIT_DIR", ".git"),
            ("GIT_WORK_TREE", "sub/.."),
            ("GIT_INDEX_FILE", "/w/repo/.git/next-index.lock"),
            ("GIT_COMMON_DIR", "/w/repo/.git"),
            ("GIT_OBJECT_DIRECTORY", "/w/repo/.git/objects"),
            ("GIT_ALTERNATE_OBJECT_DIRECTORIES", "/elsewhere/objects"),
            ("GIT_REFERENCE_BACKEND", "files:///w/repo/.git"),
            ("GIT_LITERAL_PATHSPECS", "1"),
            ("GIT_GLOB_PATHSPECS", "0"),
            ("GIT_NOGLOB_PATHSPECS", "0"),
            ("GIT_ICASE_PATHSPECS", "1"),
            ("GIT_CONFIG", "/w/other.config"),
            ("GIT_CONFIG_PARAMETERS", "'color.ui'='never'"),
            ("GIT_PAGER", "cat"),
            ("GIT_NAMESPACE", "space"),
        ]
        .into_iter()
        .map(|(name, value)| (name.into(), value.into()))
        .collect()
    }

    fn built(
        against: &Against,
        command: &str,
        own_paths: bool,
        received: Vec<(OsString, OsString)>,
    ) -> BTreeMap<OsString, OsString> {
        of(
            against,
            (!command.is_empty()).then_some(OsStr::new(command)),
            own_paths,
            false,
            received,
            || Ok(PathBuf::from("/w/repo/sub")),
        )
        .expect("an environment")
    }

    /// The names `received` holds that `environment` lacks, and the ones it holds with
    /// another value, each with the value it now has.
    fn changes(environment: &BTreeMap<OsString, OsString>) -> Vec<(String, Option<String>)> {
        let before: BTreeMap<OsString, OsString> = received().into_iter().collect();
        let mut changed = Vec::new();
        for (name, value) in &before {
            match environment.get(name) {
                Some(now) if now == value => {}
                now => changed.push((
                    name.to_string_lossy().into_owned(),
                    now.map(|now| now.to_string_lossy().into_owned()),
                )),
            }
        }
        for (name, value) in environment {
            if !before.contains_key(name) {
                changed.push((
                    name.to_string_lossy().into_owned(),
                    Some(value.to_string_lossy().into_owned()),
                ));
            }
        }
        changed.sort();
        changed
    }

    fn named(pairs: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
        let mut pairs: Vec<_> = pairs
            .iter()
            .map(|(name, value)| (name.to_string(), value.map(str::to_string)))
            .collect();
        pairs.sort();
        pairs
    }

    fn private() -> Against<'static> {
        Against::Private {
            git_directory: Path::new("/w/repo/.git/dupe"),
            root: Path::new("/w/repo"),
        }
    }

    #[test]
    fn a_public_run_makes_relative_locations_absolute_and_changes_nothing_else() {
        let environment = built(&Against::Public, "rev-parse", false, received());
        assert_eq!(
            changes(&environment),
            named(&[
                ("GIT_DIR", Some("/w/repo/sub/.git")),
                ("GIT_WORK_TREE", Some("/w/repo/sub/sub/..")),
            ])
        );
    }

    #[test]
    fn a_public_listing_of_git_dupes_own_paths_reads_the_public_index() {
        let environment = built(&Against::Public, "ls-files", true, received());
        assert_eq!(
            changes(&environment),
            named(&[
                ("GIT_DIR", Some("/w/repo/sub/.git")),
                ("GIT_WORK_TREE", Some("/w/repo/sub/sub/..")),
                ("GIT_INDEX_FILE", None),
                ("GIT_LITERAL_PATHSPECS", None),
                ("GIT_GLOB_PATHSPECS", None),
                ("GIT_NOGLOB_PATHSPECS", None),
                ("GIT_ICASE_PATHSPECS", None),
            ])
        );
    }

    #[test]
    fn the_listing_in_the_users_index_keeps_it_and_a_private_run_never_does() {
        let in_the_users_index = |against: &Against| {
            of(
                against,
                Some(OsStr::new("ls-files")),
                true,
                true,
                received(),
                || Ok(PathBuf::from("/w/repo/sub")),
            )
            .expect("an environment")
        };
        let environment = in_the_users_index(&Against::Public);
        assert_eq!(
            changes(&environment),
            named(&[
                ("GIT_DIR", Some("/w/repo/sub/.git")),
                ("GIT_WORK_TREE", Some("/w/repo/sub/sub/..")),
                ("GIT_LITERAL_PATHSPECS", None),
                ("GIT_GLOB_PATHSPECS", None),
                ("GIT_NOGLOB_PATHSPECS", None),
                ("GIT_ICASE_PATHSPECS", None),
            ])
        );
        let environment = in_the_users_index(&private());
        assert!(!environment.contains_key(OsStr::new("GIT_INDEX_FILE")));
    }

    #[test]
    fn a_public_check_ignore_keeps_the_index_variable_it_does_not_read() {
        let environment = built(&Against::Public, "check-ignore", true, received());
        assert_eq!(
            environment.get(OsStr::new("GIT_INDEX_FILE")),
            Some(&OsString::from("/w/repo/.git/next-index.lock"))
        );
        assert!(!environment.contains_key(OsStr::new("GIT_LITERAL_PATHSPECS")));
    }

    #[test]
    fn a_private_run_names_the_private_repository_whatever_the_caller_set() {
        let environment = built(&private(), "cat-file", false, received());
        assert_eq!(
            changes(&environment),
            named(&[
                ("GIT_DIR", Some("/w/repo/.git/dupe")),
                ("GIT_WORK_TREE", Some("/w/repo")),
                ("GIT_INDEX_FILE", None),
                ("GIT_COMMON_DIR", None),
                ("GIT_OBJECT_DIRECTORY", None),
                ("GIT_ALTERNATE_OBJECT_DIRECTORIES", None),
                ("GIT_REFERENCE_BACKEND", None),
            ])
        );
    }

    #[test]
    fn a_private_run_of_git_dupes_own_paths_drops_the_pathspec_variables() {
        let environment = built(&private(), "ls-files", true, received());
        for name in PATHSPEC {
            assert!(!environment.contains_key(OsStr::new(name)), "{name}");
        }
        // What Git set for the global options before `dupe` still reaches the run.
        for name in [
            "GIT_CONFIG_PARAMETERS",
            "GIT_PAGER",
            "GIT_NAMESPACE",
            "GIT_CONFIG",
        ] {
            assert!(environment.contains_key(OsStr::new(name)), "{name}");
        }
    }

    #[test]
    fn a_config_run_of_either_kind_drops_git_config() {
        for against in [&Against::Public, &private()] {
            let environment = built(against, "config", false, received());
            assert!(!environment.contains_key(OsStr::new("GIT_CONFIG")));
            assert!(environment.contains_key(OsStr::new("GIT_LITERAL_PATHSPECS")));
        }
    }

    #[test]
    fn a_run_whose_command_word_is_the_users_takes_no_rule_from_that_word() {
        // The empty word stands for the user's: `config` and `ls-files` typed by the user
        // keep `GIT_CONFIG` and `GIT_INDEX_FILE` as plain Git would, and the pathspec
        // variables reach the user's own pathspecs.
        let environment = built(&Against::Public, "", false, received());
        assert_eq!(
            changes(&environment),
            named(&[
                ("GIT_DIR", Some("/w/repo/sub/.git")),
                ("GIT_WORK_TREE", Some("/w/repo/sub/sub/..")),
            ])
        );
        let environment = built(&private(), "", false, received());
        for name in ["GIT_CONFIG", "GIT_LITERAL_PATHSPECS", "GIT_ICASE_PATHSPECS"] {
            assert!(environment.contains_key(OsStr::new(name)), "{name}");
        }
        assert!(!environment.contains_key(OsStr::new("GIT_INDEX_FILE")));
    }

    #[test]
    fn where_the_locate_named_only_a_git_directory_the_run_has_no_working_tree() {
        let against = Against::PrivateWithoutWorkTree {
            git_directory: Path::new("/w/bare.git/dupe"),
        };
        let environment = built(&against, "", false, received());
        assert_eq!(
            changes(&environment),
            named(&[
                ("GIT_DIR", Some("/w/bare.git/dupe")),
                ("GIT_WORK_TREE", None),
                ("GIT_INDEX_FILE", None),
                ("GIT_COMMON_DIR", None),
                ("GIT_OBJECT_DIRECTORY", None),
                ("GIT_ALTERNATE_OBJECT_DIRECTORIES", None),
                ("GIT_REFERENCE_BACKEND", None),
            ])
        );
        let environment = built(&against, "config", false, received());
        assert!(!environment.contains_key(OsStr::new("GIT_CONFIG")));
    }

    #[test]
    fn a_private_run_sets_the_locations_where_the_caller_set_none() {
        let environment = built(&private(), "ls-files", true, Vec::new());
        assert_eq!(
            environment,
            BTreeMap::from([
                ("GIT_DIR".into(), "/w/repo/.git/dupe".into()),
                ("GIT_WORK_TREE".into(), "/w/repo".into()),
            ])
        );
    }

    #[test]
    fn absolute_locations_need_no_directory_and_are_kept_as_bytes() {
        let received = vec![
            (
                OsString::from("GIT_DIR"),
                OsString::from("/w/caf\u{e9}/.git"),
            ),
            (
                OsString::from("GIT_WORK_TREE"),
                OsStr::from_bytes(b"/w/caf\xe9").to_owned(),
            ),
        ];
        let environment = of(
            &Against::Public,
            Some(OsStr::new("ls-files")),
            true,
            false,
            received.clone(),
            || Err(io::Error::other("no directory")),
        )
        .expect("no directory needed");
        assert_eq!(environment, received.into_iter().collect());
    }

    #[test]
    fn a_relative_location_without_a_user_directory_is_a_failure() {
        let failed = of(
            &Against::Public,
            Some(OsStr::new("ls-files")),
            true,
            false,
            received(),
            || Err(io::Error::other("no directory")),
        );
        assert!(failed.is_err());
    }
}
