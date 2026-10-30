//! Whole-file replacement: the one way git-dupe writes a file of its own (G21, S1).
//!
//! The new content goes to a fresh file inside the private Git directory, is given the
//! original's permissions, and is renamed onto the destination, so that a kill leaves
//! the old file or the new one and never a partial one. A rename onto a symbolic link
//! replaces the link, not its target. A rename another process refuses for the moment is
//! tried again over a bounded time (S22, E7).

use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// The waits between the eight tries of a rename, 311 ms in all (S22).
const DELAYS: [Duration; 7] = [
    Duration::from_millis(0),
    Duration::from_millis(1),
    Duration::from_millis(10),
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(160),
];

/// How many names `fresh` tries before giving up.
const NAMES: u32 = 100;

/// Replaces `destination` with `content`, the fresh file given `permissions`, the
/// original's, when it had some, which on Windows are its read-only attribute alone, and
/// otherwise those a new file gets. On failure the destination is as it
/// was and the fresh file, when one was made, is removed again; the cause is returned for
/// the caller to report.
pub fn whole(
    private_directory: &Path,
    destination: &Path,
    content: &[u8],
    permissions: Option<Permissions>,
) -> io::Result<()> {
    let (fresh, mut file) = fresh(private_directory)?;
    let written = file.write_all(content).and_then(|()| match permissions {
        Some(permissions) => file.set_permissions(permissions),
        None => Ok(()),
    });
    drop(file);
    let replaced = written.and_then(|()| renamed(&fresh, destination));
    if replaced.is_err() {
        let _ = fs::remove_file(&fresh);
    }
    replaced
}

/// `fresh` renamed onto `destination`, tried again while the destination is refused.
fn renamed(fresh: &Path, destination: &Path) -> io::Result<()> {
    retried(|| fs::rename(fresh, destination), thread::sleep)
}

/// `attempt`, tried again after each of `DELAYS` where it fails with raw error 5 or 32,
/// the eighth failure returned as it is and any other error at once. On Windows those are
/// `ERROR_ACCESS_DENIED`, which a destination another process holds without delete sharing
/// answers, as a read-only one does, and `ERROR_SHARING_VIOLATION`. On Linux and macOS they
/// are other errors, which the same delays try again to the same failure. The lock can be
/// held across this: no Git run falls inside it, a replacement starting no process (R10).
fn retried(
    mut attempt: impl FnMut() -> io::Result<()>,
    mut wait: impl FnMut(Duration),
) -> io::Result<()> {
    for delay in DELAYS {
        match attempt() {
            Err(cause) if matches!(cause.raw_os_error(), Some(5 | 32)) => wait(delay),
            ended => return ended,
        }
    }
    attempt()
}

