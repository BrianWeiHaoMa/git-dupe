//! Where a command that needs a workspace stands: outside any repository it ends as Git
//! does, and in a worktree git-dupe is not attached to it is refused naming
//! `git dupe clone`, `git dupe help clone`, and `git dupe init`, whatever the other
//! worktrees of the project hold. Nothing here is attached by git-dupe, `.git/dupe` being a
//! fixture, but the linked worktrees whose own `init` shows that each worktree is attached
//! on its own, and the worktrees the refusal's own commands attach.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Scenario, Tree, Worktree, as_git_spells_bytes, common_private_url, holds, locate_words,
    name, name_accepted, names, under_each_release, word_of,
};

/// `git dupe <words>` from `directory` must be the refusal of an unattached repository:
/// nothing on standard output, one `fatal:` line naming both ways to attach, `git dupe
/// clone` with `git dupe help clone` beside it and `git dupe init`, exit 128.
fn refused_as_unattached(s: &Scenario, directory: &Path, words: &[&OsStr]) -> Vec<u8> {
    let refusal = s
        .git([OsStr::new("dupe")].iter().chain(words))
        .from(directory)
        .run();
    assert!(refusal.stdout.is_empty(), "{words:?}: {refusal:?}");
    let line = refusal.only_line("fatal");
    for named in [
        &b"'git dupe clone <url>'"[..],
        b"'git dupe help clone'",
        b"'git dupe init'",
    ] {
        assert!(holds(line, named), "{words:?}: {refusal:?}");
    }
    assert_eq!(refusal.end, End::Code(128), "{words:?}: {refusal:?}");
    line.to_vec()
}

