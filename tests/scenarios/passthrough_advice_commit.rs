//! The hints after a private `commit` that exits 1 (`Holds/G19`, S12): with `--amend` and
//! not `--allow-empty`, one `hint:` naming `git dupe commit --amend --allow-empty` and
//! `git dupe reset HEAD^`, in words conditioned on the amend that would have left the
//! commit empty, which exit 1 alone does not prove, so that a hook's refusal draws it too;
//! without `--amend`, while a cherry-pick stands stopped, the cherry-pick's line. A commit
//! during a stopped revert gets none: Git's advice there names only the revert's own
//! commands, which fail in the project's repository. The amend's hint asks nothing; the
//! other asks `CHERRY_PICK_HEAD`, and only after exit 1 (G22).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, asked_after, hinted, private_add, private_alias, private_commit,
    under_each_release, unhinted, write, write_executable,
};

const EMPTY_AMEND: [&[u8]; 2] = [
    b"'git dupe commit --amend --allow-empty'",
    b"'git dupe reset HEAD^'",
];

const CHERRY_PICK: [&[u8]; 5] = [
    b"'git dupe add <path>'",
    b"'git dupe commit --allow-empty'",
    b"'git dupe cherry-pick --continue'",
    b"'git dupe cherry-pick --skip'",
    b"'git dupe cherry-pick --abort'",
];

/// The question of a stopped cherry-pick.
const PICK: &[&str] = &["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"];

/// An attached workspace whose private `main`, checked out and tagged `start`, holds three
/// commits of `notes.txt`, reading `one`, `two`, and `three`, `.gitdupe` committed in the
/// first; the commit editor succeeds.
fn fixture(s: &Scenario) -> PathBuf {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
        ("core.editor", "true"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let root = s.dir().join("workspace");
    s.repository(&root);
    s.init(&root);
    write(&root, ".gitdupe", b"notes.txt\n");
    private_add(s, &root, ".gitdupe");
    for content in ["one\n", "two\n", "three\n"] {
        write(&root, "notes.txt", content.as_bytes());
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
    }
    s.private(&root).git(["tag", "start"]).succeeds();
    s.git(["dupe", "status"]).from(&root).succeeds();
    root
}

/// Puts the private `main` of the workspace at `root` back at `start` and its index at
/// the parent's tree, so that an amend would leave the commit empty.
fn emptied(s: &Scenario, root: &Path) {
    s.private(root)
        .git(["reset", "-q", "--hard", "start"])
        .succeeds();
    s.private(root).git(["read-tree", "HEAD^"]).succeeds();
}

/// The commit a revision names in the repository a run of `git` reaches.
fn named(output: Output) -> Vec<u8> {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    output.stdout
}

#[test]
fn an_amend_that_refuses_ends_with_one_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        let project_head = || named(s.git(["rev-parse", "HEAD"]).from(&root).run());
        let before = project_head();
        emptied(s, &root);
        let refused = hinted(s, &root, &["commit", "--amend", "--no-edit"], &EMPTY_AMEND);
        assert_eq!(refused.end, End::Code(1), "{refused:?}");
        // Its commands do what it says, in the private repository.
        let parent = named(s.private(&root).git(["rev-parse", "HEAD^"]).run());
        s.git(["dupe", "reset", "HEAD^"]).from(&root).succeeds();
        assert_eq!(
            named(s.private(&root).git(["rev-parse", "HEAD"]).run()),
            parent
        );
        assert_eq!(fs::read(root.join("notes.txt")).unwrap(), b"three\n");
        assert_eq!(project_head(), before);
        emptied(s, &root);
        let amended = unhinted(
            s,
            &root,
            &["commit", "--amend", "--allow-empty", "--no-edit"],
        );
        assert_eq!(amended.end, End::Code(0), "{amended:?}");

        // `--allow-empty-message` is another option; an alias's expansion is read.
        emptied(s, &root);
        hinted(
            s,
            &root,
            &["commit", "--amend", "--allow-empty-message", "--no-edit"],
            &EMPTY_AMEND,
        );
        private_alias(s, &root, "ca", "commit --amend --no-edit");
        hinted(s, &root, &["ca"], &EMPTY_AMEND);
        unhinted(s, &root, &["commit", "--amend", "-h"]);

        // A hook's refusal exits 1 too: the line is conditioned, and `--allow-empty` keeps
        // it away.
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        let hook = root.join(".git/dupe/hooks/pre-commit");
        write_executable(s, &hook, b"#!/bin/sh\nexit 1\n");
        let hooked = hinted(s, &root, &["commit", "--amend", "--no-edit"], &EMPTY_AMEND);
        assert_eq!(hooked.end, End::Code(1), "{hooked:?}");
        let allowed = unhinted(
            s,
            &root,
            &["commit", "--amend", "--allow-empty", "--no-edit"],
        );
        assert_eq!(allowed.end, End::Code(1), "{allowed:?}");
        fs::remove_file(&hook).unwrap();

        // At a rebase's stop the word run is `commit`: the amend's line, not the stop's.
        let stopped = s
            .private(&root)
            .git(["rebase", "-x", "false", "HEAD~1"])
            .run();
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        assert!(root.join(".git/dupe/rebase-merge").is_dir());
        s.private(&root).git(["read-tree", "HEAD^"]).succeeds();
        let at_the_stop = hinted(s, &root, &["commit", "--amend", "--no-edit"], &EMPTY_AMEND);
        assert_eq!(at_the_stop.end, End::Code(1), "{at_the_stop:?}");
        s.private(&root).git(["rebase", "--abort"]).succeeds();

        // Decided from the words and the exit: no question.
        emptied(s, &root);
        asked_after(
            s,
            &root,
            &["commit", "--amend", "--no-edit"],
            End::Code(1),
            &[],
        );
    });
}

