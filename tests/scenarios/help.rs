//! `git dupe help` and `git dupe -h`: the general text and `help`'s own, printed without a
//! repository, without Git, and with nothing installed but the executable; `git help` for
//! a word that is not one of git-dupe's commands. The texts of `init`, `hide`, `unhide`,
//! `status`, `add`, and `clean` are pinned beside their commands, and `clone`'s here.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::harness::{
    End, general_text, holds, oldest_supported, run_traced, under_each_release, usage_line,
};

#[test]
fn help_needs_only_the_installed_executable_outside_any_repository() {
    under_each_release(|s| {
        let built = Path::new(env!("CARGO_BIN_EXE_git-dupe"));
        let installed = s.dir().join("installed");
        fs::create_dir(&installed).unwrap();
        let executable = installed.join("git-dupe");
        fs::copy(built, &executable).unwrap();
        assert_eq!(
            fs::metadata(&executable).unwrap().permissions().mode(),
            fs::metadata(built).unwrap().permissions().mode()
        );
        assert_eq!(
            fs::read_dir(&installed)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>(),
            [OsStr::new("git-dupe")]
        );

        let answer = s.git(["--exec-path"]).succeeds();
        let exec_path = Path::new(OsStr::from_bytes(answer.stdout.trim_ascii_end()));
        assert!(exec_path.ends_with("libexec/git-core"));
        let release = exec_path.parent().unwrap().parent().unwrap();
        let bin = release.join("bin");
        assert!(bin.join("git").is_file());
        let path = std::env::join_paths([&installed, &bin]).unwrap();
        let directories = std::env::split_paths(&path).collect::<Vec<_>>();
        assert_eq!(directories, [installed.as_path(), bin.as_path()]);
        assert!(!directories.iter().any(|dir| dir == built.parent().unwrap()));

        let inside = fs::read(&executable).unwrap();
        for words in [
            &["dupe", "help"][..],
            &["dupe", "-h"],
            &["dupe", "add", "-h"],
            &["dupe", "hide", "-h"],
        ] {
            let ordinary = s.git(words).from(s.dir()).succeeds();
            assert!(ordinary.stderr.is_empty(), "{words:?}: {ordinary:?}");
            assert!(!ordinary.stdout.is_empty(), "{words:?}: {ordinary:?}");
            let help = s.git(words).from(s.dir()).variable("PATH", &path).run();
            assert_eq!(help.end, End::Code(0), "{words:?}: {help:?}");
            assert!(help.stderr.is_empty(), "{words:?}: {help:?}");
            assert_eq!(help.stdout, ordinary.stdout, "{words:?}: {help:?}");
            // The text is in the executable itself, never read from the build's tree.
            assert!(holds(&inside, &help.stdout), "{words:?}");
        }
    });
}

#[test]
fn h_alone_and_help_print_the_general_text_whatever_the_workspace() {
    under_each_release(|s| {
        let general = general_text(s);
        // F4's range: the oldest release or newer, below 3.0.0.
        let range = format!("Git {} or newer, below 3.0.0", oldest_supported());
        assert!(
            holds(&general, range.as_bytes()),
            "{}",
            String::from_utf8_lossy(&general)
        );

        let plain = s.dir().join("plain");
        let bare = s.dir().join("bare.git");
        s.repository(&plain);
        s.bare_repository(&bare);
        for place in [s.dir(), &plain, &plain.join(".git"), &bare] {
            for words in [["dupe", "-h"], ["dupe", "help"]] {
                let help = s.git(words).from(place).run();
                assert_eq!(help.stdout, general, "{words:?}: {help:?}");
                assert!(help.stderr.is_empty(), "{words:?}: {help:?}");
                assert_eq!(help.end, End::Code(0), "{words:?}: {help:?}");
            }
        }
    });
}

