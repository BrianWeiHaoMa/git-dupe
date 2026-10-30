//! The one place under `src/` where a unit check takes a tool the standard library gives
//! under `std::os::unix` or `std::os::windows` alone: a symbolic link, a mode bit, a file
//! identity, and what this platform can do (`Composition/The checks`). Declared for the
//! unit checks alone, so that R11's count of the executable never reads it. Every item
//! answers on every platform; a check whose tool answers `None` returns at once, pinning
//! nothing there.

use std::fs::{self, Permissions};
use std::io;
use std::path::Path;

/// A symbolic link at `at` to `target`.
#[cfg(unix)]
pub fn symlink(target: impl AsRef<Path>, at: impl AsRef<Path>) -> io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

/// A symbolic link at `at` to `target`, a directory's link where `target`, read against
/// `at`'s parent, is a directory. Developer Mode or an elevated session makes one (S24).
#[cfg(windows)]
pub fn symlink(target: impl AsRef<Path>, at: impl AsRef<Path>) -> io::Result<()> {
    let target = target.as_ref();
    let at = at.as_ref();
    let read = at.parent().unwrap_or(Path::new("")).join(target);
    if fs::metadata(read).is_ok_and(|found| found.is_dir()) {
        std::os::windows::fs::symlink_dir(target, at)
    } else {
        std::os::windows::fs::symlink_file(target, at)
    }
}

/// Removes the symbolic link at `at`, which on Windows is a directory's where its target
/// was one.
pub fn remove_link(at: impl AsRef<Path>) -> io::Result<()> {
    let at = at.as_ref();
    fs::remove_file(at).or_else(|_| fs::remove_dir(at))
}

/// Permissions distinct from what a fresh file gets, for a check that a replacement carries
/// the original's.
#[cfg(unix)]
pub fn permissions_case() -> Option<Permissions> {
    use std::os::unix::fs::PermissionsExt;
    Some(Permissions::from_mode(0o640))
}

/// None: the one bit `Permissions` holds here, read-only, would refuse the rename itself
/// (S22, S24).
#[cfg(windows)]
pub fn permissions_case() -> Option<Permissions> {
    None
}

/// The permission bits of what stands at `path` by `lstat`.
#[cfg(unix)]
pub fn mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    let found = fs::symlink_metadata(path).ok()?;
    Some(found.permissions().mode() & 0o7777)
}

/// None: Windows has no mode bits.
#[cfg(windows)]
pub fn mode(_: &Path) -> Option<u32> {
    None
}

/// The device and inode of what stands at `path` by `lstat`.
#[cfg(unix)]
pub fn identity(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let found = fs::symlink_metadata(path).ok()?;
    Some((found.dev(), found.ino()))
}

/// None: a file's identity is unstable on the pinned toolchain here (S24).
#[cfg(windows)]
pub fn identity(_: &Path) -> Option<(u64, u64)> {
    None
}

/// Whether every byte string is a name here: what branches a check that pins what
/// `bytes::name` refuses on Windows.
pub fn names_every_byte_string() -> bool {
    crate::bytes::name(b"\xff").is_some()
}

/// Whether a child can end by a signal here.
pub fn signals() -> bool {
    cfg!(unix)
}
