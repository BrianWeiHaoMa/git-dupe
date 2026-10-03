//! `git dupe add notes/`, `git dupe unhide notes/`, and `git dupe init`, each killed at
//! every point of its run and then run again: `.gitdupe` and `.git/info/exclude` each
//! either as they were or as the uninterrupted run leaves them, never partially written,
//! and after the second run what the uninterrupted run leaves (G21, `Done when`). The
//! sweep itself is the harness's.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, GITDUPE, Killed, Output, Point, Scenario, Sweep, Version, copied, files, holds, lived_in,
    private_add, private_commit, staged_gitdupe, under_each_release, write,
};

const NAME: &str = "Public Name";
const EMAIL: &str = "public@example.invalid";

/// Runs the command in `dir`.
fn run(sweep: &Sweep, dir: &Path) -> Output {
    sweep
        .s
        .git(["dupe"].iter().chain(sweep.words))
        .from(dir)
        .run()
}

/// Runs the command again in `dir`: exit 0 without a `fatal:` line, and both files what the
/// uninterrupted run leaves.
fn rerun(sweep: &Sweep, dir: &Path) -> Output {
    let output = run(sweep, dir);
    assert_eq!(output.end, End::Code(0), "{output:?}");
    assert!(output.lines("fatal").is_empty(), "{output:?}");
    assert_eq!(files(dir), sweep.after, "{output:?}");
    output
}

/// Sweeps `git dupe <words>` over the fixture and completes each kill by running the same
/// words again, then a third time. `after_kill` adds the command's own checks of the killed
/// workspace; `after_rerun` its own checks of the workspace the second run left, given what
/// `after_kill` returned. Returns each kill with that value.
fn every_kill_point<T>(
    s: &Scenario,
    fixture: &Path,
    words: &[&str],
    after_kill: impl Fn(&Sweep, &Killed) -> T,
    after_rerun: impl Fn(&Sweep, &Killed, &T, &Output),
) -> Vec<(Killed, T)> {
    let sweep = Sweep::new(s, fixture, words);
    let mut kills = Vec::new();
    for &point in sweep.points() {
        let killed = sweep.kill(point);
        let then = after_kill(&sweep, &killed);
        let second = rerun(&sweep, &killed.dir);
        after_rerun(&sweep, &killed, &then, &second);
        rerun(&sweep, &killed.dir);
        kills.push((killed, then));
    }
    kills
}

/// An attached project whose `.gitdupe` lists `scratch` beside what `listed` names, every
/// file of `tracked` privately tracked and committed, `.gitdupe` with them, and the region
/// settled by `git dupe status`.
fn attached_fixture(s: &Scenario, listed: &[u8], tracked: &[&str]) -> PathBuf {
    let dir = s.dir().join("fixture");
    s.attached_project(&dir);
    write(&dir, GITDUPE, &[listed, b"scratch\n"].concat());
    write(&dir, "scratch/plan.md", b"plan\n");
    for path in [GITDUPE, "scratch/plan.md"].iter().chain(tracked) {
        if !dir.join(path).exists() {
            write(&dir, path, b"private\n");
        }
        private_add(s, &dir, path);
    }
    private_commit(s, &dir);
    s.git(["dupe", "status"]).from(&dir).succeeds();
    lived_in(&dir);
    dir
}

