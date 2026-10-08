//! Each worktree attaches independently and every command uses its own private state
//! (G4, F2, F3, G5, G10, G19, G24, G28, S17).

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, Worktree, held_publicly, holds, names, names_number, records,
    region_in, unchanged, under_each_release, write,
};

fn refused(output: &Output, main: &Path) {
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let fatal = output.only_line("fatal");
    names(fatal, b"git dupe init");
    assert!(!holds(fatal, main.as_os_str().as_bytes()), "{output:?}");
}

/// One command's public-state boundary, including the other private repositories.
fn run(s: &Scenario, wt: &Worktree, from: &Path, words: &[&str]) -> Output {
    let before = wt.public_git();
    let output = s.git(words).from(from).run();
    let changed = before.changed_in(&wt.public_git());
    assert!(
        changed.is_empty(),
        "{words:?}: {output:?}, changed {changed:?}"
    );
    output
}

fn success(s: &Scenario, wt: &Worktree, from: &Path, words: &[&str]) -> Output {
    let output = run(s, wt, from, words);
    assert_eq!(output.end, End::Code(0), "{words:?}: {output:?}");
    assert!(output.lines("fatal").is_empty(), "{output:?}");
    output
}

#[test]
fn unattached_linked_worktrees_refuse_every_route_but_keep_attachment_free_commands() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.repository(&main);
        s.init(&main);
        s.git(["config", "--global", "alias.st", "status"])
            .succeeds();
        s.private(&main)
            .git(["config", "alias.main-only", "status"])
            .succeeds();
        let plain = s.dir().join("plain");
        s.repository(&plain);
        for name in ["one", "two"] {
            let root = s.dir().join(name);
            s.linked_worktree(&main, &root);
            let wt = Worktree::read(s, &root);
            let sub = root.join("sub");
            fs::create_dir(&sub).unwrap();
            for from in [&root, &sub] {
                for words in [
                    &["dupe", "hide", "x"][..],
                    &["dupe", "status"],
                    &["dupe", "add", "-A"],
                    &["dupe", "stash", "-u"],
                    &["dupe", "log"],
                    &["dupe", "git", "log"],
                    &["dupe", "st"],
                    &["dupe", "main-only"],
                ] {
                    refused(&run(s, &wt, from, words), &main);
                }
                for words in [
                    &["dupe", "help"][..],
                    &["dupe", "-h"],
                    &["dupe", "status", "-h"],
                    &["dupe", "--version"],
                    &["dupe"],
                ] {
                    let expected = s.git(words).from(&plain).run();
                    let actual = run(s, &wt, from, words);
                    assert_eq!(actual.end, expected.end, "{words:?}: {actual:?}");
                    assert_eq!(actual.stdout, expected.stdout, "{words:?}: {actual:?}");
                }
            }
            assert!(!wt.private_directory().exists());
            assert!(wt.region().is_none());
        }
    });
}

#[test]
fn linked_inits_create_distinct_private_directories_and_preserve_the_main_region() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.repository(&main);
        s.init(&main);
        s.git(["dupe", "hide", "main-hidden"])
            .from(&main)
            .succeeds();
        let main_wt = Worktree::read(s, &main);
        let main_region = main_wt.region_bytes();
        let main_private = Tree::of(&main_wt.private_directory());
        let mut worktrees = vec![main_wt.clone()];
        for name in ["one", "two"] {
            let root = s.dir().join(name);
            s.linked_worktree(&main, &root);
            worktrees.push(Worktree::read(s, &root));
        }
        for (index, wt) in worktrees.iter().enumerate().skip(1) {
            // Ask this Git for its init line at this exact directory, then restore the
            // unattached fixture so git-dupe must create it itself.
            let expected = wt.private(s).git(["init"]).succeeds();
            fs::remove_dir_all(wt.private_directory()).unwrap();
            let init = success(s, wt, &wt.root, &["dupe", "init"]);
            assert_eq!(init.stdout, expected.stdout, "{init:?}");
            assert!(init.lines("warning").is_empty(), "{init:?}");
            assert!(wt.region().unwrap().rules.contains(&b"/.gitdupe".to_vec()));
            success(
                s,
                wt,
                &wt.root,
                &["dupe", "hide", wt.name().unwrap().to_str().unwrap()],
            );
            assert_eq!(main_wt.region_bytes(), main_region);
            unchanged(&main_private, &main_wt.private_directory());
            if index == 1 {
                refused(
                    &run(s, &worktrees[2], &worktrees[2].root, &["dupe", "status"]),
                    &main,
                );
            }
        }
        let mut worktrees_config = Vec::new();
        for wt in &worktrees {
            assert!(wt.private_directory().is_dir());
            assert!(wt.private_directory().join("HEAD").is_file());
            assert!(wt.private_directory().join("index").is_file());
            let config = wt
                .private(s)
                .git(["config", "core.worktree"])
                .succeeds()
                .stdout;
            let relative = Path::new(OsStr::from_bytes(config.trim_ascii_end()));
            assert!(!relative.is_absolute());
            assert_eq!(
                fs::canonicalize(wt.private_directory().join(relative)).unwrap(),
                wt.root
            );
            assert!(!worktrees_config.contains(&config));
            worktrees_config.push(config);
            assert_eq!(
                wt.private(s).git(["ls-files", "-z"]).succeeds().stdout,
                b".gitdupe\0"
            );
        }
        assert_eq!(worktrees_config[0], b"../..\n");
        assert_eq!(
            main_wt.private_directory(),
            main_wt.common_directory.join("dupe")
        );
        assert_ne!(
            worktrees[1].private_directory(),
            worktrees[2].private_directory()
        );
    });
}

