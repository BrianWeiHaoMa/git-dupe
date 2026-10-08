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
//!
//! A run is started, and waited for later, through one control directory at a time: its
//! records are that run's. The same limit with `SIGXFSZ` ignored makes git-dupe's own
//! writes fail, not kill it, while Git's runs write as ever.
//!
//! The same forwarder can select a run by its argument prefix and public/private
//! environment, to fail it, truncate its real answer, observe bytes before it starts,
//! or pause it. A pause has a bounded wait and its process group is cleaned up even
//! when the scenario fails; no product hook or second forwarding script is needed.

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::output::{End, Output};
use super::running::Running;
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
if [ -f "$control/selection" ]; then
    read -r scope < "$control/scope"
    read -r selected < "$control/selection"
    actual=public
    case ${GIT_DIR-} in */dupe) actual=private ;; esac
    if [ "$scope" = "$actual" ]; then
        case "$*" in "$selected"|"$selected "*)
            candidate=1
            if [ -f "$control/candidates" ]; then
                read -r candidate < "$control/candidates"
                candidate=$((candidate + 1))
            fi
            printf '%s\n' "$candidate" > "$control/candidates"
            wanted=1
            if [ -f "$control/occurrence" ]; then read -r wanted < "$control/occurrence"; fi
            if [ "$candidate" -ne "$wanted" ]; then exec "$release" "$@"; fi
            if [ -f "$control/observed-file" ]; then
                read -r observed < "$control/observed-file"
                if cmp -s "$observed" "$control/expected"; then
                    printf 'equal\n' > "$control/observed"
                else
                    printf 'different\n' > "$control/observed"
                fi
            fi
            printf 'selected\n' >> "$control/matched"
            read -r effect < "$control/effect"
            case $effect in
            exit*) exit "${effect#exit }" ;;
            truncate)
                "$release" "$@" > "$control/answer"
                status=$?
                count=$(wc -c < "$control/answer")
                [ "$count" -gt 0 ] || exit 125
                head -c "$((count - 1))" "$control/answer"
                exit "$status"
                ;;
            pause)
                n=0
                while [ ! -f "$control/released" ]; do
                    [ "$n" -lt 3000 ] || exit 125
                    sleep 0.01
                    n=$((n + 1))
                done
                ;;
            esac
            ;;
        esac
    fi
fi
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
    /// Whether a run started through the control directory has not been waited for.
    started: Cell<bool>,
}

impl Scenario {
    /// `git dupe <words>`, to be killed, its control directory `name` below the scenario's
    /// directory and outside every workspace.
    pub fn killing(&self, name: &str, words: &[&str]) -> Killing<'_> {
        let control = self.dir().join(name);
        fs::create_dir(&control).unwrap_or_else(|cause| panic!("{}: {cause}", control.display()));
        // Written by a process of its own: a descriptor of this process open to write it
        // could be inherited by a child another scenario's thread is starting, and running
        // the script while that child holds it fails with ETXTBSY.
        let script = control.join("git");
        self.program(
            "/bin/sh",
            [
                OsStr::new("-c"),
                OsStr::new(r#"cat > "$0" && chmod 755 "$0""#),
                script.as_os_str(),
            ],
        )
        .input(SCRIPT.as_bytes())
        .succeeds();
        fs::write(
            control.join("release"),
            [self.release_git().as_os_str().as_encoded_bytes(), b"\n"].concat(),
        )
        .unwrap();
        Killing {
            scenario: self,
            words: ["dupe"].iter().chain(words).map(OsString::from).collect(),
            control,
            started: Cell::new(false),
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
        self.start_killed(dir, point).wait()
    }

    /// Starts the command in the workspace `dir`, to be killed at `point`, and returns at
    /// once; `KilledRun::wait` checks the kill as `killed` does.
    pub fn start_killed(&self, dir: &Path, point: Point) -> KilledRun<'_> {
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
        let started = Started::through(self);
        KilledRun {
            killing: self,
            point,
            expected,
            running: git.start(),
            started,
        }
    }

    /// Runs the command uninterrupted in the workspace `dir`, through the script, with
    /// every write of git-dupe's own failing: under a file size limit of no block with
    /// `SIGXFSZ` ignored, so that its first write of a byte fails with `EFBIG` and it goes
    /// on. Fails if the script killed anything or git-dupe died of the limit.
    pub fn writes_failing(&self, dir: &Path) -> Output {
        self.prepare(None);
        let output = self
            .git(dir)
            .file_size_limit(0)
            .file_size_signal_ignored()
            .run();
        assert!(
            self.record().is_none() && output.end != End::Code(128 + SIGXFSZ),
            "git {:?} did not go on past its failed writes: {output:?}",
            self.words
        );
        output
    }

