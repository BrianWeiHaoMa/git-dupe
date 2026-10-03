//! The private status run: option words, confinement, and the boundaries before it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::harness::{
    End, Output, Runs, Scenario, holds, names, names_number, private_add, private_commit,
    region_rules, run_traced, under_each_release, write,
};

const SCOPE: [&str; 4] = [
    ":(top,literal).env.local",
    ":(top,literal).gitdupe",
    ":(top,literal).vscode",
    ":(top,literal)notes",
];

fn fixture(s: &Scenario) -> PathBuf {
    let dir = s.dir().join("workspace");
    s.attached_project(&dir);
    write(&dir, ".gitdupe", b"notes\n.vscode\n");
    for path in ["notes/a.md", ".env.local", ".vscode/settings.json"] {
        write(&dir, path, b"private\n");
        private_add(s, &dir, path);
    }
    private_add(s, &dir, ".gitdupe");
    private_commit(s, &dir);
    write(&dir, "notes/today.md", b"today\n");
    write(&dir, ".vscode/launch.json", b"launch\n");
    write(&dir, ".env.local", b"changed\n");
    dir
}

/// The specification's run, with the presence of user overrides supplied by the case.
fn private_status(
    s: &Scenario,
    dir: &Path,
    options: &[&str],
    untracked: bool,
    ignored: bool,
    scope: &[&str],
) -> Output {
    let mut words = vec!["status"];
    words.extend_from_slice(options);
    if !untracked {
        words.push("--untracked-files=normal");
    }
    if !ignored {
        words.push("--ignored");
    }
    words.push("--");
    words.extend_from_slice(scope);
    s.private(dir).git(words).run()
}

fn same_status(output: &Output, expected: &Output) {
    assert!(output.lines("error").is_empty(), "{output:?}");
    assert_eq!(
        output.end, expected.end,
        "{output:?}; private: {expected:?}"
    );
    assert_eq!(
        output.stdout, expected.stdout,
        "{output:?}; private: {expected:?}"
    );
}

#[test]
fn every_status_option_spelling_keeps_gits_answer() {
    under_each_release(|s| {
        let dir = fixture(s);
        // The booleans explicitly mark overrides rather than repeating the parser.
        let cases: &[(&[&str], bool, bool)] = &[
            (&[], false, false),
            (&["-s"], false, false),
            (&["--short"], false, false),
            (&["-b"], false, false),
            (&["--branch"], false, false),
            (&["-sb"], false, false),
            (&["--show-stash"], false, false),
            (&["--ahead-behind"], false, false),
            (&["--no-ahead-behind"], false, false),
            (&["--porcelain"], false, false),
            (&["--porcelain=v1"], false, false),
            (&["--porcelain=v2"], false, false),
            (&["--long"], false, false),
            (&["-z"], false, false),
            (&["--null"], false, false),
            (&["-v"], false, false),
            (&["--verbose"], false, false),
            (&["-vv"], false, false),
            (&["-u"], true, false),
            (&["-uno"], true, false),
            (&["-unormal"], true, false),
            (&["-uall"], true, false),
            (&["-su"], true, false),
            (&["--untracked-files"], true, false),
            (&["--untracked-files=no"], true, false),
            (&["--untracked-files=normal"], true, false),
            (&["--untracked-files=all"], true, false),
            (&["--ignored"], false, true),
            (&["--ignored=traditional"], false, true),
            (&["--ignored=matching"], false, true),
            (&["--ignored=no"], false, true),
            (&["--ignore-submodules"], false, false),
            (&["--ignore-submodules=all"], false, false),
            (&["--column"], false, false),
            (&["--column=always"], false, false),
            (&["--no-column"], false, false),
            (&["--renames"], false, false),
            (&["--no-renames"], false, false),
            (&["-M"], false, false),
            (&["-M50"], false, false),
            (&["--find-renames"], false, false),
            (&["--find-renames=50"], false, false),
            (&["-sb", "--show-stash", "--no-ahead-behind"], false, false),
            (&["--porcelain=v2", "-z", "-uall"], true, false),
            (&["-s", "--ignored=matching", "--no-column"], false, true),
            (&["--long", "-vv", "--renames", "-M50"], false, false),
            (&["-su", "--ignored=no"], true, true),
            (&["-uall", "-uno", "--ignored", "--ignored=no"], true, true),
        ];
        for &(options, untracked, ignored) in cases {
            let expected = private_status(s, &dir, options, untracked, ignored, &SCOPE);
            // Every listed release takes the spelling.
            assert_eq!(expected.end, End::Code(0), "{options:?}: {expected:?}");
            let output = s
                .git(
                    ["dupe", "status"]
                        .into_iter()
                        .chain(options.iter().copied()),
                )
                .from(&dir)
                .run();
            same_status(&output, &expected);
        }
    });
}

