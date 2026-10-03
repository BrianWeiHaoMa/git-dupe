//! The hint after a `push` given no repository whose private configuration chooses no
//! remote (`Holds/G18`, G25): Git's own run and output unchanged, then, when the run did
//! not exit 0, one `hint:` naming `git dupe remote add` and then `git dupe push -u`; none
//! where a key or `origin`'s URL chooses the remote, after a run that exits 0, after a
//! help request, after `pull` and `fetch`, and after `git dupe git push` (G19, G20, F9).

use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Runs, Scenario, Transfer, holds, names, run_traced, under_each_release,
};

/// The words of the hint that no line of Git's holds.
const HINT: &[u8] = b"'git dupe remote add <name> <url>'";

fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 scenario path")
}

/// `git dupe <words>` from the root `from`, and Git's own run of the words against the
/// private repository there, after `-c help.autocorrect=0` as git-dupe runs them, in the
/// same state; `git dupe git <words>`, guarded by nothing, ends as Git's own run (G20).
/// The words leave the state as it was.
fn ours_and_gits(s: &Scenario, from: &Path, words: &[&str]) -> (Output, Output) {
    let gits = s
        .private(from)
        .git(
            ["-c", "help.autocorrect=0"]
                .into_iter()
                .chain(words.iter().copied()),
        )
        .run();
    let unguarded = s
        .git(["dupe", "git"].into_iter().chain(words.iter().copied()))
        .from(from)
        .run();
    assert_eq!(unguarded, gits, "git dupe git {words:?}");
    let ours = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(from)
        .run();
    (ours, gits)
}

/// `git dupe <words>` ends as Git's own run of them, with every byte of its output, and
/// then the one hint, before anything settle adds.
fn hinted(s: &Scenario, from: &Path, words: &[&str]) {
    let (ours, gits) = ours_and_gits(s, from, words);
    assert_ne!(gits.end, End::Code(0), "{words:?}: {gits:?}");
    assert_eq!(ours.end, gits.end, "{words:?}: {ours:?}");
    assert_eq!(ours.stdout, gits.stdout, "{words:?}: {ours:?}");
    let hint = ours
        .stderr
        .strip_prefix(gits.stderr.as_slice())
        .and_then(|after| after.strip_prefix(b"hint: "))
        .and_then(|after| after.strip_suffix(b"\n"))
        .filter(|line| !line.contains(&b'\n'))
        .unwrap_or_else(|| panic!("{words:?}: not Git's output and one hint: {ours:?}"));
    names(hint, HINT);
    names(hint, b"'git dupe push -u <name> <branch>'");
}

/// `git dupe <words>` ends as Git's own run of them, with every byte of its output and
/// nothing of git-dupe's.
fn unhinted(s: &Scenario, from: &Path, words: &[&str]) {
    let (ours, gits) = ours_and_gits(s, from, words);
    assert_eq!(ours, gits, "{words:?}");
}

/// The command word of each run git-dupe started itself, in order.
fn commands(runs: &Runs) -> Vec<String> {
    let own = runs.own();
    own.commands()
        .iter()
        .map(|word| String::from_utf8_lossy(word).into_owned())
        .collect()
}

fn config(s: &Scenario, t: &Transfer, words: &[&str]) {
    s.git(
        ["dupe", "git", "config"]
            .into_iter()
            .chain(words.iter().copied()),
    )
    .from(&t.root)
    .succeeds();
}

#[test]
fn a_failed_push_with_no_remote_chosen_ends_with_one_hint() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let before = t.everything();
        hinted(s, root, &["push"]);
        before.unchanged();
        config(s, &t, &["alias.p", "push"]);
        hinted(s, root, &["p"]);
        // A repository given, a help request, or a `-h` or `--help` Git takes as a value
        // of `-o` before it pushes: Git's own output alone.
        for words in [
            &["push", "nosuch"][..],
            &["push", "--repo=nosuch"],
            &["push", "-h"],
            &["push", "-o", "--", "-h"],
            &["push", "-o", "-h"],
            &["push", "-o", "--help"],
            &["fetch"],
            &["pull"],
        ] {
            unhinted(s, root, words);
        }
        // Inside the Git directory only a help request runs, and it is Git's alone.
        let git_directory = root.join(".git");
        for words in [&["push", "-h"][..], &["push", "-o", "--", "-h"]] {
            let expected = s
                .git(
                    ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied()),
                )
                .variable("GIT_DIR", git_directory.join("dupe"))
                .from(&git_directory)
                .run();
            let ours = s
                .git(["dupe"].into_iter().chain(words.iter().copied()))
                .from(&git_directory)
                .run();
            assert_eq!(ours, expected, "{words:?} inside the Git directory");
        }
        s.git(["dupe", "checkout", "-q", "--detach"])
            .from(root)
            .succeeds();
        hinted(s, root, &["push"]);
        s.git(["dupe", "checkout", "-q", "--orphan", "unborn"])
            .from(root)
            .succeeds();
        hinted(s, root, &["push"]);
    });
}

