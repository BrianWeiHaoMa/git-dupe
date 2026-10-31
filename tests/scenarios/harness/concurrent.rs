//! Two commands started together, as two worktrees' commands run at the same time (E2,
//! G28): the second started as soon as the first has been, before either is waited for,
//! and each then waited for within a bound. Nothing orders them further, so whether their
//! runs overlap, and which takes the lock of the common Git directory first, is the
//! machine's; the bound guards against a stall and measures nothing.
//!
//! What such a command printed is read through `warnings_beside_another`: where a Git
//! reading `.git/info/exclude` can fail at the moment the other command replaces it, the
//! seventh accepted limit, the one warning that limit names is admitted, with Git's own two
//! lines above it, and reported with its release. Nothing is retried.
//!
//! Each command is a plain run, `Git::start`, or one to be killed,
//! `Killing::start_killed`, whose kill `KilledRun` checks as it is waited for. A failure
//! after either start — in the other start, in a wait, or in a check of a killed run —
//! drops what was started and not yet collected, and `Running` kills and reaps its
//! process group, so that nothing of either command goes on.

use std::time::Duration;

use super::kill::KilledRun;
use super::output::Output;
use super::platform::a_replacement_can_fail_a_reader;
use super::running::Running;
use super::scenario::{Scenario, report};

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

/// The `warning:` line that says exposure could not be asked because Git failed as the limit
/// says, exit 128 (`src/keeper/exposure.rs`); no other failure of the question is admitted.
const UNASKED: &[u8] = b"warning: cannot ask public Git whether it ignores the hidden paths \
      (git exited with 128); exposure was not checked";

/// What a command run at once with another worktree's said.
pub struct Warnings<'o> {
    /// The texts of its `warning:` lines, but what the limit admits.
    pub lines: Vec<&'o [u8]>,
    /// The texts of its `fatal:` lines, but what the limit admits.
    pub fatals: Vec<&'o [u8]>,
    /// Whether the command could not ask about exposure, so that an assertion that needs
    /// Git's answer does not hold.
    pub unasked: bool,
}

/// The one reading of what a command run at once with another worktree's printed. Elsewhere
/// than under the seventh accepted limit this is every such line, so that a scenario asserts
/// over it without a branch on the platform or the release.
pub fn warnings_beside_another<'o>(s: &Scenario, output: &'o Output) -> Warnings<'o> {
    let said = read_beside_another(&output.stderr, a_replacement_can_fail_a_reader());
    if said.unasked {
        report(&format!(
            "exposure unasked beside another command, under {}",
            s.release_word()
        ));
    }
    said
}