#[test]
fn bare_linked_worktrees_attach_without_a_main_region() {
    under_each_release(|s| {
        let project = s.dir().join("project");
        s.repository(&project);
        let bare = s.dir().join("bare");
        s.git([
            OsStr::new("clone"),
            OsStr::new("--bare"),
            project.as_os_str(),
            bare.as_os_str(),
        ])
        .succeeds();
        let mut worktrees = Vec::new();
        for name in ["one", "two"] {
            let root = s.dir().join(name);
            s.linked_worktree(&bare, &root);
            let wt = Worktree::read(s, &root);
            success(s, &wt, &root, &["dupe", "init"]);
            assert_eq!(wt.common_directory, bare);
            assert_eq!(
                wt.private_directory(),
                bare.join("worktrees").join(wt.name().unwrap()).join("dupe")
            );
            assert!(wt.private_directory().is_dir());
            assert!(wt.region().unwrap().rules.contains(&b"/.gitdupe".to_vec()));
            worktrees.push(wt);
        }
        assert!(region_in(&bare, None).is_none());
        assert_ne!(
            worktrees[0].private_directory(),
            worktrees[1].private_directory()
        );
        assert_eq!(
            records(&fs::read(bare.join("info/exclude")).unwrap(), b'\n')
                .iter()
                .filter(|line| line.starts_with(b"# BEGIN git-dupe worktree "))
                .count(),
            2
        );
    });
}

#[test]
fn sanitized_colliding_and_non_utf8_roots_use_gits_admin_names_for_regions() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.repository(&main);
        s.init(&main);
        let main_wt = Worktree::read(s, &main);
        let main_region = main_wt.region_bytes();
        let main_private = Tree::of(&main_wt.private_directory());
        let roots = [
            s.dir().join("with space"),
            s.dir().join("a:b"),
            s.dir().join("first/same"),
            s.dir().join("second/same"),
            s.dir().join(OsStr::from_bytes(b"caf\xe9")),
        ];
        let mut worktrees = Vec::new();
        for root in roots {
            fs::create_dir_all(root.parent().unwrap()).unwrap();
            s.linked_worktree(&main, &root);
            let wt = Worktree::read(s, &root);
            success(s, &wt, &root, &["dupe", "init"]);
            worktrees.push(wt);
        }
        assert_eq!(worktrees[0].name(), Some(OsStr::new("with-space")));
        assert_eq!(worktrees[1].name(), Some(OsStr::new("a-b")));
        assert_eq!(worktrees[2].name(), Some(OsStr::new("same")));
        let collision = worktrees[3].name().unwrap().as_bytes();
        assert!(
            collision.starts_with(b"same")
                && collision[4..].iter().all(u8::is_ascii_digit)
                && collision.len() > 4
        );
        assert_eq!(worktrees[4].name(), Some(OsStr::from_bytes(b"caf\xe9")));
        for (index, wt) in worktrees.iter().enumerate() {
            let before: Vec<_> = worktrees.iter().map(Worktree::region_bytes).collect();
            let lists: Vec<_> = worktrees
                .iter()
                .map(|other| {
                    let path = other.root.join(".gitdupe");
                    path.try_exists().unwrap().then(|| fs::read(path).unwrap())
                })
                .collect();
            let path = format!("private-{index}");
            let output = success(s, wt, &wt.root, &["dupe", "hide", &path]);
            assert!(output.stdout.is_empty());
            assert!(output.lines("warning").is_empty(), "{output:?}");
            assert_eq!(
                fs::read(wt.root.join(".gitdupe")).unwrap(),
                format!("{path}\n").as_bytes()
            );
            assert!(
                wt.region()
                    .unwrap()
                    .rules
                    .contains(&format!("/{path}").into_bytes())
            );
            for ((other, old), list) in worktrees.iter().zip(&before).zip(&lists) {
                if other != wt {
                    assert_eq!(other.region_bytes(), *old);
                    let path = other.root.join(".gitdupe");
                    assert_eq!(
                        path.try_exists().unwrap().then(|| fs::read(path).unwrap()),
                        *list
                    );
                }
            }
            assert_eq!(main_wt.region_bytes(), main_region);
            unchanged(&main_private, &main_wt.private_directory());
        }
    });
}

