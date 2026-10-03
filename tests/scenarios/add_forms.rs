//! `add`'s directory, root, and all-flag forms over the hidden paths.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, daily_state, holds, names, private_add, private_commit, region_rules,
    run_traced, under_each_release, write,
};

const NORMAL_CHANGES: &[u8] = b"M\0.env.local\0M\0.vscode/settings.json\0A\0notes/today.md\0";

fn changes(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    for path in [
        "notes/today.md",
        ".vscode/launch.json",
        "docs/api.md",
        "src/scratch.py",
        ".env.local",
        ".vscode/settings.json",
    ] {
        write(&dir, path, b"changed\n");
    }
    dir
}

fn staged(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.private(dir)
        .git(["diff", "--cached", "--name-status", "-z"])
        .succeeds()
        .stdout
}

fn no_own_hint(output: &Output) {
    assert!(
        output
            .lines("hint")
            .iter()
            .all(|line| !holds(line, b"git dupe")),
        "{output:?}"
    );
}

#[test]
fn directory_form_stages_only_hidden_paths_under_the_current_directory() {
    under_each_release(|s| {
        let dir = changes(s, "notes");
        s.git(["dupe", "add", "."])
            .from(&dir.join("notes"))
            .succeeds();
        assert_eq!(staged(s, &dir), b"A\0notes/today.md\0");
    });
}

/// `./` is the directory form: from a directory that holds hidden paths, at or below a
/// hidden path, or holding none, it stages what `.` stages there and hides nothing, where a
/// literal directory would be hidden or refused.
#[test]
fn dot_slash_is_the_directory_form() {
    under_each_release(|s| {
        for (index, (from, expected)) in [
            ("docs", &b"M\0docs/notes.md\0"[..]),
            ("notes", b"A\0notes/sub/x.md\0A\0notes/today.md\0"),
            ("notes/sub", b"A\0notes/sub/x.md\0"),
            ("src", b""),
        ]
        .into_iter()
        .enumerate()
        {
            let dir = changes(s, &format!("dot-slash-{index}"));
            let twin = changes(s, &format!("dot-twin-{index}"));
            for workspace in [&dir, &twin] {
                write(workspace, "docs/notes.md", b"changed\n");
                write(workspace, "notes/sub/x.md", b"new\n");
            }
            let dot = s.git(["dupe", "add", "."]).from(&twin.join(from)).run();
            let output = s.git(["dupe", "add", "./"]).from(&dir.join(from)).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert_eq!(output.end, dot.end, "{output:?}; dot: {dot:?}");
            assert_eq!(output.stderr, dot.stderr, "{output:?}; dot: {dot:?}");
            assert_eq!(staged(s, &dir), expected, "from {from}");
            assert_eq!(staged(s, &twin), expected, "from {from}");
            assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n.vscode\n");
            assert_eq!(region_rules(&dir), region_rules(&twin));
        }
    });
}

/// `--all` with no operand, from a directory outside every hidden path, stages every
/// change under every hidden path and no public file.
#[test]
fn long_all_without_an_operand_stages_every_hidden_change_from_outside_them() {
    under_each_release(|s| {
        let dir = changes(s, "long-all");
        let (output, runs) = run_traced(
            s.git(["dupe", "add", "--all"]).from(&dir.join("src")),
            &s.dir().join("long-all.trace"),
        );
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(runs.of("add"), 2, "{runs:?}");
        assert_eq!(staged(s, &dir), NORMAL_CHANGES);
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n.vscode\n");
    });
}

