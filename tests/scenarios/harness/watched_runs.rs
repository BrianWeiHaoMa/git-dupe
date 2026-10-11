//! The Git runs of one watched `git dupe` command, each recorded as it starts and answered
//! with a request for the lock on the common Git directory before it goes on
//! (`exclude_lock.rs`), with nothing added to the product (N6).
//!
//! As in the kill harness (`kill.rs`), `GIT_EXEC_PATH` names a control directory holding
//! the script `git` below, so every Git run git-dupe starts is that script, whose parent is
//! git-dupe. The script appends each run's start to the directory's log, waits for the
//! harness's answer to it, and becomes the release's own `git`, so that how Git's run ends,
//! an exit status or a signal, reaches git-dupe as it would without the script; a run's
//! end is read from `ps`. A thread of the harness reads the log and answers each run
//! started: first one `try_lock` on the common Git directory from a descriptor of its own,
//! let go at once (S16), then the file the run waits for. git-dupe waits for the script, so
//! the request finds the lock as git-dupe held it when it started the run; one it took and
//! let go between starting the run and the script's first step is not seen. A lock found
//! taken is git-dupe's only where nothing else can hold it (`Holders`); elsewhere the run
//! is answered without a verdict.
//!
//! A run's words go to a file of their own, never into the log, so that no word, one
//! holding a newline included, can read as a record there.

use std::fs::{self, File, TryLockError};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::executable::write_executable;
use super::running::parent_name;
use super::scenario::Scenario;

/// How long the harness waits between two readings of a log.
const READING: Duration = Duration::from_millis(2);

/// The `git` that git-dupe finds while its runs are watched. Its files beside it:
/// `release`, the path of the release's `git`; `looks`, how many times a run looks for its
/// answer, a hundredth of a second apart, before it gives up; `count`, the runs so far;
/// `words-<n>`, the words of run `n`; `log`, one line per event, each written by one
/// `printf`: `started <n> <process> <scope> <parent>` as run `n` starts, with the script's
/// process, which Git's becomes, `private` or `public` as `GIT_DIR` names the private
/// repository or not, and its parent's name; `unanswered <n>` where the run gave up; and
/// `answer-<n>`, the harness's answer to run `n`, which the run waits for before Git runs.
const WATCHING: &str = concat!(
    r#"#!/bin/sh
control=${0%/*}
read -r release < "$control/release"
read -r looks < "$control/looks"
n=1
if [ -f "$control/count" ]; then read -r n < "$control/count"; n=$((n + 1)); fi
printf '%s\n' "$n" > "$control/count"
"#,
    parent_name!(),
    r#"scope=public
case ${GIT_DIR-} in */dupe) scope=private ;; esac
printf '%s' "$*" > "$control/words-$n"
printf 'started %s %s %s %s\n' "$n" "$$" "$scope" "$parent" >> "$control/log"
looked=0
while [ ! -f "$control/answer-$n" ]; do
    if [ "$looked" -ge "$looks" ]; then
        printf 'unanswered %s\n' "$n" >> "$control/log"
        exit 125
    fi
    sleep 0.01
    looked=$((looked + 1))
done
unset GIT_EXEC_PATH
exec "$release" "$@"
"#
);

/// Who besides git-dupe may hold the lock on one common Git directory, for every command
/// watched there: the scenario while it holds the lock itself, and each command while it
/// runs, which may hold it in its own replacement. A request is judged only where neither
/// can, no hold and one command, and under the same mutex as every change of these, so
/// that no request falls between a change and its record.
#[derive(Default)]
pub(super) struct Holders(Mutex<Others>);

#[derive(Default)]
struct Others {
    scenario: bool,
    commands: usize,
}

impl Holders {
    /// Records that the scenario holds the lock, before it takes it.
    pub(super) fn scenario_takes(&self) {
        self.others().scenario = true;
    }

    /// Lets the scenario's lock go, by dropping `lock`, and records it in the same moment.
    pub(super) fn scenario_lets_go(&self, lock: File) {
        let mut others = self.others();
        drop(lock);
        others.scenario = false;
    }

    pub(super) fn scenario_holds(&self) -> bool {
        self.others().scenario
    }

