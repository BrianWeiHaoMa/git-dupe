//! The lock on the common Git directory under which every worktree's command replaces
//! `.git/info/exclude` and deletes its region (`Holds/G28`), as a scenario takes and
//! watches it: held as another worktree's command holds it, found free, waited at by
//! commands, and never held by a `git-dupe` that starts a Git run (R10).
//!
//! The harness sees the lock only through `flock` requests of its own on the common Git
//! directory, which answer alike on Linux and macOS (S16), and git-dupe's work only
//! through the Git runs it starts, each recorded and answered with such a request before
//! it goes on (`watched_runs.rs`). A request that finds the lock taken as a run starts
//! finds git-dupe holding it, which R10 forbids, where nothing else can hold it: not while
//! the scenario holds it itself, and not while a second command of the watch runs, which
//! may hold it in its own replacement. There the run is answered without a verdict, so R10
//! is judged where one watched command runs at a time and the scenario runs nothing else
//! that takes the lock.
//!
//! That a command waits at the lock is seen in the order of its runs, which
//! `Composition/Keeper` fixes, and in the file once it ends, never in the kernel's list of
//! waiters, which macOS lacks. Settle reads the hidden paths, its last run a private
//! `ls-files`, then replaces the region under the lock, and only then starts its public
//! listing; `detach` reads the hidden paths and deletes its region under the lock the same
//! way. So while the scenario holds the lock, a command waiting at it stands after that
//! `ls-files` has ended, starts no run, and leaves the file as it was, for as long as the
//! hold lasts. Under its hold the scenario then replaces another worktree's region, as
//! that worktree's own command replaces it, the one change to the file E2 admits while a
//! command runs, and lets go: the command goes on with its public listing and ends with
//! that region kept beside its own, which a replacement composed from the file as read
//! before the change would lose (G28). A git-dupe that took no lock and paused between that
//! `ls-files` and its read of the file for longer than the hold, two `GRACE`s, would pass
//! both observations: the hold sizes that window.

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::attached::region_in;
use super::output::Output;
use super::running::{Running, ended};
use super::scenario::Scenario;
use super::watched_runs::{Event, Holders, Verdict, WatchedRuns};

/// How long a command is given to stand at the lock, and to end.
const LIMIT: Duration = Duration::from_secs(30);

/// How long every waiting command must stand at the lock, unmoving, before the scenario
/// changes the file under its hold, and then again before it lets go.
const GRACE: Duration = Duration::from_millis(500);

/// How many times a run looks for its answer, a hundredth of a second apart, before it
/// gives up, as the kill harness's pause gives up (`kill.rs`).
const LOOKS: u32 = 3000;

/// Takes the lock on the common Git directory `common`, as another worktree's command
/// takes it, until the descriptor returned is dropped.
pub fn hold_the_lock(common: &Path) -> File {
    let directory = File::open(common).unwrap();
    directory.lock().unwrap();
    directory
}

/// Whether nobody holds the lock on `common`: another descriptor takes it at once.
pub fn lock_is_free(common: &Path) -> bool {
    File::open(common).unwrap().try_lock().is_ok()
}

/// `git dupe` commands whose every Git run is watched for the lock on one common Git
/// directory.
pub struct LockWatch<'s> {
    scenario: &'s Scenario,
    common: PathBuf,
    control: PathBuf,
    /// How many commands it has started, each with a control directory of its own.
    started: Cell<usize>,
    /// Who besides git-dupe may hold the lock, which decides whether a run is judged.
    holders: Arc<Holders>,
    /// Whether the harness answers runs, and how many times a run looks for its answer:
    /// for a check of the harness itself, which withholds the answers.
    answered: Cell<bool>,
    looks: Cell<u32>,
}

impl Scenario {
    /// A watch of the lock on the common Git directory `common`, its control directories
    /// below `name` in the scenario's directory, outside every workspace.
    pub fn watching_the_lock(&self, name: &str, common: &Path) -> LockWatch<'_> {
        let control = self.dir().join(name);
        fs::create_dir(&control).unwrap_or_else(|cause| panic!("{}: {cause}", control.display()));
        LockWatch {
            scenario: self,
            common: common.to_path_buf(),
            control,
            started: Cell::new(0),
            holders: Arc::default(),
            answered: Cell::new(true),
            looks: Cell::new(LOOKS),
        }
    }
}

