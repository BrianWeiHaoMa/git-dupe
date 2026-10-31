//! The Windows arms of the platform module, each item the one `platform.rs` holds for Linux
//! and macOS. What the standard library does not give is declared here and in `job.rs`, no
//! crate (R7).

use std::ffi::{OsString, c_void};
use std::fs;
use std::io;
use std::os::windows::fs::{OpenOptionsExt, symlink_dir, symlink_file};
use std::os::windows::io::AsRawHandle;
// For `job.rs`, so that the module's two files alone name the platform's standard library.
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::ptr;
use std::sync::OnceLock;

use super::{FreshDirectory, SecondFilesystem, SecondFilesystemFound};
use crate::harness::releases::mingit;
use crate::harness::scenario::report;

mod job;
pub use job::{Group, HeldProcess, ended, process_held, spawn_grouped};

type Handle = *mut c_void;

const DRIVE_FIXED: u32 = 3;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_EVERYTHING: u32 = 0x7;

#[repr(C)]
#[derive(Default)]
struct ByHandleFileInformation {
    attributes: u32,
    times: [u32; 6],
    volume_serial_number: u32,
    size: [u32; 2],
    links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetDriveTypeW(root: *const u16) -> u32;
    fn GetFileInformationByHandle(file: Handle, found: *mut ByHandleFileInformation) -> i32;
    fn GetLogicalDrives() -> u32;
    fn GetVolumeInformationW(
        root: *const u16,
        name: *mut u16,
        name_size: u32,
        serial: *mut u32,
        component_length: *mut u32,
        flags: *mut u32,
        filesystem: *mut u16,
        filesystem_size: u32,
    ) -> i32;
}

/// The platform the checks run on, for a check that holds on one of them alone: `windows`.
pub fn word() -> &'static str {
    "windows"
}

/// The machine's architecture, `x86_64` here, with no `uname` to ask (P3).
pub fn architecture() -> String {
    std::env::consts::ARCH.to_owned()
}

/// The name these bytes spell: valid UTF-8 alone is one here (S20).
pub fn name(bytes: &[u8]) -> Option<OsString> {
    std::str::from_utf8(bytes).ok().map(OsString::from)
}

/// The real path of `path` without the verbatim prefix `fs::canonicalize` gives it, so that
/// Git accepts it as a word (S23, S24): `\\?\C:\x` is `C:\x`, `\\?\UNC\s\h\x` is `\\s\h\x`.
pub fn canonical(path: &Path) -> io::Result<PathBuf> {
    let real = fs::canonicalize(path)?;
    let text = real
        .to_str()
        .ok_or_else(|| io::Error::other(format!("{} is not UTF-8", real.display())))?;
    Ok(
        match (text.strip_prefix(r"\\?\UNC\"), text.strip_prefix(r"\\?\")) {
            (Some(share), _) => PathBuf::from(format!(r"\\{share}")),
            (None, Some(drive)) => PathBuf::from(drive),
            (None, None) => real,
        },
    )
}

/// A symbolic link at `at` to `target`, a directory's link where `target`, read against
/// `at`'s parent, is a directory, each `/` of it stored as `\`, because Windows reads a link
/// holding a `/` as no name (error 123). Developer Mode or an elevated session makes one
/// (S24).
pub fn symlink(target: impl AsRef<Path>, at: impl AsRef<Path>) -> io::Result<()> {
    let target = target.as_ref().components().collect::<PathBuf>();
    let target = target.as_path();
    let at = at.as_ref();
    let read = at.parent().unwrap_or(Path::new("")).join(target);
    if fs::metadata(read).is_ok_and(|found| found.is_dir()) {
        symlink_dir(target, at)
    } else {
        symlink_file(target, at)
    }
}

/// The volume serial number and file index of what stands at `path`, a link's own as `lstat`
/// gives it, through a handle on the entry: `MetadataExt`'s identity is unstable on the
/// pinned toolchain (S24). The same pair after a command is the same file, and a rename onto
/// `path` changes it.
pub fn identity(path: &Path) -> (u64, u64) {
    let failed = |cause: io::Error| -> ! { panic!("{}: {cause}", path.display()) };
    let opened = fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_EVERYTHING)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .unwrap_or_else(|cause| failed(cause));
    let mut found = ByHandleFileInformation::default();
    // SAFETY: the handle is open for the call and `found` has the layout it fills.
    if unsafe { GetFileInformationByHandle(opened.as_raw_handle(), &mut found) } == 0 {
        failed(io::Error::last_os_error());
    }
    let index = (u64::from(found.file_index_high) << 32) | u64::from(found.file_index_low);
    (u64::from(found.volume_serial_number), index)
}

