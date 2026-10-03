//! `git dupe clone` killed at every point of its run on a second machine that holds its own
//! differing `.env.local`, then started over by `git dupe detach --force` and
//! `git dupe clone` (G21 "a killed `clone` as G2 describes", G2, `State` "`clone`", E6).
//! At every kill each entry that existed before is as it was, every other path absent or as
//! the uninterrupted run left it, and `.git/info/exclude` as it was or as that run left it,
//! never partially written: the sweep's own checks (`Sweep::writing`). Then `detach --force`
//! exits 0 wherever `.git/dupe` is a directory and is refused naming `git dupe init` where
//! it is not, and the second `clone` leaves what the uninterrupted run left.
//!
//! The kill facility never kills inside a Git run, and nothing may be added to the product
//! for a check to stop one (N6): files that a killed `checkout-index` left partly written,
//! `.gitdupe` among them, are made by hand instead, cut short once the write step has run,
//! and the start over keeps each, names it, and shows it modified.
//!
//! Each point runs three commands, under every release, so the points are divided among the
//! scenarios below by the part of the run they fall in, which run side by side; together
//! they hold every point.

use std::fs;
use std::path::{Path, PathBuf};

use crate::harness::{
    End, Killed, Output, Point, Scenario, Sweep, Tree, Version, copied, detached, files, lived_in,
    names, outside, region, region_rules, run_traced, under_each_release, write,
};

/// The parts of `clone`'s run, in order, each named for the Git run that begins it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// The refusals' runs, before Git's `init`.
    Refusals,
    /// Git's `init` and the settings that follow it.
    Init,
    /// From `remote add` through `reset -q`.
    Steps,
    /// The write step: the listing of absent files, `checkout-index`, and `diff`.
    Write,
    /// Settle's runs, and its write of the region.
    Settle,
}

/// The second machine, holding its own `.env.local`, with the developer's own lines in its
/// exclude file around where the region will stand, and that file's permissions other than
/// a new file's; and the URL of the first machine's private repository.
fn fixture(s: &Scenario) -> (PathBuf, String) {
    let m = s.second_machine("project");
    write(&m.root, ".env.local", b"mine\n");
    lived_in(&m.root);
    let url = m.first.private_remote.to_str().unwrap().to_owned();
    (m.root, url)
}

/// The command word of each of git-dupe's own Git runs of an uninterrupted `clone`, in
/// order: the run a point's number counts.
fn commands(s: &Scenario, fixture: &Path, words: &[&str]) -> Vec<String> {
    let dir = copied(s, fixture, "traced");
    let (output, runs) = run_traced(
        s.git(["dupe"].iter().chain(words)).from(&dir),
        &s.dir().join("trace"),
    );
    assert_eq!(output.end, End::Code(0), "{output:?}");
    runs.own()
        .commands()
        .into_iter()
        .map(|word| String::from_utf8(word.to_vec()).unwrap())
        .collect()
}

/// The number, counted from 1, of the first run of `command`.
fn first(commands: &[String], command: &str) -> usize {
    1 + commands
        .iter()
        .position(|word| word == command)
        .unwrap_or_else(|| panic!("no {command} among {commands:?}"))
}

/// Where a kill falls in the run of an uninterrupted `clone` whose own Git runs are
/// `commands`.
struct Run<'c> {
    commands: &'c [String],
}

impl Run<'_> {
    /// The part of the run `point` falls in. A kill inside a write is inside settle's,
    /// the one write of git-dupe's own in `clone`.
    fn part(&self, point: Point) -> Part {
        let n = match point {
            Point::BeforeRun(n) | Point::AfterRun(n) => n,
            Point::InsideWrite { .. } => return Part::Settle,
        };
        let starts = [
            (Part::Settle, first(self.commands, "diff") + 1),
            (Part::Write, first(self.commands, "reset") + 1),
            (Part::Steps, first(self.commands, "remote")),
            (Part::Init, first(self.commands, "init")),
        ];
        starts
            .into_iter()
            .find(|(_, start)| n >= *start)
            .map_or(Part::Refusals, |(part, _)| part)
    }

    /// Whether the run of `command` had ended when the command was killed at `point`.
    fn ran(&self, point: Point, command: &str) -> bool {
        let n = first(self.commands, command);
        match point {
            Point::BeforeRun(at) => at > n,
            Point::AfterRun(at) => at >= n,
            // Settle writes the region between its last two runs.
            Point::InsideWrite { .. } => n < self.commands.len(),
        }
    }

    /// Whether settle had renamed the region into place: it does so after its public
    /// listing and before it asks `check-ignore`, the last run of all.
    fn renamed(&self, point: Point) -> bool {
        let last = self.commands.len();
        assert_eq!(
            self.commands[last - 1],
            "check-ignore",
            "{:?}",
            self.commands
        );
        matches!(point, Point::BeforeRun(n) | Point::AfterRun(n) if n == last)
    }
}

