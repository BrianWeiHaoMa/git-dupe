//! The words of `git dupe clean`, its refusal, and its wiring: the table read before
//! anything is located, the user's words reaching Git as typed with git-dupe's patterns
//! where they decide, `-X` beside `-e` refused, the help text, the alias, and the
//! workspaces it requires (G16, G26, G24, G19, G4, R3).

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Runs, Scenario, Tree, copy, daily_state, locate_words, names, region_rules,
    run_traced, under_each_release, usage_line, write,
};

/// A command's text, through its own help where no repository is.
fn clean_text(s: &Scenario) -> Vec<u8> {
    let output = s.git(["dupe", "help", "clean"]).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output.stdout
}

/// An attached workspace in daily use, with an untracked file beside the hidden ones.
fn attached(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    write(&dir, "scratch.txt", b"scratch\n");
    write(&dir, "notes/untracked.md", b"untracked\n");
    let settled = s.git(["dupe", "status"]).from(&dir).run();
    assert_eq!(settled.end, End::Code(0), "{settled:?}");
    dir
}

/// The one `clean` run git-dupe started, with its words after the program name.
fn clean_run(runs: &Runs) -> Vec<Vec<u8>> {
    let own = runs.own();
    let cleans: Vec<_> = own
        .words()
        .iter()
        .zip(own.commands())
        .filter(|(_, command)| *command == b"clean")
        .map(|(words, _)| words.clone())
        .collect();
    assert_eq!(cleans.len(), 1, "{runs:?}");
    cleans.into_iter().next().unwrap()
}

fn bytes(words: &[&str]) -> Vec<Vec<u8>> {
    words.iter().map(|word| word.as_bytes().to_vec()).collect()
}

#[test]
fn the_users_words_reach_git_in_order_with_the_patterns_before_the_users_end_of_options() {
    under_each_release(|s| {
        let dir = attached(s, "workspace");
        let log = s.dir().join("trace");
        let patterns = ["-e", "/.env.local", "-e", "/.gitdupe", "-e", "/.vscode"];
        let more = ["-e", "/docs/notes.md", "-e", "/notes"];
        for (words, before, after) in [
            // The first `--` is the value of `-e`; the second is the user's.
            (
                &["-f", "-e", "--", "--", "x"][..],
                &["-f", "-e", "--"][..],
                &["--", "x"][..],
            ),
            (&["-n", "x", "-d"], &["-n", "x", "-d"], &[]),
            (&["-n", "--", "-d", "--"], &["-n"], &["--", "-d", "--"]),
            (
                &["--dry-run", "--exclude", "--"],
                &["--dry-run", "--exclude", "--"],
                &[],
            ),
        ] {
            let (output, runs) = run_traced(
                s.git(["dupe", "clean"].iter().chain(words)).from(&dir),
                &log,
            );
            let expected: Vec<&str> = ["-c", "help.autocorrect=0", "clean"]
                .iter()
                .chain(before)
                .chain(&patterns)
                .chain(&more)
                .chain(after)
                .copied()
                .collect();
            assert_eq!(clean_run(&runs), bytes(&expected), "{words:?}: {output:?}");
        }
    });
}

#[test]
fn an_exclude_lacking_its_value_runs_the_users_words_alone_and_git_refuses_them() {
    under_each_release(|s| {
        let dir = attached(s, "workspace");
        let log = s.dir().join("trace");
        for words in [
            &["-f", "-x", "-e"][..],
            &["-fxe"],
            &["-f", "-x", "--exclude"],
        ] {
            let edited = [&fs::read(dir.join(".gitdupe")).unwrap()[..], b"late\n"].concat();
            write(&dir, ".gitdupe", &edited);
            let before = Tree::of(&dir).without(&[&dir.join(".git/info/exclude")]);
            let expected = s.git(["clean"].iter().chain(words)).from(&dir).run();
            let (output, runs) = run_traced(
                s.git(["dupe", "clean"].iter().chain(words)).from(&dir),
                &log,
            );
            let typed: Vec<&str> = ["-c", "help.autocorrect=0", "clean"]
                .iter()
                .chain(words)
                .copied()
                .collect();
            assert_eq!(clean_run(&runs), bytes(&typed), "{words:?}: {output:?}");
            assert_eq!(output.end, expected.end, "{words:?}: {output:?}");
            assert_ne!(output.end, End::Code(0), "{words:?}: {output:?}");
            let changed =
                before.changed_in(&Tree::of(&dir).without(&[&dir.join(".git/info/exclude")]));
            assert!(changed.is_empty(), "{words:?} changed {changed:?}");
            // Settled: the line added by hand is in the region.
            assert!(region_rules(&dir).contains(&b"/late".to_vec()), "{words:?}");
            write(&dir, ".gitdupe", b"notes\n.vscode\n");
        }
    });
}

