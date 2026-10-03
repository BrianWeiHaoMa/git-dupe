//! Attachment and completion through `git dupe init`, including its failure boundaries.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::harness::{
    End, Output, Scenario, Tree, holds, locate_words, names, names_number, refused_stash_untracked,
    region, region_rules, unchanged, under_each_release, usage_line,
};

/// An unattached repository and the directory to run from: its root, or `sub` below it.
fn unattached_workspace(s: &Scenario, name: &str, below: bool) -> (PathBuf, PathBuf) {
    let dir = s.dir().join(format!("{name}-{below}"));
    s.repository(&dir);
    let from = from(&dir, below);
    (dir, from)
}

fn from(dir: &Path, below: bool) -> PathBuf {
    let from = if below {
        dir.join("sub")
    } else {
        dir.to_path_buf()
    };
    fs::create_dir_all(&from).unwrap();
    from
}

fn identity(s: &Scenario, dir: &Path) {
    s.git(["config", "--local", "user.name", "Local Name"])
        .from(dir)
        .succeeds();
    s.git(["config", "--local", "user.email", "local@example.invalid"])
        .from(dir)
        .succeeds();
}

fn key(s: &Scenario, dir: &Path, name: &str, expected: &str) {
    let output = s
        .private(dir)
        .git(["config", "--local", "--get", name])
        .run();
    assert_eq!(output.end, End::Code(0), "{name}: {output:?}");
    assert_eq!(output.stdout, format!("{expected}\n").as_bytes(), "{name}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn keys(s: &Scenario, dir: &Path) {
    key(s, dir, "status.showUntrackedFiles", "no");
    key(s, dir, "advice.statusHints", "false");
    key(s, dir, "core.worktree", "../..");
    key(s, dir, "user.name", "Local Name");
    key(s, dir, "user.email", "local@example.invalid");
}

/// Exit 0, `hints` hints, and no `fatal:` or `error:` line; Git's own output may be on
/// standard output.
fn succeeded_with_hints(output: &Output, hints: usize) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert_eq!(output.lines("hint").len(), hints, "{output:?}");
    assert!(output.lines("fatal").is_empty(), "{output:?}");
    assert!(output.lines("error").is_empty(), "{output:?}");
}

fn gits_init_line(output: &Output, dir: &Path) {
    names(&output.stdout, dir.join(".git/dupe").as_os_str().as_bytes());
    // Only Git's output belongs on stdout; git-dupe's levels belong on stderr.
    for line in output.stdout.split(|&byte| byte == b'\n') {
        for level in [b"hint: ".as_slice(), b"warning: ", b"fatal: ", b"error: "] {
            assert!(!line.starts_with(level), "{output:?}");
        }
    }
}

#[test]
fn first_attachment_copies_local_identity_and_preserves_the_public_repository() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "first", below);
            identity(s, &dir);
            let user = b"# user text\r\n*.o\n";
            fs::write(dir.join(".git/info/exclude"), user).unwrap();
            let excluded = [dir.join(".git/dupe"), dir.join(".git/info/exclude")];
            let before = Tree::of(&dir.join(".git")).without(&[&excluded[0], &excluded[1]]);
            let output = s.git(["dupe", "init"]).from(&from).run();
            succeeded_with_hints(&output, 0);
            gits_init_line(&output, &dir);
            assert!(output.stderr.is_empty(), "{output:?}");
            assert_eq!(
                fs::read(dir.join(".git/dupe/HEAD")).unwrap(),
                b"ref: refs/heads/main\n"
            );
            keys(s, &dir);
            let found = region(&dir).unwrap();
            assert_eq!(found.before, user);
            assert_eq!(found.rules, [b"/.gitdupe"]);
            assert!(found.after.is_empty());
            assert!(!dir.join(".gitdupe").exists());
            let changed = before
                .changed_in(&Tree::of(&dir.join(".git")).without(&[&excluded[0], &excluded[1]]));
            assert!(changed.is_empty(), "changed: {changed:?}");
            let log = s.private(&dir).git(["log"]).run();
            assert_eq!(log.end, End::Code(128), "{log:?}");
            // Standard output is Git's own line: what Git's `init` prints for the same
            // private Git directory.
            let detached = s.git(["dupe", "detach", "--force"]).from(&dir).run();
            assert_eq!(detached.end, End::Code(0), "{detached:?}");
            let gits = s
                .git(["init", "--initial-branch=main"])
                .from(&dir)
                .variable("GIT_DIR", dir.join(".git/dupe"))
                .variable("GIT_WORK_TREE", &dir)
                .succeeds();
            assert_eq!(output.stdout, gits.stdout, "{output:?}");
        }
    });
}

