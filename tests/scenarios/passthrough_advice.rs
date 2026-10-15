//! The hint after a word passed through whose outcome Git's advice names a plain `git`
//! command for (`Holds/G19`, G25): a stopped `rebase` or `am`, a stopped `cherry-pick` or
//! `revert`, a `checkout` or `switch` that leaves a branch for a detached `HEAD`, and a
//! `branch -d` that refuses. Git's own run and output unchanged, then one `hint:` naming
//! the `git dupe` form, before settle's warnings, the exit status Git's (G19, F9); none
//! where the state or the words say Git printed no such advice, after a help request, or
//! after `git dupe git`, whose run each comparison also checks is Git's own (G20).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, global_alias, hinted, holds, private_add, private_alias, private_commit,
    run_traced, text, under_each_release, unhinted, write, write_executable,
};

const REBASE: [&[u8]; 4] = [
    b"'git dupe add <path>'",
    b"'git dupe rebase --continue'",
    b"'git dupe rebase --skip'",
    b"'git dupe rebase --abort'",
];

const AM: [&[u8]; 4] = [
    b"'git dupe add <path>'",
    b"'git dupe am --continue'",
    b"'git dupe am --skip'",
    b"'git dupe am --abort'",
];

const CHERRY_PICK: [&[u8]; 5] = [
    b"'git dupe add <path>'",
    b"'git dupe commit --allow-empty'",
    b"'git dupe cherry-pick --continue'",
    b"'git dupe cherry-pick --skip'",
    b"'git dupe cherry-pick --abort'",
];

const REVERT: [&[u8]; 4] = [
    b"'git dupe add <path>'",
    b"'git dupe revert --continue'",
    b"'git dupe revert --skip'",
    b"'git dupe revert --abort'",
];

const DETACHED: [&[u8]; 2] = [
    b"'git dupe switch -c <new-branch-name>'",
    b"'git dupe switch -'",
];

const UNMERGED: [&[u8]; 1] = [b"'git dupe branch -D <branch>'"];

/// An attached workspace whose private `notes.txt` reads `one` in the first commit, `side`
/// on the branch `side`, and `main` on `main`, which is checked out, with `.gitdupe`
/// committed beside it: a rebase of `main` onto `side`, a pick of `side`, and a revert of
/// the first commit each stop on a conflict.
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
    for (switch, content) in [
        (&[][..], "one\n"),
        (&["switch", "-q", "-c", "side"], "side\n"),
        (&["switch", "-q", "main"], "main\n"),
    ] {
        if !switch.is_empty() {
            s.private(&root).git(switch).succeeds();
        }
        write(&root, "notes.txt", content.as_bytes());
        private_add(s, &root, "notes.txt");
        private_commit(s, &root);
    }
    s.git(["dupe", "status"]).from(&root).succeeds();
    root
}

/// Puts the private `HEAD` of the workspace at `root` back on `main`.
fn back_on_main(s: &Scenario, root: &Path) {
    s.private(root).git(["switch", "-q", "main"]).succeeds();
}

/// The command word of each run `git dupe <words>` started itself from `root`, in order.
fn own_runs(s: &Scenario, root: &Path, words: &[&str]) -> Vec<String> {
    let (_, runs) = run_traced(
        s.git(["dupe"].iter().chain(words)).from(root),
        &s.dir().join("trace"),
    );
    runs.own()
        .commands()
        .iter()
        .map(|word| String::from_utf8_lossy(word).into_owned())
        .collect()
}

