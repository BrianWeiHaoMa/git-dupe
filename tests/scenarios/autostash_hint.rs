//! The hint after a private `merge`, `rebase`, or `pull` whose autostash came back with
//! conflicts (`Holds/G19`, `Holds/G18`, S12): the run exits 0, a stash entry keeps the
//! changes, and the index holds unmerged entries, which such a run leaves no other way but
//! under `--quit`, which `pull` lacks. One `hint:` follows Git's output, naming
//! `git dupe add`, `git dupe stash drop`, `git dupe reset --hard`, and `git dupe stash pop`,
//! whether `--autostash` or the configuration asked for the autostash. A merge stopped by
//! its own conflicts exits 1 and gets none, as does a pull stopped by its merge's
//! conflicts; a stopped rebase, a pull's included, gets its own line; a `--quit`, which exits 0
//! leaving its conflicts unmerged and any autostash unapplied in the stash, gets none. The
//! question is one `ls-files -u -z` after a run that exited 0 and left no stop, and none
//! after any other end (G22).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, STOPPED_REBASE_HINT, Scenario, asked_after, dupe_succeeds, gits_then_one_hint, hinted,
    private_add, private_alias, private_commit, text, transfer_asked_after, under_each_release,
    unhinted, write,
};

/// The commands the hint after an autostash that came back with conflicts names.
const AUTOSTASH: [&[u8]; 4] = [
    b"'git dupe add <path>'",
    b"'git dupe stash drop'",
    b"'git dupe reset --hard'",
    b"'git dupe stash pop'",
];

/// An attached workspace whose private `notes.txt` and `other.txt` read `one` and `other`
/// in the first commit, `notes.txt` reading `side` on the branch `side` and `other.txt`
/// reading `main` on `main`, which is checked out and tagged `start`; `.gitdupe`, listing
/// both, is committed in the first. Merging or rebasing onto `side` with `notes.txt`
/// edited brings the edit back with conflicts.
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
    write(&root, ".gitdupe", b"notes.txt\nother.txt\n");
    write(&root, "other.txt", b"other\n");
    for (switch, path, content) in [
        (&[][..], "notes.txt", "one\n"),
        (&["switch", "-q", "-c", "side"], "notes.txt", "side\n"),
        (&["switch", "-q", "main"], "other.txt", "main\n"),
    ] {
        if !switch.is_empty() {
            s.private(&root).git(switch).succeeds();
        }
        write(&root, path, content.as_bytes());
        for added in [".gitdupe", "notes.txt", "other.txt"] {
            private_add(s, &root, added);
        }
        private_commit(s, &root);
    }
    s.private(&root).git(["tag", "start"]).succeeds();
    s.git(["dupe", "status"]).from(&root).succeeds();
    root
}

/// Puts the private `main` of the workspace at `root` back at `start`, with no stash
/// entry, and edits `notes.txt`.
fn edited(s: &Scenario, root: &Path) {
    s.private(root)
        .git(["reset", "-q", "--hard", "start"])
        .succeeds();
    s.private(root).git(["stash", "clear"]).succeeds();
    write(root, "notes.txt", b"edited\n");
}

/// Gives the workspace at `root` the private remote `origin`, a bare repository beside it
/// whose `main` holds the commit of `side`, as the upstream of `main`: a pull onto `start`
/// with `notes.txt` edited brings the edit back with conflicts, as the merge does.
fn tracking_side(s: &Scenario, root: &Path) {
    let remote = s.dir().join("private.git");
    s.bare_repository(&remote);
    for words in [
        &["remote", "add", "origin", text(&remote)][..],
        &["push", "-q", "origin", "side:main"],
        &["fetch", "-q", "origin"],
        &["branch", "-q", "--set-upstream-to=origin/main", "main"],
    ] {
        dupe_succeeds(s, root, words);
    }
}

/// Whether the private index of the workspace at `root` holds an unmerged entry.
fn unmerged(s: &Scenario, root: &Path) -> bool {
    !s.private(root)
        .git(["ls-files", "-u", "-z"])
        .succeeds()
        .stdout
        .is_empty()
}

/// How many entries the private stash of the workspace at `root` holds.
fn stashed(s: &Scenario, root: &Path) -> usize {
    let listed = s
        .private(root)
        .git(["stash", "list", "--format=%H"])
        .succeeds();
    listed.stdout.split(|&byte| byte == b'\n').count() - 1
}

