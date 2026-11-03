//! The hints after a `push` that Git refuses for its branch's state or that a remote
//! rejects (`Holds/G18`, G25): on a branch with no commit and no upstream, `git dupe
//! commit` before the push the upstream hint names; on a branch whose upstream has another
//! name, the push to that upstream and the push to the branch's own name; on a detached
//! `HEAD`, the push of `HEAD` to a branch the remote has; and after a rejected push given a
//! repository, the pull and fetch of that repository. Git's own run and output unchanged,
//! then one `hint:`; none after a help request or `git dupe git` (G19, G20, F9); and the
//! one question it adds, whether `HEAD` names a commit, asked only after a push owed the
//! upstream hint (G22). The hints `transfer_advice` and `transfer_push_hint` pin stay as
//! they are.

use std::path::Path;

use crate::harness::{
    End, Scenario, as_git_spells, commit_here, committing, diverged, dupe_succeeds,
    gits_then_one_hint, hinted, holds, succeeded_without_a_hint, text, transfer_asked_after,
    under_each_release, unhinted, without_maintenance, write,
};

const UPSTREAM: &[u8] = b"'git dupe push -u origin HEAD'";
const COMMIT: &[u8] = b"'git dupe commit'";
const DETACHED: &[u8] = b"'git dupe push origin HEAD:refs/heads/<branch>'";
const OTHER_NAME: [&[u8]; 2] = [
    b"'git dupe push origin HEAD:refs/heads/main'",
    b"'git dupe push origin HEAD'",
];

/// `git dupe <words>` from the root ends as `ended`, and the Git runs it started itself are
/// those of a transfer command, `HEAD` read where `reads_head`, and, where `asks`, the one
/// question whether `HEAD` names a commit after the run (G22).
fn runs(s: &Scenario, root: &Path, words: &[&str], ended: End, reads_head: bool, asks: bool) {
    let verify: &[&str] = &["rev-parse", "-q", "--verify", "HEAD"];
    let questions = if asks { &[verify][..] } else { &[] };
    transfer_asked_after(s, root, words, ended, reads_head, questions);
}

/// The hint's line in `stderr` names `first` before `then`.
fn in_order(stderr: &[u8], first: &[u8], then: &[u8]) {
    let at = |part: &[u8]| stderr.windows(part.len()).position(|window| window == part);
    assert!(
        at(first).is_some() && at(first) < at(then),
        "{}",
        stderr.escape_ascii()
    );
}

#[test]
fn a_push_from_a_branch_with_no_commit_names_the_first_commit_and_then_push_u() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        dupe_succeeds(
            s,
            root,
            &["remote", "add", "origin", text(&t.private_remote)],
        );
        // On a branch with commits, the question is asked and the upstream hint follows.
        let ours = hinted(s, root, &["push"], &[UPSTREAM]);
        assert!(!holds(&ours.stderr, COMMIT), "{ours:?}");
        runs(s, root, &["push"], End::Code(128), true, true);
        runs(s, root, &["push", "origin"], End::Code(128), true, true);
        // Another remote, or a refspec, that Git refuses too, for a destination that
        // names nothing: no hint, and nothing asked.
        let nowhere = s.dir().join("nowhere.git");
        dupe_succeeds(s, root, &["remote", "add", "gone", text(&nowhere)]);
        for words in [&["push", "gone"][..], &["push", "gone", "HEAD:main"]] {
            assert_eq!(unhinted(s, root, words).end, End::Code(128), "{words:?}");
        }
        runs(s, root, &["push", "gone"], End::Code(128), true, false);
        runs(
            s,
            root,
            &["push", "gone", "HEAD:main"],
            End::Code(128),
            false,
            false,
        );
        dupe_succeeds(s, root, &["remote", "remove", "gone"]);
        // A branch with no commit, the index and so the hidden paths kept.
        dupe_succeeds(s, root, &["checkout", "-q", "--orphan", "fresh"]);
        dupe_succeeds(s, root, &["config", "alias.p", "push"]);
        for words in [&["push"][..], &["push", "origin"], &["p"]] {
            let ours = hinted(s, root, words, &[COMMIT, UPSTREAM]);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
            in_order(&ours.stderr, COMMIT, UPSTREAM);
        }
        runs(s, root, &["push"], End::Code(128), true, true);
        runs(s, root, &["push", "origin"], End::Code(128), true, true);
        // A help request: Git's output alone, and nothing asked.
        for words in [&["push", "-h"][..], &["push", "-o", "--help", "origin"]] {
            unhinted(s, root, words);
        }
        // The pushes the hint names, in its order, succeed and set the upstream.
        committing(s, root);
        dupe_succeeds(s, root, &["commit", "-qm", "first"]);
        let output = s
            .git(["dupe", "push", "-u", "origin", "HEAD"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let merge = s
            .private(root)
            .git(["config", "branch.fresh.merge"])
            .succeeds();
        assert_eq!(merge.stdout, b"refs/heads/fresh\n");
    });
}