    /// What one request for the lock on `common`, from a descriptor of the harness's own,
    /// finds, where nothing but git-dupe can hold it. The descriptor is closed before the
    /// verdict is returned, which lets the lock go where the request took it.
    fn asked(&self, common: &Path) -> Verdict {
        let others = self.others();
        if others.scenario || others.commands > 1 {
            return Verdict::Unjudged;
        }
        let directory = match File::open(common) {
            Ok(directory) => directory,
            Err(cause) => return Verdict::Failed(format!("{}: {cause}", common.display())),
        };
        match directory.try_lock() {
            Ok(()) => Verdict::Free,
            Err(TryLockError::WouldBlock) => Verdict::Held,
            Err(TryLockError::Error(cause)) => {
                Verdict::Failed(format!("{}: {cause}", common.display()))
            }
        }
    }

    fn others(&self) -> MutexGuard<'_, Others> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// One command counted among the holders until this is dropped.
struct Counted(Arc<Holders>);

impl Counted {
    fn of(holders: &Arc<Holders>) -> Counted {
        holders.others().commands += 1;
        Counted(Arc::clone(holders))
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.others().commands -= 1;
    }
}

/// The runs of one command, recorded in a control directory of its own, so that nothing an
/// earlier command left in another answers them, and answered by a thread stopped when
/// this is dropped. The command counts among the holders from before it starts until this
/// is dropped, which its owner does once nothing of the command runs.
pub(super) struct WatchedRuns {
    control: PathBuf,
    answering: Option<Answering>,
    _counted: Counted,
}

impl WatchedRuns {
    /// Makes the control directory `control`, which must not exist, and starts answering
    /// the runs it records with requests for the lock on `common`, judged as `holders`
    /// allows. Where not `answered`, for a check of the harness itself, nothing answers
    /// them, and each gives up once it has looked for its answer `looks` times.
    pub(super) fn through(
        s: &Scenario,
        control: PathBuf,
        common: &Path,
        holders: &Arc<Holders>,
        answered: bool,
        looks: u32,
    ) -> WatchedRuns {
        fs::create_dir(&control).unwrap_or_else(|cause| panic!("{}: {cause}", control.display()));
        write_executable(s, &control.join("git"), WATCHING.as_bytes());
        let release = s.release_git().as_os_str().as_encoded_bytes();
        fs::write(control.join("release"), [release, b"\n"].concat()).unwrap();
        fs::write(control.join("looks"), format!("{looks}\n")).unwrap();
        let counted = Counted::of(holders);
        let answering = answered.then(|| Answering::start(&control, common, holders));
        WatchedRuns {
            control,
            answering,
            _counted: counted,
        }
    }

    /// The directory for `GIT_EXEC_PATH`.
    pub(super) fn control(&self) -> &Path {
        &self.control
    }

    /// The events the log holds so far.
    pub(super) fn events(&self) -> Vec<Event> {
        events(&self.control)
    }

    /// The log as text, for a message.
    pub(super) fn log(&self) -> String {
        String::from_utf8_lossy(&log_bytes(&self.control)).into_owned()
    }

    /// For a command that has ended, `named` with what it printed: stops answering and
    /// returns the log's events and the verdict on each run answered, in order. Fails
    /// unless every Git run it started came through the script, had its answer, and did not
    /// start while git-dupe held the lock.
    pub(super) fn checked(self, named: &str) -> (Vec<Event>, Vec<Verdict>) {
        let log = self.log();
        let verdicts = self.answering.map(Answering::stopped).unwrap_or_default();
        let events = events(&self.control);
        let started: Vec<&Event> = events
            .iter()
            .filter(|event| matches!(event, Event::Started { .. }))
            .collect();
        let unanswered: Vec<&&Event> = events
            .iter()
            .filter_map(|event| match event {
                Event::Unanswered { run } => started.get(run - 1),
                Event::Started { .. } => None,
            })
            .collect();
        assert!(
            unanswered.is_empty(),
            "{named}: {unanswered:?} had no answer within its bound; {log}"
        );
        assert!(
            !started.is_empty()
                && started.iter().all(
                    |event| matches!(event, Event::Started { parent, .. } if parent == "git-dupe")
                ),
            "{named}: its Git runs did not all come through the script: {log}"
        );
        for (run, verdict) in started.iter().zip(&verdicts) {
            match verdict {
                Verdict::Free | Verdict::Unjudged => {}
                Verdict::Held => panic!("{named}: started Git while it held the lock: {run:?}"),
                Verdict::Failed(cause) => {
                    panic!("{named}: cannot ask for the lock as {run:?} starts: {cause}")
                }
            }
        }
        (events, verdicts)
    }
}

/// One event of a watched command's log.
#[derive(Debug)]
pub(super) enum Event {
    /// Run `run` started: its process, the script's and then Git's, whether it runs
    /// against the private repository, its parent's name, and its words.
    Started {
        run: usize,
        process: String,
        private: bool,
        parent: String,
        words: String,
    },
    Unanswered {
        run: usize,
    },
}

impl Event {
    /// Whether this is the start of an `ls-files` against the private repository, where
    /// `private`, or against the public one.
    pub(super) fn listing(&self, private: bool) -> bool {
        matches!(
            self,
            Event::Started { private: scope, words, .. }
                if *scope == private && words.starts_with("ls-files ")
        )
    }
}

/// What the harness found as a run started.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// The lock was free: git-dupe did not hold it.
    Free,
    /// The lock was taken, by git-dupe.
    Held,
    /// Not asked: something other than git-dupe may hold the lock.
    Unjudged,
    /// The request could not be made.
    Failed(String),
}

