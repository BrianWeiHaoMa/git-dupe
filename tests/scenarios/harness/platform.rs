//! What differs between the platforms the checks run on, Linux and macOS: the one file of
//! the repository conditional on the platform, because the executable holds no such code
//! (R11) and a scenario names no platform (`Composition/The checks`). A tool that answers
//! alike on both, `ps` or `man` given a page's path, is run where its purpose lives, not
//! here.
//!
//! The platform's word and the machine's architecture together name the release archive
//! (P3): its name takes both from here and spells neither itself.
//!
//! The second filesystem that F5's refusal across filesystems needs: a fresh directory on
//! `/dev/shm` on Linux, which a machine may lack, and on macOS on a ram disk attached for
//! it, without `sudo`, and detached when it is dropped, which every macOS has.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use super::fresh::FreshDirectory;

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

/// The number of `EILSEQ`, the error with which a filesystem refuses a name that is no
/// valid encoding, as macOS's APFS refuses one whose bytes are no UTF-8; the standard
/// library gives it no kind of its own.
#[cfg(target_os = "linux")]
pub const EILSEQ: i32 = 84;
#[cfg(target_os = "macos")]
pub const EILSEQ: i32 = 92;

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

/// On Linux, a fresh directory on `/dev/shm`; the reason, for the caller to report, where
/// this machine has none there that can be written and lies on another device.
#[cfg(target_os = "linux")]
pub fn second_filesystem() -> Result<SecondFilesystem, String> {
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
pub fn second_filesystem() -> Result<SecondFilesystem, String> {
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

fn device(path: &Path) -> u64 {
    fs::metadata(path)
        .unwrap_or_else(|cause| panic!("{}: {cause}", path.display()))
        .dev()
}

#[cfg(target_os = "macos")]
mod ram_disk {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A ram disk attached by this process, detached when dropped.
    pub struct RamDisk {
        /// `/dev/diskN`, as `hdiutil attach` answered.
        device: String,
        /// Where its volume is mounted, `/Volumes/<name>`.
        mount: PathBuf,
    }

    impl RamDisk {
        /// A ram disk of 32 MiB, attached, then formatted as `format` and mounted under a
        /// volume name of this process's that nothing stands at yet, so that two ram disks
        /// never share a mount point. The disk is detached from the moment `hdiutil attach`
        /// answers whatever fails after: a failed erase detaches it, then fails the check
        /// naming the command, what it printed, and how the detach went.
        pub fn attached(format: &str) -> RamDisk {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let (name, mount) = loop {
                let name = format!(
                    "git-dupe-checks-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                );
                let mount = Path::new("/Volumes").join(&name);
                if std::fs::symlink_metadata(&mount).is_err() {
                    break (name, mount);
                }
            };
            let attached = run("hdiutil", &["attach", "-nomount", "ram://65536"])
                .unwrap_or_else(|cause| panic!("{cause}"));
            let device = attached
                .split_whitespace()
                .next()
                .filter(|device| device.starts_with("/dev/disk"))
                .unwrap_or_else(|| panic!("hdiutil attach named no device: {attached:?}"))
                .to_owned();
            let disk = RamDisk { device, mount };
            if let Err(cause) = run("diskutil", &["eraseVolume", format, &name, &disk.device]) {
                let detached = disk.detach();
                // Detached already: dropped, it would be detached a second time.
                std::mem::forget(disk);
                panic!("{cause}; {}", detached.unwrap_or_else(|why| why));
            }
            disk
        }

        pub fn mount(&self) -> &Path {
            &self.mount
        }

        /// Detaches the disk: what was done, or why it was not.
        fn detach(&self) -> Result<String, String> {
            run("hdiutil", &["detach", &self.device])
                .map(|_| format!("{} detached", self.device))
                .map_err(|cause| format!("{} left attached: {cause}", self.device))
        }
    }

    impl Drop for RamDisk {
        /// A ram disk left attached fails the check that held it; while that check is
        /// already failing, the cause is printed instead, because a second panic would
        /// abort every check of the process.
        fn drop(&mut self) {
            if let Err(cause) = self.detach() {
                match std::thread::panicking() {
                    true => eprintln!("{cause}"),
                    false => panic!("{cause}"),
                }
            }
        }
    }

    /// What `program <words>` printed when it exited 0; otherwise the command, its
    /// status, and what it printed.
    fn run(program: &str, words: &[&str]) -> Result<String, String> {
        let ran = Command::new(program)
            .args(words)
            .output()
            .map_err(|cause| format!("cannot run {program}: {cause}"))?;
        let printed = String::from_utf8_lossy(&ran.stdout).into_owned();
        match ran.status.success() {
            true => Ok(printed),
            false => Err(format!(
                "{program} {words:?} {}: {printed}{}",
                ran.status,
                String::from_utf8_lossy(&ran.stderr)
            )),
        }
    }

    #[cfg(test)]
    mod tests {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        use std::thread;

        use super::super::{device, second_filesystem};
        use super::*;

        /// Two second filesystems alive at once, from two threads, are two ram disks at two
        /// mount points; each is gone once dropped, the one a failing check held while it
        /// unwound included.
        #[test]
        fn every_ram_disk_is_detached_once_dropped_even_by_a_failing_check() {
            let (one, two) = thread::scope(|scope| {
                let one = scope.spawn(second_filesystem);
                let two = scope.spawn(second_filesystem);
                (one.join().unwrap().unwrap(), two.join().unwrap().unwrap())
            });
            assert_ne!(one.disk.device, two.disk.device);
            assert_ne!(one.disk.mount, two.disk.mount);
            assert_ne!(device(one.path()), device(two.path()));
            let mounts = [one.disk.mount.clone(), two.disk.mount.clone()];
            let failed = catch_unwind(AssertUnwindSafe(move || {
                let _held = one;
                panic!("a check fails while it holds a second filesystem");
            }));
            assert!(failed.is_err());
            assert!(
                !mounts[0].exists(),
                "{} is still mounted",
                mounts[0].display()
            );
            drop(two);
            assert!(
                !mounts[1].exists(),
                "{} is still mounted",
                mounts[1].display()
            );
        }

        /// A ram disk that cannot be detached when it is dropped, because a process works in
        /// its volume, fails the check that held it, naming it; detached once that process
        /// is gone.
        #[test]
        fn a_ram_disk_left_attached_fails_the_check_that_held_it() {
            let second = second_filesystem().unwrap();
            let device = second.disk.device.clone();
            let mut holder = Command::new("sleep")
                .arg("60")
                .current_dir(&second.disk.mount)
                .spawn()
                .unwrap();
            let failed = catch_unwind(AssertUnwindSafe(move || drop(second)));
            let _ = holder.kill();
            let _ = holder.wait();
            let detached = run("hdiutil", &["detach", &device]);
            let Err(failed) = failed else {
                panic!("{device} was detached while a process worked in its volume");
            };
            let said = failed
                .downcast_ref::<String>()
                .expect("a failure that says why");
            assert!(said.contains(&format!("{device} left attached")), "{said}");
            detached.unwrap_or_else(|cause| panic!("{cause}"));
        }

        /// A ram disk that `diskutil` refuses to format is detached before the check fails,
        /// and the failure says so.
        #[test]
        fn a_ram_disk_that_cannot_be_formatted_is_detached_before_the_failure() {
            let Err(failed) = catch_unwind(|| RamDisk::attached("No Such Format")) else {
                panic!("a ram disk formatted as no format");
            };
            let said = failed
                .downcast_ref::<String>()
                .expect("a failure that says why");
            assert!(
                said.contains("diskutil") && said.contains(" detached"),
                "{said}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::architecture;

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
}