impl LockWatch<'_> {
    /// Runs `git dupe <words>` from `dir` and returns what it printed, as `Watched::wait`
    /// checks it.
    pub fn run(&self, dir: &Path, words: &[&str]) -> Output {
        self.start(dir, words).wait()
    }

    /// Starts `git dupe <words>` from `dir`, each Git run it starts answered from a thread
    /// of the harness, and returns at once.
    pub fn start(&self, dir: &Path, words: &[&str]) -> Watched {
        let number = self.started.get() + 1;
        self.started.set(number);
        let runs = WatchedRuns::through(
            self.scenario,
            self.control.join(number.to_string()),
            &self.common,
            &self.holders,
            self.answered.get(),
            self.looks.get(),
        );
        let words: Vec<OsString> = ["dupe"].iter().chain(words).map(OsString::from).collect();
        let running = self
            .scenario
            .git(&words)
            .from(dir)
            .variable("GIT_EXEC_PATH", runs.control())
            .start();
        Watched {
            named: format!("git {words:?}"),
            running,
            runs,
        }
    }

    /// Takes the lock on the common Git directory as another worktree's command takes it,
    /// until the hold is let go: by `Waiting::let_go`, or by dropping it.
    pub fn hold(&self) -> Held<'_> {
        assert!(
            !self.holders.scenario_holds(),
            "the scenario holds the lock through this watch already"
        );
        self.holders.scenario_takes();
        let lock = hold_the_lock(&self.common);
        let path = self.exclude();
        let exclude = fs::read(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
        Held {
            watch: self,
            lock: Some(lock),
            exclude,
        }
    }

    /// For a check of the harness itself: the commands started from now on are answered
    /// nothing, each run giving up once it has looked for its answer `looks` times.
    #[cfg(test)]
    fn withholding_answers(&self, looks: u32) {
        self.answered.set(false);
        self.looks.set(looks);
    }

    fn exclude(&self) -> PathBuf {
        self.common.join("info/exclude")
    }
}

/// A watched command, started and not yet waited for. Dropped unwaited, as a failing
/// scenario drops it, it is killed with its process group, the script waiting for an
/// answer among them, and then its runs are no longer answered.
pub struct Watched {
    named: String,
    running: Running,
    runs: WatchedRuns,
}

impl Watched {
    /// Waits for the command and returns what it printed. Fails unless it ended within
    /// `LIMIT`, every Git run it started came through the script and had its answer, and
    /// none of them started while git-dupe held the lock.
    fn wait(self) -> Output {
        self.finished().output
    }

    /// Waits for the command, stops answering its runs, and checks them as `wait` says.
    fn finished(self) -> Finished {
        let Watched {
            named,
            running,
            runs,
        } = self;
        let output = running.wait_within(LIMIT);
        let (events, verdicts) = runs.checked(&format!("{named}: {output:?}"));
        Finished {
            named,
            output,
            events,
            verdicts,
        }
    }

    /// How many events its log holds, where the command stands at the lock: its last
    /// event the start of a private `ls-files` whose process has ended, which in settle and
    /// in `detach` is the last run before the lock. Fails where the command has ended.
    fn stands_at_the_lock(&mut self) -> Option<usize> {
        assert!(
            !self.running.finished(),
            "{} ended while the scenario held the lock: {}",
            self.named,
            self.runs.log()
        );
        let events = self.runs.events();
        match events.last() {
            Some(last @ Event::Started { process, .. }) if last.listing(true) && ended(process) => {
                Some(events.len())
            }
            _ => None,
        }
    }
}

/// A watched command once ended: what it printed, its log's events, and the verdict on
/// each run the harness answered, in order.
struct Finished {
    named: String,
    output: Output,
    events: Vec<Event>,
    verdicts: Vec<Verdict>,
}

/// The lock held by the scenario through a watch, as another worktree's command holds it.
pub struct Held<'w> {
    watch: &'w LockWatch<'w>,
    lock: Option<File>,
    /// The exclude file as it stood when the lock was taken.
    exclude: Vec<u8>,
}

impl<'w> Held<'w> {
    /// Watched commands started while the hold lasts, found waiting at the lock: each
    /// stands after its last private `ls-files`, not ended, its log unmoving, and the
    /// exclude file as it was when the lock was taken, all of it holding at once for
    /// `GRACE`, within `LIMIT`. The scenario may look at more before it lets go.
    pub fn waiting_at_the_lock<const N: usize>(self, mut commands: [Watched; N]) -> Waiting<'w, N> {
        let path = self.watch.exclude();
        let standing = standing_at_the_lock(&mut commands, &path, &self.exclude, None);
        Waiting {
            held: self,
            commands,
            standing,
        }
    }
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        if let Some(lock) = self.lock.take() {
            self.watch.holders.scenario_lets_go(lock);
        }
    }
}

