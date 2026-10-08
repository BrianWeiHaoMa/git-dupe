//! Relative private work trees survive main-project renames, and `init` completes the
//! settings of linked worktrees moved by Git or left unfinished (F2, G1, G21).

use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Output, Point, Scenario, Tree, Worktree, names, unchanged, under_each_release,
};

fn git_directory(path: &Path) -> OsString {
    let mut word = OsString::from("--git-dir=");
    word.push(path);
    word
}

/// Plain Git, without the harness's explicit --work-tree, must resolve the stored path.
fn plain_top(s: &Scenario, wt: &Worktree, from: &Path) {
    let output = s
        .git([
            git_directory(&wt.private_directory()),
            "rev-parse".into(),
            "--show-toplevel".into(),
        ])
        .from(from)
        .succeeds();
    assert_eq!(
        output.stdout,
        [wt.root.as_os_str().as_bytes(), b"\n"].concat(),
        "{output:?}"
    );
}

fn key(s: &Scenario, wt: &Worktree, name: &str) -> Vec<u8> {
    wt.private(s)
        .git(["config", "--local", "--get", name])
        .succeeds()
        .stdout
        .strip_suffix(b"\n")
        .unwrap()
        .to_vec()
}

/// Compute the specification's lexical relative path from independently read Git facts.
fn relative(wt: &Worktree) -> Vec<u8> {
    let from = fs::canonicalize(&wt.git_directory).unwrap().join("dupe");
    let to = fs::canonicalize(&wt.root).unwrap();
    let common = from
        .ancestors()
        .find(|ancestor| to.starts_with(ancestor))
        .unwrap();
    let mut path = PathBuf::new();
    for _ in from.strip_prefix(common).unwrap().components() {
        path.push("..");
    }
    path.push(to.strip_prefix(common).unwrap());
    path.as_os_str().as_bytes().to_vec()
}

fn relative_and_resolved(s: &Scenario, wt: &Worktree) {
    let value = key(s, wt, "core.worktree");
    assert!(!value.starts_with(b"/"));
    assert_eq!(value, relative(wt));
    plain_top(s, wt, s.dir());
}

fn completed_hint(output: &Output, completed: bool) {
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let line = output.only_line("hint");
    names(line, b"already attached");
    names(
        line,
        if completed {
            b"completed its settings"
        } else {
            b"nothing was missing"
        },
    );
}

fn init(s: &Scenario, wt: &Worktree) -> Output {
    let before = wt.public_git();
    let output = s.git(["dupe", "init"]).from(&wt.root).succeeds();
    assert!(before.changed_in(&wt.public_git()).is_empty());
    output
}

fn linked(s: &Scenario, main: &Path, root: &Path, branch: &str) -> Worktree {
    s.git([
        OsStr::new("worktree"),
        OsStr::new("add"),
        OsStr::new("-q"),
        root.as_os_str(),
        OsStr::new("-b"),
        OsStr::new(branch),
    ])
    .from(main)
    .succeeds();
    Worktree::read(s, root)
}

#[test]
fn main_private_worktree_follows_a_project_rename() {
    under_each_release(|s| {
        let root = s.dir().join("project");
        s.repository(&root);
        let wt = Worktree::read(s, &root);
        init(s, &wt);
        assert_eq!(key(s, &wt, "core.worktree"), b"../..");
        plain_top(s, &wt, &root);
        let new = s.dir().join("renamed");
        fs::rename(&root, &new).unwrap();
        let wt = Worktree::read(s, &new);
        s.git(["dupe", "status"]).from(&new).succeeds();
        s.git([git_directory(&wt.private_directory()), "status".into()])
            .from(&new)
            .succeeds();
        plain_top(s, &wt, &new);
        assert_eq!(key(s, &wt, "core.worktree"), b"../..");
    });
}

