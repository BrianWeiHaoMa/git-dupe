//! What `git dupe detach` leaves, its warnings, its words, and attaching again (G3).

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Transfer, Tree, changed_since, daily_warnings, detached, holds,
    locate_words, names, now_visible, private_add, private_commit, region, region_rules,
    run_traced, unchanged, under_each_release, warnings, write,
};

const USER_BEFORE: &[u8] = b"# developer before\r\n*.o\n# caf\xe9\n";
const USER_AFTER: &[u8] = b"# developer after\r\n*.bak\n# no final newline";

/// Put developer text on both sides of the existing region, without settling again.
fn developer_exclude(root: &Path) {
    let mut bytes = USER_BEFORE.to_vec();
    bytes.extend_from_slice(b"# BEGIN git-dupe\n");
    for rule in region_rules(root) {
        bytes.extend_from_slice(&rule);
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(b"# END git-dupe\n");
    bytes.extend_from_slice(USER_AFTER);
    let exclude = root.join(".git/info/exclude");
    fs::write(&exclude, bytes).unwrap();
    fs::set_permissions(exclude, fs::Permissions::from_mode(0o640)).unwrap();
}

/// Everything a successful detach must preserve, except its two authorized deletions.
struct Preserved {
    working: Tree,
    public: Tree,
    linked: Tree,
    public_remote: Tree,
    private_remote: Tree,
}

impl Preserved {
    fn of(t: &Transfer) -> Self {
        Self {
            working: Tree::working(&t.root),
            public: public_tree(&t.root),
            linked: Tree::of(&t.linked),
            public_remote: Tree::of(&t.public_remote),
            private_remote: Tree::of(&t.private_remote),
        }
    }

    fn after_detach(&self, t: &Transfer) {
        assert!(changed_since(&self.working, &t.root).is_empty());
        assert!(self.public.changed_in(&public_tree(&t.root)).is_empty());
        unchanged(&self.linked, &t.linked);
        unchanged(&self.public_remote, &t.public_remote);
        unchanged(&self.private_remote, &t.private_remote);
        detached(&t.root);
    }
}

fn public_tree(root: &Path) -> Tree {
    Tree::of(&root.join(".git"))
        .without(&[&root.join(".git/dupe"), &root.join(".git/info/exclude")])
}

fn succeeded(output: &Output) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
}

fn visible(s: &Scenario, root: &Path, paths: &[&str]) {
    let status = s.git(["status", "--porcelain", "-z"]).from(root).succeeds();
    let records: Vec<&[u8]> = status.stdout.split(|&b| b == 0).collect();
    for path in paths {
        let record = format!("?? {path}");
        assert!(records.contains(&record.as_bytes()), "{status:?}");
    }
}

fn push(s: &Scenario, root: &Path) {
    s.git(["dupe", "push", "origin", "main"])
        .from(root)
        .succeeds();
}

#[test]
fn leaves_files_user_exclude_public_git_and_remotes_from_root_or_notes() {
    under_each_release(|s| {
        for below in [false, true] {
            let t = s.pushed_workspace(if below { "below" } else { "root" });
            developer_exclude(&t.root);
            let before = Preserved::of(&t);
            let from = if below {
                t.root.join("notes")
            } else {
                t.root.clone()
            };
            let output = s.git(["dupe", "detach"]).from(&from).run();
            succeeded(&output);
            warnings(&output, daily_warnings());
            before.after_detach(&t);
            let exclude = t.root.join(".git/info/exclude");
            assert_eq!(
                fs::read(&exclude).unwrap(),
                [USER_BEFORE, USER_AFTER].concat()
            );
            assert_eq!(
                fs::metadata(exclude).unwrap().permissions().mode() & 0o7777,
                0o640
            );
            visible(s, &t.root, &["notes/", ".gitdupe"]);
        }
    });
}

#[test]
fn absent_listed_path_is_not_named() {
    under_each_release(|s| {
        let t = s.pushed_workspace("absent");
        write(&t.root, ".gitdupe", b"notes\n.vscode\nscratch\n");
        private_add(s, &t.root, ".gitdupe");
        private_commit(s, &t.root);
        push(s, &t.root);
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach"]).from(&t.root).run();
        succeeded(&output);
        warnings(&output, daily_warnings());
        before.after_detach(&t);
    });
}