/// `output` is the refusal of `-X` beside `-e`: one `fatal:` line, exit 128, nothing on
/// standard output.
fn refused_x_with_e(output: &Output) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("fatal");
    names(line, b"-X");
    names(line, b"-e");
}

#[test]
fn x_beside_e_in_every_spelling_is_refused_deleting_nothing_and_settles() {
    under_each_release(|s| {
        let dir = attached(s, "workspace");
        let exclude = dir.join(".git/info/exclude");
        let log = s.dir().join("trace");
        for words in [
            &["-fX", "-e", "x"][..],
            &["-fX", "-ex"],
            &["-fX", "--exclude=x"],
            &["-fX", "--exclude", "x"],
            &["-fXe", "x"],
            &["-e", "x", "-fX"],
            &["-nX", "-e", "x"],
            &["-fX", "-e"],
        ] {
            write(&dir, ".gitdupe", b"notes\n.vscode\nby-hand\n");
            let before = Tree::of(&dir).without(&[&exclude]);
            let (output, runs) = run_traced(
                s.git(["dupe", "clean"].iter().chain(words)).from(&dir),
                &log,
            );
            refused_x_with_e(&output);
            assert_eq!(runs.of("clean"), 0, "{words:?}: {runs:?}");
            let changed = before.changed_in(&Tree::of(&dir).without(&[&exclude]));
            assert!(changed.is_empty(), "{words:?} changed {changed:?}");
            // Settle ran: the line added by hand since the last command is in the region.
            assert!(
                region_rules(&dir).contains(&b"/by-hand".to_vec()),
                "{words:?}"
            );
            write(&dir, ".gitdupe", b"notes\n.vscode\n");
            let settled = s.git(["dupe", "status"]).from(&dir).run();
            assert_eq!(settled.end, End::Code(0), "{settled:?}");
        }
        // `-eX` is `-e` with the value `X`, and no `-X`; after `--`, `-e` is a pathspec,
        // and no `-e`: both run.
        for words in [&["-n", "-eX"][..], &["-nX", "--", "-e"]] {
            let (output, runs) = run_traced(
                s.git(["dupe", "clean"].iter().chain(words)).from(&dir),
                &log,
            );
            assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
            assert_eq!(runs.own().of("clean"), 1, "{words:?}: {output:?}");
        }
    });
}

#[test]
fn x_beside_e_where_nothing_is_attached_ends_as_every_command_there_ends() {
    under_each_release(|s| {
        let unattached = s.dir().join("unattached");
        s.repository(&unattached);
        let before = Tree::of(&unattached);
        for words in [&["-fX", "-e", "x"][..], &["-n"]] {
            let output = s
                .git(["dupe", "clean"].iter().chain(words))
                .from(&unattached)
                .run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            names(output.only_line("fatal"), b"git dupe init");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(before.changed_in(&Tree::of(&unattached)).is_empty());
            // Outside any repository, Git's own message and status.
            let outside = s
                .git(["dupe", "clean"].iter().chain(words))
                .from(s.dir())
                .run();
            let gits = s.git(locate_words(false)).from(s.dir()).run();
            assert_ne!(gits.end, End::Code(0), "{gits:?}");
            assert!(outside.stdout.is_empty(), "{outside:?}");
            assert_eq!(outside.end, gits.end, "{outside:?}");
            assert_eq!(outside.stderr, gits.stderr, "{outside:?}");
        }
    });
}