/// What the private repository of the workspace at `dir` holds that an uninterrupted
/// `clone` decides: its index, its local configuration, its `HEAD`, and every ref with
/// its upstream.
fn private_state(s: &Scenario, dir: &Path) -> Vec<Vec<u8>> {
    let words: [&[&str]; 4] = [
        &["ls-files", "-s", "-z"],
        &["config", "-z", "--local", "--list"],
        &["symbolic-ref", "HEAD"],
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname) %(upstream)",
        ],
    ];
    words
        .into_iter()
        .map(|words| s.private(dir).git(words).succeeds().stdout)
        .collect()
}

/// What `git dupe status --porcelain -z` shows in the workspace at `dir`.
fn status(s: &Scenario, dir: &Path) -> Vec<u8> {
    s.git(["dupe", "status", "--porcelain", "-z"])
        .from(dir)
        .succeeds()
        .stdout
}

/// What the kill at `killed.point` left, by the part of the run it fell in: no private
/// repository before Git's `init` ran, then one; the working tree as it was until
/// `checkout-index` ran, then as the uninterrupted run left it; the region absent until
/// settle renamed it into place. Where the files are written and no region stands,
/// `git dupe status`, in a copy, settles it (`Composition/Keeper`: the region is absent
/// "after a kill before the first settle of `init` or `clone`").
fn left_by_the_kill(sweep: &Sweep, run: &Run, killed: &Killed) {
    let point = killed.point;
    let attached = killed.dir.join(".git/dupe").is_dir();
    assert_eq!(attached, run.ran(point, "init"), "{killed:?}");
    let written = run.ran(point, "checkout-index");
    let expected = if written {
        outside(&sweep.reference)
    } else {
        outside(&sweep.fixture)
    };
    let changed = expected.changed_in(&outside(&killed.dir));
    assert!(changed.is_empty(), "{killed:?} changed {changed:?}");
    let gitdupe = if written { Version::New } else { Version::Old };
    assert_eq!(killed.gitdupe, gitdupe, "{killed:?}");
    let renamed = run.renamed(point);
    let exclude = if renamed { Version::New } else { Version::Old };
    assert_eq!(killed.exclude, exclude, "{killed:?}");

    if written && !renamed {
        assert!(region(&killed.dir).is_none(), "{killed:?}");
        let instead = copied(sweep.s, &killed.dir, "instead");
        let settled = sweep.s.git(["dupe", "status"]).from(&instead).run();
        assert_eq!(settled.end, End::Code(0), "{killed:?}: {settled:?}");
        assert_eq!(
            region_rules(&instead),
            region_rules(&sweep.reference),
            "{killed:?}"
        );
    }
}

