//! `git dupe hide`: literal paths, private staging, refusals, and the following settle.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Tree, gitdupe_written_and_staged, holds, lines_in_order, names, names_number,
    names_the_route_to_private, private_add, region_rules, unchanged, under_each_release,
    usage_line,
};

/// The hints, in order, each naming its path and `git dupe unhide`.
fn hinted(output: &Output, paths: &[&[u8]]) {
    lines_in_order(output, "hint", paths);
    for hint in output.lines("hint") {
        names(hint, b"git dupe unhide");
    }
}

/// Exit 0, nothing on standard output, these hints and then these warnings in order, and
/// no `fatal:` or `error:` line.
fn succeeded_naming(output: &Output, hints: &[&[u8]], warnings: &[&[u8]]) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    hinted(output, hints);
    lines_in_order(output, "warning", warnings);
    lines_in_order(output, "fatal", &[]);
    lines_in_order(output, "error", &[]);
}

/// Exit 128, nothing on standard output, one `fatal:` line naming `count` and
/// `git rm --cached`, no hint, and then settle's warnings in order.
fn refused_then_warnings(output: &Output, count: &[u8], warnings: &[&[u8]]) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    lines_in_order(output, "fatal", &[count]);
    names(output.lines("fatal")[0], b"git rm --cached");
    // Public ownership of `.gitdupe` has its own refusal, outside this route.
    if count != b"git rm --cached .gitdupe" {
        let fatal = output.lines("fatal")[0];
        names(fatal, b"one file at a time");
        names_the_route_to_private(fatal);
    }
    hinted(output, &[]);
    lines_in_order(output, "warning", warnings);
}

#[test]
fn absent_and_present_paths_are_hidden_once_and_new_children_are_ignored() {
    under_each_release(|s| {
        for present in [false, true] {
            let dir = s.dir().join(format!("notes-{present}"));
            s.project(&dir);
            s.init(&dir);
            if present {
                fs::create_dir(dir.join("notes")).unwrap();
                fs::write(dir.join("notes/today"), b"private\n").unwrap();
            }
            let output = s.git(["dupe", "hide", "notes/"]).from(&dir).run();
            succeeded_naming(&output, &[b"notes"], &[]);
            // The one hint also names what versions the path, and says when nothing
            // stands there yet, as a mistyped path would show.
            let hint = output.lines("hint")[0];
            names(
                hint,
                b"run from the root, 'git dupe add -- notes' versions it",
            );
            names(hint, b"'git dupe unhide -- notes' stops hiding it");
            assert_eq!(
                holds(hint, b"nothing stands there yet"),
                !present,
                "{output:?}"
            );
            gitdupe_written_and_staged(s, &dir, b"notes\n");
            assert_eq!(
                s.private(&dir).git(["ls-files", "-z"]).run().stdout,
                b".gitdupe\0"
            );
            assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
            assert!(
                s.git(["status", "--porcelain"])
                    .from(&dir)
                    .succeeds()
                    .stdout
                    .is_empty()
            );
            fs::create_dir_all(dir.join("notes")).unwrap();
            fs::write(dir.join("notes/x"), b"new\n").unwrap();
            assert_eq!(
                s.git([
                    "status",
                    "--porcelain",
                    "--ignored",
                    "--untracked-files=all",
                    "--",
                    "notes/x"
                ])
                .from(&dir)
                .succeeds()
                .stdout,
                b"!! notes/x\n"
            );
            for path in ["notes", "notes/sub"] {
                let output = s.git(["dupe", "hide", path]).from(&dir).run();
                succeeded_naming(&output, &[], &[]);
                gitdupe_written_and_staged(s, &dir, b"notes\n");
            }
            let output = s.git(["dupe", "hide", "a", "b"]).from(&dir).run();
            succeeded_naming(&output, &[b"a", b"b"], &[]);
            gitdupe_written_and_staged(s, &dir, b"notes\na\nb\n");
            // The command the hint names does what it says, typed at the root.
            s.git(["dupe", "add", "--", "notes"]).from(&dir).succeeds();
            let tracked = s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout;
            assert!(holds(&tracked, b"notes/x\0"), "{tracked:?}");
        }
    });
}

