//! A ram disk for the second filesystem on macOS, without `sudo`.

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

    use super::super::unix::{device, second_filesystem};
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