#[test]
fn a_killed_add_leaves_each_file_old_or_new_and_its_rerun_completes() {
    under_each_release(|s| {
        let fixture = attached_fixture(s, b"", &[]);
        write(&fixture, "notes/one.md", b"one\n");
        write(&fixture, "notes/two.md", b"two\n");
        let old_staged = staged_gitdupe(s, &fixture);
        let kills = every_kill_point(
            s,
            &fixture,
            &["add", "notes/"],
            |sweep, killed| {
                match killed.point {
                    Point::InsideWrite { blocks: 0 } => {
                        assert_eq!(
                            (killed.gitdupe, killed.exclude),
                            (Version::Old, Version::Old)
                        );
                    }
                    Point::InsideWrite { .. } => {
                        assert_eq!(
                            (killed.gitdupe, killed.exclude),
                            (Version::New, Version::Old)
                        );
                    }
                    _ => {}
                }
                if (killed.gitdupe, killed.exclude) == (Version::New, Version::Old) {
                    // Not the same command: the next command G6 covers brings the region up
                    // to date.
                    let instead = copied(s, &killed.dir, "instead");
                    let status = s.git(["dupe", "status"]).from(&instead).run();
                    assert_eq!(status.end, End::Code(0), "{status:?}");
                    assert_eq!(files(&instead), sweep.after, "{killed:?}");
                }
                staged_gitdupe(s, &killed.dir)
            },
            |sweep, killed, staged, _| {
                // The files under `notes` are staged after every rerun.
                let listed = s.private(&killed.dir).git(["ls-files", "-z"]).succeeds();
                let paths: Vec<&[u8]> = listed.stdout.split(|&byte| byte == 0).collect();
                for path in [&b"notes/one.md"[..], b"notes/two.md"] {
                    assert!(paths.contains(&path), "{killed:?}: {listed:?}");
                }
                let new = sweep.after.gitdupe.as_ref().map(|new| new.bytes.clone());
                let rerun_staged = staged_gitdupe(s, &killed.dir);
                // A rerun finds `notes` hidden and stages `.gitdupe` only when asked to.
                assert!(
                    rerun_staged == old_staged || rerun_staged == new,
                    "{killed:?}"
                );
                if rerun_staged != new {
                    assert_eq!(*staged, old_staged, "{killed:?}");
                    let add = s.git(["dupe", "add", GITDUPE]).from(&killed.dir).run();
                    assert_eq!(add.end, End::Code(0), "{add:?}");
                }
                let index = s.private(&killed.dir).git(["ls-files", "-s", "-z"]).run();
                assert_eq!(
                    index.stdout,
                    sweep.reference_index(),
                    "{killed:?}: {index:?}"
                );
            },
        );
        let reached = |gitdupe, exclude, staged_new: bool| {
            kills.iter().any(|(killed, staged)| {
                (killed.gitdupe, killed.exclude) == (gitdupe, exclude)
                    && (*staged != old_staged) == staged_new
            })
        };
        // Before the first rename, between the rename of `.gitdupe` and its staging, between
        // the staging and the rename of the exclude file, and after it.
        assert!(reached(Version::Old, Version::Old, false));
        assert!(reached(Version::New, Version::Old, false));
        assert!(reached(Version::New, Version::Old, true));
        assert!(reached(Version::New, Version::New, true));
    });
}

/// Whether one of the warnings names `notes` as visible to public Git.
fn releases_notes(warnings: &[&[u8]]) -> bool {
    warnings
        .iter()
        .any(|line| holds(line, b"notes") && holds(line, b"visible to public Git"))
}

#[test]
fn a_killed_unhide_leaves_each_file_old_or_new_and_its_rerun_completes() {
    under_each_release(|s| {
        let fixture = attached_fixture(s, b"notes\n", &["notes/one.md", "notes/two.md"]);
        // Something public Git sees once `notes` is no longer hidden.
        write(&fixture, "notes/draft.md", b"draft\n");
        let kills = every_kill_point(
            s,
            &fixture,
            &["unhide", "notes/"],
            |_, killed| match killed.point {
                Point::InsideWrite { blocks: 0 } => {
                    assert_eq!(
                        (killed.gitdupe, killed.exclude),
                        (Version::Old, Version::Old)
                    );
                }
                Point::InsideWrite { .. } => {
                    assert_eq!(
                        (killed.gitdupe, killed.exclude),
                        (Version::New, Version::Old)
                    );
                }
                _ => {}
            },
            |sweep, killed, (), rerun| {
                // Staged whenever it exists on disk: a retry stages what the killed run wrote.
                let new = sweep.after.gitdupe.as_ref().map(|new| new.bytes.clone());
                assert_eq!(staged_gitdupe(s, &killed.dir), new, "{killed:?}");
                let index = s.private(&killed.dir).git(["ls-files", "-s", "-z"]).run();
                assert_eq!(
                    index.stdout,
                    sweep.reference_index(),
                    "{killed:?}: {index:?}"
                );
                // A rerun that starts from the old region still releases `notes`; after the
                // region's rename only the warnings can be missing.
                if killed.exclude == Version::Old {
                    let named = sweep.uninterrupted.lines("warning");
                    assert!(!named.is_empty(), "{:?}", sweep.uninterrupted);
                    assert!(releases_notes(&named), "{:?}", sweep.uninterrupted);
                    assert_eq!(rerun.lines("warning"), named, "{killed:?}: {rerun:?}");
                }
            },
        );
        for (gitdupe, exclude) in [
            (Version::Old, Version::Old),
            (Version::New, Version::Old),
            (Version::New, Version::New),
        ] {
            assert!(
                kills
                    .iter()
                    .any(|(k, ())| (k.gitdupe, k.exclude) == (gitdupe, exclude))
            );
        }
    });
}

