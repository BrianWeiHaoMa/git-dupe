//! What differs between the platforms the checks run on, Linux, macOS, and Windows: the one
//! place under `tests/` conditional on the platform, because the executable holds no such
//! code but R11's two sites and a scenario names no platform except to report itself, or a
//! part of itself, unavailable (`Composition/The checks`). This file holds the Linux and
//! macOS arms, `platform/windows.rs` the Windows ones, and every item answers on every
//! platform; a harness module branches on an answer here, never on anything else about the
//! platform. `src/links_and_modes.rs` is the executable's own such place, for its unit
//! checks, and its items read as these do.
//!
//! The platform's word and the machine's architecture together name the release archive
//! (P3): its name takes both from here and spells neither itself. So do the machines no
//! archive is built for, for a check that pretends to be one.
//!
//! The second filesystem that F5's refusal across filesystems needs, one entry per way this
//! platform reaches one: a fresh directory on `/dev/shm` on Linux, which a machine may lack,
//! on macOS on a ram disk attached for it, without `sudo`, and detached when it is dropped,
//! which every macOS has, and on Windows on a second fixed NTFS volume.

use std::path::{MAIN_SEPARATOR, Path};

use super::fresh::FreshDirectory;

#[cfg(target_os = "macos")]
mod ram_disk;
#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;
#[cfg(windows)]
mod windows;

/// What `uname -s` and `uname -m` print on machines no release archive is built for (F1,
/// N10, P3): Linux on 64-bit Arm, and an operating system git-dupe does not run on.
pub const MACHINES_WITHOUT_AN_ARCHIVE: [(&str, &str); 2] =
    [("Linux", "aarch64"), ("FreeBSD", "amd64")];

/// `path` as Git spells it, for a comparison with a path Git printed or a git-dupe line
/// named, or for a file Git reads: `/` for every separator, the drive letter as the path
/// holds it (S23: Git prints `C:/x`). Panics on a path that is not UTF-8.
pub fn as_git_spells(path: &Path) -> String {
    let text = path.to_str().expect("a UTF-8 path to spell as Git does");
    text.replace(MAIN_SEPARATOR, "/")
}

/// The same for a path a case holds in bytes that may be no UTF-8, off Windows, where no
/// byte of a name is a separator but `/` itself.
pub fn as_git_spells_bytes(path: &Path) -> Vec<u8> {
    let bytes = path.as_os_str().as_encoded_bytes();
    let spelled = bytes.iter().map(|byte| match *byte as char {
        MAIN_SEPARATOR => b'/',
        _ => *byte,
    });
    spelled.collect()
}

/// Whether `\` separates a path's components here (G29), so that a word holding it says
/// nothing about a pattern.
pub fn backslash_separates() -> bool {
    std::path::is_separator('\\')
}

/// Removes the symbolic link at `at`, which on Windows is a directory's where its target
/// was one.
pub fn remove_link(at: impl AsRef<Path>) -> std::io::Result<()> {
    let at = at.as_ref();
    std::fs::remove_file(at).or_else(|_| std::fs::remove_dir(at))
}

/// A fresh, empty directory, its path canonical, on a local filesystem whose device is not
/// the one under which `FreshDirectory::create` makes the scenarios' directories. Dropped,
/// the directory is removed and, on macOS, its ram disk detached: no path on it is used
/// after that.
pub struct SecondFilesystem {
    // Fields drop in their order: the directory before the disk that holds it.
    directory: FreshDirectory,
    #[cfg(target_os = "macos")]
    disk: ram_disk::RamDisk,
}

impl SecondFilesystem {
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

/// One way this platform reaches a second filesystem, and what it found there: the
/// filesystem, or the reason none can be had, for the caller to report.
pub struct SecondFilesystemFound {
    pub reached: &'static str,
    pub found: Result<SecondFilesystem, String>,
}

#[cfg(unix)]
mod unix {
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::io;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, ExitStatus};

    #[cfg(target_os = "macos")]
    use super::ram_disk;
    use super::{FreshDirectory, SecondFilesystem, SecondFilesystemFound};

