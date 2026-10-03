//! The words of commands with a table: early decisions and literal operands.

use std::path::PathBuf;

use crate::harness::{
    End, Output, Scenario, daily_state, holds, locate_words, names, region_rules, run_traced,
    under_each_release, usage_line, write,
};

/// The same reading in an attached workspace, an unattached repository, and outside.
fn reading_places(s: &Scenario) -> [PathBuf; 3] {
    let attached = s.dir().join("attached");
    s.attached_project(&attached);
    let unattached = s.dir().join("unattached");
    s.repository(&unattached);
    [attached, unattached, s.dir().to_path_buf()]
}

/// A command's text, obtained through its own help without a repository.
fn command_text(s: &Scenario, command: &str) -> Vec<u8> {
    let output = s.git(["dupe", "help", command]).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output.stdout
}

/// Exactly one error and the command's usage line, with no standard output.
fn usage_error<'o>(output: &'o Output, text: &[u8]) -> &'o [u8] {
    assert_eq!(output.end, End::Code(129), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    output.line_then("error", usage_line(text))
}

/// An early error names the unknown word and the unguarded route.
fn unknown_word(output: &Output, text: &[u8], word: &str) {
    let line = usage_error(output, text);
    names(line, word.as_bytes());
    names(line, b"git dupe git");
}

/// Help succeeds with exactly the command's text.
fn printed_text(output: &Output, text: &[u8]) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert_eq!(output.stdout, text, "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn status_unknown_words_decide_before_location_and_never_settle() {
    under_each_release(|s| {
        let text = command_text(s, "status");
        let places = reading_places(s);
        write(&places[0], ".gitdupe", b"notes\n");
        let log = s.dir().join("trace");
        for word in [
            "--porc",
            "--end-of-options",
            "--porcelian",
            "-x",
            "--no-verbose",
            "--short=1",
            "-sX",
            "-vh",
            "-",
        ] {
            for words in [vec![word], vec!["notes", word], vec![word, "-h"]] {
                for place in &places {
                    let (output, runs) = run_traced(
                        s.git(["dupe", "status"].into_iter().chain(words.iter().copied()))
                            .from(place),
                        &log,
                    );
                    unknown_word(&output, &text, word);
                    assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
                    assert_eq!(region_rules(&places[0]), [b"/.gitdupe".as_slice()]);
                }
            }
        }
    });
}

#[test]
fn status_unknown_word_with_a_newline_stays_one_error_line() {
    under_each_release(|s| {
        let text = command_text(s, "status");
        let places = reading_places(s);
        write(&places[0], ".gitdupe", b"notes\n");
        let log = s.dir().join("trace");
        let mut errors = Vec::new();
        for words in [
            vec!["--a\nb"],
            vec!["notes", "--a\nb"],
            vec!["--a\nb", "-h"],
        ] {
            for place in &places {
                let (output, runs) = run_traced(
                    s.git(["dupe", "status"].into_iter().chain(words.iter().copied()))
                        .from(place),
                    &log,
                );
                let line = usage_error(&output, &text);
                names(line, b"git dupe git");
                assert_eq!(
                    output.stderr.split_inclusive(|&b| b == b'\n').count(),
                    2,
                    "{output:?}"
                );
                assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
                assert_eq!(region_rules(&places[0]), [b"/.gitdupe".as_slice()]);
                if !holds(line, b"--a\\nb") {
                    errors.push((words.clone(), place.clone(), output));
                }
            }
        }
        // Escape the newline so the unknown word can be named on one line. Check after
        // all readings so a naming failure does not hide their process and region checks.
        assert!(
            errors.is_empty(),
            "each error must name the newline-bearing word: {errors:?}"
        );
    });
}

#[test]
fn status_first_help_or_unknown_word_decides_and_double_dash_ends_reading() {
    under_each_release(|s| {
        let text = command_text(s, "status");
        let places = reading_places(s);
        let log = s.dir().join("trace");
        for place in &places {
            for words in [
                &["-h", "--porc"][..],
                &["x", "--help"],
                &["--help"],
                &["-s", "-h"],
            ] {
                let (output, runs) = run_traced(
                    s.git(["dupe", "status"].into_iter().chain(words.iter().copied()))
                        .from(place),
                    &log,
                );
                printed_text(&output, &text);
                assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
            }
            let output = s.git(["dupe", "status", "--porc", "-h"]).from(place).run();
            unknown_word(&output, &text, "--porc");
        }
        for word in ["--porc", "-h"] {
            let output = s.git(["dupe", "status", "--", word]).from(&places[0]).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.lines("error").is_empty(), "{output:?}");
        }
    });
}