    /// Clears the record of the last run and sets the point to kill at. Fails while a run
    /// started through the control directory has not been waited for.
    fn prepare(&self, point: Option<&str>) {
        assert!(
            !self.started.get(),
            "git {:?} is started again before its last run was waited for",
            self.words
        );
        for record in [
            "count",
            "parents",
            "killed",
            "kill",
            "matched",
            "candidates",
            "observed",
            "released",
            "answer",
        ] {
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

    /// The script's record of the point at which it killed and whom, if it killed.
    fn record(&self) -> Option<String> {
        match fs::read_to_string(self.control.join("killed")) {
            Ok(killed) => Some(killed),
            Err(cause) if cause.kind() == ErrorKind::NotFound => None,
            Err(cause) => panic!("{}: {cause}", self.control.display()),
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

/// A run to be killed, started and not yet waited for.
pub struct KilledRun<'k> {
    killing: &'k Killing<'k>,
    point: Point,
    expected: Option<String>,
    running: Running,
    started: Started<'k>,
}

impl KilledRun<'_> {
    /// Waits for the run: fails unless git-dupe itself was killed at its point, and
    /// returns what it printed once no process of the run is left.
    pub fn wait(self) -> Output {
        self.checked(Running::wait)
    }

    /// Waits as `wait` does, `limit` at most, as `Running::wait_within` bounds it: past
    /// that, the scenario fails, and the run is killed and reaped.
    pub fn wait_within(self, limit: Duration) -> Output {
        self.checked(|running| running.wait_within(limit))
    }

    /// The run waited for by `wait`, then checked: git-dupe itself was killed at its point.
    fn checked(self, wait: impl FnOnce(Running) -> Output) -> Output {
        let KilledRun {
            killing,
            point,
            expected,
            running,
            started,
        } = self;
        let output = wait(running);
        let killed = killing.record();
        let (signal, record) = match expected {
            Some(expected) => (SIGKILL, Some(format!("{expected} git-dupe\n"))),
            None => (SIGXFSZ, None),
        };
        assert!(
            output.end == End::Code(128 + signal) && killed == record,
            "git {:?} was not killed at {point:?}: the script killed {killed:?}; {output:?}",
            killing.words
        );
        drop(started);
        output
    }
}

/// That a run started through a control directory has not been waited for, while this
/// lives.
struct Started<'k>(&'k Cell<bool>);

impl<'k> Started<'k> {
    fn through(killing: &'k Killing) -> Started<'k> {
        killing.started.set(true);
        Started(&killing.started)
    }
}

impl Drop for Started<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// One selected Git run's effect, while every other run forwards unchanged.
pub enum ForwardEffect {
    Exit(i32),
    Truncate,
    Pause,
}

/// A command using the kill harness's one forwarder, selected by arguments and scope.
pub struct Forwarded<'s> {
    killing: Killing<'s>,
}

impl Scenario {
    /// Selects an argument prefix in either the private (`GIT_DIR` ending in `/dupe`)
    /// or public environment. The prefix ends at a word boundary.
    pub fn forwarded(
        &self,
        name: &str,
        words: &[&str],
        private: bool,
        prefix: &[&str],
        effect: ForwardEffect,
    ) -> Forwarded<'_> {
        let killing = self.killing(name, words);
        assert!(!prefix.is_empty());
        fs::write(killing.control.join("selection"), prefix.join(" ")).unwrap();
        fs::write(
            killing.control.join("scope"),
            if private { "private" } else { "public" },
        )
        .unwrap();
        let effect = match effect {
            ForwardEffect::Exit(code) => {
                assert!(code > 0 && code < 256);
                format!("exit {code}")
            }
            ForwardEffect::Truncate => "truncate".into(),
            ForwardEffect::Pause => "pause".into(),
        };
        fs::write(killing.control.join("effect"), effect).unwrap();
        Forwarded { killing }
    }
}

impl Forwarded<'_> {
    /// Selects the nth matching run, when a handler and settle use the same arguments.
    /// This counts matching arguments and scope, never unrelated runs.
    pub fn on_match(self, occurrence: usize) -> Self {
        assert!(occurrence > 0);
        fs::write(
            self.killing.control.join("occurrence"),
            occurrence.to_string(),
        )
        .unwrap();
        self
    }

    /// Records whether `file` holds exactly `bytes` when the selected run starts.
    pub fn observe(&self, file: &Path, bytes: &[u8]) {
        fs::write(
            self.killing.control.join("observed-file"),
            file.as_os_str().as_encoded_bytes(),
        )
        .unwrap();
        fs::write(self.killing.control.join("expected"), bytes).unwrap();
    }

    pub fn observed_equal(&self) {
        assert_eq!(
            fs::read(self.killing.control.join("observed")).unwrap(),
            b"equal\n"
        );
    }

    pub fn run(&self, dir: &Path) -> Output {
        self.start(dir).wait()
    }

    /// Starts in a process group: dropping the handle cleans up a failed pause too.
    pub fn start(&self, dir: &Path) -> ForwardedRun<'_> {
        self.killing.prepare(None);
        ForwardedRun {
            forwarded: self,
            running: self.killing.git(dir).start(),
            _started: Started::through(&self.killing),
        }
    }
}

pub struct ForwardedRun<'f> {
    forwarded: &'f Forwarded<'f>,
    running: Running,
    _started: Started<'f>,
}

impl ForwardedRun<'_> {
    /// Waits until the selected run has started, with no sleep standing in for the seam.
    pub fn selected(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !self.forwarded.killing.control.join("matched").exists() {
            assert!(
                !self.running.finished(),
                "command ended before its selected run"
            );
            assert!(Instant::now() < deadline, "selected Git run did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn release(self) -> Output {
        fs::write(self.forwarded.killing.control.join("released"), b"").unwrap();
        self.wait()
    }

    pub fn wait(self) -> Output {
        let output = self.running.wait_within(Duration::from_secs(35));
        assert_eq!(
            fs::read(self.forwarded.killing.control.join("matched")).unwrap(),
            b"selected\n",
            "selected run must occur exactly once: {output:?}"
        );
        output
    }
}