/// `init` from the project at `dir`, which must be unattached, with user text in its
/// exclude file and an identity in its local configuration.
fn unattached_fixture(s: &Scenario) -> PathBuf {
    let dir = s.dir().join("fixture");
    s.unattached_project(&dir);
    for (key, value) in [("user.name", NAME), ("user.email", EMAIL)] {
        s.git(["config", "--local", key, value])
            .from(&dir)
            .succeeds();
    }
    lived_in(&dir);
    dir
}

/// The private repository of `dir` is configured as F2 and G1 say, on the project's
/// branch.
fn configured(s: &Scenario, dir: &Path) {
    let listed = s
        .private(dir)
        .git(["config", "-z", "--local", "--list"])
        .succeeds();
    let records: Vec<&[u8]> = listed.stdout.split(|&byte| byte == 0).collect();
    for (key, value) in [
        ("core.worktree", "../.."),
        ("status.showuntrackedfiles", "no"),
        ("advice.statushints", "false"),
        ("user.name", NAME),
        ("user.email", EMAIL),
    ] {
        let record = format!("{key}\n{value}");
        let found: Vec<_> = records
            .iter()
            .filter(|record| record.starts_with(format!("{key}\n").as_bytes()))
            .collect();
        assert_eq!(found, [&record.as_bytes()], "{key}: {listed:?}");
    }
    let head = s.private(dir).git(["symbolic-ref", "HEAD"]).succeeds();
    assert_eq!(head.stdout, b"refs/heads/main\n");
}

/// The value of `core.worktree` in the private repository of `dir`, if any.
fn worktree(s: &Scenario, dir: &Path) -> Option<Vec<u8>> {
    let value = s
        .private(dir)
        .git(["config", "--local", "--get", "core.worktree"])
        .run();
    (value.end == End::Code(0)).then_some(value.stdout)
}

/// With a lock file Git left behind in the private repository of a copy of `killed`, the
/// rerun ends as Git's own `config` ends there and does not finish; with the lock removed,
/// the next rerun completes.
fn locked_then_completed(sweep: &Sweep, killed: &Killed) {
    let s = sweep.s;
    let dir = copied(s, &killed.dir, "locked");
    let lock = dir.join(".git/dupe/config.lock");
    fs::write(&lock, b"locked\n").unwrap();
    let output = run(sweep, &dir);
    let gits = s
        .private(&dir)
        .git([
            "config",
            "--local",
            "--replace-all",
            "status.showUntrackedFiles",
            "no",
        ])
        .run();
    assert_ne!(gits.end, End::Code(0), "{gits:?}");
    assert_eq!(output.end, gits.end, "{killed:?}: {output:?}");
    assert_eq!(output.stderr, gits.stderr, "{killed:?}: {output:?}");
    assert_ne!(
        worktree(s, &dir).as_deref(),
        Some(&b"../..\n"[..]),
        "{killed:?}"
    );
    fs::remove_file(&lock).unwrap();
    let output = rerun(sweep, &dir);
    assert_eq!(output.lines("hint").len(), 1, "{killed:?}: {output:?}");
    configured(s, &dir);
}

#[test]
fn a_killed_init_is_completed_by_running_it_again() {
    under_each_release(|s| {
        let fixture = unattached_fixture(s);
        let locked = Cell::new(false);
        let kills = every_kill_point(
            s,
            &fixture,
            &["init"],
            |sweep, killed| {
                if let Point::InsideWrite { .. } = killed.point {
                    assert_eq!(
                        (killed.gitdupe, killed.exclude),
                        (Version::Unchanged, Version::Old)
                    );
                }
                let attached = killed.dir.join(".git/dupe").is_dir();
                let unfinished =
                    attached && worktree(s, &killed.dir).as_deref() != Some(&b"../..\n"[..]);
                // At the first such point only: a run after it adds nothing a rerun does not
                // show, and Git's own `config` waits a while on a lock in some releases.
                if unfinished && !locked.replace(true) {
                    locked_then_completed(sweep, killed);
                }
                (attached, unfinished)
            },
            |_, killed, (attached, _), rerun| {
                // A first `init` says nothing of its own; completing one says so.
                assert_eq!(
                    rerun.lines("hint").len(),
                    usize::from(*attached),
                    "{killed:?}: {rerun:?}"
                );
                configured(s, &killed.dir);
            },
        );
        for state in [(false, false), (true, true), (true, false)] {
            assert!(
                kills.iter().any(|(_, reached)| *reached == state),
                "{state:?}"
            );
        }
    });
}
