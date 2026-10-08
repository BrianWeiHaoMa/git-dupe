//! Linked worktrees clone and exchange their own private history (G2, G18, G21,
//! G28, In use 11), leaving public history and other worktrees' private state alone.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Scenario, Tree, Worktree, names, private_add, private_commit, unchanged,
    under_each_release, warnings_in_any_order, write,
};

fn history(s: &Scenario) -> Worktree {
    let root = s.dir().join("project");
    s.attached_project(&root);
    for (path, bytes) in [
        (".env.local", &b"private settings\n"[..]),
        ("notes/a.md", b"private note\n"),
        (".gitdupe", b"notes\n"),
    ] {
        write(&root, path, bytes);
        private_add(s, &root, path);
    }
    private_commit(s, &root);
    s.git(["dupe", "status"]).from(&root).succeeds();
    Worktree::read(s, &root)
}

fn linked(s: &Scenario, main: &Worktree) -> Worktree {
    s.git(["worktree", "add", "-q", "../project-agent", "-b", "agent"])
        .from(&main.root)
        .succeeds();
    Worktree::read(s, &s.dir().join("project-agent"))
}

/// One clone may write only its own private repository and the shared exclude file.
fn clone(s: &Scenario, wt: &Worktree, url: &Path, branch: Option<&str>) -> Output {
    let public = wt.public_git();
    let mut words = vec![OsStr::new("dupe"), OsStr::new("clone"), url.as_os_str()];
    if let Some(branch) = branch {
        words.extend([OsStr::new("-b"), OsStr::new(branch)]);
    }
    let output = s.git(words).from(&wt.root).run();
    let changed = public.changed_in(&wt.public_git());
    assert!(changed.is_empty(), "{output:?}: changed {changed:?}");
    output
}

fn origin(s: &Scenario, wt: &Worktree, url: &Path) {
    assert!(wt.private_directory().join("HEAD").is_file());
    let config = wt
        .private(s)
        .git(["config", "--get", "remote.origin.url"])
        .succeeds();
    assert_eq!(config.stdout, [url.as_os_str().as_bytes(), b"\n"].concat());
}

fn arrived(s: &Scenario, main: &Worktree, wt: &Worktree) {
    for path in [".gitdupe", ".env.local", "notes/a.md"] {
        assert_eq!(
            fs::read(wt.root.join(path)).unwrap(),
            fs::read(main.root.join(path)).unwrap(),
            "{path}"
        );
    }
    assert_eq!(
        wt.region().unwrap().rules,
        [
            b"/.env.local".to_vec(),
            b"/.gitdupe".to_vec(),
            b"/notes".to_vec()
        ]
    );
    let public = s
        .git(["status", "--porcelain", "--untracked-files=all"])
        .from(&wt.root)
        .succeeds();
    assert!(public.stdout.is_empty(), "{public:?}");
    let staged = wt.private(s).git(["diff", "--cached", "--quiet"]).run();
    assert_eq!(staged.end, End::Code(0), "{staged:?}");
    let status = s
        .git(["dupe", "status", "--porcelain", "-z"])
        .from(&wt.root)
        .succeeds();
    assert!(status.stdout.is_empty(), "{status:?}");
    assert_eq!(
        wt.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout,
        main.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout
    );
}

fn bare_remote(s: &Scenario, main: &Worktree) -> PathBuf {
    let remote = s.dir().join("private.git");
    s.bare_repository(&remote);
    main.private(s)
        .git([
            OsStr::new("push"),
            remote.as_os_str(),
            OsStr::new("HEAD:main"),
        ])
        .succeeds();
    s.git(["symbolic-ref", "HEAD", "refs/heads/main"])
        .from(&remote)
        .succeeds();
    remote
}