#[test]
fn a_merge_whose_autostash_comes_back_with_conflicts_ends_with_one_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        edited(s, &root);
        let words = ["merge", "--autostash", "--no-edit", "side"];
        let merged = hinted(s, &root, &words, &AUTOSTASH);
        assert_eq!(merged.end, End::Code(0), "{merged:?}");
        assert!(unmerged(s, &root));
        assert_eq!(stashed(s, &root), 1);
        // Its commands do what it says: resolved, the entry is dropped.
        s.git(["dupe", "add", "notes.txt"]).from(&root).succeeds();
        s.git(["dupe", "stash", "drop"]).from(&root).succeeds();
        assert!(!unmerged(s, &root));
        assert_eq!(stashed(s, &root), 0);

        // Or the changes that came back are thrown away, the entry keeping them.
        edited(s, &root);
        hinted(s, &root, &words, &AUTOSTASH);
        s.git(["dupe", "reset", "--hard"]).from(&root).succeeds();
        assert!(!unmerged(s, &root));
        assert_eq!(stashed(s, &root), 1);
        assert_eq!(fs::read(root.join("notes.txt")).unwrap(), b"side\n");
        let shown = s
            .git(["dupe", "stash", "show", "-p"])
            .from(&root)
            .succeeds();
        assert!(
            shown.stdout.windows(7).any(|window| window == b"+edited"),
            "{shown:?}"
        );
    });
}

#[test]
fn a_rebase_and_the_configured_autostash_end_with_the_same_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        edited(s, &root);
        let rebased = hinted(s, &root, &["rebase", "--autostash", "side"], &AUTOSTASH);
        assert_eq!(rebased.end, End::Code(0), "{rebased:?}");
        assert!(unmerged(s, &root));
        assert_eq!(stashed(s, &root), 1);
        // The configuration asks for the autostash as the option does: the index decides.
        for (key, words) in [
            ("rebase.autoStash", &["rebase", "side"][..]),
            ("merge.autoStash", &["merge", "--no-edit", "side"]),
        ] {
            s.private(&root).git(["config", key, "true"]).succeeds();
            edited(s, &root);
            let ended = hinted(s, &root, words, &AUTOSTASH);
            assert_eq!(ended.end, End::Code(0), "{ended:?}");
            s.private(&root).git(["config", "--unset", key]).succeeds();
        }
        private_alias(s, &root, "ma", "merge --autostash --no-edit");
        edited(s, &root);
        hinted(s, &root, &["ma", "side"], &AUTOSTASH);

        // The unmerged entry lies outside the user's directory: the index is asked whole.
        let below = root.join("below");
        fs::create_dir(&below).unwrap();
        edited(s, &root);
        let moment = "@1700000000 +0000";
        let words = ["merge", "--autostash", "--no-edit", "side"];
        let gits = s
            .private(&root)
            .git(["-c", "help.autocorrect=0"].iter().chain(&words))
            .from(&below)
            .variable("GIT_AUTHOR_DATE", moment)
            .variable("GIT_COMMITTER_DATE", moment)
            .run();
        edited(s, &root);
        let ours = s
            .git(["dupe"].iter().chain(&words))
            .from(&below)
            .variable("GIT_AUTHOR_DATE", moment)
            .variable("GIT_COMMITTER_DATE", moment)
            .run();
        gits_then_one_hint(&ours, &gits, &AUTOSTASH);
        assert_eq!(ours.end, End::Code(0), "{ours:?}");
    });
}

#[test]
fn a_merge_or_rebase_that_stops_or_brings_nothing_back_gets_no_autostash_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        edited(s, &root);
        unhinted(s, &root, &["merge", "--autostash", "-h"]);
        // Nothing edited: the merge exits 0 with nothing unmerged.
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        let clean = unhinted(s, &root, &["merge", "--no-edit", "side"]);
        assert_eq!(clean.end, End::Code(0), "{clean:?}");
        // A merge stopped by its own conflicts leaves unmerged entries and exits 1: Git's
        // advice there names a bare commit, and the rule owes no hint.
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        write(&root, "notes.txt", b"main\n");
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
        let conflicted = unhinted(s, &root, &["merge", "--no-edit", "side"]);
        assert_eq!(conflicted.end, End::Code(1), "{conflicted:?}");
        assert!(unmerged(s, &root));
        let aborted = unhinted(s, &root, &["merge", "--abort"]);
        assert_eq!(aborted.end, End::Code(0), "{aborted:?}");
        // A rebase stopped by a conflict gets the stop's line, not the autostash's.
        let stopped = hinted(s, &root, &["rebase", "side"], &STOPPED_REBASE_HINT);
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        let aborted = unhinted(s, &root, &["rebase", "--abort"]);
        assert_eq!(aborted.end, End::Code(0), "{aborted:?}");

        // `--quit` exits 0 leaving the conflicts unmerged, and no autostash came back.
        let conflicted = s.private(&root).git(["merge", "--no-edit", "side"]).run();
        assert_eq!(conflicted.end, End::Code(1), "{conflicted:?}");
        for _ in 0..2 {
            let quit = unhinted(s, &root, &["merge", "--quit"]);
            assert_eq!(quit.end, End::Code(0), "{quit:?}");
            assert!(unmerged(s, &root));
        }
        s.private(&root).git(["reset", "-q", "--hard"]).succeeds();
        // With an autostash, `rebase --quit` keeps it in the stash, never applied: its
        // `stash drop` would throw the edit away.
        write(&root, "other.txt", b"edited\n");
        let stopped = hinted(
            s,
            &root,
            &["rebase", "--autostash", "side"],
            &STOPPED_REBASE_HINT,
        );
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        let quit = unhinted(s, &root, &["rebase", "--quit"]);
        assert_eq!(quit.end, End::Code(0), "{quit:?}");
        assert!(unmerged(s, &root));
        assert_eq!(stashed(s, &root), 1);
    });
}