#[test]
fn a_stopped_rebase_or_am_ends_with_one_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        let stopped = hinted(s, &root, &["rebase", "side"], &REBASE);
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        // Continued with the conflict unresolved, it stays stopped.
        let continued = hinted(s, &root, &["rebase", "--continue"], &REBASE);
        assert_eq!(continued.end, End::Code(1), "{continued:?}");
        unhinted(s, &root, &["rebase", "--abort"]);
        // A rebase that fails before it starts leaves no stopped rebase behind.
        let refused = unhinted(s, &root, &["rebase", "nosuch"]);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        hinted(s, &root, &["rebase", "--apply", "side"], &REBASE);
        unhinted(s, &root, &["rebase", "--abort"]);

        let patch = s.dir().join("side.patch");
        let made = s
            .private(&root)
            .git(["format-patch", "-1", "--stdout", "side"])
            .succeeds();
        fs::write(&patch, made.stdout).unwrap();
        let applying = hinted(s, &root, &["am", text(&patch)], &AM);
        assert_eq!(applying.end, End::Code(128), "{applying:?}");
        unhinted(s, &root, &["am", "--abort"]);

        // A hook that refuses the rebase leaves nothing stopped: the state decides.
        let hook = root.join(".git/dupe/hooks/pre-rebase");
        write_executable(s, &hook, b"#!/bin/sh\nexit 1\n");
        let hooked = unhinted(s, &root, &["rebase", "side"]);
        assert_ne!(hooked.end, End::Code(0), "{hooked:?}");
        fs::remove_file(&hook).unwrap();

        // Git killed by a signal while its rebase stands stopped: git-dupe ends with 128
        // plus the signal, and the hint follows.
        let killed = hinted(
            s,
            &root,
            &["rebase", "-x", "kill -9 $PPID", "HEAD~1"],
            &REBASE,
        );
        assert_eq!(killed.end, End::Code(128 + 9), "{killed:?}");
        unhinted(s, &root, &["rebase", "--abort"]);
        // The stopped rebase is read by `lstat`, at no Git run.
        assert_eq!(
            own_runs(s, &root, &["rebase", "side"]),
            [
                "rev-parse",
                "config",
                "rebase",
                "ls-files",
                "ls-files",
                "check-ignore"
            ]
        );
    });
}

#[test]
fn a_stopped_cherry_pick_or_revert_ends_with_one_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        let stopped = hinted(s, &root, &["cherry-pick", "side"], &CHERRY_PICK);
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        let continued = hinted(s, &root, &["cherry-pick", "--continue"], &CHERRY_PICK);
        assert_ne!(continued.end, End::Code(0), "{continued:?}");
        unhinted(s, &root, &["cherry-pick", "--abort"]);
        // A pick of what `HEAD` already holds stops as empty.
        let empty = hinted(s, &root, &["cherry-pick", "HEAD"], &CHERRY_PICK);
        assert_eq!(empty.end, End::Code(1), "{empty:?}");
        unhinted(s, &root, &["cherry-pick", "--abort"]);
        let reverted = hinted(s, &root, &["revert", "HEAD~1"], &REVERT);
        assert_eq!(reverted.end, End::Code(1), "{reverted:?}");
        unhinted(s, &root, &["revert", "--abort"]);
        // A pick that fails before it starts leaves nothing stopped.
        let refused = unhinted(s, &root, &["cherry-pick", "nosuch"]);
        assert_eq!(refused.end, End::Code(128), "{refused:?}");

        // One question, after a run that did not exit 0, and none after one that did.
        assert_eq!(
            own_runs(s, &root, &["cherry-pick", "side"]),
            [
                "rev-parse",
                "config",
                "cherry-pick",
                "rev-parse",
                "ls-files",
                "ls-files",
                "check-ignore"
            ]
        );
        assert_eq!(
            own_runs(s, &root, &["cherry-pick", "--abort"]),
            [
                "rev-parse",
                "config",
                "cherry-pick",
                "ls-files",
                "ls-files",
                "check-ignore"
            ]
        );
    });
}

