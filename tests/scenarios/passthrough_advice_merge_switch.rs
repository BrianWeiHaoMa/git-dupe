//! The hint after a private `checkout -m` or `switch -m` that left conflicts (`Holds/G19`,
//! S12): the run exits 0 with unmerged entries, and from 2.55.0 Git keeps the local change
//! as a stash entry and names plain `git stash drop`, `git stash pop`, and `git reset
//! --hard`, where earlier releases keep none. The same one line follows under every release,
//! conditioned on that entry, which no question here proves; what Git printed, and whether
//! the private stash holds an entry, is read from no scenario. The run's one question is
//! `ls-files -u -z` after a run that exited 0 (G22).

use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, dupe_succeeds, hinted, private_add, private_alias, private_commit, run_traced,
    run_words, under_each_release, unhinted, write,
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
fn the_one_question_is_asked_after_a_run_that_exited_0() {
    under_each_release(|s| {
        let root = fixture(s);
        let (output, runs) = run_traced(
            s.git(["dupe", "switch", "-m", "side"]).from(&root),
            &s.dir().join("trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let own = runs.own();
        let expected: [&[u8]; 9] = [
            b"rev-parse",
            b"config",
            b"symbolic-ref",
            b"switch",
            b"symbolic-ref",
            b"ls-files",
            b"ls-files",
            b"ls-files",
            b"check-ignore",
        ];
        assert_eq!(own.commands(), expected, "{runs:?}");
        assert_eq!(own.words()[5], run_words(&["ls-files", "-u", "-z"]));
    });
}
