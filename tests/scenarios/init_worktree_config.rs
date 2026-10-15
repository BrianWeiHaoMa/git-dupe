//! `init` reads `core.worktree` as Git reads it and writes it where Git reads it (F2, G1):
//! after a private sparse checkout moved the key into `config.worktree`, which Git then
//! reads before the local file; under a template that turns on
//! `extensions.worktreeConfig`; and in a private repository with a worktree of its own,
//! where Git refuses `config --worktree` without that extension.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::harness::{
    End, Output, Scenario, Tree, Worktree, completed_hint, linked_on_branch, private_add,
    private_commit, unchanged, under_each_release, worktree_clone, worktree_dupe, write,
};

/// An identity in the local configuration of the project at `root`.
fn identity(s: &Scenario, root: &Path) {
    for (key, value) in [
        ("user.name", "Local Name"),
        ("user.email", "local@example.invalid"),
    ] {
        s.git(["config", "--local", key, value])
            .from(root)
            .succeeds();
    }
}

/// A project attached at `project` with an identity in its local configuration, whose
/// private history holds `.gitdupe`, listing `notes`, and `notes/a.md`.
fn project(s: &Scenario) -> Worktree {
    let root = s.dir().join("project");
    s.unattached_project(&root);
    identity(s, &root);
    s.init(&root);
    for (path, bytes) in [(".gitdupe", &b"notes\n"[..]), ("notes/a.md", b"private\n")] {
        write(&root, path, bytes);
        private_add(s, &root, path);
    }
    private_commit(s, &root);
    Worktree::read(s, &root)
}

/// The value of `key` in the private repository of `wt` as Git reads it: every file Git
/// reads for it, `config.worktree` after the local file where the extension is set.
fn read(s: &Scenario, wt: &Worktree, key: &str) -> Option<Vec<u8>> {
    let answer = wt.private(s).git(["config", "--get", key]).run();
    assert!(matches!(answer.end, End::Code(0 | 1)), "{key}: {answer:?}");
    (answer.end == End::Code(0)).then(|| answer.stdout.strip_suffix(b"\n").unwrap().to_vec())
}

/// Each key holds its value as Git reads it.
fn reads(s: &Scenario, wt: &Worktree, expected: &[(&str, &[u8])]) {
    for (key, value) in expected {
        assert_eq!(read(s, wt, key).as_deref(), Some(*value), "{key}");
    }
}

/// Plain Git given the private Git directory alone, from the scenario's directory: it
/// finds the work tree through the `core.worktree` recorded there and nothing else.
fn plain(s: &Scenario, wt: &Worktree, words: &[&str]) -> Output {
    let mut git_directory = OsString::from("--git-dir=");
    git_directory.push(wt.private_directory());
    s.git(
        [git_directory]
            .into_iter()
            .chain(words.iter().map(OsString::from)),
    )
    .from(s.dir())
    .run()
}

/// Plain Git finds `wt`'s root as the private repository's work tree.
fn plain_git_finds_the_root(s: &Scenario, wt: &Worktree) {
    let top = plain(s, wt, &["rev-parse", "--show-toplevel"]);
    assert_eq!(top.end, End::Code(0), "{top:?}");
    assert_eq!(top.stdout, [wt.root.as_os_str().as_bytes(), b"\n"].concat());
}

/// A private sparse checkout of `notes` and `.gitdupe`, which turns on
/// `extensions.worktreeConfig` and moves `core.worktree` out of the local file into
/// `config.worktree`.
fn sparse(s: &Scenario, wt: &Worktree) {
    let words = [
        "sparse-checkout",
        "set",
        "--no-cone",
        "/notes/",
        "/.gitdupe",
    ];
    worktree_dupe(s, wt, &words);
    reads(s, wt, &[("extensions.worktreeConfig", b"true")]);
    let local = wt
        .private(s)
        .git(["config", "--local", "--get", "core.worktree"])
        .run();
    assert_eq!(local.end, End::Code(1), "{local:?}");
    assert!(wt.private_directory().join("config.worktree").is_file());
}

