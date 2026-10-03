//! What settle names after every command in an attached workspace.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, holds, private_add, private_commit, refused_stash_untracked,
    region_rules, stash_refusal_first, stash_untracked_line, under_each_release,
    warnings_in_any_order, write,
};

/// A workspace attached by the fixture `attached_repository`, with a directory `sub` below
/// its root.
fn fixture_attached(s: &Scenario, name: &str) -> PathBuf {
    let dir = s.dir().join(name);
    s.attached_repository(&dir);
    fs::create_dir(dir.join("sub")).unwrap();
    dir
}

/// A steady state must settle identically from the root and from below it.
fn steady(s: &Scenario, dir: &Path, rules: &[&[u8]], names: &[&[&[u8]]]) -> Output {
    let root = s.git(["dupe", "stash", "-u"]).from(dir).run();
    refused_stash_untracked(s, &root);
    warnings_in_any_order(&root, names);
    assert_eq!(region_rules(dir), rules, "{root:?}");
    let below = s.git(["dupe", "stash", "-u"]).from(&dir.join("sub")).run();
    refused_stash_untracked(s, &below);
    warnings_in_any_order(&below, names);
    assert_eq!(below.stderr, root.stderr);
    assert_eq!(below.end, root.end);
    assert_eq!(region_rules(dir), rules, "{below:?}");
    root
}

#[test]
fn privately_tracked_file_is_hidden_and_dual_tracking_warns_until_released() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, "conf/local.ini", b"private\n");
        private_add(s, &dir, "conf/local.ini");
        let rules: &[&[u8]] = &[b"/.gitdupe", b"/conf/local.ini"];
        steady(s, &dir, rules, &[]);
        s.git(["add", "-f", "--", "conf/local.ini"])
            .from(&dir)
            .succeeds();
        steady(s, &dir, rules, &[&[b"conf/local.ini"]]);
        s.git(["rm", "--cached", "--", "conf/local.ini"])
            .from(&dir)
            .succeeds();
        steady(s, &dir, rules, &[]);
    });
}

/// A public checkout or pull writes a public file onto a privately tracked path, as Git
/// overwrites any file it ignores: the path stays hidden and is named as tracked by both
/// until the private side releases it.
#[test]
fn public_checkout_or_pull_onto_a_privately_tracked_file_warns_until_released_privately() {
    under_each_release(|s| {
        for arrival in ["checkout", "pull"] {
            let dir = fixture_attached(s, arrival);
            let teammate = s.dir().join(format!("{arrival}-teammate"));
            let public = if arrival == "checkout" {
                s.git(["switch", "-q", "-c", "teammate"])
                    .from(&dir)
                    .succeeds();
                &dir
            } else {
                s.git([
                    OsStr::new("clone"),
                    OsStr::new("-q"),
                    dir.as_os_str(),
                    teammate.as_os_str(),
                ])
                .succeeds();
                &teammate
            };
            write(public, "conf.toml", b"public\n");
            s.git(["add", "conf.toml"]).from(public).succeeds();
            s.commit_public(public);
            if arrival == "checkout" {
                s.git(["switch", "-q", "main"]).from(&dir).succeeds();
            }
            write(&dir, "conf.toml", b"private\n");
            private_add(s, &dir, "conf.toml");
            private_commit(s, &dir);
            let rules: &[&[u8]] = &[b"/.gitdupe", b"/conf.toml"];
            steady(s, &dir, rules, &[]);

            let arrived = if arrival == "checkout" {
                s.git(["checkout", "-q", "teammate"]).from(&dir).run()
            } else {
                s.git([
                    OsStr::new("-c"),
                    OsStr::new("maintenance.auto=false"),
                    OsStr::new("pull"),
                    OsStr::new("-q"),
                    OsStr::new("--ff-only"),
                    teammate.as_os_str(),
                    OsStr::new("main"),
                ])
                .from(&dir)
                .run()
            };
            assert_eq!(arrived.end, End::Code(0), "{arrived:?}");
            assert_eq!(fs::read(dir.join("conf.toml")).unwrap(), b"public\n");
            let both: &[&[&[u8]]] = &[&[b"conf.toml", b"tracked by both"]];
            steady(s, &dir, rules, both);
            let status = s.git(["dupe", "status", "--porcelain"]).from(&dir).run();
            assert_eq!(status.end, End::Code(0), "{status:?}");
            warnings_in_any_order(&status, both);
            // Tracked by both, it is updated and shown as any privately tracked file.
            assert!(holds(&status.stdout, b" M conf.toml\n"), "{status:?}");

            let released = s
                .git(["dupe", "rm", "-q", "--cached", "conf.toml"])
                .from(&dir)
                .run();
            assert_eq!(released.end, End::Code(0), "{released:?}");
            assert!(
                released
                    .lines("warning")
                    .iter()
                    .all(|line| !holds(line, b"tracked by both")),
                "{released:?}"
            );
            steady(s, &dir, &[b"/.gitdupe"], &[]);
        }
    });
}

