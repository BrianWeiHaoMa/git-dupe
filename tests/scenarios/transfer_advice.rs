//! The hints after a `push` or `pull` whose outcome Git's own advice names a plain `git`
//! command for (`Holds/G18`, G25): a `push` on a branch with no upstream given no
//! repository, or given the branch's remote and no refspec, a rejected `push`, a `pull` on
//! a branch with no upstream given no repository, or given its `branch.<name>.remote` and
//! no refspec, and a `pull` that finds divergent branches with no way to reconcile chosen.
//! Git's own run and output unchanged, then one `hint:` naming the `git dupe` form; none
//! after a help request, after a run whose end or state the family does not name, or
//! after `git dupe git` (G19, G20, F9). The hint `push` owes where no remote is chosen is
//! `transfer_push_hint`'s.

use std::path::Path;

use crate::harness::{
    End, Output, Scenario, commit_here, committing, diverged, hinted, holds,
    succeeded_without_a_hint, text, under_each_release, unhinted, without_maintenance, write,
    write_executable,
};

const UPSTREAM: &[u8] = b"'git dupe push -u origin HEAD'";
const DETACHED: &[u8] = b"'git dupe push origin HEAD:<branch>'";
const REJECTED: [&[u8]; 2] = [b"'git dupe pull'", b"'git dupe fetch'"];
const NOT_TRACKING: [&[u8]; 2] = [
    b"'git dupe pull <remote> <branch>'",
    b"'git dupe branch --set-upstream-to=<remote>/<branch>'",
];
const RECONCILE: [&[u8]; 2] = [b"'git dupe config pull.rebase", b"'git dupe config pull.ff"];
const FAST_FORWARD_ONLY: &[u8] = b"'git dupe merge --no-ff FETCH_HEAD'";

/// `git dupe <words>` from the root, which must succeed.
fn dupe(s: &Scenario, root: &Path, words: &[&str]) -> Output {
    s.git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(root)
        .succeeds()
}

