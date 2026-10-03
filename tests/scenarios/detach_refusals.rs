//! What `git dupe detach` refuses, and that a refusal changes nothing anywhere.

use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::harness::{
    End, Everything, Output, Scenario, Transfer, Tree, holds, names, private_add, private_commit,
    region, run_traced, stale_file_timestamp, stale_timestamp, unchanged, under_each_release,
    write,
};

fn before_refusal(transfer: &Transfer) -> Everything {
    stale_timestamp(&transfer.root);
    transfer.everything()
}

fn refusal(output: &Output) -> &[u8] {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, b"git dupe detach --force");
    line
}

fn refused(s: &Scenario, transfer: &Transfer) -> Vec<u8> {
    let before = before_refusal(transfer);
    let output = s.git(["dupe", "detach"]).from(&transfer.root).run();
    let line = refusal(&output).to_vec();
    before.unchanged();
    assert!(transfer.root.join(".git/dupe").is_dir());
    assert!(region(&transfer.root).is_some());
    line
}

fn clean(s: &Scenario, root: &Path) {
    let status = s
        .private(root)
        .git([
            "--no-optional-locks",
            "status",
            "--porcelain",
            "-z",
            "--untracked-files=no",
        ])
        .succeeds();
    assert!(status.stdout.is_empty(), "{status:?}");
}

#[test]
fn uncommitted_modification_addition_and_deletion() {
    under_each_release(|s| {
        let mut lines = Vec::new();
        for cause in ["modified", "added", "deleted"] {
            let transfer = s.pushed_workspace(cause);
            let root = &transfer.root;
            match cause {
                "modified" => write(root, "notes/a.md", b"changed\n"),
                "added" => {
                    write(root, "notes/new.md", b"new\n");
                    private_add(s, root, "notes/new.md");
                }
                "deleted" => {
                    s.private(root)
                        .git(["rm", "--cached", "--", "notes/a.md"])
                        .succeeds();
                }
                _ => unreachable!(),
            }
            let line = refused(s, &transfer);
            names(&line, b"not committed");
            lines.push(line);
        }
        assert!(lines.windows(2).all(|pair| pair[0] == pair[1]));

        let unpushed = s.pushed_workspace("unpushed");
        write(&unpushed.root, "notes/a.md", b"committed\n");
        private_add(s, &unpushed.root, "notes/a.md");
        private_commit(s, &unpushed.root);
        let line = refused(s, &unpushed);
        names(&line, b"no remote-tracking branch");
        assert_ne!(line, lines[0]);
    });
}

#[test]
fn unpushed_stash_tag_and_second_branch() {
    under_each_release(|s| {
        let mut lines = Vec::new();
        for cause in ["stash", "older-stash", "tag", "branch"] {
            let transfer = s.pushed_workspace(cause);
            let root = &transfer.root;
            if cause == "stash" {
                stash(s, root, b"stashed\n");
            } else if cause == "older-stash" {
                // Every stash entry counts, not only the newest: here the newest is on a
                // remote-tracking branch and the older one alone is on none.
                stash(s, root, b"older\n");
                stash(s, root, b"newest\n");
                s.private(root)
                    .git(["push", "-q", "origin", "refs/stash:refs/heads/kept"])
                    .succeeds();
                s.private(root).git(["fetch", "-q", "origin"]).succeeds();
                let unpushed = |entry: &str| {
                    let found = s
                        .private(root)
                        .git(["rev-list", "-n", "1", entry, "--not", "--remotes"])
                        .succeeds();
                    !found.stdout.is_empty()
                };
                assert!(!unpushed("stash@{0}"));
                assert!(unpushed("stash@{1}"));
            } else {
                let checkout = if cause == "tag" {
                    ["checkout", "--detach", "main"]
                } else {
                    ["checkout", "-b", "second"]
                };
                s.private(root).git(checkout).succeeds();
                write(root, "notes/a.md", b"committed\n");
                private_add(s, root, "notes/a.md");
                private_commit(s, root);
                if cause == "tag" {
                    s.private(root).git(["tag", "only-tag"]).succeeds();
                }
                s.private(root).git(["checkout", "main"]).succeeds();
            }
            clean(s, root);
            let line = refused(s, &transfer);
            names(&line, b"no remote-tracking branch");
            lines.push(line);
        }
        assert!(lines.windows(2).all(|pair| pair[0] == pair[1]));
    });
}

/// Stashes a change of `notes/a.md` to `content`, leaving the tree clean.
fn stash(s: &Scenario, root: &Path, content: &[u8]) {
    write(root, "notes/a.md", content);
    s.private(root)
        .git([
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "stash",
            "push",
            "-q",
        ])
        .succeeds();
}

#[test]
fn no_remote_counts_commits_even_with_stale_remote_tracking_refs() {
    under_each_release(|s| {
        let removed = s.pushed_workspace("removed");
        s.private(&removed.root)
            .git(["config", "--remove-section", "remote.origin"])
            .succeeds();
        s.private(&removed.root)
            .git(["show-ref", "--verify", "refs/remotes/origin/main"])
            .succeeds();
        clean(s, &removed.root);
        let removed_line = refused(s, &removed);
        names(&removed_line, b"no remote is configured");

        let never = s.transfer_workspace("never-configured");
        clean(s, &never.root);
        let never_line = refused(s, &never);
        assert_eq!(removed_line, never_line);

        let pushed = s.pushed_workspace("pushed");
        s.git(["dupe", "detach"]).from(&pushed.root).succeeds();
        assert!(!pushed.root.join(".git/dupe").exists());
    });
}

