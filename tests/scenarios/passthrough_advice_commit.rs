//! The hints after a private `commit` that exits 1 (`Holds/G19`, S12): with `--amend`, or a
//! prefix Git reads as it, and not `--allow-empty`, one `hint:` naming `--allow-empty` added
//! to the command as typed and `git dupe reset HEAD^`, in words conditioned on the amend that would have left the
//! commit empty, which exit 1 alone does not prove, so that a hook's refusal draws it too;
//! without `--amend`, while a cherry-pick stands stopped, the cherry-pick's line, or, where
//! a rebase stands stopped at a pick that became empty, which keeps `CHERRY_PICK_HEAD` too,
//! the rebase's, a stopped `am` beside the pick being no rebase. A commit during a stopped
//! revert gets none: Git's advice there names only the revert's own commands, which fail
//! in the project's repository. The amend's hint asks nothing; the other asks
//! `CHERRY_PICK_HEAD`, and only after exit 1 (G22), and reads a rebase's stop by `lstat`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, STOPPED_REBASE_HINT as REBASE, Scenario, asked_after, hinted, private_add,
    private_alias, private_commit, text, under_each_release, unhinted, write, write_executable,
};

const EMPTY_AMEND: [&[u8]; 2] = [
    b"the same 'git dupe commit' as typed with --allow-empty added",
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

/// `--allow-empty` added to the command as typed keeps its `--only`, where a bare `commit
/// --amend --allow-empty` would take what else is staged.
#[test]
fn the_amend_hints_command_is_the_one_typed_and_commits_only_what_it_named() {
    under_each_release(|s| {
        let root = fixture(s);
        write(&root, ".gitdupe", b"notes.txt\nother.txt\n");
        private_add(s, &root, ".gitdupe");
        private_commit(s, &root);
        write(&root, "other.txt", b"staged\n");
        s.git(["dupe", "add", "--", "other.txt"])
            .from(&root)
            .succeeds();
        // The commit before the last holds what the working tree now holds in `notes.txt`.
        s.private(&root)
            .git(["reset", "-q", "--soft", "start"])
            .succeeds();
        write(&root, "notes.txt", b"two\n");
        let typed = ["commit", "--amend", "--only", "--no-edit", "notes.txt"];
        let refused = hinted(s, &root, &typed, &EMPTY_AMEND);
        assert_eq!(refused.end, End::Code(1), "{refused:?}");

        let amended = [
            "commit",
            "--amend",
            "--only",
            "--no-edit",
            "--allow-empty",
            "notes.txt",
        ];
        s.git(["dupe"].iter().chain(&amended))
            .from(&root)
            .succeeds();
        let in_head = s
            .private(&root)
            .git(["cat-file", "-e", "HEAD:other.txt"])
            .run();
        assert_ne!(in_head.end, End::Code(0), "{in_head:?}");
    });
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

        // A prefix Git reads as `--amend` draws the same line; `--a` is ambiguous to it.
        for word in ["--amen", "--ame", "--am"] {
            let refused = hinted(s, &root, &["commit", word, "--no-edit"], &EMPTY_AMEND);
            assert_eq!(refused.end, End::Code(1), "{word}: {refused:?}");
        }
        let ambiguous = unhinted(s, &root, &["commit", "--a", "--no-edit"]);
        assert_eq!(ambiguous.end, End::Code(129), "{ambiguous:?}");

        // Under `--dry-run` the named command would amend nothing.
        for word in ["--amend", "--amen"] {
            let dry = unhinted(s, &root, &["commit", word, "--dry-run", "--no-edit"]);
            assert_eq!(dry.end, End::Code(1), "{word}: {dry:?}");
        }

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

#[test]
fn a_commit_during_a_rebase_stopped_at_a_pick_that_became_empty_ends_with_the_rebases_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        // A branch from the second commit whose first commit makes the change the third made:
        // rebased onto `main` with every pick kept, that pick becomes empty, and `rebase -i`
        // stops there with `CHERRY_PICK_HEAD` standing.
        s.private(&root)
            .git(["switch", "-q", "-c", "side", "start~1"])
            .succeeds();
        for content in ["three", "four"] {
            write(&root, "notes.txt", format!("{content}\n").as_bytes());
            private_add(s, &root, "notes.txt");
            // A message of its own, so that the first is not the very commit `main` holds.
            let message = format!("side {content}");
            s.private(&root)
                .git(["commit", "-qm", message.as_str()])
                .succeeds();
        }
        let stopped = s
            .private(&root)
            .git(["rebase", "-i", "--reapply-cherry-picks", "main"])
            .run();
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        let picking = s
            .private(&root)
            .git(["rev-parse", "-q", "--verify", "CHERRY_PICK_HEAD"])
            .run();
        assert_eq!(picking.end, End::Code(0), "{picking:?}");
        let refused = hinted(s, &root, &["commit", "-m", "x"], &REBASE);
        assert_eq!(refused.end, End::Code(1), "{refused:?}");
        // The rebase's directory is read by `lstat`: one question, as for a cherry-pick.
        asked_after(s, &root, &["commit", "-m", "x"], End::Code(1), &[PICK]);
        // A cherry-pick run there is no stopped cherry-pick: no hint, the same question.
        let continued = unhinted(s, &root, &["cherry-pick", "--continue"]);
        assert_ne!(continued.end, End::Code(0), "{continued:?}");
        let words = ["cherry-pick", "--continue"];
        asked_after(s, &root, &words, continued.end, &[PICK]);
        // The rebase's `--skip` leaves the empty pick out, and the rebase ends.
        s.git(["dupe", "rebase", "--skip"]).from(&root).succeeds();
        assert_eq!(fs::read(root.join("notes.txt")).unwrap(), b"four\n");
        assert!(!root.join(".git/dupe/rebase-merge").exists());

        // Beside a stopped `am`, which keeps `rebase-apply` holding a file of its own and not
        // a rebase's, a cherry-pick that becomes empty stops a pick of its own: it draws the
        // cherry-pick's line, and so does a commit while it stands, whose commands Git runs
        // while the `am` stands.
        s.private(&root).git(["switch", "-q", "main"]).succeeds();
        let patch = s.dir().join("two.patch");
        let made = s
            .private(&root)
            .git(["format-patch", "-1", "--stdout", "start~1"])
            .succeeds();
        fs::write(&patch, made.stdout).unwrap();
        let applying = s.private(&root).git(["am", text(&patch)]).run();
        assert_eq!(applying.end, End::Code(128), "{applying:?}");
        let empty = hinted(s, &root, &["cherry-pick", "HEAD"], &CHERRY_PICK);
        assert_eq!(empty.end, End::Code(1), "{empty:?}");
        let beside = hinted(s, &root, &["commit", "-m", "x"], &CHERRY_PICK);
        assert_eq!(beside.end, End::Code(1), "{beside:?}");
        asked_after(s, &root, &["commit", "-m", "x"], End::Code(1), &[PICK]);
        // The line's `--abort` ends the pick, and the `am` stands still.
        s.git(["dupe", "cherry-pick", "--abort"])
            .from(&root)
            .succeeds();
        assert!(root.join(".git/dupe/rebase-apply/applying").exists());
        s.private(&root).git(["am", "--abort"]).succeeds();
    });
}