/// None: Windows has no mode bits.
pub fn mode(_: &Path) -> Option<u32> {
    None
}

/// None: Windows has no mode bits.
pub fn permissions(_: u32) -> Option<fs::Permissions> {
    None
}

/// Whether a child can end by a signal here: no.
pub fn signals() -> bool {
    false
}

/// None: Windows has no signals; a child's end is its exit code (F9).
pub fn signal_of(_: &ExitStatus) -> Option<i32> {
    None
}

/// Whether Git reaches a linked worktree's common Git directory through a symbolic link above
/// its Git directory: no, because Windows folds the `..` of its `commondir` file by the path's
/// spelling, below the link.
pub fn commondir_read_through_a_link() -> bool {
    false
}

/// Whether a run can be started under a soft `RLIMIT_FSIZE`: Windows has none.
pub fn file_size_limits() -> bool {
    false
}

/// Whether a Git reading `.git/info/exclude` can fail at the moment another command replaces
/// it, the seventh accepted limit: under every release here, because a check holds no value
/// per release.
pub fn a_replacement_can_fail_a_reader() -> bool {
    true
}

/// What one command line may carry here, in UTF-16 code units (S20).
pub fn command_line_limit() -> usize {
    32_767
}

/// `Ok` once an empty entry named `name` was made directly in `dir`, where nothing stands at
/// that name, and removed; the reason where Windows cannot hold the name, decided before
/// anything is made, because `a:b` would make a stream of `a` and a trailing dot or space
/// another name, or where the entry made lists under another name (E4, S23). Any other
/// failure fails the scenario.
pub fn name_accepted(dir: &Path, name: &[u8]) -> Result<(), String> {
    let refused = |why: String| {
        Err(format!(
            "a name Windows cannot hold or one that is not valid Unicode: \"{}\" {why}",
            name.escape_ascii()
        ))
    };
    let Ok(text) = std::str::from_utf8(name) else {
        return refused("is not valid UTF-8".to_owned());
    };
    let forbidden = |found: char| "<>:\"|?*\\".contains(found) || found < ' ';
    if let Some(found) = text.chars().find(|found| forbidden(*found)) {
        return refused(format!("holds {found:?}"));
    }
    let stem = text.split('.').next().unwrap_or(text);
    if is_reserved(stem) {
        return refused("is a reserved device name".to_owned());
    }
    if text.ends_with(['.', ' ']) {
        return refused("ends in a dot or a space".to_owned());
    }
    let path = dir.join(text);
    let failed = |cause: io::Error| -> ! { panic!("{}: {cause}", path.display()) };
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap_or_else(|cause| failed(cause));
    let listed = fs::read_dir(dir)
        .unwrap_or_else(|cause| failed(cause))
        .any(|entry| entry.is_ok_and(|entry| entry.file_name() == text));
    if !listed {
        // The name made is another: the file is found by the path it was made by, and goes.
        fs::remove_file(&path).unwrap_or_else(|cause| failed(cause));
        return refused("lists under another name".to_owned());
    }
    fs::remove_file(&path).unwrap_or_else(|cause| failed(cause));
    Ok(())
}

/// Whether `stem`, a name up to its first dot, names a device (S23).
fn is_reserved(stem: &str) -> bool {
    let stem = stem.trim_end_matches(' ').to_ascii_uppercase();
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix)
            .is_some_and(|digit| matches!(digit.as_bytes(), [b'1'..=b'9']))
    };
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT")
}

/// The directories that lead a scenario's `PATH`, in order (S21).
pub fn release_programs(release: &Path) -> Vec<PathBuf> {
    vec![release.join("cmd"), release.join("usr").join("bin")]
}

/// The release's own `git` program.
pub fn release_git(release: &Path) -> PathBuf {
    release.join("cmd").join("git.exe")
}

/// The shell a scenario runs a command line through: the release's own.
pub fn shell(release: &Path) -> PathBuf {
    release.join("usr").join("bin").join("sh.exe")
}

/// What `git --version` prints under the listed release `version` (S21).
pub fn git_version_line(version: &str) -> String {
    format!("git version {}.windows.1\n", mingit(version).build)
}

/// The variables of the caller's a scenario's environment needs (`Composition/The checks`).
pub fn environment_passed_through() -> &'static [&'static str] {
    &["SystemRoot", "TEMP", "TMP", "PATHEXT"]
}

