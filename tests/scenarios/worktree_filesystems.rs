//! A linked worktree outside E1 still refuses an indivisible `.gitdupe` replacement
//! across filesystems (F5, Composition/Keeper), then settles the existing hidden set (G6).

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::Path;

use crate::harness::{
    End, FreshDirectory, Scenario, Tree, Worktree, names, report, under_each_release, write,
};

/// One refusal leaves both indexes, all working files, and the declared hidden list
/// unchanged. The shared exclude file may acquire the existing hidden set's region.
fn replacement_refused(s: &Scenario, wt: &Worktree, words: &[&str], cause: &str) {
    let index = wt.private(s).git(["ls-files", "-s"]).succeeds().stdout;
    let public_index = s.git(["ls-files", "-s"]).from(&wt.root).succeeds().stdout;
    let listed = match fs::read(wt.root.join(".gitdupe")) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("cannot read .gitdupe: {error}"),
    };
    let files = Tree::of(&wt.root);
    let public = wt.public_git();
    let region = wt.region().unwrap();

    let refused = s.git(words).from(&wt.root).run();
    assert_eq!(refused.end, End::Code(128), "{words:?}: {refused:?}");
    assert!(refused.stdout.is_empty(), "{words:?}: {refused:?}");
    names(refused.only_line("fatal"), b".gitdupe");
    names(refused.only_line("fatal"), cause.as_bytes());
    assert_eq!(
        wt.private(s).git(["ls-files", "-s"]).succeeds().stdout,
        index
    );
    assert_eq!(
        s.git(["ls-files", "-s"]).from(&wt.root).succeeds().stdout,
        public_index
    );
    match &listed {
        Some(bytes) => assert_eq!(fs::read(wt.root.join(".gitdupe")).unwrap(), *bytes),
        None => assert_eq!(
            fs::symlink_metadata(wt.root.join(".gitdupe"))
                .unwrap_err()
                .kind(),
            ErrorKind::NotFound
        ),
    }
    let changed = files.changed_in(&Tree::of(&wt.root));
    assert!(
        changed.is_empty(),
        "{words:?}: working files changed {changed:?}"
    );
    let changed = public.changed_in(&wt.public_git());
    assert!(
        changed.is_empty(),
        "{words:?}: public Git changed {changed:?}"
    );
    let settled = wt.region().unwrap();
    let mut rules = vec![b"/.gitdupe".to_vec()];
    if listed.is_some() {
        rules.push(b"/notes".to_vec());
    }
    assert_eq!(settled.rules, rules, "{words:?}");
    assert_eq!(settled.before, region.before);
    assert_eq!(settled.after, region.after);
}

#[test]
fn a_linked_worktree_on_another_filesystem_refuses_hidden_list_replacements() {
    under_each_release(|s| {
        let Some(second) = FreshDirectory::create_in(Path::new("/dev/shm")) else {
            report("no writable second filesystem at /dev/shm; replacement cases unavailable");
            return;
        };
        let second_path = second.path().to_path_buf();
        let main = s.dir().join("project");
        // Catch failures so removal is checked and Git pruned before the main fixture drops.
        let checked = catch_unwind(AssertUnwindSafe(|| {
            let first_device = fs::metadata(s.dir()).unwrap().dev();
            let second_device = fs::metadata(&second_path).unwrap().dev();
            report(&format!(
                "worktree filesystems: scenario device {first_device}, second device {second_device}"
            ));
            if first_device == second_device {
                report("/dev/shm has the scenario directory's device; no second filesystem");
                return;
            }
            s.attached_project(&main);
            let root = second_path.join("linked");
            s.linked_worktree(&main, &root);
            let wt = Worktree::read(s, &root);
            assert_eq!(fs::metadata(&wt.git_directory).unwrap().dev(), first_device);
            assert_eq!(fs::metadata(&wt.root).unwrap().dev(), second_device);
            let init = s.git(["dupe", "init"]).from(&root).run();
            if init.end != End::Code(0) {
                report(&format!(
                    "cross-filesystem linked worktree init refused: {init:?}"
                ));
            }
            assert_eq!(init.end, End::Code(0), "{init:?}");

            // Observe this filesystem's rename cause directly, rather than fixing its wording.
            let source = s.dir().join("rename-probe");
            fs::write(&source, b"probe\n").unwrap();
            let cause = fs::rename(&source, second_path.join("rename-probe")).unwrap_err();
            assert_eq!(cause.kind(), ErrorKind::CrossesDevices);
            fs::remove_file(&source).unwrap();
            let cause = cause.to_string();

            write(&root, "unrelated", b"leave this file\n");
            write(&root, "notes/kept", b"already hidden\n");
            write(&root, "newpath", b"not hidden\n");
            write(&root, "new-directory/file", b"not staged\n");
            assert!(!root.join(".gitdupe").exists());
            assert!(
                wt.private(s)
                    .git(["ls-files", "-s"])
                    .succeeds()
                    .stdout
                    .is_empty()
            );
            for present in [false, true] {
                if present {
                    write(&root, ".gitdupe", b"notes\n");
                    wt.private(s).git(["add", "-f", ".gitdupe"]).succeeds();
                }
                replacement_refused(s, &wt, &["dupe", "hide", "newpath"], &cause);
                replacement_refused(s, &wt, &["dupe", "add", "new-directory"], &cause);
                if present {
                    replacement_refused(s, &wt, &["dupe", "unhide", "notes"], &cause);
                }
            }
        }));
        drop(second);
        let removed = fs::symlink_metadata(&second_path);
        if main.exists() {
            s.git(["worktree", "prune", "--expire=now"])
                .from(&main)
                .succeeds();
            let remaining = s
                .git(["worktree", "list", "--porcelain"])
                .from(&main)
                .succeeds();
            assert_eq!(
                remaining
                    .stdout
                    .split(|&b| b == b'\n')
                    .filter(|line| line.starts_with(b"worktree "))
                    .count(),
                1
            );
        }
        assert_eq!(removed.unwrap_err().kind(), ErrorKind::NotFound);
        if let Err(failure) = checked {
            resume_unwind(failure);
        }
    });
}