#[test]
fn a_help_request_among_the_words_of_help_prints_its_own_text() {
    let invocations: [&[&str]; 4] = [
        &["help"],
        &["-h"],
        &["log", "--help"],
        &["log", "status", "-h"],
    ];
    under_each_release(|s| {
        let general = general_text(s);
        let own = s.git(["dupe", "help", "help"]).run().stdout;
        assert!(own.starts_with(b"usage: git dupe help"), "{own:?}");
        assert_ne!(own, general);
        for words in invocations {
            let help = s.git(["dupe", "help"].iter().chain(words)).run();
            assert_eq!(help.stdout, own, "{words:?}: {help:?}");
            assert!(help.stderr.is_empty(), "{words:?}: {help:?}");
            assert_eq!(help.end, End::Code(0), "{words:?}: {help:?}");
        }
    });
}

#[test]
fn a_misused_help_is_a_usage_error_without_a_repository() {
    let invocations: [&[&str]; 4] = [
        &["log", "status"],
        &["--all"],
        &["--", "-h"],
        &["--all\nhint: typed\x1b[31m"],
    ];
    under_each_release(|s| {
        let own = s.git(["dupe", "help", "help"]).run().stdout;
        for words in invocations {
            // Where no repository is: had `help` located first, this would end with 128.
            let error = s.git(["dupe", "help"].iter().chain(words)).run();
            assert!(error.stdout.is_empty(), "{words:?}: {error:?}");
            let fault = error.line_then("error", usage_line(&own));
            assert!(!fault.contains(&0x1b), "{words:?}: {error:?}");
            assert_eq!(error.end, End::Code(129), "{words:?}: {error:?}");
        }
    });
}

#[test]
fn help_for_any_other_word_is_git_help_with_gits_own_end() {
    let not_utf8 = OsStr::from_bytes(b"\xff\xfe");
    under_each_release(|s| {
        for command in [OsStr::new("log"), not_utf8] {
            // Where no repository is, `git help` still answers, and git-dupe adds nothing.
            let gits = s.git([OsStr::new("help"), command]).run();
            let ours = s
                .git([OsStr::new("dupe"), OsStr::new("help"), command])
                .run();
            assert!(ours == gits, "{ours:?}, where Git ends as {gits:?}");
            let after_the_end_of_options = s
                .git(
                    ["dupe", "help", "--"]
                        .map(OsStr::new)
                        .into_iter()
                        .chain([command]),
                )
                .run();
            assert!(
                after_the_end_of_options == gits,
                "{after_the_end_of_options:?}, where Git ends as {gits:?}"
            );
        }
    });
}

#[test]
fn help_clone_prints_its_text_anywhere_without_running_git() {
    under_each_release(|s| {
        let text = s.git(["dupe", "help", "clone"]).succeeds();
        assert!(text.stderr.is_empty(), "{text:?}");
        assert_eq!(
            usage_line(&text.stdout),
            b"usage: git dupe clone URL [-b BRANCH]\n"
        );
        let words = String::from_utf8_lossy(&text.stdout);
        let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
        // The way to start over, as the refusal in an attached workspace names it.
        for phrase in [
            "Unlike \"git clone\"",
            "-b",
            "\"git dupe detach\"",
            "\"git dupe detach --force\"",
            "\"git dupe clone\" start over",
        ] {
            assert!(words.contains(phrase), "missing {phrase}: {words}");
        }
        // Git's own `clone` attaches nothing: no route to it is offered for a refused URL.
        assert!(!words.contains("git dupe git"), "{words}");

        let plain = s.dir().join("plain");
        let attached = s.dir().join("attached");
        let bare = s.dir().join("bare.git");
        s.repository(&plain);
        s.attached_project(&attached);
        s.bare_repository(&bare);
        let log = s.dir().join("trace");
        for words in [
            &["help", "clone"][..],
            &["clone", "-h"],
            &["clone", "--help"],
            &["clone", "x", "-b", "y", "-h"],
            &["clone", "--depth", "1", "--help"],
        ] {
            for place in [s.dir(), &plain, &plain.join(".git"), &attached, &bare] {
                let (help, runs) =
                    run_traced(s.git(["dupe"].iter().chain(words)).from(place), &log);
                assert_eq!(help, text, "{words:?}");
                // Answered without a repository and without Git, not even `git help`.
                assert_eq!(runs.own().count(), 0, "{words:?}: {runs:?}");
            }
        }
    });
}