/// `git -c <setting> dupe init` in `wt`, which must exit 0 and leave the public repository
/// as it was.
fn init_with(s: &Scenario, wt: &Worktree, setting: &str) -> Output {
    let before = wt.public_git();
    let output = s
        .git(["-c", setting, "dupe", "init"])
        .from(&wt.root)
        .succeeds();
    assert!(before.changed_in(&wt.public_git()).is_empty());
    output
}

/// A second `init` finds nothing missing and changes nothing in the private repository.
fn finished(s: &Scenario, wt: &Worktree) {
    let before = Tree::of(&wt.private_directory());
    completed_hint(&worktree_dupe(s, wt, &["init"]), false);
    unchanged(&before, &wt.private_directory());
}

#[test]
fn a_moved_linked_worktree_whose_sparse_checkout_moved_core_worktree_is_recorded_for_plain_git() {
    under_each_release(|s| {
        let main = project(s);
        let old = linked_on_branch(s, &main.root, &s.dir().join("agent-old"), "agent");
        worktree_clone(s, &old, &main.private_directory());
        sparse(s, &old);
        let new = s.dir().join("agent-new");
        s.git([
            OsStr::new("worktree"),
            OsStr::new("move"),
            old.root.as_os_str(),
            new.as_os_str(),
        ])
        .from(&main.root)
        .succeeds();
        let moved = Worktree::read(s, &new);
        let stale = plain(s, &moved, &["rev-parse", "--show-toplevel"]);
        assert_eq!(stale.end, End::Code(128), "{stale:?}");

        completed_hint(&worktree_dupe(s, &moved, &["init"]), true);
        // From `<common Git directory>/worktrees/agent-old/dupe`, the name kept by the move.
        reads(
            s,
            &moved,
            &[
                ("core.worktree", b"../../../../../agent-new"),
                ("status.showUntrackedFiles", b"no"),
                ("advice.statusHints", b"false"),
            ],
        );
        plain_git_finds_the_root(s, &moved);
        write(&new, "notes/a.md", b"edited at the new place\n");
        let status = plain(s, &moved, &["status", "--porcelain"]);
        assert_eq!(status.end, End::Code(0), "{status:?}");
        assert_eq!(status.stdout, b" M notes/a.md\n", "{status:?}");
        finished(s, &moved);
    });
}

#[test]
fn a_finished_attachment_whose_sparse_checkout_moved_core_worktree_keeps_its_edited_settings() {
    under_each_release(|s| {
        let main = project(s);
        let edited: [(&str, &[u8]); 3] = [
            ("user.name", b"Edited Name"),
            ("status.showUntrackedFiles", b"all"),
            ("advice.statusHints", b"true"),
        ];
        for (key, value) in edited {
            let value = std::str::from_utf8(value).unwrap();
            worktree_dupe(s, &main, &["config", key, value]);
        }
        sparse(s, &main);
        finished(s, &main);
        reads(s, &main, &edited);
        reads(
            s,
            &main,
            &[
                ("user.email", b"local@example.invalid"),
                ("core.worktree", b"../.."),
            ],
        );
        plain_git_finds_the_root(s, &main);
    });
}

