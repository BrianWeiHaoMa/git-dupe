//! The private repository as the ordinary Git repository it is: plain Git reads it,
//! clones it, and fetches and pushes from it; its own status and commit template list no
//! file of the project unless asked to, and give no hint; the project's hooks do not run
//! for private work (F2).

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, daily_edited, daily_state, holds, under_each_release, write_executable,
};

/// Plain Git, given the private Git directory and nothing else.
fn plain(s: &Scenario, dir: &Path, words: &[&str]) -> Output {
    let private = dir.join(".git/dupe");
    s.git(
        [OsStr::new("--git-dir"), private.as_os_str()]
            .into_iter()
            .chain(words.iter().map(OsStr::new)),
    )
    .from(dir)
    .run()
}

#[test]
fn plain_git_reads_clones_fetches_and_pushes_from_the_private_repository() {
    under_each_release(|s| {
        let dir = s.dir().join("project");
        daily_state(s, &dir);
        let private = dir.join(".git/dupe");
        let head = s.private(&dir).git(["rev-parse", "HEAD"]).succeeds().stdout;

        let log = plain(s, &dir, &["log", "--format=%H", "-1"]);
        assert_eq!(log.end, End::Code(0), "{log:?}");
        assert_eq!(log.stdout, head, "{log:?}");

        let copy = s.dir().join("copy");
        s.git([
            OsStr::new("clone"),
            OsStr::new("-q"),
            private.as_os_str(),
            copy.as_os_str(),
        ])
        .succeeds();
        let copied = s.git(["rev-parse", "HEAD"]).from(&copy).succeeds();
        assert_eq!(copied.stdout, head);
        assert_eq!(fs::read(copy.join("notes/a.md")).unwrap(), b"private\n");
        assert_eq!(
            fs::read(copy.join(".gitdupe")).unwrap(),
            b"notes\n.vscode\n"
        );

        let other = s.dir().join("other");
        s.repository(&other);
        s.git([
            OsStr::new("fetch"),
            OsStr::new("-q"),
            private.as_os_str(),
            OsStr::new("main"),
        ])
        .from(&other)
        .succeeds();
        let fetched = s.git(["rev-parse", "FETCH_HEAD"]).from(&other).succeeds();
        assert_eq!(fetched.stdout, head);

        let bare = s.dir().join("bare.git");
        s.bare_repository(&bare);
        let bare_word = bare.to_str().unwrap();
        let pushed = plain(s, &dir, &["push", "-q", bare_word, "main"]);
        assert_eq!(pushed.end, End::Code(0), "{pushed:?}");
        let arrived = s.git(["rev-parse", "main"]).from(&bare).succeeds();
        assert_eq!(arrived.stdout, head);
    });
}

#[test]
fn its_own_status_and_template_list_no_project_file_unless_asked_and_no_public_hook_runs() {
    under_each_release(|s| {
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
        ] {
            s.git(["config", "--global", key, value]).succeeds();
        }
        let dir = daily_edited(s, "project");
        // README.md and docs/design.md are the project's, untracked in the private
        // repository.
        let status = plain(s, &dir, &["status"]);
        assert_eq!(status.end, End::Code(0), "{status:?}");
        assert!(holds(&status.stdout, b".env.local"), "{status:?}");
        assert!(!holds(&status.stdout, b"README.md"), "{status:?}");
        let asked = plain(s, &dir, &["status", "--untracked-files=normal"]);
        assert!(holds(&asked.stdout, b"README.md"), "{asked:?}");
        let hinted = plain(s, &dir, &["-c", "advice.statusHints=true", "status"]);
        assert_ne!(hinted.stdout, status.stdout, "{hinted:?}");

        // The status in the commit template, captured by an editor that leaves the
        // message empty, so that no commit is made.
        s.private(&dir).git(["add", ".env.local"]).succeeds();
        let template = |name: &str, words: &[&str]| -> Vec<u8> {
            let capture = s.dir().join(name);
            let editor = format!("cat >> '{}' <", capture.display());
            let words: Vec<&str> = words.iter().copied().chain(["commit"]).collect();
            let private = dir.join(".git/dupe");
            let output = s
                .git(
                    [OsStr::new("--git-dir"), private.as_os_str()]
                        .into_iter()
                        .chain(words.iter().map(OsStr::new)),
                )
                .from(&dir)
                .variable("GIT_EDITOR", &editor)
                .run();
            assert_ne!(output.end, End::Code(0), "{output:?}");
            fs::read(&capture).unwrap()
        };
        let own = template("template", &[]);
        assert!(holds(&own, b".env.local"), "{}", own.escape_ascii());
        assert!(!holds(&own, b"README.md"), "{}", own.escape_ascii());
        let asked = template(
            "template-asked",
            &["-c", "status.showUntrackedFiles=normal"],
        );
        assert!(holds(&asked, b"README.md"), "{}", asked.escape_ascii());

        // A public hook that refuses every commit refuses the project's, and is not run
        // for a private one.
        let hook = dir.join(".git/hooks/pre-commit");
        write_executable(s, &hook, b"#!/bin/sh\nexit 1\n");
        let public = s
            .git(["commit", "-q", "--allow-empty", "-m", "public"])
            .from(&dir)
            .run();
        assert_ne!(public.end, End::Code(0), "{public:?}");
        let before = s.private(&dir).git(["rev-parse", "HEAD"]).succeeds().stdout;
        let private = s
            .git(["dupe", "commit", "-q", "-m", "private"])
            .from(&dir)
            .run();
        assert_eq!(private.end, End::Code(0), "{private:?}");
        let after = s.private(&dir).git(["rev-parse", "HEAD"]).succeeds().stdout;
        assert_ne!(after, before);
    });
}
