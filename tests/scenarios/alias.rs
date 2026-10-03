//! A first word reached through an alias goes on as the command its chain reaches:
//! its reading, its table, its guard, or its refusal, in an attached workspace, an
//! unattached one, and outside any; and `stage` is `add`. An alias that reaches `detach`
//! is `detach`, which never settles.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Runs, Scenario, Tree, copy, daily_state, general_text, holds, names, region_rules,
    run_traced, unchanged, under_each_release, usage_line, warnings_in_any_order, write,
};

fn aliases(s: &Scenario, dir: &Path, private: bool, entries: &[(&str, &str)]) {
    for (word, expansion) in entries {
        let key = format!("alias.{word}");
        if private {
            // A conflicting global value makes the private source observable.
            s.git(["config", "--global", &key, "help"]).succeeds();
            s.private(dir).git(["config", &key, expansion]).succeeds();
        } else {
            s.git(["config", "--global", &key, expansion]).succeeds();
        }
    }
}

fn settled(runs: &Runs) {
    assert!(
        runs.commands()
            .ends_with(&[b"ls-files".as_slice(), b"ls-files", b"check-ignore"]),
        "{runs:?}"
    );
}

fn same_command(s: &Scenario, dir: &Path, typed: &[&str], literal: &[&str]) -> Output {
    let expected = s
        .git(["dupe"].into_iter().chain(literal.iter().copied()))
        .from(dir)
        .run();
    let output = s
        .git(["dupe"].into_iter().chain(typed.iter().copied()))
        .from(dir)
        .run();
    assert_eq!(
        output, expected,
        "alias words: {typed:?}, literal: {literal:?}"
    );
    output
}

#[test]
fn status_alias_usage_and_help_are_read_in_every_place() {
    under_each_release(|s| {
        let help = s.git(["dupe", "status", "-h"]).succeeds();
        for private in [false, true] {
            let attached = s.dir().join(format!("attached-{private}"));
            s.attached_project(&attached);
            aliases(
                s,
                &attached,
                private,
                &[("sp", "status --porc"), ("sh", "status -h")],
            );
            write(&attached, ".gitdupe", b"notes\n");
            let unattached = s.dir().join(format!("unattached-{private}"));
            s.repository(&unattached);
            let places = if private {
                vec![attached.as_path()]
            } else {
                vec![attached.as_path(), unattached.as_path(), s.dir()]
            };
            for dir in places {
                let before = Tree::of(dir);
                let expected = s.git(["dupe", "status", "--porc"]).from(dir).run();
                let (output, runs) = run_traced(
                    s.git(["dupe", "sp"]).from(dir),
                    &s.dir().join("status-trace"),
                );
                assert_eq!(output, expected);
                assert_eq!(output.end, End::Code(129), "{output:?}");
                assert!(output.stdout.is_empty(), "{output:?}");
                let error = output.line_then("error", usage_line(&help.stdout));
                names(error, b"--porc");
                names(error, b"git dupe git");
                assert_eq!(runs.of("status"), 0, "{runs:?}");
                assert_eq!(runs.of("ls-files"), 0, "{runs:?}");
                assert_eq!(runs.of("check-ignore"), 0, "{runs:?}");
                assert_eq!(s.git(["dupe", "sh"]).from(dir).run(), help);
                // The trace is outside repositories, but within the outside fixture.
                let after = Tree::of(dir).without(&[&s.dir().join("status-trace")]);
                let before = before.without(&[&s.dir().join("status-trace")]);
                assert!(before.changed_in(&after).is_empty());
            }
            assert_eq!(region_rules(&attached), [b"/.gitdupe"]);
        }
    });
}

fn edited(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    daily_state(s, &dir);
    s.git(["dupe", "status"]).from(&dir).succeeds();
    for (path, bytes) in [
        ("notes/a.md", b"changed\n".as_slice()),
        ("notes/new.md", b"new\n"),
        (".env.local", b"changed\n"),
        ("docs/notes.md", b"changed\n"),
        ("README.md", b"public change\n"),
        ("public-new", b"public new\n"),
    ] {
        write(&dir, path, bytes);
    }
    dir
}