/// The events of the log in `control`, from its whole lines: a line the script is still
/// writing is read the next time.
fn events(control: &Path) -> Vec<Event> {
    let bytes = log_bytes(control);
    let whole = bytes
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |end| end + 1);
    String::from_utf8_lossy(&bytes[..whole])
        .lines()
        .map(|line| {
            event(control, line)
                .unwrap_or_else(|| panic!("{}: no event: {line}", control.display()))
        })
        .collect()
}

fn event(control: &Path, line: &str) -> Option<Event> {
    let mut fields = line.splitn(5, ' ');
    let kind = fields.next()?;
    let run = fields.next()?.parse().ok()?;
    match kind {
        "started" => Some(Event::Started {
            run,
            process: fields.next()?.to_owned(),
            private: fields.next()? == "private",
            parent: fields.next().unwrap_or_default().to_owned(),
            words: words(control, run),
        }),
        "unanswered" => Some(Event::Unanswered { run }),
        _ => None,
    }
}

/// The words of run `run`, which the script wrote before it recorded the run's start.
fn words(control: &Path, run: usize) -> String {
    let file = control.join(format!("words-{run}"));
    let words = fs::read(&file).unwrap_or_else(|cause| panic!("{}: {cause}", file.display()));
    String::from_utf8_lossy(&words).into_owned()
}

fn log_bytes(control: &Path) -> Vec<u8> {
    let log = control.join("log");
    match fs::read(&log) {
        Ok(bytes) => bytes,
        Err(cause) if cause.kind() == ErrorKind::NotFound => Vec::new(),
        Err(cause) => panic!("{}: {cause}", log.display()),
    }
}

/// The thread answering one command's runs, stopped when this is dropped.
struct Answering {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Vec<Verdict>>>,
}

impl Answering {
    fn start(control: &Path, common: &Path, holders: &Arc<Holders>) -> Answering {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (control, common) = (control.to_path_buf(), common.to_path_buf());
            let (holders, stop) = (Arc::clone(holders), Arc::clone(&stop));
            thread::spawn(move || answer(&control, &common, &holders, &stop))
        };
        Answering {
            stop,
            thread: Some(thread),
        }
    }

    /// Stops the thread and returns its verdict on each run it answered, in order.
    fn stopped(mut self) -> Vec<Verdict> {
        self.stop.store(true, Ordering::SeqCst);
        let thread = self.thread.take().expect("a thread not yet stopped");
        thread.join().expect("the thread answering the runs")
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Until `stop`, answers each run that the log in `control` records as started, in order:
/// its verdict, then the file it waits for.
fn answer(control: &Path, common: &Path, holders: &Holders, stop: &AtomicBool) -> Vec<Verdict> {
    let mut verdicts = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        for event in events(control) {
            if let Event::Started { run, .. } = event
                && run == verdicts.len() + 1
            {
                verdicts.push(holders.asked(common));
                let answer = control.join(format!("answer-{run}"));
                fs::write(&answer, b"")
                    .unwrap_or_else(|cause| panic!("{}: {cause}", answer.display()));
            }
        }
        thread::sleep(READING);
    }
    verdicts
}
