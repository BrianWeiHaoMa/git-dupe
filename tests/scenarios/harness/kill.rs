//! Killing a running `git dupe` from outside, at each point where its own state can
//! change, with nothing added to the product for it (N6): at each of its Git runs, and
//! inside each of its own writes.
//!
//! Git puts its exec path first on the `PATH` it hands `git-dupe`, so git-dupe finds
//! `git` there. With `GIT_EXEC_PATH` naming a control directory that holds only the
//! script `git` below, every Git run git-dupe starts is that script, and its parent is
//! git-dupe. The script counts its runs, forgets `GIT_EXEC_PATH`, and runs the release's
//! own `git` with the same words and streams, so that Git's own children never come back
//! through it. At the chosen run it kills its parent with `SIGKILL`, either before Git
//! runs or once Git has run to its end: git-dupe's own writes fall between one run's end
//! and the next run's start, and the two kills stand on either side of each.
//!
//! A kill inside a write: the run starts under a soft file size limit, which the script
//! raises again before Git runs, so that only git-dupe's own writes meet it, and git-dupe
//! dies of `SIGXFSZ` in the first write of its own that passes the limit, what it wrote up
//! to the limit left in the file.
//!
//! Git reports a dashed command killed by a signal with the status a shell gives it, 128
//! plus the signal's number; that status and the script's record of whom it killed tell a
//! delivered kill from a run that ended by itself.

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::output::{End, Output};
use super::scenario::Scenario;

const SIGKILL: i32 = 9;
const SIGXFSZ: i32 = 25;

/// The `git` that git-dupe finds. Its files beside it: `release`, the path of the
/// release's `git`; `kill`, the point to kill at, `before <n>` or `after <n>`, absent for
/// none; `count`, the runs so far; `parents`, the name of each run's parent; `killed`, the
/// point at which it killed and whom.
const SCRIPT: &str = r#"#!/bin/sh
ulimit -S -f "$(ulimit -H -f)" || exit 125
control=${0%/*}
n=1
if [ -f "$control/count" ]; then read -r n < "$control/count"; n=$((n + 1)); fi
printf '%s\n' "$n" > "$control/count"
read -r parent < "/proc/$PPID/comm"
printf '%s\n' "$parent" >> "$control/parents"
unset GIT_EXEC_PATH
read -r release < "$control/release"
point=
if [ -f "$control/kill" ]; then read -r point < "$control/kill"; fi
case $point in
"before $n")
    printf '%s %s\n' "$point" "$parent" > "$control/killed"
    kill -KILL "$PPID"
    exit 0
    ;;
"after $n")
    "$release" "$@"
    printf '%s %s\n' "$point" "$parent" > "$control/killed"
    kill -KILL "$PPID"
    exit 0
    ;;
esac
exec "$release" "$@"
"#;

/// Where a run of a `git dupe` command is killed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Point {
    /// As it starts its `n`-th Git run, counted from 1, before Git runs.
    BeforeRun(usize),
    /// Once its `n`-th Git run has ended, before it acts on what the run did.
    AfterRun(usize),
    /// Inside the first write of its own that passes `blocks` of the shell's blocks: 0
    /// stops the first write of at least one byte, 1 the first larger than a block, which
    /// is 512 bytes in some shells and 1,024 in others.
    InsideWrite { blocks: u32 },
}

/// One `git dupe` command, run in a workspace given each time, uninterrupted or killed.
pub struct Killing<'s> {
    scenario: &'s Scenario,
    words: Vec<OsString>,
    control: PathBuf,
}

impl Scenario {
    /// `git dupe <words>`, to be killed, its control directory `name` below the scenario's
    /// directory and outside every workspace.
    pub fn killing(&self, name: &str, words: &[&str]) -> Killing<'_> {
        let control = self.dir().join(name);
        fs::create_dir(&control).unwrap_or_else(|cause| panic!("{}: {cause}", control.display()));
        let script = control.join("git");
        fs::write(&script, SCRIPT).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(
            control.join("release"),
            [self.release_git().as_os_str().as_encoded_bytes(), b"\n"].concat(),
        )
        .unwrap();
        Killing {
            scenario: self,
            words: ["dupe"].iter().chain(words).map(OsString::from).collect(),
            control,
        }
    }
}

impl Killing<'_> {
    /// Runs the command uninterrupted in the workspace `dir`, through the script, and
    /// returns what it printed and every point at which it can be killed, in the order it
    /// reaches them: before and after each of its Git runs, then inside a write under a
    /// limit of no block and of one.
    pub fn uninterrupted(&self, dir: &Path) -> (Output, Vec<Point>) {
        self.prepare(None);
        let output = self.git(dir).run();
        let parents = fs::read_to_string(self.control.join("parents")).unwrap_or_default();
        let runs = parents.lines().count();
        assert!(
            runs > 0 && parents.lines().all(|parent| parent == "git-dupe"),
            "git-dupe's Git runs did not all come through the script: {parents:?}; {output:?}"
        );
        let points = (1..=runs)
            .flat_map(|n| [Point::BeforeRun(n), Point::AfterRun(n)])
            .chain([0, 1].map(|blocks| Point::InsideWrite { blocks }))
            .collect();
        (output, points)
    }

    /// Runs the command in the workspace `dir`, killed at `point`, and returns what it
    /// printed. Fails unless git-dupe itself was killed there, and returns only once no
    /// process of the run is left.
    pub fn killed(&self, dir: &Path, point: Point) -> Output {
        let expected = match point {
            Point::BeforeRun(n) => Some(format!("before {n}")),
            Point::AfterRun(n) => Some(format!("after {n}")),
            Point::InsideWrite { .. } => None,
        };
        self.prepare(expected.as_deref());
        let git = match point {
            Point::InsideWrite { blocks } => self.git(dir).file_size_limit(blocks),
            _ => self.git(dir),
        };
        let output = git.run();
        let killed = match fs::read_to_string(self.control.join("killed")) {
            Ok(killed) => Some(killed),
            Err(cause) if cause.kind() == ErrorKind::NotFound => None,
            Err(cause) => panic!("{}: {cause}", self.control.display()),
        };
        let (signal, record) = match expected {
            Some(expected) => (SIGKILL, Some(format!("{expected} git-dupe\n"))),
            None => (SIGXFSZ, None),
        };
        assert!(
            output.end == End::Code(128 + signal) && killed == record,
            "git {:?} was not killed at {point:?}: the script killed {killed:?}; {output:?}",
            self.words
        );
        output
    }

    /// Clears the record of the last run and sets the point to kill at.
    fn prepare(&self, point: Option<&str>) {
        for record in ["count", "parents", "killed", "kill"] {
            match fs::remove_file(self.control.join(record)) {
                Ok(()) => {}
                Err(cause) if cause.kind() == ErrorKind::NotFound => {}
                Err(cause) => panic!("{}: {cause}", self.control.display()),
            }
        }
        if let Some(point) = point {
            fs::write(self.control.join("kill"), format!("{point}\n")).unwrap();
        }
    }

    fn git(&self, dir: &Path) -> super::scenario::Git<'_> {
        self.scenario
            .git(&self.words)
            .from(dir)
            .variable("GIT_EXEC_PATH", &self.control)
            .leaving_no_process()
    }
}