#[test]
fn reincluded_hidden_directory_warns_on_each_command_until_the_rule_is_removed() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        write(&dir, "notes/keep.txt", b"kept\n");
        write(&dir, ".gitignore", b"# deciding rule follows\n!notes\n");
        let rules: &[&[u8]] = &[b"/.gitdupe", b"/notes"];
        steady(s, &dir, rules, &[&[b"notes", b".gitignore:2"]]);
        write(&dir, ".gitignore", b"");
        steady(s, &dir, rules, &[]);
    });
}

/// Every hidden path is asked about, not only the region's: below a re-included hidden
/// directory, each privately tracked file gets public Git's own answer, a deciding rule of
/// its own, none, or ignored again.
#[test]
fn reincluded_hidden_directory_names_its_privately_tracked_files_too() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        for path in ["notes/keep.txt", "notes/plain.txt", "notes/ignored.txt"] {
            write(&dir, path, b"private\n");
            private_add(s, &dir, path);
        }
        write(&dir, ".gitignore", b"!notes\n");
        write(&dir, "notes/.gitignore", b"ignored.txt\n!keep.txt\n");
        let rules: &[&[u8]] = &[b"/.gitdupe", b"/notes"];
        steady(
            s,
            &dir,
            rules,
            &[
                &[b"notes", b".gitignore:1"],
                &[b"notes/keep.txt", b"notes/.gitignore:2"],
                &[b"notes/plain.txt"],
            ],
        );
        let public = s
            .git(["status", "--porcelain", "--untracked-files=all"])
            .from(&dir)
            .succeeds();
        for record in [&b"?? notes/keep.txt\n"[..], b"?? notes/plain.txt\n"] {
            assert!(holds(&public.stdout, record), "{public:?}");
        }
        assert!(!holds(&public.stdout, b"notes/ignored.txt"), "{public:?}");
    });
}

#[test]
fn negation_below_an_excluded_directory_does_not_reinclude_its_child() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        write(&dir, "notes/keep.txt", b"kept\n");
        write(&dir, ".gitignore", b"!notes/keep.txt\n");
        steady(s, &dir, &[b"/.gitdupe", b"/notes"], &[]);
    });
}

#[test]
fn reincluded_privately_tracked_child_names_the_deciding_rule() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, "notes/keep.txt", b"kept\n");
        private_add(s, &dir, "notes/keep.txt");
        write(
            &dir,
            ".gitignore",
            b"# deciding rule follows\n!notes/keep.txt\n",
        );
        let rules: &[&[u8]] = &[b"/.gitdupe", b"/notes/keep.txt"];
        steady(s, &dir, rules, &[&[b"notes/keep.txt", b".gitignore:2"]]);
        write(&dir, ".gitignore", b"");
        steady(s, &dir, rules, &[]);
    });
}

