//! Where a command that needs a workspace stands: outside any repository it ends as Git
//! does, and in a repository git-dupe is not attached to it is refused naming
//! `git dupe init`. Nothing here is attached by git-dupe: `.git/dupe` is a fixture.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{End, Scenario, Tree, holds, locate_words, names, under_each_release};

/// Whether `line` names the main working tree itself, and no path below it.
fn names_the_main_working_tree(line: &[u8], main: &Path) -> bool {
    let main = main.as_os_str().as_bytes();
    holds(line, main) && !holds(line, &[main, b"/"].concat())
}

/// `git dupe <words>` from `directory` must be the refusal of an unattached repository:
/// nothing on standard output, one `fatal:` line naming `git dupe init`, exit 128.
fn refused_as_unattached(s: &Scenario, directory: &Path, words: &[&OsStr]) -> Vec<u8> {
    let refusal = s
        .git([OsStr::new("dupe")].iter().chain(words))
        .from(directory)
        .run();
    assert!(refusal.stdout.is_empty(), "{words:?}: {refusal:?}");
    let line = refusal.only_line("fatal");
    assert!(holds(line, b"git dupe init"), "{words:?}: {refusal:?}");
    assert_eq!(refusal.end, End::Code(128), "{words:?}: {refusal:?}");
    line.to_vec()
}

#[test]
fn where_git_finds_no_working_tree_the_command_ends_with_gits_message_and_status() {
    under_each_release(|s| {
        let plain = s.dir().join("plain");
        let bare = s.dir().join("bare.git");
        s.repository(&plain);
        s.bare_repository(&bare);
        for place in [s.dir(), &bare, &plain.join(".git")] {
            let gits = s.git(locate_words(false)).from(place).run();
            assert_ne!(gits.end, End::Code(0), "{}: {gits:?}", place.display());
            assert!(!gits.stderr.is_empty(), "{}: {gits:?}", place.display());
            // `init` and `clone` included.
            for words in [&["status"][..], &["log"], &["init"], &["clone", "x"]] {
                let ours = s.git(["dupe"].iter().chain(words)).from(place).run();
                assert!(ours.stdout.is_empty(), "{words:?}: {ours:?}");
                assert_eq!(ours.stderr, gits.stderr, "{words:?}: {ours:?}");
                assert_eq!(ours.end, gits.end, "{words:?}: {ours:?}");
            }
        }
    });
}

#[test]
fn an_unattached_repository_is_refused_naming_git_dupe_init_from_any_directory() {
    let not_utf8 = OsStr::from_bytes(b"\xff\xfe");
    let typed = OsStr::new("status\nhint: typed\x1b[31m");
    under_each_release(|s| {
        let plain = s.dir().join("plain");
        let below = plain.join("sub/dir");
        // A newline in a directory below the root is inside the envelope of a repository
        // git-dupe is not attached to; its prefix answer holds `\n/`.
        let newline = plain.join("new\nline\n");
        s.repository(&plain);
        fs::create_dir_all(&below).unwrap();
        fs::create_dir(&newline).unwrap();
        let before = Tree::of(&plain);

        let line = refused_as_unattached(s, &plain, &[OsStr::new("status")]);
        let status_text = s.git(["dupe", "status", "-h"]).run();
        assert_eq!(status_text.end, End::Code(0), "{status_text:?}");
        let mut private_git_dir = OsString::from("--git-dir=");
        private_git_dir.push(plain.join(".git/dupe"));
        let mut private_work_tree = OsString::from("--work-tree=");
        private_work_tree.push(&plain);
        for directory in [&plain, &below, &newline] {
            for word in [OsStr::new("status"), OsStr::new("log"), not_utf8, typed] {
                assert_eq!(refused_as_unattached(s, directory, &[word]), line);
                // `-h` after a command whose help is git-dupe's is that help, whatever the
                // workspace (G4); a passthrough word asks Git in the private repository,
                // even though it does not exist (G24).
                let help = [word, OsStr::new("-h")];
                if word == "status" {
                    let answered = s
                        .git([OsStr::new("dupe")].iter().chain(&help))
                        .from(directory)
                        .run();
                    assert_eq!(answered, status_text, "{help:?}");
                } else {
                    let answered = s
                        .git([OsStr::new("dupe")].iter().chain(&help))
                        .from(directory)
                        .run();
                    let gits = s
                        .git(
                            [
                                OsStr::new("-c"),
                                OsStr::new("help.autocorrect=0"),
                                &private_git_dir,
                                &private_work_tree,
                            ]
                            .iter()
                            .chain(&help),
                        )
                        .from(directory)
                        .run();
                    assert_eq!(answered, gits, "{help:?}");
                }
            }
        }

        // Git's options before `dupe` name the repository as Git read them.
        let mut git_dir = OsString::from("--git-dir=");
        git_dir.push(plain.join(".git"));
        let mut work_tree = OsString::from("--work-tree=");
        work_tree.push(&plain);
        let from_outside: [&[&OsStr]; 2] = [
            &[OsStr::new("-C"), plain.as_os_str()],
            &[&git_dir, &work_tree],
        ];
        for options in from_outside {
            let refusal = s
                .git(options.iter().chain(&["dupe", "status"].map(OsStr::new)))
                .run();
            assert!(refusal.stdout.is_empty(), "{options:?}: {refusal:?}");
            assert_eq!(refusal.only_line("fatal"), line, "{options:?}: {refusal:?}");
            assert_eq!(refusal.end, End::Code(128), "{options:?}: {refusal:?}");
        }

        // The commands that attach are not told to attach first: `clone` gets as far as
        // its own refusals, here of a word naming the project's working tree, read from
        // the root wherever it is typed, and `init` attaches.
        for directory in [&plain, &below, &newline] {
            let refusal = s.git(["dupe", "clone", "."]).from(directory).run();
            assert!(refusal.stdout.is_empty(), "{refusal:?}");
            let line = refusal.only_line("fatal");
            assert!(!holds(line, b"git dupe init"), "{refusal:?}");
            names(line, b"git dupe git");
            assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
        }

        let changed = before.changed_in(&Tree::of(&plain));
        assert!(changed.is_empty(), "changed: {changed:?}");

        let init = s.git(["dupe", "init"]).from(&newline).run();
        assert_eq!(init.end, End::Code(0), "{init:?}");
        assert!(init.lines("fatal").is_empty(), "{init:?}");
        assert!(plain.join(".git/dupe/HEAD").is_file(), "{init:?}");
    });
}

