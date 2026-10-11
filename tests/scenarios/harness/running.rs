//! A run started and not yet waited for: what `Git::start` returns, for a scenario that
//! acts while the run goes on, and what `Git::run` waits for at once.
//!
//! From its start, a thread feeds the run's standard input, when it has any, and one
//! collects each captured stream, so that a run printing more than a pipe holds while it
//! reads its input never stalls the scenario, and an observation made before the run ends
//! loses nothing it prints. A run in a process group of its own is waited for until no
//! process of that group is left. A handle dropped before its run was collected whole, by
//! a scenario that failed while the run went on or by a failed check of the run itself,
//! kills what is left of the run, its group where it has one, and reaps it, so that no
//! process of a failed scenario goes on or holds up the check.
//!
//! Processes are observed through `ps`, which answers alike on Linux and macOS for the
//! columns read here, never through Linux's process filesystem, which macOS lacks.
//! `ps -g` is not used: it selects a group on macOS and a session on Linux.

use std::io::{self, Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, ExitStatus};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::output::{End, Output};

pub struct Running {
    /// The program and its words, for a message.
    named: String,
    child: Child,
    /// The process group the run leads, when it has one of its own.
    group: Option<u32>,
    /// Whether `child` has been reaped: from then on only the other processes of its
    /// group keep its process ID from naming another.
    reaped: bool,
    feeder: Option<JoinHandle<io::Result<()>>>,
    stdout: Option<JoinHandle<io::Result<Vec<u8>>>>,
    stderr: Option<JoinHandle<io::Result<Vec<u8>>>>,
    /// Whether the run was collected whole: ended, read, fed, and accounted for.
    collected: bool,
}

