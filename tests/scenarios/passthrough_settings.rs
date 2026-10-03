//! Passed-through commands keep Git's settings, caller directory, and output bytes.

use std::path::PathBuf;

use crate::harness::{End, Scenario, daily_state, holds, names, under_each_release, write};

fn fixture(s: &Scenario) -> PathBuf {
    let dir = s.dir().join("workspace");
    daily_state(s, &dir);
    // Settle the initial hidden paths before comparing commands with plain Git.
    let settled = s.git(["dupe", "ls-files"]).from(&dir).succeeds();
    assert!(settled.stderr.is_empty(), "{settled:?}");
    dir
}

#[test]
fn repository_options_and_directory_options_from_outside_use_private_history() {
    under_each_release(|s| {
        let dir = fixture(s);
        let private_head = s.private(&dir).git(["rev-parse", "HEAD"]).succeeds();
        let public_head = s.git(["rev-parse", "HEAD"]).from(&dir).succeeds();
        assert_ne!(private_head.stdout, public_head.stdout);

        let git_dir = format!("--git-dir={}/.git", dir.display());
        let work_tree = format!("--work-tree={}", dir.display());
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "log", "-1"])
            .from(s.dir())
            .succeeds();
        assert!(holds(
            &expected.stdout,
            private_head.stdout.trim_ascii_end()
        ));
        assert!(!holds(
            &expected.stdout,
            public_head.stdout.trim_ascii_end()
        ));
        let output = s.git([&git_dir, &work_tree, "dupe", "log", "-1"]).run();
        assert_eq!(output, expected);

        let root = dir.to_str().unwrap();
        let expected = s
            .private(&dir)
            .git(["-C", root, "-c", "help.autocorrect=0", "log", "-1"])
            .from(s.dir())
            .succeeds();
        let output = s.git(["-C", root, "dupe", "log", "-1"]).run();
        assert_eq!(output, expected);
    });
}

#[test]
fn color_options_before_dupe_keep_gits_log_bytes() {
    under_each_release(|s| {
        let dir = fixture(s);
        let expected = s
            .private(&dir)
            .git([
                "-c",
                "color.ui=always",
                "-c",
                "help.autocorrect=0",
                "log",
                "-1",
            ])
            .succeeds();
        assert!(expected.stdout.contains(&0x1b), "{expected:?}");
        let output = s
            .git(["-c", "color.ui=always", "dupe", "log", "-1"])
            .from(&dir)
            .run();
        assert_eq!(output, expected);
    });
}

#[test]
fn no_pager_before_dupe_keeps_gits_log_answer() {
    under_each_release(|s| {
        let dir = fixture(s);
        let expected = s
            .private(&dir)
            .git(["--no-pager", "-c", "help.autocorrect=0", "log"])
            .succeeds();
        assert!(!expected.stdout.is_empty(), "{expected:?}");
        let output = s.git(["--no-pager", "dupe", "log"]).from(&dir).run();
        assert_eq!(output, expected);

        // Standard output is no terminal here, so no pager starts either way: the pager a
        // run would use is what tells a run that kept `--no-pager` from one that lost it.
        let pager = |options: &[&str]| {
            s.git(options.iter().chain(&["dupe", "var", "GIT_PAGER"]))
                .from(&dir)
                .succeeds()
                .stdout
        };
        let gits = s
            .private(&dir)
            .git(["--no-pager", "var", "GIT_PAGER"])
            .succeeds()
            .stdout;
        assert_eq!(pager(&["--no-pager"]), gits);
        assert_ne!(pager(&[]), gits);
    });
}

#[test]
fn literal_pathspecs_before_dupe_apply_to_the_users_ls_files_operand() {
    under_each_release(|s| {
        let dir = fixture(s);
        let expected = s
            .private(&dir)
            .git([
                "--literal-pathspecs",
                "-c",
                "help.autocorrect=0",
                "ls-files",
                "*",
            ])
            .succeeds();
        assert!(expected.stdout.is_empty(), "{expected:?}");
        let output = s
            .git(["--literal-pathspecs", "dupe", "ls-files", "*"])
            .from(&dir)
            .run();
        assert_eq!(output, expected);

        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "ls-files", "*"])
            .succeeds();
        assert!(holds(&expected.stdout, b"notes/a.md\n"), "{expected:?}");
        let output = s.git(["dupe", "ls-files", "*"]).from(&dir).run();
        assert_eq!(output, expected);
    });
}

