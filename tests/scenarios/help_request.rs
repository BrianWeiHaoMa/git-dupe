//! Help requests after words Git runs: the selected repository, guards, and settle.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, lines_in_order, names, private_add, private_commit, region_rules,
    unchanged, under_each_release, warnings_in_any_order, write,
};

fn private_help(s: &Scenario, common: &Path, from: &Path, command: &str) -> Output {
    let mut git_dir = OsString::from("--git-dir=");
    git_dir.push(common.join("dupe"));
    // The harness starts with no GIT_WORK_TREE: this form must leave it absent.
    s.git(
        [
            OsString::from("-c"),
            OsString::from("help.autocorrect=0"),
            git_dir,
        ]
        .into_iter()
        .chain([command, "-h"].map(OsString::from)),
    )
    .from(from)
    .run()
}

#[test]
fn attached_help_runs_privately_and_settles() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        write(&dir, ".gitdupe", b"notes\n");
        write(&dir, "notes/keep", b"private\n");
        write(&dir, ".gitignore", b"!notes\n");
        for command in ["log", "stash"] {
            let gits = s
                .private(&dir)
                .git(["-c", "help.autocorrect=0", command, "-h"])
                .run();
            let ours = s.git(["dupe", command, "-h"]).from(&dir).run();
            assert_eq!(ours.end, gits.end, "{ours:?}, Git: {gits:?}");
            assert_eq!(ours.stdout, gits.stdout);
            assert!(
                ours.stderr.starts_with(&gits.stderr),
                "{ours:?}, Git: {gits:?}"
            );
            let tail = Output {
                stdout: vec![],
                stderr: ours.stderr[gits.stderr.len()..].to_vec(),
                end: ours.end,
            };
            names(tail.only_line("warning"), b"notes");
            warnings_in_any_order(&ours, &[&[b"notes", b".gitignore:1"]]);
            assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
        }
    });
}

#[test]
fn unattached_help_runs_privately_without_creating_or_settling() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.repository(&dir);
        let before = Tree::of(&dir);
        for command in ["log", "stash", "push", "pull", "fetch"] {
            let gits = s
                .private(&dir)
                .git(["-c", "help.autocorrect=0", command, "-h"])
                .run();
            let ours = s.git(["dupe", command, "-h"]).from(&dir).run();
            assert_eq!(ours, gits);
            unchanged(&before, &dir);
            assert!(!dir.join(".git/dupe").exists());
        }
        for words in [
            &["dupe", "log", "--", "-h"][..],
            &["dupe", "push", "origin"],
        ] {
            let refusal = s.git(words).from(&dir).run();
            assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
            assert!(refusal.stdout.is_empty());
            names(refusal.only_line("fatal"), b"git dupe init");
            unchanged(&before, &dir);
        }
    });
}

#[test]
fn linked_worktree_help_runs_where_no_repository_exists() {
    under_each_release(|s| {
        // The main working tree attached with private history; its linked worktree has
        // none. A word Git reads as a value rather than as `-h` must not act on that
        // history: `commit -am -h` would commit the deletion of every private file.
        let main = s.dir().join("main");
        let linked = s.dir().join("linked");
        s.attached_project(&main);
        write(&main, "notes", b"private\n");
        private_add(s, &main, "notes");
        private_commit(s, &main);
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "s@example.invalid"),
        ] {
            s.private(&main).git(["config", key, value]).succeeds();
        }
        s.linked_worktree(&main, &linked);
        let asked = s
            .git(["rev-parse", "--absolute-git-dir"])
            .from(&linked)
            .succeeds();
        let git_directory =
            Path::new(OsStr::from_bytes(asked.stdout.strip_suffix(b"\n").unwrap())).to_path_buf();
        let private = main.join(".git/dupe");
        let before = Tree::of(&private);
        for words in [&["commit", "-am", "-h"][..], &["log", "-h"]] {
            let mut git_dir = OsString::from("--git-dir=");
            git_dir.push(git_directory.join("dupe"));
            let mut work_tree = OsString::from("--work-tree=");
            work_tree.push(&linked);
            let gits = s
                .git(
                    [OsString::from("-c"), OsString::from("help.autocorrect=0")]
                        .into_iter()
                        .chain([git_dir, work_tree])
                        .chain(words.iter().map(OsString::from)),
                )
                .from(&linked)
                .run();
            let ours = s.git(["dupe"].iter().chain(words)).from(&linked).run();
            assert_eq!(ours, gits, "{words:?}");
            unchanged(&before, &private);
            assert!(!git_directory.join("dupe").exists());
        }

        // Attached on its own, the linked worktree's help runs against its own private
        // repository, and settles it; the main worktree's history is never reached.
        s.init(&linked);
        let linked_private = s.private_at(&linked, &git_directory.join("dupe"));
        let gits = linked_private
            .git(["-c", "help.autocorrect=0", "log", "-h"])
            .run();
        let ours = s.git(["dupe", "log", "-h"]).from(&linked).run();
        assert_eq!(ours.end, gits.end, "{ours:?}, Git: {gits:?}");
        assert_eq!(ours.stdout, gits.stdout);
        assert_eq!(ours.stderr, gits.stderr);
        unchanged(&before, &private);
        assert!(git_directory.join("dupe/HEAD").is_file());
    });
}