#[test]
fn linked_clone_admits_the_common_private_repository_and_attaches_only_here() {
    under_each_release(|s| {
        let main = history(s);
        let wt = linked(s, &main);
        let url = wt.common_directory.join("dupe");
        assert_eq!(url, main.private_directory());
        let source = Tree::of(&url);
        let main_region = main.region_bytes();
        let output = clone(s, &wt, &url, None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        for level in ["warning", "hint", "fatal", "error"] {
            assert!(output.lines(level).is_empty(), "{output:?}");
        }
        origin(s, &wt, &url);
        arrived(s, &main, &wt);
        assert_eq!(main.region_bytes(), main_region);
        unchanged(&source, &url);

        // Compare Git's own init answer at the same Git directory, without asserting
        // the wording of its message or guessing the worktree's administrative name.
        s.git(["dupe", "detach", "--force"])
            .from(&wt.root)
            .succeeds();
        let init = s
            .git(["init", "--initial-branch=main"])
            .from(&wt.root)
            .variable("GIT_DIR", wt.private_directory())
            .variable("GIT_WORK_TREE", &wt.root)
            .succeeds();
        assert_eq!(output.stdout, init.stdout);
        unchanged(&source, &url);
        assert_eq!(main.region_bytes(), main_region);
    });
}

#[test]
fn linked_clone_keeps_differing_files_and_a_file_obstructing_private_notes() {
    under_each_release(|s| {
        let main = history(s);
        let wt = linked(s, &main);
        for (path, bytes) in [
            (".env.local", &b"local settings\n"[..]),
            (".gitdupe", b"scratch\n"),
            ("notes", b"local file\n"),
        ] {
            write(&wt.root, path, bytes);
        }
        let working = Tree::working(&wt.root);
        let source = Tree::of(&main.private_directory());
        let main_region = main.region_bytes();
        let output = clone(s, &wt, &wt.common_directory.join("dupe"), None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        // G27: main hides notes, while the kept .gitdupe lists only scratch
        // and the private index hides notes/a.md, not its obstructing parent.
        let foreign_warning = &b"notes stands here and is hidden by the main worktree alone: public Git ignores it here, and this worktree does not hide it; run from the root, 'git dupe hide -- notes' hides it here too"[..];
        warnings_in_any_order(
            &output,
            &[
                &[b".env.local was kept", b"differs"],
                &[b".gitdupe was kept", b"differs"],
                &[b"notes was kept", b"file"],
                &[foreign_warning],
            ],
        );
        assert!(
            output.lines("warning").contains(&foreign_warning),
            "{output:?}"
        );
        assert!(working.changed_in(&Tree::working(&wt.root)).is_empty());
        assert!(wt.root.join("notes").is_file());
        assert!(!wt.root.join("notes/a.md").exists());
        let status = s
            .git(["dupe", "status", "--porcelain", "-z"])
            .from(&wt.root)
            .succeeds();
        assert_eq!(
            status.stdout,
            b" M .env.local\0 M .gitdupe\0 D notes/a.md\0"
        );
        assert!(wt.region().unwrap().rules.contains(&b"/scratch".to_vec()));
        assert_eq!(main.region_bytes(), main_region);
        unchanged(&source, &main.private_directory());
    });
}

#[test]
fn private_push_and_merge_exchange_worktree_history_without_public_refs_or_commits() {
    under_each_release(|s| {
        let main = history(s);
        let wt = linked(s, &main);
        let output = clone(s, &wt, &wt.common_directory.join("dupe"), None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        write(&wt.root, "notes/a.md", b"agent note\n");
        s.git(["dupe", "add", "notes/a.md"])
            .from(&wt.root)
            .succeeds();
        s.git([
            "-c",
            "maintenance.auto=false",
            "-c",
            "user.name=Scenario",
            "-c",
            "user.email=scenario@example.invalid",
            "dupe",
            "commit",
            "-m",
            "agent note",
        ])
        .from(&wt.root)
        .succeeds();
        let commit = wt.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout;
        let public_history = |root: &Path| {
            [
                s.git(["log", "--all", "--format=%H"])
                    .from(root)
                    .succeeds()
                    .stdout,
                s.git(["for-each-ref", "--format=%(refname) %(objectname)"])
                    .from(root)
                    .succeeds()
                    .stdout,
            ]
        };
        let before = [public_history(&main.root), public_history(&wt.root)];
        let main_region = main.region_bytes();
        let agent_region = wt.region_bytes();
        for (root, words) in [
            (&wt.root, &["dupe", "push", "origin", "HEAD:agent"][..]),
            (&main.root, &["dupe", "merge", "agent"][..]),
        ] {
            s.git(words).from(root).succeeds();
            assert_eq!(
                [public_history(&main.root), public_history(&wt.root)],
                before
            );
            assert_eq!(
                main.private(s)
                    .git(["rev-parse", "agent"])
                    .succeeds()
                    .stdout,
                commit
            );
        }
        assert_eq!(
            main.private(s).git(["rev-parse", "HEAD"]).succeeds().stdout,
            commit
        );
        assert_eq!(
            fs::read(main.root.join("notes/a.md")).unwrap(),
            b"agent note\n"
        );
        assert_eq!(main.region_bytes(), main_region);
        assert_eq!(wt.region_bytes(), agent_region);
    });
}

#[test]
fn a_second_linked_worktree_clones_a_separate_private_remote_independently() {
    under_each_release(|s| {
        let main = history(s);
        let first = linked(s, &main);
        let output = clone(s, &first, &first.common_directory.join("dupe"), None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let remote = bare_remote(s, &main);
        let root = s.dir().join("project-second-agent");
        s.linked_worktree(&main.root, &root);
        let second = Worktree::read(s, &root);
        let main_private = Tree::of(&main.private_directory());
        let first_private = Tree::of(&first.private_directory());
        let first_working = Tree::working(&first.root);
        let regions = [main.region_bytes(), first.region_bytes()];
        let remote_tree = Tree::of(&remote);
        let output = clone(s, &second, &remote, None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert!(output.lines("warning").is_empty(), "{output:?}");
        origin(s, &second, &remote);
        arrived(s, &main, &second);
        assert_ne!(second.private_directory(), first.private_directory());
        assert_eq!([main.region_bytes(), first.region_bytes()], regions);
        unchanged(&main_private, &main.private_directory());
        unchanged(&first_private, &first.private_directory());
        assert!(
            first_working
                .changed_in(&Tree::working(&first.root))
                .is_empty()
        );
        unchanged(&remote_tree, &remote);
    });
}

#[test]
fn linked_clone_refuses_every_public_place_before_attachment() {
    under_each_release(|s| {
        let main = history(s);
        let wt = linked(s, &main);
        let other_root = s.dir().join("other-agent");
        s.linked_worktree(&main.root, &other_root);
        let other = Worktree::read(s, &other_root);
        let exclude = main.common_directory.join("info/exclude");
        let bytes = fs::read(&exclude).unwrap();
        let working = Tree::working(&wt.root);
        for place in [
            main.root.clone(),
            main.root.join(".git"),
            main.git_directory.clone(),
            wt.root.clone(),
            wt.root.join(".git"),
            wt.git_directory.clone(),
            other.root.clone(),
            other.root.join(".git"),
            other.git_directory.clone(),
        ] {
            let output = clone(s, &wt, &place, None);
            assert_eq!(output.end, End::Code(128), "{place:?}: {output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            let fatal = output.only_line("fatal");
            names(fatal, place.as_os_str().as_bytes());
            names(fatal, b"git dupe git");
            assert!(!wt.private_directory().exists());
            assert!(wt.region().is_none());
            assert_eq!(fs::read(&exclude).unwrap(), bytes);
            assert!(working.changed_in(&Tree::working(&wt.root)).is_empty());
        }
    });
}

#[test]
fn failed_linked_clones_stay_attached_and_start_over_without_changing_other_worktrees() {
    under_each_release(|s| {
        let main = history(s);
        let first = linked(s, &main);
        let output = clone(s, &first, &main.private_directory(), None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let good = main.private_directory();
        let missing = s.dir().join("nonexistent");
        for (name, url, branch) in [
            ("missing-branch", &good, Some("missing")),
            ("failed-fetch", &missing, None),
        ] {
            let root = s.dir().join(name);
            s.linked_worktree(&main.root, &root);
            let wt = Worktree::read(s, &root);
            let main_private = Tree::of(&main.private_directory());
            let first_private = Tree::of(&first.private_directory());
            let regions = [main.region_bytes(), first.region_bytes()];
            let failed = clone(s, &wt, url, branch);
            assert_ne!(failed.end, End::Code(0), "{failed:?}");
            origin(s, &wt, url);
            assert_eq!([main.region_bytes(), first.region_bytes()], regions);
            unchanged(&main_private, &main.private_directory());
            unchanged(&first_private, &first.private_directory());
            let public = wt.public_git();
            s.git(["dupe", "detach", "--force"])
                .from(&wt.root)
                .succeeds();
            assert!(public.changed_in(&wt.public_git()).is_empty());
            assert!(!wt.private_directory().exists());
            assert!(wt.region().is_none());
            let again = clone(s, &wt, &good, None);
            assert_eq!(again.end, End::Code(0), "{again:?}");
            origin(s, &wt, &good);
            arrived(s, &main, &wt);
            assert_eq!([main.region_bytes(), first.region_bytes()], regions);
            unchanged(&main_private, &main.private_directory());
            unchanged(&first_private, &first.private_directory());
        }
    });
}

#[test]
fn an_empty_private_remote_attaches_a_linked_worktree_with_one_hint_and_no_commit() {
    under_each_release(|s| {
        let main = history(s);
        let wt = linked(s, &main);
        let remote = s.dir().join("empty.git");
        s.bare_repository(&remote);
        let source = Tree::of(&main.private_directory());
        let main_region = main.region_bytes();
        let output = clone(s, &wt, &remote, None);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        assert_eq!(output.lines("hint").len(), 1, "{output:?}");
        assert!(output.lines("warning").is_empty(), "{output:?}");
        origin(s, &wt, &remote);
        let head = wt.private(s).git(["rev-parse", "--verify", "HEAD"]).run();
        assert_ne!(head.end, End::Code(0), "{head:?}");
        assert_eq!(wt.region().unwrap().rules, [b"/.gitdupe"]);
        assert_eq!(main.region_bytes(), main_region);
        unchanged(&source, &main.private_directory());
    });
}