#[test]
fn a_private_directory_symlink_does_not_attach_a_linked_worktree() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.repository(&main);
        s.init(&main);
        let root = s.dir().join("linked");
        s.linked_worktree(&main, &root);
        let wt = Worktree::read(s, &root);
        let target = wt.common_directory.join("dupe");
        symlink(&target, wt.private_directory()).unwrap();
        let main_private = Tree::of(&target);
        let region = fs::read(wt.common_directory.join("info/exclude")).unwrap();
        refused(&run(s, &wt, &root, &["dupe", "status"]), &main);
        assert_eq!(fs::read_link(wt.private_directory()).unwrap(), target);
        unchanged(&main_private, &target);
        assert_eq!(
            fs::read(wt.common_directory.join("info/exclude")).unwrap(),
            region
        );
    });
}

#[test]
fn linked_commands_from_root_and_subdirectory_use_only_their_own_private_state() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.attached_project(&main);
        write(&main, "main-seed", b"main\n");
        s.git(["dupe", "add", "main-seed"]).from(&main).succeeds();
        s.git([
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "dupe",
            "commit",
            "-qm",
            "main first",
        ])
        .from(&main)
        .succeeds();
        let main_wt = Worktree::read(s, &main);
        let root = s.dir().join("linked");
        s.linked_worktree(&main, &root);
        let wt = Worktree::read(s, &root);
        s.init(&root);
        for (key, value) in [
            ("user.name", "Scenario"),
            ("user.email", "scenario@example.invalid"),
            ("maintenance.auto", "false"),
            ("alias.lg", "log --format=%s"),
        ] {
            wt.private(s).git(["config", key, value]).succeeds();
        }
        let sub = root.join("sub");
        fs::create_dir(&sub).unwrap();
        let main_private = Tree::of(&main_wt.private_directory());
        let main_region = main_wt.region_bytes();
        let mut heads = Vec::new();
        for (from, prefix, subject) in [
            (&root, "", "linked root"),
            (&sub, "sub/", "linked subdirectory"),
        ] {
            write(from, "secret", subject.as_bytes());
            let hide = success(s, &wt, from, &["dupe", "hide", "secret"]);
            assert!(hide.stdout.is_empty());
            assert!(hide.lines("warning").is_empty(), "{hide:?}");
            assert!(
                wt.region()
                    .unwrap()
                    .rules
                    .contains(&format!("/{prefix}secret").into_bytes())
            );
            let added = success(s, &wt, from, &["dupe", "add", "secret"]);
            assert!(added.lines("warning").is_empty(), "{added:?}");
            assert!(
                records(&wt.private(s).git(["ls-files", "-z"]).succeeds().stdout, 0)
                    .contains(&format!("{prefix}secret").as_bytes())
            );
            let unhide = success(s, &wt, from, &["dupe", "unhide", "secret"]);
            names(unhide.only_line("hint"), b"is no longer listed in .gitdupe");
            assert_eq!(fs::read(root.join(".gitdupe")).unwrap(), b"");
            success(s, &wt, from, &["dupe", "stage", "secret"]);
            success(s, &wt, from, &["dupe", "commit", "-qm", subject]);
            let head = wt.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout;
            assert!(!heads.contains(&head));
            heads.push(head);
            let status = success(s, &wt, from, &["dupe", "status", "--porcelain"]);
            assert!(status.stdout.is_empty(), "{status:?}");
            let expected = wt.private(s).git(["log", "--format=%s"]).succeeds();
            assert_eq!(records(&expected.stdout, b'\n')[0], subject.as_bytes());
            for words in [
                &["dupe", "log", "--format=%s"][..],
                &["dupe", "git", "log", "--format=%s"],
                &["dupe", "lg"],
            ] {
                let actual = success(s, &wt, from, words);
                assert_eq!(actual.stdout, expected.stdout, "{actual:?}");
            }
            let git_help = wt.private(s).git(["log", "-h"]).from(from).run();
            let help = run(s, &wt, from, &["dupe", "log", "-h"]);
            assert_eq!(help.end, git_help.end, "{help:?}");
            assert_eq!(help.stdout, git_help.stdout, "{help:?}");
            assert_eq!(help.stderr, git_help.stderr, "{help:?}");
            for words in [
                &["status", "--porcelain", "--untracked-files=all"][..],
                &["add", "-A", "--dry-run"],
            ] {
                let public = s.git(words).from(&root).succeeds();
                for private in [b"secret".as_slice(), b".gitdupe"] {
                    assert!(!holds(&public.stdout, private), "{public:?}");
                }
            }
            unchanged(&main_private, &main_wt.private_directory());
            assert_eq!(main_wt.region_bytes(), main_region);
        }
        let linked_private = Tree::of(&wt.private_directory());
        let linked_region = wt.region_bytes();
        write(&main, "main-secret", b"main\n");
        success(s, &main_wt, &main, &["dupe", "add", "main-secret"]);
        let status = success(s, &main_wt, &main, &["dupe", "status", "--porcelain"]);
        names(&status.stdout, b"main-secret");
        success(
            s,
            &main_wt,
            &main,
            &[
                "-c",
                "user.name=Scenario",
                "-c",
                "user.email=scenario@example.invalid",
                "dupe",
                "commit",
                "-qm",
                "main second",
            ],
        );
        assert_eq!(
            main_wt
                .private(s)
                .git(["log", "--format=%s"])
                .succeeds()
                .stdout,
            b"main second\nmain first\n"
        );
        unchanged(&linked_private, &wt.private_directory());
        assert_eq!(wt.region_bytes(), linked_region);
        assert!(!holds(
            &wt.private(s).git(["log", "--format=%s"]).succeeds().stdout,
            b"main second"
        ));
    });
}