#[test]
fn a_file_at_the_private_repositorys_path_attaches_nothing() {
    under_each_release(|s| {
        let plain = s.dir().join("plain");
        s.repository(&plain);
        fs::write(plain.join(".git/dupe"), "not a directory\n").unwrap();
        refused_as_unattached(s, &plain, &[OsStr::new("status")]);
    });
}

#[test]
fn a_linked_worktree_is_never_attached_and_its_refusal_names_the_main_working_tree() {
    under_each_release(|s| {
        // The main working tree unattached, and one that a directory at `.git/dupe`
        // makes attached: its linked worktree is unattached all the same.
        for (name, attached) in [("plain", false), ("attached", true)] {
            let main = s.dir().join(name);
            // Named so that the main working tree's path is no part of the linked one's.
            let linked = s.dir().join(format!("linked-of-{name}"));
            s.repository(&main);
            if attached {
                fs::create_dir(main.join(".git/dupe")).unwrap();
            }
            s.linked_worktree(&main, &linked);
            fs::create_dir(linked.join("sub")).unwrap();
            fs::create_dir(linked.join("new\nline\n")).unwrap();
            for directory in [&linked, &linked.join("sub"), &linked.join("new\nline\n")] {
                let line = refused_as_unattached(s, directory, &[OsStr::new("status")]);
                assert!(
                    names_the_main_working_tree(&line, &main),
                    "{name}: {}",
                    line.escape_ascii()
                );
            }
        }
    });
}

#[test]
fn a_linked_worktree_whose_main_git_directory_lies_elsewhere_is_refused_naming_no_path() {
    under_each_release(|s| {
        // The main working tree's `.git` entry is a file naming `admin`: the common Git
        // directory's parent is no working tree, and Git's own `worktree list` names
        // `admin` itself. Where Git does not know the main working tree, none is named.
        let main = s.dir().join("main");
        let linked = s.dir().join("linked");
        let mut separate = OsString::from("--separate-git-dir=");
        separate.push(s.dir().join("admin"));
        s.repository(&main);
        let moved = s
            .git([OsStr::new("init"), OsStr::new("-q"), &separate])
            .from(&main)
            .run();
        assert_eq!(moved.end, End::Code(0), "{moved:?}");
        s.linked_worktree(&main, &linked);
        fs::create_dir(linked.join("sub")).unwrap();

        refused_as_unattached(s, &main, &[OsStr::new("status")]);
        for directory in [&linked, &linked.join("sub")] {
            let line = refused_as_unattached(s, directory, &[OsStr::new("status")]);
            assert!(!line.contains(&b'/'), "{}", line.escape_ascii());
        }
    });
}

#[test]
fn a_path_that_is_not_utf8_is_named_byte_for_byte() {
    under_each_release(|s| {
        let main = s.dir().join(OsStr::from_bytes(b"caf\xe9/main"));
        let linked = s.dir().join(OsStr::from_bytes(b"caf\xe9/linked"));
        s.repository(&main);
        s.linked_worktree(&main, &linked);

        refused_as_unattached(s, &main, &[OsStr::new("status")]);
        let line = refused_as_unattached(s, &linked, &[OsStr::new("status")]);
        assert!(
            names_the_main_working_tree(&line, &main),
            "{}",
            line.escape_ascii()
        );
    });
}