#[test]
fn global_identity_is_not_copied_into_private_local_config() {
    under_each_release(|s| {
        fs::write(
            s.dir().join("home/.gitconfig"),
            b"[user]\n name = Global Name\n",
        )
        .unwrap();
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "global", below);
            let output = s.git(["dupe", "init"]).from(&from).run();
            succeeded_with_hints(&output, 0);
            let local = s
                .private(&dir)
                .git(["config", "--local", "--get", "user.name"])
                .run();
            assert_eq!(local.end, End::Code(1), "{local:?}");
            assert!(local.stdout.is_empty(), "{local:?}");
            assert!(local.stderr.is_empty(), "{local:?}");
        }
    });
}

/// The only value of `name` in the private local configuration.
fn only_value(s: &Scenario, dir: &Path, name: &str, expected: &str) {
    let output = s
        .private(dir)
        .git(["config", "--local", "--get-all", name])
        .run();
    assert_eq!(output.end, End::Code(0), "{name}: {output:?}");
    assert_eq!(output.stdout, format!("{expected}\n").as_bytes(), "{name}");
}

#[test]
fn a_templates_repeated_keys_are_replaced_and_an_included_local_identity_is_copied() {
    under_each_release(|s| {
        let template = s.dir().join("template");
        fs::create_dir(&template).unwrap();
        fs::write(
            template.join("config"),
            b"[user]\n name = First\n name = Second\n\
              [status]\n showUntrackedFiles = all\n showUntrackedFiles = normal\n",
        )
        .unwrap();
        let mut template_dir = OsString::from("init.templateDir=");
        template_dir.push(&template);
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "template", below);
            fs::write(
                dir.join(".git/identity"),
                b"[user]\n name = Local Name\n email = local@example.invalid\n",
            )
            .unwrap();
            s.git(["config", "--local", "include.path", "identity"])
                .from(&dir)
                .succeeds();
            let output = s
                .git(
                    [OsStr::new("-c"), &template_dir]
                        .into_iter()
                        .chain(["dupe", "init"].map(OsStr::new)),
                )
                .from(&from)
                .run();
            succeeded_with_hints(&output, 0);
            gits_init_line(&output, &dir);
            only_value(s, &dir, "user.name", "Local Name");
            only_value(s, &dir, "status.showUntrackedFiles", "no");
            keys(s, &dir);
        }
    });
}

#[test]
fn initial_branch_uses_the_option_public_head_or_gits_default() {
    under_each_release(|s| {
        fs::write(
            s.dir().join("home/.gitconfig"),
            b"[init]\n defaultBranch = trunk\n",
        )
        .unwrap();
        for below in [false, true] {
            for (index, (options, branch)) in [
                (&["-b", "other"][..], "other"),
                (&["--initial-branch=other"][..], "other"),
                (&["--initial-branch", "other"][..], "other"),
                (&[][..], "feature/x"),
                (&[][..], "trunk"),
            ]
            .into_iter()
            .enumerate()
            {
                let (dir, from) = unattached_workspace(s, &format!("branch-{index}"), below);
                if branch == "feature/x" {
                    s.git(["checkout", "-q", "-b", "feature/x"])
                        .from(&dir)
                        .succeeds();
                } else if branch == "trunk" {
                    s.git(["checkout", "-q", "--detach"]).from(&dir).succeeds();
                }
                let output = s
                    .git(["dupe", "init"].iter().chain(options))
                    .from(&from)
                    .run();
                succeeded_with_hints(&output, 0);
                assert_eq!(
                    fs::read(dir.join(".git/dupe/HEAD")).unwrap(),
                    format!("ref: refs/heads/{branch}\n").as_bytes()
                );
            }
        }
    });
}