#[test]
fn a_git_child_killed_by_a_signal_ends_git_dupe_with_128_plus_its_number() {
    const SIGTERM: i32 = 15;
    under_each_release(|s| {
        // A viewer that kills the `git help` that runs it.
        for setting in [["man.viewer", "boom"], ["man.boom.cmd", "kill -TERM $$ #"]] {
            let set = s.git(["config", "--global"].iter().chain(&setting)).run();
            assert_eq!(set.end, End::Code(0), "{set:?}");
        }
        let gits = s.git(["help", "log"]).run();
        assert_eq!(gits.end, End::Signal(SIGTERM), "{gits:?}");

        let ours = s.git(["dupe", "help", "log"]).run();
        assert_eq!(ours.end, End::Code(128 + SIGTERM), "{ours:?}");
        assert_eq!(ours.stderr, gits.stderr, "{ours:?}");
    });
}

#[test]
fn output_that_cannot_be_written_is_a_refusal_and_never_a_panic() {
    let invocations: [&[&str]; 5] = [&[], &["-h"], &["--version"], &["help"], &["help", "help"]];
    under_each_release(|s| {
        for words in invocations {
            let refusal = s
                .git(["dupe"].iter().chain(words))
                .standard_output_to(Path::new("/dev/full"))
                .run();
            // One line of git-dupe's own, not a panic's text; 128, not a panic's 101.
            refusal.only_line("fatal");
            assert_eq!(refusal.end, End::Code(128), "{words:?}: {refusal:?}");
        }
    });
}

#[test]
fn general_help_names_hidden_paths_stash_alternatives_and_recovery() {
    under_each_release(|s| {
        let general = general_text(s);
        for phrase in [
            "git dupe add <path>",
            "git dupe hide",
            ".gitdupe",
            "git clean -x",
            "git stash -a",
            "git add -f",
            "git dupe restore .",
            "git dupe git",
        ] {
            assert!(holds(&general, phrase.as_bytes()), "missing {phrase}");
        }
        let stash_sentence = general
            .split(|byte| *byte == b'\n')
            .find(|line| holds(line, b"git dupe stash"))
            .expect("stash alternative sentence");
        let sentence = String::from_utf8_lossy(stash_sentence);
        assert!(sentence.find("git dupe add").unwrap() < sentence.find("git dupe stash").unwrap());

        // Each item the general text must state, by the words that state it, read across
        // its line breaks; the transfer refusals and the versions have scenarios of their
        // own.
        let text = String::from_utf8_lossy(&general);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in [
            // Every Git command, as git-dupe runs it.
            r#"every Git command runs on their repository as "git dupe <command>"."#,
            // What a hidden path is, and the three ways to make one.
            r#"The project's Git ignores a hidden path: .gitdupe, a path it lists ("git dupe hide" or by hand), or a file "git dupe add <path>" tracks."#,
            // The refused forms of stash, and what to run instead.
            r#"Refused: stash -u and -a ("git dupe add" first, then "git dupe stash");"#,
            // Plain git, what it does to private files, and the recovery.
            "Plain git is not guarded:",
            "git clean -x deletes private files,",
            "git stash -a stashes them,",
            "git add -f makes them public,",
            "a pull or checkout may overwrite them; commit first:",
            r#""git dupe restore ." brings back what was staged."#,
            // Each worktree on its own, and what removing a linked one takes with it.
            "Each worktree is attached on its own, and git worktree remove takes its private repository with it.",
            // The unguarded route.
            "git dupe git WORDS Git with WORDS as typed, guarded by nothing",
        ] {
            assert!(text.contains(phrase), "missing {phrase}: {text}");
        }
    });
}

