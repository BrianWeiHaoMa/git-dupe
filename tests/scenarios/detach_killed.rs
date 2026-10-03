//! `git dupe detach` and `git dupe detach --force`, each killed at every point of its run,
//! and every state a kill inside the removal of `.git/dupe` can leave: each completed by
//! `git dupe detach --force`, which, run once more, is refused naming `git dupe init` and
//! changes nothing (G21, G4, `State` "`detach`" and "Acknowledgement"). A kill never
//! leaves `.git/info/exclude` partially written, never changes `.gitdupe`, and changes
//! nothing outside `.git/dupe` and that file.
//!
//! No kill reaches inside the removal. The kill facility stops git-dupe at its Git runs
//! and inside a write of its own, and the removal comes after the last Git run and writes
//! nothing: it deletes. Nothing may be added to the product for a check to stop it there
//! (N6). The states such a kill can leave are built instead, from the state a kill after
//! the last Git run leaves, and each is completed as a killed one would be.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, GITDUPE, Killed, Output, Point, Scenario, Sweep, Tree, Version, copied, daily_warnings,
    detached, files, holds, lived_in, names, now_visible, outside, private_add, private_commit,
    region, region_rules, unchanged, under_each_release, warnings, write,
};

/// The pushed workspace `fixture`, where `git dupe detach` succeeds without `--force`, its
/// exclude file holding the developer's own lines around the region, so that both kills
/// inside a write stop the region's deletion, and permissions other than a new file's.
fn pushed(s: &Scenario) -> PathBuf {
    let root = s.pushed_workspace("fixture").root;
    lived_in(&root);
    root
}

/// `pushed` with work that exists nowhere else: `docs/notes.md` committed and not pushed,
/// and `notes/a.md` changed and not committed. Only `--force` detaches it.
fn unpushed(s: &Scenario) -> PathBuf {
    let root = s.pushed_workspace("fixture").root;
    write(&root, "docs/notes.md", b"unpushed\n");
    private_add(s, &root, "docs/notes.md");
    private_commit(s, &root);
    write(&root, "notes/a.md", b"not committed\n");
    lived_in(&root);
    let refused = s.git(["dupe", "detach"]).from(&root).run();
    assert_eq!(refused.end, End::Code(128), "{refused:?}");
    root
}

/// The uninterrupted run, which every kill is held to, left the developer's text as it
/// stood around the region and nothing else (`Composition/Keeper` **Remove**).
fn leaves_the_users_text(sweep: &Sweep) {
    let around = region(&sweep.fixture).expect("a region in the fixture");
    let exclude = sweep.after.exclude.as_ref().expect("an exclude file");
    assert_eq!(
        exclude.bytes,
        [around.before, around.after].concat(),
        "{:?}",
        sweep.uninterrupted
    );
}

/// What a kill of `detach` leaves beyond the sweep's own checks: `.gitdupe` as it was, an
/// inside-write kill stopped before the rename, and the private repository whole, every
/// entry as in the fixture, the one fresh file of an inside-write kill aside.
fn attached_as_it_was(killed: &Killed, whole: &Tree) {
    assert_eq!(killed.gitdupe, Version::Unchanged, "{killed:?}");
    let inside_write = matches!(killed.point, Point::InsideWrite { .. });
    if inside_write {
        assert_eq!(killed.exclude, Version::Old, "{killed:?}");
    }
    let changed = whole.changed_in(&Tree::relative(&killed.dir.join(".git/dupe")));
    assert!(
        changed.len() == usize::from(inside_write) && changed.iter().all(|at| !whole.holds(at)),
        "{killed:?} changed {changed:?} in the private repository"
    );
}

