//! `add` with literal operands: hiding directories first, tracking files, and refusals.

use std::fs;
use std::os::unix::fs::symlink;

use crate::harness::{
    End, daily_state, gitdupe_written_and_staged, holds, lines_in_order, names, names_number,
    names_the_route_to_private, private_add, region_rules, staged_gitdupe, under_each_release,
    warnings_in_any_order, write,
};

#[test]
fn a_new_directory_is_hidden_staged_and_can_be_unhidden() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, "notes/a.md", b"a\n");
        write(&dir, "notes/b.md", b"b\n");
        let output = s.git(["dupe", "add", "notes/"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        lines_in_order(&output, "hint", &[b"notes"]);
        names(output.lines("hint")[0], b"git dupe unhide");
        // `add` is what versions it: its hint does not send the developer to `add`.
        assert!(
            !holds(output.lines("hint")[0], b"git dupe add"),
            "{output:?}"
        );
        gitdupe_written_and_staged(s, &dir, b"notes\n");
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b".gitdupe\0notes/a.md\0notes/b.md\0"
        );
        assert_eq!(
            s.private(&dir)
                .git(["show", ":notes/a.md"])
                .succeeds()
                .stdout,
            b"a\n"
        );
        assert_eq!(
            s.private(&dir)
                .git(["show", ":notes/b.md"])
                .succeeds()
                .stdout,
            b"b\n"
        );
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
        assert!(
            s.git(["status", "--porcelain"])
                .from(&dir)
                .succeeds()
                .stdout
                .is_empty()
        );

        // Git owns the porcelain answer; use the same staging in a fresh twin.
        let twin = s.dir().join("twin");
        s.attached_project(&twin);
        write(&twin, ".gitdupe", b"notes\n");
        write(&twin, "notes/a.md", b"a\n");
        write(&twin, "notes/b.md", b"b\n");
        for path in [".gitdupe", "notes/a.md", "notes/b.md"] {
            private_add(s, &twin, path);
        }
        let expected = s
            .private(&twin)
            .git(["status", "--porcelain", "--", ":(top,literal)notes"])
            .succeeds();
        let status = s
            .git(["dupe", "status", "--porcelain"])
            .from(&dir)
            .succeeds();
        // status also includes the staged listing, which is outside this comparison's scope.
        let notes: Vec<_> = status
            .stdout
            .split_inclusive(|&b| b == b'\n')
            .filter(|line| holds(line, b"notes/"))
            .flatten()
            .copied()
            .collect();
        assert_eq!(notes, expected.stdout, "{status:?}");
        assert_eq!(notes, b"A  notes/a.md\nA  notes/b.md\n");
        let unhide = s.git(["dupe", "unhide", "notes/"]).from(&dir).succeeds();
        lines_in_order(&unhide, "hint", &[b"notes"]);
        assert_eq!(
            region_rules(&dir),
            [b"/.gitdupe".as_slice(), b"/notes/a.md", b"/notes/b.md"]
        );
    });
}

#[test]
fn an_ignored_literal_file_keeps_gits_refusal() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        let twin = s.dir().join("twin");
        for place in [&dir, &twin] {
            s.attached_project(place);
            write(place, ".env.local", b"private\n");
        }
        let expected = s
            .private(&twin)
            .git(["add", "--", ":(top,literal).env.local"])
            .run();
        let output = s.git(["dupe", "add", ".env.local"]).from(&dir).run();
        assert_eq!(
            output.end, expected.end,
            "{output:?}; private: {expected:?}"
        );
        assert!(
            s.private(&dir)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout
                .is_empty()
        );
        assert!(!dir.join(".gitdupe").exists());
        assert!(
            output
                .lines("hint")
                .iter()
                .all(|line| !holds(line, b"git dupe")),
            "{output:?}"
        );
    });
}

#[test]
fn force_adds_ignored_files_and_hides_an_ignored_directory() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".env.local", b"private\n");
        write(&dir, ".vscode/settings.json", b"settings\n");
        let output = s
            .git(["dupe", "add", "-f", ".env.local", ".vscode/"])
            .from(&dir)
            .succeeds();
        lines_in_order(&output, "hint", &[b".vscode"]);
        names(output.lines("hint")[0], b"git dupe unhide");
        gitdupe_written_and_staged(s, &dir, b".vscode\n");
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b".env.local\0.gitdupe\0.vscode/settings.json\0"
        );
        assert_eq!(
            region_rules(&dir),
            [b"/.env.local".as_slice(), b"/.gitdupe", b"/.vscode"]
        );
    });
}