fn staged(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.private(dir)
        .git(["diff", "--cached", "--name-only", "-z"])
        .succeeds()
        .stdout
}

fn same_staging(s: &Scenario, actual: &Path, expected: &Path, words: &[&str]) {
    let public = fs::read(actual.join(".git/index")).unwrap();
    let output = s
        .git(["dupe"].into_iter().chain(words.iter().copied()))
        .from(actual)
        .run();
    let literal = s.git(["dupe", "add", "-A"]).from(expected).run();
    assert_eq!(output, literal);
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert_eq!(staged(s, actual), staged(s, expected));
    assert_eq!(
        staged(s, actual),
        b".env.local\0docs/notes.md\0notes/a.md\0notes/new.md\0"
    );
    assert_eq!(
        s.private(actual)
            .git(["ls-files", "--stage", "-z"])
            .succeeds(),
        s.private(expected)
            .git(["ls-files", "--stage", "-z"])
            .succeeds()
    );
    assert_eq!(fs::read(actual.join(".git/index")).unwrap(), public);
    assert!(!holds(
        &fs::read(actual.join(".gitdupe")).unwrap(),
        b"public-new"
    ));
}

#[test]
fn add_alias_stages_only_hidden_paths_and_requires_attachment() {
    under_each_release(|s| {
        for private in [false, true] {
            let actual = edited(s, &format!("actual-{private}"));
            let expected = edited(s, &format!("expected-{private}"));
            aliases(s, &actual, private, &[("aa", "add -A")]);
            same_staging(s, &actual, &expected, &["aa"]);
        }
        let unattached = s.dir().join("unattached");
        s.repository(&unattached);
        aliases(s, &unattached, false, &[("aa", "add -A")]);
        let before = Tree::of(&unattached);
        let output = same_command(s, &unattached, &["aa"], &["add", "-A"]);
        assert_eq!(output.end, End::Code(128), "{output:?}");
        names(output.only_line("fatal"), b"git dupe init");
        unchanged(&before, &unattached);
        same_command(s, s.dir(), &["aa"], &["add", "-A"]);
    });
}

#[test]
fn stash_aliases_keep_the_untracked_guard_and_settle() {
    under_each_release(|s| {
        for private in [false, true] {
            let dir = edited(s, &format!("workspace-{private}"));
            aliases(
                s,
                &dir,
                private,
                &[("su", "stash -u"), ("s", "stash"), ("sl", "stash list")],
            );
            // Both refusals must settle this newly listed directory.
            write(&dir, ".gitdupe", b"notes\n.vscode\nscratch\n");
            write(&dir, "scratch/keep", b"keep\n");
            for typed in [&["su"][..], &["s", "-u"]] {
                let exclude = dir.join(".git/info/exclude");
                let starting_region = fs::read(&exclude).unwrap();
                let expected = s.git(["dupe", "stash", "-u"]).from(&dir).run();
                fs::write(&exclude, starting_region).unwrap();
                let before = Tree::of(&dir).without(&[&exclude]);
                let (output, runs) = run_traced(
                    s.git(["dupe"].into_iter().chain(typed.iter().copied()))
                        .from(&dir),
                    &s.dir().join("stash-trace"),
                );
                assert_eq!(output, expected);
                assert_eq!(output.end, End::Code(128), "{output:?}");
                assert!(output.stdout.is_empty());
                let line = output.only_line("fatal");
                names(line, b"git dupe add");
                names(line, b"git dupe stash");
                let line = String::from_utf8_lossy(line);
                assert!(line.find("git dupe add") < line.find("git dupe stash"));
                assert_eq!(runs.of("stash"), 0, "{runs:?}");
                settled(&runs);
                assert!(
                    before
                        .changed_in(&Tree::of(&dir).without(&[&exclude]))
                        .is_empty()
                );
                assert_eq!(fs::read(dir.join("notes/new.md")).unwrap(), b"new\n");
                assert!(
                    s.private(&dir)
                        .git(["stash", "list"])
                        .succeeds()
                        .stdout
                        .is_empty()
                );
                assert!(region_rules(&dir).contains(&b"/scratch".to_vec()));
            }
            same_command(s, &dir, &["sl"], &["stash", "list"]);
        }
    });
}

