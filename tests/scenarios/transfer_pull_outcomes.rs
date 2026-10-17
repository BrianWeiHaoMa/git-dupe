//! The hints after a `pull` that Git refuses for where `HEAD` stands or for how it was told
//! to meet divergent branches (`Holds/G18`, G25): on a detached `HEAD`, given no repository
//! or a repository word and no refspec, the pull of a remote's branch; and after a pull told
//! to fast-forward only, by `--ff-only` or `pull.ff=only`, that exited 128, the merge and the
//! rebase of what it fetched. Git's own run and output unchanged, then one `hint:`; none
//! after a help request or `git dupe git` (G19, G20, F9); neither adds a run (G22). The
//! detached hint follows exit 1 alone, with which Git's advice for it comes: a pull given a
//! path on a detached `HEAD` has the remote's `HEAD` to merge, and where it cannot, Git
//! exits 128 with the divergent-branches or fast-forward-only advice, whose hint follows.
//! The hints `transfer_advice` pins, and the autostash's (`autostash_hint`), stay as they
//! are.

use std::fs;

use crate::harness::{
    End, Scenario, Transfer, commit_here, committing, diverged, dupe_succeeds, gits_then_one_hint,
    hinted, holds, private_add, private_commit, remote_moves_on, succeeded_without_a_hint, text,
    transfer_asked_after, under_each_release, unhinted, without_maintenance, write,
};

const DETACHED: &[u8] = b"'git dupe pull <remote> <branch>'";
const FAST_FORWARD_ONLY: [&[u8]; 2] = [
    b"'git dupe merge --no-ff FETCH_HEAD'",
    b"'git dupe rebase FETCH_HEAD'",
];
const UNRECONCILED: &[u8] = b"'git dupe config pull.rebase";
const NOT_TRACKING: &[u8] = b"'git dupe branch --set-upstream-to=<remote>/<branch>'";

/// Whether the private `HEAD` of the workspace `t` holds the commit its private remote's
/// `main` names.
fn holds_the_remotes_main(s: &Scenario, t: &Transfer) -> bool {
    let named = s
        .git(["rev-parse", "main"])
        .from(&t.private_remote)
        .succeeds();
    let commit = String::from_utf8(named.stdout).unwrap();
    let asked = s
        .private(&t.root)
        .git(["merge-base", "--is-ancestor", commit.trim_end(), "HEAD"])
        .run();
    asked.end == End::Code(0)
}

/// The private `notes/a.md` of the workspace at `t` reads `side` on a new branch `side` and
/// `main` on `main`, which is checked out, and `git dupe merge side` stops on the conflict,
/// leaving unmerged entries in the index.
fn conflicted(s: &Scenario, t: &Transfer) {
    let root = &t.root;
    for (switch, content) in [
        (&["switch", "-q", "-c", "side"][..], "side\n"),
        (&["switch", "-q", "main"], "main\n"),
    ] {
        dupe_succeeds(s, root, switch);
        write(root, "notes/a.md", content.as_bytes());
        private_add(s, root, "notes/a.md");
        private_commit(s, root);
    }
    let merged = s
        .git(["dupe", "merge", "--no-edit", "side"])
        .from(root)
        .run();
    assert_eq!(merged.end, End::Code(1), "{merged:?}");
}

