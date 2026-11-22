//! The hint after a private `merge` that left a merge in progress (`Holds/G19`, S12): one
//! stopped on a conflict, exit 1, and one Git refused because the first stands, exit 128,
//! end with Git's own output and then one `hint:` naming `git dupe add <path>`,
//! `git dupe merge --continue`, and `git dupe merge --abort`, and saying that plain
//! `git add` acts on the project's repository. A merge Git refused before starting, over a
//! local change or a word that names no commit, and a `--squash` that stopped, leave no
//! merge in progress and get none. The commands named, run as written beside a rebase
//! paused in the project's repository, conclude or give up the private merge and leave the
//! project as it was. The one question, `rev-parse -q --verify MERGE_HEAD`, follows only a
//! run that did not exit 0 (G22).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, MERGE_HEAD, MERGE_IN_PROGRESS_HINT as MERGE, Scenario, asked_after, dupe_succeeds, hinted,
    private_add, private_alias, private_commit, private_head, under_each_release, unhinted, write,
};

/// An attached workspace whose private `main`, checked out, and `side` each changed
/// `notes.txt` from `start`, so that `merge side` stops on the conflict; the commit editor
/// succeeds, and the public repository holds `README.md`.
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
    write(&root, "README.md", b"start\n");
    s.git(["add", "README.md"]).from(&root).succeeds();
    s.commit_public(&root);
    s.init(&root);
    write(&root, ".gitdupe", b"notes.txt\n");
    private_add(s, &root, ".gitdupe");
    for (switch, content) in [
        (&[][..], "start\n"),
        (&["switch", "-q", "-c", "side"], "side\n"),
        (&["switch", "-q", "main"], "main\n"),
    ] {
        if !switch.is_empty() {
            dupe_succeeds(s, &root, switch);
        }
        write(&root, "notes.txt", content.as_bytes());
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
    }
    root
}

/// Whether a merge is in progress in the private repository of the workspace at `root`.
fn merging(s: &Scenario, root: &Path) -> bool {
    let asked = s.private(root).git(MERGE_HEAD).run();
    asked.end == End::Code(0)
}

#[test]
fn a_merge_stopped_on_a_conflict_ends_with_one_hint_under_every_release() {
    under_each_release(|s| {
        let root = fixture(s);
        let stopped = hinted(s, &root, &["merge", "--no-edit", "side"], &MERGE);
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        assert!(merging(s, &root));
        // Refused while the first stands, and continued with the conflict unresolved.
        let again = hinted(s, &root, &["merge", "--no-edit", "side"], &MERGE);
        assert_ne!(again.end, End::Code(0), "{again:?}");
        let continued = hinted(s, &root, &["merge", "--continue"], &MERGE);
        assert_ne!(continued.end, End::Code(0), "{continued:?}");
        // A help request: Git's output alone.
        unhinted(s, &root, &["merge", "-h"]);
        let aborted = unhinted(s, &root, &["merge", "--abort"]);
        assert_eq!(aborted.end, End::Code(0), "{aborted:?}");
        assert!(!merging(s, &root));
        // Through an alias, the words it reaches decide.
        private_alias(s, &root, "together", "merge --no-edit");
        let aliased = hinted(s, &root, &["together", "side"], &MERGE);
        assert_eq!(aliased.end, End::Code(1), "{aliased:?}");
    });
}

#[test]
fn a_merge_that_leaves_no_merge_in_progress_gets_no_hint_under_every_release() {
    under_each_release(|s| {
        let root = fixture(s);
        // A word that names no commit.
        let nothing = unhinted(s, &root, &["merge", "nosuch"]);
        assert_ne!(nothing.end, End::Code(0), "{nothing:?}");
        // A local change in the way: Git refuses before starting.
        write(&root, "notes.txt", b"edited\n");
        let refused = unhinted(s, &root, &["merge", "--no-edit", "side"]);
        assert_ne!(refused.end, End::Code(0), "{refused:?}");
        assert!(!merging(s, &root));
        s.private(&root)
            .git(["checkout", "--", "notes.txt"])
            .succeeds();
        // A squash stopped on the conflict keeps no `MERGE_HEAD`, and `merge --continue`
        // would fail: no line names it.
        let squashed = unhinted(s, &root, &["merge", "--squash", "side"]);
        assert_ne!(squashed.end, End::Code(0), "{squashed:?}");
        assert!(!merging(s, &root));
    });
}