#[test]
fn the_unmerged_entries_are_asked_about_once_after_a_run_that_exited_0() {
    under_each_release(|s| {
        let root = fixture(s);
        let unmerged_entries: &[&str] = &["ls-files", "-u", "-z"];
        let merge = ["merge", "--no-edit", "side"];
        asked_after(s, &root, &merge, End::Code(0), &[unmerged_entries]);
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        write(&root, "notes.txt", b"main\n");
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
        asked_after(s, &root, &merge, End::Code(1), &[]);
        asked_after(
            s,
            &root,
            &["merge", "--abort"],
            End::Code(0),
            &[unmerged_entries],
        );
        asked_after(s, &root, &["rebase", "side"], End::Code(1), &[]);
        asked_after(
            s,
            &root,
            &["rebase", "--abort"],
            End::Code(0),
            &[unmerged_entries],
        );
        s.private(&root).git(["merge", "--no-edit", "side"]).run();
        asked_after(s, &root, &["merge", "--quit"], End::Code(0), &[]);
    });
}

#[test]
fn a_pull_whose_autostash_comes_back_with_conflicts_ends_with_the_same_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        tracking_side(s, &root);
        edited(s, &root);
        let words = ["pull", "--autostash", "--no-rebase", "--no-edit"];
        let pulled = hinted(s, &root, &words, &AUTOSTASH);
        assert_eq!(pulled.end, End::Code(0), "{pulled:?}");
        assert!(unmerged(s, &root));
        assert_eq!(stashed(s, &root), 1);
        // Its commands do what it says: resolved, the entry is dropped.
        s.git(["dupe", "add", "notes.txt"]).from(&root).succeeds();
        s.git(["dupe", "stash", "drop"]).from(&root).succeeds();
        assert!(!unmerged(s, &root));
        assert_eq!(stashed(s, &root), 0);
        // A rebasing pull, its autostash asked for by the configuration.
        s.private(&root)
            .git(["config", "rebase.autoStash", "true"])
            .succeeds();
        edited(s, &root);
        let rebased = hinted(s, &root, &["pull", "--rebase"], &AUTOSTASH);
        assert_eq!(rebased.end, End::Code(0), "{rebased:?}");
        assert!(unmerged(s, &root));
        s.private(&root)
            .git(["config", "--unset", "rebase.autoStash"])
            .succeeds();
        private_alias(s, &root, "pa", "pull --autostash --no-rebase --no-edit");
        edited(s, &root);
        hinted(s, &root, &["pa"], &AUTOSTASH);
        edited(s, &root);
        unhinted(s, &root, &["pull", "--autostash", "-h"]);

        // Nothing edited: the pull exits 0 with nothing unmerged.
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        let clean = unhinted(
            s,
            &root,
            &["pull", "--autostash", "--no-rebase", "--no-edit"],
        );
        assert_eq!(clean.end, End::Code(0), "{clean:?}");
        // A pull stopped by its merge's conflicts exits 1: none, Git's advice there naming
        // a bare commit. Stopped by its rebase's, it gets the stop's line (`Holds/G18`).
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        write(&root, "notes.txt", b"main\n");
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
        let conflicted = unhinted(s, &root, &["pull", "--no-rebase", "--no-edit"]);
        assert_eq!(conflicted.end, End::Code(1), "{conflicted:?}");
        assert!(unmerged(s, &root));
        s.git(["dupe", "merge", "--abort"]).from(&root).succeeds();
        let stopped = hinted(s, &root, &["pull", "--rebase"], &STOPPED_REBASE_HINT);
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        s.git(["dupe", "rebase", "--abort"]).from(&root).succeeds();
    });
}

#[test]
fn the_unmerged_entries_are_asked_about_once_after_a_pull_that_exited_0() {
    under_each_release(|s| {
        let root = fixture(s);
        tracking_side(s, &root);
        let unmerged_entries: &[&str] = &["ls-files", "-u", "-z"];
        let pull = ["pull", "--no-rebase", "--no-edit"];
        transfer_asked_after(s, &root, &pull, End::Code(0), true, &[unmerged_entries]);
        // Given a refspec, `HEAD` is not read; the words after it are options to Git.
        let refspec = ["pull", "origin", "main", "--no-rebase", "--no-edit"];
        transfer_asked_after(s, &root, &refspec, End::Code(0), false, &[unmerged_entries]);
        // Stopped by its merge's conflicts, exit 1: nothing asked.
        s.private(&root)
            .git(["reset", "-q", "--hard", "start"])
            .succeeds();
        write(&root, "notes.txt", b"main\n");
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
        transfer_asked_after(s, &root, &pull, End::Code(1), true, &[]);
    });
}