#[test]
fn a_private_commit_survives_the_workspaces_rename() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, _) = unattached_workspace(s, "before", below);
            identity(s, &dir);
            s.init(&dir);
            fs::write(dir.join("notes"), b"private\n").unwrap();
            for words in [
                &["add", "-f", "--", "notes"][..],
                &[
                    "-c",
                    "maintenance.auto=false",
                    "commit",
                    "-q",
                    "-m",
                    "private",
                ],
            ] {
                let output = s.private(&dir).git(words).run();
                assert_eq!(output.end, End::Code(0), "{output:?}");
            }
            let renamed = s.dir().join(format!("renamed-{below}"));
            fs::rename(&dir, &renamed).unwrap();
            let from = from(&renamed, below);
            let mut private = OsString::from("--git-dir=");
            private.push(renamed.join(".git/dupe"));
            for words in [&["status", "--porcelain"][..], &["log"]] {
                // No --work-tree: Git must resolve the stored relative core.worktree.
                let output = s
                    .git(
                        [private.as_os_str()]
                            .into_iter()
                            .chain(words.iter().map(OsStr::new)),
                    )
                    .from(&from)
                    .run();
                assert_eq!(output.end, End::Code(0), "{output:?}");
            }
            fs::write(renamed.join(".gitdupe"), b"notes\n").unwrap();
            let output = s.git(["dupe", "stash", "-u"]).from(&from).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            refused_stash_untracked(s, &output);
            output.only_line("fatal");
            assert_eq!(region_rules(&renamed), [b"/.gitdupe".as_slice(), b"/notes"]);
        }
    });
}

#[test]
fn repeated_init_preserves_edits_and_restores_unfinished_or_deleted_config() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "repeat", below);
            identity(s, &dir);
            s.init(&dir);
            for edited in [false, true] {
                if edited {
                    let output = s
                        .private(&dir)
                        .git(["config", "user.name", "Edited Name"])
                        .run();
                    assert_eq!(output.end, End::Code(0), "{output:?}");
                }
                let before = Tree::of(&dir.join(".git"));
                let output = s.git(["dupe", "init"]).from(&from).run();
                succeeded_with_hints(&output, 1);
                names(output.lines("hint")[0], b"already attached");
                assert!(output.stdout.is_empty(), "{output:?}");
                unchanged(&before, &dir.join(".git"));
                key(
                    s,
                    &dir,
                    "user.name",
                    if edited { "Edited Name" } else { "Local Name" },
                );
            }
            for words in [
                &["config", "--unset", "status.showUntrackedFiles"][..],
                &["config", "core.logallrefupdates", "false"],
            ] {
                let output = s.private(&dir).git(words).run();
                assert_eq!(output.end, End::Code(0), "{output:?}");
            }
            let output = s
                .private(&dir)
                .git([
                    OsStr::new("config"),
                    OsStr::new("core.worktree"),
                    dir.as_os_str(),
                ])
                .run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            let output = s.git(["dupe", "init"]).from(&from).run();
            succeeded_with_hints(&output, 1);
            keys(s, &dir);
            key(s, &dir, "core.logallrefupdates", "false");
            fs::remove_file(dir.join(".git/dupe/config")).unwrap();
            let output = s.git(["dupe", "init"]).from(&from).run();
            succeeded_with_hints(&output, 1);
            keys(s, &dir);
        }
    });
}

#[test]
fn settle_over_arg_max_replaces_success_after_the_hint_without_writing_exclude() {
    under_each_release(|s| {
        let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
        assert!(limit.status.success());
        let limit: usize = std::str::from_utf8(&limit.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let count = limit / 128 + 1;
        let mut listed = Vec::new();
        for index in 0..count {
            listed.extend_from_slice(format!("path-{index:016}-{}\n", "x".repeat(128)).as_bytes());
        }
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "arg-max", below);
            s.init(&dir);
            fs::write(dir.join(".gitdupe"), &listed).unwrap();
            let exclude = fs::read(dir.join(".git/info/exclude")).unwrap();
            let output = s.git(["dupe", "init"]).from(&from).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let hints = output.lines("hint");
            let fatals = output.lines("fatal");
            assert_eq!(hints.len(), 1, "{output:?}");
            assert_eq!(fatals.len(), 1, "{output:?}");
            names_number(fatals[0], count + 1);
            assert_eq!(
                output.stderr,
                [
                    b"hint: ".as_slice(),
                    hints[0],
                    b"\nfatal: ",
                    fatals[0],
                    b"\n"
                ]
                .concat()
            );
            assert_eq!(fs::read(dir.join(".git/info/exclude")).unwrap(), exclude);
        }
    });
}