#[test]
fn leaving_a_branch_for_a_detached_head_ends_with_one_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        let detached = hinted(s, &root, &["checkout", "HEAD~1"], &DETACHED);
        assert_eq!(detached.end, End::Code(0), "{detached:?}");
        // Already detached, moving to another commit or to a branch: no advice to correct.
        unhinted(s, &root, &["checkout", "side~0"]);
        unhinted(s, &root, &["switch", "main"]);
        for words in [
            &["switch", "--detach", "HEAD~1"][..],
            &["checkout", "--detach", "main"],
            &["checkout", "-q", "HEAD~1"],
            &["switch", "-q", "-d", "HEAD~1"],
        ] {
            unhinted(s, &root, words);
            back_on_main(s, &root);
            // Words Git prints no advice after put no question to the state.
            let runs = own_runs(s, &root, words);
            assert!(
                !runs.contains(&"symbolic-ref".into()),
                "{words:?}: {runs:?}"
            );
            back_on_main(s, &root);
        }
        unhinted(s, &root, &["checkout", "-b", "x"]);
        back_on_main(s, &root);
        write(&root, "notes.txt", b"edited\n");
        unhinted(s, &root, &["checkout", "--", "notes.txt"]);

        // Two questions where the words admit the hint, one before the run and one after,
        // and none otherwise: the same runs whatever either repository holds (G22).
        assert_eq!(
            own_runs(s, &root, &["switch", "side"]),
            [
                "rev-parse",
                "config",
                "symbolic-ref",
                "switch",
                "symbolic-ref",
                "ls-files",
                "ls-files",
                "check-ignore"
            ]
        );
        assert_eq!(
            own_runs(s, &root, &["switch", "-q", "main"]),
            [
                "rev-parse",
                "config",
                "switch",
                "ls-files",
                "ls-files",
                "check-ignore"
            ]
        );

        // A setting that silences Git's advice leaves the hint, which follows the state.
        s.private(&root)
            .git(["config", "advice.detachedHead", "false"])
            .succeeds();
        hinted(s, &root, &["checkout", "HEAD~1"], &DETACHED);
    });
}

#[test]
fn a_detached_head_question_that_fails_gives_no_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        let head = root.join(".git/dupe/HEAD");
        // The hook leaves `HEAD` holding bytes that are no reference, so that the question
        // after the run cannot be answered.
        let hook = format!("#!/bin/sh\nprintf 'no reference\\n' > '{}'\n", text(&head));
        write_executable(
            s,
            &root.join(".git/dupe/hooks/post-checkout"),
            hook.as_bytes(),
        );
        let gits = s
            .private(&root)
            .git(["-c", "help.autocorrect=0", "checkout", "HEAD~1"])
            .run();
        fs::write(&head, b"ref: refs/heads/main\n").unwrap();
        s.private(&root).git(["reset", "-q", "--hard"]).succeeds();
        let ours = s.git(["dupe", "checkout", "HEAD~1"]).from(&root).run();
        assert_eq!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(ours.end, gits.end, "{ours:?}");
        assert_eq!(ours.stdout, gits.stdout, "{ours:?}");
        let after = ours
            .stderr
            .strip_prefix(gits.stderr.as_slice())
            .unwrap_or_else(|| panic!("not Git's output first: {ours:?}"));
        assert!(!holds(after, b"hint: "), "{ours:?}");
    });
}

#[test]
fn an_alias_chain_is_read_by_the_command_it_reaches() {
    under_each_release(|s| {
        let root = fixture(s);
        private_alias(s, &root, "co", "checkout");
        private_alias(s, &root, "cod", "checkout --detach");
        private_alias(s, &root, "sh", "!git checkout HEAD~1");
        hinted(s, &root, &["co", "HEAD~1"], &DETACHED);
        back_on_main(s, &root);
        // The expansion's own words keep the hint away as typed words do.
        unhinted(s, &root, &["cod", "HEAD~1"]);
        back_on_main(s, &root);
        // A `!` alias runs a shell: what it runs is not read.
        unhinted(s, &root, &["sh"]);
        // After the expansion's `--` a typed `-h` is a branch name, and no help request.
        private_alias(s, &root, "bd", "branch -d --");
        let missing = hinted(s, &root, &["bd", "-h"], &UNMERGED);
        assert_eq!(missing.end, End::Code(1), "{missing:?}");
    });
}

