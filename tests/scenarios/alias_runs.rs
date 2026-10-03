//! The Git runs of a command reached through an alias follow its chain and never
//! the files either repository holds.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Runs, Scenario, private_add, private_commit, run_traced, under_each_release, write,
};

fn fixture(s: &Scenario, name: &str, count: usize) -> PathBuf {
    let dir = s.dir().join(name);
    s.attached_project(&dir);
    write(&dir, ".gitdupe", b"notes\n");
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
    // Bring settle to the same steady state before comparing commands.
    s.git(["dupe", "status"]).from(&dir).succeeds();
    dir
}

fn alias(s: &Scenario, dir: &Path, name: &str, expansion: &str) {
    s.private(dir)
        .git(["config", &format!("alias.{name}"), expansion])
        .succeeds();
}

fn with_questions<'a>(runs: &'a Runs, questions: &[&'a [u8]]) -> Vec<&'a [u8]> {
    let commands = runs.commands();
    assert_eq!(&commands[..2], &[b"dupe".as_slice(), b"rev-parse"]);
    commands[..2]
        .iter()
        .copied()
        .chain(questions.iter().copied())
        .chain(commands[2..].iter().copied())
        .collect()
}

#[test]
fn aliased_status_add_and_log_runs_do_not_grow_with_private_files() {
    under_each_release(|s| {
        let mut previous: Option<(Runs, Runs, Runs)> = None;
        for count in [5, 500] {
            let dir = fixture(s, &format!("workspace-{count}"), count);
            for (name, expansion) in [("sa", "status"), ("aa", "add -A"), ("l", "log -1")] {
                alias(s, &dir, name, expansion);
            }
            write(&dir, "notes/0.md", b"changed\n");
            let (status, status_runs) = run_traced(
                s.git(["dupe", "status"]).from(&dir),
                &s.dir().join("literal-status"),
            );
            let (sa, sa_runs) = run_traced(
                s.git(["dupe", "sa"]).from(&dir),
                &s.dir().join("aliased-status"),
            );
            assert_eq!(status.end, End::Code(0), "{status:?}");
            assert_eq!(sa, status);
            assert_eq!(
                sa_runs.commands(),
                with_questions(&status_runs, &[b"config", b""])
            );

            let index = fs::read(dir.join(".git/dupe/index")).unwrap();
            let (add, add_runs) = run_traced(
                s.git(["dupe", "add", "-A"]).from(&dir),
                &s.dir().join("literal-add"),
            );
            assert_eq!(add.end, End::Code(0), "{add:?}");
            let staged = s
                .private(&dir)
                .git(["ls-files", "--stage", "-z"])
                .succeeds();
            let changed = s
                .private(&dir)
                .git(["diff", "--cached", "--name-only", "-z"])
                .succeeds();
            assert_eq!(changed.stdout, b"notes/0.md\0");
            fs::write(dir.join(".git/dupe/index"), index).unwrap();
            let (aa, aa_runs) = run_traced(
                s.git(["dupe", "aa"]).from(&dir),
                &s.dir().join("aliased-add"),
            );
            assert_eq!(aa, add);
            assert_eq!(
                s.private(&dir)
                    .git(["ls-files", "--stage", "-z"])
                    .succeeds(),
                staged
            );
            assert_eq!(
                aa_runs.commands(),
                with_questions(&add_runs, &[b"config", b""])
            );

            let (log, log_runs) = run_traced(
                s.git(["dupe", "log", "-1"]).from(&dir),
                &s.dir().join("literal-log"),
            );
            let (l, l_runs) = run_traced(
                s.git(["dupe", "l"]).from(&dir),
                &s.dir().join("aliased-log"),
            );
            assert_eq!(log.end, End::Code(0), "{log:?}");
            assert_eq!(l, log);
            let mut expected = with_questions(&log_runs, &[b"config", b""]);
            // Git itself starts another process to expand this passed-through alias.
            let log_at = expected.iter().position(|&word| word == b"log").unwrap();
            expected.insert(log_at, b"l");
            assert_eq!(l_runs.commands(), expected);
            if let Some((before_sa, before_aa, before_l)) = &previous {
                for (now, before) in [
                    (&sa_runs, before_sa),
                    (&aa_runs, before_aa),
                    (&l_runs, before_l),
                ] {
                    assert_eq!(now.count(), before.count());
                    assert_eq!(now.commands(), before.commands());
                }
            }
            previous = Some((sa_runs, aa_runs, l_runs));
        }
    });
}

#[test]
fn a_second_alias_link_adds_only_one_lookup() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace", 5);
        for (name, expansion) in [("a", "b"), ("b", "status"), ("c", "status")] {
            alias(s, &dir, name, expansion);
        }
        let (one, one_runs) =
            run_traced(s.git(["dupe", "c"]).from(&dir), &s.dir().join("one-link"));
        let (two, two_runs) =
            run_traced(s.git(["dupe", "a"]).from(&dir), &s.dir().join("two-links"));
        assert_eq!(one.end, End::Code(0), "{one:?}");
        assert_eq!(two, one);
        let status = s.git(["dupe", "status"]).from(&dir).succeeds();
        assert_eq!(one, status);
        let mut expected = one_runs.commands();
        assert_eq!(
            &expected[..4],
            &[b"dupe".as_slice(), b"rev-parse", b"config", b""]
        );
        expected.insert(4, b"config");
        assert_eq!(two_runs.commands(), expected);
    });
}

