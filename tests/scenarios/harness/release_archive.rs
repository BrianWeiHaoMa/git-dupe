//! The release archive: this package built in the release profile for the release's target
//! (`release_build.rs`), and its members packed into
//! `git-dupe-<version>-<platform>-<architecture>.<extension>` beside the executable,
//! the platform's word and the architecture from the platform module (P3), all under the
//! one directory `git-dupe-<version>`, with a `.sha256` beside it that names it by its bare
//! name. On Linux and macOS it is a tar of the executable, the manual page that build
//! rendered, and the install note `INSTALL`; on Windows a zip of stored entries, which a
//! check reads without an inflater, of the executable and `INSTALL-windows.txt`, no page
//! though the build renders one (F10). It is made anew by every check command that needs
//! it and replaces whole the one an earlier command made, which holds the binary of an
//! earlier source; a build that fails ends the command's archive checks instead. Needs
//! `tar`, on Windows Windows's own. And the `PATH` of a run that installs from it, or uses
//! what was installed.

use std::env::consts::EXE_SUFFIX;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::fresh::FreshDirectory;
use super::platform::{architecture, permissions, word};
use super::provision::windows_tar;
use super::release_build::{built_for_the_release, run, tested_executable};
use super::scenario::Scenario;
use super::sha256::hex_digest;

/// What one check command made.
pub struct ReleaseArchive {
    /// The archive, beside the release executable.
    pub path: PathBuf,
    /// `<path>.sha256`: the archive's digest, naming it by its bare name.
    pub digest: PathBuf,
    /// The release executable, where Cargo named it for this build.
    pub executable: PathBuf,
    /// The page that build rendered, in the output directory Cargo named for its build
    /// script; packed on Linux and macOS alone.
    pub page: PathBuf,
}

impl ReleaseArchive {
    /// Unpacks the archive into `directory`, as anyone unpacks it.
    pub fn unpack_into(&self, s: &Scenario, directory: &Path) {
        s.program(
            unpacker(),
            [OsString::from("-xf"), self.path.clone().into()],
        )
        .from(directory)
        .succeeds();
    }
}

/// The program that reads this platform's archive: by its full path on Windows, the one
/// `tar` that reads `C:\...` as a path (S21).
fn unpacker() -> OsString {
    match word() {
        "windows" => windows_tar().unwrap().into(),
        _ => "tar".into(),
    }
}

/// The archive's extension here: the one place that decides it (P3).
pub fn archive_extension() -> &'static str {
    match word() {
        "windows" => "zip",
        _ => "tar",
    }
}

/// The archive of this check command, made by its first caller; the others wait for it.
pub fn release_archive() -> &'static ReleaseArchive {
    static MADE: OnceLock<Result<ReleaseArchive, String>> = OnceLock::new();
    match MADE.get_or_init(make) {
        Ok(made) => made,
        Err(cause) => panic!("{cause}"),
    }
}

/// The install note, packed as the repository holds it.
pub fn install_note() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("INSTALL")
}

/// The Windows install note, packed as the repository holds it.
pub fn windows_install_note() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("INSTALL-windows.txt")
}

/// The command lines of an install note: each line marked `$ ` or `PS> `, taken as written.
pub fn note_commands(note: &str) -> Vec<String> {
    note.lines()
        .map(str::trim_start)
        .filter_map(|line| line.strip_prefix("$ ").or(line.strip_prefix("PS> ")))
        .map(str::to_owned)
        .collect()
}