/// Git compares the raw value of `branch.<name>.merge` with `refs/heads/<name>` and
/// refuses to push to an upstream of another name under `push.default` unset or `simple`
/// (S13); a branch with `merge` and no `remote` has no upstream to it at all.
#[test]
fn a_push_to_an_upstream_of_another_name_names_both_pushes_git_offers() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe_succeeds(s, root, &["remote", "add", "backup", text(&backup)]);
        dupe_succeeds(s, root, &["switch", "-q", "-c", "topic"]);
        commit_here(s, root, "on-topic");
        dupe_succeeds(s, root, &["config", "branch.topic.remote", "origin"]);
        dupe_succeeds(
            s,
            root,
            &["config", "branch.topic.merge", "refs/heads/main"],
        );
        dupe_succeeds(s, root, &["config", "alias.p", "push"]);
        for words in [&["push"][..], &["push", "origin"], &["p"]] {
            let ours = hinted(s, root, words, &OTHER_NAME);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
            assert!(!holds(&ours.stderr, UPSTREAM), "{ours:?}");
        }
        runs(s, root, &["push"], End::Code(128), true, false);
        runs(s, root, &["push", "origin"], End::Code(128), true, false);
        for words in [&["push", "-h"][..], &["push", "-o", "--help"]] {
            unhinted(s, root, words);
        }
        // `simple` set, in the configuration or by a `-c` before `dupe`.
        dupe_succeeds(s, root, &["config", "push.default", "simple"]);
        assert_eq!(hinted(s, root, &["push"], &OTHER_NAME).end, End::Code(128));
        dupe_succeeds(s, root, &["config", "--unset", "push.default"]);
        let gits = s
            .private(root)
            .git([
                "-c",
                "push.default=simple",
                "-c",
                "help.autocorrect=0",
                "push",
            ])
            .run();
        let ours = s
            .git(["-c", "push.default=simple", "dupe", "push"])
            .from(root)
            .run();
        gits_then_one_hint(&ours, &gits, &OTHER_NAME);
        // A value without `refs/heads/` is another name to Git, the branch's own included.
        dupe_succeeds(s, root, &["config", "branch.topic.merge", "topic"]);
        let named = [
            &b"'git dupe push origin HEAD:topic'"[..],
            b"'git dupe push origin HEAD'",
        ];
        assert_eq!(hinted(s, root, &["push"], &named).end, End::Code(128));
        // Two values: Git refuses naming no command.
        dupe_succeeds(
            s,
            root,
            &["config", "branch.topic.merge", "refs/heads/main"],
        );
        dupe_succeeds(
            s,
            root,
            &["config", "--add", "branch.topic.merge", "refs/heads/other"],
        );
        assert_eq!(unhinted(s, root, &["push"]).end, End::Code(128));
        dupe_succeeds(s, root, &["config", "--unset-all", "branch.topic.merge"]);
        dupe_succeeds(
            s,
            root,
            &["config", "branch.topic.merge", "refs/heads/main"],
        );
        // With `merge` and no `remote`, `push` counts no upstream: the upstream hint.
        dupe_succeeds(s, root, &["config", "--unset", "branch.topic.remote"]);
        let ours = hinted(s, root, &["push"], &[UPSTREAM]);
        assert_eq!(ours.end, End::Code(128), "{ours:?}");
        assert!(!holds(&ours.stderr, OTHER_NAME[0]), "{ours:?}");
        dupe_succeeds(s, root, &["config", "branch.topic.remote", "origin"]);
        // Another remote is pushed to under the branch's own name, and one that names
        // nothing refuses it as Git does for any push there: no hint.
        succeeded_without_a_hint(&s.git(["dupe", "push", "backup"]).from(root).run());
        let nowhere = s.dir().join("nowhere.git");
        dupe_succeeds(s, root, &["remote", "add", "gone", text(&nowhere)]);
        assert_eq!(unhinted(s, root, &["push", "gone"]).end, End::Code(128));
        // Under `current`, a push that fails for a destination naming nothing: no hint.
        dupe_succeeds(
            s,
            root,
            &["config", "remote.origin.pushurl", text(&nowhere)],
        );
        dupe_succeeds(s, root, &["config", "push.default", "current"]);
        assert_eq!(unhinted(s, root, &["push"]).end, End::Code(128));
        dupe_succeeds(s, root, &["config", "--unset", "push.default"]);
        assert_eq!(hinted(s, root, &["push"], &OTHER_NAME).end, End::Code(128));
        dupe_succeeds(s, root, &["config", "--unset", "remote.origin.pushurl"]);
        // The first push the hint names moves the upstream to the branch's commit, though
        // the remote has a tag of the upstream's name, beside which its short name is
        // ambiguous.
        s.git(["tag", "main", "main"])
            .from(&t.private_remote)
            .succeeds();
        let output = s
            .git(["dupe", "push", "origin", "HEAD:refs/heads/main"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let pushed = s
            .git(["rev-parse", "refs/heads/main"])
            .from(&t.private_remote)
            .succeeds();
        let head = s.private(root).git(["rev-parse", "HEAD"]).succeeds();
        assert_eq!(pushed.stdout, head.stdout);
        // Under `current` and `upstream`, Git pushes.
        for default in ["current", "upstream"] {
            dupe_succeeds(s, root, &["config", "push.default", default]);
            succeeded_without_a_hint(&s.git(["dupe", "push"]).from(root).run());
        }
    });
}