#[test]
fn help_inside_a_linked_worktrees_git_directory_runs_where_no_repository_exists() {
    under_each_release(|s| {
        // From `.git/worktrees/<name>` the locate fails after naming that Git directory
        // and the main one. Neither a word Git reads as a value nor a private alias may
        // reach the main working tree's private history from there.
        let main = s.dir().join("main");
        let linked = s.dir().join("linked");
        s.attached_project(&main);
        write(&main, "notes", b"private\n");
        private_add(s, &main, "notes");
        private_commit(s, &main);
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "s@example.invalid"),
            ("alias.note", "tag -m"),
        ] {
            s.private(&main).git(["config", key, value]).succeeds();
        }
        s.linked_worktree(&main, &linked);
        let asked = s
            .git(["rev-parse", "--absolute-git-dir"])
            .from(&linked)
            .succeeds();
        let git_directory =
            Path::new(OsStr::from_bytes(asked.stdout.strip_suffix(b"\n").unwrap())).to_path_buf();
        let private = main.join(".git/dupe");
        let before = Tree::of(&private);
        for words in [
            &["tag", "-m", "-h", "t"][..],
            &["note", "-h", "t"],
            &["log", "-h"],
        ] {
            let mut git_dir = OsString::from("--git-dir=");
            git_dir.push(git_directory.join("dupe"));
            let gits = s
                .git(
                    [
                        OsString::from("-c"),
                        OsString::from("help.autocorrect=0"),
                        git_dir,
                    ]
                    .into_iter()
                    .chain(words.iter().map(OsString::from)),
                )
                .from(&git_directory)
                .run();
            let ours = s
                .git(["dupe"].iter().chain(words))
                .from(&git_directory)
                .run();
            assert_eq!(ours, gits, "{words:?}");
            unchanged(&before, &private);
            assert!(!git_directory.join("dupe").exists());
        }
    });
}

#[test]
fn outside_help_is_gits_run_in_the_received_environment() {
    under_each_release(|s| {
        let before = Tree::of(s.dir());
        for command in ["log", "stash"] {
            let gits = s.git(["-c", "help.autocorrect=0", command, "-h"]).run();
            let ours = s.git(["dupe", command, "-h"]).run();
            assert_eq!(ours, gits);
            unchanged(&before, s.dir());
        }
    });
}

#[test]
fn help_inside_git_directories_never_reads_their_public_aliases() {
    under_each_release(|s| {
        let bare = s.dir().join("bare.git");
        let ordinary = s.dir().join("ordinary");
        s.bare_repository(&bare);
        s.repository(&ordinary);
        for common in [&bare, &ordinary.join(".git")] {
            s.git(["config", "alias.walk", "!git config walk.ran yes"])
                .from(common)
                .succeeds();
            let before = Tree::of(common);
            for command in ["walk", "log"] {
                let gits = private_help(s, common, common, command);
                let ours = s.git(["dupe", command, "-h"]).from(common).run();
                assert_eq!(ours, gits);
                if command == "walk" {
                    assert_eq!(ours.end, End::Code(1));
                }
                let ran = s.git(["config", "--get", "walk.ran"]).from(common).run();
                assert_eq!(ran.end, End::Code(1), "{ran:?}");
                assert!(ran.stdout.is_empty());
                unchanged(&before, common);
            }
        }
    });
}

#[test]
fn help_inside_an_attached_git_directory_reads_only_the_private_aliases() {
    under_each_release(|s| {
        let dir = s.dir().join("workspace");
        s.attached_project(&dir);
        let common = dir.join(".git");
        let before = Tree::of(&dir);
        let gits = private_help(s, &common, &common, "log");
        let ours = s.git(["dupe", "log", "-h"]).from(&common).run();
        assert_eq!(ours, gits);
        unchanged(&before, &dir);
        // The private repository's own aliases are resolved there: one passed through
        // runs as Git runs it, and one that reaches `status` asks for its text.
        s.private(&dir)
            .git(["config", "alias.walk", "log"])
            .succeeds();
        s.private(&dir)
            .git(["config", "alias.sh", "status -h"])
            .succeeds();
        let before = Tree::of(&dir);
        let gits = private_help(s, &common, &common, "walk");
        let ours = s.git(["dupe", "walk", "-h"]).from(&common).run();
        assert_eq!(ours, gits);
        let text = s.git(["dupe", "status", "-h"]).run();
        assert_eq!(text.end, End::Code(0), "{text:?}");
        assert_eq!(s.git(["dupe", "sh"]).from(&common).run(), text);
        unchanged(&before, &dir);
    });
}

#[test]
fn stash_untracked_help_is_refused_without_stashing_or_changing_files() {
    under_each_release(|s| {
        let attached = s.dir().join("attached");
        let unattached = s.dir().join("unattached");
        s.attached_project(&attached);
        s.repository(&unattached);
        // Settle has already reached a steady state in the attached fixture.
        for place in [&attached, &unattached, s.dir()] {
            write(place, "untracked", b"keep\n");
            let before = Tree::of(place);
            let refusal = s.git(["dupe", "stash", "-u", "-h"]).from(place).run();
            assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
            assert!(refusal.stdout.is_empty());
            let line = refusal.only_line("fatal");
            names(line, b"git dupe add");
            names(line, b"git dupe stash");
            lines_in_order(&refusal, "fatal", &[b"git dupe add"]);
            let text = String::from_utf8_lossy(line);
            assert!(text.find("git dupe add").unwrap() < text.find("git dupe stash").unwrap());
            unchanged(&before, place);
        }
    });
}