/// Under the hidden paths, untracked and ignored directories are collapsed as Git collapses
/// them, an ignored directory that holds a hidden path and no privately tracked file is
/// that one directory, and nothing a directory named `.git` holds is listed.
#[test]
fn directories_are_collapsed_as_git_collapses_them_and_no_git_directory_is_entered() {
    under_each_release(|s| {
        let dir = fixture(s);
        write(&dir, ".gitdupe", b"notes\n.vscode\nbuild/cache\n");
        write(
            &dir,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\ncache/\n",
        );
        for path in [
            "notes/cache/one",
            "notes/cache/deep/two",
            "notes/drafts/x.md",
            "notes/drafts/y.md",
            "notes/.git/stray",
            "build/cache/item",
            "build/out.js",
        ] {
            write(&dir, path, b"new\n");
        }
        s.git(["init", "-q", "notes/vendor"]).from(&dir).succeeds();
        write(&dir, "notes/vendor/lib.txt", b"vendored\n");
        let scope = [
            ":(top,literal).env.local",
            ":(top,literal).gitdupe",
            ":(top,literal).vscode",
            ":(top,literal)build/cache",
            ":(top,literal)notes",
        ];
        for (options, untracked, ignored) in [
            (vec!["--porcelain"], false, false),
            (vec!["--porcelain", "-uall"], true, false),
            (vec!["--porcelain", "--ignored=matching"], false, true),
            (vec![], false, false),
        ] {
            let expected = private_status(s, &dir, &options, untracked, ignored, &scope);
            let output = s
                .git(
                    ["dupe", "status"]
                        .into_iter()
                        .chain(options.iter().copied()),
                )
                .from(&dir)
                .run();
            same_status(&output, &expected);
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(!holds(&output.stdout, b"/.git/"), "{output:?}");
            assert!(!holds(&output.stdout, b"stray"), "{output:?}");
            if options == ["--porcelain"] {
                let records: Vec<&[u8]> = output.stdout.split(|&byte| byte == b'\n').collect();
                for record in [
                    &b"!! notes/cache/"[..],
                    b"?? notes/drafts/",
                    b"?? notes/vendor/",
                    b"!! build/",
                ] {
                    assert!(records.contains(&record), "{output:?}");
                }
            }
        }
    });
}

#[test]
fn ignored_and_untracked_modes_replace_the_defaults_and_keep_gits_refusal() {
    under_each_release(|s| {
        let dir = fixture(s);
        for (options, untracked, ignored) in [
            (vec!["--ignored=matching"], false, true),
            (vec!["--porcelain", "-uno"], true, false),
            (vec!["-uno", "--ignored=matching"], true, true),
        ] {
            let expected = private_status(s, &dir, &options, untracked, ignored, &SCOPE);
            let output = s
                .git(
                    ["dupe", "status"]
                        .into_iter()
                        .chain(options.iter().copied()),
                )
                .from(&dir)
                .run();
            same_status(&output, &expected);
            if options == ["--ignored=matching"] {
                assert_eq!(output.end, End::Code(0), "{output:?}");
            } else if options == ["--porcelain", "-uno"] {
                assert_eq!(output.end, End::Code(0), "{output:?}");
                assert!(!holds(&output.stdout, b"??"), "{output:?}");
                assert!(!holds(&output.stdout, b"!!"), "{output:?}");
            } else {
                assert_ne!(expected.end, End::Code(0), "{expected:?}");
                assert_eq!(output.stderr, expected.stderr, "{output:?}");
            }
        }
    });
}

