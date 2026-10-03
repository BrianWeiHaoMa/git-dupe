//! An alias chain as Git follows it: the prefix its expansions set, the lookups
//! that carry it, and every chain git-dupe passes through for Git to run or report.

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, daily_state, names, private_add, private_commit, region_rules,
    run_traced, unchanged, under_each_release, warnings_in_any_order, write,
};

fn fixture(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    s.git(["dupe", "status"]).from(&dir).succeeds();
    write(&dir, "notes/a.md", b"changed\n");
    write(&dir, "notes/new.md", b"untracked\n");
    dir
}

fn alias(s: &Scenario, dir: &Path, global: bool, name: &str, value: &OsStr) {
    let key = format!("alias.{name}");
    if global {
        s.git([
            OsStr::new("config"),
            OsStr::new("--global"),
            OsStr::new(&key),
            value,
        ])
        .succeeds();
    } else {
        s.private(dir)
            .git([OsStr::new("config"), OsStr::new(&key), value])
            .succeeds();
    }
}

fn clear_global(s: &Scenario, global: bool, names: &[&str]) {
    if global {
        for name in names {
            s.git(["config", "--global", "--unset", &format!("alias.{name}")])
                .succeeds();
        }
    }
}

fn as_git(ours: &Output, gits: &Output) {
    assert_eq!(ours.end, gits.end, "{ours:?}, Git: {gits:?}");
    assert_eq!(ours.stdout, gits.stdout, "{ours:?}, Git: {gits:?}");
    assert!(
        ours.stderr.starts_with(&gits.stderr),
        "{ours:?}, Git: {gits:?}"
    );
}

fn stash_refusal(s: &Scenario, dir: &Path, output: &Output, before: &Tree) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    names(output.only_line("fatal"), b"git dupe add");
    names(output.only_line("fatal"), b"git dupe stash");
    unchanged(before, dir);
    assert!(
        s.private(dir)
            .git(["stash", "list"])
            .succeeds()
            .stdout
            .is_empty()
    );
}

#[test]
fn alias_status_prefix_reaches_every_run_after_resolution() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(
                s,
                &dir,
                global,
                "st",
                OsStr::new("-c status.short=true status"),
            );
            let expected = s
                .git(["-c", "status.short=true", "dupe", "status"])
                .from(&dir)
                .run();
            let (ours, runs) = run_traced(s.git(["dupe", "st"]).from(&dir), &s.dir().join("trace"));
            assert_eq!(ours, expected);
            let commands = runs.commands();
            assert_eq!(
                &commands[..4],
                &[b"dupe".as_slice(), b"rev-parse", b"config", b""]
            );
            assert_eq!(runs.of("status"), 1);
            assert_eq!(
                &commands[commands.len() - 3..],
                &[b"ls-files".as_slice(), b"ls-files", b"check-ignore"]
            );
            let prefix = [b"-c".to_vec(), b"status.short=true".to_vec()];
            for words in &runs.words()[..4] {
                assert!(!words.starts_with(&prefix), "{runs:?}");
            }
            for words in &runs.words()[4..] {
                assert!(words.starts_with(&prefix), "{runs:?}");
            }
            let status = runs
                .words()
                .iter()
                .zip(commands)
                .find(|(_, command)| *command == b"status")
                .unwrap()
                .0;
            assert_eq!(
                &status[..5],
                &[
                    b"-c".to_vec(),
                    b"status.short=true".to_vec(),
                    b"-c".to_vec(),
                    b"help.autocorrect=0".to_vec(),
                    b"status".to_vec()
                ]
            );
            clear_global(s, global, &["st"]);
        }
    });
}

#[test]
fn alias_configuration_prefix_defines_the_next_guarded_link() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(s, &dir, global, "a", OsStr::new("-c alias.b='stash -u' b"));
            let before = Tree::of(&dir);
            let (ours, runs) = run_traced(s.git(["dupe", "a"]).from(&dir), &s.dir().join("trace"));
            stash_refusal(s, &dir, &ours, &before);
            assert_eq!(runs.of("config"), 2);
            let lookups: Vec<_> = runs
                .words()
                .iter()
                .zip(runs.commands())
                .filter(|(_, command)| *command == b"config")
                .map(|(words, _)| words)
                .collect();
            assert_eq!(lookups[0][0], b"config");
            assert_eq!(
                &lookups[1][..3],
                &[
                    b"-c".to_vec(),
                    b"alias.b=stash -u".to_vec(),
                    b"config".to_vec()
                ]
            );
            assert_eq!(runs.of("stash"), 0);
            clear_global(s, global, &["a"]);
        }
    });
}