#[test]
fn listed_path_beyond_a_link_warns_without_losing_other_exposure_answers() {
    under_each_release(|s| {
        let t = s.pushed_workspace("ancestor-link");
        write(s.dir(), "target/x", b"private\n");
        symlink(s.dir().join("target"), t.root.join("linked")).unwrap();
        write(&t.root, ".gitdupe", b"notes\n.vscode\nlinked/x\n");
        private_add(s, &t.root, ".gitdupe");
        private_commit(s, &t.root);
        push(s, &t.root);
        let target = Tree::of(&s.dir().join("target"));
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach"]).from(&t.root).run();
        succeeded(&output);
        let mut expected = daily_warnings();
        expected.push(b"linked/x lies beyond a symbolic link, where public Git does not look; whether it is ignored was not asked".to_vec());
        warnings(&output, expected);
        before.after_detach(&t);
        unchanged(&target, &s.dir().join("target"));
    });
}

#[test]
fn force_distinguishes_a_hand_released_path_from_one_still_hidden() {
    under_each_release(|s| {
        for edited in [false, true] {
            let t = s.pushed_workspace(if edited { "released" } else { "hidden" });
            write(&t.root, "scratch/todo.txt", b"todo\n");
            s.git(["dupe", "hide", "scratch"]).from(&t.root).succeeds();
            if edited {
                write(&t.root, ".gitdupe", b"notes\n.vscode\n");
            }
            let before = Preserved::of(&t);
            let output = s.git(["dupe", "detach", "--force"]).from(&t.root).run();
            succeeded(&output);
            let mut expected = daily_warnings();
            expected.push(if edited {
                b"scratch is no longer hidden and is visible to public Git".to_vec()
            } else {
                now_visible("scratch")
            });
            warnings(&output, expected);
            before.after_detach(&t);
            visible(s, &t.root, &["scratch/"]);
        }
    });
}

#[test]
fn a_privately_tracked_dangling_link_is_left_and_named() {
    under_each_release(|s| {
        let t = s.pushed_workspace("dangling");
        symlink("missing-target", t.root.join("dangling")).unwrap();
        private_add(s, &t.root, "dangling");
        private_commit(s, &t.root);
        push(s, &t.root);
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach"]).from(&t.root).run();
        succeeded(&output);
        let mut expected = daily_warnings();
        expected.push(now_visible("dangling"));
        warnings(&output, expected);
        before.after_detach(&t);
        assert_eq!(
            fs::read_link(t.root.join("dangling")).unwrap(),
            Path::new("missing-target")
        );
    });
}

#[test]
fn missing_gitdupe_refuses_then_force_uses_the_staged_listing_without_recreating_it() {
    under_each_release(|s| {
        let t = s.pushed_workspace("missing-list");
        fs::remove_file(t.root.join(".gitdupe")).unwrap();
        let everything = t.everything();
        let refusal = s.git(["dupe", "detach"]).from(&t.root).run();
        assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
        assert!(refusal.stdout.is_empty(), "{refusal:?}");
        names(refusal.only_line("fatal"), b"changes not committed");
        everything.unchanged();
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach", "--force"]).from(&t.root).run();
        succeeded(&output);
        warnings(
            &output,
            ["notes", "notes/a.md", "docs/notes.md"]
                .into_iter()
                .map(now_visible)
                .collect(),
        );
        before.after_detach(&t);
        assert!(!t.root.join(".gitdupe").exists());
    });
}