/// A caller's variables that locate a repository, an index, or an object store, each
/// naming the linked worktree's public repository as a hook run there could, reach no
/// private run: `add` stages in the linked worktree's private index alone, and nothing
/// of the public repository or the main worktree's private one changes.
#[test]
fn a_linked_hook_environment_cannot_redirect_the_private_index_or_object_store() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.attached_project(&main);
        let root = s.dir().join("linked");
        s.linked_worktree(&main, &root);
        let wt = Worktree::read(s, &root);
        s.init(&root);
        success(s, &wt, &root, &["dupe", "hide", "hook-secret"]);
        let common = &wt.common_directory;
        let mut references = OsString::from("files://");
        references.push(common);
        write(&root, "hook-secret", b"private\n");
        let public = wt.public_git();
        let main_private = Tree::of(&common.join("dupe"));
        let public_index = fs::read(wt.git_directory.join("index")).unwrap();
        let private_index = fs::read(wt.private_directory().join("index")).unwrap();
        let output = s
            .git(["dupe", "add", "hook-secret"])
            .from(&root)
            .variable("GIT_DIR", &wt.git_directory)
            .variable("GIT_WORK_TREE", &root)
            .variable("GIT_INDEX_FILE", wt.git_directory.join("index"))
            .variable("GIT_COMMON_DIR", common)
            .variable("GIT_OBJECT_DIRECTORY", common.join("objects"))
            .variable("GIT_ALTERNATE_OBJECT_DIRECTORIES", common.join("objects"))
            .variable("GIT_REFERENCE_BACKEND", &references)
            .run();
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.lines("warning").is_empty(), "{output:?}");
        assert_eq!(
            wt.private(s).git(["ls-files", "-z"]).succeeds().stdout,
            b".gitdupe\0hook-secret\0"
        );
        assert_ne!(
            fs::read(wt.private_directory().join("index")).unwrap(),
            private_index
        );
        assert_eq!(
            fs::read(wt.git_directory.join("index")).unwrap(),
            public_index
        );
        assert!(public.changed_in(&wt.public_git()).is_empty());
        unchanged(&main_private, &common.join("dupe"));
        // The blob staged privately is in the private object store, not the public one.
        let blob = wt
            .private(s)
            .git(["rev-parse", ":hook-secret"])
            .succeeds()
            .stdout;
        assert!(held_publicly(s, &main, &blob).is_empty(), "{blob:?}");
    });
}

