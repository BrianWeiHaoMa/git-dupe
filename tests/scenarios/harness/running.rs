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

use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
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
            Some(group) if !self.reaped || !running_in(group).is_empty() => {
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
        let running = running_in(group);
        if running.is_empty() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{named} left processes running: {running:?}"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

/// The processes of the group `group` that have not ended, each as its `/proc` status
/// line, read from `/proc/<pid>/stat`: the name in parentheses, then the state, the
/// parent, and the process group.
fn running_in(group: u32) -> Vec<String> {
    let mut running = Vec::new();
    for entry in fs::read_dir("/proc").expect("/proc") {
        let Ok(entry) = entry else { continue };
        if !entry.file_name().as_bytes().iter().all(u8::is_ascii_digit) {
            continue;
        }
        // A process that ends meanwhile leaves nothing to read.
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((_, after_name)) = stat.rsplit_once(") ") else {
            continue;
        };
        let fields: Vec<&str> = after_name.split(' ').collect();
        if fields.get(2) == Some(&group.to_string().as_str()) && fields[0] != "Z" {
            running.push(stat.trim_end().to_owned());
        }
    }
    running
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::mpsc;

    use super::super::scenario::Scenario;
    use super::*;

    /// How long the check below gives a start, then a wait.
    const LIMIT: Duration = Duration::from_secs(20);

    /// A variable naming the run the check's watchdog may kill.
    const WATCHED: &str = "HARNESS_WATCHED_RUN";

    /// A program fed more than a pipe holds, printing it back as it reads it, can neither
    /// be fed whole before it prints nor print whole before it is fed: the start returns at
    /// once, and the wait returns every byte it printed. Where either would stall, the
    /// watchdog kills the program and the check fails instead of hanging the suite.
    #[test]
    fn a_started_run_is_fed_while_what_it_prints_is_collected() {
        let s = Scenario::of_the_first_release();
        let input: Vec<u8> = (0..1u32 << 20).map(|n| (n % 251) as u8).collect();
        let mark = format!("{}-{:?}", std::process::id(), thread::current().id());
        let (started, start_returned) = mpsc::channel();
        let (waited, wait_returned) = mpsc::channel();
        let stalled = thread::scope(|scope| {
            let check = scope.spawn(|| {
                let running = s
                    .program("cat", [] as [&str; 0])
                    .variable(WATCHED, &mark)
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
                kill_marked(&mark);
            }
            // Whatever the killed run's thread then makes of it, the stall is the failure.
            let _ = check.join();
            stalled
        });
        if let Some(stalled) = stalled {
            panic!("the run stalled in its {stalled}, and was killed");
        }
    }

    /// Whether the process `process` has ended: gone, or a zombie.
    fn ended(process: &str) -> bool {
        fs::read_to_string(format!("/proc/{process}/stat")).map_or(true, |stat| {
            stat.rsplit_once(") ")
                .is_some_and(|(_, after)| after.starts_with('Z'))
        })
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

    /// Kills each child of this process whose environment holds `WATCHED=<mark>`.
    fn kill_marked(mark: &str) {
        let wanted = format!("{WATCHED}={mark}");
        let parent = std::process::id().to_string();
        for entry in fs::read_dir("/proc").expect("/proc").flatten() {
            let path = entry.path();
            let Ok(stat) = fs::read_to_string(path.join("stat")) else {
                continue;
            };
            let child = stat
                .rsplit_once(") ")
                .is_some_and(|(_, after)| after.split(' ').nth(1) == Some(parent.as_str()));
            let marked = fs::read(path.join("environ")).is_ok_and(|environment| {
                environment
                    .split(|&byte| byte == 0)
                    .any(|variable| variable == wanted.as_bytes())
            });
            if child && marked {
                let _ = Command::new("/bin/sh")
                    .args(["-c", r#"kill -s KILL "$0""#])
                    .arg(entry.file_name())
                    .status();
            }
        }
    }
}