#[test]
fn exclude_link_is_replaced_with_user_bytes_and_its_target_is_untouched() {
    under_each_release(|s| {
        let t = s.pushed_workspace("exclude-link");
        developer_exclude(&t.root);
        let exclude = t.root.join(".git/info/exclude");
        let target = s.dir().join("exclude-target");
        fs::rename(&exclude, &target).unwrap();
        symlink(&target, &exclude).unwrap();
        let target_before = fs::read(&target).unwrap();
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach"]).from(&t.root).run();
        succeeded(&output);
        let mut expected = daily_warnings();
        let replacement = output
            .lines("warning")
            .into_iter()
            .filter(|line| holds(line, b".git/info/exclude"))
            .collect::<Vec<_>>();
        assert_eq!(replacement.len(), 1, "{output:?}");
        names(
            replacement[0],
            b"was a symbolic link; it is now a regular file",
        );
        names(replacement[0], b"its target is untouched");
        names(replacement[0], exclude.as_os_str().as_bytes());
        expected.push(replacement[0].to_vec());
        warnings(&output, expected);
        before.after_detach(&t);
        assert!(fs::symlink_metadata(&exclude).unwrap().is_file());
        assert_eq!(
            fs::read(exclude).unwrap(),
            [USER_BEFORE, USER_AFTER].concat()
        );
        assert_eq!(fs::read(target).unwrap(), target_before);
    });
}

#[test]
fn force_does_not_create_an_absent_info_directory() {
    under_each_release(|s| {
        let root = s.dir().join("absent-info");
        s.attached_repository(&root);
        fs::remove_dir_all(root.join(".git/info")).unwrap();
        let before = Tree::of(&root).without(&[&root.join(".git/dupe")]);
        let output = s.git(["dupe", "detach", "--force"]).from(&root).run();
        succeeded(&output);
        warnings(&output, vec![]);
        detached(&root);
        assert!(!root.join(".git/info").exists());
        assert!(before.changed_in(&Tree::of(&root)).is_empty());
    });
}