/// `git dupe detach --force` completes the kill: exit 0 with nothing on standard error but
/// the warnings the uninterrupted run wrote, `.git/dupe` and the region gone, both files as
/// the uninterrupted run leaves them, the exclude file's permissions included, and nothing
/// else changed. Run once more, it is refused naming `git dupe init` and changes nothing:
/// all that idempotent means once the workspace is unattached.
fn completed(sweep: &Sweep, killed: &Killed) {
    let s = sweep.s;
    let unchanged_outside = outside(&killed.dir);
    let output = s.git(["dupe", "detach", "--force"]).from(&killed.dir).run();
    assert_eq!(output.end, End::Code(0), "{killed:?}: {output:?}");
    assert!(output.stdout.is_empty(), "{killed:?}: {output:?}");
    warnings(&output, daily_warnings());
    detached(&killed.dir);
    assert_eq!(files(&killed.dir), sweep.after, "{killed:?}");
    let changed = unchanged_outside.changed_in(&outside(&killed.dir));
    assert!(changed.is_empty(), "{killed:?} changed {changed:?}");

    let before = Tree::of(&killed.dir);
    let again = s.git(["dupe", "detach", "--force"]).from(&killed.dir).run();
    assert_eq!(again.end, End::Code(128), "{killed:?}: {again:?}");
    assert!(again.stdout.is_empty(), "{killed:?}: {again:?}");
    names(again.only_line("fatal"), b"git dupe init");
    unchanged(&before, &killed.dir);
}

/// In a copy of the killed workspace, still attached, `git dupe status` in place of
/// `detach --force` settles the region back with every rule it held (G6), and
/// `git dupe detach` then leaves as the uninterrupted run did.
fn settled_back(sweep: &Sweep, killed: &Killed) {
    let s = sweep.s;
    let instead = copied(s, &killed.dir, "instead");
    let status = s.git(["dupe", "status"]).from(&instead).run();
    assert_eq!(status.end, End::Code(0), "{killed:?}: {status:?}");
    assert_eq!(
        region_rules(&instead),
        region_rules(&sweep.fixture),
        "{killed:?}"
    );
    let detach = s.git(["dupe", "detach"]).from(&instead).run();
    assert_eq!(detach.end, End::Code(0), "{killed:?}: {detach:?}");
    warnings(&detach, daily_warnings());
    detached(&instead);
    assert_eq!(files(&instead), sweep.after, "{killed:?}");
}

#[test]
fn a_killed_detach_is_completed_by_detach_force_or_settled_back_by_status() {
    under_each_release(|s| {
        let fixture = pushed(s);
        let sweep = Sweep::new(s, &fixture, &["detach"]);
        leaves_the_users_text(&sweep);
        warnings(&sweep.uninterrupted, daily_warnings());
        // Before and after each of its eight Git runs, and inside the region's deletion.
        assert_eq!(sweep.points().len(), 2 * 8 + 2, "{:?}", sweep.points());
        let whole = Tree::relative(&fixture.join(".git/dupe"));
        let mut reached = Vec::new();
        for &point in sweep.points() {
            let killed = sweep.kill(point);
            attached_as_it_was(&killed, &whole);
            settled_back(&sweep, &killed);
            completed(&sweep, &killed);
            reached.push(killed.exclude);
        }
        // Killed with the region whole, and after its deletion with `.git/dupe` whole.
        assert!(reached.contains(&Version::Old) && reached.contains(&Version::New));
    });
}

#[test]
fn a_killed_detach_force_is_completed_by_detach_force() {
    under_each_release(|s| {
        let fixture = unpushed(s);
        let sweep = Sweep::new(s, &fixture, &["detach", "--force"]);
        leaves_the_users_text(&sweep);
        warnings(&sweep.uninterrupted, daily_warnings());
        // Before and after each of its four Git runs, and inside the region's deletion.
        assert_eq!(sweep.points().len(), 2 * 4 + 2, "{:?}", sweep.points());
        let whole = Tree::relative(&fixture.join(".git/dupe"));
        let mut reached = Vec::new();
        for &point in sweep.points() {
            let killed = sweep.kill(point);
            attached_as_it_was(&killed, &whole);
            completed(&sweep, &killed);
            reached.push(killed.exclude);
        }
        assert!(reached.contains(&Version::Old) && reached.contains(&Version::New));
    });
}

/// git-dupe's own lines of `output`: what follows Git's own message, the standard error of
/// `git`, which must come first.
fn after_git(output: &Output, git: &Output) -> Output {
    let stderr = output
        .stderr
        .strip_prefix(git.stderr.as_slice())
        .unwrap_or_else(|| panic!("Git's message first, {git:?}: {output:?}"));
    Output {
        stdout: output.stdout.clone(),
        stderr: stderr.to_vec(),
        end: output.end,
    }
}

/// Deletes each of `names` from the private Git directory of `dir`, as a removal that was
/// stopped may have.
fn delete(dir: &Path, names: &[OsString]) {
    let private = dir.join(".git/dupe");
    for name in names {
        let path = private.join(name);
        let deleted = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        deleted.unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
    }
}

