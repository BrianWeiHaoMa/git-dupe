//! `add` under the pathspec settings and from a public hook.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::harness::{End, Scenario, daily_edited, leaving_public_git, under_each_release, write};

/// Read the staged changes and the blobs; both settings and hook runs must match these.
fn private_staging(s: &Scenario, dir: &Path) -> Vec<u8> {
    let changed = s
        .private(dir)
        .git(["diff", "--cached", "--name-status", "-z"])
        .succeeds();
    assert_eq!(changed.stdout, b"M\0.env.local\0A\0notes/today.md\0");
    for (path, content) in [
        (".env.local", &b"changed\n"[..]),
        ("notes/today.md", b"today\n"),
    ] {
        let staged = s.private(dir).git(["show", &format!(":{path}")]).succeeds();
        assert_eq!(staged.stdout, content, "{staged:?}");
    }
    s.private(dir)
        .git(["ls-files", "--stage", "-z"])
        .succeeds()
        .stdout
}

#[test]
fn caller_literal_pathspec_settings_keep_plain_private_staging_and_public_index() {
    under_each_release(|s| {
        for setting in ["option", "environment"] {
            let plain = daily_edited(s, &format!("plain-{setting}"));
            let dir = daily_edited(s, setting);
            let plain_index = fs::read(plain.join(".git/index")).unwrap();
            let public_index = fs::read(dir.join(".git/index")).unwrap();
            s.git(["dupe", "add", "."]).from(&plain).succeeds();
            let expected = private_staging(s, &plain);
            let git = if setting == "option" {
                s.git(["--literal-pathspecs", "dupe", "add", "."])
            } else {
                s.git(["dupe", "add", "."])
                    .variable("GIT_LITERAL_PATHSPECS", "1")
            };
            let output = git.from(&dir).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert_eq!(private_staging(s, &dir), expected);
            assert_eq!(fs::read(dir.join(".git/index")).unwrap(), public_index);
            assert_eq!(fs::read(plain.join(".git/index")).unwrap(), plain_index);
        }
    });
}

#[test]
fn public_pre_commit_add_stages_only_in_the_private_index() {
    under_each_release(|s| {
        let plain = daily_edited(s, "plain");
        s.git(["dupe", "add", "."]).from(&plain).succeeds();
        let expected = private_staging(s, &plain);
        let dir = daily_edited(s, "hook");
        let public_paths = s.git(["ls-files", "-z"]).from(&dir).succeeds().stdout;
        let hook = dir.join(".git/hooks/pre-commit");
        fs::write(&hook, b"#!/bin/sh\ngit dupe add .\n").unwrap();
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
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(private_staging(s, &dir), expected);
        assert_eq!(
            s.git(["ls-files", "-z"]).from(&dir).succeeds().stdout,
            public_paths
        );
        assert_eq!(
            s.git(["ls-tree", "-r", "--name-only", "-z", "HEAD"])
                .from(&dir)
                .succeeds()
                .stdout,
            public_paths
        );
        assert!(
            s.git(["diff", "--cached", "--name-status", "-z"])
                .from(&dir)
                .succeeds()
                .stdout
                .is_empty()
        );
        assert_eq!(
            s.git(["show", "HEAD:README.md"])
                .from(&dir)
                .succeeds()
                .stdout,
            b"public change\n"
        );
    });
}

#[test]
fn public_shell_alias_add_stages_only_in_the_private_index() {
    under_each_release(|s| {
        let plain = daily_edited(s, "plain");
        s.git(["dupe", "add", "."]).from(&plain).succeeds();
        let expected = private_staging(s, &plain);
        for located in [false, true] {
            let dir = daily_edited(s, &format!("alias-{located}"));
            // Git runs a shell alias from the root; given `--git-dir` and `--work-tree`, it
            // hands the alias both, naming the public repository, as `GIT_DIR` and
            // `GIT_WORK_TREE`.
            s.git(["config", "alias.save-private", "!git dupe add ."])
                .from(&dir)
                .succeeds();
            let public_index = fs::read(dir.join(".git/index")).unwrap();
            let mut words = Vec::new();
            if located {
                let mut git_dir = OsString::from("--git-dir=");
                git_dir.push(dir.join(".git"));
                let mut work_tree = OsString::from("--work-tree=");
                work_tree.push(&dir);
                words.extend([git_dir, work_tree]);
            }
            words.push("save-private".into());
            let output = s.git(words).from(&dir.join("notes")).run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert_eq!(private_staging(s, &dir), expected);
            assert_eq!(fs::read(dir.join(".git/index")).unwrap(), public_index);
        }
    });
}

