//! Two commands started together, as two worktrees' commands run at the same time (E2,
//! G28): the second started as soon as the first has been, before either is waited for,
//! and each then waited for within a bound. Nothing orders them further, so whether their
//! runs overlap, and which takes the lock on the common Git directory first, is the
//! machine's; the bound guards against a stall and measures nothing.
//!
//! Each command is a plain run, `Git::start`, or one to be killed,
//! `Killing::start_killed`, whose kill `KilledRun` checks as it is waited for. A failure
//! after either start — in the other start, in a wait, or in a check of a killed run —
//! drops what was started and not yet collected, and `Running` kills and reaps its
//! process group, so that nothing of either command goes on.

use std::time::Duration;

use super::kill::KilledRun;
use super::output::Output;
use super::running::Running;

/// How long each command is given to end once it is waited for.
const LIMIT: Duration = Duration::from_secs(30);

/// A command started and not yet waited for.
pub trait StartedRun {
    /// Waits for it, `limit` at most, and checks it as its own wait does.
    fn wait_within(self, limit: Duration) -> Output;
}

impl StartedRun for Running {
    fn wait_within(self, limit: Duration) -> Output {
        Running::wait_within(self, limit)
    }
}

impl StartedRun for KilledRun<'_> {
    fn wait_within(self, limit: Duration) -> Output {
        KilledRun::wait_within(self, limit)
    }
}

/// Starts `first`, then `second` at once, and only then waits for the first and then the
/// second, 30 seconds at most each; returns what each printed, in that order.
pub fn started_together<F: StartedRun, S: StartedRun>(
    first: impl FnOnce() -> F,
    second: impl FnOnce() -> S,
) -> (Output, Output) {
    together_within(LIMIT, first, second)
}

fn together_within<F: StartedRun, S: StartedRun>(
    limit: Duration,
    first: impl FnOnce() -> F,
    second: impl FnOnce() -> S,
) -> (Output, Output) {
    let first = first();
    let second = second();
    let first = first.wait_within(limit);
    let second = second.wait_within(limit);
    (first, second)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::Path;
    use std::thread;
    use std::time::Instant;

    use super::super::output::End;
    use super::super::scenario::Scenario;
    use super::*;

    /// How long a check below gives a process to record itself or to end.
    const ENDING: Duration = Duration::from_secs(20);

    /// A run that records its process ID in `file`, whole, and then sleeps far longer than
    /// any check; returned once the ID is recorded.
    fn sleeping(s: &Scenario, file: &Path) -> Running {
        let script = r#"echo $$ > "$0.new" && mv "$0.new" "$0" && exec sleep 120"#;
        let running = s
            .program("/bin/sh", ["-c", script, file.to_str().unwrap()])
            .start();
        recorded(file);
        running
    }

    /// The process ID recorded in `file`, once it is there.
    fn recorded(file: &Path) -> String {
        let deadline = Instant::now() + ENDING;
        loop {
            if let Ok(process) = fs::read_to_string(file) {
                return process.trim_end().to_owned();
            }
            assert!(
                Instant::now() < deadline,
                "{} was not written",
                file.display()
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Waits until the process `process` has ended: gone, or a zombie.
    fn ends(process: &str) {
        let deadline = Instant::now() + ENDING;
        loop {
            let ended = fs::read_to_string(format!("/proc/{process}/stat")).map_or(true, |stat| {
                stat.rsplit_once(") ")
                    .is_some_and(|(_, after)| after.starts_with('Z'))
            });
            if ended {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the process {process} still runs"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// A first command that ends only once the second has started: waiting for the first
    /// before starting the second would never see it end.
    #[test]
    fn the_second_starts_before_the_first_is_waited_for() {
        let s = Scenario::of_the_first_release();
        let started = s.dir().join("second-started");
        let waiting = r#"while [ ! -e "$0" ]; do sleep 0.01; done"#;
        let (first, second) = together_within(
            ENDING,
            || {
                s.program("/bin/sh", ["-c", waiting, started.to_str().unwrap()])
                    .start()
            },
            || {
                s.program("/bin/sh", ["-c", r#": > "$0""#, started.to_str().unwrap()])
                    .start()
            },
        );
        assert_eq!(first.end, End::Code(0), "{first:?}");
        assert_eq!(second.end, End::Code(0), "{second:?}");
    }

    /// The second start fails while the first command runs: the first is killed.
    #[test]
    fn a_failure_after_the_first_start_leaves_no_process_of_it() {
        let s = Scenario::of_the_first_release();
        let first = s.dir().join("first");
        let failed = catch_unwind(AssertUnwindSafe(|| {
            together_within(
                ENDING,
                || sleeping(&s, &first),
                || -> Running { panic!("the second command cannot start") },
            )
        }));
        assert!(failed.is_err(), "the failed start passed");
        ends(&recorded(&first));
    }

    /// The wait for the first command fails while the second runs: both are killed.
    #[test]
    fn a_failed_wait_leaves_no_process_of_either_command() {
        let s = Scenario::of_the_first_release();
        let (one, two) = (s.dir().join("one"), s.dir().join("two"));
        let began = Instant::now();
        let failed = catch_unwind(AssertUnwindSafe(|| {
            together_within(
                Duration::from_millis(500),
                || sleeping(&s, &one),
                || sleeping(&s, &two),
            )
        }));
        assert!(failed.is_err(), "the wait for a sleeping command passed");
        assert!(
            began.elapsed() < ENDING,
            "the waits took {:?}",
            began.elapsed()
        );
        for file in [&one, &two] {
            ends(&recorded(file));
        }
    }
}
