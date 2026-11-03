//! The keys `init` gives the private repository (F2, G1), and how it tells whether they
//! are set: one private `config -z --show-scope --no-includes --get-all core.worktree`,
//! whose values in the repository's own files — the local `config` and, where
//! `extensions.worktreeConfig` is set, as a private sparse checkout sets it, moving the
//! key there, `config.worktree`, which Git reads after it (S12), the two files Git takes
//! the work tree from — are all the relative work tree only after the last key of a
//! finished `init`. Then none is set again, so that a key the developer edited
//! afterward stays as they left it (S6).
//!
//! The **relative work tree** is the lexical path from the private Git directory to the
//! root over their canonical forms (`Composition/Attachment`, R9): `../..` in the main
//! worktree, wherever the project is moved; in a linked worktree, a path out of
//! `.git/worktrees/<name>/dupe` to its root, which `git worktree move` makes wrong. It is
//! computed once per command, before any write, and that one spelling is both what the
//! recorded value is compared with, byte for byte, and what is written: any other value,
//! an absolute one naming the right root included, is an `init` that did not finish.
//! The private Git directory is reached through the Git directory, canonical, and not
//! resolved itself, because it may not exist yet, and a symbolic link there would be
//! followed somewhere else.
//!
//! Each key is its own `git config --replace-all` run, in a fixed order with
//! `core.worktree` last: a key a template set more than once would make a plain `git
//! config` refuse, and every later `init` with it. Every key but `core.worktree` goes to
//! the local file; `core.worktree` goes there too and, where Git read it from
//! `config.worktree`, there as well, so that no stale value can outrank the one written.
//! The identity is what
//! the public repository's local configuration holds, a file it includes counted, where
//! it holds one: `config --local --includes --get` answers nothing, exit 1, for a key set
//! only globally.
//!
//! A key whose effective value a file the `config` includes gives is not written: a
//! `--replace-all` of a key the file does not hold appends it after the include, where it
//! would outrank the included value, to which F2 and G1 give the last word. Two `config
//! -z --local --list` runs, with and without `--includes`, tell them apart (`Holds/G1`).
//! `core.worktree` is never so kept: Git follows no include for it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use super::step;
use crate::bytes;
use crate::keeper::Failed;
use crate::runner::locate::{Workspace, private_directory};
use crate::runner::records;
use crate::runner::{End, Run, Word};

/// The keys every private repository gets, whatever a template put there.
const FIXED: [(&str, &str); 2] = [
    ("status.showUntrackedFiles", "no"),
    ("advice.statusHints", "false"),
];

/// The keys copied from the public repository's local configuration where it sets them.
const COPIED: [&str; 2] = ["user.name", "user.email"];

/// A path the relative work tree is computed from that has no canonical form, and why.
pub struct Unresolved {
    pub path: PathBuf,
    pub cause: io::Error,
}

/// The relative work tree of this worktree, as bytes: from `dupe` in the canonical Git
/// directory to the canonical root.
pub fn relative_work_tree(workspace: &Workspace) -> Result<Vec<u8>, Unresolved> {
    let canonical = |path: &Path| {
        fs::canonicalize(path).map_err(|cause| Unresolved {
            path: path.to_path_buf(),
            cause,
        })
    };
    let private = private_directory(&canonical(workspace.git_directory())?);
    let root = canonical(workspace.root())?;
    Ok(relative(&private, &root))
}

/// The lexical path from the absolute `from` to the absolute `to`: one `..` for each
/// component of `from` past the components they share, then the rest of `to`.
fn relative(from: &Path, to: &Path) -> Vec<u8> {
    let from: Vec<Component> = from.components().collect();
    let to: Vec<Component> = to.components().collect();
    let shared = from
        .iter()
        .zip(&to)
        .take_while(|(one, other)| one == other)
        .count();
    let up = from[shared..].iter().map(|_| &b".."[..]);
    let down = to[shared..]
        .iter()
        .map(|component| bytes::of(component.as_os_str()));
    let parts: Vec<&[u8]> = up.chain(down).collect();
    if parts.is_empty() {
        return b".".to_vec();
    }
    parts.join(&b'/')
}

/// `core.worktree` as the private repository's own files record it, from one private
/// `config -z --show-scope --no-includes --get-all`: the values of scope `local`, its
/// `config`, and `worktree`, its `config.worktree`, which Git reads after the local file
/// and only where `extensions.worktreeConfig` is set. Git takes the work tree from these
/// alone, following no `include.path`: a value given before `dupe` with `-c`, set in the
/// global or system configuration, or reached through an include is no record of the
/// repository's and is left out. Git answers nothing, exit 1, for an unset key, in a
/// repository without a configuration file too (S12).
pub struct Recorded {
    values: Vec<Vec<u8>>,
    in_worktree_file: bool,
}