#[test]
fn alias_lookups_remove_git_config_even_after_a_prefix() {
    under_each_release(|s| {
        write(s.dir(), "empty-config", b"");
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(s, &dir, global, "a", OsStr::new("-c x.y=1 b"));
            alias(s, &dir, global, "b", OsStr::new("stash -u"));
            let before = Tree::of(&dir);
            let ours = s
                .git(["dupe", "a"])
                .from(&dir)
                .variable("GIT_CONFIG", s.dir().join("empty-config"))
                .run();
            stash_refusal(s, &dir, &ours, &before);
            clear_global(s, global, &["a", "b"]);
        }
    });
}

#[test]
fn alias_loops_leave_gits_answer_and_only_lookup_and_settle_runs() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            for (name, expansion) in [("l1", "l2"), ("l2", "l1"), ("self", "self")] {
                alias(s, &dir, global, name, OsStr::new(expansion));
            }
            for word in ["l1", "self"] {
                let before = Tree::of(&dir);
                let gits = s
                    .private(&dir)
                    .git(["-c", "help.autocorrect=0", word])
                    .run();
                let (ours, runs) =
                    run_traced(s.git(["dupe", word]).from(&dir), &s.dir().join("trace"));
                as_git(&ours, &gits);
                unchanged(&before, &dir);
                assert_eq!(runs.of("config"), if word == "l1" { 2 } else { 1 });
                assert_eq!(runs.of(""), 1);
                let commands = runs.commands();
                assert_eq!(&commands[..2], &[b"dupe".as_slice(), b"rev-parse"]);
                let passthrough = commands
                    .iter()
                    .position(|command| *command == word.as_bytes())
                    .unwrap();
                assert!(
                    commands[2..passthrough]
                        .iter()
                        .all(|command| *command == b"config" || command.is_empty()),
                    "{runs:?}"
                );
                assert_eq!(
                    &commands[passthrough + 1..],
                    &[b"ls-files".as_slice(), b"ls-files", b"check-ignore"]
                );
            }
            clear_global(s, global, &["l1", "l2", "self"]);
        }
    });
}

#[test]
fn refused_query_and_empty_expansions_pass_the_typed_word_to_git() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            for (word, expansion) in [
                ("bad", "status 'x"),
                ("env", "--git-dir=. status"),
                ("q", "--exec-path status"),
                ("v", "--version"),
                ("empty", ""),
                ("blanks", " \t \r\n"),
            ] {
                alias(s, &dir, global, word, OsStr::new(expansion));
                let before = Tree::of(&dir);
                let gits = s
                    .private(&dir)
                    .git(["-c", "help.autocorrect=0", word])
                    .run();
                let (ours, runs) =
                    run_traced(s.git(["dupe", word]).from(&dir), &s.dir().join("trace"));
                as_git(&ours, &gits);
                assert_eq!(runs.of(word), 1, "{runs:?}");
                assert_eq!(runs.of("status"), 0, "{runs:?}");
                unchanged(&before, &dir);
                clear_global(s, global, &[word]);
            }
        }
    });
}

#[test]
fn shell_aliases_run_as_private_git_runs_them() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(
                s,
                &dir,
                global,
                "shell",
                OsStr::new("!printf '%s\\n' shell; git rev-parse --git-dir"),
            );
            let before = Tree::of(&dir);
            let gits = s
                .private(&dir)
                .git(["-c", "help.autocorrect=0", "shell"])
                .run();
            let ours = s.git(["dupe", "shell"]).from(&dir).run();
            as_git(&ours, &gits);
            unchanged(&before, &dir);
            clear_global(s, global, &["shell"]);
        }
    });
}

#[test]
fn an_alias_exec_path_does_not_preempt_the_next_alias() {
    under_each_release(|s| {
        let programs = s.dir().join("programs");
        write(&programs, "git-zed", b"#!/bin/sh\nprintf ran > marker\n");
        fs::set_permissions(programs.join("git-zed"), fs::Permissions::from_mode(0o755)).unwrap();
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(
                s,
                &dir,
                global,
                "a1",
                OsStr::new(&format!("--exec-path={} zed", programs.display())),
            );
            alias(s, &dir, global, "zed", OsStr::new("stash -u"));
            let before = Tree::of(&dir);
            let ours = s.git(["dupe", "a1"]).from(&dir).run();
            stash_refusal(s, &dir, &ours, &before);
            assert!(!dir.join("marker").exists());
            alias(
                s,
                &dir,
                global,
                "zed",
                OsStr::new("rev-parse --sq-quote alias"),
            );
            let gits = s
                .private(&dir)
                .git(["-c", "help.autocorrect=0", "a1"])
                .succeeds();
            let answer = s
                .private(&dir)
                .git(["rev-parse", "--sq-quote", "alias"])
                .succeeds();
            assert_eq!(gits.stdout, answer.stdout);
            assert!(!dir.join("marker").exists());
            let before = Tree::of(&dir);
            let ours = s.git(["dupe", "a1"]).from(&dir).run();
            as_git(&ours, &gits);
            assert!(!dir.join("marker").exists());
            unchanged(&before, &dir);
            clear_global(s, global, &["a1", "zed"]);
        }
    });
}