    /// The platform the checks run on, for a check that holds on one of them alone: `linux`.
    #[cfg(target_os = "linux")]
    pub fn word() -> &'static str {
        "linux"
    }

    /// The platform the checks run on, for a check that holds on one of them alone: `macos`.
    #[cfg(target_os = "macos")]
    pub fn word() -> &'static str {
        "macos"
    }

    /// The machine's architecture as `uname -m` prints it, without its newline: `x86_64`, or
    /// `arm64` on Apple silicon, where the compiler's own name for it is `aarch64` (P3).
    pub fn architecture() -> String {
        let ran = Command::new("uname")
            .arg("-m")
            .output()
            .unwrap_or_else(|cause| panic!("cannot run uname -m: {cause}"));
        let printed = String::from_utf8_lossy(&ran.stdout);
        let architecture = printed.strip_suffix('\n').unwrap_or(&printed);
        assert!(
            ran.status.success() && !architecture.is_empty(),
            "uname -m {}: {printed:?}{}",
            ran.status,
            String::from_utf8_lossy(&ran.stderr)
        );
        architecture.to_owned()
    }

    /// The name these bytes spell: every byte string is one here.
    pub fn name(bytes: &[u8]) -> Option<OsString> {
        Some(OsStr::from_bytes(bytes).to_owned())
    }

    /// The real path of `path`: a path Git accepts as a word as it stands.
    pub fn canonical(path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    /// A symbolic link at `at` to `target`.
    pub fn symlink(target: impl AsRef<Path>, at: impl AsRef<Path>) -> io::Result<()> {
        std::os::unix::fs::symlink(target, at)
    }

    /// The device and inode of what stands at `path`, by `lstat`: the same pair after a
    /// command is the same file, and a rename onto `path` changes it.
    pub fn identity(path: &Path) -> (u64, u64) {
        let found = fs::symlink_metadata(path)
            .unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
        (found.dev(), found.ino())
    }

    /// The permission bits of what stands at `path`, by `lstat`: a symbolic link's own.
    pub fn mode(path: &Path) -> Option<u32> {
        let found = fs::symlink_metadata(path)
            .unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
        Some(found.permissions().mode() & 0o7777)
    }

    /// Permissions holding the bits `mode`.
    pub fn permissions(mode: u32) -> Option<fs::Permissions> {
        Some(fs::Permissions::from_mode(mode))
    }

    /// Whether a child can end by a signal here.
    pub fn signals() -> bool {
        true
    }

    /// The signal that ended a child, where one did.
    pub fn signal_of(status: &ExitStatus) -> Option<i32> {
        status.signal()
    }

    /// Whether Git reaches a linked worktree's common Git directory through a symbolic link
    /// above its Git directory, as the kernel reads the `..` of its `commondir` file.
    pub fn commondir_read_through_a_link() -> bool {
        true
    }

    /// Whether a run can be started under a soft `RLIMIT_FSIZE`.
    pub fn file_size_limits() -> bool {
        true
    }

    /// A handle on the file at `path`, which stops no removal here.
    pub fn held_from_deletion(path: &Path) -> fs::File {
        fs::File::open(path).unwrap_or_else(|cause| panic!("{}: {cause}", path.display()))
    }

    /// Whether a Git reading `.git/info/exclude` can fail at the moment another command
    /// replaces it: not here, where a rename gives a reader the old file or the new (S1).
    pub fn a_replacement_can_fail_a_reader() -> bool {
        false
    }

    /// `getconf ARG_MAX`: what one command line may carry here.
    pub fn command_line_limit() -> usize {
        let limit = Command::new("getconf").arg("ARG_MAX").output().unwrap();
        assert!(limit.status.success());
        let printed = String::from_utf8(limit.stdout).unwrap();
        printed.trim().parse().unwrap()
    }

    /// The number of `EILSEQ`, the error with which a filesystem refuses a name that is no
    /// valid encoding, as macOS's APFS refuses one whose bytes are no UTF-8; the standard
    /// library gives it no kind of its own.
    #[cfg(target_os = "linux")]
    const EILSEQ: i32 = 84;
    #[cfg(target_os = "macos")]
    const EILSEQ: i32 = 92;

    /// `Ok` once an empty entry named `name` was made directly in `dir`, where nothing stands
    /// at that name, and removed; the reason where the filesystem refuses the name as no
    /// valid encoding (`EILSEQ`). Any other failure fails the scenario.
    pub fn name_accepted(dir: &Path, name: &[u8]) -> Result<(), String> {
        let path = dir.join(OsStr::from_bytes(name));
        let made = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        match made {
            Ok(_) => {
                fs::remove_file(&path)
                    .unwrap_or_else(|cause| panic!("{}: {cause}", path.display()));
                Ok(())
            }
            Err(cause) if cause.raw_os_error() == Some(EILSEQ) => Err(format!(
                "the name \"{}\" is refused here ({cause})",
                name.escape_ascii()
            )),
            Err(cause) => panic!("{}: {cause}", path.display()),
        }
    }

    /// The directories that lead a scenario's `PATH`, in order.
    pub fn release_programs(release: &Path) -> Vec<PathBuf> {
        vec![release.join("bin")]
    }

    /// The release's own `git` program.
    pub fn release_git(release: &Path) -> PathBuf {
        release.join("bin").join("git")
    }

    /// The shell a scenario runs a command line through.
    pub fn shell(_release: &Path) -> PathBuf {
        PathBuf::from("/bin/sh")
    }

    /// What `git --version` prints under the listed release `version`.
    pub fn git_version_line(version: &str) -> String {
        format!("git version {version}\n")
    }

    /// The variables of the caller's a scenario's environment needs: none here.
    pub fn environment_passed_through() -> &'static [&'static str] {
        &[]
    }

    /// The variables a scenario's environment sets for this platform: none here.
    pub fn environment_set() -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// A process group of its own, led by the run.
    pub struct Group {
        leader: u32,
    }

    impl Group {
        /// The processes of the group that have not ended, each as its row of
        /// `ps -A -o pid=,stat=,pgid=,comm=`: the process, its state, its group, and its
        /// name.
        pub fn running(&self) -> Result<Vec<String>, String> {
            let rows = ps(&["-A", "-o", "pid=,stat=,pgid=,comm="])?;
            Ok(rows
                .lines()
                .filter(|row| {
                    let mut columns = row.split_whitespace();
                    let (_, state, of) = (columns.next(), columns.next(), columns.next());
                    of == Some(self.leader.to_string().as_str())
                        && state.is_some_and(|state| !ended_in(state))
                })
                .map(|row| row.trim().to_owned())
                .collect())
        }

        /// Kills every process of the group at once.
        pub fn kill(&self) {
            let _ = Command::new("/bin/sh")
                .args(["-c", r#"kill -s KILL -- "-$0" 2>/dev/null"#])
                .arg(self.leader.to_string())
                .status();
        }
    }

    /// Starts `command` in a process group of its own.
    pub fn spawn_grouped(command: &mut Command) -> io::Result<(Child, Group)> {
        command.process_group(0);
        let child = command.spawn()?;
        let group = Group { leader: child.id() };
        Ok((child, group))
    }

    /// A process whose number stays its own here for as long as anyone looks.
    pub struct HeldProcess(String);

    impl HeldProcess {
        /// Whether the process has ended.
        pub fn ended(&self) -> bool {
            ended(&self.0)
        }
    }

    /// `process`, which runs.
    pub fn process_held(process: &str) -> HeldProcess {
        HeldProcess(process.to_owned())
    }

    /// A command of the shell's printing the process its parameter `$<parameter>` names, as
    /// `ended` reads it.
    pub fn shell_process_number(parameter: &str) -> String {
        format!(r#"echo "${parameter}""#)
    }

    /// A process that lives `seconds` and ends by itself.
    pub fn sleeping(seconds: u32) -> Command {
        let mut sleep = Command::new("sleep");
        sleep.arg(seconds.to_string());
        sleep
    }

    /// None: the stand-in `git` here is the script its caller writes.
    pub fn stand_in_git_program() -> Option<&'static Path> {
        None
    }

    /// Whether the process `process` has ended: gone, or a zombie.
    pub fn ended(process: &str) -> bool {
        let state = ps(&["-o", "stat=", "-p", process]).unwrap_or_else(|cause| panic!("{cause}"));
        state.trim().is_empty() || ended_in(state.trim())
    }

    /// Whether a state `ps` prints is a zombie's, whatever letters follow its first: `Z` on
    /// Linux, `Z`, `Z+`, or `Zs` on macOS.
    fn ended_in(state: &str) -> bool {
        state.starts_with('Z')
    }

    /// What `ps <words>` printed: `ps` answers alike on Linux and macOS for the columns read
    /// here, never Linux's process filesystem, which macOS lacks, and `ps -g` is not used,
    /// which selects a group on macOS and a session on Linux. `ps` exits 1 and prints nothing
    /// on either stream when the processes it was asked about are gone, which is an answer;
    /// any other status, and anything on standard error, is a failure, never an empty answer.
    fn ps(words: &[&str]) -> Result<String, String> {
        let ps = Command::new("ps")
            .args(words)
            .output()
            .map_err(|cause| format!("cannot run ps {words:?}: {cause}"))?;
        let printed = String::from_utf8_lossy(&ps.stdout).into_owned();
        match (ps.status.code(), ps.stderr.is_empty()) {
            (Some(0), true) => Ok(printed),
            (Some(1), true) if printed.trim().is_empty() => Ok(printed),
            _ => Err(format!(
                "ps {words:?} {}: {}",
                ps.status,
                String::from_utf8_lossy(&ps.stderr)
            )),
        }
    }

    /// Each way of this platform: on Linux, `/dev/shm`.
    #[cfg(target_os = "linux")]
    pub fn second_filesystems() -> Vec<SecondFilesystemFound> {
        vec![SecondFilesystemFound {
            reached: "on /dev/shm",
            found: second_filesystem(),
        }]
    }

    /// Each way of this platform: on macOS, a ram disk.
    #[cfg(target_os = "macos")]
    pub fn second_filesystems() -> Vec<SecondFilesystemFound> {
        vec![SecondFilesystemFound {
            reached: "on a ram disk",
            found: second_filesystem(),
        }]
    }

    /// On Linux, a fresh directory on `/dev/shm`; the reason, for the caller to report, where
    /// this machine has none there that can be written and lies on another device.
    #[cfg(target_os = "linux")]
    fn second_filesystem() -> Result<SecondFilesystem, String> {
        let base = Path::new("/dev/shm");
        let directory = FreshDirectory::create_in(base)
            .ok_or_else(|| format!("no fresh directory can be made at {}", base.display()))?;
        if device(directory.path()) == device(&std::env::temp_dir()) {
            return Err(format!(
                "{} lies on the device of the scenarios' directories",
                base.display()
            ));
        }
        Ok(SecondFilesystem { directory })
    }

    /// On macOS, a fresh directory on a ram disk of its own, case-sensitive APFS. A ram disk
    /// that cannot be attached, formatted, or used fails the check, naming the command and
    /// what it printed: it is part of every macOS, so it is never reported unavailable.
    #[cfg(target_os = "macos")]
    pub(super) fn second_filesystem() -> Result<SecondFilesystem, String> {
        let disk = ram_disk::RamDisk::attached("Case-sensitive APFS");
        let directory = FreshDirectory::create_in(disk.mount()).unwrap_or_else(|| {
            panic!(
                "no fresh directory can be made on the ram disk at {}",
                disk.mount().display()
            )
        });
        assert_ne!(
            device(directory.path()),
            device(&std::env::temp_dir()),
            "the ram disk at {} lies on the device of the scenarios' directories",
            disk.mount().display()
        );
        Ok(SecondFilesystem { directory, disk })
    }

    pub(super) fn device(path: &Path) -> u64 {
        fs::metadata(path)
            .unwrap_or_else(|cause| panic!("{}: {cause}", path.display()))
            .dev()
    }

    #[cfg(test)]
    mod tests {
        use std::process::Command;

        use super::*;

        /// One word, as the archive's name needs it, and the machine's own: what `uname -m`
        /// prints, asked through the shell, with nothing but its newline removed.
        #[test]
        fn the_architecture_is_one_word_as_uname_prints_it() {
            let architecture = architecture();
            assert!(
                !architecture.is_empty() && !architecture.contains(char::is_whitespace),
                "{architecture:?}"
            );
            let asked = Command::new("/bin/sh")
                .args(["-c", "uname -m"])
                .output()
                .unwrap();
            assert!(asked.status.success(), "{asked:?}");
            assert_eq!(format!("{architecture}\n").into_bytes(), asked.stdout);
        }

        /// The limit is the kernel's own, asked through the shell.
        #[test]
        fn the_command_line_limit_is_what_getconf_prints() {
            let asked = Command::new("/bin/sh")
                .args(["-c", "getconf ARG_MAX"])
                .output()
                .unwrap();
            assert!(asked.status.success(), "{asked:?}");
            assert_eq!(
                format!("{}\n", command_line_limit()).into_bytes(),
                asked.stdout
            );
        }

        /// `ps` refusing what it was asked is no answer: a process it says nothing about is
        /// gone only where it says nothing on either stream, as for a process that has ended.
        #[test]
        fn a_failing_ps_is_no_answer_about_a_process() {
            assert!(ps(&["-o", "stat=", "-p", "not-a-process"]).is_err());
            let mut child = Command::new("/bin/sh")
                .args(["-c", "exit 0"])
                .spawn()
                .unwrap();
            let process = child.id().to_string();
            child.wait().unwrap();
            assert_eq!(ps(&["-o", "stat=", "-p", &process]).as_deref(), Ok(""));
            assert!(ended(&process));
        }

        /// A command that starts a long-lived child and exits leaves the group running: the
        /// child stands in `running` until `kill` ends it, and `ended` then turns true.
        #[test]
        fn a_group_is_named_until_it_is_killed() {
            let mut command = Command::new("/bin/sh");
            command
                .args(["-c", "sleep 120 >/dev/null 2>&1 & echo $!"])
                .stdout(std::process::Stdio::piped());
            let (child, group) = spawn_grouped(&mut command).unwrap();
            let printed = child.wait_with_output().unwrap();
            let left = String::from_utf8(printed.stdout).unwrap();
            let left = left.trim_end();
            let rows = group.running().unwrap();
            assert!(
                rows.iter().any(|row| row.starts_with(left)) && !ended(left),
                "{left}: {rows:?}"
            );
            group.kill();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while !group.running().unwrap().is_empty() {
                assert!(std::time::Instant::now() < deadline, "the child still runs");
            }
            assert!(ended(left));
        }
    }
}

#[cfg(test)]
mod tests;