#[test]
fn appending_preserves_existing_bytes_and_reads_the_staged_source() {
    under_each_release(|s| {
        for (index, (before, after)) in [
            (
                b"notes/\n\n/docs/x/\n".as_slice(),
                b"notes/\n\n/docs/x/\nscratch\n".as_slice(),
            ),
            (b"notes", b"notes\nscratch\n"),
        ]
        .into_iter()
        .enumerate()
        {
            let dir = s.dir().join(format!("append-{index}"));
            s.project(&dir);
            s.init(&dir);
            fs::write(dir.join(".gitdupe"), before).unwrap();
            let output = s.git(["dupe", "hide", "scratch"]).from(&dir).run();
            succeeded_naming(&output, &[b"scratch"], &[]);
            gitdupe_written_and_staged(s, &dir, after);
            fs::remove_file(dir.join(".gitdupe")).unwrap();
            let output = s.git(["dupe", "hide", "more"]).from(&dir).run();
            succeeded_naming(&output, &[b"more"], &[]);
            gitdupe_written_and_staged(s, &dir, &[after, b"more\n"].concat());
        }
    });
}

#[test]
fn operands_are_cleaned_lexically_from_the_callers_directory() {
    under_each_release(|s| {
        let dir = s.dir().join("cleaned");
        s.project(&dir);
        s.init(&dir);
        let from = dir.join("sub");
        for (operand, path) in [("./a//b/", b"sub/a/b".as_slice()), ("../x", b"x")] {
            let output = s.git(["dupe", "hide", operand]).from(&from).run();
            succeeded_naming(&output, &[path], &[]);
            // The hint names the root-relative path, and so the root as where to run the
            // commands it names with it.
            let add = format!(
                "run from the root, 'git dupe add -- {}'",
                path.escape_ascii()
            );
            names(output.lines("hint")[0], add.as_bytes());
            assert!(
                !output.lines("hint")[0]
                    .windows(operand.len())
                    .any(|part| part == operand.as_bytes()),
                "{output:?}"
            );
        }
        let root = s
            .git(["rev-parse", "--show-toplevel"])
            .from(&dir)
            .succeeds()
            .stdout;
        let root = Path::new(OsStr::from_bytes(root.trim_ascii_end()));
        let absolute = root.join("sub/y");
        let output = s
            .git([OsStr::new("dupe"), OsStr::new("hide"), absolute.as_os_str()])
            .from(&from)
            .run();
        succeeded_naming(&output, &[b"sub/y"], &[]);
        gitdupe_written_and_staged(s, &dir, b"sub/a/b\nx\nsub/y\n");
        let help = s.git(["dupe", "hide", "-h"]).run();
        for operand in [PathBuf::from(".."), root.parent().unwrap().join("outside")] {
            let before = Tree::of(&dir);
            let output = s
                .git([OsStr::new("dupe"), OsStr::new("hide"), operand.as_os_str()])
                .from(&from)
                .run();
            assert_eq!(output.end, End::Code(129), "{output:?}");
            output.line_then("error", usage_line(&help.stdout));
            lines_in_order(&output, "warning", &[]);
            unchanged(&before, &dir);
        }
        // No filesystem traversal: a symlink's parent is removed lexically.
        symlink(s.dir(), dir.join("sub/link")).unwrap();
        let output = s.git(["dupe", "hide", "link/../z"]).from(&from).run();
        succeeded_naming(&output, &[b"sub/z"], &[]);
        gitdupe_written_and_staged(s, &dir, b"sub/a/b\nx\nsub/y\nsub/z\n");
    });
}