#[test]
fn a_branch_delete_that_refuses_ends_with_one_hint() {
    under_each_release(|s| {
        let root = fixture(s);
        let unmerged = hinted(s, &root, &["branch", "-d", "side"], &UNMERGED);
        assert_eq!(unmerged.end, End::Code(1), "{unmerged:?}");
        hinted(s, &root, &["branch", "--delete", "side"], &UNMERGED);
        // Decided from the exit alone, so a branch that does not exist draws it too.
        let missing = hinted(s, &root, &["branch", "-d", "nosuch"], &UNMERGED);
        assert_eq!(missing.end, End::Code(1), "{missing:?}");
        let forced = unhinted(s, &root, &["branch", "-D", "side"]);
        assert_eq!(forced.end, End::Code(0), "{forced:?}");
    });
}

#[test]
fn a_help_request_gets_no_hint_where_the_state_would_draw_one() {
    under_each_release(|s| {
        let root = fixture(s);
        // A stopped rebase stands, and a release whose help request exits 129 ends a run
        // that did not exit 0.
        let stopped = s.private(&root).git(["rebase", "side"]).run();
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        for words in [
            &["rebase", "-h"][..],
            &["rebase", "--continue", "--help"],
            &["am", "-h"],
        ] {
            unhinted(s, &root, words);
        }
        // A help request an alias's expansion holds is one too; a typed `-h` after the
        // expansion's own `--` is none.
        private_alias(s, &root, "rbh", "rebase -h");
        unhinted(s, &root, &["rbh"]);
        private_alias(s, &root, "rb", "rebase --");
        hinted(s, &root, &["rb", "-h"], &REBASE);
        s.private(&root).git(["rebase", "--abort"]).succeeds();
        let stopped = s.private(&root).git(["cherry-pick", "side"]).run();
        assert_eq!(stopped.end, End::Code(1), "{stopped:?}");
        unhinted(s, &root, &["cherry-pick", "-h"]);
        unhinted(s, &root, &["cherry-pick", "--continue", "--help"]);
        s.private(&root).git(["cherry-pick", "--abort"]).succeeds();
        unhinted(s, &root, &["checkout", "HEAD~1", "--help"]);
        unhinted(s, &root, &["branch", "-d", "side", "-h"]);

        // Where no workspace is attached only a help request runs, Git's alone, and no
        // question is put to a private repository that is not there: not even for an
        // expansion that makes the typed `-h` an operand.
        let unattached = s.dir().join("unattached");
        s.repository(&unattached);
        global_alias(s, "co", "checkout --");
        for words in [["rebase", "-h"], ["co", "-h"]] {
            let expected = s
                .git(["-c", "help.autocorrect=0"].iter().chain(&words))
                .variable("GIT_DIR", unattached.join(".git/dupe"))
                .from(&unattached)
                .run();
            let ours = s.git(["dupe"].iter().chain(&words)).from(&unattached).run();
            assert_eq!(ours, expected, "{words:?}");
        }
    });
}

#[test]
fn the_hint_comes_after_gits_output_and_before_settles_warnings() {
    under_each_release(|s| {
        let root = fixture(s);
        // A rule that re-includes the privately tracked `notes.txt`, which settle names.
        write(&root, ".gitignore", b"!notes.txt\n");
        let gits = s
            .private(&root)
            .git(["-c", "help.autocorrect=0", "rebase", "side"])
            .run();
        s.private(&root).git(["rebase", "--abort"]).succeeds();
        let ours = s.git(["dupe", "rebase", "side"]).from(&root).run();
        assert_eq!(ours.end, gits.end, "{ours:?}");
        assert_eq!(ours.stdout, gits.stdout, "{ours:?}");
        let after = ours
            .stderr
            .strip_prefix(gits.stderr.as_slice())
            .unwrap_or_else(|| panic!("not Git's output first: {ours:?}"));
        let lines: Vec<&[u8]> = after.split_inclusive(|&byte| byte == b'\n').collect();
        assert_eq!(lines.len(), 2, "{ours:?}");
        assert!(lines[0].starts_with(b"hint: "), "{ours:?}");
        for part in REBASE {
            assert!(holds(lines[0], part), "{ours:?}");
        }
        assert!(lines[1].starts_with(b"warning: "), "{ours:?}");
        assert!(holds(lines[1], b"notes.txt"), "{ours:?}");
        assert!(holds(lines[1], b".gitignore"), "{ours:?}");
    });
}
