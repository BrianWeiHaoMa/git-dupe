//! What `add` leaves unstaged under the hidden paths, the pathspecs it drops, and
//! its Git runs (G13, G14, G22, G23).

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

use crate::harness::{
    End, Output, Runs, Scenario, daily_edited, holds, lines_in_order, names, names_number,
    names_the_route_to_private, private_add, private_commit, region_rules, run_traced,
    under_each_release, write,
};

fn staged_blob(s: &Scenario, dir: &Path, path: &str, expected: &[u8]) {
    let output = s.private(dir).git(["show", &format!(":{path}")]).succeeds();
    assert_eq!(output.stdout, expected, "{output:?}");
}

fn not_in_private_index(s: &Scenario, dir: &Path, path: &str) {
    let output = s.private(dir).git(["ls-files", "-z"]).succeeds();
    assert!(
        !output
            .stdout
            .split(|&byte| byte == 0)
            .any(|entry| entry == path.as_bytes()),
        "{path} in private index: {output:?}"
    );
}

fn skipped_one(output: &Output) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    let warnings = output.lines("warning");
    assert_eq!(warnings.len(), 1, "{output:?}");
    names_number(warnings[0], 1);
    names(warnings[0], b"left unstaged");
    names(warnings[0], b"one file at a time");
    names_the_route_to_private(warnings[0]);
}

#[test]
fn public_files_under_hidden_directories_stay_unstaged_even_with_force() {
    under_each_release(|s| {
        for (index, words) in [
            &["dupe", "add", "notes/"][..],
            &["dupe", "add", "-f", "notes/"],
            &["dupe", "add", "."],
        ]
        .into_iter()
        .enumerate()
        {
            let dir = daily_edited(s, &format!("public-{index}"));
            write(&dir, "notes/shared.md", b"public\n");
            s.git(["add", "-f", "notes/shared.md"])
                .from(&dir)
                .succeeds();
            s.commit_public(&dir);
            let output = s.git(words).from(&dir).run();
            skipped_one(&output);
            staged_blob(s, &dir, "notes/today.md", b"today\n");
            not_in_private_index(s, &dir, "notes/shared.md");
            let status = s
                .git(["dupe", "status", "--porcelain"])
                .from(&dir)
                .succeeds();
            assert!(!holds(&status.stdout, b"notes/shared.md"), "{status:?}");
        }
    });
}

