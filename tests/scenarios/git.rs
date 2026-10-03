//! The unguarded private Git route and its dispatch boundaries (G20).

use crate::harness::{
    End, Tree, daily_edited, names, region_rules, run_traced, unchanged, under_each_release,
    usage_line, warnings_in_any_order, write,
};

#[test]
fn git_runs_private_status_and_interactive_add_as_git() {
    under_each_release(|s| {
        let dir = daily_edited(s, "workspace");
        s.git(["dupe", "status"]).from(&dir).succeeds();
        for words in [&["status", "--porc"][..], &["add", "-i"]] {
            let expected = s
                .private(&dir)
                .git(
                    ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied()),
                )
                .run();
            let output = s
                .git(["dupe", "git"].into_iter().chain(words.iter().copied()))
                .from(&dir)
                .run();
            assert_eq!(&output, &expected);
        }
        let guarded = s.git(["dupe", "status", "--porc"]).from(&dir).run();
        assert_eq!(guarded.end, End::Code(129), "{guarded:?}");
    });
}

#[test]
fn git_stash_can_take_untracked_files_and_still_settles() {
    under_each_release(|s| {
        let dir = daily_edited(s, "workspace");
        s.private(&dir)
            .git(["config", "user.name", "Scenario"])
            .succeeds();
        s.private(&dir)
            .git(["config", "user.email", "scenario@example.invalid"])
            .succeeds();
        assert!(!region_rules(&dir).contains(&b"/notes".to_vec()));
        let output = s.git(["dupe", "git", "stash", "-u"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(!dir.join("notes/today.md").exists());
        s.private(&dir)
            .git(["rev-parse", "--verify", "refs/stash"])
            .succeeds();
        assert!(region_rules(&dir).contains(&b"/notes".to_vec()));
    });
}

/// A command on the unguarded route that Git fails keeps Git's answer and still settles:
/// the region is brought up to date, and a hidden path a `!` rule re-includes is named
/// with that rule.
#[test]
fn a_failing_git_route_keeps_gits_answer_and_still_settles() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let dir = daily_edited(s, "workspace");
        write(&dir, ".gitignore", b"!notes\n");
        assert!(!region_rules(&dir).contains(&b"/notes".to_vec()));
        let words = ["commit", "-m", "nothing staged"];
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0"].into_iter().chain(words))
            .run();
        assert_ne!(expected.end, End::Code(0), "{expected:?}");
        let output = s
            .git(["dupe", "git"].into_iter().chain(words))
            .from(&dir)
            .run();
        assert_eq!(output.end, expected.end, "{output:?}; Git: {expected:?}");
        assert_eq!(
            output.stdout, expected.stdout,
            "{output:?}; Git: {expected:?}"
        );
        let after_git = output
            .stderr
            .strip_prefix(expected.stderr.as_slice())
            .expect("Git's answer precedes settle's warnings");
        assert!(after_git.starts_with(b"warning: "), "{output:?}");
        warnings_in_any_order(&output, &[&[b"notes", b".gitignore:1"], &[b"notes/a.md"]]);
        assert!(region_rules(&dir).contains(&b"/notes".to_vec()));
    });
}

#[test]
fn git_empty_words_locate_before_usage_and_do_not_settle() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "git"]).succeeds();
        let dir = daily_edited(s, "workspace");
        write(&dir, ".gitignore", b"!notes\n");
        let before = Tree::of(&dir);
        let output = s.git(["dupe", "git"]).from(&dir).run();
        assert_eq!(output.end, End::Code(129), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        output.line_then("error", usage_line(&help.stdout));
        assert!(output.lines("warning").is_empty(), "{output:?}");
        unchanged(&before, &dir);
        let unattached = s.dir().join("unattached");
        s.repository(&unattached);
        let output = s.git(["dupe", "git"]).from(&unattached).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        names(output.only_line("fatal"), b"git dupe init");
        let outside = s.git(["dupe", "git"]).run();
        let locate = s.git(["dupe", "status"]).run();
        assert_eq!(&outside, &locate);
        assert_eq!(outside.end, End::Code(128), "{outside:?}");
    });
}

#[test]
fn git_global_options_are_usage_errors_before_locating() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "git"]).succeeds();
        let attached = daily_edited(s, "workspace");
        for dir in [&attached, s.dir()] {
            for words in [&["-c", "a=b", "status"][..], &["--", "status"]] {
                let (output, runs) = run_traced(
                    s.git(["dupe", "git"].into_iter().chain(words.iter().copied()))
                        .from(dir),
                    &s.dir().join("trace"),
                );
                assert_eq!(output.end, End::Code(129), "{output:?}");
                assert!(output.stdout.is_empty(), "{output:?}");
                names(
                    output.line_then("error", usage_line(&help.stdout)),
                    b"git <options> dupe",
                );
                assert_eq!(runs.count(), 1, "{runs:?}");
                assert_eq!(runs.commands(), [b"dupe".as_slice()], "{runs:?}");
                assert_eq!(runs.of("rev-parse"), 0, "{runs:?}");
            }
        }
    });
}

#[test]
fn git_help_reads_only_the_first_word_without_locating() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "git"]).succeeds();
        let attached = daily_edited(s, "workspace");
        let unattached = s.dir().join("unattached");
        s.repository(&unattached);
        for dir in [&attached, &unattached, s.dir()] {
            for option in ["-h", "--help"] {
                let output = s.git(["dupe", "git", option]).from(dir).run();
                assert_eq!(&output, &help);
            }
        }
        let output = s.git(["dupe", "git", "log", "-h"]).from(&unattached).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        names(output.only_line("fatal"), b"git dupe init");
    });
}
