//! The keys `init` gives the private repository (F2, G1), and how it tells whether they
//! are set: one private `config -z --local --list`, whose `core.worktree` is the
//! relative work tree only after the last key of a finished `init`. Then none is set
//! again, so that a key the developer edited afterward stays as they left it (S6).
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
//! config` refuse, and every later `init` with it. The identity is what the public
//! repository's local configuration holds, a file it includes counted, where it holds
//! one: `config --local --includes --get` answers nothing, exit 1, for a key set only
//! globally.

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use super::step;
use crate::keeper::Failed;
use crate::runner::locate::Workspace;
use crate::runner::records;
use crate::runner::{End, Run};

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
    let private = canonical(workspace.git_directory())?.join("dupe");
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
        .map(|component| component.as_os_str().as_bytes());
    let parts: Vec<&[u8]> = up.chain(down).collect();
    if parts.is_empty() {
        return b".".to_vec();
    }
    parts.join(&b'/')
}

/// Whether the private configuration lacks `core.worktree` spelled as `work_tree`: an
/// `init` that did not finish, or a linked worktree moved since. A repository without a
/// configuration file has none of the keys, and Git refuses to list a file that is not
/// there.
pub fn unfinished(workspace: &Workspace, private: &Path, work_tree: &[u8]) -> Result<bool, Failed> {
    if fs::symlink_metadata(private.join("config")).is_err() {
        return Ok(true);
    }
    let run = Run::private(
        private,
        workspace.root(),
        ["config", "-z", "--local", "--list"],
    )
    .capture_output();
    let listing = step(run, &[0])?.stdout;
    Ok(worktree(&listing) != Some(work_tree))
}

/// Sets every key, `core.worktree` last, to `work_tree`. The public values are read
/// before the first write, so that a failed read writes nothing.
pub fn set(workspace: &Workspace, private: &Path, work_tree: &[u8]) -> Result<(), Failed> {
    let mut copied = Vec::new();
    for key in COPIED {
        if let Some(value) = public_local(key)? {
            copied.push((key, value));
        }
    }
    let keys = FIXED
        .map(|(key, value)| (key, value.as_bytes().to_vec()))
        .into_iter()
        .chain(copied)
        .chain([("core.worktree", work_tree.to_vec())]);
    for (key, value) in keys {
        let words = [
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("--replace-all"),
            OsStr::new(key),
            OsStr::from_bytes(&value),
        ];
        step(Run::private(private, workspace.root(), words), &[0])?;
    }
    Ok(())
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

/// The last `core.worktree` of a `config -z --list`, as Git reads it; `None` when it is
/// absent or has no value.
fn worktree(listing: &[u8]) -> Option<&[u8]> {
    records::config_records(listing)
        .filter(|(key, _)| *key == b"core.worktree")
        .last()
        .and_then(|(_, value)| value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_core_worktree_decides() {
        assert_eq!(worktree(b"core.worktree\n../..\0"), Some(&b"../.."[..]));
        assert_eq!(
            worktree(b"core.worktree\n/w/repo\0core.worktree\n../..\0"),
            Some(&b"../.."[..])
        );
        assert_eq!(
            worktree(b"core.worktree\n../..\0core.worktree\n/w/repo\0"),
            Some(&b"/w/repo"[..])
        );
        assert_eq!(worktree(b"core.worktree\0"), None);
        assert_eq!(worktree(b"core.bare\nfalse\0"), None);
        // A value is never a key: `x.y` holding the text of another record.
        assert_eq!(worktree(b"x.y\ncore.worktree\n../..\0"), None);
    }

    fn relative_of(from: &[u8], to: &[u8]) -> Vec<u8> {
        relative(
            Path::new(OsStr::from_bytes(from)),
            Path::new(OsStr::from_bytes(to)),
        )
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
            assert_eq!(relative_of(from, to), expected, "{}", from.escape_ascii());
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
            relative_of(b"/w/repo/.git/worktrees/re/dupe", b"/w/re"),
            b"../../../../../re"
        );
    }
}