impl Running {
    /// Starts `command`, built with its streams, in a process group of its own when
    /// `grouped`, and feeds it `input` when there is some.
    pub(super) fn start(
        mut command: Command,
        input: Option<Vec<u8>>,
        named: String,
        grouped: bool,
    ) -> Running {
        if grouped {
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .unwrap_or_else(|cause| panic!("cannot run {named}: {cause}"));
        let feeder = input.map(|bytes| {
            let mut stdin = child.stdin.take().expect("piped standard input");
            thread::spawn(move || stdin.write_all(&bytes))
        });
        let stdout = child.stdout.take().map(collecting);
        let stderr = child.stderr.take().map(collecting);
        Running {
            named,
            group: grouped.then(|| child.id()),
            child,
            reaped: false,
            feeder,
            stdout,
            stderr,
            collected: false,
        }
    }

    /// Whether the run has ended, without waiting for it. What it printed is kept for
    /// `wait`.
    pub fn finished(&mut self) -> bool {
        let ended = self
            .child
            .try_wait()
            .unwrap_or_else(|cause| panic!("cannot look at {}: {cause}", self.named));
        self.reaped |= ended.is_some();
        ended.is_some()
    }

    /// Waits for the run to end, then for what it printed and, in a group of its own,
    /// until no process of the group is left.
    pub fn wait(mut self) -> Output {
        let status = self
            .child
            .wait()
            .unwrap_or_else(|cause| panic!("cannot wait for {}: {cause}", self.named));
        self.reaped = true;
        self.collect(status, None)
    }

    /// Waits as `wait` does, `limit` at most for the run to end and for what it started to
    /// let its streams go: past that, the scenario fails, and the run is killed and reaped.
    pub fn wait_within(mut self, limit: Duration) -> Output {
        let deadline = Instant::now() + limit;
        loop {
            let ended = self
                .child
                .try_wait()
                .unwrap_or_else(|cause| panic!("cannot look at {}: {cause}", self.named));
            if let Some(status) = ended {
                self.reaped = true;
                return self.collect(status, Some((deadline, limit)));
            }
            assert!(
                Instant::now() < deadline,
                "{} did not end within {limit:?}",
                self.named
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// What the run printed and how it ended, once its threads have ended, by `deadline`
    /// where there is one, and its group, where it has one, has no process left.
    fn collect(&mut self, status: ExitStatus, deadline: Option<(Instant, Duration)>) -> Output {
        if let Some((deadline, limit)) = deadline {
            while !self.threads_ended() {
                assert!(
                    Instant::now() < deadline,
                    "{} ended, and what it started still held its streams after {limit:?}",
                    self.named
                );
                thread::sleep(Duration::from_millis(5));
            }
        }
        let stdout = collected(self.stdout.take(), &self.named);
        let stderr = collected(self.stderr.take(), &self.named);
        if let Some(feeder) = self.feeder.take() {
            feeder
                .join()
                .expect("the thread feeding the run")
                .unwrap_or_else(|cause| panic!("cannot feed {}: {cause}", self.named));
        }
        if let Some(group) = self.group {
            no_process_left(group, &self.named);
        }
        self.collected = true;
        let end = match status.signal() {
            Some(signal) => End::Signal(signal),
            None => End::Code(status.code().expect("an exit code where no signal")),
        };
        Output {
            stdout,
            stderr,
            end,
        }
    }

    /// Whether the threads feeding and collecting the run have ended.
    fn threads_ended(&self) -> bool {
        [self.stdout.as_ref(), self.stderr.as_ref()]
            .into_iter()
            .flatten()
            .all(JoinHandle::is_finished)
            && self.feeder.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for Running {
    /// A run not collected whole is killed, what is left of its group with it, and reaped.
    /// Its threads end once the pipes close; one still blocked ten seconds later is left
    /// behind, so that dropping the handle never hangs.
    fn drop(&mut self) {
        if self.collected {
            return;
        }
        match self.group {
            // The group's ID is the leader's: while the leader is not reaped, or another
            // process of the group runs, no other process can be given it.
            Some(group) if !self.reaped || still_running(group, &self.named) => {
                let _ = Command::new("/bin/sh")
                    .args(["-c", r#"kill -s KILL -- "-$0" 2>/dev/null"#])
                    .arg(group.to_string())
                    .status();
            }
            Some(_) => {}
            // A child already reaped is not signalled.
            None => {
                let _ = self.child.kill();
            }
        }
        let _ = self.child.wait();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.threads_ended() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        if self.threads_ended() {
            for stream in [self.stdout.take(), self.stderr.take()]
                .into_iter()
                .flatten()
            {
                let _ = stream.join();
            }
            if let Some(feeder) = self.feeder.take() {
                let _ = feeder.join();
            }
        }
    }
}

/// A thread reading `stream` to its end.
fn collecting(mut stream: impl Read + Send + 'static) -> JoinHandle<io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).map(|_| bytes)
    })
}

/// What a collecting thread read; nothing where the stream was not captured.
fn collected(stream: Option<JoinHandle<io::Result<Vec<u8>>>>, named: &str) -> Vec<u8> {
    let Some(stream) = stream else {
        return Vec::new();
    };
    stream
        .join()
        .expect("the thread collecting the run's output")
        .unwrap_or_else(|cause| panic!("cannot read what {named} printed: {cause}"))
}

/// Waits, ten seconds at most, until no process of the process group `group` is running.
/// A zombie has ended and is not counted: who reaps an orphan is the machine's.
fn no_process_left(group: u32, named: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let running = running_in(group).unwrap_or_else(|cause| panic!("{named}: {cause}"));
        if running.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{named} left processes running: {running:?}"
        );
        // Each look is a run of `ps` over every process of the machine.
        thread::sleep(Duration::from_millis(20));
    }
}

/// Whether a process of the group `group` is still running. Where `ps` cannot say, the
/// answer is no and the cause is printed: the group's ID may name another's by now, and a
/// panic here would abort a check that is already failing.
fn still_running(group: u32, named: &str) -> bool {
    match running_in(group) {
        Ok(running) => !running.is_empty(),
        Err(cause) => {
            eprintln!("{named}: {cause}");
            false
        }
    }
}

/// The processes of the group `group` that have not ended, each as its row of
/// `ps -A -o pid=,stat=,pgid=,comm=`: the process, its state, its group, and its name.
fn running_in(group: u32) -> Result<Vec<String>, String> {
    let rows = ps(&["-A", "-o", "pid=,stat=,pgid=,comm="])?;
    Ok(rows
        .lines()
        .filter(|row| {
            let mut columns = row.split_whitespace();
            let (_, state, of) = (columns.next(), columns.next(), columns.next());
            of == Some(group.to_string().as_str()) && state.is_some_and(|state| !ended_in(state))
        })
        .map(|row| row.trim().to_owned())
        .collect())
}

/// Whether the process `process` has ended: gone, or a zombie.
#[cfg(test)]
pub(super) fn ended(process: &str) -> bool {
    let state = ps(&["-o", "stat=", "-p", process]).unwrap_or_else(|cause| panic!("{cause}"));
    state.trim().is_empty() || ended_in(state.trim())
}

/// The lines by which a `git` script the harness puts in `GIT_EXEC_PATH` names the process
/// that ran it, as `$parent`: what `ps` prints as its name, to its last path component,
/// because Linux prints the kernel's name for it and macOS the path it was started by. A
/// script splices them in with `concat!`, so that every script reads the name alike.
macro_rules! parent_name {
    () => {
        r#"parent=$(ps -o comm= -p "$PPID")
parent=${parent##*/}
"#
    };
}
pub(super) use parent_name;

/// Whether a state `ps` prints is a zombie's, whatever letters follow its first: `Z` on
/// Linux, `Z`, `Z+`, or `Zs` on macOS.
fn ended_in(state: &str) -> bool {
    state.starts_with('Z')
}

/// What `ps <words>` printed. `ps` exits 1 and prints nothing on either stream when the
/// processes it was asked about are gone, which is an answer; any other status, and
/// anything on standard error, is a failure, never an empty answer.
fn ps(words: &[&str]) -> Result<String, String> {
    let ps = Command::new("ps")
        .args(words)
        .output()
        .map_err(|cause| format!("cannot run ps {words:?}: {cause}"))?;
    let printed = String::from_utf8_lossy(&ps.stdout).into_owned();
    match (ps.status.code(), ps.stderr.is_empty()) {
        (Some(0), true) => Ok(printed),
        (Some(1), true) if printed.trim().is_empty() => Ok(printed),
        _ => Err(format!(
            "ps {words:?} {}: {}",
            ps.status,
            String::from_utf8_lossy(&ps.stderr)
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::Path;
    use std::sync::mpsc;

    use super::super::scenario::Scenario;
    use super::*;

    /// How long the check below gives a start, then a wait.
    const LIMIT: Duration = Duration::from_secs(20);

    /// A program fed more than a pipe holds, printing it back as it reads it, can neither
    /// be fed whole before it prints nor print whole before it is fed: the start returns at
    /// once, and the wait returns every byte it printed. Where either would stall, the
    /// watchdog kills the program, which recorded its process ID before it became `cat`,
    /// and the check fails instead of hanging the suite.
    #[test]
    fn a_started_run_is_fed_while_what_it_prints_is_collected() {
        let s = Scenario::of_the_first_release();
        let input: Vec<u8> = (0..1u32 << 20).map(|n| (n % 251) as u8).collect();
        let recorded = s.dir().join("cat-process");
        let (started, start_returned) = mpsc::channel();
        let (waited, wait_returned) = mpsc::channel();
        let stalled = thread::scope(|scope| {
            let check = scope.spawn(|| {
                let running = s
                    .program(
                        "/bin/sh",
                        [
                            "-c",
                            r#"echo $$ > "$0" && exec cat"#,
                            recorded.to_str().unwrap(),
                        ],
                    )
                    .input(&input)
                    .start();
                let _ = started.send(());
                let _ = waited.send(running.wait());
            });
            let stalled = match start_returned.recv_timeout(LIMIT) {
                Err(_) => Some("start"),
                Ok(()) => match wait_returned.recv_timeout(LIMIT) {
                    Err(_) => Some("wait"),
                    Ok(output) => {
                        assert_eq!(output.end, End::Code(0));
                        assert!(output.stdout == input, "cat printed back other bytes");
                        None
                    }
                },
            };
            if stalled.is_some() {
                kill_recorded(&recorded);
            }
            // Whatever the killed run's thread then makes of it, the stall is the failure.
            let _ = check.join();
            stalled
        });
        if let Some(stalled) = stalled {
            panic!("the run stalled in its {stalled}, and was killed");
        }
    }

    /// A run that leaves a process behind fails its wait, the bounded one before the
    /// process lets the run's streams go, the other once its group is still not empty; and
    /// either way the process is killed, so that nothing of a failed check goes on.
    #[test]
    fn a_run_that_leaves_a_process_behind_fails_and_leaves_none() {
        let s = Scenario::of_the_first_release();
        for (holding, bounded) in [("", true), (" >/dev/null 2>&1", false)] {
            let left = s.dir().join("left");
            let script = format!(r#"sleep 120{holding} & echo $! > "$0"; exit 0"#);
            let running = s
                .program("/bin/sh", ["-c", &script, left.to_str().unwrap()])
                .start();
            let began = Instant::now();
            let waited = catch_unwind(AssertUnwindSafe(|| match bounded {
                true => running.wait_within(Duration::from_millis(500)),
                false => running.wait(),
            }));
            assert!(
                waited.is_err(),
                "the run left a process and its wait passed"
            );
            assert!(
                began.elapsed() < LIMIT,
                "the wait took {:?}",
                began.elapsed()
            );
            let process = fs::read_to_string(&left).unwrap();
            let deadline = Instant::now() + LIMIT;
            while !ended(process.trim_end()) {
                assert!(
                    Instant::now() < deadline,
                    "the process left behind still runs"
                );
                thread::sleep(Duration::from_millis(5));
            }
            fs::remove_file(&left).unwrap();
        }
    }

    /// `ps` refusing what it was asked is no answer: a process it says nothing about is
    /// gone only where it says nothing on either stream, as for a process that has ended.
    #[test]
    fn a_failing_ps_is_no_answer_about_a_process() {
        assert!(ps(&["-o", "stat=", "-p", "not-a-process"]).is_err());
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let process = child.id().to_string();
        child.wait().unwrap();
        assert_eq!(ps(&["-o", "stat=", "-p", &process]).as_deref(), Ok(""));
        assert!(ended(&process));
    }

    /// Kills the process whose ID the stalled run recorded in `file`: it is still at work,
    /// so the ID names it and nothing else.
    fn kill_recorded(file: &Path) {
        if let Ok(process) = fs::read_to_string(file) {
            let _ = Command::new("/bin/sh")
                .args(["-c", r#"kill -s KILL "$0""#])
                .arg(process.trim_end())
                .status();
        }
    }
}