#[test]
fn linked_init_uses_its_public_branch_and_included_local_identity() {
    under_each_release(|s| {
        let main = s.dir().join("project");
        s.repository(&main);
        let included = s.dir().join("identity");
        fs::write(
            &included,
            b"[user]\n name = Included Name\n email = included@example.invalid\n",
        )
        .unwrap();
        s.git([
            OsStr::new("config"),
            OsStr::new("--local"),
            OsStr::new("include.path"),
            included.as_os_str(),
        ])
        .from(&main)
        .succeeds();
        assert_eq!(
            s.git(["symbolic-ref", "--short", "HEAD"])
                .from(&main)
                .succeeds()
                .stdout,
            b"main\n"
        );
        for (directory, branch, explicit) in [
            ("project-agent", "agent", false),
            ("project-second", "second", true),
        ] {
            let wt = linked(s, &main, &s.dir().join(directory), branch);
            let before = wt.public_git();
            let words = if explicit {
                &["dupe", "init", "-b", "other"][..]
            } else {
                &["dupe", "init"][..]
            };
            s.git(words).from(&wt.root).succeeds();
            assert!(before.changed_in(&wt.public_git()).is_empty());
            relative_and_resolved(s, &wt);
            assert_eq!(
                wt.private(s)
                    .git(["symbolic-ref", "--short", "HEAD"])
                    .succeeds()
                    .stdout,
                if explicit {
                    b"other\n".as_slice()
                } else {
                    b"agent\n"
                }
            );
            assert_eq!(
                wt.private(s).git(["config", "user.name"]).succeeds().stdout,
                b"Included Name\n"
            );
            assert_eq!(
                wt.private(s)
                    .git(["config", "user.email"])
                    .succeeds()
                    .stdout,
                b"included@example.invalid\n"
            );
        }
    });
}

fn moved_init(s: &Scenario, recreate_old: bool) {
    let main = s.dir().join("project");
    s.attached_project(&main);
    let old = s.dir().join("agent-old");
    let new = s.dir().join("agent-new");
    let wt = linked(s, &main, &old, "agent");
    init(s, &wt);
    relative_and_resolved(s, &wt);
    let old_value = key(s, &wt, "core.worktree");
    let exclude = wt.common_directory.join("info/exclude");
    let markers = fs::read(&exclude).unwrap();
    let region = wt.region();
    s.git([
        OsStr::new("worktree"),
        OsStr::new("move"),
        old.as_os_str(),
        new.as_os_str(),
    ])
    .from(&main)
    .succeeds();
    let moved = Worktree::read(s, &new);
    assert_eq!(moved.git_directory, wt.git_directory);
    assert_eq!(moved.name(), wt.name());
    assert_eq!(moved.region(), region);
    assert_eq!(fs::read(&exclude).unwrap(), markers);
    assert_eq!(key(s, &moved, "core.worktree"), old_value);
    assert_ne!(old_value, relative(&moved));
    let before = moved.public_git();
    s.git(["dupe", "status"]).from(&new).succeeds();
    assert!(before.changed_in(&moved.public_git()).is_empty());
    let failed = s
        .git([git_directory(&moved.private_directory()), "status".into()])
        .from(&new)
        .run();
    assert_eq!(failed.end, End::Code(128), "{failed:?}");
    if recreate_old {
        fs::create_dir(&old).unwrap();
        // The obsolete relative spelling now resolves successfully, to the wrong root.
        let top = s
            .git([
                git_directory(&moved.private_directory()),
                "rev-parse".into(),
                "--show-toplevel".into(),
            ])
            .from(&new)
            .succeeds();
        assert_eq!(top.stdout, [old.as_os_str().as_bytes(), b"\n"].concat());
    }
    let config_path = moved.private_directory().join("config");
    let private_before = Tree::of(&moved.private_directory()).without(&[&config_path]);
    completed_hint(&init(s, &moved), true);
    assert!(
        private_before
            .changed_in(&Tree::of(&moved.private_directory()).without(&[&config_path]))
            .is_empty()
    );
    relative_and_resolved(s, &moved);
    plain_top(s, &moved, &new);
    let config = fs::read(moved.private_directory().join("config")).unwrap();
    let private = Tree::of(&moved.private_directory());
    completed_hint(&init(s, &moved), false);
    assert_eq!(
        fs::read(moved.private_directory().join("config")).unwrap(),
        config
    );
    unchanged(&private, &moved.private_directory());
    assert_eq!(fs::read(&exclude).unwrap(), markers);
}

#[test]
fn moved_linked_worktree_is_completed_once_by_init() {
    under_each_release(|s| {
        moved_init(s, false);
    });
}

#[test]
fn moved_linked_worktree_is_repaired_even_when_the_old_root_exists() {
    under_each_release(|s| {
        moved_init(s, true);
    });
}