/// What `add`, `clean`, and settle mean in the main worktree they mean in an attached
/// linked one, over its own private repository: its own `info/exclude` and not the main
/// worktree's decides what needs `-f`, a publicly tracked path is refused or skipped,
/// `clean` spares its hidden paths, and a refusing command still names a path a `!` rule
/// exposes, a path tracked by both repositories, and a released path, on standard error
/// alone (G6, G8, G9, G13–G16, G25).
#[test]
fn a_linked_worktree_keeps_the_meanings_of_add_clean_and_settle() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.attached_project(&main);
        let main_wt = Worktree::read(s, &main);
        let root = s.dir().join("linked");
        s.linked_worktree(&main, &root);
        let wt = Worktree::read(s, &root);
        s.init(&root);
        let private_tracked = || {
            records(&wt.private(s).git(["ls-files", "-z"]).succeeds().stdout, 0)
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>()
        };

        // Its own `info/exclude` is the one that makes a file need `-f`.
        for (private, pattern) in [
            (&wt.private_directory(), "*.log\n"),
            (&main_wt.private_directory(), "*.tmp\n"),
        ] {
            fs::create_dir_all(private.join("info")).unwrap();
            fs::write(private.join("info/exclude"), pattern).unwrap();
        }
        write(&root, "x.log", b"log\n");
        write(&root, "y.tmp", b"tmp\n");
        let ignored = run(s, &wt, &root, &["dupe", "add", "x.log"]);
        assert_eq!(ignored.end, End::Code(1), "{ignored:?}");
        assert!(!private_tracked().contains(&b"x.log".to_vec()));
        success(s, &wt, &root, &["dupe", "add", "-f", "x.log"]);
        success(s, &wt, &root, &["dupe", "add", "y.tmp"]);
        assert!(private_tracked().contains(&b"x.log".to_vec()));
        assert!(private_tracked().contains(&b"y.tmp".to_vec()));

        // A publicly tracked file is refused, and one under a hidden directory skipped.
        let refused_add = run(s, &wt, &root, &["dupe", "add", "README.md"]);
        assert_eq!(refused_add.end, End::Code(128), "{refused_add:?}");
        names(refused_add.only_line("fatal"), b"git rm --cached");
        success(s, &wt, &root, &["dupe", "hide", "notes"]);
        write(&root, "notes/a.md", b"private\n");
        write(&root, "notes/shared.md", b"public\n");
        s.git(["add", "-f", "notes/shared.md"])
            .from(&root)
            .succeeds();
        let skipped = success(s, &wt, &root, &["dupe", "add", "notes/"]);
        let tracked = private_tracked();
        assert!(tracked.contains(&b"notes/a.md".to_vec()), "{tracked:?}");
        assert!(
            !tracked.contains(&b"notes/shared.md".to_vec()),
            "{tracked:?}"
        );
        assert!(!tracked.contains(&b"README.md".to_vec()), "{tracked:?}");
        names_number(skipped.only_line("warning"), 1);

        // `clean` deletes the project's untracked file and spares every hidden path.
        write(&root, "scratch.txt", b"scratch\n");
        write(&root, "notes/b.md", b"untracked\n");
        success(s, &wt, &root, &["dupe", "clean", "-fdx"]);
        assert!(!root.join("scratch.txt").exists());
        for kept in [
            "notes/a.md",
            "notes/b.md",
            "notes/shared.md",
            "x.log",
            "y.tmp",
            ".gitdupe",
        ] {
            assert!(root.join(kept).exists(), "{kept}");
        }

        // A path a `!` rule exposes, one tracked by both repositories, and a released one,
        // each named after a command that refuses.
        success(s, &wt, &root, &["dupe", "hide", "released"]);
        write(&root, "released", b"was hidden\n");
        success(s, &wt, &root, &["dupe", "status"]);
        fs::write(root.join(".gitdupe"), b"notes\n").unwrap();
        let gitignore = [
            fs::read(root.join(".gitignore")).unwrap(),
            b"!/y.tmp\n".to_vec(),
        ]
        .concat();
        let rule_line = records(&gitignore, b'\n').len().to_string();
        fs::write(root.join(".gitignore"), &gitignore).unwrap();
        s.git(["add", "-f", "x.log"]).from(&root).succeeds();
        let refusing = run(s, &wt, &root, &["dupe", "add", "README.md"]);
        assert_eq!(refusing.end, End::Code(128), "{refusing:?}");
        let warnings = refusing.lines("warning");
        let rule = format!(".gitignore:{rule_line}");
        for parts in [
            &[b"y.tmp".as_slice(), rule.as_bytes()][..],
            &[b"x.log", b"tracked by both"],
            &[b"released", b"visible"],
        ] {
            assert!(
                warnings
                    .iter()
                    .any(|line| parts.iter().all(|part| holds(line, part))),
                "no warning naming {parts:?}: {refusing:?}"
            );
        }

        // Machine-readable output stays Git's: the warnings go to standard error alone.
        for words in [
            &["dupe", "status", "--porcelain"][..],
            &["dupe", "status", "-z"],
        ] {
            let status = run(s, &wt, &root, words);
            assert_eq!(status.end, End::Code(0), "{status:?}");
            assert!(!status.lines("warning").is_empty(), "{status:?}");
            for level in [b"warning:".as_slice(), b"hint:", b"fatal:", b"error:"] {
                assert!(!holds(&status.stdout, level), "{status:?}");
            }
            assert!(holds(&status.stdout, b"y.tmp"), "{status:?}");
        }
    });
}

