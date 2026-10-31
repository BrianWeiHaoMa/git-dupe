//! The stand-in `git` of a scenario's exec path on Windows, where `std::process::Command` finds
//! only a `.exe` and `sh` cannot name its native parent (S20, S21): one program for the two
//! scripts of `harness/kill.rs` and `harness/watched_runs.rs`, which are its protocol, every
//! step of theirs in their order, on the same files beside it. A `looks` file there makes it
//! the watching script. `rustc` compiles it alone, once per check run, so it stands in no
//! target and holds nothing of the harness.
//!
//! The scripts become Git; this runs Git as a child with the same words and streams and ends
//! with its status, so that `ended(<this process>)` turns true exactly when Git's run has ended.

use std::env;
use std::ffi::{OsString, c_void};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, exit};
use std::thread::sleep;
use std::time::Duration;

/// The status a script's own failure ends with.
const GAVE_UP: i32 = 125;
/// The status of a process killed by `SIGKILL` as a shell shows it, which `Git` hands on.
const KILLED: u32 = 137;
/// How long a pause and an answer are looked for apart.
const LOOK: Duration = Duration::from_millis(10);
/// How many times a pause looks for its release.
const PAUSE_LOOKS: u32 = 3000;

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const PROCESS_TERMINATE: u32 = 0x0001;
const SYNCHRONIZE: u32 = 0x0010_0000;
const TH32CS_SNAPPROCESS: u32 = 0x2;
const WAIT_OBJECT_0: u32 = 0;
/// How long a killed parent is given to end, in milliseconds.
const ENDING: u32 = 30_000;

type Handle = *mut c_void;

#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    process: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    class_priority: i32,
    flags: u32,
    executable: [u16; 260],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CloseHandle(handle: Handle) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process: u32) -> Handle;
    fn OpenProcess(access: u32, inherit: i32, process: u32) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry) -> i32;
    fn TerminateProcess(process: Handle, code: u32) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
}

/// Says what failed and ends as a script's `exit 125` does.
fn gave_up(what: &str) -> ! {
    eprintln!("the stand-in git: {what}");
    exit(GAVE_UP)
}

/// This process's parent: its identifier and its name without the executable suffix.
fn parent() -> (u32, String) {
    // SAFETY: the snapshot takes no pointer.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        gave_up("no process list");
    }
    // SAFETY: all zeroes is a valid entry, and each call below is given its size.
    let mut entry: ProcessEntry = unsafe { mem::zeroed() };
    entry.size = mem::size_of::<ProcessEntry>() as u32;
    let mut processes = Vec::new();
    // SAFETY: `entry` carries its size and the snapshot is open for each call.
    let mut more = unsafe { Process32FirstW(snapshot, &mut entry) };
    while more != 0 {
        let ends = entry.executable.iter().position(|unit| *unit == 0);
        let spelled = &entry.executable[..ends.unwrap_or(entry.executable.len())];
        processes.push((
            entry.process,
            entry.parent,
            String::from_utf16_lossy(spelled),
        ));
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot, &mut entry) };
    }
    // SAFETY: the snapshot is this function's to close.
    unsafe { CloseHandle(snapshot) };
    let myself = std::process::id();
    let Some((_, parent, _)) = processes.iter().find(|(process, ..)| *process == myself) else {
        gave_up("not in the process list");
    };
    let named = processes.iter().find(|(process, ..)| process == parent);
    let name = named.map_or("", |(_, _, name)| name.as_str());
    (
        *parent,
        name.strip_suffix(".exe").unwrap_or(name).to_owned(),
    )
}

/// Ends `process` with the status of a kill and waits for it to end, so that the process
/// takes no step after the kill however this one's own end falls.
fn kill(process: u32) {
    // SAFETY: the call takes no pointer.
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE | SYNCHRONIZE, 0, process) };
    if handle.is_null() {
        gave_up("cannot open the parent");
    }
    // SAFETY: the handle is open for each call, and this function's to close.
    let ended = unsafe {
        TerminateProcess(handle, KILLED) != 0
            && WaitForSingleObject(handle, ENDING) == WAIT_OBJECT_0
    };
    unsafe { CloseHandle(handle) };
    if !ended {
        gave_up("cannot kill the parent");
    }
}

/// The first line of the file `path`, where it stands.
fn line(path: &Path) -> Option<String> {
    let text = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&text);
    Some(text.lines().next().unwrap_or("").to_owned())
}

/// `line` of a file the protocol says is there.
fn needed(path: &Path) -> String {
    line(path).unwrap_or_else(|| gave_up(&format!("no {}", path.display())))
}

fn write(path: &Path, text: &str) {
    fs::write(path, text).unwrap_or_else(|_| gave_up(&format!("cannot write {}", path.display())));
}

/// One write at the end of `path`, as a script's `printf … >>` is, a reader seeing whole lines
/// or none of the last.
fn append(path: &Path, text: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|_| gave_up(&format!("cannot open {}", path.display())));
    file.write_all(text.as_bytes())
        .unwrap_or_else(|_| gave_up(&format!("cannot write {}", path.display())));
}

/// One more than the number `path` holds, written back: a count of the runs so far.
fn counted(path: &Path) -> u32 {
    let so_far = line(path).and_then(|count| count.parse().ok());
    let number = so_far.map_or(1, |count: u32| count + 1);
    write(path, &format!("{number}\n"));
    number
}

/// One Git run git-dupe started: the control directory, the words, and the release's `git`.
struct Run {
    control: PathBuf,
    words: Vec<OsString>,
    release: String,
}

impl Run {
    fn file(&self, name: &str) -> PathBuf {
        self.control.join(name)
    }