/// Reads `core.worktree` in the private repository at `private`.
pub fn recorded(workspace: &Workspace, private: &Path) -> Result<Recorded, Failed> {
    let words = [
        "config",
        "-z",
        "--show-scope",
        "--no-includes",
        "--get-all",
        "core.worktree",
    ];
    let run = Run::private(private, workspace.root(), words).capture_output();
    let answer = step(run, &[0, 1])?;
    let scoped = match answer.end {
        End::Code(0) => records::config_scoped_values(&answer.stdout).unwrap_or_default(),
        _ => Vec::new(),
    };
    let own: Vec<_> = scoped
        .into_iter()
        .filter(|(scope, _)| matches!(*scope, b"local" | b"worktree"))
        .collect();
    Ok(Recorded {
        in_worktree_file: own.iter().any(|(scope, _)| *scope == b"worktree"),
        values: own.into_iter().map(|(_, value)| value.to_vec()).collect(),
    })
}

impl Recorded {
    /// Whether no value is recorded, or one is not spelled as `work_tree`: an `init` that
    /// did not finish, or a linked worktree moved since. Every value counts, not only the
    /// one Git reads: a template's `config.worktree` can name the root before any key is
    /// set, Git's own `init` having left its absolute path in the local file.
    pub fn unfinished(&self, work_tree: &[u8]) -> bool {
        self.values.is_empty() || self.values.iter().any(|value| value != work_tree)
    }

    /// The scopes `core.worktree` is written with, so that every value Git reads is the
    /// one written: `--local`, and `--worktree`, which writes `config.worktree`, where Git
    /// read a value from there. Only there: without the extension `--worktree` would mean
    /// the local file, but Git refuses it, exit 128, in a repository with a worktree of
    /// its own, as `git dupe worktree add` makes one.
    fn scopes(&self) -> &'static [&'static str] {
        if self.in_worktree_file {
            &["--local", "--worktree"]
        } else {
            &["--local"]
        }
    }
}

/// Sets every key, `core.worktree` last, to `work_tree`, `core.worktree` in each file
/// `recorded` read it from. The public values are read before the first write, so that a
/// failed read writes nothing.
pub fn set(
    workspace: &Workspace,
    private: &Path,
    work_tree: &[u8],
    recorded: &Recorded,
) -> Result<(), Failed> {
    let mut copied = Vec::new();
    for key in COPIED {
        if let Some(value) = public_local(key)? {
            copied.push(("--local", key, value));
        }
    }
    let included = included_keys(workspace, private)?;
    let keys = FIXED
        .map(|(key, value)| ("--local", key, value.as_bytes().to_vec()))
        .into_iter()
        .chain(copied)
        .filter(|(_, key, _)| !included.contains(key.to_ascii_lowercase().as_bytes()))
        .chain(
            recorded
                .scopes()
                .iter()
                .map(|&scope| (scope, "core.worktree", work_tree.to_vec())),
        );
    for (scope, key, value) in keys {
        let words: [Word; 5] = [
            "config".into(),
            scope.into(),
            "--replace-all".into(),
            key.into(),
            value.into(),
        ];
        step(Run::private(private, workspace.root(), words), &[0])?;
    }
    Ok(())
}

/// The keys, as Git spells them in a `--list`, whose effective value a file the private
/// `config` includes gives, from one private `config -z --local --list` with `--includes`
/// and one with `--no-includes` (`Holds/G1`, S12).
fn included_keys(workspace: &Workspace, private: &Path) -> Result<BTreeSet<Vec<u8>>, Failed> {
    // `--list`, unlike `--get`, fails where the file is gone, as it can be after a kill.
    if fs::symlink_metadata(private.join("config")).is_err() {
        return Ok(BTreeSet::new());
    }
    let list = |includes: &'static str| {
        let words = ["config", "-z", "--local", includes, "--list"];
        step(
            Run::private(private, workspace.root(), words).capture_output(),
            &[0],
        )
    };
    let with = list("--includes")?;
    let without = list("--no-includes")?;
    Ok(takes_from_an_include(&with.stdout, &without.stdout))
}

/// The keys of `with`, the records under `--includes`, whose last value is missing from
/// `without`, those of the file alone, or differs from the last value there. A key's last
/// record is the value Git takes (S12), and a value of none, a key alone, is one too.
fn takes_from_an_include(with: &[u8], without: &[u8]) -> BTreeSet<Vec<u8>> {
    let last =
        |listing| -> BTreeMap<&[u8], Option<&[u8]>> { records::config_records(listing).collect() };
    let own = last(without);
    last(with)
        .into_iter()
        .filter(|(key, value)| own.get(key) != Some(value))
        .map(|(key, _)| key.to_vec())
        .collect()
}

