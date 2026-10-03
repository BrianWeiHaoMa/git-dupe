//! Which branch `git dupe clone` checks out, and how it ends where there is none to check
//! out (G2, `Holds/G2`, S12): `-b BRANCH`, else the remote's default branch; a remote with
//! no branch attaches with nothing checked out; a remote whose `HEAD` names no branch it
//! has is refused naming `-b`; a `BRANCH` the remote lacks and a `URL` that is no
//! repository end with Git's status, attached; and from each, `git dupe detach --force`
//! and `git dupe clone` start over.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, names, private_add, private_commit, region_rules, under_each_release,
    usage_line, write,
};

/// `git dupe clone <words>` from `dir`.
fn clone(s: &Scenario, dir: &Path, words: &[&OsStr]) -> Output {
    let dupe = [OsStr::new("dupe"), OsStr::new("clone")];
    s.git(dupe.iter().chain(words)).from(dir).run()
}

/// `git dupe detach --force` from `dir`, which must succeed.
fn detached(s: &Scenario, dir: &Path) {
    let output = s.git(["dupe", "detach", "--force"]).from(dir).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(!dir.join(".git/dupe").exists());
}

/// The workspace at `dir` attached from the first machine's history on `main`: the files
/// written, `main` checked out and tracking `origin`.
fn attached_on_main(s: &Scenario, dir: &Path, first: &Path) {
    assert_eq!(
        fs::read(dir.join("notes/a.md")).unwrap(),
        fs::read(first.join("notes/a.md")).unwrap()
    );
    let head = s.private(dir).git(["symbolic-ref", "HEAD"]).succeeds();
    assert_eq!(head.stdout, b"refs/heads/main\n");
    let upstream = s
        .private(dir)
        .git(["rev-parse", "--symbolic-full-name", "main@{upstream}"])
        .succeeds();
    assert_eq!(upstream.stdout, b"refs/remotes/origin/main\n");
}

/// The workspace at `dir` is attached and was settled: its region holds a rule.
fn attached_and_settled(dir: &Path) {
    assert!(dir.join(".git/dupe").is_dir());
    assert!(!region_rules(dir).is_empty());
}

/// The local branches of the private repository at `dir`.
fn local_branches(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.private(dir)
        .git(["for-each-ref", "--format=%(refname)", "refs/heads/"])
        .succeeds()
        .stdout
}

#[test]
fn b_checks_out_that_branch_tracking_origin() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        s.private(first)
            .git(["checkout", "-q", "-b", "other"])
            .succeeds();
        write(first, "notes/other.md", b"only on other\n");
        private_add(s, first, "notes/other.md");
        private_commit(s, first);
        s.private(first)
            .git(["push", "-q", "origin", "other"])
            .succeeds();

        let url = m.first.private_remote.as_os_str();
        let output = clone(s, second, &[url, OsStr::new("-b"), OsStr::new("other")]);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.lines("fatal").is_empty(), "{output:?}");
        let head = s.private(second).git(["symbolic-ref", "HEAD"]).succeeds();
        assert_eq!(head.stdout, b"refs/heads/other\n");
        let upstream = s
            .private(second)
            .git(["rev-parse", "--symbolic-full-name", "other@{upstream}"])
            .succeeds();
        assert_eq!(upstream.stdout, b"refs/remotes/origin/other\n");
        assert_eq!(local_branches(s, second), b"refs/heads/other\n");
        assert_eq!(
            fs::read(second.join("notes/other.md")).unwrap(),
            b"only on other\n"
        );
    });
}

#[test]
fn a_branch_the_remote_lacks_ends_with_gits_status_attached_and_starts_over() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let url = m.first.private_remote.as_os_str();
        let failed = clone(s, second, &[url, OsStr::new("-b"), OsStr::new("missing")]);
        attached_and_settled(second);
        // What was done stays: `origin` configured and fetched.
        let origin = s
            .private(second)
            .git(["config", "--get", "remote.origin.url"])
            .succeeds();
        assert_eq!(origin.stdout, [url.as_bytes(), b"\n"].concat());
        s.private(second)
            .git(["rev-parse", "-q", "--verify", "refs/remotes/origin/main"])
            .succeeds();
        // The step that failed, run again as Git: the same end and message, and nothing of
        // git-dupe's after it.
        let gits = s
            .private(second)
            .git([
                "branch",
                "--track",
                "--",
                "missing",
                "refs/remotes/origin/missing",
            ])
            .run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(failed.end, gits.end, "{failed:?}");
        assert!(failed.stderr.ends_with(&gits.stderr), "{failed:?}");
        assert!(local_branches(s, second).is_empty());

        detached(s, second);
        let again = clone(s, second, &[url]);
        assert_eq!(again.end, End::Code(0), "{again:?}");
        attached_on_main(s, second, first);
    });
}