#[test]
fn a_public_only_alias_is_unknown_and_stages_nothing() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        s.git(["config", "alias.pub", "add -A"])
            .from(&dir)
            .succeeds();
        write(&dir, "public-new", b"public\n");
        let before = Tree::of(&dir);
        let public_index = s.git(["ls-files", "--stage", "-z"]).from(&dir).succeeds();
        let private_index = s
            .private(&dir)
            .git(["ls-files", "--stage", "-z"])
            .succeeds();
        let gits = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "pub"])
            .run();
        let ours = s.git(["dupe", "pub"]).from(&dir).run();
        as_git(&ours, &gits);
        assert_eq!(ours.end, End::Code(1));
        assert_eq!(
            s.git(["ls-files", "--stage", "-z"]).from(&dir).succeeds(),
            public_index
        );
        assert_eq!(
            s.private(&dir)
                .git(["ls-files", "--stage", "-z"])
                .succeeds(),
            private_index
        );
        unchanged(&before, &dir);
    });
}

#[test]
fn alias_log_help_runs_privately_unattached_and_outside() {
    under_each_release(|s| {
        s.git(["config", "--global", "alias.lg", "log --oneline"])
            .succeeds();
        let dir = s.dir().join("unattached");
        s.repository(&dir);
        for place in [&dir, s.dir()] {
            let before = Tree::of(place);
            let gits = if place == dir {
                s.private(&dir)
                    .git(["-c", "help.autocorrect=0", "lg", "-h"])
                    .run()
            } else {
                s.git(["-c", "help.autocorrect=0", "lg", "-h"]).run()
            };
            let ours = s.git(["dupe", "lg", "-h"]).from(place).run();
            assert_eq!(ours, gits);
            unchanged(&before, place);
            assert!(!dir.join(".git/dupe").exists());
        }
    });
}

#[test]
fn unrelated_aliases_do_not_guard_private_log_and_still_settle() {
    under_each_release(|s| {
        for source in ["global", "private", "command-line"] {
            let dir = s.dir().join(source);
            s.attached_project(&dir);
            write(&dir, ".gitdupe", b"");
            private_add(s, &dir, ".gitdupe");
            private_commit(s, &dir);
            write(&dir, ".gitdupe", b"notes\n");
            write(&dir, "notes/keep", b"private\n");
            write(&dir, ".gitignore", b"!notes\n");
            match source {
                "global" => {
                    s.git(["config", "--global", "alias.x", "y"]).succeeds();
                }
                "private" => {
                    s.private(&dir).git(["config", "alias.x", "y"]).succeeds();
                }
                _ => {}
            }
            let prefix: &[&str] = if source == "command-line" {
                &["-c", "alias.x=y"]
            } else {
                &[]
            };
            let gits = s
                .private(&dir)
                .git(
                    prefix
                        .iter()
                        .copied()
                        .chain(["-c", "help.autocorrect=0", "log", "-1"]),
                )
                .run();
            let (ours, runs) = run_traced(
                s.git(prefix.iter().copied().chain(["dupe", "log", "-1"]))
                    .from(&dir),
                &s.dir().join("trace"),
            );
            as_git(&ours, &gits);
            let tail = Output {
                stdout: vec![],
                stderr: ours.stderr[gits.stderr.len()..].to_vec(),
                end: ours.end,
            };
            names(tail.only_line("warning"), b"notes");
            warnings_in_any_order(&ours, &[&[b"notes", b".gitignore:1"]]);
            assert_eq!(
                runs.commands(),
                [
                    b"dupe".as_slice(),
                    b"rev-parse",
                    b"config",
                    b"log",
                    b"ls-files",
                    b"ls-files",
                    b"check-ignore"
                ]
            );
            assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
            clear_global(s, source == "global", &["x"]);
        }
    });
}

#[test]
fn alias_stash_guard_accepts_non_utf8_operands() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(s, &dir, global, "w", OsStr::from_bytes(b"stash -u -- \xff"));
            let before = Tree::of(&dir);
            let ours = s.git(["dupe", "w"]).from(&dir).run();
            stash_refusal(s, &dir, &ours, &before);
            clear_global(s, global, &["w"]);
        }
    });
}