#[test]
fn a_push_on_a_branch_with_no_upstream_names_push_u_with_the_remote_git_takes() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        dupe(
            s,
            root,
            &["remote", "add", "origin", text(&t.private_remote)],
        );
        let ours = hinted(s, root, &["push"], &[UPSTREAM]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        dupe(s, root, &["config", "alias.p", "push"]);
        hinted(s, root, &["p"], &[UPSTREAM]);
        // The branch's remote given, with no refspec: the same hint. A help request, or a
        // `--help` Git takes as the value of `-o` before it fails so: none.
        let ours = hinted(s, root, &["push", "origin"], &[UPSTREAM]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        for words in [&["push", "-h"][..], &["push", "-o", "--help"]] {
            unhinted(s, root, words);
        }
        // Where `push.autoSetupRemote` sets the upstream, Git succeeds: no hint.
        dupe(s, root, &["switch", "-q", "-c", "topic"]);
        let output = s
            .git(["-c", "push.autoSetupRemote=true", "dupe", "push"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let merge = s
            .private(root)
            .git(["config", "branch.topic.merge"])
            .succeeds();
        assert_eq!(merge.stdout, b"refs/heads/topic\n");
        dupe(s, root, &["switch", "-q", "main"]);
        // The form the hint names succeeds and sets the upstream; then no hint is owed.
        let output = s
            .git(["dupe", "push", "-u", "origin", "HEAD"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let merge = s
            .private(root)
            .git(["config", "branch.main.merge"])
            .succeeds();
        assert_eq!(merge.stdout, b"refs/heads/main\n");
        assert_eq!(unhinted(s, root, &["push"]).end, End::Code(0));
        // On a detached `HEAD` no branch lacks an upstream: the detached hint instead.
        dupe(s, root, &["checkout", "-q", "--detach"]);
        let ours = hinted(s, root, &["push"], &[DETACHED]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, UPSTREAM), "{ours:?}");
    });
}

#[test]
fn a_push_given_the_branchs_remote_and_no_refspec_names_push_u_with_it() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let remote = text(&t.private_remote);
        dupe(s, root, &["remote", "add", "origin", remote]);
        dupe(s, root, &["config", "alias.p", "push"]);
        // An option or an option's value about the remote is no refspec.
        for words in [
            &["push", "--", "origin"][..],
            &["push", "--repo", "origin"],
            &["push", "--repo=origin"],
            &["push", "-v", "origin"],
            &["push", "origin", "-n"],
            &["p", "origin"],
        ] {
            let ours = hinted(s, root, words, &[UPSTREAM]);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
        }
        // A help request among the words, wherever it stands: Git's output alone.
        for words in [
            &["push", "origin", "--help"][..],
            &["push", "-o", "--help", "origin"],
        ] {
            unhinted(s, root, words);
        }
        // Where the push URL names nothing, Git refuses a refspec too, naming no command.
        let nowhere = s.dir().join("nowhere.git");
        dupe(
            s,
            root,
            &["config", "remote.origin.pushurl", text(&nowhere)],
        );
        let ours = hinted(s, root, &["push", "origin"], &[UPSTREAM]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        for words in [
            &["push", "origin", "main"][..],
            &["push", "origin", "HEAD"],
            &["push", "--", "origin", "main"],
            &["push", "origin", "--", "main"],
        ] {
            assert_eq!(unhinted(s, root, words).end, End::Code(128), "{words:?}");
        }
        dupe(s, root, &["config", "--unset", "remote.origin.pushurl"]);
        // The branch's remote by a name a shell reads back only quoted: named as typed.
        dupe(s, root, &["remote", "add", "it's", remote]);
        dupe(s, root, &["config", "branch.main.remote", "it's"]);
        let named = b"'git dupe push -u \"it's\" HEAD'";
        let ours = hinted(s, root, &["push", "it's"], &[named]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        dupe(s, root, &["config", "--unset", "branch.main.remote"]);
        // With an upstream, or on a detached `HEAD`, no upstream hint is owed.
        let output = s
            .git(["dupe", "push", "-u", "origin", "HEAD"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        assert_eq!(unhinted(s, root, &["push", "origin"]).end, End::Code(0));
        dupe(s, root, &["checkout", "-q", "--detach"]);
        let ours = hinted(s, root, &["push", "origin"], &[DETACHED]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, UPSTREAM), "{ours:?}");
    });
}

/// Git compares a remote given with no refspec with `branch.<name>.remote`, else `origin`,
/// whatever the push keys hold, and pushes the branch to another under its name (S13).
#[test]
fn a_push_given_another_remote_than_the_branchs_owes_no_upstream_hint() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe(
            s,
            root,
            &["remote", "add", "origin", text(&t.private_remote)],
        );
        dupe(s, root, &["remote", "add", "backup", text(&backup)]);
        let pushed = |remote: &str| {
            let output = s.git(["dupe", "push", remote]).from(root).run();
            succeeded_without_a_hint(&output);
        };
        pushed("backup");
        for key in ["remote.pushDefault", "branch.main.pushRemote"] {
            dupe(s, root, &["config", key, "backup"]);
            let ours = hinted(s, root, &["push", "origin"], &[UPSTREAM]);
            assert_eq!(ours.end, End::Code(128), "{key}: {ours:?}");
            pushed("backup");
            dupe(s, root, &["config", "--unset", key]);
        }
        dupe(s, root, &["config", "branch.main.remote", "backup"]);
        let named = b"'git dupe push -u backup HEAD'";
        let ours = hinted(s, root, &["push", "backup"], &[named]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        pushed("origin");
        dupe(s, root, &["config", "--unset", "branch.main.remote"]);
        // Under `upstream` Git refuses another remote naming no command; under `current`
        // it pushes to the branch's.
        dupe(s, root, &["config", "push.default", "upstream"]);
        assert_eq!(unhinted(s, root, &["push", "backup"]).end, End::Code(128));
        let ours = hinted(s, root, &["push", "origin"], &[UPSTREAM]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        dupe(s, root, &["config", "push.default", "current"]);
        pushed("origin");
    });
}

/// Where a tag shares the branch's name, `push` refuses the name as ambiguous; the command
/// the hint names, run as shown, pushes the branch all the same and sets its upstream.
#[test]
fn the_upstream_hint_names_a_push_that_succeeds_where_a_tag_shares_the_branch_name() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        dupe(
            s,
            root,
            &["remote", "add", "origin", text(&t.private_remote)],
        );
        dupe(s, root, &["tag", "v1"]);
        dupe(s, root, &["switch", "-q", "-c", "v1"]);
        commit_here(s, root, "on-v1");
        let ours = hinted(s, root, &["push"], &[UPSTREAM]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        let output = s
            .git(["dupe", "push", "-u", "origin", "HEAD"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let merge = s
            .private(root)
            .git(["config", "branch.v1.merge"])
            .succeeds();
        assert_eq!(merge.stdout, b"refs/heads/v1\n");
    });
}

#[test]
fn a_push_branch_key_names_the_remote_in_the_hint() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let origin = s.dir().join("origin.git");
        s.bare_repository(&origin);
        dupe(
            s,
            root,
            &["remote", "add", "backup", text(&t.private_remote)],
        );
        dupe(s, root, &["remote", "add", "origin", text(&origin)]);
        // Git refuses every push given no refspec, and the hint is true all the same.
        dupe(s, root, &["config", "push.default", "nothing"]);
        for key in [
            "branch.main.pushRemote",
            "remote.pushDefault",
            "branch.main.remote",
        ] {
            dupe(s, root, &["config", key, "backup"]);
            let named = b"'git dupe push -u backup HEAD'";
            assert_eq!(hinted(s, root, &["push"], &[named]).end, End::Code(128));
            dupe(s, root, &["config", "--unset", key]);
        }
        // With no key, `origin`, which has a URL.
        assert_eq!(hinted(s, root, &["push"], &[UPSTREAM]).end, End::Code(128));
        // A name a shell takes as written, and one it takes only quoted.
        for (remote, named) in [
            ("caf\u{e9}", "'git dupe push -u caf\u{e9} HEAD'"),
            ("it's", "'git dupe push -u \"it's\" HEAD'"),
        ] {
            dupe(s, root, &["remote", "add", remote, text(&t.private_remote)]);
            dupe(s, root, &["config", "remote.pushDefault", remote]);
            let ours = hinted(s, root, &["push"], &[named.as_bytes()]);
            assert_eq!(ours.end, End::Code(128), "{ours:?}");
        }
    });
}