/// The general text has a line for every command of git-dupe's own and states the
/// refusals of G18 for every command that has them, with both routes.
#[test]
fn general_help_names_every_command_and_the_transfer_refusals() {
    under_each_release(|s| {
        let general = general_text(s);
        for line in [
            &b"  git dupe init [-b BRANCH] "[..],
            b"  git dupe clone URL ",
            b"  git dupe detach [--force] ",
        ] {
            assert!(
                general
                    .split(|byte| *byte == b'\n')
                    .any(|found| found.starts_with(line)),
                "{}",
                String::from_utf8_lossy(&general)
            );
        }
        // The command list holds a row for each of the ten commands, one after another, an
        // empty line before it; the paragraph after it begins at its own margin, the empty
        // line there having given way to what the screen must state.
        let lines: Vec<&[u8]> = general.split(|byte| *byte == b'\n').collect();
        let rows: Vec<usize> = (0..lines.len())
            .filter(|&at| lines[at].starts_with(b"  git dupe "))
            .collect();
        let (first, last) = (rows[0], rows[rows.len() - 1]);
        assert_eq!(rows, (first..=last).collect::<Vec<_>>());
        assert!(lines[first - 1].is_empty());
        assert!(lines[last + 1].starts_with(b"Refused: "));
        for word in [
            "init", "clone", "detach", "hide", "unhide", "status", "add", "clean", "git", "help",
        ] {
            let row = format!("  git dupe {word} ");
            assert!(
                rows.iter().any(|&at| lines[at].starts_with(row.as_bytes())),
                "no row for {word}"
            );
        }
        let text = String::from_utf8_lossy(&general);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(!text.contains("This build"), "{text}");
        assert!(!text.contains("refuses clone"), "{text}");
        let transfer = text
            .split(r#""git dupe stash"); "#)
            .nth(1)
            .expect("the transfer refusal text")
            .split(". Plain git")
            .next()
            .unwrap();
        // The places in full, a working tree among them, and the private remote's URL and
        // the default remote, are the page's (`manual_page`, its explanations).
        for phrase in [
            "push, pull, fetch, remote, or clone",
            "naming the project's repository or a remote of it",
            "git dupe git",
            "git dupe remote remove",
            "for a private remote",
        ] {
            assert!(transfer.contains(phrase), "missing {phrase}: {transfer}");
        }
    });
}

/// Each of the ten commands' texts says what the command does and how it differs from
/// Git, by the words that say it, read across its line breaks.
#[test]
fn each_commands_text_says_what_it_does_and_how_it_differs_from_git() {
    let differences: [(&str, &[&str]); 10] = [
        (
            "init",
            &[
                "an empty Git repository named dupe in the worktree's own Git directory, at .git/dupe in the main worktree and at .git/worktrees/<name>/dupe in a linked one",
                r#"after "git worktree move" init run in the linked worktree records its new place for plain Git"#,
                r#"Unlike "git init", it takes no directory and no other option, and run again it does not reinitialize"#,
            ],
        ),
        (
            "clone",
            &[
                "from an existing one at URL",
                r#"Unlike "git clone", it makes no new directory, takes no option but -b, and never overwrites a file."#,
            ],
        ),
        (
            "detach",
            &[
                "Removes this worktree's private repository, and its region of .git/info/exclude",
                // G27: a region whose private repository is gone is dropped by any
                // worktree's command, detach's included; G3 names what another worktree
                // still hides.
                "the other worktrees' regions stay, but for one whose private repository is gone",
                "and each that another worktree still hides, naming it",
                "Every file stays on disk",
                "There is no Git command of this name.",
            ],
        ),
        (
            "hide",
            &[
                "Hides each PATH from the project's Git: lists it in .gitdupe",
                r#"Such a path becomes private one file at a time: "git rm --cached <path>" first, a deletion from the project that every other clone receives once pushed, then "git dupe add <path>", with -f where the project ignores it."#,
                r#"There is no Git command of this name; "git dupe unhide" undoes it."#,
            ],
        ),
        (
            "unhide",
            &[
                "Stops hiding each PATH: removes every line of .gitdupe that names PATH",
                "One hint per PATH, listed or not,",
                "There is no Git command of this name.",
            ],
        ),
        (
            "git",
            &[
                "Runs Git with <command> and <args> exactly as typed, against the private repository, guarded by nothing",
            ],
        ),
        (
            "help",
            &[
                r#"with any other COMMAND, it runs "git help COMMAND""#,
                "The manual page of git-dupe, where it is installed, holds these texts and more: a quick start, examples of daily use, and what one screen has no room for, such as hidden and private paths, worktrees, a file the project ignores, what plain git does to private files, and the refused commands in full.",
                r#""man git-dupe" shows it, and so does "git dupe --help" while Git's help format is man, its default."#,
            ],
        ),
        (
            "status",
            &[
                "Shows Git's own status of the private repository, confined to the hidden paths and the paths whose deletion is staged.",
            ],
        ),
        (
            "add",
            &[
                "Git's add, against the private repository.",
                "A path only the project's Git tracks is never staged",
            ],
        ),
        (
            "clean",
            &[
                "Runs Git's own clean of the project, in the project's repository, from the current directory, with your words, and never deletes a hidden path",
            ],
        ),
    ];
    under_each_release(|s| {
        for (command, phrases) in differences {
            let help = s.git(["dupe", "help", command]).succeeds();
            let text = String::from_utf8_lossy(&help.stdout);
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            for phrase in phrases {
                assert!(text.contains(phrase), "{command}: missing {phrase}: {text}");
            }
        }
    });
}

#[test]
fn help_git_prints_its_own_text_anywhere() {
    under_each_release(|s| {
        let ordinary = s.dir().join("ordinary");
        let attached = s.dir().join("attached");
        let bare = s.dir().join("bare.git");
        s.repository(&ordinary);
        s.attached_project(&attached);
        s.bare_repository(&bare);
        let own = s.git(["dupe", "git", "-h"]).succeeds();
        assert!(own.stdout.starts_with(b"usage: git dupe git"), "{own:?}");
        for place in [
            s.dir(),
            &ordinary,
            &attached,
            &ordinary.join(".git"),
            &attached.join(".git"),
            &bare,
        ] {
            let help = s.git(["dupe", "help", "git"]).from(place).run();
            assert_eq!(help, own);
        }
    });
}

#[test]
fn help_stash_and_transfer_commands_are_plain_git_help() {
    under_each_release(|s| {
        for command in ["stash", "push", "pull", "fetch", "remote"] {
            let gits = s.git(["help", command]).run();
            let ours = s.git(["dupe", "help", command]).run();
            assert_eq!(ours, gits, "{command}");
        }
    });
}

#[test]
fn help_detach_says_that_what_force_removes_is_gone_anywhere() {
    under_each_release(|s| {
        let text = s.git(["dupe", "help", "detach"]).succeeds();
        assert!(text.stderr.is_empty(), "{text:?}");
        assert_eq!(
            usage_line(&text.stdout),
            b"usage: git dupe detach [--force]\n"
        );
        let words = String::from_utf8_lossy(&text.stdout);
        let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
        for phrase in [
            "--force",
            "History removed by \"git dupe detach --force\" is gone; git-dupe keeps no copy.",
            ".gitdupe included",
        ] {
            assert!(words.contains(phrase), "missing {phrase}: {words}");
        }
        let plain = s.dir().join("plain");
        s.repository(&plain);
        for words in [&["detach", "-h"][..], &["detach", "--force", "--help"]] {
            for place in [s.dir(), &plain] {
                let help = s.git(["dupe"].iter().chain(words)).from(place).run();
                assert_eq!(help, text, "{words:?}");
            }
        }
    });
}