/// The value of `key` in the public repository's local configuration, from one public
/// `config -z --local --includes --get`, whose exit 1 means it is not set there. Without
/// `--includes`, Git reads `--local` as the one file and skips what it includes.
fn public_local(key: &str) -> Result<Option<Vec<u8>>, Failed> {
    let words = ["config", "-z", "--local", "--includes", "--get", key];
    let run = Run::public(words).capture_output();
    let answer = step(run, &[0, 1])?;
    if answer.end != End::Code(0) {
        return Ok(None);
    }
    Ok(Some(records::config_value(&answer.stdout).to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_kept_by_an_include_where_the_files_own_last_value_is_not_the_one_git_takes() {
        // Reading order: what an include gives stands at its record, the last value decides.
        let with = b"a.b\nown\0a.b\nincluded\0c.d\nsame\0c.d\nsame\0e.f\nincluded\0\
                     g.h\nincluded\0g.h\nown\0flag.only\0";
        let without = b"a.b\nown\0c.d\nsame\0g.h\nown\0";
        let kept: Vec<&[u8]> = vec![b"a.b", b"e.f", b"flag.only"];
        assert_eq!(
            takes_from_an_include(with, without),
            kept.into_iter().map(<[u8]>::to_vec).collect()
        );
        assert!(takes_from_an_include(b"", b"").is_empty());
    }

    #[test]
    fn the_relative_work_tree_leaves_the_private_git_directory_for_the_root() {
        for (from, to, expected) in [
            // The main worktree, wherever the project stands.
            (&b"/w/repo/.git/dupe"[..], &b"/w/repo"[..], &b"../.."[..]),
            (b"/.git/dupe", b"/", b"../.."),
            // A linked worktree beside the project, of an ordinary repository and of a
            // bare one, and one elsewhere.
            (
                b"/w/repo/.git/worktrees/agent/dupe",
                b"/w/project-agent",
                b"../../../../../project-agent",
            ),
            (
                b"/w/bare.git/worktrees/one/dupe",
                b"/w/one",
                b"../../../../one",
            ),
            (
                b"/w/repo/.git/worktrees/l/dupe",
                b"/elsewhere/deep/l",
                b"../../../../../../elsewhere/deep/l",
            ),
            // A linked worktree inside the main one's directory, below its root.
            (
                b"/w/repo/.git/worktrees/in/dupe",
                b"/w/repo/trees/in",
                b"../../../../trees/in",
            ),
            // Bytes, never text.
            (
                b"/w/caf\xe9/.git/worktrees/x\xff/dupe",
                b"/w/x\xff",
                b"../../../../../x\xff",
            ),
        ] {
            // A row of bytes that are no name here stands for nothing on disk.
            let (Some(from), Some(to)) = (bytes::name(from), bytes::name(to)) else {
                continue;
            };
            assert_eq!(relative(Path::new(from), Path::new(to)), expected);
        }
    }

    #[test]
    fn a_path_without_a_canonical_form_is_named_and_nothing_is_computed() {
        let scratch =
            std::env::temp_dir().join(format!("git-dupe-settings-{}", std::process::id()));
        let common = scratch.join("repo/.git");
        let root = scratch.join("moved-away");
        let linked = Workspace::linked_at(&common, b"one", &root);
        // Neither the Git directory nor the root exists: the Git directory is named first.
        match relative_work_tree(&linked) {
            Err(unresolved) => assert_eq!(unresolved.path, linked.git_directory()),
            Ok(found) => panic!("computed {}", found.escape_ascii()),
        }
        fs::create_dir_all(common.join("worktrees/one")).unwrap();
        match relative_work_tree(&linked) {
            Err(unresolved) => assert_eq!(unresolved.path, root),
            Ok(found) => panic!("computed {}", found.escape_ascii()),
        }
        fs::create_dir(&root).unwrap();
        assert_eq!(
            relative_work_tree(&linked).ok(),
            Some(b"../../../../../moved-away".to_vec())
        );
        fs::remove_dir_all(&scratch).unwrap();
    }

    #[test]
    fn components_are_compared_whole() {
        // `/w/re` is no ancestor of `/w/repo`.
        assert_eq!(
            relative(
                Path::new("/w/repo/.git/worktrees/re/dupe"),
                Path::new("/w/re")
            ),
            b"../../../../../re"
        );
    }
}
