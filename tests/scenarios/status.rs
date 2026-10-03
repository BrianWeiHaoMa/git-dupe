//! `git dupe status`: Git's output confined to hidden paths and staged deletions.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, daily_state, holds, names, private_add, private_commit,
    under_each_release, write,
};

const CHANGES: &[&str] = &[
    " M .env.local",
    "?? notes/today.md",
    "!! .vscode/launch.json",
];

fn fixture(s: &Scenario, dir: &Path) {
    daily_state(s, dir);
    for path in [
        "notes/today.md",
        ".vscode/launch.json",
        "docs/api.md",
        "src/scratch.py",
    ] {
        write(dir, path, b"new\n");
    }
    write(dir, ".env.local", b"changed\n");
}

/// Git owns record order; the set and count together also reject duplicates.
fn records(output: &Output, expected: &[&str]) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    let lines: Vec<_> = output
        .stdout
        .split_inclusive(|&byte| byte == b'\n')
        .map(|line| {
            line.strip_suffix(b"\n")
                .expect("a porcelain record ends in LF")
        })
        .collect();
    assert_eq!(lines.len(), expected.len(), "{output:?}");
    assert_eq!(
        lines.into_iter().collect::<BTreeSet<_>>(),
        expected.iter().map(|line| line.as_bytes()).collect(),
        "{output:?}"
    );
}

fn quiet_records(output: &Output, expected: &[&str]) {
    records(output, expected);
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn only_the_listing_is_hidden_hints_to_add_and_prints_gits_status() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let expected = s
            .private(&dir)
            .git([
                "status",
                "--untracked-files=normal",
                "--ignored",
                "--",
                ":(top,literal).gitdupe",
            ])
            .succeeds();
        let output = s.git(["dupe", "status"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(output.stdout, expected.stdout, "{output:?}");
        names(output.only_line("hint"), b"git dupe add");
        let porcelain = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        records(&porcelain, &[]);
        names(porcelain.only_line("hint"), b"git dupe add");
        assert_eq!(porcelain.stderr, output.stderr, "{porcelain:?}");
        s.git(["dupe", "hide", "notes"]).from(&dir).succeeds();
        let output = s.git(["dupe", "status"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    });
}

#[test]
fn a_file_tracked_below_a_directory_at_gitdupe_is_hidden_and_no_hint_follows() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe/x", b"private\n");
        private_add(s, &dir, ".gitdupe/x");
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        records(&output, &["A  .gitdupe/x"]);
        assert!(output.lines("hint").is_empty(), "{output:?}");
    });
}

#[test]
fn porcelain_shows_private_changes_from_root_and_subdirectory() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        for from in [&dir, &dir.join("docs")] {
            let output = s.git(["dupe", "status", "--porcelain"]).from(from).run();
            quiet_records(&output, CHANGES);
        }
    });
}

#[test]
fn publicly_tracked_files_under_hidden_paths_are_excluded() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        s.git(["dupe", "status"]).from(&dir).succeeds();
        write(&dir, "notes/shared.md", b"public\n");
        s.git(["add", "-f", "--", "notes/shared.md"])
            .from(&dir)
            .succeeds();
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        quiet_records(&output, CHANGES);
    });
}

#[test]
fn staged_deletions_under_hidden_paths_show_retained_files() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        write(&dir, "notes/old.md", b"old\n");
        private_add(s, &dir, "notes/old.md");
        private_commit(s, &dir);
        s.private(&dir)
            .git(["rm", "--cached", "-q", "--", "notes/old.md"])
            .succeeds();
        let mut expected = CHANGES.to_vec();
        expected.extend(["D  notes/old.md", "?? notes/old.md"]);
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        quiet_records(&output, &expected);
        fs::remove_file(dir.join("notes/old.md")).unwrap();
        expected.pop();
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        quiet_records(&output, &expected);
    });
}

#[test]
fn a_staged_deletion_below_a_publicly_tracked_file_is_shown_without_that_file() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"notes\nh\n");
        // A name holding each byte a glob reads, and a file beside it whose name begins
        // with it: only the exact path may be left out.
        let odd = "notes/w*[?]\\x";
        let beside = "notes/w*[?]\\xz";
        // Names spelled byte for byte as the escaped pattern of `notes/sub` and of the odd
        // name with the last byte bracketed alone. Git compares a pattern's raw bytes with
        // each path before matching it (S4), so these stay shown only because the pattern
        // the run is given holds a newline, which no path here holds (E4).
        let spelled_file = "notes/su[\\b]";
        let spelled_directory = r"notes/w\*\[\?]\\[\x]";
        let below = |parent: &str, file: &str| format!("{parent}/{file}");
        let private_literal_add = |path: &str| {
            s.private(&dir)
                .git(["--literal-pathspecs", "add", "-f", "--", path])
                .succeeds();
        };
        private_literal_add(".gitdupe");
        for path in [
            "notes/a",
            "notes/sub/y",
            "h/x",
            &below(odd, "y"),
            beside,
            spelled_file,
            &below(spelled_directory, "gone"),
        ] {
            write(&dir, path, b"private\n");
            private_literal_add(path);
        }
        private_commit(s, &dir);
        s.private(&dir)
            .git([
                "--literal-pathspecs",
                "rm",
                "--cached",
                "-q",
                "--",
                &below(spelled_directory, "gone"),
            ])
            .succeeds();
        write(&dir, &below(spelled_directory, "new"), b"untracked\n");
        write(&dir, spelled_file, b"changed\n");
        // Each deletion staged, and a teammate's file now standing at its parent: below a
        // hidden directory, at a hidden path itself, and at the odd name.
        for (parent, file) in [("notes/sub", "y"), ("h", "x"), (odd, "y")] {
            s.private(&dir)
                .git([
                    "--literal-pathspecs",
                    "rm",
                    "--cached",
                    "-q",
                    "--",
                    &below(parent, file),
                ])
                .succeeds();
            fs::remove_dir_all(dir.join(parent)).unwrap();
            write(&dir, parent, b"public\n");
            s.git(["--literal-pathspecs", "add", "-f", "--", parent])
                .from(&dir)
                .succeeds();
        }
        write(&dir, beside, b"changed\n");
        let expected: BTreeSet<Vec<u8>> = [
            "D  notes/sub/y".to_owned(),
            "D  h/x".to_owned(),
            format!("D  {odd}/y"),
            format!(" M {beside}"),
            format!(" M {spelled_file}"),
            format!("D  {spelled_directory}/gone"),
            format!("?? {spelled_directory}/gone"),
            format!("?? {spelled_directory}/new"),
        ]
        .into_iter()
        .map(String::into_bytes)
        .collect();
        for from in [dir.clone(), dir.join("notes")] {
            let output = s
                .git(["dupe", "status", "--porcelain", "-z"])
                .from(&from)
                .run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            let records: Vec<_> = output
                .stdout
                .split_inclusive(|&byte| byte == 0)
                .map(|record| record.strip_suffix(b"\0").unwrap().to_vec())
                .collect();
            assert_eq!(records.len(), expected.len(), "{output:?}");
            assert_eq!(
                records.into_iter().collect::<BTreeSet<_>>(),
                expected,
                "{output:?}"
            );
        }
    });
}

