//! Files a scenario writes into a workspace, the names a filesystem refuses to hold, a
//! workspace copied whole, and what a scenario reads of what stands at a path: its
//! permission bits and its device and inode by `lstat`, a directory's entry names, and a
//! path as text.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::path::Path;

use super::platform::EILSEQ;
use super::scenario::report;

/// Writes `bytes` at `path` below `dir`, making the directories above it first. A file a
/// scenario runs is written by `write_executable` instead.
pub fn write(dir: &Path, path: &str, bytes: &[u8]) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// Whether the filesystem of `dir` holds an entry named `name`, one component whose bytes a
/// case needs as they are: an empty file is made directly in `dir`, where nothing stands at
/// that name, and removed. A filesystem that refuses the name as no valid encoding
/// (`EILSEQ`), as macOS's APFS refuses bytes that are no UTF-8, makes `case` unavailable
/// here: it is reported and the answer is no. Any other failure fails the scenario.
pub fn name_accepted(dir: &Path, name: &[u8], case: &str) -> bool {
    let path = dir.join(OsStr::from_bytes(name));
    let made = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path);
    match made {
        Ok(_) => {
            fs::remove_file(&path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
            true
        }
        Err(cause) if cause.raw_os_error() == Some(EILSEQ) => {
            report(&format!(
                "{case} unavailable: the name \"{}\" is refused here ({cause})",
                name.escape_ascii()
            ));
            false
        }
        Err(cause) => panic!("{}: {cause}", path.display()),
    }
}

/// Copies the directory `from` to `to`, which must not exist yet: every directory, file,
/// and symbolic link below it, with its permissions, so that `Tree` reads the copy as it
/// read the original. Git reads the copied files as changed since its index was written
/// and compares their content, never trusting what it recorded of the originals.
pub fn copy(from: &Path, to: &Path) {
    let failed =
        |path: &Path, cause: std::io::Error| -> ! { panic!("{}: {cause}", path.display()) };
    fs::create_dir(to).unwrap_or_else(|cause| failed(to, cause));
    for entry in fs::read_dir(from).unwrap_or_else(|cause| failed(from, cause)) {
        let entry = entry.unwrap_or_else(|cause| failed(from, cause));
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let metadata = fs::symlink_metadata(&source).unwrap_or_else(|cause| failed(&source, cause));
        if metadata.is_symlink() {
            let link = fs::read_link(&source).unwrap_or_else(|cause| failed(&source, cause));
            symlink(link, &target).unwrap_or_else(|cause| failed(&target, cause));
        } else if metadata.is_dir() {
            copy(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap_or_else(|cause| failed(&target, cause));
        }
    }
    let permissions = fs::metadata(from).unwrap_or_else(|cause| failed(from, cause));
    fs::set_permissions(to, permissions.permissions()).unwrap_or_else(|cause| failed(to, cause));
}

/// The permission bits of what stands at `path`, by `lstat`: a symbolic link's own.
pub fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o7777
}

/// The device and inode of what stands at `path`, by `lstat`: the same pair after a
/// command is the same file, and a rename onto `path` changes it.
pub fn inode(path: &Path) -> (u64, u64) {
    let found = fs::symlink_metadata(path).unwrap();
    (found.dev(), found.ino())
}

/// The names of the entries of the directory `directory`.
pub fn entry_names(directory: &Path) -> BTreeSet<OsString> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect()
}

/// `path` as text, for a word a scenario types or a part a line must name: the scenario's
/// own paths are UTF-8.
pub fn text(path: &Path) -> &str {
    path.to_str().expect("a UTF-8 scenario path")
}