#[test]
fn head_only_and_empty_private_directories_are_completed() {
    under_each_release(|s| {
        // Prevent Git's branch advice from being counted as git-dupe's completion hint.
        fs::write(
            s.dir().join("home/.gitconfig"),
            b"[init]\n defaultBranch = main\n",
        )
        .unwrap();
        for below in [false, true] {
            for head in [false, true] {
                let (dir, from) = unattached_workspace(s, &format!("partial-{head}"), below);
                identity(s, &dir);
                let private = dir.join(".git/dupe");
                fs::create_dir(&private).unwrap();
                if head {
                    fs::write(private.join("HEAD"), b"ref: refs/heads/main\n").unwrap();
                }
                let output = s.git(["dupe", "init"]).from(&from).run();
                succeeded_with_hints(&output, 1);
                gits_init_line(&output, &dir);
                assert!(private.join("objects").is_dir());
                assert!(private.join("refs").is_dir());
                assert_eq!(
                    fs::read(private.join("HEAD")).unwrap(),
                    b"ref: refs/heads/main\n"
                );
                keys(s, &dir);
                assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
                if head && private.join("hooks").exists() {
                    for entry in fs::read_dir(private.join("hooks")).unwrap() {
                        assert_ne!(
                            entry.unwrap().path().extension(),
                            Some(OsStr::new("sample"))
                        );
                    }
                }
            }
        }
    });
}

#[test]
fn config_lock_relays_gits_failure_and_removing_it_allows_completion() {
    under_each_release(|s| {
        fs::write(
            s.dir().join("home/.gitconfig"),
            b"[init]\n defaultBranch = main\n",
        )
        .unwrap();
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "lock", below);
            identity(s, &dir);
            let private = dir.join(".git/dupe");
            let made = s
                .git(["init", "-q"])
                .from(&dir)
                .variable("GIT_DIR", &private)
                .variable("GIT_WORK_TREE", &dir)
                .run();
            assert_eq!(made.end, End::Code(0), "{made:?}");
            // Git's absolute core.worktree makes this an unfinished attachment.
            key(s, &dir, "core.worktree", dir.to_str().unwrap());
            let missing = s
                .private(&dir)
                .git(["config", "--local", "--get", "status.showUntrackedFiles"])
                .run();
            assert_eq!(missing.end, End::Code(1), "{missing:?}");
            fs::write(private.join("config.lock"), b"locked\n").unwrap();
            let output = s.git(["dupe", "init"]).from(&from).run();
            let mut git_dir = OsString::from("--git-dir=");
            git_dir.push(&private);
            let gits = s
                .git([
                    git_dir.as_os_str(),
                    OsStr::new("config"),
                    OsStr::new("--local"),
                    OsStr::new("status.showUntrackedFiles"),
                    OsStr::new("no"),
                ])
                .from(&from)
                .run();
            assert_ne!(gits.end, End::Code(0), "{gits:?}");
            assert_eq!(output.end, gits.end, "{output:?}");
            assert_eq!(output.stderr, gits.stderr, "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(output.lines("hint").is_empty(), "{output:?}");
            fs::remove_file(private.join("config.lock")).unwrap();
            let output = s.git(["dupe", "init"]).from(&from).run();
            succeeded_with_hints(&output, 1);
            keys(s, &dir);
        }
    });
}