#[test]
fn a_literal_file_hides_only_itself_and_its_public_parent_cannot_be_hidden() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, "docs/notes.md", b"private\n");
        let output = s
            .git(["dupe", "add", "docs/notes.md"])
            .from(&dir)
            .succeeds();
        lines_in_order(&output, "hint", &[]);
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b"docs/notes.md\0"
        );
        assert!(!dir.join(".gitdupe").exists());
        assert_eq!(
            region_rules(&dir),
            [b"/.gitdupe".as_slice(), b"/docs/notes.md"]
        );
        write(&dir, "docs/design.md", b"edited\n");
        assert_eq!(
            s.git(["status", "--porcelain"])
                .from(&dir)
                .succeeds()
                .stdout,
            b" M docs/design.md\n"
        );
        let refused = s.git(["dupe", "hide", "docs/"]).from(&dir).run();
        assert_eq!(refused.end, End::Code(128), "{refused:?}");
        names_number(refused.only_line("fatal"), 1);
        names(refused.only_line("fatal"), b"git rm --cached");
    });
}

#[test]
fn dry_run_only_names_the_directory_it_would_hide() {
    under_each_release(|s| {
        for option in ["-n", "-nv", "--dry-run"] {
            let dir = s.dir().join(option);
            s.attached_project(&dir);
            write(&dir, "scratch/a", b"private\n");
            let listing = fs::read(dir.join(".gitdupe")).ok();
            let index = s.private(&dir).git(["ls-files", "-s"]).succeeds().stdout;
            let exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
            let output = s
                .git(["dupe", "add", option, "scratch/"])
                .from(&dir)
                .succeeds();
            lines_in_order(&output, "hint", &[b"scratch"]);
            names(output.lines("hint")[0], b"would be hidden");
            assert_eq!(fs::read(dir.join(".gitdupe")).ok(), listing);
            assert_eq!(
                s.private(&dir).git(["ls-files", "-s"]).succeeds().stdout,
                index
            );
            assert_eq!(fs::read(dir.join(".git/info/exclude")).unwrap(), exclude);
        }
    });
}

#[test]
fn a_symlink_to_a_directory_is_staged_as_a_file() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, "target/a", b"private\n");
        symlink("target", dir.join("link")).unwrap();
        let output = s.git(["dupe", "add", "link"]).from(&dir).succeeds();
        lines_in_order(&output, "hint", &[]);
        assert!(!dir.join(".gitdupe").exists());
        let index = s.private(&dir).git(["ls-files", "-s"]).succeeds();
        assert!(index.stdout.starts_with(b"120000 "), "{index:?}");
        assert!(index.stdout.ends_with(b"\tlink\n"), "{index:?}");
        assert_eq!(
            s.private(&dir).git(["show", ":link"]).succeeds().stdout,
            b"target"
        );
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/link"]);
    });
}

#[test]
fn hidden_directories_and_descendants_add_without_hints_but_all_with_an_operand_hides() {
    under_each_release(|s| {
        for (operand, path) in [
            ("notes/", "notes/new.md"),
            ("notes/sub/", "notes/sub/new.md"),
        ] {
            let dir = s.dir().join(path.replace('/', "-"));
            daily_state(s, &dir);
            write(&dir, path, b"new\n");
            let listing = fs::read(dir.join(".gitdupe")).unwrap();
            let output = s.git(["dupe", "add", operand]).from(&dir).succeeds();
            lines_in_order(&output, "hint", &[]);
            assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), listing);
            assert_eq!(
                s.private(&dir)
                    .git(["diff", "--cached", "--name-status", "-z"])
                    .succeeds()
                    .stdout,
                format!("A\0{path}\0").as_bytes()
            );
        }
        let dir = s.dir().join("all-literal");
        daily_state(s, &dir);
        write(&dir, "scratch/new.md", b"new\n");
        let output = s
            .git(["dupe", "add", "-A", "scratch/"])
            .from(&dir)
            .succeeds();
        lines_in_order(&output, "hint", &[b"scratch"]);
        names(output.lines("hint")[0], b"git dupe unhide");
        gitdupe_written_and_staged(s, &dir, b"notes\n.vscode\nscratch\n");
        assert_eq!(
            s.private(&dir)
                .git(["show", ":scratch/new.md"])
                .succeeds()
                .stdout,
            b"new\n"
        );
    });
}

#[test]
fn public_operands_refuse_before_any_directory_is_hidden_or_file_staged() {
    under_each_release(|s| {
        for (number, words) in [
            vec!["README.md"],
            vec!["-f", "README.md"],
            vec!["docs/"],
            vec!["scratch/", "docs/"],
            vec!["README.md", "scratch/"],
        ]
        .iter()
        .enumerate()
        {
            let dir = s.dir().join(format!("refusal-{number}"));
            daily_state(s, &dir);
            write(&dir, "scratch/new.md", b"new\n");
            let index = s.private(&dir).git(["ls-files", "-s"]).succeeds().stdout;
            let listing = fs::read(dir.join(".gitdupe")).unwrap();
            let output = s
                .git(["dupe", "add"].into_iter().chain(words.iter().copied()))
                .from(&dir)
                .run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            let fatal = output.only_line("fatal");
            names(fatal, b"git rm --cached");
            names_the_route_to_private(fatal);
            assert!(output.lines("hint").is_empty(), "{output:?}");
            if words.contains(&"docs/") {
                names_number(fatal, 1);
                names(fatal, b"one file at a time");
            } else {
                names(fatal, b"README.md");
            }
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(
                s.private(&dir).git(["ls-files", "-s"]).succeeds().stdout,
                index
            );
            assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), listing);
        }
    });
}