#[test]
fn absolute_worktree_spelling_completes_settings_but_finished_init_preserves_edits() {
    under_each_release(|s| {
        let main = s.dir().join("project");
        s.repository(&main);
        let wt = linked(s, &main, &s.dir().join("agent"), "agent");
        init(s, &wt);
        wt.private(s)
            .git([
                OsStr::new("config"),
                OsStr::new("core.worktree"),
                wt.root.as_os_str(),
            ])
            .succeeds();
        wt.private(s)
            .git(["config", "status.showUntrackedFiles", "all"])
            .succeeds();
        plain_top(s, &wt, s.dir());
        completed_hint(&init(s, &wt), true);
        relative_and_resolved(s, &wt);
        assert_eq!(key(s, &wt, "status.showUntrackedFiles"), b"no");
        wt.private(s)
            .git(["config", "status.showUntrackedFiles", "all"])
            .succeeds();
        let before = Tree::of(&wt.private_directory());
        completed_hint(&init(s, &wt), false);
        assert_eq!(key(s, &wt, "status.showUntrackedFiles"), b"all");
        unchanged(&before, &wt.private_directory());
    });
}

#[test]
fn linked_init_records_canonical_git_directory_and_non_utf8_root_bytes() {
    under_each_release(|s| {
        let main = s.dir().join("project");
        s.repository(&main);
        let wt = linked(s, &main, &s.dir().join("agent"), "agent");
        let via = s.dir().join("via");
        symlink(wt.common_directory.join("worktrees"), &via).unwrap();
        let through_link = via.join(wt.name().unwrap());
        let before = wt.public_git();
        s.git([git_directory(&through_link), "dupe".into(), "init".into()])
            .from(&wt.root)
            .succeeds();
        assert!(before.changed_in(&wt.public_git()).is_empty());
        relative_and_resolved(s, &wt);
        completed_hint(&init(s, &wt), false);
        let bytes = linked(
            s,
            &main,
            &s.dir().join(OsStr::from_bytes(b"caf\xe9")),
            "bytes",
        );
        init(s, &bytes);
        relative_and_resolved(s, &bytes);
        completed_hint(&init(s, &bytes), false);
    });
}

#[test]
fn killed_linked_init_completes_without_changing_the_main_attachment() {
    under_each_release(|s| {
        let main = s.dir().join("project");
        s.attached_project(&main);
        s.git(["dupe", "hide", "main-notes"]).from(&main).succeeds();
        let main_wt = Worktree::read(s, &main);
        let main_private = Tree::of(&main_wt.private_directory());
        let main_region = main_wt.region_bytes();
        let main_marker = fs::read(main.join(".gitdupe")).unwrap();
        let killing = s.killing("control", &["init"]);
        let uninterrupted = linked(s, &main, &s.dir().join("uninterrupted"), "uninterrupted");
        let (output, points) = killing.uninterrupted(&uninterrupted.root);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        relative_and_resolved(s, &uninterrupted);
        unchanged(&main_private, &main_wt.private_directory());
        assert_eq!(main_wt.region_bytes(), main_region);
        let runs: Vec<_> = points
            .into_iter()
            .filter(|point| matches!(point, Point::BeforeRun(_) | Point::AfterRun(_)))
            .collect();
        assert!(!runs.is_empty());
        for (index, point) in runs.into_iter().enumerate() {
            // Git must create each fresh linked fixture; copying retains stale backlinks.
            let branch = format!("agent-{index}");
            let wt = linked(s, &main, &s.dir().join(&branch), &branch);
            let public_before = wt.public_git();
            killing.killed(&wt.root, point);
            assert!(
                public_before.changed_in(&wt.public_git()).is_empty(),
                "{point:?}"
            );
            unchanged(&main_private, &main_wt.private_directory());
            assert_eq!(main_wt.region_bytes(), main_region);
            for name in ["config.lock", "index.lock", "HEAD.lock"] {
                let lock = wt.private_directory().join(name);
                if lock.exists() {
                    fs::remove_file(lock).unwrap();
                }
            }
            init(s, &wt);
            relative_and_resolved(s, &wt);
            assert_eq!(key(s, &wt, "status.showUntrackedFiles"), b"no", "{point:?}");
            assert_eq!(key(s, &wt, "advice.statusHints"), b"false", "{point:?}");
            let before = wt.public_git();
            s.git(["dupe", "status"]).from(&wt.root).succeeds();
            assert!(before.changed_in(&wt.public_git()).is_empty());
            assert!(wt.region().unwrap().rules.contains(&b"/.gitdupe".to_vec()));
            unchanged(&main_private, &main_wt.private_directory());
            assert_eq!(main_wt.region_bytes(), main_region);
            assert_eq!(fs::read(main.join(".gitdupe")).unwrap(), main_marker);
        }
    });
}
