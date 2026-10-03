//! `git dupe remote` wherever it can stand: unattached, in a linked worktree, inside a
//! Git directory, and outside any repository; a help request among its words is compared
//! as any other word wherever a run could follow (G18, `Holds/G18`, `Holds/G24`, G4).

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, holds, names, run_traced, unchanged, under_each_release,
};

fn refused(output: &Output, route: &[u8], place: &[u8]) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, route);
    names(line, place);
}

fn git_directory(s: &Scenario, worktree: &Path) -> PathBuf {
    let answer = s
        .git(["rev-parse", "--absolute-git-dir"])
        .from(worktree)
        .succeeds();
    Path::new(OsStr::from_bytes(
        answer.stdout.strip_suffix(b"\n").unwrap(),
    ))
    .to_path_buf()
}

fn help_in_git_directory(s: &Scenario, directory: &Path) -> Output {
    s.git(["-c", "help.autocorrect=0", "remote", "-h"])
        .variable("GIT_DIR", directory.join("dupe"))
        .from(directory)
        .run()
}

/// A help word used as an option value cannot configure a public destination from `.git`.
#[test]
fn git_directory_refuses_public_destinations_without_settling() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let directory = transfer.root.join(".git");
        for (remote, destination, place) in [
            (
                "leak",
                "https://example.com/team/project",
                b"origin".as_slice(),
            ),
            (
                "leak2",
                transfer.root.to_str().unwrap(),
                transfer.root.as_os_str().as_bytes(),
            ),
            (
                "leak3",
                transfer.linked.to_str().unwrap(),
                transfer.linked.as_os_str().as_bytes(),
            ),
        ] {
            let before = transfer.everything();
            let (output, runs) = run_traced(
                s.git(["dupe", "remote", "add", "-t", "-h", remote, destination])
                    .from(&directory),
                &s.dir().join("refusal-trace"),
            );
            refused(&output, b"git dupe git", place);
            before.unchanged();
            let own = runs.own();
            assert_eq!(own.of("remote"), 0, "{own:?}");
            assert_eq!(own.of("ls-files"), 0, "{own:?}");
            assert_eq!(own.of("check-ignore"), 0, "{own:?}");
        }
    });
}

/// An allowed destination with `-h` as `-t`'s value reaches the private configuration.
#[test]
fn git_directory_runs_allowed_destination_with_help_as_value() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let output = s
            .git([
                "dupe",
                "remote",
                "add",
                "-t",
                "-h",
                "ok",
                "https://example.org/team/x",
            ])
            .from(&transfer.root.join(".git"))
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let configured = s
            .private(&transfer.root)
            .git(["config", "--get", "remote.ok.url"])
            .succeeds();
        assert_eq!(configured.stdout, b"https://example.org/team/x\n");
    });
}

/// Even a help run from a linked Git directory compares the main working tree's name.
#[test]
fn linked_git_directory_refuses_the_main_working_tree() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let directory = git_directory(s, &transfer.linked);
        let before = transfer.everything();
        let output = s
            .git([
                "dupe",
                "remote",
                "add",
                "-t",
                "-h",
                "leak",
                transfer.root.to_str().unwrap(),
            ])
            .from(&directory)
            .run();
        refused(
            &output,
            b"git dupe git",
            transfer.root.as_os_str().as_bytes(),
        );
        before.unchanged();
    });
}

/// Without a help request, the linked worktree requires attaching the main working tree.
#[test]
fn linked_worktree_requires_init_in_the_main_working_tree() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let before = transfer.everything();
        let output = s.git(["dupe", "remote", "-v"]).from(&transfer.linked).run();
        refused(
            &output,
            b"git dupe init",
            transfer.root.as_os_str().as_bytes(),
        );
        before.unchanged();
    });
}

/// The public destination guard precedes the attachment refusal for a help request.
#[test]
fn linked_worktree_refuses_public_destination_before_init() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let before = transfer.everything();
        let output = s
            .git([
                "dupe",
                "remote",
                "add",
                "-t",
                "-h",
                "leak",
                transfer.root.to_str().unwrap(),
            ])
            .from(&transfer.linked)
            .run();
        refused(
            &output,
            b"git dupe git",
            transfer.root.as_os_str().as_bytes(),
        );
        assert!(!holds(output.only_line("fatal"), b"git dupe init"));
        before.unchanged();
    });
}

