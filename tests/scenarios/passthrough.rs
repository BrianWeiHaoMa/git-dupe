//! Commands passed through to the private repository, with Git's answers and settle.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Runs, Scenario, Tree, daily_state, holds, names, private_add, private_commit,
    region_rules, run_traced, unchanged, under_each_release, warnings_in_any_order, write,
    write_executable,
};

fn fixture(s: &Scenario, name: &str) -> PathBuf {
    for (key, value) in [
        ("user.name", "Scenario"),
        ("user.email", "scenario@example.invalid"),
        ("maintenance.auto", "false"),
    ] {
        s.git(["config", "--global", key, value]).succeeds();
    }
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    s.git(["dupe", "log", "-1"]).from(&dir).succeeds();
    dir
}

fn private_head(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.private(dir).git(["rev-parse", "HEAD"]).succeeds().stdout
}

fn stage_change(s: &Scenario, dir: &Path) {
    write(dir, "notes/a.md", b"changed\n");
    private_add(s, dir, "notes/a.md");
}

#[test]
fn commit_changes_only_private_history_and_leaves_status_and_add_working() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        stage_change(s, &dir);
        let private = private_head(s, &dir);
        let public = s.git(["rev-parse", "HEAD"]).from(&dir).succeeds();
        let index = fs::read(dir.join(".git/index")).unwrap();
        let objects = Tree::of(&dir.join(".git/objects"));
        s.git(["dupe", "commit", "-m", "x"]).from(&dir).succeeds();
        assert_ne!(private_head(s, &dir), private);
        assert_eq!(&s.git(["rev-parse", "HEAD"]).from(&dir).succeeds(), &public);
        assert_eq!(fs::read(dir.join(".git/index")).unwrap(), index);
        unchanged(&objects, &dir.join(".git/objects"));
        s.git(["dupe", "status"]).from(&dir).succeeds();
        s.git(["dupe", "add", "."]).from(&dir).succeeds();
    });
}

#[test]
fn autocorrect_never_turns_a_misspelling_into_stash() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "override" });
            s.git([
                "config",
                "--global",
                "help.autocorrect",
                if global { "immediate" } else { "0" },
            ])
            .succeeds();
            write(&dir, "untracked", b"keep\n");
            let expected = s
                .private(&dir)
                .git(["-c", "help.autocorrect=0", "stahs", "-u"])
                .run();
            let words = if global {
                vec!["dupe", "stahs", "-u"]
            } else {
                vec!["-c", "help.autocorrect=immediate", "dupe", "stahs", "-u"]
            };
            let output = s.git(words).from(&dir).run();
            assert_eq!(&output, &expected);
            assert_eq!(output.end, End::Code(1));
            assert_eq!(fs::read(dir.join("untracked")).unwrap(), b"keep\n");
            assert!(
                s.private(&dir)
                    .git(["stash", "list"])
                    .succeeds()
                    .stdout
                    .is_empty()
            );
            // Prove this spelling would be destructive if the override were absent
            // or placed after the command word.
            s.private(&dir)
                .git(["-c", "help.autocorrect=immediate", "stahs", "-u"])
                .succeeds();
            assert!(!dir.join("untracked").exists());
            assert!(
                !s.private(&dir)
                    .git(["stash", "list"])
                    .succeeds()
                    .stdout
                    .is_empty()
            );
        }
    });
}

#[test]
fn cached_removal_settles_and_warns_about_the_new_public_path() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        let index = fs::read(dir.join(".git/dupe/index")).unwrap();
        let expected = s
            .private(&dir)
            .git([
                "-c",
                "help.autocorrect=0",
                "rm",
                "--cached",
                "docs/notes.md",
            ])
            .run();
        fs::write(dir.join(".git/dupe/index"), index).unwrap();
        let output = s
            .git(["dupe", "rm", "--cached", "docs/notes.md"])
            .from(&dir)
            .run();
        assert_eq!(output.end, expected.end);
        assert_eq!(output.end, End::Code(0));
        assert_eq!(output.stdout, expected.stdout);
        assert!(output.stderr.starts_with(&expected.stderr));
        warnings_in_any_order(&output, &[&[b"docs/notes.md", b"visible to public Git"]]);
        names(output.lines("warning")[0], b"docs/notes.md");
        let public = s.git(["status", "--porcelain"]).from(&dir).succeeds();
        assert!(holds(&public.stdout, b"?? docs/notes.md\n"), "{public:?}");
        assert!(
            !region_rules(&dir)
                .iter()
                .any(|rule| rule == b"/docs/notes.md")
        );
    });
}