#[test]
fn a_file_tracked_by_both_is_updated_and_warned_by_settle_only() {
    under_each_release(|s| {
        let dir = daily_edited(s, "both");
        write(&dir, "notes/both.md", b"before\n");
        private_add(s, &dir, "notes/both.md");
        private_commit(s, &dir);
        s.git(["add", "-f", "notes/both.md"]).from(&dir).succeeds();
        s.commit_public(&dir);
        write(&dir, "notes/both.md", b"after\n");
        let output = s.git(["dupe", "add", "."]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        lines_in_order(&output, "warning", &[b"notes/both.md"]);
        assert!(
            !holds(output.lines("warning")[0], b"left unstaged"),
            "{output:?}"
        );
        staged_blob(s, &dir, "notes/both.md", b"after\n");
        let diff = s
            .private(&dir)
            .git(["diff", "--cached", "--name-status", "-z"])
            .succeeds();
        names(&diff.stdout, b"M\0notes/both.md\0");
    });
}

#[test]
fn absent_paths_and_paths_beyond_symlinks_do_not_poison_root_add() {
    under_each_release(|s| {
        let dir = daily_edited(s, "dropped");
        write(&dir, ".gitdupe", b"notes\n.vscode\nghost\nlink/x\n");
        let outside = s.dir().join("outside");
        write(&outside, "x/f", b"outside\n");
        symlink(&outside, dir.join("link")).unwrap();
        let output = s.git(["dupe", "add", "."]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        staged_blob(s, &dir, "notes/today.md", b"today\n");
        staged_blob(s, &dir, ".env.local", b"changed\n");
        not_in_private_index(s, &dir, "link/x/f");
        not_in_private_index(s, &dir, "ghost");
        assert_eq!(fs::read(outside.join("x/f")).unwrap(), b"outside\n");
    });
}

#[test]
fn a_missing_hidden_directory_keeps_pathspecs_for_tracked_deletions() {
    under_each_release(|s| {
        let dir = daily_edited(s, "deletions");
        write(&dir, ".gitdupe", b"notes\n.vscode\ngone\n");
        for path in ["gone/a", "gone/b"] {
            write(&dir, path, b"private\n");
        }
        private_add(s, &dir, "gone");
        private_add(s, &dir, ".gitdupe");
        private_commit(s, &dir);
        fs::remove_dir_all(dir.join("gone")).unwrap();
        let output = s.git(["dupe", "add", "."]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let diff = s
            .private(&dir)
            .git(["diff", "--cached", "--name-status", "-z"])
            .succeeds();
        for path in [b"D\0gone/a\0".as_slice(), b"D\0gone/b\0"] {
            names(&diff.stdout, path);
        }
    });
}

#[test]
fn update_and_refresh_drop_hidden_directories_with_no_tracked_files() {
    under_each_release(|s| {
        for option in ["-u", "--refresh"] {
            let dir = daily_edited(s, option);
            write(&dir, ".gitdupe", b"notes\n.vscode\nscratch\n");
            write(&dir, "scratch/x", b"untracked\n");
            let output = s.git(["dupe", "add", option, "."]).from(&dir).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            not_in_private_index(s, &dir, "scratch/x");
            not_in_private_index(s, &dir, "notes/today.md");
            if option == "-u" {
                staged_blob(s, &dir, ".env.local", b"changed\n");
            }
        }
    });
}

#[test]
fn ignored_public_files_lose_exclusions_but_remain_unstaged_with_force() {
    under_each_release(|s| {
        let dir = daily_edited(s, "ignored-public");
        write(&dir, "notes/shared.log", b"public\n");
        let mut ignore = fs::read(dir.join(".gitignore")).unwrap();
        ignore.extend_from_slice(b"*.log\n");
        write(&dir, ".gitignore", &ignore);
        s.git(["add", "-f", ".gitignore", "notes/shared.log"])
            .from(&dir)
            .succeeds();
        s.commit_public(&dir);
        for words in [&["dupe", "add", "."][..], &["dupe", "add", "-f", "."]] {
            let output = s.git(words).from(&dir).run();
            skipped_one(&output);
            not_in_private_index(s, &dir, "notes/shared.log");
            staged_blob(s, &dir, "notes/today.md", b"today\n");
        }
    });
}

#[test]
fn exclusions_over_arg_max_refuse_with_the_carried_count_and_preserve_the_index() {
    under_each_release(|s| {
        let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
        assert!(limit.status.success());
        let limit: usize = std::str::from_utf8(&limit.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let count = limit / 128 + 1;
        let dir = daily_edited(s, "too-long");
        write(&dir, ".gitdupe", b"notes\n.vscode\nvendor\n");
        for index in 0..count {
            write(
                &dir,
                &format!("vendor/file-{index:016}-{}", "x".repeat(128)),
                b"public\n",
            );
        }
        s.git(["add", "-f", "--", "vendor"]).from(&dir).succeeds();
        // Settle before measuring the index and scope; it does not stage the listing.
        s.git(["dupe", "status", "--porcelain", "notes"])
            .from(&dir)
            .succeeds();
        let scope = region_rules(&dir);
        // These two ignored, privately tracked scope paths move to the second (-u) run.
        let kept = scope
            .iter()
            .filter(|path| ![b"/.env.local".as_slice(), b"/.vscode"].contains(&path.as_slice()))
            .count();
        let index = fs::read(dir.join(".git/dupe/index")).unwrap();
        let (output, runs) = run_traced(
            s.git(["dupe", "add", "."]).from(&dir),
            &s.dir().join("too-long-trace"),
        );
        assert_eq!(output.end, End::Code(128), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        names_number(output.only_line("fatal"), kept + count);
        assert_eq!(runs.of("add"), 0, "{runs:?}");
        assert_eq!(fs::read(dir.join(".git/dupe/index")).unwrap(), index);
    });
}

#[test]
fn git_run_count_does_not_grow_with_privately_tracked_files() {
    under_each_release(|s| {
        let mut previous: Option<Runs> = None;
        for count in [5, 500] {
            let dir = daily_edited(s, &format!("runs-{count}"));
            // Replace daily_state's one notes file, so the directory has exactly count
            // privately tracked files, then leave today.md untracked for the add run.
            s.private(&dir).git(["rm", "notes/a.md"]).succeeds();
            fs::remove_file(dir.join("notes/today.md")).unwrap();
            for index in 0..count {
                write(&dir, &format!("notes/{index}.md"), b"private\n");
            }
            private_add(s, &dir, "notes");
            private_commit(s, &dir);
            write(&dir, "notes/today.md", b"today\n");
            let (output, runs) = run_traced(
                s.git(["dupe", "add", "."]).from(&dir),
                &s.dir().join(format!("trace-{count}")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            staged_blob(s, &dir, "notes/today.md", b"today\n");
            staged_blob(s, &dir, ".env.local", b"changed\n");
            if let Some(before) = &previous {
                assert_eq!(
                    runs.count(),
                    before.count(),
                    "before: {before:?}; after: {runs:?}"
                );
            }
            previous = Some(runs);
        }
    });
}

/// Every word other than an operand reaches Git's `add` as written and in its order, before
/// `--` and the pathspecs.
#[test]
fn option_words_reach_gits_add_as_written() {
    under_each_release(|s| {
        for (index, options) in [
            &["-v", "--chmod", "+x"][..],
            &["--chmod=-x", "-Nv"],
            &["--ignore-missing", "-n", "--sparse"],
        ]
        .into_iter()
        .enumerate()
        {
            let dir = daily_edited(s, &format!("words-{index}"));
            write(&dir, "x.sh", b"echo private\n");
            let (output, runs) = run_traced(
                s.git(
                    ["dupe", "add"]
                        .into_iter()
                        .chain(options.iter().copied())
                        .chain(["x.sh"]),
                )
                .from(&dir),
                &s.dir().join(format!("words-{index}.trace")),
            );
            assert_eq!(output.end, End::Code(0), "{output:?}");
            let own = runs.own();
            let adds: Vec<&Vec<Vec<u8>>> = own
                .words()
                .iter()
                .zip(own.commands())
                .filter(|(_, command)| *command == b"add")
                .map(|(words, _)| words)
                .collect();
            assert_eq!(adds.len(), 1, "{runs:?}");
            let words = adds[0];
            let command = words.iter().position(|word| word == b"add").unwrap();
            let end = words.iter().position(|word| word == b"--").unwrap();
            let given: Vec<&[u8]> = options.iter().map(|word| word.as_bytes()).collect();
            let passed: Vec<&[u8]> = words[command + 1..end].iter().map(Vec::as_slice).collect();
            assert_eq!(passed, given, "{runs:?}");
        }
    });
}