#[test]
fn the_users_config_command_keeps_git_config_from_the_environment() {
    under_each_release(|s| {
        let dir = fixture(s);
        let config = s.dir().join("caller.config");
        write(s.dir(), "caller.config", b"[x]\n\ty = from-file\n");
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "config", "x.y"])
            .variable("GIT_CONFIG", &config)
            .succeeds();
        assert_eq!(expected.stdout, b"from-file\n");
        let output = s
            .git(["dupe", "config", "x.y"])
            .from(&dir)
            .variable("GIT_CONFIG", &config)
            .run();
        assert_eq!(output, expected);

        let absent = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "config", "x.y"])
            .run();
        assert_eq!(absent.end, End::Code(1), "{absent:?}");
        assert!(absent.stdout.is_empty(), "{absent:?}");
    });
}

#[test]
fn ls_files_and_diff_from_a_hidden_directory_keep_relative_paths() {
    under_each_release(|s| {
        let dir = fixture(s);
        let notes = dir.join("notes");
        // Git's default stat is root-relative; this setting makes the caller's
        // directory observable in the stat as well as in ls-files.
        s.private(&dir)
            .git(["config", "diff.relative", "true"])
            .succeeds();
        write(&dir, "notes/a.md", b"changed\n");
        for words in [vec!["ls-files"], vec!["diff", "--stat"]] {
            let expected = s
                .private(&dir)
                .git(
                    ["-c", "help.autocorrect=0"]
                        .into_iter()
                        .chain(words.iter().copied()),
                )
                .from(&notes)
                .succeeds();
            assert!(holds(&expected.stdout, b"a.md"), "{expected:?}");
            assert!(!holds(&expected.stdout, b"notes/a.md"), "{expected:?}");
            let output = s
                .git(["dupe"].into_iter().chain(words.iter().copied()))
                .from(&notes)
                .run();
            assert_eq!(output, expected);
        }
    });
}

#[test]
fn machine_output_keeps_settle_warnings_on_standard_error() {
    under_each_release(|s| {
        let dir = fixture(s);
        write(&dir, ".env.local", b"changed\n");
        write(
            &dir,
            ".gitignore",
            b".env.local\n.vscode/\nbuild/\n!.env.local\n",
        );
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "ls-files", "-z"])
            .succeeds();
        assert!(expected.stdout.contains(&0), "{expected:?}");
        assert!(expected.stderr.is_empty(), "{expected:?}");
        let listed = s.git(["dupe", "ls-files", "-z"]).from(&dir).succeeds();
        assert_eq!(listed.stdout, expected.stdout);
        names(listed.only_line("warning"), b".env.local");

        let status = s
            .git(["dupe", "status", "--porcelain"])
            .from(&dir)
            .succeeds();
        names(status.only_line("warning"), b".env.local");
        assert!(!status.stdout.is_empty(), "{status:?}");
        for output in [&listed, &status] {
            assert!(!holds(&output.stdout, b"warning:"), "{output:?}");
            assert!(!holds(&output.stdout, b"hint:"), "{output:?}");
            assert!(!output.stdout.contains(&0x1b), "{output:?}");
        }
    });
}

#[test]
fn the_callers_nonexistent_index_does_not_replace_the_private_index() {
    under_each_release(|s| {
        let dir = fixture(s);
        let index = s.dir().join("nonexistent-index");
        assert!(!index.exists());
        let expected = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "ls-files"])
            .succeeds();
        assert!(holds(&expected.stdout, b"notes/a.md\n"), "{expected:?}");
        let caller_index = s
            .private(&dir)
            .git(["-c", "help.autocorrect=0", "ls-files"])
            .variable("GIT_INDEX_FILE", &index)
            .succeeds();
        assert!(caller_index.stdout.is_empty(), "{caller_index:?}");
        let output = s
            .git(["dupe", "ls-files"])
            .from(&dir)
            .variable("GIT_INDEX_FILE", &index)
            .run();
        assert_eq!(output, expected);
        assert!(!index.exists());
    });
}

#[test]
fn options_before_dupe_reach_status_as_they_reach_a_command_passed_through() {
    under_each_release(|s| {
        let dir = fixture(s);
        write(&dir, ".env.local", b"changed\n");
        let plain = s.git(["dupe", "status"]).from(&dir).succeeds();

        // `-C` names the workspace from outside it.
        let root = dir.to_str().unwrap();
        let elsewhere = s.git(["-C", root, "dupe", "status"]).succeeds();
        assert_eq!(elsewhere, plain);

        // A `-c` before `dupe` decides the color, as Git's configuration does.
        let colored = s
            .git(["-c", "color.ui=always", "dupe", "status"])
            .from(&dir)
            .succeeds();
        assert!(colored.stdout.contains(&0x1b), "{colored:?}");
        let uncolored = s
            .git(["-c", "color.ui=never", "dupe", "status"])
            .from(&dir)
            .succeeds();
        assert!(!uncolored.stdout.contains(&0x1b), "{uncolored:?}");
        assert!(holds(&uncolored.stdout, b".env.local"), "{uncolored:?}");
    });
}
