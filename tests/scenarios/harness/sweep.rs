//! The kill sweep: a `git dupe` command killed at every point of its run, each time in a
//! fresh copy of its fixture, and what every kill leaves checked (G21). What completes a
//! killed run is the command's own: each scenario file that sweeps a command says it.
//!
//! A command writes nothing outside the private Git directory and the two files but, for
//! `clone`, the files it adds to the working tree (G5): `Sweep::writing` sweeps such a
//! command, holding every other path, at every kill, as it was or as the uninterrupted run
//! left it, and every path that existed before as it was.
//!
//! The points are found, not written down: an uninterrupted run gives the reference and
//! the number of git-dupe's Git runs, and the command is then killed in a fresh copy of the
//! fixture before and after each run and inside a write of its own. A hard link made to
//! each old file beforehand still holds the old bytes afterwards, so the old version was
//! never written into: with the inside-write kill, that tells a rename from a write in
//! place, a truncate-and-write, and a copy over the destination.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use super::files::copy;
use super::kill::{Killing, Point};
use super::output::{End, Output};
use super::scenario::Scenario;
use super::tree::Tree;

/// What a workspace holds at one of the two paths git-dupe writes of its own.
#[derive(Clone, PartialEq, Eq)]
pub struct Written {
    pub bytes: Vec<u8>,
    pub mode: u32,
}

impl std::fmt::Debug for Written {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:o} \"{}\"", self.mode, self.bytes.escape_ascii())
    }
}

/// `.gitdupe` and `.git/info/exclude` of a workspace, `None` where nothing stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Files {
    pub gitdupe: Option<Written>,
    pub exclude: Option<Written>,
}

pub const GITDUPE: &str = ".gitdupe";
const EXCLUDE: &str = ".git/info/exclude";

pub fn files(dir: &Path) -> Files {
    let read = |path: &str| {
        let path = dir.join(path);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => Some(Written {
                bytes: fs::read(&path).unwrap_or_else(|cause| panic!("{path:?}: {cause}")),
                mode: metadata.permissions().mode() & 0o7777,
            }),
            Err(cause) if cause.kind() == ErrorKind::NotFound => None,
            Err(cause) => panic!("{}: {cause}", path.display()),
        }
    };
    Files {
        gitdupe: read(GITDUPE),
        exclude: read(EXCLUDE),
    }
}

/// Which version a file is after a kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    /// As before the command, and the uninterrupted run changes it.
    Old,
    /// As the uninterrupted run leaves it, a change.
    New,
    /// The uninterrupted run leaves it as it was, and so did this one.
    Unchanged,
}

fn version(
    path: &str,
    now: &Option<Written>,
    before: &Option<Written>,
    after: &Option<Written>,
) -> Version {
    match (now == before, now == after) {
        (true, true) => Version::Unchanged,
        (true, false) => Version::Old,
        (false, true) => Version::New,
        (false, false) => panic!(
            "{path} is neither as it was, {before:?}, nor as the uninterrupted run leaves it, \
             {after:?}: {now:?}"
        ),
    }
}

/// The command, its fixture, what its uninterrupted run left, and the points of its run.
pub struct Sweep<'s> {
    pub s: &'s Scenario,
    pub words: &'s [&'s str],
    pub fixture: PathBuf,
    pub before: Files,
    pub reference: PathBuf,
    pub after: Files,
    pub uninterrupted: Output,
    /// For a command that writes files in the working tree, what the uninterrupted run left
    /// outside the private Git directory and the two files.
    written: Option<Tree>,
    killing: Killing<'s>,
    points: Vec<Point>,
}

/// A run killed at `point`, in the workspace `dir`, as the kill left it.
#[derive(Clone, Debug)]
pub struct Killed {
    pub point: Point,
    pub dir: PathBuf,
    pub gitdupe: Version,
    pub exclude: Version,
}

