//! The hint git-dupe writes after a Git run of the user's words whose outcome Git's own
//! advice names a plain `git` command for (`Holds/G19`, `Holds/G18`, G25): the words run
//! three times from one state at one path, as the release's own Git against the private
//! repository, as `git dupe git`, and as `git dupe`, and git-dupe's output compared with
//! Git's. A scenario names no Git text: the state it builds decides what Git prints.

use std::fs;
use std::path::{Path, PathBuf};

use super::files::copy;
use super::fresh::FreshDirectory;
use super::output::{End, Output, names};
use super::scenario::Scenario;
use super::tree::Tree;

/// The workspace at `root` copied aside, to be put back at the same path, so that the
/// paths Git's output names are the same in every execution.
struct Saved {
    root: PathBuf,
    tree: Tree,
    aside: FreshDirectory,
}

impl Saved {
    fn of(root: &Path) -> Saved {
        let aside = FreshDirectory::create();
        copy(root, &aside.path().join("workspace"));
        Saved {
            root: root.to_path_buf(),
            tree: Tree::of(root),
            aside,
        }
    }

    /// Puts the copy back where a run changed anything below the root, so that a scenario
    /// that checks afterwards that words changed nothing observes every execution.
    fn put_back(&self) {
        if self.tree.changed_in(&Tree::of(&self.root)).is_empty() {
            return;
        }
        fs::remove_dir_all(&self.root)
            .unwrap_or_else(|cause| panic!("{}: {cause}", self.root.display()));
        copy(&self.aside.path().join("workspace"), &self.root);
    }
}

/// How git-dupe ends after a Git child that ended as `end`: with its status, or with 128
/// plus the signal that killed it.
fn as_git_dupe_ends(end: End) -> End {
    match end {
        End::Signal(signal) => End::Code(128 + signal),
        code => code,
    }
}

/// `git dupe <words>` from the root of the workspace at `root`, and Git's own run of the
/// words against its private repository after `-c help.autocorrect=0`, as git-dupe runs
/// them, from the same state: the workspace is put back between the runs, and
/// `git dupe git <words>`, guarded by nothing, ends as Git's own run (G20). The words may
/// change only what lies below the root; the scenario goes on from the state `git dupe`
/// left.
fn ours_and_gits(s: &Scenario, root: &Path, words: &[&str]) -> (Output, Output) {
    let saved = Saved::of(root);
    let mut gits = s
        .private(root)
        .git(
            ["-c", "help.autocorrect=0"]
                .into_iter()
                .chain(words.iter().copied()),
        )
        .run();
    gits.end = as_git_dupe_ends(gits.end);
    saved.put_back();
    let unguarded = s
        .git(["dupe", "git"].into_iter().chain(words.iter().copied()))
        .from(root)
        .run();
    assert_eq!(unguarded, gits, "git dupe git {words:?}");
    saved.put_back();
    let ours = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(root)
        .run();
    (ours, gits)
}

/// `git dupe <words>` ends as Git's own run of them, with every byte of its output, and
/// then one `hint:` line naming each of `parts`, before anything settle adds; its output.
pub fn hinted(s: &Scenario, root: &Path, words: &[&str], parts: &[&[u8]]) -> Output {
    let (ours, gits) = ours_and_gits(s, root, words);
    assert_eq!(ours.end, gits.end, "{words:?}: {ours:?}");
    assert_eq!(ours.stdout, gits.stdout, "{words:?}: {ours:?}");
    let hint = ours
        .stderr
        .strip_prefix(gits.stderr.as_slice())
        .and_then(|after| after.strip_prefix(b"hint: "))
        .and_then(|after| after.strip_suffix(b"\n"))
        .filter(|line| !line.contains(&b'\n'))
        .unwrap_or_else(|| panic!("{words:?}: not Git's output and one hint: {ours:?}"));
    for part in parts {
        names(hint, part);
    }
    ours
}

/// `git dupe <words>` ends as Git's own run of them, with every byte of its output and
/// nothing of git-dupe's; its output.
pub fn unhinted(s: &Scenario, root: &Path, words: &[&str]) -> Output {
    let (ours, gits) = ours_and_gits(s, root, words);
    assert_eq!(ours, gits, "{words:?}");
    ours
}