/// `git dupe init` from `from` is refused with one `fatal:` line alone, and nothing below
/// `dir` changed since `before`.
fn init_refused_changing_nothing(s: &Scenario, dir: &Path, from: &Path, before: &Tree) -> Output {
    let output = s.git(["dupe", "init"]).from(from).run();
    assert_eq!(output.end, End::Code(128), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    output.only_line("fatal");
    unchanged(before, dir);
    output
}

#[test]
fn linked_worktrees_separate_git_directories_and_symlinks_are_refused() {
    under_each_release(|s| {
        for below in [false, true] {
            for separate in [false, true] {
                let (main, _) = unattached_workspace(s, &format!("main-{separate}"), below);
                let admin = s.dir().join(format!("admin-{separate}-{below}"));
                if separate {
                    let mut option = OsString::from("--separate-git-dir=");
                    option.push(&admin);
                    let output = s
                        .git([OsStr::new("init"), OsStr::new("-q"), &option])
                        .from(&main)
                        .run();
                    assert_eq!(output.end, End::Code(0), "{output:?}");
                    let before = Tree::of(&admin);
                    let entry = fs::read(main.join(".git")).unwrap();
                    let output =
                        init_refused_changing_nothing(s, &admin, &from(&main, below), &before);
                    names(output.only_line("fatal"), b".git");
                    assert_eq!(fs::read(main.join(".git")).unwrap(), entry);
                }
                let linked = s.dir().join(format!("linked-{separate}-{below}"));
                s.linked_worktree(&main, &linked);
                let git_dir = if separate {
                    admin.clone()
                } else {
                    main.join(".git")
                };
                let before = Tree::of(&git_dir);
                let entry = fs::read(linked.join(".git")).unwrap();
                let output =
                    init_refused_changing_nothing(s, &git_dir, &from(&linked, below), &before);
                let line = output.only_line("fatal");
                if separate {
                    assert!(!line.contains(&b'/'), "{output:?}");
                } else {
                    names(line, main.as_os_str().as_bytes());
                    assert!(
                        !line
                            .windows(main.as_os_str().as_bytes().len() + 1)
                            .any(|part| part == [main.as_os_str().as_bytes(), b"/"].concat()),
                        "{output:?}"
                    );
                }
                assert_eq!(fs::read(linked.join(".git")).unwrap(), entry);
            }
            // A working tree named apart from its repository: its own `.git` is another
            // repository's directory, or it has none. `core.worktree` `../..` relative
            // to the named repository's `.git/dupe` would not be this root.
            let (named, _) = unattached_workspace(s, "named", below);
            let (other, _) = unattached_workspace(s, "other", below);
            let mut git_dir = OsString::from("--git-dir=");
            git_dir.push(named.join(".git"));
            for work_tree in [other.clone(), named.join("sub")] {
                let mut option = OsString::from("--work-tree=");
                option.push(&work_tree);
                let before = [Tree::of(&named.join(".git")), Tree::of(&other.join(".git"))];
                let output = s
                    .git(
                        [&git_dir, &option]
                            .map(OsString::as_os_str)
                            .into_iter()
                            .chain(["dupe", "init"].map(OsStr::new)),
                    )
                    .from(&from(&work_tree, below))
                    .run();
                assert_eq!(output.end, End::Code(128), "{output:?}");
                assert!(output.stdout.is_empty(), "{output:?}");
                names(output.only_line("fatal"), b".git");
                unchanged(&before[0], &named.join(".git"));
                unchanged(&before[1], &other.join(".git"));
            }
            let (dir, from) = unattached_workspace(s, "symlink", below);
            let admin = s.dir().join(format!("symlink-admin-{below}"));
            fs::rename(dir.join(".git"), &admin).unwrap();
            symlink(&admin, dir.join(".git")).unwrap();
            let before = Tree::of(&admin);
            let output = init_refused_changing_nothing(s, &admin, &from, &before);
            names(output.only_line("fatal"), b".git");
            assert_eq!(fs::read_link(dir.join(".git")).unwrap(), admin);
        }
    });
}

#[test]
fn submodules_and_publicly_tracked_gitdupe_are_refused() {
    under_each_release(|s| {
        for below in [false, true] {
            let (public, _) = unattached_workspace(s, "public", below);
            let (superproject, _) = unattached_workspace(s, "super", below);
            if below {
                fs::remove_dir(superproject.join("sub")).unwrap();
            }
            let added = s
                .git([
                    OsStr::new("-c"),
                    OsStr::new("protocol.file.allow=always"),
                    OsStr::new("submodule"),
                    OsStr::new("add"),
                    public.as_os_str(),
                    OsStr::new("sub"),
                ])
                .from(&superproject)
                .run();
            assert_eq!(added.end, End::Code(0), "{added:?}");
            let submodule = superproject.join("sub");
            let entry = fs::read(submodule.join(".git")).unwrap();
            let before = Tree::of(&superproject.join(".git"));
            let refused = init_refused_changing_nothing(
                s,
                &superproject.join(".git"),
                &from(&submodule, below),
                &before,
            );
            names(refused.only_line("fatal"), b"submodule");
            assert_eq!(fs::read(submodule.join(".git")).unwrap(), entry);
            let (dir, from) = unattached_workspace(s, "tracked", below);
            fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
            s.git(["add", "-f", "--", ".gitdupe"]).from(&dir).succeeds();
            let before = Tree::of(&dir.join(".git"));
            let output = init_refused_changing_nothing(s, &dir.join(".git"), &from, &before);
            names(output.only_line("fatal"), b".gitdupe");
        }
    });
}

#[test]
fn tracked_gitdupe_after_attachment_is_refused_but_still_settles() {
    under_each_release(|s| {
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "attached-tracked", below);
            s.init(&dir);
            // A path listed since the last command: the refusal's settle must bring the
            // region up to date, not only ask about exposure.
            fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
            fs::write(dir.join(".gitignore"), b"!.gitdupe\n").unwrap();
            s.git(["add", "-f", "--", ".gitdupe"]).from(&dir).succeeds();
            let exclude = dir.join(".git/info/exclude");
            let before = Tree::of(&dir.join(".git")).without(&[&exclude]);
            let output = s.git(["dupe", "init"]).from(&from).run();
            assert_eq!(output.end, End::Code(128), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(output.lines("hint").is_empty(), "{output:?}");
            names(output.lines("fatal")[0], b".gitdupe");
            assert_eq!(output.lines("fatal").len(), 1, "{output:?}");
            let warning = output.lines("warning");
            assert_eq!(warning.len(), 1, "{output:?}");
            names(warning[0], b".gitdupe");
            // Public Git tracks `.gitdupe`: no ignore rule decides for it, the `!` rule
            // included (G7, `Holds/G6, G8, G9`).
            assert!(!holds(warning[0], b".gitignore"), "{output:?}");
            assert_eq!(region_rules(&dir), [b"/.gitdupe".as_slice(), b"/notes"]);
            let changed = before.changed_in(&Tree::of(&dir.join(".git")).without(&[&exclude]));
            assert!(changed.is_empty(), "changed: {changed:?}");
        }
    });
}