#[test]
fn usage_waits_for_location_and_attachment_but_help_does_not() {
    under_each_release(|s| {
        let help = s.git(["dupe", "hide", "-h"]).run();
        assert_eq!(help.end, End::Code(0), "{help:?}");
        assert!(help.stderr.is_empty());
        let misuse: &[&[&str]] = &[
            &[],
            &["-f", "x"],
            &["a*"],
            &["a?"],
            &["a[1]"],
            &[":/x"],
            &[":x"],
            &["."],
            &["/"],
            &["../x"],
        ];
        let locate = s
            .git([
                "rev-parse",
                "--show-toplevel",
                "--show-prefix",
                "--absolute-git-dir",
            ])
            .run();
        assert_eq!(locate.end, End::Code(128), "{locate:?}");
        for attached in [false, true] {
            let dir = s.dir().join(format!("usage-{attached}"));
            s.project(&dir);
            if attached {
                s.init(&dir);
            }
            fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
            fs::write(dir.join(".gitignore"), b"!notes\n").unwrap();
            let before = Tree::of(&dir);
            for words in misuse {
                let output = s
                    .git(["dupe", "hide"].into_iter().chain(words.iter().copied()))
                    .from(&dir)
                    .run();
                assert!(output.stdout.is_empty(), "{output:?}");
                if attached {
                    assert_eq!(output.end, End::Code(129), "{output:?}");
                    output.line_then("error", usage_line(&help.stdout));
                } else {
                    assert_eq!(output.end, End::Code(128), "{output:?}");
                    names(output.only_line("fatal"), b"git dupe init");
                }
                lines_in_order(&output, "warning", &[]);
                unchanged(&before, &dir);
                let outside = s
                    .git(["dupe", "hide"].into_iter().chain(words.iter().copied()))
                    .run();
                assert_eq!(outside.end, locate.end, "{outside:?}");
                assert_eq!(outside.stderr, locate.stderr);
                assert!(outside.stdout.is_empty());
            }
            for place in [&dir, s.dir()] {
                for words in [&["-h"][..], &["--help"], &["x", "-h"], &["-f", "x", "-h"]] {
                    let output = s
                        .git(["dupe", "hide"].into_iter().chain(words.iter().copied()))
                        .from(place)
                        .run();
                    assert_eq!(output, help);
                    unchanged(&before, &dir);
                }
            }
        }
        let dir = s.dir().join("dashed");
        s.project(&dir);
        s.init(&dir);
        let output = s.git(["dupe", "hide", "--", "-x"]).from(&dir).run();
        succeeded_naming(&output, &[b"-x"], &[]);
        gitdupe_written_and_staged(s, &dir, b"-x\n");
    });
}

#[test]
fn public_tracking_refuses_before_writing_and_still_settles() {
    under_each_release(|s| {
        let dir = s.dir().join("tracked");
        s.project(&dir);
        s.init(&dir);
        fs::create_dir(dir.join("conf")).unwrap();
        fs::write(dir.join("conf/local.ini"), b"both\n").unwrap();
        s.git(["add", "-f", "conf/local.ini"]).from(&dir).succeeds();
        private_add(s, &dir, "conf/local.ini");
        for (path, count) in [
            ("README.md", b"1".as_slice()),
            ("docs/", b"2"),
            ("conf/", b"1"),
        ] {
            let exclude = dir.join(".git/info/exclude");
            let before = Tree::of(&dir).without(&[&exclude]);
            let output = s.git(["dupe", "hide", path]).from(&dir).run();
            refused_then_warnings(&output, count, &[b"conf/local.ini"]);
            names(
                output.lines("fatal")[0],
                path.trim_end_matches('/').as_bytes(),
            );
            // One file is named as the file the project's Git tracks; a directory as
            // holding the count it does.
            let file = path == "README.md";
            let wording: &[u8] = if file {
                b"README.md is tracked by the project's Git"
            } else {
                b"at or below"
            };
            names(output.lines("fatal")[0], wording);
            names(output.lines("fatal")[0], b"one file at a time");
            names_number(
                output.lines("fatal")[0],
                std::str::from_utf8(count).unwrap().parse().unwrap(),
            );
            let changed = before.changed_in(&Tree::of(&dir).without(&[&exclude]));
            assert!(changed.is_empty(), "changed: {changed:?}");
            assert!(!dir.join(".gitdupe").exists());
            assert_eq!(
                region_rules(&dir),
                [b"/.gitdupe".as_slice(), b"/conf/local.ini"]
            );
        }
        let before = Tree::of(&dir).without(&[&dir.join(".git/info/exclude")]);
        let output = s
            .git(["dupe", "hide", "README.md", "docs/"])
            .from(&dir)
            .run();
        refused_then_warnings(&output, b"3", &[b"conf/local.ini"]);
        names(
            output.lines("fatal")[0],
            b"3 paths the project's Git tracks lie at or below README.md, docs",
        );
        names(output.lines("fatal")[0], b"one file at a time");
        let after = Tree::of(&dir).without(&[&dir.join(".git/info/exclude")]);
        assert!(before.changed_in(&after).is_empty());
        let dir = s.dir().join("gitdupe-tracked");
        s.project(&dir);
        s.init(&dir);
        fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
        s.git(["add", "-f", ".gitdupe"]).from(&dir).succeeds();
        let index = Tree::of(&dir.join(".git/dupe"));
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        // Public Git tracks `.gitdupe`, so it does not ignore it (G6, G7).
        refused_then_warnings(&output, b"git rm --cached .gitdupe", &[b".gitdupe"]);
        unchanged(&index, &dir.join(".git/dupe"));
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"notes\n");
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
        private_add(s, &dir, ".gitdupe");
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        succeeded_naming(&output, &[b"x"], &[b".gitdupe"]);
        gitdupe_written_and_staged(s, &dir, b"notes\nx\n");
        let before = Tree::of(&dir);
        let output = s.git(["dupe", "hide", "docs/"]).from(&dir).run();
        refused_then_warnings(&output, b"2", &[b".gitdupe"]);
        names(output.lines("fatal")[0], b"docs");
        names(output.lines("fatal")[0], b"one file at a time");
        names_number(output.lines("fatal")[0], 2);
        unchanged(&before, &dir);
    });
}