/// A file made for this replacement under a name nothing in the private Git directory
/// has, so that no file already there is written over or removed (R8). A kill before
/// the rename leaves it there, inside the private Git directory.
fn fresh(private_directory: &Path) -> io::Result<(PathBuf, File)> {
    for attempt in 0..NAMES {
        let name = format!("git-dupe-{}-{attempt}.new", std::process::id());
        let path = private_directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(cause) => return Err(cause),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "no fresh file name is free in {}",
            private_directory.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links_and_modes::{mode, permissions_case, symlink};
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;
    use std::time::Instant;

    /// A directory of the test's own below the system's temporary one, removed after.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let path = std::env::temp_dir()
                .join(format!("git-dupe-replace-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("private")).unwrap();
            Scratch(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_destination_is_replaced_with_the_given_permissions_and_nothing_is_left_behind() {
        let scratch = Scratch::new("mode");
        let destination = scratch.0.join("exclude");
        fs::write(&destination, "old\n").unwrap();
        whole(
            &scratch.0.join("private"),
            &destination,
            b"new\n",
            permissions_case(),
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new\n");
        assert_eq!(mode(&destination), permissions_case().map(|_| 0o640));
        assert_eq!(fs::read_dir(scratch.0.join("private")).unwrap().count(), 0);
    }

    #[test]
    fn a_link_is_replaced_by_a_regular_file_and_its_target_is_untouched() {
        let scratch = Scratch::new("link");
        let target = scratch.0.join("target");
        let destination = scratch.0.join("exclude");
        fs::write(&target, "target\n").unwrap();
        symlink(&target, &destination).unwrap();
        whole(&scratch.0.join("private"), &destination, b"new\n", None).unwrap();
        assert!(fs::symlink_metadata(&destination).unwrap().is_file());
        assert_eq!(fs::read(&destination).unwrap(), b"new\n");
        assert_eq!(fs::read(&target).unwrap(), b"target\n");
    }

    #[test]
    fn a_rename_that_fails_leaves_the_destination_and_no_fresh_file() {
        let scratch = Scratch::new("fails");
        let destination = scratch.0.join("exclude");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("inside"), "kept\n").unwrap();
        let failed = whole(&scratch.0.join("private"), &destination, b"new\n", None);
        assert!(failed.is_err());
        assert_eq!(fs::read(destination.join("inside")).unwrap(), b"kept\n");
        assert_eq!(fs::read_dir(scratch.0.join("private")).unwrap().count(), 0);
    }

    /// The time a rename onto a directory takes where its error is none the retry takes:
    /// well under the 311 ms of its delays, which only Windows's raw error 5 spends here.
    #[test]
    fn a_rename_refused_for_another_cause_spends_no_delay() {
        let scratch = Scratch::new("no-delay");
        let destination = scratch.0.join("exclude");
        fs::create_dir(&destination).unwrap();
        let began = Instant::now();
        let refused = whole(&scratch.0.join("private"), &destination, b"new\n", None);
        let took = began.elapsed();
        let cause = refused.unwrap_err();
        if !matches!(cause.raw_os_error(), Some(5 | 32)) {
            assert!(took < Duration::from_millis(250), "it took {took:?}");
        }
    }

    /// What `retried` does with an attempt that fails with `errors` in turn and then
    /// succeeds: the tries made, the waits between them, and what it returned.
    fn tried(errors: &[i32]) -> (usize, Vec<Duration>, io::Result<()>) {
        let tries = Cell::new(0);
        let waits = RefCell::new(Vec::new());
        let ended = retried(
            || {
                let try_number = tries.get();
                tries.set(try_number + 1);
                match errors.get(try_number) {
                    Some(error) => Err(io::Error::from_raw_os_error(*error)),
                    None => Ok(()),
                }
            },
            |delay| waits.borrow_mut().push(delay),
        );
        (tries.get(), waits.into_inner(), ended)
    }

    #[test]
    fn a_refused_rename_is_tried_eight_times_over_the_delays_of_311_ms() {
        let (tries, waits, ended) = tried(&[5; 100]);
        assert_eq!(tries, 8);
        assert_eq!(waits, DELAYS);
        assert_eq!(waits.iter().sum::<Duration>(), Duration::from_millis(311));
        assert_eq!(ended.unwrap_err().raw_os_error(), Some(5));
        // The eighth failure comes back as it is, whichever of the two it is.
        let (tries, _, ended) = tried(&[5, 32, 5, 32, 5, 32, 5, 32, 5]);
        assert_eq!(tries, 8);
        assert_eq!(ended.unwrap_err().raw_os_error(), Some(32));
    }

    #[test]
    fn a_refusal_that_ends_within_the_tries_succeeds_and_waits_no_more() {
        let (tries, waits, ended) = tried(&[32, 32, 32]);
        assert_eq!(tries, 4);
        assert_eq!(waits, DELAYS[..3]);
        assert!(ended.is_ok());
        let (tries, waits, ended) = tried(&[5; 7]);
        assert_eq!(tries, 8);
        assert_eq!(waits, DELAYS);
        assert!(ended.is_ok());
        let (tries, waits, ended) = tried(&[]);
        assert_eq!((tries, waits.len()), (1, 0));
        assert!(ended.is_ok());
    }

    #[test]
    fn any_other_error_returns_at_once() {
        let (tries, waits, ended) = tried(&[21, 5]);
        assert_eq!((tries, waits.len()), (1, 0));
        assert_eq!(ended.unwrap_err().raw_os_error(), Some(21));
        let (tries, waits, ended) = tried(&[5, 21, 5]);
        assert_eq!((tries, waits.len()), (2, 1));
        assert_eq!(ended.unwrap_err().raw_os_error(), Some(21));
    }

    /// Where this platform refuses a read-only destination, as Windows does with raw error
    /// 5, the refusal comes after the delays and leaves the destination (S22).
    #[test]
    fn a_read_only_destination_refused_spends_the_delays_and_stays() {
        let scratch = Scratch::new("read-only");
        let destination = scratch.0.join("exclude");
        fs::write(&destination, "old\n").unwrap();
        let mut read_only = fs::metadata(&destination).unwrap().permissions();
        read_only.set_readonly(true);
        fs::set_permissions(&destination, read_only).unwrap();
        let began = Instant::now();
        let replaced = whole(&scratch.0.join("private"), &destination, b"new\n", None);
        let took = began.elapsed();
        if let Err(cause) = replaced {
            assert_eq!(cause.raw_os_error(), Some(5));
            assert!(took >= Duration::from_millis(311), "it took {took:?}");
            assert_eq!(fs::read(&destination).unwrap(), b"old\n");
            assert_eq!(fs::read_dir(scratch.0.join("private")).unwrap().count(), 0);
        }
    }

    #[test]
    fn a_file_already_in_the_private_directory_is_neither_reused_nor_removed() {
        let scratch = Scratch::new("left");
        let private = scratch.0.join("private");
        // Whatever stands under the first name tried: a file of the user's, or one a
        // killed run left behind.
        let left = private.join(format!("git-dupe-{}-0.new", std::process::id()));
        fs::write(&left, "someone's\n").unwrap();
        let destination = scratch.0.join("exclude");
        whole(&private, &destination, b"new\n", None).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new\n");
        assert_eq!(fs::read(&left).unwrap(), b"someone's\n");
        assert_eq!(fs::read_dir(&private).unwrap().count(), 1);
    }
}