#[test]
fn hidden_directory_ignores_new_files_and_keeps_publicly_tracked_files_visible() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        write(&dir, "notes/shared.md", b"shared\n");
        s.git(["add", "-f", "--", "notes/shared.md"])
            .from(&dir)
            .succeeds();
        s.git([
            "-c",
            "maintenance.auto=false",
            "-c",
            "user.name=A Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "commit",
            "-q",
            "-m",
            "shared",
        ])
        .from(&dir)
        .succeeds();
        steady(s, &dir, &[b"/.gitdupe", b"/notes"], &[]);
        // No git-dupe command occurs between creating the file and observing public Git.
        write(&dir, "notes/new.txt", b"new\n");
        write(&dir, "notes/shared.md", b"modified\n");
        let root = s
            .git([
                "status",
                "--porcelain",
                "-z",
                "--ignored",
                "--untracked-files=all",
            ])
            .from(&dir)
            .succeeds();
        let records: Vec<_> = root.stdout.split(|&byte| byte == 0).collect();
        assert!(records.contains(&&b"!! notes/new.txt"[..]), "{root:?}");
        assert!(records.contains(&&b" M notes/shared.md"[..]), "{root:?}");
        let below = s
            .git([
                "status",
                "--porcelain",
                "-z",
                "--ignored",
                "--untracked-files=all",
            ])
            .from(&dir.join("sub"))
            .succeeds();
        assert_eq!(below.stdout, root.stdout);
        assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
    });
}

#[test]
fn released_directory_warns_once_only_when_something_remains_on_disk() {
    under_each_release(|s| {
        for present in [true, false] {
            let mut first = None;
            for below in [false, true] {
                let dir = fixture_attached(s, &format!("workspace-{present}-{below}"));
                write(&dir, ".gitdupe", b"notes\n");
                if present {
                    write(&dir, "notes/x", b"left\n");
                }
                steady(s, &dir, &[b"/.gitdupe", b"/notes"], &[]);
                write(&dir, ".gitdupe", b"");
                let from = if below { dir.join("sub") } else { dir.clone() };
                let output = s.git(["dupe", "stash", "-u"]).from(&from).run();
                refused_stash_untracked(s, &output);
                warnings_in_any_order(&output, if present { &[&[b"notes"]] } else { &[] });
                if present {
                    let warning = output.lines("warning")[0];
                    assert!(holds(warning, b"visible to public Git"), "{output:?}");
                }
                assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
                if let Some(first) = &first {
                    assert_eq!(&output.stderr, first);
                } else {
                    first = Some(output.stderr);
                }
                steady(s, &dir, &[b"/.gitdupe"], &[]);
            }
        }
    });
}

#[test]
fn symlink_ancestor_warns_without_preventing_other_exposure_answers() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, "target/secret", b"secret\n");
        symlink("target", dir.join("link")).unwrap();
        write(&dir, ".gitdupe", b"link/secret\nnotes\n");
        write(&dir, "notes/keep.txt", b"kept\n");
        write(&dir, ".gitignore", b"# deciding rule follows\n!notes\n");
        steady(
            s,
            &dir,
            &[b"/.gitdupe", b"/link/secret", b"/notes"],
            &[&[b"link/secret"], &[b"notes", b".gitignore:2"]],
        );
    });
}

#[test]
fn missing_gitdupe_uses_its_private_staged_version() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        private_add(s, &dir, ".gitdupe");
        fs::remove_file(dir.join(".gitdupe")).unwrap();
        steady(s, &dir, &[b"/.gitdupe", b"/notes"], &[]);
    });
}

#[test]
fn directory_at_gitdupe_warns_and_does_not_use_the_staged_list() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        private_add(s, &dir, ".gitdupe");
        fs::remove_file(dir.join(".gitdupe")).unwrap();
        fs::create_dir(dir.join(".gitdupe")).unwrap();
        write(&dir, "conf/local.ini", b"private\n");
        private_add(s, &dir, "conf/local.ini");
        steady(
            s,
            &dir,
            &[b"/.gitdupe", b"/conf/local.ini"],
            &[&[b".gitdupe"]],
        );
    });
}