#[test]
fn root_and_all_forms_split_ignored_tracked_changes_into_a_second_run() {
    under_each_release(|s| {
        for (index, (options, from)) in [
            (vec!["."], ""),
            (vec![":/"], "docs"),
            (vec!["-A"], "docs"),
            (vec!["absolute"], "docs"),
            (vec!["-Av"], ""),
        ]
        .into_iter()
        .enumerate()
        {
            let dir = changes(s, &format!("root-{index}"));
            let absolute = dir.to_str().unwrap();
            let options = if options == ["absolute"] {
                vec![absolute]
            } else {
                options
            };
            let (output, runs) = run_traced(
                s.git(["dupe", "add"].into_iter().chain(options))
                    .from(&dir.join(from)),
                &s.dir().join(format!("root-{index}.trace")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert_eq!(runs.of("add"), 2, "{runs:?}");
            assert_eq!(staged(s, &dir), NORMAL_CHANGES);
            assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n.vscode\n");
        }
    });
}

#[test]
fn force_forms_stage_ignored_files_without_splitting() {
    under_each_release(|s| {
        for (index, operand) in [".", ".vscode/launch.json"].into_iter().enumerate() {
            let dir = changes(s, &format!("force-{index}"));
            let (output, runs) = run_traced(
                s.git(["dupe", "add", "-f", operand]).from(&dir),
                &s.dir().join(format!("force-{index}.trace")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert_eq!(runs.of("add"), 1, "{runs:?}");
            let expected: &[u8] = if operand == "." {
                b"M\0.env.local\0A\0.vscode/launch.json\0M\0.vscode/settings.json\0A\0notes/today.md\0"
            } else {
                b"A\0.vscode/launch.json\0"
            };
            assert_eq!(staged(s, &dir), expected);
        }
    });
}

#[test]
fn first_failed_run_keeps_its_status_and_the_second_run_still_stages() {
    under_each_release(|s| {
        let dir = changes(s, "missing");
        let twin = changes(s, "missing-twin");
        let expected = s
            .private(&twin)
            .git(["add", "--", ":(top,literal)missing.txt"])
            .run();
        assert_ne!(expected.end, End::Code(0), "{expected:?}");
        let (output, runs) = run_traced(
            s.git(["dupe", "add", ".", "missing.txt"]).from(&dir),
            &s.dir().join("missing.trace"),
        );
        assert_eq!(
            output.end, expected.end,
            "{output:?}; private: {expected:?}"
        );
        assert_eq!(runs.of("add"), 2, "{runs:?}");
        assert_eq!(
            staged(s, &dir),
            b"M\0.env.local\0M\0.vscode/settings.json\0"
        );
    });
}

#[test]
fn forms_with_nothing_hidden_hint_without_running_add_or_changing_the_index() {
    under_each_release(|s| {
        for (index, (daily, words, from)) in [
            (true, vec!["."], "src"),
            (false, vec!["."], ""),
            (false, vec![":/"], ""),
            (false, vec!["-A"], ""),
        ]
        .into_iter()
        .enumerate()
        {
            let dir = s.dir().join(format!("empty-{index}"));
            if daily {
                daily_state(s, &dir);
                fs::create_dir(dir.join("src")).unwrap();
            } else {
                s.attached_project(&dir);
            }
            let before = s
                .private(&dir)
                .git(["ls-files", "--stage", "-z"])
                .succeeds()
                .stdout;
            let (output, runs) = run_traced(
                s.git(["dupe", "add"].into_iter().chain(words))
                    .from(&dir.join(from)),
                &s.dir().join(format!("empty-{index}.trace")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            names(output.only_line("hint"), b"git dupe add <path>");
            assert_eq!(runs.of("add"), 0, "{runs:?}");
            assert_eq!(
                s.private(&dir)
                    .git(["ls-files", "--stage", "-z"])
                    .succeeds()
                    .stdout,
                before
            );
        }
    });
}

#[test]
fn directory_forms_never_hide_neighbours_and_limit_scope_below_a_hidden_path() {
    under_each_release(|s| {
        let dir = s.dir().join("scratch");
        daily_state(s, &dir);
        write(&dir, "scratch/x.md", b"new\n");
        s.git(["dupe", "add", "."]).from(&dir).succeeds();
        assert!(staged(s, &dir).is_empty());
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n.vscode\n");
        assert!(
            !region_rules(&dir)
                .iter()
                .any(|rule| holds(rule, b"scratch"))
        );
        assert!(!holds(
            &s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b"scratch"
        ));

        let dir = s.dir().join("subdirectory");
        daily_state(s, &dir);
        write(&dir, "notes/sub/x.md", b"new\n");
        write(&dir, "notes/today.md", b"new\n");
        s.git(["dupe", "add", "."])
            .from(&dir.join("notes/sub"))
            .succeeds();
        assert_eq!(staged(s, &dir), b"A\0notes/sub/x.md\0");
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n.vscode\n");
    });
}

#[test]
fn no_operand_with_all_clear_keeps_the_users_words_and_gits_status() {
    under_each_release(|s| {
        let dir = changes(s, "update");
        let output = s
            .git(["dupe", "add", "-u"])
            .from(&dir.join("docs"))
            .succeeds();
        no_own_hint(&output);
        assert_eq!(
            staged(s, &dir),
            b"M\0.env.local\0M\0.vscode/settings.json\0"
        );
        for (index, options) in [vec![], vec!["--no-all"], vec!["-A", "--no-all"]]
            .into_iter()
            .enumerate()
        {
            let dir = changes(s, &format!("unchanged-{index}"));
            let twin = changes(s, &format!("unchanged-twin-{index}"));
            let before = fs::read(dir.join(".git/dupe/index")).unwrap();
            let expected = s
                .private(&twin)
                .git(["add"].into_iter().chain(options.iter().copied()))
                .from(&twin.join("docs"))
                .run();
            let output = s
                .git(["dupe", "add"].into_iter().chain(options))
                .from(&dir.join("docs"))
                .run();
            assert_eq!(
                output.end, expected.end,
                "{output:?}; private: {expected:?}"
            );
            no_own_hint(&output);
            assert_eq!(fs::read(dir.join(".git/dupe/index")).unwrap(), before);
            assert!(staged(s, &dir).is_empty());
        }
    });
}

#[test]
fn last_all_flag_sets_scope_but_a_clear_flag_word_prevents_the_ignore_question() {
    under_each_release(|s| {
        let dir = changes(s, "last-all");
        let twin = changes(s, "last-all-twin");
        let expected = s
            .private(&twin)
            .git([
                "add",
                "--no-all",
                "-A",
                "--",
                ":(top,literal).env.local",
                ":(top,literal).gitdupe",
                ":(top,literal).vscode",
                ":(top,literal)docs/notes.md",
                ":(top,literal)notes",
            ])
            .from(&twin.join("docs"))
            .run();
        let (output, runs) = run_traced(
            s.git(["dupe", "add", "--no-all", "-A"])
                .from(&dir.join("docs")),
            &s.dir().join("last-all.trace"),
        );
        assert_eq!(
            output.end, expected.end,
            "{output:?}; private: {expected:?}"
        );
        assert_eq!(runs.of("add"), 1, "{runs:?}");
        // Settle still asks its public ignore question; there is no private one here.
        assert_eq!(runs.of("check-ignore"), 1, "{runs:?}");
        assert_eq!(staged(s, &dir), staged(s, &twin));
        assert!(holds(&staged(s, &dir), b"A\0notes/today.md\0"));
    });
}

#[test]
fn patch_forms_inherit_answers_and_stage_only_the_accepted_hunk() {
    under_each_release(|s| {
        let original = (1..=30)
            .map(|line| format!("line {line}\n"))
            .collect::<String>();
        let first = original.replace("line 2\n", "first change\n");
        let both = first.replace("line 29\n", "second change\n");
        for (index, options) in [vec!["-p"], vec!["-p", "notes/"]].into_iter().enumerate() {
            let dir = s.dir().join(format!("patch-{index}"));
            daily_state(s, &dir);
            write(&dir, "notes/a.md", original.as_bytes());
            private_add(s, &dir, "notes/a.md");
            private_commit(s, &dir);
            write(&dir, "notes/a.md", both.as_bytes());
            s.git(["dupe", "add"].into_iter().chain(options))
                .from(&dir)
                .input(b"y\nn\n")
                .succeeds();
            assert_eq!(
                s.private(&dir)
                    .git(["show", ":notes/a.md"])
                    .succeeds()
                    .stdout,
                first.as_bytes()
            );
            assert_eq!(fs::read(dir.join("notes/a.md")).unwrap(), both.as_bytes());
        }
    });
}

#[test]
fn files_tracked_below_a_directory_at_gitdupe_are_hidden_paths_the_forms_stage() {
    under_each_release(|s| {
        for (index, words) in [&["."][..], &["-A"], &[":/"]].into_iter().enumerate() {
            let dir = s.dir().join(format!("listing-directory-{index}"));
            s.attached_project(&dir);
            write(&dir, ".gitdupe/x", b"old\n");
            private_add(s, &dir, ".gitdupe/x");
            write(&dir, ".gitdupe/x", b"new\n");
            let (output, runs) = run_traced(
                s.git(["dupe", "add"].into_iter().chain(words.iter().copied()))
                    .from(&dir),
                &s.dir().join(format!("listing-directory-{index}.trace")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            no_own_hint(&output);
            assert_eq!(runs.of("add"), 1, "{runs:?}");
            assert_eq!(
                s.private(&dir)
                    .git(["show", ":.gitdupe/x"])
                    .succeeds()
                    .stdout,
                b"new\n"
            );
        }
    });
}