#[test]
fn the_hint_follows_whichever_remote_git_takes_and_no_chosen_one() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let backup = text(&t.private_remote);
        s.git(["dupe", "remote", "add", "backup", backup])
            .from(root)
            .succeeds();
        // Git takes `backup`, the only remote, and fails for want of an upstream.
        hinted(s, root, &["push"]);
        config(s, &t, &["push.default", "nothing"]);
        let trace = s.dir().join("trace");
        let words = ["dupe", "push"];
        let (output, hinting) = run_traced(s.git(words).from(root), &trace);
        assert_ne!(output.end, End::Code(0), "{output:?}");
        assert!(holds(&output.stderr, HINT), "{output:?}");
        // Each key that chooses a remote: the same failure, and nothing of git-dupe's.
        for key in [
            "branch.main.pushRemote",
            "remote.pushDefault",
            "branch.main.remote",
        ] {
            config(s, &t, &[key, "backup"]);
            unhinted(s, root, &["push"]);
            if key == "branch.main.pushRemote" {
                // The runs of a push owed the hint are those of one owed none, the
                // sequence of a transfer command given no repository (G22).
                let (output, choosing) = run_traced(s.git(words).from(root), &trace);
                assert!(!holds(&output.stderr, HINT), "{output:?}");
                assert_eq!(commands(&hinting), commands(&choosing));
                let sequence = [
                    "rev-parse",
                    "config",
                    "worktree",
                    "config",
                    "symbolic-ref",
                    "push",
                    "ls-files",
                    "ls-files",
                    "check-ignore",
                ];
                assert_eq!(commands(&hinting), sequence);
            }
            config(s, &t, &["--unset", key]);
        }
        // A URL of `origin`, or a push URL alone, chooses it.
        s.git(["dupe", "remote", "add", "origin", backup])
            .from(root)
            .succeeds();
        unhinted(s, root, &["push"]);
        config(s, &t, &["--unset", "remote.origin.url"]);
        config(s, &t, &["remote.origin.pushurl", backup]);
        unhinted(s, root, &["push"]);
        s.git(["dupe", "remote", "remove", "origin"])
            .from(root)
            .succeeds();
        config(s, &t, &["--unset", "push.default"]);
        // Two remotes and neither `origin`: Git takes `origin`, which has no destination.
        let other = s.dir().join("other.git");
        s.bare_repository(&other);
        s.git(["dupe", "remote", "add", "other", text(&other)])
            .from(root)
            .succeeds();
        hinted(s, root, &["push"]);
    });
}

#[test]
fn a_push_to_a_remote_a_file_defines_succeeds_without_the_hint() {
    under_each_release(|s| {
        let t = s.transfer_workspace("workspace");
        let root = &t.root;
        let private = root.join(".git/dupe");
        let branches = s.dir().join("branches.git");
        s.bare_repository(&branches);
        for (file, line, destination) in [
            (
                "remotes/origin",
                format!(
                    "URL: {}\nPush: refs/heads/main:refs/heads/main\n",
                    text(&t.private_remote)
                ),
                &t.private_remote,
            ),
            (
                "branches/origin",
                format!("{}#main\n", text(&branches)),
                &branches,
            ),
        ] {
            let path = private.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, line).unwrap();
            let output = s.git(["dupe", "push"]).from(root).run();
            assert_eq!(output.end, End::Code(0), "{file}: {output:?}");
            assert!(!holds(&output.stderr, HINT), "{file}: {output:?}");
            let head = s.private(root).git(["rev-parse", "HEAD"]).succeeds();
            let received = s
                .git(["rev-parse", "refs/heads/main"])
                .from(destination)
                .succeeds();
            assert_eq!(head.stdout, received.stdout, "{file}");
            fs::remove_file(&path).unwrap();
        }
    });
}
