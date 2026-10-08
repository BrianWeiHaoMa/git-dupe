//! A program a scenario runs, a hook, an editor, or a `git` in place of the release's,
//! written by a process of its own.
//!
//! The scenarios run on many threads of one process. A descriptor this process holds open
//! to write a file is inherited by every child another thread forks meanwhile, and stays
//! open in that child until it runs its program: close-on-exec closes it only then. While
//! any process holds such a descriptor, running the file fails with `ETXTBSY`. Closing this
//! process's own descriptor first, or writing another file and renaming it into place,
//! leaves the same inode open in such a child; only a file this process never opens to
//! write is out of the way. So a shell started for it writes the file, the bytes fed on its
//! standard input, and the file can be run once that shell has ended.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::scenario::Scenario;

/// Writes `bytes` at `path`, below the scenario's directory, as a file of mode 0755,
/// making the directories above it first, and returns once it can be run. The path is one
/// word of the shell's and the bytes its standard input, so that neither is read as shell
/// text. Fails the scenario, naming the path, when the file cannot be written whole.
pub fn write_executable(s: &Scenario, path: &Path, bytes: &[u8]) {
    assert!(
        path.starts_with(s.dir()),
        "{} is outside the scenario's directory",
        path.display()
    );
    let parent = path
        .parent()
        .expect("a file below the scenario's directory");
    fs::create_dir_all(parent).unwrap_or_else(|cause| {
        panic!(
            "{}: cannot make the directories above it: {cause}",
            path.display()
        )
    });
    s.program(
        "/bin/sh",
        [
            OsStr::new("-c"),
            OsStr::new(r#"cat > "$0" && chmod 755 "$0""#),
            path.as_os_str(),
        ],
    )
    .input(bytes)
    .succeeds();
}

#[cfg(test)]
mod tests {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::super::output::End;
    use super::*;

    /// A path no shell reads as written, below directories that do not exist yet, and
    /// bytes of every value, more than a pipe holds: the file holds those bytes, mode 0755,
    /// and nothing named in the path ran. A shorter script written over it replaces it
    /// whole and runs as soon as the writer returns.
    #[test]
    fn the_file_is_written_whole_and_runs_at_once() {
        let s = Scenario::of_the_first_release();
        let name = OsStr::from_bytes(b"a b'c\"d $(touch ran) `touch ran`; * \xff");
        let path = s.dir().join("missing/below").join(name);
        let bytes: Vec<u8> = (0..1u32 << 20).map(|n| (n % 256) as u8).collect();
        write_executable(&s, &path, &bytes);
        assert!(
            fs::read(&path).unwrap() == bytes,
            "other bytes were written"
        );
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o755);
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>(),
            [name]
        );
        assert!(
            !s.dir().join("ran").exists(),
            "the path was read as shell text"
        );

        let script = b"#!/bin/sh\nprintf 'ran %s\\n' \"$#\"\n";
        write_executable(&s, &path, script);
        assert_eq!(fs::read(&path).unwrap(), script);
        let ran = s.program(&path, ["word"]).run();
        assert_eq!(ran.end, End::Code(0), "{ran:?}");
        assert_eq!(ran.stdout, b"ran 1\n");
    }

    /// A destination that cannot be written fails the writer, naming it: a directory
    /// standing at it, whether the bytes fit in a pipe or not, and a file standing where a
    /// directory above it must be made. What stood there is left as it was.
    #[test]
    fn a_file_that_cannot_be_written_fails_the_scenario() {
        let s = Scenario::of_the_first_release();
        let directory = s.dir().join("a-directory");
        fs::create_dir(&directory).unwrap();
        let file = s.dir().join("a-file");
        fs::write(&file, b"data\n").unwrap();
        let script = b"#!/bin/sh\n".to_vec();
        for (path, bytes) in [
            (directory.clone(), script.clone()),
            (directory.clone(), vec![b'x'; 1 << 20]),
            (file.join("run"), script),
        ] {
            let failed = catch_unwind(AssertUnwindSafe(|| write_executable(&s, &path, &bytes)));
            let cause = failed.expect_err("a file was written where none can be");
            let said = cause
                .downcast_ref::<String>()
                .expect("a failure that says what failed");
            assert!(
                said.contains(path.to_str().unwrap()),
                "the failure does not name {}: {said}",
                path.display()
            );
        }
        assert!(fs::symlink_metadata(&directory).unwrap().is_dir());
        assert_eq!(fs::read(&file).unwrap(), b"data\n");
    }
}