/// The places a word is read in: an attached workspace, an unattached repository, and
/// outside any repository.
fn places(s: &Scenario) -> [PathBuf; 3] {
    let attached = attached(s, "attached");
    let unattached = s.dir().join("unattached");
    s.repository(&unattached);
    let outside = s.dir().join("outside");
    fs::create_dir(&outside).unwrap();
    [attached, unattached, outside]
}

#[test]
fn a_word_the_table_does_not_hold_is_a_usage_error_before_anything_is_located() {
    under_each_release(|s| {
        let text = clean_text(s);
        let places = places(s);
        let log = s.dir().join("trace");
        let exclude = places[0].join(".git/info/exclude");
        let region = fs::read(&exclude).unwrap();
        for (words, word) in [
            (&["--dry"][..], "--dry"),
            (&["-fdq", "--end-of-options"], "--end-of-options"),
            (&["--no-quiet"], "--no-quiet"),
            (&["--d"], "--d"),
            (&["--X"], "--X"),
            (&["--=x"], "--=x"),
            (&["--interactive=1"], "--interactive=1"),
            (&["-fZ"], "-fZ"),
            (&["-n", "--exclude-standard"], "--exclude-standard"),
            (&["--no-exclude"], "--no-exclude"),
            (&["-n", "x", "-X", "-e", "y", "--forc"], "--forc"),
            // The first help request or word not held decides.
            (&["--dry", "-h"], "--dry"),
        ] {
            for place in &places {
                let (output, runs) = run_traced(
                    s.git(["dupe", "clean"].iter().chain(words)).from(place),
                    &log,
                );
                assert_eq!(output.end, End::Code(129), "{words:?}: {output:?}");
                assert!(output.stdout.is_empty(), "{words:?}: {output:?}");
                let line = output.line_then("error", usage_line(&text));
                names(line, word.as_bytes());
                names(line, b"git dupe git");
                assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
                assert_eq!(
                    fs::read(&exclude).unwrap(),
                    region,
                    "settled after {words:?}"
                );
            }
        }
    });
}

#[test]
fn every_spelling_of_the_help_request_prints_the_text_of_clean_anywhere() {
    under_each_release(|s| {
        let text = clean_text(s);
        assert!(text.starts_with(b"usage: git dupe clean"), "{text:?}");
        for spelling in [
            "-q",
            "--quiet",
            "-n",
            "--dry-run",
            "-f",
            "--force",
            "-i",
            "--interactive",
            "-d",
            "-x",
            "-X",
            "-e",
            "--exclude",
        ] {
            names(&text, spelling.as_bytes());
        }
        let places = places(s);
        let log = s.dir().join("trace");
        for words in [
            &["clean", "-h"][..],
            &["clean", "--help"],
            &["help", "clean"],
            &["clean", "-fdx", "-h"],
            &["clean", "x", "--help", "--dry"],
        ] {
            for place in &places {
                let (output, runs) =
                    run_traced(s.git(["dupe"].iter().chain(words)).from(place), &log);
                assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
                assert_eq!(output.stdout, text, "{words:?}");
                assert!(output.stderr.is_empty(), "{words:?}: {output:?}");
                assert_eq!(runs.commands(), [b"dupe".as_slice()], "{words:?}");
            }
        }
        // After `--`, `-h` is a pathspec of the user's: Git's own `clean` runs.
        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-n", "--", "-h"]).from(&places[0]),
            &log,
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(runs.own().of("clean"), 1, "{output:?}");
    });
}