#[test]
fn command_listing_is_shared_by_all_links_and_readings() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace", 5);
        for (name, expansion) in [
            ("a", "b"),
            ("b", "c"),
            ("c", "d"),
            ("d", "status"),
            ("log", "show"),
            ("history", "log"),
        ] {
            alias(s, &dir, name, expansion);
        }
        let (status, status_runs) = run_traced(
            s.git(["dupe", "status"]).from(&dir),
            &s.dir().join("literal-status"),
        );
        let (plain, plain_runs) = run_traced(
            s.git(["dupe", "a"]).from(&dir),
            &s.dir().join("plain-chain"),
        );
        assert_eq!(status.end, End::Code(0), "{status:?}");
        assert_eq!(plain, status);
        assert_eq!(
            plain_runs.commands(),
            with_questions(
                &status_runs,
                &[b"config", b"", b"config", b"config", b"config"]
            )
        );
        assert!(plain_runs.of("") <= 1);

        // This section's record has a form the releases read differently.
        alias(s, &dir, "odd.command", "log");
        for (word, questions) in [
            ("log", vec![b"config".as_slice(), b"", b"config"]),
            (
                "history",
                vec![b"config".as_slice(), b"", b"config", b"config"],
            ),
            ("odd", vec![b"config".as_slice(), b"config", b"", b"config"]),
        ] {
            let (expected, git_runs) = run_traced(
                s.private(&dir)
                    .git(["-c", "help.autocorrect=0", word, "-1"]),
                &s.dir().join(format!("git-trace-{word}")),
            );
            let (output, runs) = run_traced(
                s.git(["dupe", word, "-1"]).from(&dir),
                &s.dir().join(format!("trace-{word}")),
            );
            assert_eq!(output, expected);
            let expected_commands: Vec<_> = [b"dupe".as_slice(), b"rev-parse"]
                .into_iter()
                .chain(questions)
                .chain(git_runs.commands())
                .chain([b"ls-files".as_slice(), b"ls-files", b"check-ignore"])
                .collect();
            assert_eq!(runs.commands(), expected_commands);
            assert!(runs.of("") <= 1);
        }
    });
}

#[test]
fn a_word_without_an_alias_record_needs_one_lookup_and_no_listing() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace", 5);
        // An unrelated record must not cause the commands to be listed.
        alias(s, &dir, "sa", "status");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "log", "-1"])
            .succeeds();
        let (output, runs) = run_traced(
            s.git(["dupe", "log", "-1"]).from(&dir),
            &s.dir().join("trace"),
        );
        assert_eq!(output, expected);
        assert_eq!(
            runs.commands(),
            [
                b"dupe".as_slice(),
                b"rev-parse",
                b"config",
                b"log",
                b"ls-files",
                b"ls-files",
                b"check-ignore",
            ]
        );
    });
}