/// Linked worktree help keeps Git's answer against its never-created private directory.
#[test]
fn linked_worktree_help_runs_against_its_own_missing_private_repository() {
    under_each_release(|s| {
        let transfer = s.transfer_workspace("workspace");
        let directory = git_directory(s, &transfer.linked);
        let before = transfer.everything();
        let expected = s
            .git(["-c", "help.autocorrect=0", "remote", "-h"])
            .variable("GIT_DIR", directory.join("dupe"))
            .variable("GIT_WORK_TREE", &transfer.linked)
            .from(&transfer.linked)
            .run();
        let output = s.git(["dupe", "remote", "-h"]).from(&transfer.linked).run();
        assert_eq!(output, expected);
        assert!(!directory.join("dupe").exists());
        before.unchanged();
    });
}

/// Ordinary unattached repositories require init, but help retains Git's private answer.
#[test]
fn unattached_repository_requires_init_and_keeps_gits_help_answer() {
    under_each_release(|s| {
        let root = s.dir().join("workspace");
        s.repository(&root);
        let before = Tree::of(&root);
        let output = s.git(["dupe", "remote", "-v"]).from(&root).run();
        refused(&output, b"git dupe init", b"git dupe init");
        unchanged(&before, &root);
        let expected = s
            .private(&root)
            .git(["-c", "help.autocorrect=0", "remote", "-h"])
            .run();
        let output = s.git(["dupe", "remote", "-h"]).from(&root).run();
        assert_eq!(output, expected);
        unchanged(&before, &root);
        assert!(!root.join(".git/dupe").exists());
    });
}

/// An unattached repository's root is guarded even when `-h` could be an option value.
#[test]
fn unattached_repository_refuses_its_root_without_creating_private_state() {
    under_each_release(|s| {
        let root = s.dir().join("workspace");
        s.repository(&root);
        let before = Tree::of(&root);
        let output = s
            .git([
                "dupe",
                "remote",
                "add",
                "-t",
                "-h",
                "leak",
                root.to_str().unwrap(),
            ])
            .from(&root)
            .run();
        refused(&output, b"git dupe git", root.as_os_str().as_bytes());
        unchanged(&before, &root);
        assert!(!root.join(".git/dupe").exists());
    });
}

/// Bare repositories guard their path and run allowed help against `<bare>/dupe`.
#[test]
fn bare_repository_refuses_itself_and_keeps_gits_private_help_answer() {
    under_each_release(|s| {
        let bare = s.dir().join("bare.git");
        s.bare_repository(&bare);
        let before = Tree::of(&bare);
        let output = s
            .git([
                "dupe",
                "remote",
                "add",
                "-t",
                "-h",
                "leak",
                bare.to_str().unwrap(),
            ])
            .from(&bare)
            .run();
        refused(&output, b"git dupe git", bare.as_os_str().as_bytes());
        unchanged(&before, &bare);
        let expected = help_in_git_directory(s, &bare);
        let output = s.git(["dupe", "remote", "-h"]).from(&bare).run();
        assert_eq!(output, expected);
        unchanged(&before, &bare);
        assert!(!bare.join("dupe").exists());
    });
}

/// Outside a repository help runs unchanged without collecting any public places.
#[test]
fn outside_help_runs_without_config_or_worktree_queries() {
    under_each_release(|s| {
        let expected = s.git(["-c", "help.autocorrect=0", "remote", "-h"]).run();
        let (output, runs) = run_traced(
            s.git(["dupe", "remote", "-h"]),
            &s.dir().join("outside-trace"),
        );
        assert_eq!(output, expected);
        let own = runs.own();
        assert_eq!(own.of("config"), 0, "{own:?}");
        assert_eq!(own.of("worktree"), 0, "{own:?}");
        let remote: Vec<_> = own
            .words()
            .iter()
            .filter(|words| words.iter().any(|word| word == b"remote"))
            .collect();
        assert_eq!(remote.len(), 1, "{own:?}");
        assert_eq!(
            remote[0].as_slice(),
            ["-c", "help.autocorrect=0", "remote", "-h"].map(|word| word.as_bytes().to_vec())
        );
    });
}