#[test]
fn force_removes_modified_stashed_and_unpushed_history_but_keeps_disk_bytes() {
    under_each_release(|s| {
        let t = s.pushed_workspace("force-history");
        developer_exclude(&t.root);
        s.private(&t.root)
            .git(["config", "user.name", "Scenario"])
            .succeeds();
        s.private(&t.root)
            .git(["config", "user.email", "scenario@example.invalid"])
            .succeeds();
        write(&t.root, "notes/a.md", b"stashed\n");
        s.private(&t.root)
            .git(["stash", "push", "-qm", "private stash"])
            .succeeds();
        assert!(
            !s.private(&t.root)
                .git(["stash", "list", "--format=%H"])
                .succeeds()
                .stdout
                .is_empty()
        );
        write(&t.root, "docs/notes.md", b"unpushed\n");
        private_add(s, &t.root, "docs/notes.md");
        private_commit(s, &t.root);
        assert!(
            !s.private(&t.root)
                .git(["rev-list", "--branches", "--not", "--remotes"])
                .succeeds()
                .stdout
                .is_empty()
        );
        write(&t.root, "notes/a.md", b"modified on disk\n");
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach", "--force"]).from(&t.root).run();
        succeeded(&output);
        warnings(&output, daily_warnings());
        before.after_detach(&t);
        assert_eq!(
            fs::read(t.root.join("notes/a.md")).unwrap(),
            b"modified on disk\n"
        );
        assert_eq!(
            fs::read(t.root.join(".git/info/exclude")).unwrap(),
            [USER_BEFORE, USER_AFTER].concat()
        );
        assert_eq!(
            fs::metadata(t.root.join(".git/info/exclude"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o640
        );
        visible(s, &t.root, &["notes/", ".gitdupe"]);
    });
}

/// A control directory for `GIT_EXEC_PATH` whose `git`, which git-dupe finds first there,
/// kills its own run with `SIGTERM` when its words hold `check-ignore`, and otherwise
/// runs the release's `git` from the release's own exec path.
fn killing_check_ignore(s: &Scenario) -> PathBuf {
    let exec_path = s.git(["--exec-path"]).succeeds();
    let release = Path::new(OsStr::from_bytes(exec_path.stdout.trim_ascii_end())).join("git");
    let control = s.dir().join("killing-check-ignore");
    fs::create_dir(&control).unwrap();
    fs::write(
        control.join("release"),
        [release.as_os_str().as_bytes(), b"\n"].concat(),
    )
    .unwrap();
    let script = control.join("git");
    fs::write(
        &script,
        "#!/bin/sh\n\
         unset GIT_EXEC_PATH\n\
         for word; do\n    [ \"$word\" = check-ignore ] && kill -TERM $$\ndone\n\
         read -r release < \"${0%/*}/release\"\n\
         exec \"$release\" \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    control
}

#[test]
fn a_question_killed_by_a_signal_ends_detach_attached_and_force_completes_it() {
    const SIGTERM: i32 = 15;
    under_each_release(|s| {
        let control = killing_check_ignore(s);
        let commands: [(&str, &[&str]); 2] = [
            ("signalled", &["dupe", "detach"]),
            ("signalled-force", &["dupe", "detach", "--force"]),
        ];
        for (name, words) in commands {
            let t = s.pushed_workspace(name);
            developer_exclude(&t.root);
            let before = Preserved::of(&t);
            let private = Tree::of(&t.root.join(".git/dupe"));
            let signalled = s
                .git(words)
                .from(&t.root)
                .variable("GIT_EXEC_PATH", &control)
                .run();
            // The child's signal ends the command (`Composition/Front`, "Lines"), after the
            // region's deletion and before the removal: still attached, its region gone,
            // the private repository whole (`State` "`detach`").
            assert_eq!(signalled.end, End::Code(128 + SIGTERM), "{signalled:?}");
            assert!(signalled.stdout.is_empty(), "{signalled:?}");
            names(signalled.only_line("warning"), b"exposure was not checked");
            unchanged(&private, &t.root.join(".git/dupe"));
            assert!(region(&t.root).is_none(), "{signalled:?}");
            assert_eq!(
                fs::read(t.root.join(".git/info/exclude")).unwrap(),
                [USER_BEFORE, USER_AFTER].concat()
            );

            let output = s.git(["dupe", "detach", "--force"]).from(&t.root).run();
            succeeded(&output);
            warnings(&output, daily_warnings());
            before.after_detach(&t);
        }
    });
}

#[test]
fn force_with_an_unlistable_repository_uses_disk_and_standing_region() {
    under_each_release(|s| {
        let t = s.pushed_workspace("unlistable");
        fs::remove_dir_all(t.root.join(".git/dupe")).unwrap();
        fs::create_dir(t.root.join(".git/dupe")).unwrap();
        // Git's own answer to the listing git-dupe asks for, which fails here.
        let listing = s
            .private(&t.root)
            .git(["ls-files", "-z", "--full-name"])
            .run();
        assert_ne!(listing.end, End::Code(0), "{listing:?}");
        let before = Preserved::of(&t);
        let output = s.git(["dupe", "detach", "--force"]).from(&t.root).run();
        succeeded(&output);
        // Git's message comes first, and every line of git-dupe's after it is a warning.
        let ours = Output {
            stdout: Vec::new(),
            stderr: output
                .stderr
                .strip_prefix(listing.stderr.as_slice())
                .unwrap_or_else(|| panic!("Git's message first: {output:?}"))
                .to_vec(),
            end: output.end,
        };
        let unlisted = ours
            .lines("warning")
            .into_iter()
            .filter(|line| holds(line, b"cannot list the private repository"))
            .collect::<Vec<_>>();
        assert_eq!(unlisted.len(), 1, "{output:?}");
        let mut expected = [".gitdupe", "notes", "docs/notes.md"]
            .into_iter()
            .map(now_visible)
            .collect::<Vec<_>>();
        expected.push(unlisted[0].to_vec());
        before.after_detach(&t);
        warnings(&ours, expected);
    });
}

#[test]
fn after_detach_status_and_force_refuse_until_init_attaches_again() {
    under_each_release(|s| {
        let t = s.pushed_workspace("reattach");
        let output = s.git(["dupe", "detach"]).from(&t.root).run();
        succeeded(&output);
        warnings(&output, daily_warnings());
        let before = t.everything();
        for words in [&["dupe", "status"][..], &["dupe", "detach", "--force"]] {
            let refusal = s.git(words).from(&t.root).run();
            assert_eq!(refusal.end, End::Code(128), "{refusal:?}");
            assert!(refusal.stdout.is_empty(), "{refusal:?}");
            names(refusal.only_line("fatal"), b"git dupe init");
            before.unchanged();
        }
        s.git(["dupe", "init"]).from(&t.root).succeeds();
        assert_eq!(
            region_rules(&t.root),
            [b"/.gitdupe".as_slice(), b"/.vscode", b"/notes"]
        );
        s.git(["dupe", "status"]).from(&t.root).succeeds();
    });
}

#[test]
fn misuse_is_checked_after_workspace_refusals_and_never_settles() {
    under_each_release(|s| {
        let t = s.pushed_workspace("misuse");
        write(&t.root, ".gitdupe", b"notes\n.vscode\nscratch\n");
        assert!(!region_rules(&t.root).contains(&b"/scratch".to_vec()));
        let plain = s.dir().join("unattached");
        s.repository(&plain);
        let before = t.everything();
        let plain_before = Tree::of(&plain);
        let outside = s.git(locate_words(false)).run();
        assert_ne!(outside.end, End::Code(0), "{outside:?}");
        for words in [
            &["x"][..],
            &["--forc"],
            &["-f"],
            &["--force", "x"],
            &["--", "x"],
        ] {
            let command = ["dupe", "detach"]
                .into_iter()
                .chain(words.iter().copied())
                .collect::<Vec<_>>();
            let output = s.git(&command).from(&t.root).run();
            assert_eq!(output.end, End::Code(129), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            names(
                output.line_then("error", b"usage: git dupe detach [--force]\n"),
                b"git dupe detach",
            );
            before.unchanged();
            let unattached = s.git(&command).from(&plain).run();
            assert_eq!(unattached.end, End::Code(128), "{unattached:?}");
            assert!(unattached.stdout.is_empty(), "{unattached:?}");
            names(unattached.only_line("fatal"), b"git dupe init");
            unchanged(&plain_before, &plain);
            // The linked worktree of an attached main worktree is not attached itself, and
            // its refusal names no path of the main worktree's.
            let linked = s.git(&command).from(&t.linked).run();
            assert_eq!(linked.end, End::Code(128), "{linked:?}");
            assert!(linked.stdout.is_empty(), "{linked:?}");
            names(linked.only_line("fatal"), b"git dupe init");
            assert!(
                !holds(linked.only_line("fatal"), t.root.as_os_str().as_bytes()),
                "{linked:?}"
            );
            before.unchanged();
            let absent = s.git(&command).run();
            assert_eq!(absent.end, outside.end, "{absent:?}");
            assert!(absent.stdout.is_empty(), "{absent:?}");
        }
    });
}

#[test]
fn git_run_sequences_do_not_follow_private_file_counts() {
    under_each_release(|s| {
        for force in [false, true] {
            let mut sequences = Vec::new();
            for files in [5, 500] {
                let t = s.pushed_workspace(&format!("runs-{force}-{files}"));
                for index in 5..files {
                    write(&t.root, &format!("notes/file-{index}"), b"private\n");
                }
                if files > 5 {
                    private_add(s, &t.root, "notes");
                    private_commit(s, &t.root);
                    push(s, &t.root);
                }
                let listed = s.private(&t.root).git(["ls-files", "-z"]).succeeds();
                assert_eq!(listed.stdout.iter().filter(|&&b| b == 0).count(), files);
                let words = if force {
                    &["dupe", "detach", "--force"][..]
                } else {
                    &["dupe", "detach"][..]
                };
                let (output, runs) = run_traced(s.git(words).from(&t.root), &s.dir().join("trace"));
                succeeded(&output);
                detached(&t.root);
                let own = runs.own();
                let sequence = own
                    .commands()
                    .into_iter()
                    .map(<[u8]>::to_vec)
                    .collect::<Vec<_>>();
                // The private listing of the hidden paths, then `Remove`'s public listing
                // and question (`Holds/G22`).
                let expected = if force {
                    vec!["rev-parse", "ls-files", "ls-files", "check-ignore"]
                } else {
                    vec![
                        "rev-parse",
                        "status",
                        "stash",
                        "config",
                        "rev-list",
                        "ls-files",
                        "ls-files",
                        "check-ignore",
                    ]
                };
                assert_eq!(
                    sequence,
                    expected
                        .into_iter()
                        .map(|word| word.as_bytes().to_vec())
                        .collect::<Vec<_>>(),
                    "{own:?}"
                );
                sequences.push(sequence);
            }
            assert_eq!(sequences[0], sequences[1]);
        }
    });
}