/// A caller's variables that locate a repository, an index, or an object store, each naming
/// the public repository's, reach no private run: `add` and `commit` act on the private
/// repository alone, and its runs read no object of the public one.
#[test]
fn a_callers_locating_variables_reach_no_private_run() {
    under_each_release(|s| {
        let plain = daily_edited(s, "plain");
        s.git(["dupe", "add", "."]).from(&plain).succeeds();
        let expected = private_staging(s, &plain);
        let dir = daily_edited(s, "located");
        let public = dir.join(".git");
        let mut references = OsString::from("files://");
        references.push(&public);
        let located = |words: &[&str]| {
            s.git(words)
                .from(&dir)
                .variable("GIT_DIR", &public)
                .variable("GIT_WORK_TREE", &dir)
                .variable("GIT_INDEX_FILE", public.join("index"))
                .variable("GIT_COMMON_DIR", &public)
                .variable("GIT_OBJECT_DIRECTORY", public.join("objects"))
                .variable("GIT_ALTERNATE_OBJECT_DIRECTORIES", public.join("objects"))
                .variable("GIT_REFERENCE_BACKEND", &references)
        };
        let added = leaving_public_git(&dir, located(&["dupe", "add", "."]));
        assert_eq!(added.end, End::Code(0), "{added:?}");
        assert_eq!(private_staging(s, &dir), expected);
        let committed = leaving_public_git(
            &dir,
            located(&[
                "-c",
                "maintenance.auto=false",
                "-c",
                "user.name=Scenario",
                "-c",
                "user.email=scenario@example.invalid",
                "dupe",
                "commit",
                "-qm",
                "located",
            ]),
        );
        assert_eq!(committed.end, End::Code(0), "{committed:?}");
        let count = s
            .private(&dir)
            .git(["rev-list", "--count", "HEAD"])
            .succeeds();
        assert_eq!(count.stdout, b"2\n");
        // An object only the public repository holds is no object of the private one.
        let readme = s.git(["rev-parse", "HEAD:README.md"]).from(&dir).succeeds();
        let readme = String::from_utf8(readme.stdout).unwrap();
        let readme = readme.trim_end();
        let unlocated = s.git(["dupe", "cat-file", "-e", readme]).from(&dir).run();
        assert_ne!(unlocated.end, End::Code(0), "{unlocated:?}");
        let alternate = s
            .private(&dir)
            .git(["cat-file", "-e", readme])
            .variable("GIT_ALTERNATE_OBJECT_DIRECTORIES", public.join("objects"))
            .run();
        assert_eq!(alternate.end, End::Code(0), "{alternate:?}");
        let output = located(&["dupe", "cat-file", "-e", readme]).run();
        assert_eq!(output.end, unlocated.end, "{output:?}");
    });
}

/// A caller's `GIT_REFERENCE_BACKEND` naming the public `.git`, where the public repository
/// has no commit yet, reaches no private run: `git dupe commit` makes the private commit and
/// writes no public reference, under the releases that read the variable as under the rest.
#[test]
fn a_callers_reference_location_writes_no_public_reference() {
    under_each_release(|s| {
        let dir = s.dir().join("unborn");
        fs::create_dir(&dir).unwrap();
        s.git(["init", "-q", "-b", "main"]).from(&dir).succeeds();
        s.init(&dir);
        write(&dir, "notes.md", b"kept\n");
        let mut references = OsString::from("files://");
        references.push(dir.join(".git"));
        let located = |words: &[&str]| {
            s.git(words)
                .from(&dir)
                .variable("GIT_REFERENCE_BACKEND", &references)
        };
        let added = leaving_public_git(&dir, located(&["dupe", "add", "notes.md"]));
        assert_eq!(added.end, End::Code(0), "{added:?}");
        let committed = leaving_public_git(
            &dir,
            located(&[
                "-c",
                "maintenance.auto=false",
                "-c",
                "user.name=Scenario",
                "-c",
                "user.email=scenario@example.invalid",
                "dupe",
                "commit",
                "-qm",
                "private",
            ]),
        );
        assert_eq!(committed.end, End::Code(0), "{committed:?}");
        let committed = s
            .private(&dir)
            .git(["ls-tree", "-r", "--name-only", "HEAD"])
            .succeeds();
        assert_eq!(committed.stdout, b"notes.md\n");
    });
}