#[test]
fn a_push_from_a_detached_head_names_the_push_of_head_to_a_branch() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe_succeeds(s, root, &["remote", "add", "backup", text(&backup)]);
        dupe_succeeds(s, root, &["checkout", "-q", "--detach"]);
        dupe_succeeds(s, root, &["config", "alias.p", "push"]);
        for words in [
            &["push"][..],
            &["push", "origin"],
            &["p"],
            &["push", "-v", "origin"],
        ] {
            let ours = hinted(s, root, words, &[DETACHED]);
            assert_eq!(ours.end, End::Code(128), "{words:?}: {ours:?}");
        }
        // The word typed is the remote named.
        let named = b"'git dupe push backup HEAD:refs/heads/<branch>'";
        assert_eq!(
            hinted(s, root, &["push", "backup"], &[named]).end,
            End::Code(128)
        );
        runs(s, root, &["push"], End::Code(128), true, false);
        runs(s, root, &["push", "backup"], End::Code(128), true, false);
        for words in [&["push", "-h"][..], &["push", "origin", "--help"]] {
            unhinted(s, root, words);
        }
        // Given a refspec, a push Git refuses for a destination naming nothing: no hint.
        let nowhere = s.dir().join("nowhere.git");
        dupe_succeeds(s, root, &["remote", "add", "gone", text(&nowhere)]);
        assert_eq!(
            unhinted(s, root, &["push", "gone", "HEAD:main"]).end,
            End::Code(128)
        );
        runs(
            s,
            root,
            &["push", "gone", "HEAD:main"],
            End::Code(128),
            false,
            false,
        );
        // The push the hint names moves the remote's branch to a commit made here.
        commit_here(s, root, "detached");
        let output = s
            .git(["dupe", "push", "origin", "HEAD:main"])
            .from(root)
            .run();
        succeeded_without_a_hint(&output);
        let pushed = s
            .git(["rev-parse", "main"])
            .from(&t.private_remote)
            .succeeds();
        let head = s.private(root).git(["rev-parse", "HEAD"]).succeeds();
        assert_eq!(pushed.stdout, head.stdout);
    });
}