#[test]
fn caller_pathspec_index_and_relative_repository_variables_preserve_settle() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, "conf/local.ini", b"private\n");
        private_add(s, &dir, "conf/local.ini");
        s.git(["add", "-f", "--", "conf/local.ini"])
            .from(&dir)
            .succeeds();
        write(&dir, ".gitdupe", b"notes\n");
        write(&dir, "notes/keep.txt", b"kept\n");
        write(&dir, ".gitignore", b"# deciding rule follows\n!notes\n");
        let rules: &[&[u8]] = &[b"/.gitdupe", b"/conf/local.ini", b"/notes"];
        let names: &[&[&[u8]]] = &[&[b"conf/local.ini"], &[b"notes", b".gitignore:2"]];
        let ordinary = steady(s, &dir, rules, names);
        let index = s.dir().join("not-yet-existing-index");
        assert!(!index.exists());
        let public_index = fs::read(dir.join(".git/index")).unwrap();
        let private_index = fs::read(dir.join(".git/dupe/index")).unwrap();
        for from in [&dir, &dir.join("sub")] {
            for run in [
                s.git(["dupe", "stash", "-u"])
                    .variable("GIT_LITERAL_PATHSPECS", "1"),
                s.git(["--literal-pathspecs", "dupe", "stash", "-u"]),
                s.git(["dupe", "stash", "-u"])
                    .variable("GIT_INDEX_FILE", &index),
            ] {
                let output = run.from(from).run();
                refused_stash_untracked(s, &output);
                warnings_in_any_order(&output, names);
                assert_eq!(output.stderr, ordinary.stderr);
                assert_eq!(region_rules(&dir), rules);
            }
        }
        let relative = s
            .git(["dupe", "stash", "-u"])
            .from(&dir.join("sub"))
            .variable("GIT_DIR", "../.git")
            .variable("GIT_WORK_TREE", "..")
            .run();
        refused_stash_untracked(s, &relative);
        warnings_in_any_order(&relative, names);
        assert_eq!(relative.stderr, ordinary.stderr);
        assert_eq!(region_rules(&dir), rules);
        assert!(!index.exists());
        assert_eq!(fs::read(dir.join(".git/index")).unwrap(), public_index);
        assert_eq!(
            fs::read(dir.join(".git/dupe/index")).unwrap(),
            private_index
        );
    });
}

#[test]
fn unreadable_private_repository_keeps_the_region_and_names_init() {
    under_each_release(|s| {
        let dir = fixture_attached(s, "workspace");
        write(&dir, ".gitdupe", b"notes\n");
        steady(s, &dir, &[b"/.gitdupe", b"/notes"], &[]);
        let exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
        fs::rename(dir.join(".git/dupe"), dir.join("saved-private")).unwrap();
        fs::create_dir(dir.join(".git/dupe")).unwrap();
        let mut root_stderr = None;
        for from in [&dir, &dir.join("sub")] {
            let output = s.git(["dupe", "stash", "-u"]).from(from).run();
            stash_refusal_first(s, &output);
            warnings_in_any_order(&output, &[&[b"git dupe init"]]);
            // The failed listing's own message reaches standard error between the
            // handler's line and settle's warning; what it says is Git's.
            let handler_line = [b"fatal: ", &stash_untracked_line(s)[..], b"\n"].concat();
            let after_handler = output.stderr.strip_prefix(&handler_line[..]).unwrap();
            let warning_at = after_handler
                .windows(b"warning: ".len())
                .position(|part| part == b"warning: ")
                .unwrap();
            assert!(warning_at > 0, "{output:?}");
            assert_eq!(fs::read(dir.join(".git/info/exclude")).unwrap(), exclude);
            assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
            if let Some(root) = &root_stderr {
                assert_eq!(&output.stderr, root);
            } else {
                root_stderr = Some(output.stderr);
            }
        }
    });
}