#[test]
fn misuse_has_init_usage_without_settle_and_outside_uses_gits_locate_failure() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "init"]).succeeds();
        let misuse = [&["x"][..], &["--bogus"], &["-b"], &["--", "x"]];
        for below in [false, true] {
            for attached in [false, true] {
                let (dir, from) = unattached_workspace(s, &format!("misuse-{attached}"), below);
                if attached {
                    s.init(&dir);
                }
                fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
                let before = Tree::of(&dir.join(".git"));
                for words in misuse {
                    let output = s
                        .git(["dupe", "init"].iter().chain(words))
                        .from(&from)
                        .run();
                    assert_eq!(output.end, End::Code(129), "{words:?}: {output:?}");
                    assert!(output.stdout.is_empty(), "{output:?}");
                    output.line_then("error", usage_line(&help.stdout));
                    unchanged(&before, &dir.join(".git"));
                    if attached {
                        assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
                    } else {
                        assert!(region(&dir).is_none());
                    }
                }
            }
        }
        let gits = s.git(locate_words(true)).run();
        assert_eq!(gits.end, End::Code(128), "{gits:?}");
        for words in misuse {
            let output = s.git(["dupe", "init"].iter().chain(words)).run();
            assert_eq!(output.end, gits.end, "{output:?}");
            assert_eq!(output.stderr, gits.stderr, "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
        }
    });
}

#[test]
fn help_options_print_init_text_without_locating_or_settling() {
    under_each_release(|s| {
        let help = s.git(["dupe", "help", "init"]).succeeds();
        for below in [false, true] {
            let (dir, from) = unattached_workspace(s, "help", below);
            s.init(&dir);
            fs::write(dir.join(".gitdupe"), b"notes\n").unwrap();
            let before = Tree::of(&dir.join(".git"));
            for place in [&from, s.dir()] {
                for words in [&["-h"][..], &["--help"], &["-b", "x", "-h"]] {
                    let output = s
                        .git(["dupe", "init"].iter().chain(words))
                        .from(place)
                        .run();
                    assert_eq!(output, help, "{words:?}");
                    unchanged(&before, &dir.join(".git"));
                    assert_eq!(region_rules(&dir), [b"/.gitdupe"]);
                }
            }
        }
    });
}