impl<'s> Sweep<'s> {
    /// Runs `git dupe <words>` uninterrupted in a copy of `fixture`, where it must exit 0,
    /// change nothing outside the private Git directory and the two files, and keep their
    /// permissions; then through the killing script in another copy, to find the points of
    /// its run, where it must leave the same. A kill the run never reaches fails the check.
    pub fn new(s: &'s Scenario, fixture: &Path, words: &'s [&'s str]) -> Sweep<'s> {
        Sweep::sweeping(s, fixture, words, false)
    }

    /// `new` for a command that also writes files in the working tree: outside the private
    /// Git directory and the two files its uninterrupted run adds paths, at least one, and
    /// changes or removes none that existed.
    pub fn writing(s: &'s Scenario, fixture: &Path, words: &'s [&'s str]) -> Sweep<'s> {
        Sweep::sweeping(s, fixture, words, true)
    }

    fn sweeping(s: &'s Scenario, fixture: &Path, words: &'s [&'s str], writes: bool) -> Sweep<'s> {
        let links = s.dir().join("links");
        let before = files(fixture);

        let reference = copied(s, fixture, "reference");
        link(&reference, &links);
        let unchanged_outside = outside(&reference);
        let uninterrupted = s.git(["dupe"].iter().chain(words)).from(&reference).run();
        assert_eq!(uninterrupted.end, End::Code(0), "{uninterrupted:?}");
        links_hold(&links, &before);
        let left_outside = outside(&reference);
        let changed = unchanged_outside.changed_in(&left_outside);
        let written = if writes {
            let existed: Vec<_> = changed
                .iter()
                .filter(|path| unchanged_outside.holds(path))
                .collect();
            assert!(
                !changed.is_empty() && existed.is_empty(),
                "added {changed:?}, of which existed {existed:?}; {uninterrupted:?}"
            );
            Some(left_outside)
        } else {
            assert!(changed.is_empty(), "changed {changed:?}; {uninterrupted:?}");
            None
        };
        let after = files(&reference);
        for (now, was) in [
            (&after.gitdupe, &before.gitdupe),
            (&after.exclude, &before.exclude),
        ] {
            if let (Some(now), Some(was)) = (now, was) {
                assert_eq!(now.mode, was.mode, "permissions not kept");
            }
        }

        // The same run through the killing script, to count its points: it must leave what the
        // run without it leaves, a private repository with the same index, or none.
        let killing = s.killing("control", words);
        let counted = copied(s, fixture, "counted");
        let (output, points) = killing.uninterrupted(&counted);
        assert_eq!(output.end, End::Code(0), "{output:?}");
        let sweep = Sweep {
            s,
            words,
            fixture: fixture.to_path_buf(),
            before,
            reference,
            after,
            uninterrupted,
            written,
            killing,
            points,
        };
        assert_eq!(files(&counted), sweep.after);
        let counted_outside = outside(&counted);
        let changed = match &sweep.written {
            Some(written) => written.changed_in(&counted_outside),
            None => unchanged_outside.changed_in(&counted_outside),
        };
        assert!(changed.is_empty(), "changed {changed:?}; {output:?}");
        let attached = |dir: &Path| dir.join(".git/dupe").exists();
        assert_eq!(attached(&counted), attached(&sweep.reference), "{output:?}");
        if attached(&sweep.reference) {
            let index = s.private(&counted).git(["ls-files", "-s", "-z"]).run();
            assert_eq!(index.stdout, sweep.reference_index(), "{index:?}");
        }

        // A kill the run never reaches is a failure of the check, never a point.
        let runs = sweep
            .points
            .iter()
            .filter(|point| matches!(point, Point::BeforeRun(_)))
            .count();
        for unreached in [
            Point::BeforeRun(runs + 1),
            Point::InsideWrite { blocks: 1 << 20 },
        ] {
            let work = copied(s, &sweep.fixture, "work");
            let counted = catch_unwind(AssertUnwindSafe(|| sweep.killing.killed(&work, unreached)));
            assert!(counted.is_err(), "{unreached:?} was counted as a kill");
        }
        sweep
    }

    /// Every point at which the command can be killed, in the order its run reaches them:
    /// before and after each of its Git runs, then inside a write of its own.
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    /// Kills the command at `point` in a fresh copy of the fixture, and checks what every
    /// kill must leave: each of the two files as it was or as the uninterrupted run leaves
    /// it, the old one never written into, nothing else changed outside the private Git
    /// directory, or for `writing`, each path there as it was or as the uninterrupted run
    /// left it, and nothing of git-dupe's own left inside it but, after a kill inside a
    /// write, one fresh file.
    pub fn kill(&self, point: Point) -> Killed {
        let links = self.s.dir().join("links");
        let work = copied(self.s, &self.fixture, "work");
        link(&work, &links);
        let unchanged_outside = outside(&work);
        let output = self.killing.killed(&work, point);
        let now = files(&work);
        let killed = Killed {
            point,
            dir: work.clone(),
            gitdupe: version(
                GITDUPE,
                &now.gitdupe,
                &self.before.gitdupe,
                &self.after.gitdupe,
            ),
            exclude: version(
                EXCLUDE,
                &now.exclude,
                &self.before.exclude,
                &self.after.exclude,
            ),
        };
        links_hold(&links, &self.before);
        let left_outside = outside(&work);
        let changed = match &self.written {
            Some(written) => left_outside.as_neither(&unchanged_outside, written),
            None => unchanged_outside.changed_in(&left_outside),
        };
        assert!(
            changed.is_empty(),
            "{point:?} changed {changed:?}; {output:?}"
        );
        left_in_private_directory(self, &killed);
        killed
    }

    /// The private index of the uninterrupted run's workspace.
    pub fn reference_index(&self) -> Vec<u8> {
        self.s
            .private(&self.reference)
            .git(["ls-files", "-s", "-z"])
            .succeeds()
            .stdout
    }
}

/// A copy of the workspace `from` at `name` below the scenario's directory, in place of
/// whatever stood there.
pub fn copied(s: &Scenario, from: &Path, name: &str) -> PathBuf {
    let to = s.dir().join(name);
    match fs::remove_dir_all(&to) {
        Ok(()) => {}
        Err(cause) if cause.kind() == ErrorKind::NotFound => {}
        Err(cause) => panic!("{}: {cause}", to.display()),
    }
    copy(from, &to);
    to
}

/// Everything below `dir` but the private repository and the two files git-dupe writes,
/// each entry by its path below `dir`, so that the trees of two copies compare.
pub fn outside(dir: &Path) -> Tree {
    let left_out = [".git/dupe", GITDUPE, EXCLUDE].map(Path::new);
    Tree::relative(dir).without(&left_out)
}

/// Hard links, made in `links`, to each of the two files that stands in `dir`.
fn link(dir: &Path, links: &Path) {
    match fs::remove_dir_all(links) {
        Ok(()) => {}
        Err(cause) if cause.kind() == ErrorKind::NotFound => {}
        Err(cause) => panic!("{}: {cause}", links.display()),
    }
    fs::create_dir(links).unwrap();
    for (path, name) in [(GITDUPE, "gitdupe"), (EXCLUDE, "exclude")] {
        if dir.join(path).exists() {
            fs::hard_link(dir.join(path), links.join(name)).unwrap();
        }
    }
}

/// Each hard link of `link` still holds the bytes the file held before the command: the
/// old file was replaced, never written into.
fn links_hold(links: &Path, before: &Files) {
    for (written, name) in [(&before.gitdupe, "gitdupe"), (&before.exclude, "exclude")] {
        if let Some(written) = written {
            let held = fs::read(links.join(name)).unwrap();
            assert!(
                held == written.bytes,
                "the old {name} was written into: \"{}\"",
                held.escape_ascii()
            );
        }
    }
}

/// The names directly inside the private Git directory of `dir`, none when it is absent.
fn private_names(dir: &Path) -> Vec<std::ffi::OsString> {
    match fs::read_dir(dir.join(".git/dupe")) {
        Ok(listing) => listing.map(|entry| entry.unwrap().file_name()).collect(),
        Err(cause) if cause.kind() == ErrorKind::NotFound => Vec::new(),
        Err(cause) => panic!("{}: {cause}", dir.display()),
    }
}

/// A kill at a Git run leaves nothing of git-dupe's own in the private Git directory; a
/// kill inside a write leaves one fresh file there, holding the start of the new version
/// of one of the two files.
fn left_in_private_directory(sweep: &Sweep, killed: &Killed) {
    let known = [
        private_names(&sweep.fixture),
        private_names(&sweep.reference),
    ]
    .concat();
    let left: Vec<_> = private_names(&killed.dir)
        .into_iter()
        .filter(|name| !known.contains(name))
        .collect();
    let Point::InsideWrite { .. } = killed.point else {
        assert!(left.is_empty(), "{killed:?} left {left:?}");
        return;
    };
    assert_eq!(left.len(), 1, "{killed:?} left {left:?}");
    let fresh = fs::read(killed.dir.join(".git/dupe").join(&left[0])).unwrap();
    let new_versions = [&sweep.after.gitdupe, &sweep.after.exclude];
    assert!(
        new_versions
            .into_iter()
            .flatten()
            .any(|new| new.bytes.starts_with(&fresh)),
        "{killed:?} left \"{}\"",
        fresh.escape_ascii()
    );
}

/// Lines of the developer's own in the exclude file, far more than two blocks of any shell
/// in all, so that a limit of one block stops the write of the exclude file and not that of
/// `.gitdupe`.
fn own_lines(first: usize, count: usize) -> Vec<u8> {
    (first..first + count)
        .flat_map(|n| format!("# a line of the developer's own, number {n:03}\n").into_bytes())
        .collect()
}

/// Surrounds the managed region of the workspace at `dir` with the developer's own lines,
/// and gives `.gitdupe`, where it exists, and the exclude file permissions other than a new
/// file's.
pub fn lived_in(dir: &Path) {
    let exclude = dir.join(EXCLUDE);
    let region = fs::read(&exclude).unwrap();
    fs::write(
        &exclude,
        [own_lines(0, 80), region, own_lines(80, 4)].concat(),
    )
    .unwrap();
    fs::set_permissions(&exclude, fs::Permissions::from_mode(0o640)).unwrap();
    if dir.join(GITDUPE).exists() {
        fs::set_permissions(dir.join(GITDUPE), fs::Permissions::from_mode(0o600)).unwrap();
    }
}
