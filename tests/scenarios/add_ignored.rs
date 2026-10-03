//! `add` of a file an ignore rule hides: the private repository's ignore rules are those
//! of any repository, and Git's own `add` answers (G15, F2).

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use crate::harness::{
    End, private_commit, privately_tracked, run_traced, under_each_release, write,
};

/// A file the project ignores is added with `-f` once: tracked privately, its later
/// changes are Git's to take like any tracked file's, `commit -a` among them (G15).
#[test]
fn a_forced_ignored_file_has_its_edits_committed_by_commit_all_without_force() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        write(&dir, ".env.local", b"first\n");
        s.git(["dupe", "add", "-f", ".env.local"])
            .from(&dir)
            .succeeds();
        private_commit(s, &dir);

        write(&dir, ".env.local", b"edited\n");
        let identity = [
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "-c",
            "maintenance.auto=false",
        ];
        s.git(
            identity
                .iter()
                .chain(&["dupe", "commit", "-a", "-m", "edited"]),
        )
        .from(&dir)
        .succeeds();
        assert_eq!(
            s.private(&dir)
                .git(["show", "HEAD:.env.local"])
                .succeeds()
                .stdout,
            b"edited\n"
        );
    });
}

/// A file ignored only by `.gitignore`, only by the global excludes file, or only by the
/// private repository's `info/exclude` is refused as Git's `add` refuses it, with Git's
/// own message and exit status, and is staged with `-f`. A file ignored only by the
/// user's part of the public `.git/info/exclude` is no ignored file to the private
/// repository, whose `info/exclude` is its own, and is staged as written.
#[test]
fn each_ignore_source_of_the_private_repository_keeps_gits_answer_until_forced() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let global = s.dir().join("global-excludes");
        fs::write(&global, b"global.txt\n").unwrap();
        s.git([
            OsStr::new("config"),
            OsStr::new("--global"),
            OsStr::new("core.excludesFile"),
            global.as_os_str(),
        ])
        .succeeds();
        let private_exclude = dir.join(".git/dupe/info/exclude");
        fs::create_dir_all(private_exclude.parent().unwrap()).unwrap();
        fs::write(&private_exclude, b"private.txt\n").unwrap();
        let public_exclude = dir.join(".git/info/exclude");
        let mut user_part = fs::read(&public_exclude).unwrap();
        user_part.extend_from_slice(b"public.txt\n");
        fs::write(&public_exclude, user_part).unwrap();
        for path in [".env.local", "global.txt", "private.txt", "public.txt"] {
            write(&dir, path, b"private\n");
        }

        for path in [".env.local", "global.txt", "private.txt"] {
            let (ours, runs) = run_traced(
                s.git(["dupe", "add", path]).from(&dir),
                &s.dir().join("add.trace"),
            );
            let own = runs.own();
            let adds: Vec<&Vec<Vec<u8>>> = own
                .words()
                .iter()
                .zip(own.commands())
                .filter(|(_, command)| *command == b"add")
                .map(|(words, _)| words)
                .collect();
            assert_eq!(adds.len(), 1, "{path}: {runs:?}");
            // The same words, given to the release's own Git in the private repository
            // from the same directory, which the refused run left as it was.
            let words = adds[0].iter().map(|word| OsStr::from_bytes(word));
            let gits = s.private(&dir).git(words).run();
            assert_ne!(gits.end, End::Code(0), "{path} is not ignored: {gits:?}");
            assert_eq!(ours.end, gits.end, "{path}: {ours:?}; Git: {gits:?}");
            assert_eq!(ours.stderr, gits.stderr, "{path}: {ours:?}; Git: {gits:?}");
            assert_eq!(ours.stdout, gits.stdout, "{path}: {ours:?}; Git: {gits:?}");
            let path_bytes = path.as_bytes().to_vec();
            assert!(!privately_tracked(s, &dir).contains(&path_bytes), "{path}");
            assert!(!dir.join(".gitdupe").exists(), "{path}");

            s.git(["dupe", "add", "-f", path]).from(&dir).succeeds();
            assert!(privately_tracked(s, &dir).contains(&path_bytes), "{path}");
        }

        let output = s.git(["dupe", "add", "public.txt"]).from(&dir).run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(privately_tracked(s, &dir).contains(&b"public.txt".to_vec()));
    });
}