#[test]
fn an_optional_value_in_the_next_word_is_a_path_and_leaves_an_empty_scope() {
    under_each_release(|s| {
        let dir = fixture(s);
        for option in ["-u", "--untracked-files"] {
            let expected = private_status(
                s,
                &dir,
                &["--porcelain", option],
                true,
                false,
                &[":(top,literal).git"],
            );
            let output = s
                .git(["dupe", "status", "--porcelain", option, "normal"])
                .from(&dir)
                .run();
            same_status(&output, &expected);
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    });
}

#[test]
fn caller_literal_pathspec_settings_and_public_hooks_keep_the_private_status() {
    under_each_release(|s| {
        let dir = fixture(s);
        let expected = private_status(s, &dir, &["--porcelain"], false, false, &SCOPE);
        let plain = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        same_status(&plain, &expected);
        assert!(!plain.stdout.is_empty(), "{plain:?}");
        for git in [
            s.git(["--literal-pathspecs", "dupe", "status", "--porcelain"]),
            s.git(["dupe", "status", "--porcelain"])
                .variable("GIT_LITERAL_PATHSPECS", "1"),
        ] {
            let output = git.from(&dir).run();
            same_status(&output, &plain);
        }
        let captured = s.dir().join("hook-status");
        let hook = dir.join(".git/hooks/pre-commit");
        fs::write(
            &hook,
            b"#!/bin/sh\ngit dupe status --porcelain > \"$STATUS_OUTPUT\"\n",
        )
        .unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        write(&dir, "README.md", b"public change\n");
        s.git(["add", "README.md"]).from(&dir).succeeds();
        let output = s
            .git([
                "-c",
                "maintenance.auto=false",
                "-c",
                "user.name=Scenario",
                "-c",
                "user.email=scenario@example.invalid",
                "commit",
                "-qm",
                "public change",
            ])
            .from(&dir)
            .variable("STATUS_OUTPUT", &captured)
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.lines("error").is_empty(), "{output:?}");
        assert_eq!(fs::read(captured).unwrap(), plain.stdout, "{output:?}");
    });
}

#[test]
fn a_reincluded_privately_tracked_file_warns_with_the_deciding_rule() {
    under_each_release(|s| {
        let dir = fixture(s);
        write(
            &dir,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\n!.env.local\n",
        );
        let output = s.git(["dupe", "status"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let warning = output.only_line("warning");
        names(warning, b".env.local");
        names(warning, b".gitignore");
        names_number(warning, 4);
    });
}

#[test]
fn exclusions_over_arg_max_are_refused_with_the_scope_count_and_still_settle() {
    under_each_release(|s| {
        let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
        assert!(limit.status.success());
        let limit: usize = std::str::from_utf8(&limit.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let count = limit / 128 + 1;
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"vendor\n");
        for index in 0..count {
            write(
                &dir,
                &format!("vendor/file-{index:016}-{}", "x".repeat(128)),
                b"public\n",
            );
        }
        s.git(["add", "-f", "--", "vendor"]).from(&dir).succeeds();
        let (output, runs) = run_traced(
            s.git(["dupe", "status"]).from(&dir),
            &s.dir().join("status-trace"),
        );
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        names_number(output.only_line("fatal"), count + 2);
        assert_eq!(runs.of("status"), 0, "{runs:?}");
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/vendor"]);
    });
}

#[test]
fn status_runs_once_and_the_git_run_count_does_not_grow_with_files() {
    under_each_release(|s| {
        let mut previous: Option<Runs> = None;
        for count in [5, 500] {
            let dir = s.dir().join(format!("workspace-{count}"));
            s.attached_project(&dir);
            write(&dir, ".gitdupe", b"notes\n");
            for index in 0..count {
                write(&dir, &format!("notes/{index}.md"), b"private\n");
            }
            private_add(s, &dir, "notes");
            private_add(s, &dir, ".gitdupe");
            private_commit(s, &dir);
            let (output, runs) = run_traced(
                s.git(["dupe", "status"]).from(&dir),
                &s.dir().join(format!("trace-{count}")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert_eq!(runs.of("status"), 1, "{runs:?}");
            if let Some(before) = &previous {
                assert_eq!(runs.count(), before.count(), "{runs:?}");
                assert_eq!(runs.commands(), before.commands(), "{runs:?}");
            }
            previous = Some(runs);
        }
    });
}

#[test]
fn an_unreadable_private_listing_ends_before_status_and_keeps_the_region() {
    under_each_release(|s| {
        let dir = fixture(s);
        let settled = s.git(["dupe", "status"]).from(&dir).run();
        assert_eq!(settled.end, End::Code(0), "{settled:?}");
        let exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
        fs::rename(dir.join(".git/dupe"), dir.join("saved-private")).unwrap();
        fs::create_dir(dir.join(".git/dupe")).unwrap();
        let expected = s.private(&dir).git(["ls-files", "-z", "--full-name"]).run();
        assert_ne!(expected.end, End::Code(0), "{expected:?}");
        let (output, runs) = run_traced(
            s.git(["dupe", "status"]).from(&dir),
            &s.dir().join("status-trace"),
        );
        assert_eq!(
            output.end, expected.end,
            "{output:?}; private: {expected:?}"
        );
        assert_eq!(runs.of("status"), 0, "{runs:?}");
        assert!(
            holds(&output.stderr, &expected.stderr),
            "{output:?}; private: {expected:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        names(warnings[0], b"git dupe init");
        assert_eq!(fs::read(dir.join(".git/info/exclude")).unwrap(), exclude);
    });
}
