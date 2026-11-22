//! The hint after a private `checkout -m` or `switch -m` that left conflicts (`Holds/G19`,
//! S12): the run exits 0 with unmerged entries, and from 2.55.0 Git keeps the local change
//! as a stash entry and names plain `git stash drop`, `git stash pop`, and `git reset
//! --hard`, where earlier releases keep none. The same one line follows under every release,
//! conditioned on that entry, which no question here proves; what Git printed, and whether
//! the private stash holds an entry, is read from no scenario. A `checkout -m` given paths,
//! which re-creates their conflicts and switches nothing, carried no change and kept no
//! entry, so it gets none, however the paths are spelled; changes carried to another commit
//! get it whatever branch `HEAD` names. Where `HEAD` stands, the branch it names and the
//! commit, is asked before the run and after one that exited 0, and the index,
//! `ls-files -u -z`, only where `HEAD` moved (G22).

use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, asked_around, dupe_succeeds, hinted, private_add, private_alias, private_commit,
    run_traced, run_words, under_each_release, unhinted, write,
};

const MERGED_CHANGES: [&[u8]; 4] = [
    b"'git dupe add <path>'",
    b"'git dupe stash drop'",
    b"'git dupe reset --hard'",
    b"'git dupe stash pop'",
];

/// An attached workspace whose private `main` and `side` differ in `notes.txt`, `main`
/// checked out with `notes.txt` edited, so that going to `side` with `-m` conflicts.
fn fixture(s: &Scenario) -> PathBuf {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let root = s.dir().join("workspace");
    s.repository(&root);
    s.init(&root);
    write(&root, ".gitdupe", b"notes.txt\n");
    private_add(s, &root, ".gitdupe");
    write(&root, "notes.txt", b"start\n");
    private_add(s, &root, "notes.txt");
    private_commit(s, &root);
    dupe_succeeds(s, &root, &["switch", "-q", "-c", "side"]);
    write(&root, "notes.txt", b"side\n");
    private_add(s, &root, "notes.txt");
    private_commit(s, &root);
    dupe_succeeds(s, &root, &["switch", "-q", "main"]);
    edited(&root);
    root
}

fn edited(root: &Path) {
    write(root, "notes.txt", b"edited\n");
}

/// Back on `main` with the edit, nothing unmerged and no stash entry.
fn again(s: &Scenario, root: &Path) {
    s.private(root).git(["reset", "-q", "--hard"]).succeeds();
    s.private(root).git(["stash", "clear"]).succeeds();
    s.private(root).git(["switch", "-q", "main"]).succeeds();
    edited(root);
}

#[test]
fn a_merge_across_a_switch_that_conflicts_ends_with_one_hint_under_every_release() {
    under_each_release(|s| {
        let root = fixture(s);
        private_alias(s, &root, "merged", "switch -m");
        for words in [
            &["switch", "-m", "side"][..],
            &["switch", "--merge", "side"],
            &["checkout", "-m", "side"],
            &["checkout", "--merge", "side"],
            &["checkout", "-mq", "side"],
            // Git's advice follows under `-q` too, so the hint does.
            &["switch", "-m", "-q", "side"],
            &["merged", "side"],
        ] {
            let ours = hinted(s, &root, words, &MERGED_CHANGES);
            assert_eq!(ours.end, End::Code(0), "{words:?}: {ours:?}");
            again(s, &root);
        }
        let public = s.git(["stash", "list"]).from(&root).succeeds();
        assert!(public.stdout.is_empty(), "{public:?}");
    });
}

#[test]
fn the_hints_first_command_marks_a_conflict_resolved_under_every_release() {
    under_each_release(|s| {
        let root = fixture(s);
        hinted(s, &root, &["switch", "-m", "side"], &MERGED_CHANGES);
        write(&root, "notes.txt", b"resolved\n");
        dupe_succeeds(s, &root, &["add", "notes.txt"]);
        let unmerged = s.private(&root).git(["ls-files", "-u", "-z"]).succeeds();
        assert!(unmerged.stdout.is_empty(), "{unmerged:?}");
    });
}