#[test]
fn clean_alias_is_clean_with_its_help_and_settles() {
    under_each_release(|s| {
        for private in [false, true] {
            let dir = edited(s, &format!("workspace-{private}"));
            aliases(s, &dir, private, &[("wipe", "clean -fdx")]);
            for after in [&[][..], &["-h"]] {
                let literal_dir = s.dir().join(format!("literal-{private}-{}", after.len()));
                copy(&dir, &literal_dir);
                let (output, runs) = run_traced(
                    s.git(["dupe", "wipe"].into_iter().chain(after.iter().copied()))
                        .from(&dir),
                    &s.dir().join("clean-trace"),
                );
                let literal = s
                    .git(
                        ["dupe", "clean", "-fdx"]
                            .into_iter()
                            .chain(after.iter().copied()),
                    )
                    .from(&literal_dir)
                    .run();
                assert_eq!(output, literal);
                assert_eq!(output.end, End::Code(0), "{output:?}");
                // What stands afterward is what the literal command leaves.
                let left = |root: &Path| -> Vec<PathBuf> {
                    let git = root.join(".git");
                    Tree::of(root)
                        .paths()
                        .filter(|path| !path.starts_with(&git))
                        .map(|path| path.strip_prefix(root).unwrap().to_path_buf())
                        .collect()
                };
                assert_eq!(left(&dir), left(&literal_dir), "{after:?}");
                if after.is_empty() {
                    assert_eq!(runs.own().of("clean"), 1, "{runs:?}");
                    assert!(!dir.join("public-new").exists());
                    assert!(dir.join("notes/new.md").exists());
                    settled(&runs);
                } else {
                    // The text, answered after the alias is resolved: nothing runs, and
                    // nothing settles.
                    for command in ["clean", "ls-files", "check-ignore"] {
                        assert_eq!(runs.of(command), 0, "{runs:?}");
                    }
                }
            }
        }
    });
}

#[test]
fn git_aliases_reach_the_unguarded_route_and_its_usage() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "git"]).succeeds();
        for private in [false, true] {
            let dir = edited(s, &format!("workspace-{private}"));
            aliases(
                s,
                &dir,
                private,
                &[
                    ("g", "git status --porc"),
                    ("g2", "git"),
                    ("g3", "git -c a=b status"),
                ],
            );
            let output = same_command(s, &dir, &["g"], &["git", "status", "--porc"]);
            assert_eq!(
                output,
                s.private(&dir)
                    .git(["-c", "help.autocorrect=0", "status", "--porc"])
                    .run()
            );
            let (output, runs) =
                run_traced(s.git(["dupe", "g2"]).from(&dir), &s.dir().join("git-trace"));
            assert_eq!(output, s.git(["dupe", "git"]).from(&dir).run());
            assert_eq!(output.end, End::Code(129));
            output.line_then("error", usage_line(&help.stdout));
            assert_eq!(runs.of("rev-parse"), 1, "{runs:?}");
            assert_eq!(runs.of("ls-files"), 0, "{runs:?}");
            let output = same_command(s, &dir, &["g3"], &["git", "-c", "a=b", "status"]);
            assert_eq!(output.end, End::Code(129));
        }
        aliases(
            s,
            s.dir(),
            false,
            &[("g2", "git"), ("g3", "git -c a=b status")],
        );
        same_command(s, s.dir(), &["g2"], &["git"]);
        same_command(s, s.dir(), &["g3"], &["git", "-c", "a=b", "status"]);
    });
}