#[test]
fn a_template_that_turns_on_worktree_configuration_gets_every_key_where_git_reads_it() {
    under_each_release(|s| {
        // Without a `config.worktree`, with one naming another place, and with one already
        // naming the root as `init` records it: Git reads that file before the local one.
        let worktree_files = [
            None,
            Some(&b"[core]\n worktree = /nowhere\n"[..]),
            Some(b"[core]\n worktree = ../..\n"),
        ];
        for (index, worktree_file) in worktree_files.into_iter().enumerate() {
            let template = s.dir().join(format!("template-{index}"));
            fs::create_dir(&template).unwrap();
            fs::write(
                template.join("config"),
                b"[extensions]\n worktreeConfig = true\n\
                  [status]\n showUntrackedFiles = all\n\
                  [advice]\n statusHints = true\n\
                  [user]\n name = Template Name\n",
            )
            .unwrap();
            if let Some(bytes) = worktree_file {
                fs::write(template.join("config.worktree"), bytes).unwrap();
            }
            let mut template_dir = OsString::from("init.templateDir=");
            template_dir.push(&template);
            let root = s.dir().join(format!("project-{index}"));
            s.repository(&root);
            identity(s, &root);
            let wt = Worktree::read(s, &root);
            let output = s
                .git([
                    OsString::from("-c"),
                    template_dir,
                    "dupe".into(),
                    "init".into(),
                ])
                .from(&root)
                .run();
            assert_eq!(output.end, End::Code(0), "{output:?}");
            assert!(output.lines("fatal").is_empty(), "{output:?}");
            assert!(output.lines("hint").is_empty(), "{output:?}");
            reads(
                s,
                &wt,
                &[
                    ("extensions.worktreeConfig", b"true"),
                    ("core.worktree", b"../.."),
                    ("status.showUntrackedFiles", b"no"),
                    ("advice.statusHints", b"false"),
                    ("user.name", b"Local Name"),
                    ("user.email", b"local@example.invalid"),
                ],
            );
            plain_git_finds_the_root(s, &wt);
            finished(s, &wt);
        }
    });
}

#[test]
fn a_core_worktree_git_takes_no_work_tree_from_neither_stands_for_nor_hides_the_recorded_one() {
    under_each_release(|s| {
        let main = project(s);
        let edited: [(&str, &[u8]); 3] = [
            ("user.name", b"Edited Name"),
            ("status.showUntrackedFiles", b"all"),
            ("advice.statusHints", b"true"),
        ];
        for (key, value) in edited {
            let value = std::str::from_utf8(value).unwrap();
            worktree_dupe(s, &main, &["config", key, value]);
        }
        // Finished: a value given on the command line, or reached through an include, is
        // no record of the repository's, and Git takes no work tree from it.
        let before = Tree::of(&main.private_directory());
        completed_hint(&init_with(s, &main, "core.worktree=/irrelevant"), false);
        unchanged(&before, &main.private_directory());
        fs::write(
            main.private_directory().join("included"),
            b"[core]\n worktree = /nowhere\n",
        )
        .unwrap();
        worktree_dupe(s, &main, &["config", "include.path", "included"]);
        reads(s, &main, &[("core.worktree", b"/nowhere")]);
        plain_git_finds_the_root(s, &main);
        finished(s, &main);
        reads(s, &main, &edited);
        worktree_dupe(s, &main, &["config", "--unset", "include.path"]);

        // Unfinished: `config.worktree` names another place, which Git reads first.
        worktree_dupe(s, &main, &["config", "extensions.worktreeConfig", "true"]);
        let words = ["config", "--worktree", "core.worktree", "/missing"];
        worktree_dupe(s, &main, &words);
        let stale = plain(s, &main, &["status", "--porcelain"]);
        assert_eq!(stale.end, End::Code(128), "{stale:?}");
        completed_hint(&init_with(s, &main, "core.worktree=/another"), true);
        reads(s, &main, &[("core.worktree", b"../..")]);
        plain_git_finds_the_root(s, &main);
        finished(s, &main);
    });
}

#[test]
fn an_unfinished_attachment_whose_private_repository_has_a_worktree_of_its_own_is_completed() {
    under_each_release(|s| {
        let main = project(s);
        let side = s.dir().join("private-side");
        let side = side.to_str().unwrap();
        worktree_dupe(s, &main, &["worktree", "add", "-q", side, "-b", "side"]);
        // An `init` that did not finish: the root recorded as Git's `init` records it.
        let root = main.root.to_str().unwrap();
        worktree_dupe(s, &main, &["config", "core.worktree", root]);
        worktree_dupe(s, &main, &["config", "status.showUntrackedFiles", "all"]);
        assert!(read(s, &main, "extensions.worktreeConfig").is_none());

        completed_hint(&worktree_dupe(s, &main, &["init"]), true);
        reads(
            s,
            &main,
            &[
                ("core.worktree", b"../.."),
                ("status.showUntrackedFiles", b"no"),
                ("advice.statusHints", b"false"),
                ("user.name", b"Local Name"),
            ],
        );
        plain_git_finds_the_root(s, &main);
        finished(s, &main);
    });
}