/// The destination is written in full: a remote holding a tag of the branch's name refuses
/// the short one as matching more than one, and takes this (S13).
#[test]
fn the_detached_hints_destination_is_a_branch_whatever_tag_shares_its_name() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        dupe_succeeds(s, root, &["push", "origin", "main:release"]);
        dupe_succeeds(s, root, &["tag", "release"]);
        dupe_succeeds(s, root, &["push", "origin", "tag", "release"]);
        dupe_succeeds(s, root, &["checkout", "-q", "--detach"]);
        write(root, "notes/more", b"more\n");
        dupe_succeeds(s, root, &["add", "notes/more"]);
        committing(s, root);
        dupe_succeeds(s, root, &["commit", "-m", "more"]);
        assert_eq!(
            hinted(s, root, &["push", "origin"], &[DETACHED]).end,
            End::Code(128)
        );
        let short = s.git(["dupe", "push", "origin", "HEAD:release"]);
        assert_eq!(short.from(root).run().end, End::Code(1));
        let full = ["push", "origin", "HEAD:refs/heads/release"];
        dupe_succeeds(s, root, &full);
        let head = s.private(root).git(["rev-parse", "HEAD"]).succeeds();
        let arrived = s.git(["rev-parse", "refs/heads/release"]);
        assert_eq!(
            arrived.from(&t.private_remote).succeeds().stdout,
            head.stdout
        );
    });
}

/// `pull` merges into the branch checked out: the hint's words put the developer on the
/// branch pushed first, where the commands bring the remote's commits to that branch and
/// leave the one they stood on alone (`Holds/G18`).
#[test]
fn a_rejected_push_of_another_branch_is_pulled_on_that_branch() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        dupe_succeeds(s, root, &["switch", "-q", "-c", "topic"]);
        dupe_succeeds(s, root, &["push", "origin", "topic"]);
        let on_the_remote = s.dir().join("elsewhere");
        s.git([
            "clone",
            "-q",
            "-b",
            "topic",
            text(&t.private_remote),
            text(&on_the_remote),
        ])
        .succeeds();
        write(&on_the_remote, "theirs", b"theirs\n");
        s.git(["add", "theirs"]).from(&on_the_remote).succeeds();
        s.commit_public(&on_the_remote);
        s.git(["push", "-q", "origin", "topic"])
            .from(&on_the_remote)
            .succeeds();
        write(root, "notes/mine", b"mine\n");
        dupe_succeeds(s, root, &["add", "notes/mine"]);
        dupe_succeeds(s, root, &["commit", "-m", "mine"]);
        dupe_succeeds(s, root, &["switch", "-q", "main"]);

        let ours = hinted(
            s,
            root,
            &["push", "origin", "topic"],
            &[
                b"'git dupe pull origin <branch>'",
                b"run on the local branch the push sent",
                b"'git dupe switch <local-branch>' first where HEAD is on another",
                b"'git dupe fetch origin'",
            ],
        );
        assert_eq!(ours.end, End::Code(1), "{ours:?}");

        // The hint followed, with `topic` for both placeholders.
        let main = s.private(root).git(["rev-parse", "main"]).succeeds();
        dupe_succeeds(s, root, &["switch", "-q", "topic"]);
        dupe_succeeds(
            s,
            root,
            &["pull", "--no-rebase", "--no-edit", "origin", "topic"],
        );
        dupe_succeeds(s, root, &["push", "origin", "topic"]);
        assert_eq!(
            s.private(root).git(["rev-parse", "main"]).succeeds().stdout,
            main.stdout
        );
    });
}