#[test]
fn the_default_branch_is_checked_out_whatever_tags_or_dashes_the_remote_holds() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let remote = &m.first.private_remote;
        let url = remote.as_os_str();
        let main = s
            .git(["rev-parse", "refs/heads/main"])
            .from(remote)
            .succeeds();
        let hash = String::from_utf8(main.stdout).unwrap();
        let hash = hash.trim();

        // A tag named as the remote-tracking branch is, which the fetch brings along.
        s.git(["update-ref", "refs/tags/origin/main", hash])
            .from(remote)
            .succeeds();
        let output = clone(s, second, &[url]);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        attached_on_main(s, second, first);
        detached(s, second);

        // A tag named as the remote-tracking branch's whole name is.
        s.git(["update-ref", "refs/tags/refs/remotes/origin/main", hash])
            .from(remote)
            .succeeds();
        let output = clone(s, second, &[url]);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        attached_on_main(s, second, first);
        detached(s, second);

        // A default branch whose name Git would read as an option of `branch`: Git
        // refuses it as a branch name, and nothing is checked out.
        s.git(["update-ref", "refs/heads/--list", hash])
            .from(remote)
            .succeeds();
        s.git(["symbolic-ref", "HEAD", "refs/heads/--list"])
            .from(remote)
            .succeeds();
        let failed = clone(s, second, &[url]);
        attached_and_settled(second);
        let gits = s
            .private(second)
            .git([
                "branch",
                "--track",
                "--",
                "--list",
                "refs/remotes/origin/--list",
            ])
            .run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(failed.end, gits.end, "{failed:?}");
        assert!(failed.stderr.ends_with(&gits.stderr), "{failed:?}");
        assert!(local_branches(s, second).is_empty());
        detached(s, second);
        let b_main = [url, OsStr::new("-b"), OsStr::new("main")];
        let cloned = clone(s, second, &b_main);
        assert_eq!(cloned.end, End::Code(0), "{cloned:?}");
        attached_on_main(s, second, first);
    });
}

#[test]
fn a_url_or_branch_beginning_with_a_dash_is_a_usage_error_and_dot_slash_is_a_path() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let remote = &m.first.private_remote;
        // A branch Git would take `--list` for, on the remote.
        let main = s
            .git(["rev-parse", "refs/heads/main"])
            .from(remote)
            .succeeds();
        let hash = String::from_utf8(main.stdout).unwrap();
        s.git(["update-ref", "refs/heads/--list", hash.trim()])
            .from(remote)
            .succeeds();

        let text = s.git(["dupe", "help", "clone"]).succeeds().stdout;
        let url = remote.as_os_str();
        for words in [
            &[url, OsStr::new("-b"), OsStr::new("--list")][..],
            &[OsStr::new("--"), OsStr::new("-x")],
            &[OsStr::new("-b"), OsStr::new("-"), url],
        ] {
            let output = clone(s, second, words);
            assert_eq!(output.end, End::Code(129), "{words:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            output.line_then("error", usage_line(&text));
            assert!(!second.join(".git/dupe").exists(), "{words:?}");
        }

        // `./-x`, a repository at that path below the root, is a URL like any other.
        let dashed = second.join("-x");
        let words = [OsStr::new("clone"), OsStr::new("-q"), OsStr::new("--bare")];
        s.git(words.iter().chain([&url, &dashed.as_os_str()]))
            .succeeds();
        let output = clone(s, second, &[OsStr::new("./-x")]);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        attached_on_main(s, second, first);
        let configured = s
            .private(second)
            .git(["config", "--get", "remote.origin.url"])
            .succeeds();
        assert_eq!(configured.stdout, b"./-x\n");
    });
}