#[test]
fn a_rejected_push_names_pull_and_fetch() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        diverged(s, &t, "first");
        let ours = hinted(s, root, &["push"], &REJECTED);
        assert_eq!(ours.end, End::Code(1), "{ours:?}");
        // Given a repository, the hint names it, the destination that rejected the push.
        let named = [
            &b"'git dupe pull origin <branch>'"[..],
            b"'git dupe fetch origin'",
        ];
        let ours = hinted(s, root, &["push", "origin", "main"], &named);
        assert_eq!(ours.end, End::Code(1), "{ours:?}");
        // A help request, or a `--help` Git takes as the value of `-o`.
        for words in [&["push", "-h"][..], &["push", "-o", "--help"]] {
            unhinted(s, root, words);
        }
        let pulled = s
            .git(["dupe", "pull", "--no-rebase", "--no-edit"])
            .from(root)
            .run();
        succeeded_without_a_hint(&pulled);
        succeeded_without_a_hint(&s.git(["dupe", "push"]).from(root).run());
        // A hook's refusal ends the same way, and the hint says what it says either way.
        let hook = root.join(".git/dupe/hooks/pre-push");
        write_executable(s, &hook, b"#!/bin/sh\nexit 1\n");
        commit_here(s, root, "hooked");
        assert_eq!(hinted(s, root, &["push"], &REJECTED).end, End::Code(1));
    });
}

#[test]
fn a_pull_on_a_branch_with_no_upstream_names_pull_and_set_upstream_to() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        // An empty remote: Git finds nothing to merge.
        dupe(
            s,
            root,
            &["remote", "add", "origin", text(&t.private_remote)],
        );
        let ours = hinted(s, root, &["pull"], &NOT_TRACKING);
        assert_ne!(ours.end, End::Code(0), "{ours:?}");
        // A repository given, and help requests: Git's output alone.
        assert_eq!(unhinted(s, root, &["pull", "origin"]).end, End::Code(1));
        for words in [&["pull", "-h"][..], &["pull", "-v", "--help"]] {
            unhinted(s, root, words);
        }
        // With the remote's `HEAD` naming its `main`, Git still takes no branch to merge.
        dupe(s, root, &["push", "origin", "main"]);
        s.git(["symbolic-ref", "HEAD", "refs/heads/main"])
            .from(&t.private_remote)
            .succeeds();
        assert_eq!(hinted(s, root, &["pull"], &NOT_TRACKING).end, End::Code(1));
        // The form the hint names sets the upstream; then no hint is owed.
        dupe(s, root, &["branch", "--set-upstream-to=origin/main"]);
        succeeded_without_a_hint(&s.git(["dupe", "pull"]).from(root).run());
    });
}