    /// The release's own `git` with the same words and streams, forgetting `GIT_EXEC_PATH`, so
    /// that Git's own children never come back through this program.
    fn git(&self) -> Command {
        let mut git = Command::new(&self.release);
        git.args(&self.words).env_remove("GIT_EXEC_PATH");
        git
    }

    /// Runs Git to its end.
    fn status(&self) -> ExitStatus {
        self.git()
            .status()
            .unwrap_or_else(|_| gave_up("cannot run the release's git"))
    }

    /// Runs Git and ends with its status.
    fn forwarded(&self) -> ! {
        exit(self.status().code().unwrap_or(GAVE_UP))
    }

    /// The words, a space between each, as a script's `$*`.
    fn words_joined(&self) -> String {
        let words = self.words.iter().map(|word| word.to_string_lossy());
        words.collect::<Vec<_>>().join(" ")
    }

    /// `harness/watched_runs.rs`'s script.
    fn watched(&self) -> ! {
        let looks: u32 = needed(&self.file("looks"))
            .parse()
            .unwrap_or_else(|_| gave_up("the looks are no number"));
        let number = counted(&self.file("count"));
        let (_, parent) = parent();
        fs::write(self.file(&format!("words-{number}")), self.words_joined())
            .unwrap_or_else(|_| gave_up("cannot write the words"));
        let process = std::process::id();
        let log = self.file("log");
        let scope = scope();
        append(
            &log,
            &format!("started {number} {process} {scope} {parent}\n"),
        );
        let mut looked = 0;
        while !self.file(&format!("answer-{number}")).is_file() {
            if looked >= looks {
                append(&log, &format!("unanswered {number}\n"));
                exit(GAVE_UP);
            }
            sleep(LOOK);
            looked += 1;
        }
        self.forwarded()
    }

    /// `harness/kill.rs`'s script.
    fn killable(&self) -> ! {
        let number = counted(&self.file("count"));
        let (parent, name) = parent();
        append(&self.file("parents"), &format!("{name}\n"));
        let point = line(&self.file("kill")).unwrap_or_default();
        if point == format!("before {number}") {
            self.killed(parent, &point, &name);
        }
        if point == format!("after {number}") {
            self.status();
            self.killed(parent, &point, &name);
        }
        if self.file("selection").is_file() {
            self.selected();
        }
        self.forwarded()
    }

    fn killed(&self, parent: u32, point: &str, name: &str) -> ! {
        write(&self.file("killed"), &format!("{point} {name}\n"));
        kill(parent);
        exit(0)
    }

    /// Applies the effect where this run is the selected one, and returns otherwise.
    fn selected(&self) {
        let selected = needed(&self.file("selection"));
        let words = self.words_joined();
        let matches = words == selected || words.starts_with(&format!("{selected} "));
        if needed(&self.file("scope")) != scope() || !matches {
            return;
        }
        let candidate = counted(&self.file("candidates"));
        let wanted = line(&self.file("occurrence")).and_then(|wanted| wanted.parse().ok());
        if candidate != wanted.unwrap_or(1) {
            self.forwarded();
        }
        if let Some(observed) = line(&self.file("observed-file")) {
            let equal = fs::read(observed).ok() == fs::read(self.file("expected")).ok();
            let said = if equal { "equal\n" } else { "different\n" };
            write(&self.file("observed"), said);
        }
        append(&self.file("matched"), "selected\n");
        let effect = needed(&self.file("effect"));
        if let Some(code) = effect.strip_prefix("exit ") {
            exit(code.parse().unwrap_or(GAVE_UP));
        }
        match effect.as_str() {
            "truncate" => self.truncated(),
            "pause" => self.paused(),
            _ => {}
        }
    }

    /// Prints Git's answer without its last byte, and ends with its status.
    fn truncated(&self) -> ! {
        let answer = self.file("answer");
        let file = File::create(&answer).unwrap_or_else(|_| gave_up("cannot write the answer"));
        let status = self
            .git()
            .stdout(file)
            .status()
            .unwrap_or_else(|_| gave_up("cannot run the release's git"));
        let printed = fs::read(&answer).unwrap_or_default();
        let Some((_, kept)) = printed.split_last() else {
            exit(GAVE_UP);
        };
        std::io::stdout()
            .write_all(kept)
            .unwrap_or_else(|_| gave_up("cannot print the answer"));
        exit(status.code().unwrap_or(GAVE_UP))
    }

    /// Waits until the run is released, for a bounded time.
    fn paused(&self) {
        let mut looked = 0;
        while !self.file("released").is_file() {
            if looked >= PAUSE_LOOKS {
                exit(GAVE_UP);
            }
            sleep(LOOK);
            looked += 1;
        }
    }
}

/// `private` where `GIT_DIR`'s last component is `dupe`, under either separator, because
/// git-dupe's is `C:/…/.git\dupe`; `public` otherwise.
fn scope() -> &'static str {
    let directory = env::var_os("GIT_DIR").unwrap_or_default();
    let directory = directory.to_string_lossy();
    match directory.rsplit(['/', '\\']).next() {
        Some("dupe") if directory.len() > "dupe".len() => "private",
        _ => "public",
    }
}

fn main() {
    let program = env::current_exe().unwrap_or_else(|_| gave_up("no path of its own"));
    let Some(control) = program.parent().map(Path::to_path_buf) else {
        gave_up("no directory of its own");
    };
    let release = needed(&control.join("release"));
    let run = Run {
        words: env::args_os().skip(1).collect(),
        release,
        control,
    };
    if run.file("looks").is_file() {
        run.watched();
    }
    run.killable()
}
