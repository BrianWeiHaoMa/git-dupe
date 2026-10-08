//! The lock on the common Git directory under which every worktree's command replaces
//! `.git/info/exclude` and deletes its region (`Holds/G28`), as a scenario takes and
//! watches it: held as another worktree's command holds it, found free, found waited for
//! by `git-dupe` processes in `/proc/locks`, and found never held by a `git-dupe` that
//! starts a Git run (R10).
//!
//! The last is seen from each Git run git-dupe starts, with nothing added to the product
//! (N6), as the kill harness sees them (`kill.rs`): with `GIT_EXEC_PATH` naming a control
//! directory that holds only the script `git` below, every Git run git-dupe starts is that
//! script, whose parent is git-dupe. Before it runs the release's own `git`, the script
//! reads `/proc/locks` for a lock its parent holds on the common Git directory.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::ErrorKind;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use super::output::Output;
use super::scenario::Scenario;

/// How long commands are given to be seen waiting for the lock.
const LIMIT: Duration = Duration::from_secs(30);

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

/// Waits, 30 seconds at most, until `count` `git-dupe` processes wait for the lock on
/// `common`: blocked requests in `/proc/locks`, each `-> FLOCK` followed by the process
/// and the directory's device and inode.
pub fn commands_wait_for_the_lock(common: &Path, count: usize) {
    let inode = fs::metadata(common).unwrap().ino().to_string();
    let deadline = Instant::now() + LIMIT;
    loop {
        let locks = fs::read_to_string("/proc/locks").unwrap();
        let waiting = locks
            .lines()
            .filter(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                fields.get(1..3) == Some(&["->", "FLOCK"])
                    && fields
                        .get(6)
                        .and_then(|file| file.rsplit(':').next())
                        .is_some_and(|number| number == inode)
                    && fields.get(5).is_some_and(|process| {
                        fs::read_to_string(format!("/proc/{process}/comm"))
                            .is_ok_and(|name| name.trim_end() == "git-dupe")
                    })
            })
            .count();
        if waiting >= count {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{waiting} git-dupe, not {count}, waited for the lock on {}: {locks}",
            common.display()
        );
        thread::sleep(Duration::from_millis(5));
    }
}

/// The `git` that git-dupe finds while its runs are watched. Its files beside it:
/// `release`, the path of the release's `git`; `inode`, the common Git directory's;
/// `parents`, the name of each run's parent; `under-lock`, the words of each run whose
/// parent held the lock on that directory as it started.
const WATCHING: &str = r#"#!/bin/sh
control=${0%/*}
read -r release < "$control/release"
read -r inode < "$control/inode"
read -r parent < "/proc/$PPID/comm"
printf '%s\n' "$parent" >> "$control/parents"
while read -r _ kind _ _ holder file _; do
    if [ "$kind" = FLOCK ] && [ "$holder" = "$PPID" ] && [ "${file##*:}" = "$inode" ]; then
        printf '%s\n' "$*" >> "$control/under-lock"
    fi
done < /proc/locks
unset GIT_EXEC_PATH
exec "$release" "$@"
"#;

/// `git dupe` commands whose every Git run is watched for the lock on one common Git
/// directory.
pub struct LockWatch<'s> {
    scenario: &'s Scenario,
    control: PathBuf,
}

impl Scenario {
    /// A watch of the lock on the common Git directory `common`, its control directory
    /// `name` below the scenario's directory and outside every workspace.
    pub fn watching_the_lock(&self, name: &str, common: &Path) -> LockWatch<'_> {
        let control = self.dir().join(name);
        fs::create_dir(&control).unwrap_or_else(|cause| panic!("{}: {cause}", control.display()));
        // Written by a process of its own, as `kill.rs` writes its script and for its
        // reason: no descriptor of this process open to write it can reach a child.
        let script = control.join("git");
        self.program(
            "/bin/sh",
            [
                OsStr::new("-c"),
                OsStr::new(r#"cat > "$0" && chmod 755 "$0""#),
                script.as_os_str(),
            ],
        )
        .input(WATCHING.as_bytes())
        .succeeds();
        let release = self.release_git().as_os_str().as_encoded_bytes();
        fs::write(control.join("release"), [release, b"\n"].concat()).unwrap();
        let inode = fs::metadata(common).unwrap().ino();
        fs::write(control.join("inode"), format!("{inode}\n")).unwrap();
        LockWatch {
            scenario: self,
            control,
        }
    }
}

impl LockWatch<'_> {
    /// Runs `git dupe <words>` from `dir` and returns what it printed. Fails unless its
    /// Git runs all came through the script, none of them started while git-dupe held
    /// the lock.
    pub fn run(&self, dir: &Path, words: &[&str]) -> Output {
        for record in ["parents", "under-lock"] {
            match fs::remove_file(self.control.join(record)) {
                Ok(()) => {}
                Err(cause) if cause.kind() == ErrorKind::NotFound => {}
                Err(cause) => panic!("{}: {cause}", self.control.display()),
            }
        }
        let words: Vec<OsString> = ["dupe"].iter().chain(words).map(OsString::from).collect();
        let output = self
            .scenario
            .git(&words)
            .from(dir)
            .variable("GIT_EXEC_PATH", &self.control)
            .run();
        let parents = fs::read_to_string(self.control.join("parents")).unwrap_or_default();
        assert!(
            parents.lines().count() > 0 && parents.lines().all(|parent| parent == "git-dupe"),
            "git-dupe's Git runs did not all come through the script: {parents:?}; {output:?}"
        );
        let under_lock = fs::read_to_string(self.control.join("under-lock")).ok();
        assert!(
            under_lock.is_none(),
            "git {words:?} started Git while it held the lock: {under_lock:?}; {output:?}"
        );
        output
    }
}
