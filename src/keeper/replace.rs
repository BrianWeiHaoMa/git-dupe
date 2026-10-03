//! Whole-file replacement: the one way git-dupe writes a file of its own (G21, S1).
//!
//! The new content goes to a fresh file inside the private Git directory, is given the
//! original's permissions, and is renamed onto the destination, so that a kill leaves
//! the old file or the new one and never a partial one. A rename onto a symbolic link
//! replaces the link, not its target.

use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// How many names `fresh` tries before giving up.
const NAMES: u32 = 100;

/// Replaces `destination` with `content`, the fresh file given `mode` when the original
/// had one and the mode a new file gets otherwise. On failure the destination is as it
/// was and the fresh file, when one was made, is removed again; the cause is returned for
/// the caller to report.
pub fn whole(
    private_directory: &Path,
    destination: &Path,
    content: &[u8],
    mode: Option<u32>,
) -> io::Result<()> {
    let (fresh, mut file) = fresh(private_directory)?;
    let written = file.write_all(content).and_then(|()| match mode {
        Some(mode) => file.set_permissions(Permissions::from_mode(mode & 0o7777)),
        None => Ok(()),
    });
    drop(file);
    let replaced = written.and_then(|()| fs::rename(&fresh, destination));
    if replaced.is_err() {
        let _ = fs::remove_file(&fresh);
    }
    replaced
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
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

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

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
    }

    #[test]
    fn the_destination_is_replaced_with_the_given_mode_and_nothing_is_left_behind() {
        let scratch = Scratch::new("mode");
        let destination = scratch.0.join("exclude");
        fs::write(&destination, "old\n").unwrap();
        whole(
            &scratch.0.join("private"),
            &destination,
            b"new\n",
            Some(0o600),
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new\n");
        assert_eq!(mode(&destination), 0o600);
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