#[test]
fn every_state_a_kill_inside_the_removal_can_leave_is_completed_by_detach_force() {
    under_each_release(|s| {
        let fixture = pushed(s);
        let sweep = Sweep::new(s, &fixture, &["detach"]);
        leaves_the_users_text(&sweep);
        // Once the last Git run has ended the region is deleted, `.git/dupe` is whole, and
        // only the warnings stand between the kill and the removal.
        let last = sweep
            .points()
            .iter()
            .rev()
            .find(|point| matches!(point, Point::AfterRun(_)))
            .copied()
            .unwrap();
        let removing = sweep.kill(last);
        assert_eq!(removing.exclude, Version::New, "{removing:?}");
        let every: Vec<OsString> = fs::read_dir(removing.dir.join(".git/dupe"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        let all_but = |kept: &str| -> Vec<OsString> {
            every.iter().filter(|name| *name != kept).cloned().collect()
        };
        let named =
            |names: &[&str]| -> Vec<OsString> { names.iter().map(OsString::from).collect() };
        // The removal deletes the entries of each directory in the order it reads them, so
        // that what is left can be any part, a repository Git still opens among them.
        for (left, deleted, opens) in [
            ("emptied", every.clone(), false),
            ("without HEAD", named(&["HEAD"]), false),
            ("without objects", named(&["objects"]), false),
            ("without refs", named(&["refs"]), false),
            (
                "without objects and refs",
                named(&["objects", "refs"]),
                false,
            ),
            ("holding one file only", all_but("config"), false),
            ("without index", named(&["index"]), true),
        ] {
            // With `.gitdupe` removed as well, its staged version cannot be read either,
            // which means no listed path, never a refusal.
            for gitdupe_kept in [true, false] {
                let dir = copied(s, &removing.dir, "built");
                delete(&dir, &deleted);
                if !gitdupe_kept {
                    fs::remove_file(dir.join(GITDUPE)).unwrap();
                }
                let private = dir.join(".git/dupe");
                let what = format!("{left}, .gitdupe kept: {gitdupe_kept}");

                if !opens && gitdupe_kept {
                    // Without `--force`: a private repository that cannot be read.
                    let status = s
                        .private(&dir)
                        .git([
                            "--no-optional-locks",
                            "status",
                            "--porcelain",
                            "-z",
                            "--untracked-files=no",
                        ])
                        .run();
                    assert_ne!(status.end, End::Code(0), "{what}: {status:?}");
                    let all = Tree::of(&dir);
                    let refused = s.git(["dupe", "detach"]).from(&dir).run();
                    assert_eq!(refused.end, End::Code(128), "{what}: {refused:?}");
                    assert!(refused.stdout.is_empty(), "{what}: {refused:?}");
                    let line = after_git(&refused, &status);
                    names(line.only_line("fatal"), b"cannot be read");
                    names(line.only_line("fatal"), b"git dupe detach --force");
                    unchanged(&all, &dir);
                }

                // Git's own answer to the listing `detach --force` asks for.
                let listing = s.private(&dir).git(["ls-files", "-z", "--full-name"]).run();
                assert_eq!(listing.end == End::Code(0), opens, "{what}: {listing:?}");
                let before = Tree::of(&dir).without(&[&private]);
                let output = s.git(["dupe", "detach", "--force"]).from(&dir).run();
                assert_eq!(output.end, End::Code(0), "{what}: {output:?}");
                assert!(output.stdout.is_empty(), "{what}: {output:?}");
                let ours = after_git(&output, &listing);
                let mut expected: Vec<Vec<u8>> = if gitdupe_kept {
                    [".gitdupe", "notes"].into_iter().map(now_visible).collect()
                } else {
                    Vec::new()
                };
                if !opens {
                    let unlisted: Vec<_> = ours
                        .lines("warning")
                        .into_iter()
                        .filter(|line| holds(line, b"cannot list the private repository"))
                        .collect();
                    assert_eq!(unlisted.len(), 1, "{what}: {output:?}");
                    expected.push(unlisted[0].to_vec());
                }
                warnings(&ours, expected);
                assert!(!private.exists(), "{what}");
                let changed = before.changed_in(&Tree::of(&dir));
                assert!(changed.is_empty(), "{what}: changed {changed:?}");
            }
        }
    });
}