#[test]
fn failed_commits_keep_gits_answer_and_still_settle() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "commit"])
            .run();
        let output = s.git(["dupe", "commit"]).from(&dir).run();
        assert_eq!(&output, &expected);
        assert_ne!(output.end, End::Code(0));
        stage_change(s, &dir);
        write(&dir, ".gitignore", b"!notes\n");
        write(&dir, ".git/dupe/index.lock", b"");
        let head = private_head(s, &dir);
        let region = fs::read(dir.join(".git/info/exclude")).unwrap();
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "commit", "-m", "x"])
            .run();
        let output = s.git(["dupe", "commit", "-m", "x"]).from(&dir).run();
        assert_eq!(output.end, expected.end);
        assert_ne!(output.end, End::Code(0));
        assert_eq!(output.stdout, expected.stdout);
        let after_git = output
            .stderr
            .strip_prefix(expected.stderr.as_slice())
            .expect("Git's failure precedes settle");
        assert!(after_git.starts_with(b"warning: "), "{output:?}");
        warnings_in_any_order(&output, &[&[b"notes", b".gitignore:1"], &[b"notes/a.md"]]);
        assert_eq!(private_head(s, &dir), head);
        assert_eq!(fs::read(dir.join(".git/info/exclude")).unwrap(), region);
    });
}

/// A command passed to Git asks Git's own questions on the streams git-dupe was given,
/// and the answers decide as they do for Git.
#[test]
fn a_passed_command_asks_gits_own_questions_and_takes_the_answers() {
    under_each_release(|s| {
        let ours = fixture(s, "ours");
        let theirs = fixture(s, "theirs");
        for dir in [&ours, &theirs] {
            write(dir, "notes/a.md", b"private\nchanged\n");
        }
        let expected = s
            .private(&theirs)
            .git(["-c", "help.autocorrect=0", "restore", "-p", "notes/a.md"])
            .input(b"y\n")
            .run();
        let output = s
            .git(["dupe", "restore", "-p", "notes/a.md"])
            .from(&ours)
            .input(b"y\n")
            .run();
        assert_eq!(&output, &expected);
        assert!(!output.stdout.is_empty(), "{output:?}");
        for dir in [&ours, &theirs] {
            assert_eq!(fs::read(dir.join("notes/a.md")).unwrap(), b"private\n");
        }
    });
}

#[test]
fn a_private_hook_signal_becomes_the_shell_exit_status() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        stage_change(s, &dir);
        write_executable(
            s,
            &dir.join(".git/dupe/hooks/pre-commit"),
            b"#!/bin/sh\nkill -TERM $PPID\n",
        );
        let head = private_head(s, &dir);
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "commit", "-m", "x"])
            .run();
        assert_eq!(expected.end, End::Signal(15));
        let output = s.git(["dupe", "commit", "-m", "x"]).from(&dir).run();
        assert_eq!(output.end, End::Code(128 + 15));
        assert_eq!(output.stdout, expected.stdout);
        assert_eq!(output.stderr, expected.stderr);
        assert_eq!(private_head(s, &dir), head);
    });
}

#[test]
fn a_public_commit_hook_commits_the_private_index() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        stage_change(s, &dir);
        write_executable(
            s,
            &dir.join(".git/hooks/pre-commit"),
            b"#!/bin/sh\ngit dupe commit -m inner\n",
        );
        write(&dir, "README.md", b"public change\n");
        s.git(["add", "README.md"]).from(&dir).succeeds();
        let index = s.git(["ls-files", "--stage", "-z"]).from(&dir).succeeds();
        let head = s.git(["rev-parse", "HEAD"]).from(&dir).succeeds();
        s.git(["commit", "-m", "outer"]).from(&dir).succeeds();
        assert_eq!(
            s.private(&dir)
                .git(["log", "-1", "--format=%s"])
                .succeeds()
                .stdout,
            b"inner\n"
        );
        assert_eq!(
            s.git(["log", "-1", "--format=%s"])
                .from(&dir)
                .succeeds()
                .stdout,
            b"outer\n"
        );
        assert_ne!(
            s.git(["rev-parse", "HEAD"]).from(&dir).succeeds().stdout,
            head.stdout
        );
        assert_eq!(
            &s.git(["ls-files", "--stage", "-z"]).from(&dir).succeeds(),
            &index,
        );
        assert!(
            s.git(["diff", "--cached", "--name-only"])
                .from(&dir)
                .succeeds()
                .stdout
                .is_empty()
        );
    });
}

#[test]
fn commit_uses_the_callers_editor() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        stage_change(s, &dir);
        s.git(["dupe", "commit"])
            .from(&dir)
            .variable("GIT_EDITOR", "printf 'from editor\\n' >")
            .succeeds();
        assert_eq!(
            s.private(&dir)
                .git(["log", "-1", "--format=%s"])
                .succeeds()
                .stdout,
            b"from editor\n"
        );
    });
}