#[test]
fn a_commit_ends_with_the_hint_of_a_stopped_cherry_pick_alone() {
    under_each_release(|s| {
        let root = fixture(s);
        // A pick of what `HEAD` already holds stops as empty; a commit then refuses.
        let empty = s.private(&root).git(["cherry-pick", "HEAD"]).run();
        assert_eq!(empty.end, End::Code(1), "{empty:?}");
        let picked = hinted(s, &root, &["commit", "-m", "picked"], &CHERRY_PICK);
        assert_eq!(picked.end, End::Code(1), "{picked:?}");
        // One question where a cherry-pick stands.
        asked_after(s, &root, &["commit", "-m", "picked"], End::Code(1), &[PICK]);
        // Git refuses an amend while a pick stands, exit 128: neither entry follows, and
        // nothing is asked.
        let amend = ["commit", "--amend", "--no-edit"];
        let refused = unhinted(s, &root, &amend);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        asked_after(s, &root, &amend, End::Code(128), &[]);
        s.private(&root).git(["cherry-pick", "--abort"]).succeeds();

        // A revert resolved to what `HEAD` holds is empty: a commit refuses while it stands.
        let reverted = s.private(&root).git(["revert", "HEAD~1"]).run();
        assert_eq!(reverted.end, End::Code(1), "{reverted:?}");
        // Over the unresolved conflict Git refuses the commit with 128: no hint, no question.
        let unresolved = ["commit", "-m", "x"];
        let refused = unhinted(s, &root, &unresolved);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        asked_after(s, &root, &unresolved, End::Code(128), &[]);
        write(&root, "notes.txt", b"three\n");
        private_add(s, &root, "notes.txt");
        // Resolved, the commit exits 1 while the revert stands: Git's advice names only the
        // revert's own commands, so no hint follows, and only the cherry-pick is asked.
        let refused = unhinted(s, &root, &["commit", "-m", "x"]);
        assert_eq!(refused.end, End::Code(1), "{refused:?}");
        asked_after(s, &root, &["commit", "-m", "x"], End::Code(1), &[PICK]);
        let skipped = unhinted(s, &root, &["revert", "--skip"]);
        assert_eq!(skipped.end, End::Code(0), "{skipped:?}");

        // Nothing stopped: nothing to commit exits 1 with no hint, the cherry-pick asked.
        let nothing = ["commit", "-m", "nothing"];
        let unstopped = unhinted(s, &root, &nothing);
        assert_eq!(unstopped.end, End::Code(1), "{unstopped:?}");
        asked_after(s, &root, &nothing, End::Code(1), &[PICK]);
        // After exit 0, nothing is asked.
        write(&root, "notes.txt", b"staged\n");
        private_add(s, &root, "notes.txt");
        let dry = unhinted(s, &root, &["commit", "--dry-run"]);
        assert_eq!(dry.end, End::Code(0), "{dry:?}");
        asked_after(s, &root, &["commit", "--dry-run"], End::Code(0), &[]);
    });
}