/// A user path takes the staged deletions at or below it and no other, and nothing else at
/// that path but what stands on disk at the deletion.
#[test]
fn a_user_path_takes_the_staged_deletions_at_or_below_it() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        s.git(["dupe", "status"]).from(&dir).succeeds();
        s.private(&dir)
            .git(["rm", "--cached", "-q", "--", "docs/notes.md"])
            .succeeds();
        // Settle once, so that the release it names is behind the runs compared below.
        s.git(["dupe", "status"]).from(&dir).succeeds();
        let deletion = &["D  docs/notes.md", "?? docs/notes.md"][..];
        for (from, path, expected) in [
            (dir.clone(), "docs", deletion),
            (dir.clone(), "docs/notes.md", deletion),
            (dir.join("docs"), ".", deletion),
            (dir.clone(), "notes", &["?? notes/today.md"][..]),
        ] {
            let output = s
                .git(["dupe", "status", "--porcelain", path])
                .from(&from)
                .run();
            quiet_records(&output, expected);
        }
    });
}

#[test]
fn staged_deletions_outside_hidden_paths_warn_once_and_survive_public_tracking() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        // Establish the managed region before removing its privately tracked file.
        s.git(["dupe", "status"]).from(&dir).succeeds();
        s.private(&dir)
            .git(["rm", "--cached", "-q", "--", "docs/notes.md"])
            .succeeds();
        let mut expected = CHANGES.to_vec();
        expected.extend(["D  docs/notes.md", "?? docs/notes.md"]);
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        records(&output, &expected);
        let warning = output.only_line("warning");
        names(warning, b"docs/notes.md");
        names(warning, b"visible to public Git");
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        quiet_records(&output, &expected);
        fs::remove_file(dir.join("docs/notes.md")).unwrap();
        expected.pop();
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        quiet_records(&output, &expected);
        write(&dir, "docs/notes.md", b"public now\n");
        s.git(["add", "--", "docs/notes.md"]).from(&dir).succeeds();
        expected.push("?? docs/notes.md");
        let output = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
        quiet_records(&output, &expected);
    });
}

#[test]
fn literal_user_paths_intersect_hidden_paths_and_root_keeps_the_whole_scope() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        for (from, path, expected) in [
            (dir.clone(), "notes/", &["?? notes/today.md"][..]),
            (dir.join("notes"), ".", &["?? notes/today.md"][..]),
            (dir.join("docs"), ".", &[][..]),
            (dir.clone(), "src/", &[][..]),
            (dir.join("docs"), ":/", CHANGES),
            (dir.clone(), ".", CHANGES),
            (dir.join("docs"), ":/notes", &["?? notes/today.md"][..]),
        ] {
            let output = s
                .git(["dupe", "status", "--porcelain", path])
                .from(&from)
                .run();
            quiet_records(&output, expected);
        }
        write(&dir, "notes/sub/x.md", b"nested\n");
        let output = s
            .git(["dupe", "status", "--porcelain", "notes/sub"])
            .from(&dir)
            .run();
        quiet_records(&output, &["?? notes/sub/"]);
        write(&dir, ".gitdupe", b"notes\n.vscode\n-x\n");
        private_add(s, &dir, ".gitdupe");
        write(&dir, "-x", b"dashed\n");
        let output = s
            .git(["dupe", "status", "--porcelain", "--", "-x"])
            .from(&dir)
            .run();
        quiet_records(&output, &["?? -x"]);
    });
}

#[test]
fn verbose_status_keeps_gits_diff_over_the_whole_index() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        fixture(s, &dir);
        for path in ["notes/a.md", ".env.local"] {
            write(&dir, path, b"staged change\n");
            private_add(s, &dir, path);
        }
        let expected = s
            .private(&dir)
            .git([
                "status",
                "-v",
                "--untracked-files=normal",
                "--ignored",
                "--",
                ":(top,literal)notes",
            ])
            .succeeds();
        let output = s.git(["dupe", "status", "-v", "notes/"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(output.stdout, expected.stdout, "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        for path in [b"notes/a.md".as_slice(), b".env.local"] {
            assert!(holds(&output.stdout, path), "{output:?}");
        }
    });
}