#[test]
fn unreadable_missing_head_and_empty_private_directory() {
    under_each_release(|s| {
        for cause in ["missing-head", "empty-directory"] {
            let transfer = s.pushed_workspace(cause);
            stale_timestamp(&transfer.root);
            let private = transfer.root.join(".git/dupe");
            if cause == "missing-head" {
                fs::remove_file(private.join("HEAD")).unwrap();
            } else {
                fs::remove_dir_all(&private).unwrap();
                fs::create_dir(&private).unwrap();
            }
            let before = transfer.everything();
            let output = s.git(["dupe", "detach"]).from(&transfer.root).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let lines: Vec<_> = output
                .stderr
                .split_inclusive(|&byte| byte == b'\n')
                .collect();
            assert!(lines.len() > 1, "{output:?}");
            let last = lines.last().unwrap().strip_prefix(b"fatal: ").unwrap();
            names(last, b"git dupe init");
            names(last, b"git dupe detach --force");
            assert!(output.lines("warning").is_empty(), "{output:?}");
            before.unchanged();
            assert!(private.is_dir());
            assert!(region(&transfer.root).is_some());
        }
    });
}

#[test]
fn untracked_files_do_not_refuse_even_when_status_config_lists_them() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("untracked");
        s.private(&transfer.root)
            .git(["config", "status.showUntrackedFiles", "all"])
            .succeeds();
        write(&transfer.root, "notes/untracked.md", b"untracked\n");
        s.git(["dupe", "detach"]).from(&transfer.root).succeeds();
        assert!(!transfer.root.join(".git/dupe").exists());
    });
}

#[test]
fn unborn_branch_is_safe_until_hide_stages_gitdupe() {
    under_each_release(|s| {
        let empty = s.dir().join("empty");
        s.attached_project(&empty);
        s.git(["dupe", "detach"]).from(&empty).succeeds();
        assert!(!empty.join(".git/dupe").exists());

        let staged = s.dir().join("staged");
        s.attached_project(&staged);
        fs::create_dir(staged.join("notes")).unwrap();
        s.git(["dupe", "hide", "notes"]).from(&staged).succeeds();
        stale_file_timestamp(&staged.join(".gitdupe"));
        let before = Tree::of(&staged);
        let output = s.git(["dupe", "detach"]).from(&staged).run();
        names(refusal(&output), b"not committed");
        unchanged(&before, &staged);
        assert!(staged.join(".git/dupe").is_dir());
        assert!(region(&staged).is_some());
    });
}

#[test]
fn stale_region_is_not_settled_on_refusal() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("stale");
        let root = &transfer.root;
        let exclude = root.join(".git/info/exclude");
        let region_before = fs::read(&exclude).unwrap();
        write(root, ".gitdupe", b"notes\n.vscode\nextra\n");
        write(root, "notes/a.md", b"uncommitted\n");
        let before = before_refusal(&transfer);
        let output = s.git(["dupe", "detach"]).from(root).run();
        names(refusal(&output), b"not committed");
        assert!(output.lines("warning").is_empty(), "{output:?}");
        assert_eq!(fs::read(&exclude).unwrap(), region_before);
        before.unchanged();
        assert!(root.join(".git/dupe").is_dir());
        assert!(region(root).is_some());
    });
}

#[test]
fn info_link_with_region_refuses_even_force_before_private_runs() {
    under_each_release(|s| {
        let transfer = s.pushed_workspace("linked-info");
        let root = &transfer.root;
        let info = root.join(".git/info");
        let target = s.dir().join("info-elsewhere");
        fs::rename(&info, &target).unwrap();
        symlink(&target, &info).unwrap();
        let before = before_refusal(&transfer);
        // Everything records the link; explicitly snapshot the directory beyond it too.
        let beyond = Tree::of(&target);
        for words in [
            ["dupe", "detach"].as_slice(),
            &["dupe", "detach", "--force"],
        ] {
            let (output, runs) = run_traced(s.git(words).from(root), &s.dir().join("trace"));
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            names(output.only_line("fatal"), info.as_os_str().as_bytes());
            let own = runs.own();
            assert_eq!(own.commands(), [b"rev-parse".as_slice()], "{own:?}");
            before.unchanged();
            unchanged(&beyond, &target);
            assert!(root.join(".git/dupe").is_dir());
            assert!(region(root).is_some());
        }
    });
}

#[test]
fn info_link_without_region_allows_detach_and_preserves_target() {
    under_each_release(|s| {
        let root = s.dir().join("no-region");
        s.unattached_project(&root);
        let info = root.join(".git/info");
        let target = s.dir().join("info-without-region");
        fs::rename(&info, &target).unwrap();
        write(&target, "exclude", b"# user excludes\n*.local\n");
        symlink(&target, &info).unwrap();
        s.init(&root);
        assert!(region(&root).is_none());
        let link_before = fs::read_link(&info).unwrap();
        let beyond = Tree::of(&target);
        let output = s.git(["dupe", "detach"]).from(&root).succeeds();
        assert!(!root.join(".git/dupe").exists());
        assert_eq!(fs::read_link(&info).unwrap(), link_before);
        unchanged(&beyond, &target);
        assert!(
            !output
                .lines("warning")
                .iter()
                .any(|line| holds(line, b"region cannot be maintained")),
            "{output:?}"
        );
    });
}