#[test]
fn the_commands_named_conclude_or_give_up_the_merge_and_leave_the_project_as_it_was() {
    under_each_release(|s| {
        let root = fixture(s);
        // The project's own rebase, paused at a conflict resolved and staged.
        s.git(["switch", "-q", "-c", "public-side"])
            .from(&root)
            .succeeds();
        write(&root, "README.md", b"side\n");
        s.git(["add", "README.md"]).from(&root).succeeds();
        s.commit_public(&root);
        s.git(["switch", "-q", "main"]).from(&root).succeeds();
        write(&root, "README.md", b"main\n");
        s.git(["add", "README.md"]).from(&root).succeeds();
        s.commit_public(&root);
        let paused = s.git(["rebase", "public-side"]).from(&root).run();
        assert_eq!(paused.end, End::Code(1), "{paused:?}");
        write(&root, "README.md", b"resolved\n");
        s.git(["add", "README.md"]).from(&root).succeeds();
        let public_rebase = root.join(".git/rebase-merge");
        assert!(public_rebase.is_dir());
        let public_index = fs::read(root.join(".git/index")).unwrap();
        let public_head = s.git(["rev-parse", "HEAD"]).from(&root).succeeds().stdout;
        let project_as_it_was = |s: &Scenario| {
            assert!(public_rebase.is_dir());
            assert_eq!(fs::read(root.join(".git/index")).unwrap(), public_index);
            let head = s.git(["rev-parse", "HEAD"]).from(&root).succeeds().stdout;
            assert_eq!(head, public_head);
        };

        // Given up: the private branch and file as they were before the merge.
        let before = private_head(s, &root);
        hinted(s, &root, &["merge", "--no-edit", "side"], &MERGE);
        dupe_succeeds(s, &root, &["merge", "--abort"]);
        assert!(!merging(s, &root));
        assert_eq!(private_head(s, &root), before);
        assert_eq!(fs::read(root.join("notes.txt")).unwrap(), b"main\n");
        project_as_it_was(s);

        // Concluded: once the conflict is marked resolved, a merge commit of both.
        hinted(s, &root, &["merge", "--no-edit", "side"], &MERGE);
        write(&root, "notes.txt", b"both\n");
        dupe_succeeds(s, &root, &["add", "notes.txt"]);
        dupe_succeeds(s, &root, &["merge", "--continue"]);
        assert!(!merging(s, &root));
        let parents = s
            .private(&root)
            .git(["rev-list", "--parents", "-n", "1", "HEAD"])
            .succeeds();
        let words = String::from_utf8(parents.stdout).unwrap();
        assert_eq!(words.split_whitespace().count(), 3, "{words}");
        let side = s
            .private(&root)
            .git(["rev-parse", "side"])
            .succeeds()
            .stdout;
        let side = String::from_utf8(side).unwrap();
        assert!(words.contains(side.trim_end()), "{words}");
        project_as_it_was(s);
    });
}

#[test]
fn the_one_question_follows_a_merge_that_did_not_exit_0() {
    under_each_release(|s| {
        let root = fixture(s);
        let unmerged: &[&str] = &["ls-files", "-u", "-z"];
        let merge = ["merge", "--no-edit", "side"];
        asked_after(s, &root, &merge, End::Code(1), &[MERGE_HEAD]);
        asked_after(s, &root, &merge, End::Code(128), &[MERGE_HEAD]);
        // Exit 0 asks only whether an autostash came back with conflicts.
        asked_after(s, &root, &["merge", "--abort"], End::Code(0), &[unmerged]);
        asked_after(s, &root, &["merge", "nosuch"], End::Code(1), &[MERGE_HEAD]);
    });
}
