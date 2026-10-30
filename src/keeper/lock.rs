//! The lock file of G28: one empty regular file of the common Git directory, shared by
//! every worktree of the project, on every platform, because a directory cannot be locked
//! on Windows and a lock is mandatory there, so that it can lie on no file Git reads (S16).
//!
//! It is created when it is missing and never removed, truncated, or written through, and
//! nothing opens it but `open`, for `region`'s one `update` (R10): its bytes mean nothing,
//! only the lock on it does (`State`). A symbolic link standing there is not followed, so
//! that nothing is created or locked outside the common Git directory (G5).

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// The file's name, in the common Git directory.
const NAME: &str = "git-dupe.lock";

/// The file's path.
pub fn path(common_directory: &Path) -> PathBuf {
    common_directory.join(NAME)
}

/// The file, for reading and writing, as `File::lock` asks on every platform, made when it
/// is missing.
pub fn open(common_directory: &Path) -> io::Result<File> {
    let file = path(common_directory);
    if fs::symlink_metadata(&file).is_ok_and(|found| found.file_type().is_symlink()) {
        return Err(io::Error::other("a symbolic link stands there"));
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(file)
}