#[test]
fn a_remote_without_a_branch_attaches_with_origin_and_no_commit() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let second = &m.root;
        let empty = s.dir().join("empty.git");
        s.bare_repository(&empty);
        let output = clone(s, second, &[empty.as_os_str()]);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.lines("fatal").is_empty(), "{output:?}");
        assert!(output.lines("warning").is_empty(), "{output:?}");
        let hint = output.lines("hint");
        assert_eq!(hint.len(), 1, "{output:?}");
        names(hint[0], b"git dupe push -u origin HEAD");

        attached_and_settled(second);
        let configured = s
            .private(second)
            .git(["config", "--get", "remote.origin.url"])
            .succeeds();
        assert_eq!(
            configured.stdout,
            [empty.as_os_str().as_bytes(), b"\n"].concat()
        );
        let commit = s
            .private(second)
            .git(["rev-parse", "--verify", "-q", "HEAD"])
            .run();
        assert_eq!(commit.end, End::Code(1), "{commit:?}");
        let status = s.git(["dupe", "status"]).from(second).run();
        assert_eq!(status.end, End::Code(0), "{status:?}");

        // What the hint says to do next starts the remote's history.
        write(second, "notes/first.md", b"the first private file\n");
        let identity = [
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=s@example.invalid",
        ];
        for words in [
            &["add", "notes/"][..],
            &["commit", "-qm", "first"],
            &["push", "-u", "origin", "HEAD"],
        ] {
            let output = s
                .git(identity.iter().chain(&["dupe"]).chain(words))
                .from(second)
                .run();
            assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
        }
        let pushed = s
            .git(["for-each-ref", "refs/heads/"])
            .from(&empty)
            .succeeds();
        assert!(!pushed.stdout.is_empty(), "{pushed:?}");
    });
}

#[test]
fn a_remote_head_naming_no_branch_is_refused_naming_b_and_b_clones() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let url = m.first.private_remote.as_os_str();
        let hashed = s.dir().join("hashed.git");
        let missing = s.dir().join("missing.git");
        for bare in [&hashed, &missing] {
            let words = [OsStr::new("clone"), OsStr::new("-q"), OsStr::new("--bare")];
            s.git(words.iter().chain([&url, &bare.as_os_str()]))
                .succeeds();
        }
        let main = s.git(["rev-parse", "main"]).from(&hashed).succeeds();
        let hash = String::from_utf8(main.stdout).unwrap();
        s.git(["update-ref", "--no-deref", "HEAD", hash.trim()])
            .from(&hashed)
            .succeeds();
        s.git(["symbolic-ref", "HEAD", "refs/heads/absent"])
            .from(&missing)
            .succeeds();

        // Without `-b`: refused naming it, attached with `origin` fetched and no branch;
        // the route the line names, then `-b`, clones. The first starts over by plain
        // `detach`, which the line names, the second by `detach --force`.
        for (remote, force) in [(&hashed, false), (&missing, true)] {
            let refused = clone(s, second, &[remote.as_os_str()]);
            assert_eq!(refused.end, End::Code(128), "{refused:?}");
            let fatal = refused.lines("fatal");
            assert_eq!(fatal.len(), 1, "{refused:?}");
            names(fatal[0], b"-b");
            names(fatal[0], b"git dupe detach");
            assert!(refused.stderr.ends_with(b"BRANCH\n"), "{refused:?}");
            attached_and_settled(second);
            assert!(local_branches(s, second).is_empty());
            let fetched = s
                .private(second)
                .git(["for-each-ref", "refs/remotes/origin/"])
                .succeeds();
            assert!(!fetched.stdout.is_empty());

            let b_main = [remote.as_os_str(), OsStr::new("-b"), OsStr::new("main")];
            let attached = clone(s, second, &b_main);
            assert_eq!(attached.end, End::Code(128), "{attached:?}");
            names(attached.only_line("fatal"), b"already attached");
            if force {
                detached(s, second);
            } else {
                let plain = s.git(["dupe", "detach"]).from(second).run();
                assert_eq!(plain.end, End::Code(0), "{plain:?}");
            }
            let cloned = clone(s, second, &b_main);
            assert_eq!(cloned.end, End::Code(0), "{cloned:?}");
            attached_on_main(s, second, first);
            detached(s, second);
        }
    });
}

#[test]
fn a_url_that_is_no_repository_ends_with_fetchs_status_attached_and_starts_over() {
    under_each_release(|s| {
        let m = s.second_machine("project");
        let (first, second) = (&m.first.root, &m.root);
        let nothing = s.dir().join("nothing-here");
        let failed = clone(s, second, &[nothing.as_os_str()]);
        attached_and_settled(second);
        let gits = s.private(second).git(["fetch", "origin"]).run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(failed.end, gits.end, "{failed:?}");
        assert!(failed.stderr.ends_with(&gits.stderr), "{failed:?}");

        detached(s, second);
        let again = clone(s, second, &[m.first.private_remote.as_os_str()]);
        assert_eq!(again.end, End::Code(0), "{again:?}");
        attached_on_main(s, second, first);
    });
}