/// Windows PowerShell 5.1, called only where `word()` is `windows`: by its full path,
/// because `Command` searches no `WindowsPowerShell\v1.0` (S20). A run gives it
/// `-NoProfile -ExecutionPolicy Restricted -Command <line>`, the strictest policy whatever
/// the machine's, and, where the line touches `~`, `USERPROFILE` as well as `HOME`, because
/// `~` follows the first and is unset under the second alone.
pub fn windows_powershell() -> PathBuf {
    let root = std::env::var_os("SystemRoot").expect("SystemRoot names Windows's directory");
    Path::new(&root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

/// A `PATH` on which the only git-dupe is one installed into a directory of `front`: those
/// directories, the release's `git`, and the caller's `PATH` without the build directory
/// and without any directory that holds a `git-dupe`, because this machine may have one
/// installed, which would answer in place of the copy.
pub fn path_for_an_install(s: &Scenario, front: &[&Path]) -> OsString {
    let built = tested_executable().parent().unwrap();
    let callers = std::env::var_os("PATH").unwrap_or_default();
    let executable = format!("git-dupe{EXE_SUFFIX}");
    let callers = std::env::split_paths(&callers)
        .filter(|dir| dir != built && fs::symlink_metadata(dir.join(&executable)).is_err());
    let release_bin = s.release_git().parent().unwrap().to_path_buf();
    let front = front.iter().map(|dir| dir.to_path_buf());
    std::env::join_paths(front.chain([release_bin]).chain(callers)).unwrap()
}

fn make() -> Result<ReleaseArchive, String> {
    let (executable, out_dir) = built_for_the_release(true)?;
    let page = out_dir.join("git-dupe.1");
    let leading = format!("git-dupe-{}", env!("CARGO_PKG_VERSION"));

    // Staged with the modes an archive for anyone needs, whatever the builder's umask.
    let staging = FreshDirectory::create();
    let staged = staging.path().join(&leading);
    fs::create_dir(&staged).map_err(because("create", &staged))?;
    let members = packed_members(&executable, &page);
    for (from, name, mode) in &members {
        let to = staged.join(name);
        fs::copy(from, &to).map_err(because("copy", from))?;
        if let Some(permissions) = permissions(*mode) {
            fs::set_permissions(&to, permissions).map_err(because("set the mode of", &to))?;
        }
    }

    // Packed beside the archive and renamed over it, so that the path holds a whole
    // archive or none. The files are named one by one: no entry for their directory.
    let name = format!(
        "{leading}-{}-{}.{}",
        word(),
        architecture(),
        archive_extension()
    );
    let path = executable.with_file_name(&name);
    let packing = beside(&path);
    let mut tar = packer()?;
    tar.arg("--create")
        .arg("--file")
        .arg(&packing)
        .arg("--directory")
        .arg(staging.path())
        .args(
            members
                .iter()
                .map(|(_, name, _)| format!("{leading}/{name}")),
        );
    let packed = run(&mut tar).map(drop);
    placed(packed, &packing, &path)?;

    let digest = PathBuf::from(format!("{}.sha256", path.display()));
    let archive = fs::read(&path).map_err(because("read", &path))?;
    let line = format!("{}  {name}\n", hex_digest(&archive));
    let writing = beside(&digest);
    let written = fs::write(&writing, line).map_err(because("write", &writing));
    placed(written, &writing, &digest)?;

    Ok(ReleaseArchive {
        path,
        digest,
        executable,
        page,
    })
}

/// What this platform's archive holds: the file, its name under the leading directory, and
/// the mode an archive with modes gives it.
fn packed_members(executable: &Path, page: &Path) -> Vec<(PathBuf, String, u32)> {
    let executable = (
        executable.to_path_buf(),
        format!("git-dupe{EXE_SUFFIX}"),
        0o755,
    );
    match word() {
        "windows" => vec![
            executable,
            (
                windows_install_note(),
                "INSTALL-windows.txt".to_owned(),
                0o644,
            ),
        ],
        _ => vec![
            executable,
            (page.to_path_buf(), "git-dupe.1".to_owned(), 0o644),
            (install_note(), "INSTALL".to_owned(), 0o644),
        ],
    }
}

/// The tool that packs this platform's archive, with its format: entries stored, on
/// Windows, so that a check reads a member's bytes from the zip itself.
fn packer() -> Result<Command, String> {
    Ok(match word() {
        "windows" => {
            let mut tar = Command::new(windows_tar()?);
            tar.args(["--format=zip", "--options", "zip:compression=store"]);
            tar
        }
        _ => {
            let mut tar = Command::new("tar");
            tar.arg("--format=ustar");
            tar
        }
    })
}

/// A name beside `path` for a file on its way there.
fn beside(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap().to_string_lossy();
    path.with_file_name(format!("{name}.{}", std::process::id()))
}

/// Renames `from`, once it was `written`, onto `to`; `from` does not outlive a failure.
fn placed(written: Result<(), String>, from: &Path, to: &Path) -> Result<(), String> {
    let done = written.and_then(|_| fs::rename(from, to).map_err(because("rename", from)));
    if done.is_err() {
        let _ = fs::remove_file(from);
    }
    done
}

/// One entry of a zip's central directory.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZipMember {
    pub name: String,
    /// The compression method: 0 is stored.
    pub method: u16,
    /// The bytes as the archive holds them, read through the entry's local header.
    pub content: Vec<u8>,
}

/// Every entry of a zip with no archive comment, read from its central directory, so that
/// a directory entry is a member too.
pub fn zip_members(archive: &[u8]) -> Vec<ZipMember> {
    let number = |at: usize, bytes: usize| {
        let field = &archive[at..at + bytes];
        field
            .iter()
            .rev()
            .fold(0, |n, &byte| n << 8 | byte as usize)
    };
    let end = archive.len().checked_sub(22).expect("a zip's end record");
    assert_eq!(&archive[end..end + 4], b"PK\x05\x06", "a zip's end record");
    let mut at = number(end + 16, 4);
    let mut members = Vec::new();
    for _ in 0..number(end + 10, 2) {
        assert_eq!(
            &archive[at..at + 4],
            b"PK\x01\x02",
            "a central entry at byte {at}"
        );
        let (name_size, extra_size, comment_size) =
            (number(at + 28, 2), number(at + 30, 2), number(at + 32, 2));
        let name = &archive[at + 46..at + 46 + name_size];
        let header = number(at + 42, 4);
        assert_eq!(
            &archive[header..header + 4],
            b"PK\x03\x04",
            "a local header at byte {header}"
        );
        let start = header + 30 + number(header + 26, 2) + number(header + 28, 2);
        members.push(ZipMember {
            name: String::from_utf8(name.to_vec()).unwrap(),
            method: number(at + 10, 2) as u16,
            content: archive[start..start + number(at + 20, 4)].to_vec(),
        });
        at += 46 + name_size + extra_size + comment_size;
    }
    assert_eq!(at, end, "nothing between the central directory and its end");
    members
}

fn because(act: &str, path: &Path) -> impl Fn(std::io::Error) -> String {
    move |cause| format!("cannot {act} {}: {cause}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A zip of `(name, method, content)` entries in the fields `zip_members` reads, each
    /// entry with one byte of extra field locally and centrally, as a tool's differ.
    fn zip(entries: &[(&str, u16, &[u8])]) -> Vec<u8> {
        let two = |n: usize| (n as u16).to_le_bytes();
        let four = |n: usize| (n as u32).to_le_bytes();
        let (mut local, mut central) = (Vec::new(), Vec::new());
        for (name, method, content) in entries {
            let offset = local.len();
            local.extend(b"PK\x03\x04");
            local.extend([0; 22]);
            local.extend(two(name.len()));
            local.extend(two(1));
            local.extend(name.bytes());
            local.push(0);
            local.extend(*content);
            central.extend(b"PK\x01\x02");
            central.extend([0; 6]);
            central.extend(method.to_le_bytes());
            central.extend([0; 8]);
            central.extend(four(content.len()));
            central.extend(four(content.len()));
            central.extend(two(name.len()));
            central.extend(two(1));
            central.extend([0; 10]);
            central.extend(four(offset));
            central.extend(name.bytes());
            central.push(0);
        }
        let (start, size) = (local.len(), central.len());
        local.extend(central);
        local.extend(b"PK\x05\x06");
        local.extend([0; 4]);
        local.extend(two(entries.len()));
        local.extend(two(entries.len()));
        local.extend(four(size));
        local.extend(four(start));
        local.extend([0; 2]);
        local
    }

    /// A directory entry, a third entry, and a deflated one are each what the archive's
    /// check compares and refuses: the reader hides none of them.
    #[test]
    fn every_entry_of_a_zip_is_read_with_its_method_and_bytes() {
        let members = zip_members(&zip(&[
            ("d/", 0, b""),
            ("d/a", 0, b"bytes"),
            ("d/b", 8, b"deflated"),
        ]));
        let read: Vec<(&str, u16, &[u8])> = members
            .iter()
            .map(|member| (member.name.as_str(), member.method, &member.content[..]))
            .collect();
        assert_eq!(
            read,
            [
                ("d/", 0, &b""[..]),
                ("d/a", 0, &b"bytes"[..]),
                ("d/b", 8, &b"deflated"[..])
            ]
        );
    }
}
