//! The hints after a `push` or `pull` whose outcome Git's own advice names a plain `git`
//! command for (`Holds/G18`, G25): a `push` given no repository on a branch with no
//! upstream, a rejected `push`, a `pull` given no repository on a branch with no upstream,
//! and a `pull` that finds divergent branches with no way to reconcile chosen. Git's own
//! run and output unchanged, then one `hint:` naming the `git dupe` form; none after a
//! help request, after a run whose end or state the family does not name, or after
//! `git dupe git` (G19, G20, F9). The hint `push` owes where no remote is chosen is
//! `transfer_push_hint`'s.

use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Transfer, hinted, holds, private_add, private_commit, text,
    under_each_release, unhinted, without_maintenance, write, write_executable,
};

const UPSTREAM: &[u8] = b"'git dupe push -u origin main'";
const REJECTED: [&[u8]; 2] = [b"'git dupe pull'", b"'git dupe fetch'"];
const NOT_TRACKING: [&[u8]; 2] = [
    b"'git dupe pull <remote> <branch>'",
    b"'git dupe branch --set-upstream-to=<remote>/<branch>'",
];
const RECONCILE: [&[u8]; 2] = [b"'git dupe config pull.rebase", b"'git dupe config pull.ff"];

/// `git dupe <words>` from the root, which must succeed.
fn dupe(s: &Scenario, root: &Path, words: &[&str]) -> Output {
    s.git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(root)
        .succeeds()
}

/// `output` ended with status 0 and holds nothing of git-dupe's: no line of Git's names
/// `git dupe`.
fn succeeded_without_a_hint(output: &Output) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(!holds(&output.stderr, b"git dupe"), "{output:?}");
}

/// The private configuration a merge needs: an identity, and no maintenance left running.
fn committing(s: &Scenario, root: &Path) {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        dupe(s, root, &["config", key, value]);
    }
}

/// A private commit of a new file `notes/<name>.md`.
fn commit_here(s: &Scenario, root: &Path, name: &str) {
    let path = format!("notes/{name}.md");
    write(root, &path, b"here\n");
    private_add(s, root, &path);
    private_commit(s, root);
}

/// A teammate's clone of the private remote commits `notes/<name>.md` and pushes it, so
/// that the remote's `main` moves on.
fn remote_moves_on(s: &Scenario, t: &Transfer, name: &str) {
    let teammate = s.dir().join(format!("teammate-{name}"));
    s.git([
        "clone",
        "-q",
        "-b",
        "main",
        text(&t.private_remote),
        text(&teammate),
    ])
    .succeeds();
    write(&teammate, &format!("notes/{name}.md"), b"there\n");
    let identity = [
        "-c",
        "user.name=Teammate",
        "-c",
        "user.email=teammate@example.invalid",
        "-c",
        "maintenance.auto=false",
    ];
    for words in [
        &["add", "notes"][..],
        &["commit", "-qm", name],
        &["push", "-q"],
    ] {
        s.git(identity.iter().chain(words))
            .from(&teammate)
            .succeeds();
    }
}

/// The remote's `main` and the private `main` each hold a commit the other lacks.
fn diverged(s: &Scenario, t: &Transfer, name: &str) {
    remote_moves_on(s, t, &format!("{name}-there"));
    commit_here(s, &t.root, &format!("{name}-here"));
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
        // A repository given, where Git refuses the same: not given no repository. A help
        // request, or a `--help` Git takes as the value of `-o` before it fails so.
        assert_eq!(unhinted(s, root, &["push", "origin"]).end, End::Code(128));
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
            .git(["dupe", "push", "-u", "origin", "main"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let merge = s
            .private(root)
            .git(["config", "branch.main.merge"])
            .succeeds();
        assert_eq!(merge.stdout, b"refs/heads/main\n");
        assert_eq!(unhinted(s, root, &["push"]).end, End::Code(0));
        // On a detached `HEAD` no branch lacks an upstream.
        dupe(s, root, &["checkout", "-q", "--detach"]);
        assert_eq!(unhinted(s, root, &["push"]).end, End::Code(128));
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
            let named = b"'git dupe push -u backup main'";
            assert_eq!(hinted(s, root, &["push"], &[named]).end, End::Code(128));
            dupe(s, root, &["config", "--unset", key]);
        }
        // With no key, `origin`, which has a URL.
        assert_eq!(hinted(s, root, &["push"], &[UPSTREAM]).end, End::Code(128));
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
        for words in [&["push"][..], &["push", "origin", "main"]] {
            let ours = hinted(s, root, words, &REJECTED);
            assert_eq!(ours.end, End::Code(1), "{words:?}: {ours:?}");
        }
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
        // A word that chooses, or a key: Git's own refusal to fast-forward, alone.
        assert_eq!(
            unhinted(s, root, &["pull", "--ff-only"]).end,
            End::Code(128)
        );
        dupe(s, root, &["config", "pull.ff", "only"]);
        assert_eq!(unhinted(s, root, &["pull"]).end, End::Code(128));
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
        // Another branch's key is not this branch's; where a repository is given, `HEAD`
        // is not read, and any branch's key keeps the hint away.
        dupe(s, root, &["config", "branch.other.rebase", "true"]);
        assert_eq!(hinted(s, root, &["pull"], &RECONCILE).end, End::Code(128));
        let ours = unhinted(s, root, &["pull", "origin", "main"]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
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