/// `git dupe detach --force` in the killed workspace: where `.git/dupe` is a directory it
/// exits 0 and removes it and the region, every other file as it was and the exclude file
/// as before the command; where it is not, it is refused naming `git dupe init` and changes
/// nothing.
fn detach_force(sweep: &Sweep, killed: &Killed) {
    let s = sweep.s;
    let dir = &killed.dir;
    let attached = dir.join(".git/dupe").is_dir();
    let all = Tree::of(dir);
    let kept = (outside(dir), files(dir).gitdupe);
    let output = s.git(["dupe", "detach", "--force"]).from(dir).run();
    assert!(output.stdout.is_empty(), "{killed:?}: {output:?}");
    if !attached {
        assert_eq!(output.end, End::Code(128), "{killed:?}: {output:?}");
        names(output.only_line("fatal"), b"git dupe init");
        let changed = all.changed_in(&Tree::of(dir));
        assert!(changed.is_empty(), "{killed:?} changed {changed:?}");
        return;
    }
    assert_eq!(output.end, End::Code(0), "{killed:?}: {output:?}");
    for level in ["fatal", "error"] {
        assert!(output.lines(level).is_empty(), "{killed:?}: {output:?}");
    }
    detached(dir);
    let changed = kept.0.changed_in(&outside(dir));
    assert!(changed.is_empty(), "{killed:?} changed {changed:?}");
    assert_eq!(files(dir).gitdupe, kept.1, "{killed:?}");
    assert_eq!(files(dir).exclude, sweep.before.exclude, "{killed:?}");
}

/// `git dupe clone URL` again in the workspace at `dir`: exit 0, no line of git-dupe's
/// but its warnings, and the private repository and everything outside it but `.gitdupe`
/// and the exclude file as the uninterrupted run left them, but at the paths `differing`
/// of the working tree.
fn clone_again(sweep: &Sweep, dir: &Path, differing: &[&str]) -> Output {
    let s = sweep.s;
    let output = s.git(["dupe"].iter().chain(sweep.words)).from(dir).run();
    assert_eq!(output.end, End::Code(0), "{output:?}");
    for level in ["fatal", "error", "hint"] {
        assert!(output.lines(level).is_empty(), "{level}: {output:?}");
    }
    let mut changed = outside(&sweep.reference).changed_in(&outside(dir));
    changed.sort();
    assert_eq!(
        changed,
        differing.iter().map(PathBuf::from).collect::<Vec<_>>()
    );
    assert_eq!(
        private_state(s, dir),
        private_state(s, &sweep.reference),
        "{output:?}"
    );
    output
}

/// Sweeps `clone` over the second machine and, at each point of `part`, checks what the
/// kill left, then starts over with `detach --force` and `clone`, which must end as the
/// uninterrupted run did, with its warnings: `.env.local` named, no file the killed run
/// wrote named.
fn killed_in(part: Part) {
    under_each_release(|s| {
        let (fixture, url) = fixture(s);
        let words = ["clone", url.as_str()];
        let sweep = Sweep::writing(s, &fixture, &words);
        let warnings = sweep.uninterrupted.lines("warning");
        assert_eq!(warnings.len(), 1, "{:?}", sweep.uninterrupted);
        names(warnings[0], b".env.local");
        let reference_status = status(s, &sweep.reference);
        assert_eq!(reference_status, b" M .env.local\0");

        let commands = commands(s, &fixture, &words);
        let run = Run {
            commands: &commands,
        };
        // Before and after each Git run, and inside the region's write under a limit of no
        // block and of one.
        assert_eq!(sweep.points().len(), 2 * commands.len() + 2, "{commands:?}");
        let points: Vec<Point> = sweep
            .points()
            .iter()
            .copied()
            .filter(|&point| run.part(point) == part)
            .collect();
        assert!(!points.is_empty(), "{part:?}: {commands:?}");
        for point in points {
            let killed = sweep.kill(point);
            left_by_the_kill(&sweep, &run, &killed);
            detach_force(&sweep, &killed);
            let again = clone_again(&sweep, &killed.dir, &[]);
            assert_eq!(files(&killed.dir), sweep.after, "{killed:?}: {again:?}");
            assert_eq!(again.lines("warning"), warnings, "{killed:?}: {again:?}");
            assert_eq!(status(s, &killed.dir), reference_status, "{killed:?}");
        }
    });
}

#[test]
fn a_clone_killed_before_git_init_is_refused_by_detach_force_and_starts_over() {
    killed_in(Part::Refusals);
}

#[test]
fn a_clone_killed_in_init_starts_over() {
    killed_in(Part::Init);
}

#[test]
fn a_clone_killed_while_fetching_and_checking_out_starts_over() {
    killed_in(Part::Steps);
}

#[test]
fn a_clone_killed_in_its_write_step_starts_over() {
    killed_in(Part::Write);
}

#[test]
fn a_clone_killed_in_settle_starts_over_and_status_settles_it() {
    killed_in(Part::Settle);
}