#[test]
fn a_pathspec_reaches_git_as_typed_whatever_it_holds() {
    under_each_release(|s| {
        let dir = attached(s, "workspace");
        let log = s.dir().join("trace");
        for pathspec in ["*.txt", ":(glob)**/*.md", ":/", "../outside", "/absolute"] {
            let (output, runs) = run_traced(
                s.git(["dupe", "clean", "-n", "--", pathspec])
                    .from(&dir.join("notes")),
                &log,
            );
            let run = clean_run(&runs);
            assert_eq!(run.last().unwrap(), pathspec.as_bytes(), "{output:?}");
            let expected = s
                .git(["clean", "-n", "--", pathspec])
                .from(&dir.join("notes"))
                .run();
            assert_eq!(output.end, expected.end, "{pathspec}: {output:?}");
        }
    });
}

#[test]
fn an_unreadable_private_repository_ends_clean_before_it_runs_and_deletes_nothing() {
    under_each_release(|s| {
        let dir = attached(s, "workspace");
        let exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
        fs::rename(dir.join(".git/dupe"), s.dir().join("saved-private")).unwrap();
        fs::create_dir(dir.join(".git/dupe")).unwrap();
        let expected = s.private(&dir).git(["ls-files", "-z", "--full-name"]).run();
        assert_ne!(expected.end, End::Code(0), "{expected:?}");
        let before = Tree::of(&dir);
        let (output, runs) = run_traced(
            s.git(["dupe", "clean", "-fdx"]).from(&dir),
            &s.dir().join("trace"),
        );
        assert_eq!(
            output.end, expected.end,
            "{output:?}; private: {expected:?}"
        );
        assert_eq!(runs.of("clean"), 0, "{runs:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        names(warnings[0], b"git dupe init");
        assert!(before.changed_in(&Tree::of(&dir)).is_empty());
        assert_eq!(fs::read(dir.join(".git/info/exclude")).unwrap(), exclude);
    });
}

/// `dir` after `git dupe clean -fdx` from its root: the hidden paths of `daily_state` and
/// the public files stand, and the untracked files are gone.
fn cleaned_as_clean_fdx(dir: &Path) {
    for path in [
        ".gitdupe",
        ".env.local",
        ".vscode/settings.json",
        "docs/notes.md",
        "notes/a.md",
        "notes/untracked.md",
        "README.md",
        "docs/design.md",
    ] {
        assert!(dir.join(path).exists(), "{path} was deleted");
    }
    assert!(!dir.join("scratch.txt").exists(), "scratch.txt was kept");
}

#[test]
fn an_alias_that_reaches_clean_is_clean_with_its_words_and_its_help() {
    under_each_release(|s| {
        let text = clean_text(s);
        for private in [false, true] {
            let dir = attached(s, &format!("workspace-{private}"));
            for (word, expansion) in [("wipe", "clean -fdx"), ("tidy", "wipe")] {
                let key = format!("alias.{word}");
                if private {
                    // A conflicting global value makes the private source observable.
                    s.git(["config", "--global", &key, "help"]).succeeds();
                    s.private(&dir).git(["config", &key, expansion]).succeeds();
                } else {
                    s.git(["config", "--global", &key, expansion]).succeeds();
                }
            }
            let help = s.git(["dupe", "wipe", "-h"]).from(&dir).run();
            assert_eq!(help.end, End::Code(0), "{help:?}");
            assert_eq!(help.stdout, text);
            for alias in ["wipe", "tidy"] {
                let copy_of = s.dir().join(format!("{alias}-{private}"));
                copy(&dir, &copy_of);
                let copy = copy_of;
                let (output, runs) = run_traced(
                    s.git(["dupe", alias]).from(&copy),
                    &s.dir().join("alias-trace"),
                );
                assert_eq!(output.end, End::Code(0), "{alias}: {output:?}");
                assert!(output.stderr.is_empty(), "{alias}: {output:?}");
                assert_eq!(runs.own().of("clean"), 1, "{alias}: {runs:?}");
                cleaned_as_clean_fdx(&copy);
            }
            let literal = s.git(["dupe", "clean", "-fdx"]).from(&dir).run();
            assert_eq!(literal.end, End::Code(0), "{literal:?}");
            cleaned_as_clean_fdx(&dir);
        }
    });
}