/// The variables a scenario's environment sets here: without this one Git makes no symbolic
/// link (S21).
pub fn environment_set() -> &'static [(&'static str, &'static str)] {
    &[("MSYS", "winsymlinks:nativestrict")]
}

/// A command of the shell's printing the process its parameter `$<parameter>` names, as
/// `ended` reads it: the system's number, where the shell numbers its own.
pub fn shell_process_number(parameter: &str) -> String {
    format!(r#"cat "/proc/${parameter}/winpid""#)
}

/// A process that lives `seconds` and ends by itself: MinGit has no `sleep`, and `ping` waits
/// a second between its replies.
pub fn sleeping(seconds: u32) -> Command {
    let root = std::env::var_os("SystemRoot").expect("SystemRoot names Windows's directory");
    let mut ping = Command::new(Path::new(&root).join("System32").join("ping.exe"));
    ping.args(["-n", &(seconds + 1).to_string(), "127.0.0.1"]);
    ping
}

/// The stand-in `git` a caller copies into its control directory as `git.exe`, the one name
/// `Command` finds (S20): `platform/forwarder.rs`, compiled once per check run, beside the
/// built git-dupe.
pub fn stand_in_git_program() -> Option<&'static Path> {
    static COMPILED: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    match COMPILED.get_or_init(compiled_forwarder) {
        Ok(program) => Some(program),
        Err(cause) => panic!("{cause}"),
    }
}

fn compiled_forwarder() -> Result<PathBuf, String> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("scenarios")
        .join("harness")
        .join("platform")
        .join("forwarder.rs");
    let program = Path::new(env!("CARGO_BIN_EXE_git-dupe")).with_file_name("stand-in-git.exe");
    report("compiling the stand-in git program with rustc");
    let compiled = Command::new("rustc")
        .args(["--edition", "2024", "-O", "-D", "warnings", "-o"])
        .arg(&program)
        .arg(&source)
        .output()
        .map_err(|cause| format!("cannot run rustc: {cause}"))?;
    if !compiled.status.success() {
        return Err(format!(
            "rustc {}: {}: {}",
            source.display(),
            compiled.status,
            String::from_utf8_lossy(&compiled.stderr)
        ));
    }
    Ok(program)
}

/// Each way of this platform: a volume under a drive letter of its own.
pub fn second_filesystems() -> Vec<SecondFilesystemFound> {
    vec![SecondFilesystemFound {
        reached: "under its own drive letter",
        found: under_its_own_drive_letter(),
    }]
}

/// A fresh directory on a fixed NTFS volume other than the one the temporary directory lies
/// on, its serial number telling them apart, because `subst` makes a letter and no volume
/// (S24); the reason where this machine has none.
fn under_its_own_drive_letter() -> Result<SecondFilesystem, String> {
    let (first, _) = identity(&std::env::temp_dir());
    // SAFETY: the call takes no argument.
    let drives = unsafe { GetLogicalDrives() };
    let root = (0..26)
        .filter(|letter| drives & (1 << letter) != 0)
        .map(|letter| format!("{}:\\", char::from(b'A' + letter)))
        .find(|root| fixed_ntfs_serial(root).is_some_and(|serial| u64::from(serial) != first))
        .ok_or("no fixed NTFS volume but the temporary directory's has a drive letter")?;
    let directory = FreshDirectory::create_in(Path::new(&root))
        .ok_or_else(|| format!("no fresh directory can be made at {root}"))?;
    Ok(SecondFilesystem { directory })
}

/// The serial number of the volume at the drive root `root`, where it is fixed and NTFS.
fn fixed_ntfs_serial(root: &str) -> Option<u32> {
    let wide: Vec<u16> = root.encode_utf16().chain([0]).collect();
    // SAFETY: `wide` ends in its terminator.
    if unsafe { GetDriveTypeW(wide.as_ptr()) } != DRIVE_FIXED {
        return None;
    }
    let mut serial = 0;
    let mut filesystem = [0u16; 16];
    // SAFETY: each buffer is given with its size, and what is not asked for is null.
    let asked = unsafe {
        GetVolumeInformationW(
            wide.as_ptr(),
            ptr::null_mut(),
            0,
            &mut serial,
            ptr::null_mut(),
            ptr::null_mut(),
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    };
    let ntfs: Vec<u16> = "NTFS".encode_utf16().chain([0]).collect();
    (asked != 0 && filesystem[..ntfs.len()] == ntfs[..]).then_some(serial)
}

#[cfg(test)]
mod tests;
