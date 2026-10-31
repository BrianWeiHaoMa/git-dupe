//! The release archive: this package built in the release profile and its members packed
//! into `git-dupe-<version>-<platform>-<architecture>.<extension>` beside the executable,
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
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use super::fresh::FreshDirectory;
use super::platform::{architecture, permissions, word};
use super::provision::windows_tar;
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
    let built = Path::new(env!("CARGO_BIN_EXE_git-dupe")).parent().unwrap();
    let callers = std::env::var_os("PATH").unwrap_or_default();
    let executable = format!("git-dupe{EXE_SUFFIX}");
    let callers = std::env::split_paths(&callers)
        .filter(|dir| dir != built && fs::symlink_metadata(dir.join(&executable)).is_err());
    let release_bin = s.release_git().parent().unwrap().to_path_buf();
    let front = front.iter().map(|dir| dir.to_path_buf());
    std::env::join_paths(front.chain([release_bin]).chain(callers)).unwrap()
}

fn make() -> Result<ReleaseArchive, String> {
    let (executable, out_dir) = release_build()?;
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

/// Builds the package in the release profile and returns the executable and the build
/// script's output directory as Cargo names them in its messages. `.cargo/config.toml`
/// reaches this build as it reaches every other (S25).
fn release_build() -> Result<(PathBuf, PathBuf), String> {
    let said = run(Command::new(env!("CARGO"))
        .args([
            "build",
            "--release",
            "--locked",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR")))?;
    named(&said)
}

/// The executable and the output directory that Cargo's messages name, one each: a
/// `compiler-artifact` line with an `executable`, the build script's having none, and a
/// `build-script-executed` line with its `out_dir`. One package is built.
fn named(said: &str) -> Result<(PathBuf, PathBuf), String> {
    let field = |reason: &str, name: &str| -> Result<Vec<PathBuf>, String> {
        let opening = format!("{{\"reason\":\"{reason}\",");
        said.lines()
            .filter(|line| line.starts_with(&opening))
            .filter_map(|line| string_field(line, name).transpose())
            .collect()
    };
    let executables = field("compiler-artifact", "executable")?;
    let out_dirs = field("build-script-executed", "out_dir")?;
    match (&executables[..], &out_dirs[..]) {
        ([executable], [out_dir]) if names_the_executable(executable) => {
            Ok((executable.clone(), out_dir.clone()))
        }
        _ => Err(format!(
            "the release build named the executables {executables:?} and the output \
             directories {out_dirs:?}, where one `git-dupe` and one directory were expected:\n\
             {said}"
        )),
    }
}

/// Whether `path`'s last component, after either separator, is the package's executable:
/// read from the text, because the build machine's `Path` is not the message's.
fn names_the_executable(path: &Path) -> bool {
    let path = path.to_string_lossy();
    let name = path.rsplit(['/', '\\']).next();
    matches!(name, Some("git-dupe" | "git-dupe.exe"))
}

/// The value of the string field `name` of a one-line JSON message, or none where the
/// line holds no such string. Of Cargo's escapes only `\\` is read, the separator of a
/// Windows path.
fn string_field(line: &str, name: &str) -> Result<Option<PathBuf>, String> {
    let opening = format!("\"{name}\":\"");
    let Some((_, rest)) = line.split_once(&opening) else {
        return Ok(None);
    };
    let mut value = String::new();
    let mut characters = rest.chars();
    loop {
        match characters.next() {
            Some('"') => return Ok(Some(PathBuf::from(value))),
            Some('\\') if characters.next() == Some('\\') => value.push('\\'),
            Some('\\') | None => return Err(format!("cannot read the field {name} of: {line}")),
            Some(character) => value.push(character),
        }
    }
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

/// Runs a tool to its end and returns what it printed; a failure carries the command and
/// the end of what it said on standard error.
fn run(command: &mut Command) -> Result<String, String> {
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|cause| format!("cannot run {command:?}: {cause}"))?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        let lines: Vec<&str> = said.lines().collect();
        let tail = lines[lines.len().saturating_sub(40)..].join("\n");
        return Err(format!("{command:?}: {}\n{tail}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|cause| format!("{command:?}: {cause}"))
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

    /// Windows's path as Cargo escapes it, on every platform: `\\` read, every other escape
    /// refused.
    #[test]
    fn a_windows_path_is_read_from_its_escaped_separators() {
        const OUT: &str = r#"{"reason":"build-script-executed","out_dir":"C:\\p\\out"}"#;
        const EXE: &str =
            r#"{"reason":"compiler-artifact","executable":"C:\\p\\release\\git-dupe.exe"}"#;
        assert_eq!(
            named(&[EXE, OUT].join("\n")).unwrap(),
            (
                PathBuf::from(r"C:\p\release\git-dupe.exe"),
                PathBuf::from(r"C:\p\out")
            )
        );
        let quoted = EXE.replace(r"C:\\p", r#"C:\"p"#);
        assert!(named(&[&quoted, OUT].join("\n")).is_err());
        let unknown = EXE.replace(r"C:\\p", r"C:\up");
        assert!(named(&[&unknown, OUT].join("\n")).is_err());
    }

    const BUILD_SCRIPT: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///p#0.1.0","target":{"kind":["custom-build"],"name":"build-script-build"},"executable":null,"fresh":true}"#;
    const EXECUTED: &str = r#"{"reason":"build-script-executed","package_id":"path+file:///p#0.1.0","linked_libs":[],"env":[],"out_dir":"/p/target/release/build/git-dupe-1/out"}"#;
    const BINARY: &str = r#"{"reason":"compiler-artifact","package_id":"path+file:///p#0.1.0","target":{"kind":["bin"],"name":"git-dupe"},"executable":"/p/target/release/git-dupe","fresh":false}"#;
    const FINISHED: &str = r#"{"reason":"build-finished","success":true}"#;

    #[test]
    fn the_executable_and_the_output_directory_are_the_ones_cargo_named() {
        let said = [BUILD_SCRIPT, EXECUTED, BINARY, FINISHED].join("\n");
        assert_eq!(
            named(&said).unwrap(),
            (
                PathBuf::from("/p/target/release/git-dupe"),
                PathBuf::from("/p/target/release/build/git-dupe-1/out")
            )
        );
    }

    #[test]
    fn a_build_that_names_neither_or_two_is_not_read() {
        for said in [
            [BUILD_SCRIPT, FINISHED].join("\n"),
            [BUILD_SCRIPT, EXECUTED, FINISHED].join("\n"),
            [EXECUTED, BINARY, BINARY, FINISHED].join("\n"),
            [EXECUTED, EXECUTED, BINARY].join("\n"),
            [
                EXECUTED,
                &BINARY.replace("release/git-dupe\"", "release/other\""),
            ]
            .join("\n"),
        ] {
            assert!(named(&said).is_err(), "{said}");
        }
        let escaped = BINARY.replace("/p/target", "/p\\\"q/target");
        assert!(named(&[EXECUTED, &escaped].join("\n")).is_err());
    }
}