/// The way `help clone` gives to start over: `git dupe detach`, or `git dupe detach --force`
/// where it refuses, then `git dupe clone`. After a `clone` that failed, plain `detach`
/// leaves; killed before `reset -q`, or before the write step, the private repository shows
/// changes not committed, and `detach` refuses naming `--force`, which leaves. Either way
/// the second `clone` ends as the uninterrupted run did.
#[test]
fn the_way_help_clone_gives_to_start_over_ends_as_an_uninterrupted_clone() {
    under_each_release(|s| {
        let (fixture, url) = fixture(s);
        let words = ["clone", url.as_str()];
        let sweep = Sweep::writing(s, &fixture, &words);

        let failed = copied(s, &fixture, "failed");
        let output = s
            .git(["dupe", "clone", url.as_str(), "-b", "nosuch"])
            .from(&failed)
            .run();
        assert_ne!(output.end, End::Code(0), "{output:?}");
        assert!(failed.join(".git/dupe").is_dir());
        let detach = s.git(["dupe", "detach"]).from(&failed).run();
        assert_eq!(detach.end, End::Code(0), "{detach:?}");
        detached(&failed);
        clone_again(&sweep, &failed, &[]);

        let commands = commands(s, &fixture, &words);
        let reset = first(&commands, "reset");
        for point in [Point::BeforeRun(reset), Point::AfterRun(reset)] {
            let killed = sweep.kill(point);
            let detach = s.git(["dupe", "detach"]).from(&killed.dir).run();
            assert_eq!(detach.end, End::Code(128), "{killed:?}: {detach:?}");
            names(detach.only_line("fatal"), b"'git dupe detach --force'");
            detach_force(&sweep, &killed);
            clone_again(&sweep, &killed.dir, &[]);
        }
    });
}

/// Killed once `checkout-index` has run, with two files it wrote then cut short by hand, as
/// a kill inside that run could leave them: `notes/a.md` and `.gitdupe`. The start over
/// keeps each as cut, names each in one `warning:` beside `.env.local`'s, and shows each
/// modified, and the region follows the cut `.gitdupe` (G2 "the hidden paths follow it");
/// every other path is as the uninterrupted run left it.
#[test]
fn files_left_partly_written_are_kept_named_and_shown_modified_by_the_start_over() {
    under_each_release(|s| {
        let (fixture, url) = fixture(s);
        let words = ["clone", url.as_str()];
        let sweep = Sweep::writing(s, &fixture, &words);
        let commands = commands(s, &fixture, &words);
        let killed = sweep.kill(Point::AfterRun(first(&commands, "checkout-index")));
        // `notes\n.vscode\n` and `private\n`, each without its last five bytes.
        let mut cut = Vec::new();
        for path in [".gitdupe", "notes/a.md"] {
            let at = killed.dir.join(path);
            let whole = fs::read(&at).unwrap();
            let part = whole[..whole.len() - 5].to_vec();
            fs::write(&at, &part).unwrap();
            cut.push((at, part));
        }
        assert_eq!(cut[0].1, b"notes\n.vs");

        detach_force(&sweep, &killed);
        let again = clone_again(&sweep, &killed.dir, &["notes/a.md"]);
        for (at, part) in &cut {
            assert_eq!(&fs::read(at).unwrap(), part, "{}", at.display());
        }
        let warnings = again.lines("warning");
        assert_eq!(warnings.len(), 3, "{again:?}");
        for (warning, path) in warnings
            .iter()
            .zip([".env.local", ".gitdupe", "notes/a.md"])
        {
            names(
                warning,
                format!("{path} was kept as it was and differs").as_bytes(),
            );
        }
        assert_eq!(
            status(s, &killed.dir),
            b" M .env.local\0 M .gitdupe\0 M notes/a.md\0"
        );
        // The hidden paths are the cut file's and the privately tracked files.
        let rules: Vec<&[u8]> = vec![
            b"/.env.local",
            b"/.gitdupe",
            b"/.vs",
            b"/.vscode/settings.json",
            b"/docs/notes.md",
            b"/notes",
        ];
        assert_eq!(region_rules(&killed.dir), rules);
    });
}