/// Watched commands found waiting at the lock the scenario holds.
pub struct Waiting<'w, const N: usize> {
    held: Held<'w>,
    commands: [Watched; N],
    /// How many events each command's log held as it stood at the lock.
    standing: Vec<usize>,
}

impl<const N: usize> Waiting<'_, N> {
    /// Under the hold, replaces the region of the worktree `name`, the main worktree's for
    /// `None`, with `region`, its markers included, as that worktree's own command replaces
    /// it under the lock; then, every command standing still and the file as written for
    /// `GRACE` more, lets go, and returns what each command printed once it has ended, as
    /// `Watched::wait` checks it. Fails unless each went on with a public `ls-files`, the
    /// listing that follows the replacement, and the file still holds `region`, which a
    /// replacement composed from the file as read before the change would have lost (G28).
    /// A command waiting alone has every run from then on judged, and found free (R10);
    /// commands waiting together are judged once the others have ended.
    pub fn let_go(self, name: Option<&OsStr>, region: &[u8]) -> [Output; N] {
        let Waiting {
            held,
            mut commands,
            standing,
        } = self;
        let path = held.watch.exclude();
        let other = region_in(&held.watch.common, name)
            .unwrap_or_else(|| panic!("no region of {name:?} in {}", path.display()));
        let changed = [&other.before[..], region, &other.after].concat();
        fs::write(&path, &changed).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
        standing_at_the_lock(&mut commands, &path, &changed, Some(&standing));
        let alone = N == 1;
        drop(held);
        let finished = commands.map(Watched::finished);
        for (finished, at) in finished.iter().zip(standing) {
            let after = &finished.events[at..];
            let next = after.iter().find_map(|event| match event {
                Event::Started { run, .. } => Some((*run, event)),
                Event::Unanswered { .. } => None,
            });
            let Some((next, _)) = next.filter(|(_, event)| event.listing(false)) else {
                panic!(
                    "{} did not go on with its public listing once the lock was let go: \
                     {after:?}; {:?}",
                    finished.named, finished.output
                );
            };
            let judged = &finished.verdicts[next - 1..];
            assert!(
                !alone || judged.iter().all(|verdict| *verdict == Verdict::Free),
                "{}: its runs after the lock were not all found free: {judged:?}",
                finished.named
            );
        }
        let found = fs::read(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
        assert!(
            found.windows(region.len()).any(|window| window == region),
            "{} lost the region written while the scenario held the lock: \"{}\"",
            path.display(),
            found.escape_ascii()
        );
        finished.map(|finished| finished.output)
    }
}