#[test]
fn a_rejected_push_given_a_repository_names_its_pull_and_fetch() {
    under_each_release(|s| {
        without_maintenance(s);
        let t = s.pushed_workspace("workspace");
        let root = &t.root;
        committing(s, root);
        let backup = s.dir().join("backup.git");
        s.bare_repository(&backup);
        dupe_succeeds(s, root, &["remote", "add", "backup", text(&backup)]);
        diverged(s, &t, "first");
        // `backup`'s `dest` holds the commit the remote's `main` moved on to.
        s.git(["push", "-q", text(&backup), "main:dest"])
            .from(&t.private_remote)
            .succeeds();
        let origin = [
            &b"'git dupe pull origin <branch>'"[..],
            b"run on the local branch the push sent",
            b"'git dupe fetch origin'",
        ];
        for words in [
            &["push", "origin", "main"][..],
            &["push", "--all", "origin"],
            &["push", "origin"],
            // A word holding `=` takes no next word: the repository is `origin`.
            &["push", "--recurse-submodules=check", "origin", "main"],
        ] {
            let ours = hinted(s, root, words, &origin);
            assert_eq!(ours.end, End::Code(1), "{words:?}: {ours:?}");
            assert!(!holds(&ours.stderr, b"'git dupe pull'"), "{ours:?}");
        }
        let named = [
            &b"'git dupe pull backup <branch>'"[..],
            b"'git dupe fetch backup'",
        ];
        let ours = hinted(s, root, &["push", "backup", "HEAD:dest"], &named);
        assert_eq!(ours.end, End::Code(1), "{ours:?}");
        // Given as a path, the remote has no remote-tracking branch for a fetch to update: the
        // fetch and merge are claimed only where the fetch updates one.
        let path = as_git_spells(&t.private_remote);
        let path = path.as_str();
        let by_path = [
            format!("'git dupe pull {path} <branch>'"),
            format!("'git dupe fetch {path}'"),
            format!("where that fetch updates {path}/<branch>"),
        ];
        let by_path: Vec<&[u8]> = by_path.iter().map(|part| part.as_bytes()).collect();
        let ours = hinted(s, root, &["push", path, "main"], &by_path);
        assert_eq!(ours.end, End::Code(1), "{ours:?}");
        // A remote's name a shell reads back only quoted.
        dupe_succeeds(s, root, &["remote", "add", "it's", text(&t.private_remote)]);
        let quoted = [
            &b"'git dupe pull \"it's\" <branch>'"[..],
            b"'git dupe fetch \"it's\"'",
        ];
        assert_eq!(
            hinted(s, root, &["push", "it's", "main"], &quoted).end,
            End::Code(1)
        );
        runs(
            s,
            root,
            &["push", "origin", "main"],
            End::Code(1),
            false,
            false,
        );
        runs(s, root, &["push"], End::Code(1), true, false);
        for words in [
            &["push", "origin", "main", "-h"][..],
            &["push", "-o", "--help"],
        ] {
            unhinted(s, root, words);
        }
        // The pull the hint names brings the remote's commit, and the push then succeeds.
        let pulled = s
            .git(["dupe", "pull", "origin", "main", "--no-rebase", "--no-edit"])
            .from(root)
            .run();
        succeeded_without_a_hint(&pulled);
        let pushed = s.git(["dupe", "push", "origin", "main"]).from(root).run();
        succeeded_without_a_hint(&pushed);
    });
}