#[test]
fn a_switch_that_leaves_nothing_unmerged_or_is_refused_gets_no_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        // Without `-m` Git refuses over the edit.
        let refused = unhinted(s, &root, &["switch", "side"]);
        assert_ne!(refused.end, End::Code(0), "{refused:?}");
        // A help request.
        unhinted(s, &root, &["switch", "-m", "side", "-h"]);
        // Nothing to conflict: the merge is clean.
        s.private(&root).git(["reset", "-q", "--hard"]).succeeds();
        let clean = unhinted(s, &root, &["switch", "-m", "side"]);
        assert_eq!(clean.end, End::Code(0), "{clean:?}");
    });
}

#[test]
fn where_head_stands_is_asked_around_the_run_and_the_index_after_it() {
    under_each_release(|s| {
        let root = fixture(s);
        let (output, runs) = run_traced(
            s.git(["dupe", "switch", "-m", "side"]).from(&root),
            &s.dir().join("trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let own = runs.own();
        let expected: [&[u8]; 11] = [
            b"rev-parse",
            b"config",
            b"symbolic-ref",
            b"rev-parse",
            b"switch",
            b"symbolic-ref",
            b"rev-parse",
            b"ls-files",
            b"ls-files",
            b"ls-files",
            b"check-ignore",
        ];
        assert_eq!(own.commands(), expected, "{runs:?}");
        let commit = run_words(&["rev-parse", "-q", "--verify", "HEAD"]);
        assert_eq!(own.words()[3], commit);
        assert_eq!(own.words()[6], commit);
        assert_eq!(own.words()[7], run_words(&["ls-files", "-u", "-z"]));
    });
}

#[test]
fn a_checkout_given_paths_during_a_conflicted_merge_gets_no_hint_under_every_release() {
    under_each_release(|s| {
        let root = fixture(s);
        s.private(&root).git(["reset", "-q", "--hard"]).succeeds();
        write(&root, "notes.txt", b"main\n");
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
        let merge = s.private(&root).git(["merge", "--no-edit", "side"]).run();
        assert_eq!(merge.end, End::Code(1), "{merge:?}");
        for words in [
            &["checkout", "-m", "--", "notes.txt"][..],
            &["checkout", "-m", "notes.txt"],
            &["checkout", "--merge", "notes.txt"],
            &["checkout", "-qm", "--", "notes.txt"],
        ] {
            let ours = unhinted(s, &root, words);
            assert_eq!(ours.end, End::Code(0), "{words:?}: {ours:?}");
        }
        // Where `HEAD` stands, before and after; the index is not asked.
        let head: &[&str] = &["symbolic-ref", "-q", "HEAD"];
        let commit: &[&str] = &["rev-parse", "-q", "--verify", "HEAD"];
        let words = &["checkout", "-m", "notes.txt"];
        asked_around(
            s,
            &root,
            words,
            End::Code(0),
            &[head, commit],
            &[head, commit],
        );
        // Going to another branch still draws it, the index asked once `HEAD` moved.
        s.private(&root).git(["merge", "--abort"]).succeeds();
        edited(&root);
        let unmerged: &[&str] = &["ls-files", "-u", "-z"];
        let words = &["checkout", "-mq", "side"];
        let after = [head, commit, unmerged];
        asked_around(s, &root, words, End::Code(0), &[head, commit], &after);
    });
}

#[test]
fn changes_carried_to_another_commit_get_the_hint_whatever_head_names() {
    under_each_release(|s| {
        let root = fixture(s);
        // From a detached `HEAD` to another commit, and on `main` reset to `side`'s commit:
        // the branch `HEAD` names stays, the commit moves, and the changes are carried.
        for (setup, words) in [
            (
                &["checkout", "-q", "--detach", "main"][..],
                &["checkout", "-m", "side^0"][..],
            ),
            (
                &["switch", "-q", "main"],
                &["checkout", "-m", "-B", "main", "side"],
            ),
        ] {
            again(s, &root);
            s.private(&root).git(["reset", "-q", "--hard"]).succeeds();
            s.private(&root).git(setup).succeeds();
            edited(&root);
            let ours = hinted(s, &root, words, &MERGED_CHANGES);
            assert_eq!(ours.end, End::Code(0), "{words:?}: {ours:?}");
            s.private(&root).git(["reset", "-q", "--hard"]).succeeds();
            s.private(&root).git(["stash", "clear"]).succeeds();
        }
    });
}