/// `stderr` read as `warnings_beside_another` says, the limit admitted where `limited`.
fn read_beside_another(stderr: &[u8], limited: bool) -> Warnings<'_> {
    let mut lines: Vec<&[u8]> = stderr.split(|&byte| byte == b'\n').collect();
    let at = lines.iter().position(|line| *line == UNASKED);
    let at = at.filter(|_| limited);
    if let Some(at) = at {
        // The failed `check-ignore`'s standard error is inherited: its own two lines stand
        // directly above, read no further than their level, as Git's text is.
        let above = at
            .checked_sub(2)
            .map(|first| (lines[first], lines[first + 1]));
        let its_own = above.is_some_and(|(warning, fatal)| {
            warning.starts_with(b"warning: ") && fatal.starts_with(b"fatal: ")
        });
        lines.drain(if its_own { at - 2 } else { at }..=at);
    }
    let texts = |opening: &[u8]| -> Vec<&[u8]> {
        let levelled = lines.iter().filter_map(|line| line.strip_prefix(opening));
        levelled.collect()
    };
    Warnings {
        lines: texts(b"warning: "),
        fatals: texts(b"fatal: "),
        unasked: at.is_some(),
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
    use std::ffi::OsStr;
    use std::fs;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::Path;
    use std::thread;
    use std::time::Instant;

    use super::super::output::End;
    use super::super::platform::{self, shell_process_number};
    use super::super::running::ended;
    use super::super::scenario::Scenario;
    use super::*;

    /// How long a check below gives a process to record itself or to end.
    const ENDING: Duration = Duration::from_secs(20);

    /// A run that records its process ID in `file`, whole, and then sleeps far longer than
    /// any check; returned once the ID is recorded.
    fn sleeping(s: &Scenario, file: &Path) -> Running {
        let number = shell_process_number("$");
        let script = format!(r#"{number} > "$0.new" && mv "$0.new" "$0" && exec "$@""#);
        let sleep = platform::sleeping(120);
        let words = [OsStr::new("-c"), OsStr::new(&script), file.as_os_str()]
            .into_iter()
            .chain([sleep.get_program()])
            .chain(sleep.get_args());
        let running = s.program(s.shell(), words).start();
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
            if ended(process) {
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
        let waiting = r#"while [ ! -e "$0" ]; do :; done"#;
        let (first, second) = together_within(
            ENDING,
            || {
                s.program(s.shell(), ["-c", waiting, started.to_str().unwrap()])
                    .start()
            },
            || {
                s.program(s.shell(), ["-c", r#": > "$0""#, started.to_str().unwrap()])
                    .start()
            },
        );
        assert_eq!(first.end, End::Code(0), "{first:?}");
        assert_eq!(second.end, End::Code(0), "{second:?}");
    }

    const GIT_LINE: &[u8] = b"warning: unable to access 'x'\nfatal: cannot use x\n";

    fn unasked_line() -> Vec<u8> {
        [UNASKED, b"\n"].concat()
    }

    /// Under the limit the one warning goes, with Git's two lines directly above it, and
    /// every other line stays; elsewhere every line stays and nothing is admitted.
    #[test]
    fn the_limit_admits_the_failed_question_and_its_lines_alone() {
        let settle = b"warning: cannot list what the private repository tracks (x)\n";
        let stderr = [
            b"hint: x\n".as_slice(),
            b"warning: keeps\n",
            GIT_LINE,
            &unasked_line(),
            settle,
        ]
        .concat();

        let admitted = read_beside_another(&stderr, true);
        assert!(admitted.unasked);
        assert!(admitted.fatals.is_empty());
        assert_eq!(
            admitted.lines,
            [
                b"keeps".as_slice(),
                b"cannot list what the private repository tracks (x)"
            ]
        );

        let elsewhere = read_beside_another(&stderr, false);
        assert!(!elsewhere.unasked);
        assert_eq!(elsewhere.fatals, [b"cannot use x".as_slice()]);
        assert_eq!(elsewhere.lines.len(), 4);
    }

    /// A `warning:` or `fatal:` line anywhere else stays under the limit: no failed question
    /// stands below it.
    #[test]
    fn a_line_the_limit_does_not_name_stays() {
        let stray = read_beside_another(GIT_LINE, true);
        assert!(!stray.unasked);
        assert_eq!(stray.lines, [b"unable to access 'x'".as_slice()]);
        assert_eq!(stray.fatals, [b"cannot use x".as_slice()]);

        // Another failure of the question is no failure of the limit.
        let other = [
            GIT_LINE,
            b"warning: cannot ask public Git whether it ignores the hidden paths \
              (git exited with 129); exposure was not checked\n",
        ]
        .concat();
        let other = read_beside_another(&other, true);
        assert!(!other.unasked);
        assert_eq!((other.lines.len(), other.fatals.len()), (2, 1));

        // Standing alone, or below lines that are not the failed run's, it takes only itself.
        let stderr = [b"fatal: x\n".as_slice(), &unasked_line()].concat();
        let alone = read_beside_another(&stderr, true);
        assert!(alone.unasked);
        assert_eq!(alone.fatals, [b"x".as_slice()]);
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