/// Waits, `LIMIT` at most, until every command has stood at the lock for `GRACE` together,
/// the exclude file at `path` holding `expected` throughout, and returns how many events
/// each command's log held there. Where `at` gives those counts already, every command must
/// stand at them on every look.
fn standing_at_the_lock(
    commands: &mut [Watched],
    path: &Path,
    expected: &[u8],
    at: Option<&[usize]>,
) -> Vec<usize> {
    let deadline = Instant::now() + LIMIT;
    let mut since = None;
    loop {
        let found = fs::read(path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
        assert!(
            found == expected,
            "{} changed while the scenario held the lock: \"{}\", not \"{}\"",
            path.display(),
            found.escape_ascii(),
            expected.escape_ascii()
        );
        let standing: Option<Vec<usize>> = commands
            .iter_mut()
            .map(Watched::stands_at_the_lock)
            .collect();
        let logs = || {
            commands
                .iter()
                .map(|command| format!("{}: {}", command.named, command.runs.log()))
                .collect::<Vec<_>>()
                .join("; ")
        };
        if let Some(at) = at {
            assert!(
                standing.as_deref() == Some(at),
                "a command moved while the scenario held the lock: {}",
                logs()
            );
        }
        match standing {
            Some(standing) => {
                if since.get_or_insert_with(Instant::now).elapsed() >= GRACE {
                    return standing;
                }
            }
            None => since = None,
        }
        assert!(
            Instant::now() < deadline,
            "not every command stood at the lock for {GRACE:?} within {LIMIT:?}: {}",
            logs()
        );
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use std::any::Any;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::super::output::End;
    use super::*;

    /// An attached project in the scenario's directory, and its common Git directory.
    fn attached(s: &Scenario) -> (PathBuf, PathBuf) {
        let dir = s.dir().join("project");
        s.attached_project(&dir);
        let common = dir.join(".git");
        (dir, common)
    }

    /// What a failed check said.
    fn said(failed: Box<dyn Any + Send>) -> String {
        match failed.downcast::<String>() {
            Ok(message) => *message,
            Err(failed) => failed
                .downcast_ref::<&str>()
                .map(|message| message.to_string())
                .unwrap_or_default(),
        }
    }

    /// Waits, `LIMIT` at most, until the log of `command` holds an event `found` accepts,
    /// and returns it.
    fn logged(command: &Watched, found: impl Fn(&Event) -> bool) -> Event {
        let deadline = Instant::now() + LIMIT;
        loop {
            if let Some(event) = command.runs.events().into_iter().find(&found) {
                return event;
            }
            assert!(
                Instant::now() < deadline,
                "{}: {}",
                command.named,
                command.runs.log()
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// A run started while another descriptor holds the lock is found holding it, and the
    /// command's check fails naming that run: the harness's own hold, taken outside the
    /// watch, stands for git-dupe's, which is how a git-dupe starting a run under its lock
    /// is caught (R10).
    #[test]
    fn a_run_started_while_the_lock_is_held_fails_the_check_naming_it() {
        let s = Scenario::of_the_first_release();
        let (dir, common) = attached(&s);
        let watch = s.watching_the_lock("watch", &common);
        let held = hold_the_lock(&common);
        let status = watch.start(&dir, &["status"]);
        // The first run has its verdict before its answer, so once the second has started.
        logged(&status, |event| {
            matches!(event, Event::Started { run: 2, .. })
        });
        drop(held);
        let Err(failed) = catch_unwind(AssertUnwindSafe(|| status.wait())) else {
            panic!("a run started while the lock was held passed the check");
        };
        let said = said(failed);
        assert!(
            said.contains("started Git while it held the lock") && said.contains("rev-parse"),
            "{said}"
        );
    }

    /// A run never answered gives up within its bound, and the check fails naming it
    /// instead of hanging; the answers an earlier command of the same watch was given
    /// answer none of a later command's runs, each command having a control directory of
    /// its own.
    #[test]
    fn a_run_left_unanswered_gives_up_and_fails_the_check_naming_it() {
        let s = Scenario::of_the_first_release();
        let (dir, common) = attached(&s);
        let watch = s.watching_the_lock("watch", &common);
        let answered = watch.run(&dir, &["status"]);
        assert_eq!(answered.end, End::Code(0), "{answered:?}");
        watch.withholding_answers(100);
        let began = Instant::now();
        let Err(failed) = catch_unwind(AssertUnwindSafe(|| watch.run(&dir, &["status"]))) else {
            panic!("a command whose runs had no answer passed the check");
        };
        let said = said(failed);
        assert!(
            said.contains("had no answer within its bound") && said.contains("rev-parse"),
            "{said}"
        );
        assert!(began.elapsed() < LIMIT, "it took {:?}", began.elapsed());
    }

    /// Two commands of one watch running at once, with no hold of the scenario's, may each
    /// hold the lock in its replacement as the other starts a run: neither is blamed for
    /// the other's hold, however long the replacements take.
    #[test]
    fn two_commands_at_once_are_not_blamed_for_each_others_lock() {
        let s = Scenario::of_the_first_release();
        let (dir, common) = attached(&s);
        let other = s.dir().join("other");
        s.linked_worktree(&dir, &other);
        s.init(&other);
        // A long user's part makes each replacement long enough to overlap the other's runs.
        let exclude = common.join("info/exclude");
        let comment: Vec<u8> = (0..300_000)
            .flat_map(|n| format!("# the user's line {n}\n").into_bytes())
            .collect();
        fs::write(&exclude, [comment, fs::read(&exclude).unwrap()].concat()).unwrap();
        let watch = s.watching_the_lock("watch", &common);
        for n in 0..5 {
            let (main, linked) = (format!("main-{n}"), format!("linked-{n}"));
            let hiding = [
                watch.start(&dir, &["hide", &main]),
                watch.start(&other, &["hide", &linked]),
            ];
            for hidden in hiding.map(Watched::wait) {
                assert_eq!(hidden.end, End::Code(0), "{hidden:?}");
            }
        }
    }

    /// A watched command dropped before it was waited for, as a failing scenario drops it,
    /// leaves no script waiting for an answer.
    #[test]
    fn a_watched_command_dropped_unwaited_leaves_no_script_waiting() {
        let s = Scenario::of_the_first_release();
        let (dir, common) = attached(&s);
        let watch = s.watching_the_lock("watch", &common);
        watch.withholding_answers(LOOKS);
        let status = watch.start(&dir, &["status"]);
        let Event::Started { process, .. } =
            logged(&status, |event| matches!(event, Event::Started { .. }))
        else {
            unreachable!("a start was looked for");
        };
        let began = Instant::now();
        drop(status);
        while !ended(&process) {
            assert!(
                began.elapsed() < LIMIT,
                "the script waiting for its answer still runs"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
}