#[test]
fn a_pull_on_a_detached_head_names_the_pull_of_a_remotes_branch() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe_succeeds(s, root, &["remote", "add", "backup", text(&backup)]);
        remote_moves_on(s, &t, "there");
        dupe_succeeds(s, root, &["checkout", "-q", "--detach"]);
        dupe_succeeds(s, root, &["config", "alias.pl", "pull"]);
        // Given no repository, or any repository word and no refspec: no branch key names an
        // upstream, and Git fails, exit 1.
        for words in [
            &["pull"][..],
            &["pull", "origin"],
            &["pull", "backup"],
            &["pull", "-v", "origin"],
            &["pl"],
        ] {
            let ours = hinted(s, root, words, &[DETACHED]);
            assert_eq!(ours.end, End::Code(1), "{words:?}: {ours:?}");
            assert!(!holds(&ours.stderr, NOT_TRACKING), "{ours:?}");
        }
        // Neither adds a run, and the one given no repository reads `HEAD`.
        transfer_asked_after(s, root, &["pull"], End::Code(1), true, &[]);
        transfer_asked_after(s, root, &["pull", "origin"], End::Code(1), true, &[]);
        // A help request: Git's output alone.
        for words in [
            &["pull", "-h"][..],
            &["pull", "-v", "--help"],
            &["pull", "origin", "--help"],
        ] {
            unhinted(s, root, words);
        }
        // Given a refspec, a pull that fails for a branch the remote lacks: no hint.
        let missing = unhinted(s, root, &["pull", "origin", "nosuch"]);
        assert_eq!(missing.end, End::Code(1), "{missing:?}");
        // Given a refspec, Git pulls that branch: no hint; the command the hint names brings
        // the remote's commit to the detached `HEAD`.
        assert!(!holds_the_remotes_main(s, &t));
        let ours = unhinted(s, root, &["pull", "origin", "main"]);
        succeeded_without_a_hint(&ours);
        assert!(holds_the_remotes_main(s, &t));
        let unmerged: &[&str] = &["ls-files", "-u", "-z"];
        transfer_asked_after(
            s,
            root,
            &["pull", "origin", "main"],
            End::Code(0),
            false,
            &[unmerged],
        );
        // Given a path, Git merges the remote's `HEAD`: where the two have diverged and
        // nothing chooses how to reconcile them, the hint Git's advice there calls for.
        s.git(["symbolic-ref", "HEAD", "refs/heads/main"])
            .from(&t.private_remote)
            .succeeds();
        commit_here(s, root, "detached");
        remote_moves_on(s, &t, "again");
        let path = text(&t.private_remote);
        let ours = hinted(s, root, &["pull", path], &[UNRECONCILED]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, DETACHED), "{ours:?}");
        // Told to fast-forward only, it refuses, exit 128: the hint for that, where the
        // detached one follows exit 1 alone.
        let ours = hinted(s, root, &["pull", "--ff-only", path], &FAST_FORWARD_ONLY);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, DETACHED), "{ours:?}");
    });
}

#[test]
fn a_pull_told_to_fast_forward_only_that_cannot_names_the_merge_and_rebase_of_what_it_fetched() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        diverged(s, &t, "first");
        dupe_succeeds(s, root, &["config", "alias.pff", "pull --ff-only"]);
        // `--ff-only` typed, whatever else is: Git refuses, exit 128.
        for words in [
            &["pull", "--ff-only"][..],
            &["pull", "--ff-only", "origin"],
            &["pull", "--ff-only", "origin", "main"],
            &["pull", "-v", "--rebase", "--ff-only"],
            &["pff"],
        ] {
            let ours = hinted(s, root, words, &FAST_FORWARD_ONLY);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
            assert!(!holds(&ours.stderr, UNRECONCILED), "{ours:?}");
        }
        transfer_asked_after(s, root, &["pull", "--ff-only"], End::Code(128), true, &[]);
        transfer_asked_after(
            s,
            root,
            &["pull", "--ff-only", "origin", "main"],
            End::Code(128),
            false,
            &[],
        );
        for words in [
            &["pull", "--ff-only", "-h"][..],
            &["pull", "--ff-only", "--help"],
        ] {
            unhinted(s, root, words);
        }

        // `pull.ff` set to `only`, where no word of the six overrides it.
        dupe_succeeds(s, root, &["config", "pull.ff", "only"]);
        for words in [&["pull"][..], &["pull", "origin", "main"]] {
            let ours = hinted(s, root, words, &FAST_FORWARD_ONLY);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
        }
        // A word of the six overrides it: with an unstaged change, Git refuses to rebase
        // before it fetches, exit 128, and neither hint follows.
        write(root, "notes/a.md", b"unstaged\n");
        for words in [&["pull", "--rebase"][..], &["pull", "-r"]] {
            assert_eq!(unhinted(s, root, words).end, End::Code(128), "{words:?}");
        }
        write(root, "notes/a.md", b"private\n");
        // Git compares the value exactly, and refuses this one, exit 128.
        dupe_succeeds(s, root, &["config", "pull.ff", "Only"]);
        assert_eq!(unhinted(s, root, &["pull"]).end, End::Code(128));
        dupe_succeeds(s, root, &["config", "--unset", "pull.ff"]);
        // Set by a `-c` before `dupe`.
        let gits = s
            .private(root)
            .git(["-c", "pull.ff=only", "-c", "help.autocorrect=0", "pull"])
            .run();
        let ours = s
            .git(["-c", "pull.ff=only", "dupe", "pull"])
            .from(root)
            .run();
        gits_then_one_hint(&ours, &gits, &FAST_FORWARD_ONLY);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");

        // Each command the hint names does what it says: the merge of what the pull fetched,
        // then, diverged again, the rebase onto it.
        let merged = s
            .git(["dupe", "merge", "--no-ff", "--no-edit", "FETCH_HEAD"])
            .from(root)
            .run();
        succeeded_without_a_hint(&merged);
        assert!(holds_the_remotes_main(s, &t));
        s.private(root)
            .git(["rev-parse", "-q", "--verify", "HEAD^2"])
            .succeeds();
        diverged(s, &t, "second");
        assert!(!holds_the_remotes_main(s, &t));
        hinted(s, root, &["pull", "--ff-only"], &FAST_FORWARD_ONLY);
        let rebased = s.git(["dupe", "rebase", "FETCH_HEAD"]).from(root).run();
        succeeded_without_a_hint(&rebased);
        assert!(holds_the_remotes_main(s, &t));
        let pulled = s.git(["dupe", "pull", "--ff-only"]).from(root).run();
        succeeded_without_a_hint(&pulled);

        // Under `--append` an earlier fetch's entries stand first in `FETCH_HEAD`, where the
        // rebase would take them: the line makes no claim given it.
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe_succeeds(s, root, &["remote", "add", "backup", text(&backup)]);
        dupe_succeeds(s, root, &["push", "-q", "backup", "main"]);
        diverged(s, &t, "third");
        dupe_succeeds(s, root, &["fetch", "-q", "backup"]);
        let appended = [
            FAST_FORWARD_ONLY[0],
            FAST_FORWARD_ONLY[1],
            b"given neither --append nor -a",
            b"from one remote alone, not from all or a group",
        ];
        let ours = hinted(s, root, &["pull", "--ff-only", "--append"], &appended);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
    });
}

