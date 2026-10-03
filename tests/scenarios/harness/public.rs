//! The public repository across a `git dupe` command: everything inside the project's
//! `.git` but `.git/dupe` and `.git/info/exclude`, the two places git-dupe writes there
//! of its own accord once `.git/info` exists, must be byte for byte what it was (G5, R2).
//! The story's own public commands change the public repository rightly, so the
//! comparison is made around each `git dupe` command, never across a whole story. When a
//! story ends, the public repository holds no object of the private history and no path
//! the private index tracks: the listings that show it are here.

use std::path::Path;

use super::output::{Output, records};
use super::scenario::{Git, Scenario};
use super::tree::Tree;

/// The public `.git` of the workspace at `dir`, without `.git/dupe` and
/// `.git/info/exclude`.
pub fn public_git(dir: &Path) -> Tree {
    let git = dir.join(".git");
    let written = [git.join("dupe"), git.join("info/exclude")];
    Tree::of(&git).without(&written.each_ref().map(|path| path.as_path()))
}

/// Runs `git`, a `git dupe` command in the workspace at `dir`, and returns what it
/// printed, once it has left the public `.git` as it was but for the private repository
/// and the exclude file.
pub fn leaving_public_git(dir: &Path, git: Git<'_>) -> Output {
    let before = public_git(dir);
    let output = git.run();
    let changed = before.changed_in(&public_git(dir));
    assert!(
        changed.is_empty(),
        "the public repository changed: {changed:?}; {output:?}"
    );
    output
}

pub fn privately_tracked(s: &Scenario, dir: &Path) -> Vec<Vec<u8>> {
    let tracked = s.private(dir).git(["ls-files", "-z"]).succeeds();
    records(&tracked.stdout, 0)
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

pub fn publicly_tracked(s: &Scenario, dir: &Path) -> Vec<Vec<u8>> {
    let tracked = s.git(["ls-files", "-z"]).from(dir).succeeds();
    records(&tracked.stdout, 0)
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

pub const EVERY_OBJECT: [&str; 3] = [
    "cat-file",
    "--batch-all-objects",
    "--batch-check=%(objectname)",
];

/// Of `ids`, one per line, those the public repository holds.
pub fn held_publicly(s: &Scenario, dir: &Path, ids: &[u8]) -> Vec<Vec<u8>> {
    let checked = s
        .git(["cat-file", "--batch-check=%(objectname)"])
        .from(dir)
        .input(ids)
        .succeeds();
    records(&checked.stdout, b'\n')
        .into_iter()
        .filter(|line| !line.ends_with(b" missing"))
        .map(<[u8]>::to_vec)
        .collect()
}