#[test]
fn init_alias_attaches_and_refuses_a_submodule_and_help_works_outside() {
    under_each_release(|s| {
        aliases(s, s.dir(), false, &[("i", "init"), ("h", "help")]);
        let actual = s.dir().join("actual");
        let expected = s.dir().join("expected");
        s.repository(&actual);
        s.repository(&expected);
        let output = s.git(["dupe", "i"]).from(&actual).run();
        let literal = s.git(["dupe", "init"]).from(&expected).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(output.end, literal.end);
        assert_eq!(output.stderr, literal.stderr);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(literal.stdout)
                .unwrap()
                .replace(expected.to_str().unwrap(), actual.to_str().unwrap())
        );
        assert_eq!(region_rules(&actual), region_rules(&expected));
        assert_eq!(
            actual.join(".gitdupe").exists(),
            expected.join(".gitdupe").exists()
        );
        assert_eq!(
            s.private(&actual)
                .git(["config", "--local", "--list"])
                .succeeds(),
            s.private(&expected)
                .git(["config", "--local", "--list"])
                .succeeds()
        );
        aliases(s, &actual, true, &[("i", "init"), ("h", "help")]);
        same_command(s, &actual, &["i"], &["init"]);
        assert_eq!(
            s.git(["dupe", "h"]).from(&actual).run().stdout,
            general_text(s)
        );
        aliases(s, s.dir(), false, &[("i", "init"), ("h", "help")]);
        let public = s.dir().join("public");
        let superproject = s.dir().join("superproject");
        s.repository(&public);
        s.repository(&superproject);
        s.git([
            OsStr::new("-c"),
            OsStr::new("protocol.file.allow=always"),
            OsStr::new("submodule"),
            OsStr::new("add"),
            public.as_os_str(),
            OsStr::new("sub"),
        ])
        .from(&superproject)
        .succeeds();
        let submodule = superproject.join("sub");
        let before = Tree::of(&superproject);
        let output = same_command(s, &submodule, &["i"], &["init"]);
        assert_eq!(output.end, End::Code(128));
        output.only_line("fatal");
        unchanged(&before, &superproject);
        let output = s.git(["dupe", "h"]).run();
        assert_eq!(output.end, End::Code(0));
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, general_text(s));
    });
}

#[test]
fn alias_chain_places_inner_words_before_outer_and_typed_words() {
    under_each_release(|s| {
        for private in [false, true] {
            let dir = s.dir().join(format!("workspace-{private}"));
            s.attached_project(&dir);
            aliases(
                s,
                &dir,
                private,
                &[("a", "b two"), ("b", "git rev-parse --sq-quote one")],
            );
            let output = same_command(
                s,
                &dir,
                &["a", "three"],
                &["git", "rev-parse", "--sq-quote", "one", "two", "three"],
            );
            assert_eq!(
                output,
                s.private(&dir)
                    .git([
                        "-c",
                        "help.autocorrect=0",
                        "rev-parse",
                        "--sq-quote",
                        "one",
                        "two",
                        "three"
                    ])
                    .run()
            );
        }
    });
}