/// The refusal says the same in an unattached main worktree, a second machine's clone, as
/// in an unattached linked worktree, and each command it names, typed as it shows it there,
/// does what it says: `git dupe help clone` shows the URL of the main worktree's private
/// repository, `git dupe clone` of that URL in the linked worktree, or of the private remote
/// on the second machine, brings the private files, and `git dupe init` attaches an empty
/// private repository (G4, In use 5, In use 11).
#[test]
fn the_unattached_refusal_names_clone_and_init_and_each_does_what_it_says() {
    under_each_release(|s| {
        let machines = s.second_machine("named");
        let first = &machines.first;
        let main_line = refused_as_unattached(s, &machines.root, &[OsStr::new("status")]);
        let linked_line = refused_as_unattached(s, &first.linked, &[OsStr::new("status")]);
        assert_eq!(main_line, linked_line);

        let help = s
            .git(["dupe", "help", "clone"])
            .from(&first.linked)
            .succeeds();
        let url_form = br#""$(git rev-parse --git-common-dir)/dupe""#;
        assert!(holds(&help.stdout, url_form), "{help:?}");
        // The URL as a shell expands the form `help clone` shows, from the linked worktree.
        let url = common_private_url(s, &first.linked);
        let private_remote = first.private_remote.to_str().unwrap();
        let tracked = s.private(&first.root).git(["ls-files"]).succeeds().stdout;
        let tracked = String::from_utf8(tracked).unwrap();
        assert!(tracked.lines().count() > 1, "{tracked}");
        for (root, url) in [
            (&first.linked, url.as_str()),
            (&machines.root, private_remote),
        ] {
            s.git(["dupe", "clone", url]).from(root).succeeds();
            // Every private file, `.gitdupe` and the files it hides among them.
            for path in tracked.lines() {
                assert_eq!(
                    fs::read(root.join(path)).unwrap(),
                    fs::read(first.root.join(path)).unwrap(),
                    "{}: {path}",
                    root.display()
                );
            }
            s.git(["dupe", "status"]).from(root).succeeds();
        }

        let empty = s.dir().join("empty");
        s.repository(&empty);
        refused_as_unattached(s, &empty, &[OsStr::new("status")]);
        s.git(["dupe", "init"]).from(&empty).succeeds();
        assert!(empty.join(".git/dupe/HEAD").is_file());
        let log = s.git(["dupe", "log"]).from(&empty).run();
        assert_ne!(
            log.end,
            End::Code(0),
            "an empty private repository: {log:?}"
        );
        s.git(["dupe", "status"]).from(&empty).succeeds();
    });
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
    let typed = OsStr::new("status\nhint: typed\x1b[31m");
    under_each_release(|s| {
        let case = "workspace: a command word of bytes that are no UTF-8";
        let not_utf8 = word_of(s, b"\xff\xfe", case);
        let plain = s.dir().join("plain");
        let below = plain.join("sub/dir");
        // A newline in a directory below the root is inside the envelope of a repository
        // git-dupe is not attached to; its prefix answer holds `\n/`.
        let newline = plain.join("new\nline\n");
        let case = "workspace: a directory whose name holds a newline";
        let holds_newlines = name_accepted(s.dir(), b"new\nline\n", case);
        s.repository(&plain);
        fs::create_dir_all(&below).unwrap();
        if holds_newlines {
            fs::create_dir(&newline).unwrap();
        }
        let directories: Vec<&Path> = [&plain, &below, &newline]
            .into_iter()
            .filter(|directory| holds_newlines || *directory != &newline)
            .map(PathBuf::as_path)
            .collect();
        let before = Tree::of(&plain);

        let line = refused_as_unattached(s, &plain, &[OsStr::new("status")]);
        let status_text = s.git(["dupe", "status", "-h"]).run();
        assert_eq!(status_text.end, End::Code(0), "{status_text:?}");
        let mut private_git_dir = OsString::from("--git-dir=");
        private_git_dir.push(plain.join(".git/dupe"));
        let mut private_work_tree = OsString::from("--work-tree=");
        private_work_tree.push(&plain);
        for directory in &directories {
            let words = [OsStr::new("status"), OsStr::new("log"), typed];
            for word in words.into_iter().chain(not_utf8.as_deref()) {
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
        for directory in &directories {
            let refusal = s.git(["dupe", "clone", "."]).from(directory).run();
            assert!(refusal.stdout.is_empty(), "{refusal:?}");
            let line = refusal.only_line("fatal");
            assert!(!holds(line, b"git dupe init"), "{refusal:?}");
            names(line, b"git dupe git");
            assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
        }

        let changed = before.changed_in(&Tree::of(&plain));
        assert!(changed.is_empty(), "changed: {changed:?}");

        let deepest = directories[directories.len() - 1];
        let init = s.git(["dupe", "init"]).from(deepest).run();
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
fn a_linked_worktree_is_unattached_until_its_own_init_whatever_the_main_worktree_holds() {
    under_each_release(|s| {
        // The main worktree unattached, and one that a directory at `.git/dupe` makes
        // attached: its linked worktrees are unattached all the same, and the refusal
        // names no path of the main worktree's, one that is not UTF-8 included where the
        // filesystem holds such a name, and one of ASCII alone where it refuses it.
        let suffix: &[u8] = match name_accepted(
            s.dir(),
            b"caf\xe9",
            "workspace: a main worktree named by bytes that are no UTF-8",
        ) {
            true => b"-caf\xe9",
            false => b"-cafe",
        };
        let case = "workspace: a directory whose name holds a newline";
        let holds_newlines = name_accepted(s.dir(), b"new\nline\n", case);
        for (kind, attached) in [("plain", false), ("attached", true)] {
            let main = s
                .dir()
                .join(name([kind.as_bytes(), suffix].concat().as_slice()));
            let linked = s.dir().join(format!("linked-of-{kind}"));
            s.repository(&main);
            if attached {
                fs::create_dir(main.join(".git/dupe")).unwrap();
            }
            s.linked_worktree(&main, &linked);
            fs::create_dir(linked.join("sub")).unwrap();
            let newline = linked.join("new\nline\n");
            let sub = linked.join("sub");
            let mut directories = vec![&linked, &sub];
            if holds_newlines {
                fs::create_dir(&newline).unwrap();
                directories.push(&newline);
            }
            for directory in directories {
                let line = refused_as_unattached(s, directory, &[OsStr::new("status")]);
                assert!(
                    !holds(&line, &as_git_spells_bytes(&main)),
                    "{kind}: {}",
                    line.escape_ascii()
                );
            }
        }

        // A linked worktree attached by its own `init` while the main worktree is not:
        // each is attached or not on its own (G4).
        let main = s.dir().join("main");
        let linked = s.dir().join("linked");
        s.repository(&main);
        s.linked_worktree(&main, &linked);
        s.init(&linked);
        let own = Worktree::read(s, &linked);
        assert!(own.private_directory().join("HEAD").is_file());
        assert!(!main.join(".git/dupe").exists());
        s.git(["dupe", "status"]).from(&linked).succeeds();
        refused_as_unattached(s, &main, &[OsStr::new("status")]);
    });
}

#[test]
fn a_linked_worktree_of_a_repository_whose_git_directory_lies_elsewhere_is_a_worktree() {
    under_each_release(|s| {
        // The main working tree's `.git` entry is a file naming `admin`: the main worktree
        // is not one git-dupe attaches (G4), but its linked worktree is a worktree like
        // any other, refused naming `git dupe init` and no path until its own `init`.
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
        s.init(&linked);
        let own = Worktree::read(s, &linked);
        assert_eq!(own.common_directory, s.dir().join("admin"));
        assert!(own.private_directory().join("HEAD").is_file());
        s.git(["dupe", "status"])
            .from(&linked.join("sub"))
            .succeeds();
        refused_as_unattached(s, &main, &[OsStr::new("status")]);
    });
}