#[test]
fn the_fast_forward_only_hint_follows_a_128_that_got_no_other_hint() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        // On a branch with no upstream, Git fails, exit 1: the upstream's hint alone.
        dupe_succeeds(s, root, &["switch", "-q", "-c", "topic"]);
        let ours = hinted(s, root, &["pull", "--ff-only"], &[NOT_TRACKING]);
        assert_eq!(ours.end, End::Code(1), "{ours:?}");
        assert!(!holds(&ours.stderr, FAST_FORWARD_ONLY[0]), "{ours:?}");
        dupe_succeeds(s, root, &["switch", "-q", "main"]);
        // A pull whose fetch fails exits 1: no hint.
        let aside = s.dir().join("aside.git");
        fs::rename(&t.private_remote, &aside).unwrap();
        assert_eq!(unhinted(s, root, &["pull", "--ff-only"]).end, End::Code(1));
        fs::rename(&aside, &t.private_remote).unwrap();
        // Over unmerged entries Git refuses before it fetches, exit 128, the branches not
        // diverged: the hint follows, its condition false, as exit 128 alone cannot tell.
        conflicted(s, &t);
        let ours = hinted(s, root, &["pull", "--ff-only"], &FAST_FORWARD_ONLY);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        // On a detached `HEAD` there, given no repository, Git refuses with 128: the
        // divergent-branches hint follows, true as its condition reads, and with a way
        // chosen none does, the detached one following exit 1 alone.
        dupe_succeeds(s, root, &["merge", "--abort"]);
        dupe_succeeds(s, root, &["checkout", "-q", "--detach"]);
        let merged = s
            .git(["dupe", "merge", "--no-edit", "side"])
            .from(root)
            .run();
        assert_eq!(merged.end, End::Code(1), "{merged:?}");
        let ours = hinted(s, root, &["pull"], &[UNRECONCILED]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, DETACHED), "{ours:?}");
        dupe_succeeds(s, root, &["config", "pull.rebase", "false"]);
        let ours = unhinted(s, root, &["pull"]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
    });
}