/// A file the private repository tracks is hidden already: listing it names it and
/// `git dupe unhide`, and neither calls it newly hidden nor sends the developer to
/// `git dupe add` what is tracked.
#[test]
fn a_privately_tracked_file_is_listed_without_being_called_newly_hidden() {
    under_each_release(|s| {
        let dir = s.dir().join("tracked");
        s.project(&dir);
        s.init(&dir);
        fs::write(dir.join(".env"), b"secret\n").unwrap();
        private_add(s, &dir, ".env");
        let output = s.git(["dupe", "hide", ".env"]).from(&dir).run();
        succeeded_naming(&output, &[b".env"], &[]);
        let hint = output.lines("hint")[0];
        names(hint, b"the private repository tracks it");
        assert!(!holds(hint, b"git dupe add"), "{output:?}");
        assert!(!holds(hint, b"now hidden"), "{output:?}");
        gitdupe_written_and_staged(s, &dir, b".env\n");
    });
}

#[test]
fn literal_pathspec_settings_do_not_bypass_public_tracking() {
    under_each_release(|s| {
        let dir = s.dir().join("literal");
        s.project(&dir);
        s.init(&dir);
        let before = Tree::of(&dir);
        for output in [
            s.git(["--literal-pathspecs", "dupe", "hide", "docs/"])
                .from(&dir)
                .run(),
            s.git(["dupe", "hide", "docs/"])
                .from(&dir)
                .variable("GIT_LITERAL_PATHSPECS", "1")
                .run(),
        ] {
            refused_then_warnings(&output, b"2", &[]);
            unchanged(&before, &dir);
        }
    });
}

#[test]
fn public_hooks_and_repository_variables_stage_only_in_the_private_index() {
    under_each_release(|s| {
        let dir = s.dir().join("hook");
        s.project(&dir);
        s.init(&dir);
        let hook = dir.join(".git/hooks/pre-commit");
        fs::write(&hook, b"#!/bin/sh\ngit dupe hide x\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.join("README.md"), b"commit me\n").unwrap();
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
                "hook",
            ])
            .from(&dir)
            .succeeds();
        hinted(&output, &[b"x"]);
        lines_in_order(&output, "warning", &[]);
        gitdupe_written_and_staged(s, &dir, b"x\n");
        assert!(
            !s.git(["ls-files", "-z"])
                .from(&dir)
                .succeeds()
                .stdout
                .split(|&byte| byte == 0)
                .any(|path| path == b".gitdupe")
        );
        let public_index = fs::read(dir.join(".git/index")).unwrap();
        let output = s
            .git(["dupe", "hide", "y"])
            .from(&dir)
            .variable("GIT_DIR", dir.join(".git"))
            .variable("GIT_WORK_TREE", &dir)
            .variable("GIT_INDEX_FILE", dir.join(".git/index"))
            .run();
        succeeded_naming(&output, &[b"y"], &[]);
        gitdupe_written_and_staged(s, &dir, b"x\ny\n");
        assert_eq!(fs::read(dir.join(".git/index")).unwrap(), public_index);
    });
}