/// Where something other than a directory stands at a worktree's private Git directory —
/// a symbolic link to the project's common Git directory or to another worktree's private
/// repository, or a file — `init` and `clone` write nothing through it or in its place:
/// one `fatal:` line names it, exit 128, and the public repository, every private
/// repository, and the exclude file stand as they were (G5, R8, G28).
#[test]
fn init_and_clone_write_nothing_through_what_stands_at_the_private_path() {
    under_each_release(|s| {
        let main = s.dir().join("main");
        s.repository(&main);
        s.init(&main);
        let main_wt = Worktree::read(s, &main);
        let remote = s.dir().join("private.git");
        s.bare_repository(&remote);
        for (name, link_to) in [
            ("to-common", Some(main_wt.common_directory.clone())),
            ("to-main-private", Some(main_wt.private_directory())),
            ("a-file", None),
        ] {
            let root = s.dir().join(name);
            s.linked_worktree(&main, &root);
            let wt = Worktree::read(s, &root);
            match &link_to {
                Some(target) => symlink(target, wt.private_directory()).unwrap(),
                None => fs::write(wt.private_directory(), b"not a repository\n").unwrap(),
            }
            let common = Tree::of(&wt.common_directory);
            for words in [
                &[OsStr::new("dupe"), OsStr::new("init")][..],
                &[OsStr::new("dupe"), OsStr::new("clone"), remote.as_os_str()],
            ] {
                let output = s.git(words).from(&root).run();
                assert_eq!(output.end, End::Code(128), "{name} {words:?}: {output:?}");
                assert!(output.stdout.is_empty(), "{output:?}");
                names(
                    output.only_line("fatal"),
                    wt.private_directory().as_os_str().as_bytes(),
                );
                // Read through no link: the link itself, the common Git directory with
                // every private repository and the exclude file, byte for byte.
                unchanged(&common, &wt.common_directory);
            }
            refused(&run(s, &wt, &root, &["dupe", "status"]), &main);
        }
        // In the main worktree too.
        let other = s.dir().join("other");
        s.repository(&other);
        let other_wt = Worktree::read(s, &other);
        symlink(main_wt.private_directory(), other_wt.private_directory()).unwrap();
        let before = [
            Tree::of(&other_wt.common_directory),
            Tree::of(&main_wt.common_directory),
        ];
        let output = s.git(["dupe", "init"]).from(&other).run();
        assert_eq!(output.end, End::Code(128), "{output:?}");
        names(
            output.only_line("fatal"),
            other_wt.private_directory().as_os_str().as_bytes(),
        );
        unchanged(&before[0], &other_wt.common_directory);
        unchanged(&before[1], &main_wt.common_directory);
        refused(&run(s, &other_wt, &other, &["dupe", "status"]), &other);
    });
}