#[test]
fn status_help_lists_the_whole_table_on_one_screen() {
    under_each_release(|s| {
        let text = command_text(s, "status");
        assert!(text.starts_with(b"usage: git dupe status"));
        assert!(text.split_inclusive(|&b| b == b'\n').count() <= 23);
        for option in [
            "--verbose",
            "--short",
            "--branch",
            "--show-stash",
            "--ahead-behind",
            "--no-ahead-behind",
            "--porcelain",
            "--long",
            "--null",
            "--untracked-files",
            "--ignored",
            "--ignore-submodules",
            "--column",
            "--no-column",
            "--renames",
            "--no-renames",
            "--find-renames",
        ] {
            names(&text, option.as_bytes());
        }
        for place in reading_places(s) {
            for words in [
                &["dupe", "help", "status"][..],
                &["dupe", "status", "-h"],
                &["dupe", "status", "--help"],
            ] {
                let output = s.git(words).from(&place).run();
                printed_text(&output, &text);
            }
        }
    });
}

#[test]
fn status_operand_usage_waits_for_location_and_attachment_and_never_settles() {
    under_each_release(|s| {
        let text = command_text(s, "status");
        let places = reading_places(s);
        write(&places[0], ".gitdupe", b"notes\n");
        let locate = s.git(locate_words(false)).run();
        assert_eq!(locate.end, End::Code(128), "{locate:?}");
        for word in ["a*", "x?", "[x]", ":(glob)x", ":x", ":/a*", "../x"] {
            let output = s.git(["dupe", "status", word]).from(&places[0]).run();
            let line = usage_error(&output, &text);
            names(line, word.as_bytes());
            if word == "../x" {
                assert!(!holds(line, b"git dupe git status"), "{output:?}");
            } else {
                names(line, b"git dupe git status");
            }
            assert_eq!(region_rules(&places[0]), [b"/.gitdupe".as_slice()]);
            let output = s.git(["dupe", "status", word]).from(&places[1]).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            names(output.only_line("fatal"), b"git dupe init");
            let output = s.git(["dupe", "status", word]).run();
            assert_eq!(output.end, locate.end, "{output:?}");
            assert_eq!(output.stderr, locate.stderr, "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    });
}

#[test]
fn status_root_operands_are_literal_from_a_subdirectory() {
    under_each_release(|s| {
        let dir = s.dir().join("attached");
        s.attached_project(&dir);
        write(&dir, "sub/placeholder", b"");
        for word in [":/", ":/notes"] {
            let output = s.git(["dupe", "status", word]).from(&dir.join("sub")).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.lines("error").is_empty(), "{output:?}");
        }
    });
}

#[test]
fn add_unknown_words_decide_before_location_and_never_settle() {
    under_each_release(|s| {
        let text = command_text(s, "add");
        let places = reading_places(s);
        write(&places[0], ".gitdupe", b"notes\n");
        let log = s.dir().join("trace");
        for word in [
            "--pathspec-from-file=x",
            "-i",
            "--interactive",
            "--al",
            "--end-of-options",
            "-Ai",
        ] {
            for words in [vec![word], vec!["notes", word], vec![word, "-h"]] {
                for place in &places {
                    let (output, runs) = run_traced(
                        s.git(["dupe", "add"].into_iter().chain(words.iter().copied()))
                            .from(place),
                        &log,
                    );
                    unknown_word(&output, &text, word);
                    assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
                    assert_eq!(region_rules(&places[0]), [b"/.gitdupe".as_slice()]);
                }
            }
        }
    });
}

#[test]
fn add_help_lists_the_whole_table_on_one_screen() {
    under_each_release(|s| {
        let text = command_text(s, "add");
        assert!(text.starts_with(b"usage: git dupe add"));
        assert!(text.split_inclusive(|&b| b == b'\n').count() <= 23);
        for option in [
            "--dry-run",
            "--verbose",
            "--patch",
            "--edit",
            "--force",
            "--update",
            "--renormalize",
            "--intent-to-add",
            "--all",
            "--no-all",
            "--ignore-removal",
            "--no-ignore-removal",
            "--refresh",
            "--ignore-errors",
            "--ignore-missing",
            "--sparse",
            "--chmod",
        ] {
            names(&text, option.as_bytes());
        }
        let log = s.dir().join("trace");
        for place in reading_places(s) {
            for words in [
                &["dupe", "help", "add"][..],
                &["dupe", "add", "-h"],
                &["dupe", "add", "--help"],
                &["dupe", "add", "-h", "--al"],
                &["dupe", "add", "x", "--help"],
            ] {
                let (output, runs) = run_traced(s.git(words).from(&place), &log);
                printed_text(&output, &text);
                assert_eq!(runs.commands(), [b"dupe".as_slice()], "{output:?}");
            }
        }
    });
}

#[test]
fn add_operand_usage_waits_for_location_and_attachment_and_never_settles() {
    under_each_release(|s| {
        let text = command_text(s, "add");
        let places = reading_places(s);
        write(&places[0], ".gitdupe", b"notes\n");
        let outside = s.dir().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let outside = outside.to_str().unwrap();
        let locate = s.git(locate_words(false)).run();
        assert_eq!(locate.end, End::Code(128), "{locate:?}");
        for word in [
            "a*", "a?", "a[1]", ":(glob)x", ":!x", ":/a*", "../x", outside,
        ] {
            let output = s.git(["dupe", "add", word]).from(&places[0]).run();
            let line = usage_error(&output, &text);
            names(line, word.as_bytes());
            if word == "../x" || word == outside {
                names(line, b"outside the working tree");
                assert!(!holds(line, b"git dupe git add"), "{output:?}");
            } else {
                names(line, b"git dupe git add");
            }
            assert_eq!(region_rules(&places[0]), [b"/.gitdupe".as_slice()]);
            let output = s.git(["dupe", "add", word]).from(&places[1]).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            names(output.only_line("fatal"), b"git dupe init");
            let output = s.git(["dupe", "add", word]).run();
            assert_eq!(output.end, locate.end, "{output:?}");
            assert_eq!(output.stderr, locate.stderr, "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    });
}

#[test]
fn add_root_operand_stages_from_a_subdirectory() {
    under_each_release(|s| {
        let dir = s.dir().join("attached");
        daily_state(s, &dir);
        write(&dir, "notes/today.md", b"today\n");
        let output = s
            .git(["dupe", "add", ":/notes"])
            .from(&dir.join("docs"))
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let staged = s
            .private(&dir)
            .git(["diff", "--cached", "--name-status", "-z"])
            .succeeds();
        assert_eq!(staged.stdout, b"A\0notes/today.md\0");
        assert_eq!(
            s.private(&dir)
                .git(["show", ":notes/today.md"])
                .succeeds()
                .stdout,
            b"today\n"
        );
    });
}

#[test]
fn every_add_option_spelling_keeps_gits_exit_status() {
    under_each_release(|s| {
        let dir = s.dir().join("attached");
        let twin = s.dir().join("private");
        for place in [&dir, &twin] {
            daily_state(s, place);
            write(place, "x.sh", b"echo private\n");
        }
        let text = command_text(s, "add");
        let cases: &[&[&str]] = &[
            &["-n"],
            &["--dry-run"],
            &["-v"],
            &["--verbose"],
            &["-p"],
            &["--patch"],
            &["-e"],
            &["--edit"],
            &["-f"],
            &["--force"],
            &["-u"],
            &["--update"],
            &["--renormalize"],
            &["-N"],
            &["--intent-to-add"],
            &["-A"],
            &["--all"],
            &["--no-all"],
            &["--ignore-removal"],
            &["--no-ignore-removal"],
            &["--refresh"],
            &["--ignore-errors"],
            &["--ignore-missing"],
            &["--sparse"],
            &["-nv"],
            &["-Anv"],
            &["--chmod=+x"],
            &["--chmod=-x"],
            &["--chmod", "+x"],
            &["--chmod", "-x"],
            &["--chmod", "-h"],
            &["--chmod", "--"],
        ];
        for &words in cases {
            // Both runs start at the committed private index, including after -N or chmod.
            for place in [&dir, &twin] {
                s.private(place).git(["reset", "-q"]).succeeds();
            }
            let expected = s
                .private(&twin)
                .git(
                    ["add"]
                        .into_iter()
                        .chain(words.iter().copied())
                        .chain(["--", ":(top,literal)x.sh"]),
                )
                .variable("GIT_EDITOR", "true")
                .run();
            // Every listed release takes the spelling: Git's usage error is 129.
            assert_ne!(
                expected.end,
                End::Code(129),
                "words: {words:?}; private: {expected:?}"
            );
            let output = s
                .git(
                    ["dupe", "add"]
                        .into_iter()
                        .chain(words.iter().copied())
                        .chain(["x.sh"]),
                )
                .from(&dir)
                .variable("GIT_EDITOR", "true")
                .run();
            assert_eq!(
                output.end, expected.end,
                "words: {words:?}; {output:?}; private: {expected:?}"
            );
            assert!(
                !output
                    .lines("error")
                    .iter()
                    .any(|line| holds(line, b"git dupe git")),
                "words: {words:?}; {output:?}"
            );
            if words == ["--chmod", "-h"] {
                assert_ne!(output.stdout, text, "{output:?}");
            }
            // What Git's add staged: modes, blobs, and paths, as in the twin.
            let staged: Vec<Vec<u8>> = [&dir, &twin]
                .into_iter()
                .map(|place| {
                    s.private(place)
                        .git(["ls-files", "--stage", "-z"])
                        .succeeds()
                        .stdout
                })
                .collect();
            assert_eq!(staged[0], staged[1], "words: {words:?}; {output:?}");
        }
    });
}

#[test]
fn add_double_dash_ends_option_reading() {
    under_each_release(|s| {
        let dir = s.dir().join("attached");
        daily_state(s, &dir);
        for word in ["-h", "--al"] {
            write(&dir, word, b"literal\n");
            let output = s.git(["dupe", "add", "--", word]).from(&dir).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.lines("error").is_empty(), "{output:?}");
            assert_eq!(
                s.private(&dir)
                    .git(["show", &format!(":{word}")])
                    .succeeds()
                    .stdout,
                b"literal\n"
            );
        }
    });
}
