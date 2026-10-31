//! Files a scenario writes into a workspace, the names a filesystem refuses to hold, a
//! workspace copied whole, and what a scenario reads or sets of what stands at a path: its
//! permission bits, each reported unavailable where the platform has none, a directory's
//! entry names, and a path as text.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::platform;
use super::platform::symlink;
use super::scenario::{Scenario, report};

/// Writes `bytes` at `path` below `dir`, making the directories above it first. A file a
/// scenario runs is written by `write_executable` instead.
pub fn write(dir: &Path, path: &str, bytes: &[u8]) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

/// The name `bytes` spell, for a fixture whose name the platform holds: `name_accepted`
/// has answered for it, or it is valid UTF-8 and of a form every platform holds. Panics on
/// bytes the platform refuses, never converting them lossily.
pub fn name(bytes: &[u8]) -> OsString {
    platform::name(bytes)
        .unwrap_or_else(|| panic!("\"{}\" names nothing here", bytes.escape_ascii()))
}

/// The word `bytes` spell, for a case that types them whole. Where this platform's command
/// line cannot carry them, `case` is reported unavailable and the answer is `None` (S20).
pub fn word_of(s: &Scenario, bytes: &[u8], case: &str) -> Option<OsString> {
    let word = platform::name(bytes);
    if word.is_none() {
        s.unavailable(
            &format!("{case}: a name Windows cannot hold or one that is not valid Unicode"),
            &format!("\"{}\" crosses no command line here", bytes.escape_ascii()),
        );
    }
    word
}

/// Whether the filesystem of `dir` holds an entry named `name`, one component whose bytes a
/// case needs as they are (`platform::name_accepted`). A name this platform or filesystem
/// cannot hold makes `case` unavailable here: it is reported and the answer is no. Any other
/// failure fails the scenario.
pub fn name_accepted(dir: &Path, name: &[u8], case: &str) -> bool {
    match platform::name_accepted(dir, name) {
        Ok(()) => true,
        Err(reason) => {
            report(&format!("{case} unavailable: {reason}"));
            false
        }
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

/// The permission bits of what stands at `path`, by `lstat`: a symbolic link's own. Where
/// the platform has none, the `part` of the scenario that reads them reports itself
/// unavailable and the answer is `None`.
pub fn mode(s: &Scenario, path: &Path, part: &str) -> Option<u32> {
    let bits = platform::mode(path);
    if bits.is_none() {
        unavailable(s, part);
    }
    bits
}

/// What stands at `path` has the permission bits `bits`, where the platform has any.
pub fn mode_is(s: &Scenario, path: &Path, bits: u32, part: &str) {
    if let Some(found) = mode(s, path, part) {
        assert_eq!(found, bits, "the mode of {}", path.display());
    }
}

/// Gives what stands at `path` the permission bits `bits`, where the platform has any.
pub fn set_mode(s: &Scenario, path: &Path, bits: u32, part: &str) {
    match platform::permissions(bits) {
        Some(permissions) => fs::set_permissions(path, permissions)
            .unwrap_or_else(|cause| panic!("{}: {cause}", path.display())),
        None => unavailable(s, part),
    }
}

/// `part` of a scenario turns on a mode bit, and this platform has none.
fn unavailable(s: &Scenario, part: &str) {
    s.unavailable(
        &format!("{part}: a Unix mode bit"),
        "this platform has none",
    );
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

/// An NTFS junction at `at` to the absolute directory `target`, which `cmd` alone makes:
/// `symlink` makes a symbolic link, and the two differ to Git (S24).
pub fn junction(target: &Path, at: &Path) {
    let made = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .args([at, target])
        .output()
        .unwrap_or_else(|cause| panic!("cmd: {cause}"));
    assert!(made.status.success(), "mklink /J: {made:?}");
}
