//! The keys `init` gives the private repository (F2, G1), and how it tells whether they
//! are set: one private `config -z --local --list`, whose `core.worktree` is `../..`
//! only after the last key of a finished `init`. Then none is set again, so that a key
//! the developer edited afterward stays as they left it (S6).
//!
//! Each key is its own `git config --replace-all` run, in a fixed order with
//! `core.worktree` last: a key a template set more than once would make a plain `git
//! config` refuse, and every later `init` with it. The identity is what the public
//! repository's local configuration holds, a file it includes counted, where it holds
//! one: `config --local --includes --get` answers nothing, exit 1, for a key set only
//! globally.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::step;
use crate::keeper::Failed;
use crate::runner::locate::Workspace;
use crate::runner::records;
use crate::runner::{End, Run};

/// Where the working tree is recorded relative to `.git/dupe`: the root, wherever the
/// workspace is moved.
const WORKTREE: &[u8] = b"../..";

/// The keys every private repository gets, whatever a template put there.
const FIXED: [(&str, &str); 2] = [
    ("status.showUntrackedFiles", "no"),
    ("advice.statusHints", "false"),
];

/// The keys copied from the public repository's local configuration where it sets them.
const COPIED: [&str; 2] = ["user.name", "user.email"];

/// Whether the private configuration lacks `core.worktree` `../..`: an `init` that did
/// not finish. A repository without a configuration file has none of the keys, and Git
/// refuses to list a file that is not there.
pub fn unfinished(workspace: &Workspace, private: &Path) -> Result<bool, Failed> {
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
    Ok(worktree(&listing) != Some(WORKTREE))
}

/// Sets every key, `core.worktree` last. The public values are read before the first
/// write, so that a failed read writes nothing.
pub fn set(workspace: &Workspace, private: &Path) -> Result<(), Failed> {
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
        .chain([("core.worktree", WORKTREE.to_vec())]);
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
}