#[test]
fn stage_is_add_in_dispatch_and_help_but_git_stage_is_unguarded() {
    under_each_release(|s| {
        let help = s.git(["dupe", "add", "-h"]).succeeds();
        assert_eq!(s.git(["dupe", "stage", "-h"]).run(), help);
        assert_eq!(s.git(["dupe", "help", "stage"]).run(), help);
        for private in [false, true] {
            let actual = edited(s, &format!("actual-{private}"));
            let expected = edited(s, &format!("expected-{private}"));
            let public = fs::read(actual.join(".git/index")).unwrap();
            let before = Tree::of(&actual);
            let output = same_command(s, &actual, &["stage", "README.md"], &["add", "README.md"]);
            assert_eq!(output.end, End::Code(128));
            names(output.only_line("fatal"), b"README.md");
            unchanged(&before, &actual);
            aliases(s, &actual, private, &[("st", "stage notes/new.md")]);
            let output = s.git(["dupe", "st"]).from(&actual).run();
            assert_eq!(
                output,
                s.git(["dupe", "add", "notes/new.md"]).from(&expected).run()
            );
            assert_eq!(output.end, End::Code(0));
            assert_eq!(staged(s, &actual), b"notes/new.md\0");
            assert_eq!(staged(s, &actual), staged(s, &expected));
            same_staging(s, &actual, &expected, &["stage", "-A"]);
            let literal = s
                .private(&expected)
                .git(["-c", "help.autocorrect=0", "stage", "README.md"])
                .run();
            let output = s
                .git(["dupe", "git", "stage", "README.md"])
                .from(&actual)
                .run();
            assert_eq!(output.end, literal.end);
            assert_eq!(output.stdout, literal.stdout);
            assert!(output.stderr.starts_with(&literal.stderr));
            warnings_in_any_order(&output, &[&[b"README.md", b"public", b"private"]]);
            assert_eq!(
                s.private(&actual)
                    .git(["ls-files", "--stage", "-z"])
                    .succeeds(),
                s.private(&expected)
                    .git(["ls-files", "--stage", "-z"])
                    .succeeds()
            );
            assert!(holds(&staged(s, &actual), b"README.md\0"));
            assert_eq!(fs::read(actual.join(".git/index")).unwrap(), public);
        }
    });
}

#[test]
fn an_init_alias_whose_second_locate_fails_still_settles() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        s.private(&dir)
            .git(["config", "alias.i", "-c core.bare=INVALID init"])
            .succeeds();
        let gits = s
            .git(["-c", "core.bare=INVALID", "rev-parse", "--show-toplevel"])
            .from(&dir)
            .run();
        let output = s.git(["dupe", "i"]).from(&dir).run();
        assert_eq!(output.end, gits.end, "{output:?}, Git: {gits:?}");
        assert_ne!(output.end, End::Code(0), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        // Settle ran, its runs carrying the same prefix, and could not read the private
        // repository through it.
        let warnings = output.lines("warning");
        assert_eq!(warnings.len(), 1, "{output:?}");
        names(warnings[0], b"git dupe init");
    });
}

#[test]
fn detach_aliases_detach_as_the_typed_words_do_and_never_settle() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        daily_state(s, &dir);
        aliases(
            s,
            &dir,
            true,
            &[
                ("leave", "detach --force"),
                ("go", "leave"),
                ("drop", "detach x"),
            ],
        );
        // A region gone stale since the last command: a settle would rewrite it.
        write(&dir, ".gitdupe", b"notes\n.vscode\nextra\n");
        let trace = s.dir().join("detach-trace");

        let before = Tree::of(&dir);
        let (misused, runs) = run_traced(s.git(["dupe", "drop"]).from(&dir), &trace);
        assert_eq!(misused.end, End::Code(129), "{misused:?}");
        assert!(misused.stdout.is_empty(), "{misused:?}");
        let usage = misused.line_then("error", b"usage: git dupe detach [--force]\n");
        assert!(!usage.is_empty(), "{misused:?}");
        unchanged(&before, &dir);
        for command in ["ls-files", "check-ignore", "status"] {
            assert_eq!(runs.of(command), 0, "{runs:?}");
        }

        let literal = s.dir().join("literal");
        copy(&dir, &literal);
        let (output, runs) = run_traced(s.git(["dupe", "go"]).from(&dir), &trace);
        let expected = s.git(["dupe", "detach", "--force"]).from(&literal).run();
        assert_eq!(output, expected);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(!dir.join(".git/dupe").exists());
        assert!(!holds(
            &fs::read(dir.join(".git/info/exclude")).unwrap(),
            b"# BEGIN git-dupe"
        ));
        // `--force` decides nothing and nothing settles: the private listing, then
        // `Remove`'s public listing and its question (`Holds/G22`).
        assert_eq!(runs.own().of("status"), 0, "{runs:?}");
        assert_eq!(runs.own().of("ls-files"), 2, "{runs:?}");
        assert!(
            runs.own().commands().ends_with(&[
                b"ls-files".as_slice(),
                b"ls-files",
                b"check-ignore"
            ]),
            "{runs:?}"
        );
    });
}