#[test]
fn branches_merge_and_history_commands_use_the_private_repository() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        let public = s
            .git(["rev-parse", "HEAD", "--abbrev-ref", "HEAD"])
            .from(&dir)
            .succeeds();
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "switch", "-c", "experiment"])
            .succeeds();
        s.private(&dir).git(["switch", "main"]).succeeds();
        s.private(&dir)
            .git(["branch", "-D", "experiment"])
            .succeeds();
        assert_eq!(
            &s.git(["dupe", "switch", "-c", "experiment"])
                .from(&dir)
                .run(),
            &expected,
        );
        stage_change(s, &dir);
        s.git(["dupe", "commit", "-m", "experiment"])
            .from(&dir)
            .succeeds();
        let experiment = private_head(s, &dir);
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "switch", "main"])
            .succeeds();
        s.private(&dir).git(["switch", "experiment"]).succeeds();
        assert_eq!(
            &s.git(["dupe", "switch", "main"]).from(&dir).run(),
            &expected,
        );
        let main = private_head(s, &dir);
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "merge", "experiment"])
            .run();
        s.private(&dir)
            .git([
                OsStr::new("reset"),
                OsStr::new("--hard"),
                OsStr::from_bytes(main.trim_ascii_end()),
            ])
            .succeeds();
        let output = s.git(["dupe", "merge", "experiment"]).from(&dir).run();
        assert_eq!(&output, &expected);
        assert_eq!(private_head(s, &dir), experiment);
        assert_eq!(
            s.private(&dir)
                .git(["rev-parse", "refs/heads/main", "refs/heads/experiment"])
                .succeeds()
                .stdout,
            [experiment.as_slice(), experiment.as_slice()].concat()
        );
        assert_eq!(
            &s.git(["rev-parse", "HEAD", "--abbrev-ref", "HEAD"])
                .from(&dir)
                .succeeds(),
            &public,
        );
        write(&dir, "notes/a.md", b"discard\n");
        for words in [vec!["log"], vec!["diff"], vec!["ls-files", "-z"]] {
            let expected = s
                .private(&dir)
                .git(
                    ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied()),
                )
                .run();
            let output = s.git(["dupe"].into_iter().chain(words)).from(&dir).run();
            assert_eq!(&output, &expected);
        }
        write(&dir, "notes/a.md", b"discard\n");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "restore", "notes/a.md"])
            .run();
        let restored = fs::read(dir.join("notes/a.md")).unwrap();
        write(&dir, "notes/a.md", b"discard\n");
        let output = s.git(["dupe", "restore", "notes/a.md"]).from(&dir).run();
        assert_eq!(&output, &expected);
        assert_eq!(fs::read(dir.join("notes/a.md")).unwrap(), restored);
        assert_eq!(restored, b"changed\n");
    });
}

#[test]
fn unknown_command_bytes_keep_gits_answer_and_only_the_expected_runs() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        for (index, word) in [b"nosuchcommand".as_slice(), b"\xff\xfe", b"no\nsuchcommand"]
            .into_iter()
            .enumerate()
        {
            let word = OsStr::from_bytes(word);
            let expected = s
                .private(&dir)
                .git([OsStr::new("-c"), OsStr::new("help.autocorrect=0"), word])
                .run();
            let (output, runs) = run_traced(
                s.git([OsStr::new("dupe"), word]).from(&dir),
                &s.dir().join(format!("trace-{index}")),
            );
            assert_eq!(&output, &expected);
            assert_eq!(output.end, End::Code(1));
            let commands = runs.commands();
            assert_eq!(
                &commands[..4],
                &[b"dupe".as_slice(), b"rev-parse", b"config", word.as_bytes()]
            );
            assert_eq!(
                &commands[4..],
                &[b"ls-files".as_slice(), b"ls-files", b"check-ignore"]
            );
        }
    });
}

#[test]
fn commit_and_log_run_counts_do_not_grow_with_private_files() {
    under_each_release(|s| {
        let mut previous: Option<(Runs, Runs)> = None;
        for count in [5, 500] {
            let dir = s.dir().join(format!("workspace-{count}"));
            s.attached_project(&dir);
            write(&dir, ".gitdupe", b"notes\n");
            for (key, value) in [
                ("user.name", "Scenario"),
                ("user.email", "scenario@example.invalid"),
                ("maintenance.auto", "false"),
            ] {
                s.git(["config", "--global", key, value]).succeeds();
            }
            for index in 0..count - 1 {
                write(&dir, &format!("notes/{index}.md"), b"private\n");
            }
            private_add(s, &dir, "notes");
            private_add(s, &dir, ".gitdupe");
            private_commit(s, &dir);
            let tracked = s.private(&dir).git(["ls-files", "-z"]).succeeds();
            assert_eq!(
                tracked.stdout.iter().filter(|&&byte| byte == 0).count(),
                count
            );
            write(&dir, "notes/0.md", b"changed\n");
            private_add(s, &dir, "notes/0.md");
            let (commit, commit_runs) = run_traced(
                s.git(["dupe", "commit", "-m", "x"]).from(&dir),
                &s.dir().join(format!("commit-{count}")),
            );
            assert_eq!(commit.end, End::Code(0), "{commit:?}");
            let (log, log_runs) = run_traced(
                s.git(["dupe", "log", "-1"]).from(&dir),
                &s.dir().join(format!("log-{count}")),
            );
            assert_eq!(log.end, End::Code(0), "{log:?}");
            assert_eq!(commit_runs.of("commit"), 1);
            assert_eq!(log_runs.of("log"), 1);
            if let Some((before_commit, before_log)) = &previous {
                assert_eq!(commit_runs.count(), before_commit.count());
                assert_eq!(log_runs.count(), before_log.count());
                assert_eq!(commit_runs.commands(), before_commit.commands());
                assert_eq!(log_runs.commands(), before_log.commands());
            }
            previous = Some((commit_runs, log_runs));
        }
    });
}