#[test]
fn a_pull_given_the_branchs_remote_and_no_refspec_names_pull_and_set_upstream_to() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe(
            s,
            root,
            &["remote", "add", "origin", text(&t.private_remote)],
        );
        dupe(s, root, &["remote", "add", "backup", text(&backup)]);
        // Only the branch's own `branch.<name>.remote` is the form; with the key unset,
        // Git names no command (above).
        dupe(s, root, &["config", "branch.main.remote", "origin"]);
        for words in [&["pull", "origin"][..], &["pull", "-v", "origin"]] {
            let ours = hinted(s, root, words, &NOT_TRACKING);
            assert_eq!(ours.end, End::Code(1), "{words:?}: {ours:?}");
        }
        // Another remote, a refspec: Git's output alone. A negated option takes no value,
        // so the word after it is the repository Git pulls from.
        for words in [
            &["pull", "backup"][..],
            &["pull", "origin", "main"],
            &["pull", "--no-rebase", "backup"],
            &["pull", "--no-recurse-submodules", "backup"],
        ] {
            assert_ne!(unhinted(s, root, words).end, End::Code(0), "{words:?}");
        }
        let ours = hinted(s, root, &["pull", "--no-rebase", "origin"], &NOT_TRACKING);
        assert_eq!(ours.end, End::Code(1), "{ours:?}");
        // A help request, or a `--help` Git takes as the value of `-o` before it fails.
        for words in [
            &["pull", "origin", "--help"][..],
            &["pull", "-o", "--help", "origin"],
        ] {
            unhinted(s, root, words);
        }
        // With the upstream set, Git pulls.
        dupe(s, root, &["push", "-u", "origin", "HEAD"]);
        succeeded_without_a_hint(&s.git(["dupe", "pull", "origin"]).from(root).run());
    });
}

#[test]
fn a_divergent_pull_with_no_way_to_reconcile_names_pull_rebase_and_pull_ff() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        diverged(s, &t, "first");
        for words in [&["pull"][..], &["pull", "origin", "main"]] {
            let ours = hinted(s, root, words, &RECONCILE);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
        }
        // A word that chooses, or a key: Git's own refusal to fast-forward, with its own
        // hint and not this one.
        let ours = hinted(s, root, &["pull", "--ff-only"], &[FAST_FORWARD_ONLY]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, RECONCILE[0]), "{ours:?}");
        dupe(s, root, &["config", "pull.ff", "only"]);
        let ours = hinted(s, root, &["pull"], &[FAST_FORWARD_ONLY]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, RECONCILE[0]), "{ours:?}");
        dupe(s, root, &["config", "--unset", "pull.ff"]);
        // With an unstaged change, Git refuses to rebase before it fetches: a rebase the
        // branch's key, or a `-c` before `dupe`, chooses keeps the hint away.
        write(root, "notes/a.md", b"unstaged\n");
        assert_eq!(hinted(s, root, &["pull"], &RECONCILE).end, End::Code(128));
        dupe(s, root, &["config", "branch.main.rebase", "true"]);
        assert_eq!(unhinted(s, root, &["pull"]).end, End::Code(128));
        dupe(s, root, &["config", "--unset", "branch.main.rebase"]);
        let gits = s
            .private(root)
            .git(["-c", "pull.rebase=true", "-c", "help.autocorrect=0", "pull"])
            .run();
        let ours = s
            .git(["-c", "pull.rebase=true", "dupe", "pull"])
            .from(root)
            .run();
        assert_eq!(ours, gits);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        // Another branch's key is not this branch's; where a repository is given, any
        // branch's key keeps the hint away, whether or not `HEAD` is read for another hint.
        dupe(s, root, &["config", "branch.other.rebase", "true"]);
        assert_eq!(hinted(s, root, &["pull"], &RECONCILE).end, End::Code(128));
        for words in [&["pull", "origin", "main"][..], &["pull", "origin"]] {
            let ours = unhinted(s, root, words);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
        }
        dupe(s, root, &["config", "--unset", "branch.other.rebase"]);
        write(root, "notes/a.md", b"private\n");
        // Each way the hint names reconciles: no hint.
        let pulled = s
            .git(["dupe", "pull", "--no-rebase", "--no-edit"])
            .from(root)
            .run();
        succeeded_without_a_hint(&pulled);
        diverged(s, &t, "second");
        dupe(s, root, &["config", "pull.rebase", "false"]);
        succeeded_without_a_hint(&s.git(["dupe", "pull", "--no-edit"]).from(root).run());
        dupe(s, root, &["config", "--unset", "pull.rebase"]);
        diverged(s, &t, "third");
        let pulled = s
            .git(["-c", "pull.rebase=false", "dupe", "pull", "--no-edit"])
            .from(root)
            .run();
        succeeded_without_a_hint(&pulled);
    });
}
