//! Stash's untracked guard and the forms left to Git (G17).

use std::path::PathBuf;

use crate::harness::{
    End, Scenario, Tree, daily_state, lines_in_order, names, unchanged, under_each_release,
    warnings_in_any_order, write,
};

fn fixture(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    s.private(&dir)
        .git(["config", "user.name", "Scenario"])
        .succeeds();
    s.private(&dir)
        .git(["config", "user.email", "scenario@example.invalid"])
        .succeeds();
    // Paired fixtures must have identical history even across a clock tick.
    s.private(&dir)
        .git([
            "commit",
            "--amend",
            "-q",
            "--no-edit",
            "--date=2000-01-01T00:00:00Z",
        ])
        .variable("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
        .succeeds();
    // Settle before taking observations so the fixture already has its managed region.
    s.git(["dupe", "status"]).from(&dir).succeeds();
    write(&dir, "notes/a.md", b"changed\n");
    write(&dir, "notes/new.md", b"untracked\n");
    write(&dir, "-u", b"notes/a.md\n");
    dir
}

#[test]
fn stash_untracked_forms_refuse_without_changes_and_still_settle() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        for private in [false, true] {
            if private {
                s.private(&dir)
                    .git(["update-ref", "refs/stash", "HEAD"])
                    .succeeds();
            } else {
                s.git(["update-ref", "refs/stash", "HEAD"])
                    .from(&dir)
                    .succeeds();
            }
        }
        write(
            &dir,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\n!notes\n",
        );
        let forms: &[&[&str]] = &[
            &["-u"],
            &["push", "-a"],
            &["save", "--include-untracked"],
            &["-ku"],
            &["--inc"],
            &["--al"],
            &["push", "-m", "msg", "--all"],
            &["push", "-um", "msg"],
            &["-um", "msg"],
            &["save", "msg", "-u"],
            &["push", "-k", "-u"],
            &["push", "-qu", "-m", "msg"],
            &["push", "--mess", "msg", "-u"],
            &["--mess", "msg", "-u"],
            &["push", "--pathspec-from-file", "x", "-u"],
            &["push", "-m", "--end-of-options", "-u"],
        ];
        for form in forms {
            let before = Tree::of(&dir);
            let output = s
                .git(["dupe", "stash"].into_iter().chain(form.iter().copied()))
                .from(&dir)
                .run();
            assert_eq!(output.end, End::Code(128), "{form:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            lines_in_order(&output, "fatal", &[b"git dupe add"]);
            let fatal = output.lines("fatal")[0];
            names(fatal, b"git dupe stash");
            let add = fatal
                .windows(b"git dupe add".len())
                .position(|w| w == b"git dupe add")
                .unwrap();
            let stash = fatal
                .windows(b"git dupe stash".len())
                .position(|w| w == b"git dupe stash")
                .unwrap();
            assert!(add < stash, "{output:?}");
            warnings_in_any_order(&output, &[&[b"notes", b".gitignore"], &[b"notes/a.md"]]);
            unchanged(&before, &dir);
        }
    });
}

#[test]
fn stash_other_forms_keep_gits_answer_and_the_untracked_file() {
    under_each_release(|s| {
        let forms: &[&[&str]] = &[
            &[],
            &["push", "-m", "u"],
            &["push", "-m", "-u"],
            &["push", "-qm", "-u"],
            &["-qm", "-u"],
            &["push", "--message", "--all"],
            &["--message", "--all"],
            &["push", "--mes", "-u"],
            &["--mes", "-u"],
            &["-mu"],
            &["--", "-u"],
            &["push", "--no-include-untracked"],
            &["push", "--no-all"],
            &["push", "-k"],
            &["list"],
            &["show", "-u"],
            &["pop"],
            &["push", "-m", "msg", "--", "-u"],
            &["push", "--pathspec-from-file", "-u"],
            &["--pathspec-from-file", "-u"],
            &["push", "--pathspec-fr", "-u"],
            &["save", "--end-of-options", "-u"],
            &["push", "--end-of-options", "--all"],
            &["--end-of-options", "-a"],
        ];
        for (index, form) in forms.iter().enumerate() {
            let actual = fixture(s, &format!("actual-{index}"));
            let expected = fixture(s, &format!("expected-{index}"));
            if matches!(form.first(), Some(&"list" | &"show" | &"pop")) {
                for dir in [&actual, &expected] {
                    s.private(dir)
                        .git(["stash", "push", "-m", "saved"])
                        .variable("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
                        .variable("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
                        .succeeds();
                }
            }
            let git = s
                .private(&expected)
                .git(
                    ["-c", "help.autocorrect=0", "stash"]
                        .into_iter()
                        .chain(form.iter().copied()),
                )
                .run();
            let output = s
                .git(["dupe", "stash"].into_iter().chain(form.iter().copied()))
                .from(&actual)
                .run();
            assert_eq!(output.end, git.end, "{form:?}: {output:?}; Git: {git:?}");
            assert_eq!(output.stdout, git.stdout, "{form:?}");
            assert_eq!(output.stderr, git.stderr, "{form:?}");
            assert_eq!(
                std::fs::read(actual.join("notes/new.md")).unwrap(),
                b"untracked\n"
            );
            for command in [vec!["status", "--porcelain"], vec!["stash", "list"]] {
                let actual_state = s.private(&actual).git(&command).succeeds();
                let expected_state = s.private(&expected).git(&command).succeeds();
                assert_eq!(
                    actual_state.stdout, expected_state.stdout,
                    "{form:?}: {command:?}"
                );
            }
        }
    });
}

/// A `!` alias runs as Git runs it, unguarded: one whose command stashes untracked files
/// stashes and removes them as the same alias does under the release's own Git.
#[test]
fn a_shell_alias_that_stashes_untracked_files_runs_unguarded() {
    under_each_release(|s| {
        let actual = fixture(s, "actual");
        let expected = fixture(s, "expected");
        for dir in [&actual, &expected] {
            s.private(dir)
                .git(["config", "alias.keep", "!git stash -u"])
                .succeeds();
        }
        let gits = s
            .private(&expected)
            .git(["-c", "help.autocorrect=0", "keep"])
            .run();
        let ours = s.git(["dupe", "keep"]).from(&actual).run();
        assert_eq!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(ours.end, gits.end, "{ours:?}; Git: {gits:?}");
        assert_eq!(ours.stdout, gits.stdout, "{ours:?}; Git: {gits:?}");
        assert!(
            ours.stderr.starts_with(&gits.stderr),
            "{ours:?}; Git: {gits:?}"
        );
        for dir in [&actual, &expected] {
            assert!(!dir.join("notes/new.md").exists());
            s.private(dir)
                .git(["rev-parse", "--verify", "-q", "refs/stash"])
                .succeeds();
        }
    });
}