#[test]
fn a_directory_holding_a_file_tracked_by_both_refuses_then_settles() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, "conf/local.ini", b"both\n");
        s.git(["add", "-f", "--", "conf/local.ini"])
            .from(&dir)
            .succeeds();
        private_add(s, &dir, "conf/local.ini");
        let index = s.private(&dir).git(["ls-files", "-s"]).succeeds().stdout;
        let output = s.git(["dupe", "add", "conf/"]).from(&dir).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        lines_in_order(&output, "fatal", &[b"git rm --cached"]);
        names_number(output.lines("fatal")[0], 1);
        lines_in_order(&output, "warning", &[b"conf/local.ini"]);
        assert!(!dir.join(".gitdupe").exists());
        assert_eq!(
            s.private(&dir).git(["ls-files", "-s"]).succeeds().stdout,
            index
        );
    });
}

#[test]
fn a_public_listing_blocks_hiding_but_not_a_literal_file() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"");
        write(&dir, "scratch/a", b"new\n");
        write(&dir, "file.txt", b"file\n");
        s.git(["add", "-f", ".gitdupe"]).from(&dir).succeeds();
        let output = s.git(["dupe", "add", "scratch/"]).from(&dir).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        lines_in_order(&output, "fatal", &[b"git rm --cached .gitdupe"]);
        // Public Git tracks `.gitdupe`, so it does not ignore it: every command names it
        // (G6, G7).
        warnings_in_any_order(&output, &[&[b".gitdupe"]]);
        // The refusal and that warning are all of standard error: no hint of a hiding.
        let warning = [b"warning: ", output.lines("warning")[0], b"\n"].concat();
        output.line_then("fatal", &warning);
        assert!(
            s.private(&dir)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout
                .is_empty()
        );
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"");
        let output = s.git(["dupe", "add", "file.txt"]).from(&dir).succeeds();
        warnings_in_any_order(&output, &[&[b".gitdupe"]]);
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b"file.txt\0"
        );
    });
}

#[test]
fn a_directory_at_the_listing_path_blocks_hiding() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        fs::create_dir(dir.join(".gitdupe")).unwrap();
        write(&dir, "scratch/a", b"new\n");
        let output = s.git(["dupe", "add", "scratch/"]).from(&dir).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert_eq!(output.lines("fatal").len(), 1, "{output:?}");
        lines_in_order(&output, "hint", &[]);
        assert!(dir.join(".gitdupe").is_dir());
        assert!(
            s.private(&dir)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout
                .is_empty()
        );
        assert!(!region_rules(&dir).iter().any(|rule| rule == b"/scratch"));
    });
}

#[test]
fn a_locked_index_leaves_the_listing_written_and_rerun_requires_explicit_listing_add() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        let twin = s.dir().join("twin");
        for place in [&dir, &twin] {
            s.attached_project(place);
            write(place, "scratch/a", b"a\n");
            write(place, "scratch/b", b"b\n");
            write(place, ".git/dupe/index.lock", b"");
        }
        write(&twin, ".gitdupe", b"scratch\n");
        let expected = s
            .private(&twin)
            .git(["add", "-f", "--", ":(top,literal).gitdupe"])
            .run();
        let output = s.git(["dupe", "add", "scratch/"]).from(&dir).run();
        assert_eq!(
            output.end, expected.end,
            "{output:?}; private: {expected:?}"
        );
        assert_eq!(fs::read(dir.join(".gitdupe")).unwrap(), b"scratch\n");
        lines_in_order(&output, "hint", &[b"scratch"]);
        names(output.lines("hint")[0], b"git dupe unhide");
        assert!(
            s.private(&dir)
                .git(["ls-files", "-z"])
                .succeeds()
                .stdout
                .is_empty()
        );
        fs::remove_file(dir.join(".git/dupe/index.lock")).unwrap();
        let rerun = s.git(["dupe", "add", "scratch/"]).from(&dir).succeeds();
        lines_in_order(&rerun, "hint", &[]);
        assert_eq!(
            s.private(&dir).git(["ls-files", "-z"]).succeeds().stdout,
            b"scratch/a\0scratch/b\0"
        );
        assert_eq!(staged_gitdupe(s, &dir), None);
        s.git(["dupe", "add", ".gitdupe"]).from(&dir).succeeds();
        gitdupe_written_and_staged(s, &dir, b"scratch\n");
    });
}