#[test]
fn unreadable_listing_and_failed_staging_preserve_their_failure_boundaries() {
    under_each_release(|s| {
        let dir = s.dir().join("directory");
        s.project(&dir);
        s.init(&dir);
        fs::create_dir(dir.join(".gitdupe")).unwrap();
        let before = Tree::of(&dir);
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty());
        lines_in_order(&output, "fatal", &[b".gitdupe"]);
        hinted(&output, &[]);
        lines_in_order(&output, "warning", &[b".gitdupe"]);
        assert!(dir.join(".gitdupe").is_dir());
        unchanged(&before, &dir);

        let dir = s.dir().join("index-lock");
        s.project(&dir);
        s.init(&dir);
        let lock = dir.join(".git/dupe/index.lock");
        fs::write(&lock, b"locked\n").unwrap();
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        let gits = s
            .private(&dir)
            .git(["add", "-f", "--", ":(top,literal).gitdupe"])
            .run();
        assert_ne!(gits.end, End::Code(0), "{gits:?}");
        assert_eq!(output.end, gits.end);
        assert!(output.stdout.is_empty());
        hinted(&output, &[b"x"]);
        lines_in_order(&output, "warning", &[]);
        let hint = [b"hint: ".as_slice(), output.lines("hint")[0], b"\n"].concat();
        assert_eq!(output.stderr, [gits.stderr, hint].concat());
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"x\n");
        assert!(
            s.private(&dir)
                .git(["ls-files", "-z"])
                .run()
                .stdout
                .is_empty()
        );
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/x"]);
        fs::remove_file(lock).unwrap();
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        succeeded_naming(&output, &[], &[]);
        gitdupe_written_and_staged(s, &dir, b"x\n");
    });
}

#[test]
fn rewriting_replaces_symlinks_preserves_permissions_and_accepts_byte_paths() {
    under_each_release(|s| {
        let dir = s.dir().join("symlink");
        s.project(&dir);
        s.init(&dir);
        let target = s.dir().join("listing-target");
        fs::write(&target, b"notes\n").unwrap();
        symlink(&target, dir.join(".gitdupe")).unwrap();
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        succeeded_naming(&output, &[b"x"], &[b".gitdupe"]);
        assert!(
            fs::symlink_metadata(dir.join(".gitdupe"))
                .unwrap()
                .is_file()
        );
        assert_eq!(fs::read(target).unwrap(), b"notes\n");
        gitdupe_written_and_staged(s, &dir, b"notes\nx\n");
        let dir = s.dir().join("permissions");
        s.project(&dir);
        s.init(&dir);
        fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
        fs::set_permissions(dir.join(".gitdupe"), fs::Permissions::from_mode(0o600)).unwrap();
        let output = s.git(["dupe", "hide", "x"]).from(&dir).run();
        succeeded_naming(&output, &[b"x"], &[]);
        assert_eq!(
            fs::metadata(dir.join(".gitdupe"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        gitdupe_written_and_staged(s, &dir, b"notes\nx\n");
        let dir = s.dir().join("bytes");
        s.project(&dir);
        s.init(&dir);
        let output = s
            .git([
                OsStr::new("dupe"),
                OsStr::new("hide"),
                OsStr::from_bytes(b"caf\xe9"),
            ])
            .from(&dir)
            .run();
        succeeded_naming(&output, &[b"caf\xe9"], &[]);
        gitdupe_written_and_staged(s, &dir, b"caf\xe9\n");
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/caf\xe9"]);
    });
}