#[test]
fn alias_misspellings_do_not_autocorrect_even_when_requested_before_dupe() {
    under_each_release(|s| {
        for global in [true, false] {
            let dir = fixture(s, if global { "global" } else { "private" });
            alias(s, &dir, global, "x", OsStr::new("stahs -u"));
            let before = Tree::of(&dir);
            let gits = s
                .private(&dir)
                .git([
                    "-c",
                    "help.autocorrect=immediate",
                    "-c",
                    "help.autocorrect=0",
                    "x",
                ])
                .run();
            let ours = s
                .git(["-c", "help.autocorrect=immediate", "dupe", "x"])
                .from(&dir)
                .run();
            as_git(&ours, &gits);
            assert_eq!(ours.end, End::Code(1), "{ours:?}");
            unchanged(&before, &dir);
            assert!(
                s.private(&dir)
                    .git(["stash", "list"])
                    .succeeds()
                    .stdout
                    .is_empty()
            );
            clear_global(s, global, &["x"]);
        }
    });
}

#[test]
fn public_aliases_do_not_guard_a_private_passthrough() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"");
        private_add(s, &dir, ".gitdupe");
        private_commit(s, &dir);
        s.git(["config", "alias.x", "y"]).from(&dir).succeeds();
        let gits = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "log", "-1"])
            .run();
        let ours = s.git(["dupe", "log", "-1"]).from(&dir).run();
        assert_eq!(ours, gits);
    });
}

#[test]
fn malformed_private_configuration_ends_as_gits_alias_question_then_settles() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        let region = region_rules(&dir);
        writeln!(
            OpenOptions::new()
                .append(true)
                .open(dir.join(".git/dupe/config"))
                .unwrap(),
            "[broken"
        )
        .unwrap();
        let gits = s
            .private(&dir)
            .git(["config", "-z", "--get-regexp", r"^alias\."])
            .run();
        let listing = s.private(&dir).git(["ls-files", "-z", "--full-name"]).run();
        let (ours, runs) = run_traced(s.git(["dupe", "log"]).from(&dir), &s.dir().join("trace"));
        assert_eq!(ours.end, gits.end, "{ours:?}, Git: {gits:?}");
        assert_eq!(ours.stdout, gits.stdout);
        let tail = Output {
            stdout: vec![],
            stderr: ours
                .stderr
                .strip_prefix(&[gits.stderr, listing.stderr].concat()[..])
                .expect("alias question and settle listing errors first")
                .to_vec(),
            end: ours.end,
        };
        names(tail.only_line("warning"), b"git dupe init");
        assert_eq!(
            runs.commands(),
            [
                b"dupe".as_slice(),
                b"rev-parse",
                b"config",
                b"cat-file",
                b"ls-files"
            ]
        );
        assert_eq!(runs.of("log"), 0);
        assert_eq!(region_rules(&dir), region);
    });
}

#[test]
fn an_alias_given_before_dupe_reaches_the_stash_guard() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        let before = Tree::of(&dir);
        let ours = s
            .git(["-c", "alias.x=stash -u", "dupe", "x"])
            .from(&dir)
            .run();
        stash_refusal(s, &dir, &ours, &before);
    });
}

#[test]
fn a_leading_option_git_cannot_evaluate_ends_as_git_ends_the_alias() {
    under_each_release(|s| {
        let dir = fixture(s, "workspace");
        for expansion in [
            "--config-env=broken status -h",
            "--config-env=x.y=NO_SUCH_VARIABLE status --porc",
            "-c broken status -h",
        ] {
            alias(s, &dir, true, "bad", OsStr::new(expansion));
            let before = Tree::of(&dir);
            let gits = s
                .private(&dir)
                .git(["-c", "help.autocorrect=0", "bad"])
                .run();
            let (ours, runs) =
                run_traced(s.git(["dupe", "bad"]).from(&dir), &s.dir().join("trace"));
            assert_eq!(ours.end, gits.end, "{expansion}: {ours:?}, Git: {gits:?}");
            assert_ne!(ours.end, End::Code(0), "{expansion}: {ours:?}");
            assert!(ours.stdout.is_empty(), "{expansion}: {ours:?}");
            assert!(
                ours.stderr.starts_with(&gits.stderr),
                "{expansion}: {ours:?}, Git: {gits:?}"
            );
            assert_eq!(runs.of("status"), 0, "{runs:?}");
            assert!(
                runs.commands()
                    .ends_with(&[b"ls-files".as_slice(), b"ls-files", b"check-ignore"]),
                "{runs:?}"
            );
            unchanged(&before, &dir);
            clear_global(s, true, &["bad"]);
        }
    });
}
